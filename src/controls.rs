//! Adapte le clavier configurable et les manettes en actions communes au jeu et aux menus.

use gilrs::{Axis, Button, EventType, GamepadId, Gilrs};
use macroquad::prelude::{is_key_down, is_key_pressed, KeyCode};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::game::Difficulty;
use crate::render::{EffectsProfile, VisualSettings};

// Les noms JSON sont stables et lisibles, sans dependance aux codes numeriques du backend.
macro_rules! binding_keys {
    ($($name:ident => $code:ident),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub enum Key { $($name),+ }

        impl Key {
            pub fn code(self) -> KeyCode {
                match self { $(Self::$name => KeyCode::$code),+ }
            }

            pub fn from_code(code: KeyCode) -> Option<Self> {
                match code { $(KeyCode::$code => Some(Self::$name)),+, _ => None }
            }

            pub fn label(self) -> &'static str {
                match self { $(Self::$name => stringify!($name)),+ }
            }
        }
    };
}

binding_keys! {
    A => A, B => B, C => C, D => D, E => E, F => F, G => G, H => H,
    I => I, J => J, K => K, L => L, M => M, N => N, O => O, P => P,
    Q => Q, R => R, S => S, T => T, U => U, V => V, W => W, X => X, Y => Y, Z => Z,
    Digit0 => Key0, Digit1 => Key1, Digit2 => Key2, Digit3 => Key3, Digit4 => Key4,
    Digit5 => Key5, Digit6 => Key6, Digit7 => Key7, Digit8 => Key8, Digit9 => Key9,
    Espace => Space, Entree => Enter, Tab => Tab, Echap => Escape,
    Gauche => Left, Droite => Right, Haut => Up, Bas => Down,
    MajG => LeftShift, MajD => RightShift, CtrlG => LeftControl, CtrlD => RightControl,
    AltG => LeftAlt, AltD => RightAlt, Retour => Backspace,
    F1 => F1, F2 => F2, F3 => F3, F4 => F4, F5 => F5, F6 => F6,
    F7 => F7, F8 => F8, F9 => F9, F10 => F10, F11 => F11, F12 => F12,
    Inser => Insert, Suppr => Delete, Debut => Home, Fin => End,
    PageHaut => PageUp, PageBas => PageDown,
    Virgule => Comma, Point => Period, Slash => Slash, Moins => Minus, Egal => Equal,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Left,
    Right,
    Jump,
    Attack,
    NextWeapon,
    Dash,
    Interact,
    Map,
    Pause,
    Fullscreen,
}

impl Action {
    pub const ALL: [Self; 10] = [
        Self::Left,
        Self::Right,
        Self::Jump,
        Self::Attack,
        Self::NextWeapon,
        Self::Dash,
        Self::Interact,
        Self::Map,
        Self::Pause,
        Self::Fullscreen,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "MARCHER A GAUCHE",
            Self::Right => "MARCHER A DROITE",
            Self::Jump => "SAUTER",
            Self::Attack => "FRAPPER",
            Self::NextWeapon => "ARME SUIVANTE",
            Self::Dash => "RUEE",
            Self::Interact => "INTERAGIR",
            Self::Map => "CARTE",
            Self::Pause => "PAUSE",
            Self::Fullscreen => "PLEIN ECRAN",
        }
    }
}

/// Configuration durable distincte de la progression ; les champs absents gardent leurs valeurs par defaut.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Bindings {
    pub left: Vec<Key>,
    pub right: Vec<Key>,
    pub jump: Vec<Key>,
    pub attack: Vec<Key>,
    pub next_weapon: Vec<Key>,
    pub dash: Vec<Key>,
    pub interact: Vec<Key>,
    pub map: Vec<Key>,
    pub pause: Vec<Key>,
    pub fullscreen: Vec<Key>,
    // Les anciens fichiers sans volumes conservent les valeurs de Default.
    pub music_volume: f32,
    pub sfx_volume: f32,
    // Reglages d'affichage : absents des anciens fichiers, ils gardent le rendu par defaut.
    pub fx: EffectsProfile,
    pub reduced_motion: bool,
    pub flat: bool,
    pub difficulty: Difficulty,
}

impl Default for Bindings {
    fn default() -> Self {
        Self {
            left: vec![Key::Gauche, Key::A, Key::Q],
            right: vec![Key::Droite, Key::D],
            jump: vec![Key::Espace, Key::Haut, Key::W, Key::Z],
            attack: vec![Key::J, Key::X],
            next_weapon: vec![Key::R],
            dash: vec![Key::MajG, Key::MajD, Key::K, Key::C],
            interact: vec![Key::E, Key::Entree],
            map: vec![Key::Tab],
            pause: vec![Key::Echap],
            fullscreen: vec![Key::F11],
            music_volume: 0.45,
            sfx_volume: 0.65,
            fx: EffectsProfile::default(),
            reduced_motion: false,
            flat: false,
            difficulty: Difficulty::default(),
        }
    }
}

impl Bindings {
    pub fn keys(&self, action: Action) -> &[Key] {
        match action {
            Action::Left => &self.left,
            Action::Right => &self.right,
            Action::Jump => &self.jump,
            Action::Attack => &self.attack,
            Action::NextWeapon => &self.next_weapon,
            Action::Dash => &self.dash,
            Action::Interact => &self.interact,
            Action::Map => &self.map,
            Action::Pause => &self.pause,
            Action::Fullscreen => &self.fullscreen,
        }
    }

    pub fn pressed(&self, action: Action) -> bool {
        self.keys(action)
            .iter()
            .any(|key| is_key_pressed(key.code()))
    }

    pub fn down(&self, action: Action) -> bool {
        self.keys(action).iter().any(|key| is_key_down(key.code()))
    }

    // Un remappage remplace les alias de l'action ; les autres actions restent inchangees.
    pub fn assign(&mut self, action: Action, key: Key) -> Result<(), &'static str> {
        if key == Key::Echap && action != Action::Pause {
            return Err("ECHAP RESERVE AU RETOUR");
        }
        if Action::ALL
            .iter()
            .any(|other| *other != action && self.keys(*other).contains(&key))
        {
            return Err("TOUCHE DEJA ATTRIBUEE");
        }
        let keys = match action {
            Action::Left => &mut self.left,
            Action::Right => &mut self.right,
            Action::Jump => &mut self.jump,
            Action::Attack => &mut self.attack,
            Action::NextWeapon => &mut self.next_weapon,
            Action::Dash => &mut self.dash,
            Action::Interact => &mut self.interact,
            Action::Map => &mut self.map,
            Action::Pause => &mut self.pause,
            Action::Fullscreen => &mut self.fullscreen,
        };
        *keys = vec![key];
        Ok(())
    }

    // Valider aussi les fichiers edites a la main pour eviter actions inaccessibles et conflits.
    pub fn validate(&self) -> Result<(), &'static str> {
        for volume in [self.music_volume, self.sfx_volume] {
            if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
                return Err("VOLUME INVALIDE");
            }
        }
        for action in Action::ALL {
            let keys = self.keys(action);
            if keys.is_empty() || keys.len() > 8 {
                return Err("CONFIGURATION DE TOUCHES INVALIDE");
            }
            for key in keys {
                if *key == Key::Echap && action != Action::Pause {
                    return Err("ECHAP RESERVE AU RETOUR");
                }
                if Action::ALL
                    .iter()
                    .any(|other| *other != action && self.keys(*other).contains(key))
                {
                    return Err("TOUCHE DEJA ATTRIBUEE");
                }
            }
        }
        Ok(())
    }

    // Un volume nul coupe le canal ; borner les pas de l'interface entre 0 et 100 pour cent.
    pub fn adjust_volume(&mut self, music: bool, change: f32) {
        let volume = if music {
            &mut self.music_volume
        } else {
            &mut self.sfx_volume
        };
        *volume = (*volume + change).clamp(0.0, 1.0);
    }

    pub fn visuals(&self) -> VisualSettings {
        VisualSettings {
            profile: self.fx,
            reduced_motion: self.reduced_motion,
            flat: self.flat,
        }
    }

    /// L'ordre suit l'affichage du menu : calme, normal, epreuve.
    pub fn cycle_difficulty(&mut self, forward: bool) {
        const ORDER: [Difficulty; 3] = [Difficulty::Calm, Difficulty::Normal, Difficulty::Hard];
        let current = ORDER
            .iter()
            .position(|level| *level == self.difficulty)
            .unwrap_or(1);
        self.difficulty =
            ORDER[(current + if forward { 1 } else { ORDER.len() - 1 }) % ORDER.len()];
    }

    /// L'ordre suit l'affichage du menu : cinema, retro, simple.
    pub fn cycle_fx(&mut self, forward: bool) {
        const ORDER: [EffectsProfile; 3] = [
            EffectsProfile::Cinematic,
            EffectsProfile::Retro,
            EffectsProfile::Clean,
        ];
        let current = ORDER.iter().position(|fx| *fx == self.fx).unwrap_or(0);
        self.fx = ORDER[(current + if forward { 1 } else { ORDER.len() - 1 }) % ORDER.len()];
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default())
            }
            Err(error) => return Err(error.to_string()),
        };
        let bindings: Self = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        bindings.validate().map_err(str::to_string)?;
        Ok(bindings)
    }

    pub fn save(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        self.validate()?;
        std::fs::create_dir_all(path.parent().ok_or("invalid config path")?)?;
        // Le remplacement ne se fait qu'une fois le JSON entier ecrit dans le fichier temporaire.
        let temporary = path.with_extension("tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(temporary, path)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Default)]
struct State {
    direction: f32,
    jump: bool,
    attack: bool,
    next_weapon: bool,
    dash: bool,
    interact: bool,
    pause: bool,
    map: bool,
    up: bool,
    down: bool,
    back: bool,
}

#[derive(Default)]
/// Direction continue et boutons ponctuels, a lire une fois par image.
pub struct PadInput {
    pub direction: f32,
    pub jump: bool,
    pub attack: bool,
    pub attack_held: bool,
    pub next_weapon: bool,
    pub dash: bool,
    pub interact: bool,
    pub pause: bool,
    pub map: bool,
    pub up: bool,
    pub down: bool,
    pub back: bool,
}

impl PadInput {
    fn press(&mut self, button: Button) {
        match button {
            Button::South => self.jump = true,
            Button::West => self.attack = true,
            Button::RightTrigger2 => self.next_weapon = true,
            Button::East => {
                // Le contexte choisit entre la ruee en jeu et le retour dans les menus.
                self.dash = true;
                self.back = true;
            }
            Button::North => self.interact = true,
            Button::RightTrigger => self.dash = true,
            Button::Start => self.pause = true,
            Button::Select | Button::LeftTrigger => self.map = true,
            Button::DPadUp => self.up = true,
            Button::DPadDown => self.down = true,
            _ => {}
        }
    }
}

impl State {
    // Comparer les etats elimine les repetitions dues aux boutons ou au stick maintenus.
    fn presses(self, previous: Self) -> PadInput {
        PadInput {
            direction: self.direction,
            jump: self.jump && !previous.jump,
            attack: self.attack && !previous.attack,
            attack_held: self.attack,
            next_weapon: self.next_weapon && !previous.next_weapon,
            dash: self.dash && !previous.dash,
            interact: self.interact && !previous.interact,
            pause: self.pause && !previous.pause,
            map: self.map && !previous.map,
            up: self.up && !previous.up,
            down: self.down && !previous.down,
            back: self.back && !previous.back,
        }
    }
}

fn stick(value: f32) -> f32 {
    // Retirer la zone morte puis ramener la course restante dans l'intervalle [-1, 1].
    if value.abs() <= 0.2 {
        0.0
    } else {
        value.signum() * ((value.abs() - 0.2) / 0.8).min(1.0)
    }
}

pub struct Controller {
    backend: Option<Gilrs>,
    active: Option<GamepadId>,
    previous: State,
    pub disconnected: bool,
    pub error: Option<String>,
}

impl Controller {
    // Une panne du backend laisse le clavier utilisable ; les captures le desactivent.
    pub fn new(disabled: bool) -> Self {
        let mut error = None;
        let backend = if disabled {
            None
        } else {
            match Gilrs::new() {
                Ok(backend) => Some(backend),
                Err(reason) => {
                    error = Some(reason.to_string());
                    None
                }
            }
        };
        Self {
            backend,
            active: None,
            previous: State::default(),
            disconnected: false,
            error,
        }
    }

    pub fn name(&self) -> Option<String> {
        // Le nom emprunte au Gamepad temporaire doit etre copie avant de le retourner.
        Some(
            self.backend
                .as_ref()?
                .gamepad(self.active?)
                .name()
                .to_string(),
        )
    }

    pub fn poll(&mut self) -> PadInput {
        self.disconnected = false;
        let Some(backend) = &mut self.backend else {
            return PadInput::default();
        };
        // Conserver aussi les pressions relachees avant la lecture de l'etat final.
        let mut taps = Vec::new();
        while let Some(event) = backend.next_event() {
            match event.event {
                EventType::Disconnected if self.active == Some(event.id) => {
                    self.active = None;
                    self.previous = State::default();
                    self.disconnected = true;
                    taps.clear();
                }
                EventType::ButtonPressed(button, _) => {
                    // La derniere manette actionnee devient active, sans entrees de la precedente.
                    if self.active != Some(event.id) {
                        self.active = Some(event.id);
                        self.previous = State::default();
                        taps.clear();
                    }
                    taps.push(button);
                }
                _ => {}
            }
        }
        if self.active.is_none() {
            // Couvre la connexion initiale et le remplacement apres une deconnexion.
            self.active = backend
                .gamepads()
                .find(|(_, pad)| pad.is_connected())
                .map(|(id, _)| id);
        }
        let Some(id) = self.active else {
            return PadInput::default();
        };
        let pad = backend.gamepad(id);
        // La croix a priorite sur le stick analogique pour un deplacement previsible.
        let horizontal = if pad.is_pressed(Button::DPadLeft) {
            -1.0
        } else if pad.is_pressed(Button::DPadRight) {
            1.0
        } else {
            stick(pad.value(Axis::LeftStickX))
        };
        let state = State {
            direction: horizontal,
            jump: pad.is_pressed(Button::South),
            attack: pad.is_pressed(Button::West),
            next_weapon: pad.is_pressed(Button::RightTrigger2),
            dash: pad.is_pressed(Button::East) || pad.is_pressed(Button::RightTrigger),
            interact: pad.is_pressed(Button::North),
            pause: pad.is_pressed(Button::Start),
            map: pad.is_pressed(Button::Select) || pad.is_pressed(Button::LeftTrigger),
            up: pad.is_pressed(Button::DPadUp) || pad.value(Axis::LeftStickY) > 0.55,
            down: pad.is_pressed(Button::DPadDown) || pad.value(Axis::LeftStickY) < -0.55,
            back: pad.is_pressed(Button::East),
        };
        let mut input = state.presses(self.previous);
        for button in taps {
            input.press(button);
        }
        self.previous = state;
        input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_settings_default_persist_and_cycle() {
        let legacy: Bindings = serde_json::from_str(r#"{"jump":["B"]}"#).unwrap();
        assert_eq!(legacy.visuals(), VisualSettings::default());
        let mut config = legacy;
        config.cycle_fx(true);
        assert_eq!(config.fx, EffectsProfile::Retro);
        config.cycle_fx(true);
        config.cycle_fx(true);
        assert_eq!(config.fx, EffectsProfile::Cinematic);
        config.cycle_fx(false);
        assert_eq!(config.fx, EffectsProfile::Clean);
        config.reduced_motion = true;
        config.flat = true;
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"fx\":\"clean\""));
        let restored: Bindings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.visuals(), config.visuals());
        assert!(serde_json::from_str::<Bindings>(r#"{"fx":"neon"}"#).is_err());
        let mut config = Bindings::default();
        assert_eq!(config.difficulty, Difficulty::Normal);
        config.cycle_difficulty(true);
        assert_eq!(config.difficulty, Difficulty::Hard);
        config.cycle_difficulty(true);
        assert_eq!(config.difficulty, Difficulty::Calm);
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"difficulty\":\"calm\""));
    }

    #[test]
    fn audio_volumes_default_for_legacy_configs_and_are_validated() {
        let legacy: Bindings = serde_json::from_str(r#"{"jump":["B"]}"#).unwrap();
        assert_eq!(legacy.music_volume, 0.45);
        assert_eq!(legacy.sfx_volume, 0.65);
        let mut config = legacy;
        config.adjust_volume(true, -1.0);
        config.adjust_volume(false, 1.0);
        assert_eq!(config.music_volume, 0.0);
        assert_eq!(config.sfx_volume, 1.0);
        config.validate().unwrap();
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(serde_json::from_str::<Bindings>(&json).unwrap(), config);
        config.music_volume = f32::NAN;
        assert!(config.validate().is_err());
    }

    #[test]
    fn bindings_round_trip_preserves_defaults_and_custom_keys() {
        let mut bindings = Bindings::default();
        bindings.validate().unwrap();
        bindings.assign(Action::Jump, Key::B).unwrap();
        let json = serde_json::to_string(&bindings).unwrap();
        assert_eq!(serde_json::from_str::<Bindings>(&json).unwrap(), bindings);
        assert_eq!(bindings.keys(Action::Jump), &[Key::B]);
        assert_eq!(Key::from_code(KeyCode::B), Some(Key::B));
    }

    #[test]
    fn binding_conflicts_and_reserved_keys_do_not_mutate_config() {
        let mut bindings = Bindings::default();
        let original = bindings.clone();
        assert!(bindings.assign(Action::Jump, Key::D).is_err());
        assert!(bindings.assign(Action::Attack, Key::Echap).is_err());
        assert_eq!(bindings, original);
        bindings.jump.clear();
        assert!(bindings.validate().is_err());
        assert!(serde_json::from_str::<Bindings>(r#"{"jump":["UnknownKey"]}"#).is_err());
    }

    #[test]
    fn config_file_is_saved_reloaded_and_replaced_separately() {
        // Chaque test utilise son propre dossier temporaire, jamais la configuration du joueur.
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("lumen-config-{}-{unique}", std::process::id()));
        let path = directory.join("config.json");
        assert_eq!(Bindings::load(&path).unwrap(), Bindings::default());
        let mut bindings = Bindings::default();
        bindings.assign(Action::Jump, Key::B).unwrap();
        bindings.save(&path).unwrap();
        assert_eq!(Bindings::load(&path).unwrap(), bindings);
        bindings.assign(Action::Jump, Key::V).unwrap();
        bindings.save(&path).unwrap();
        assert_eq!(Bindings::load(&path).unwrap(), bindings);
        assert!(!directory.join("save.json").exists());
        std::fs::write(&path, b"invalid json").unwrap();
        assert!(Bindings::load(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"invalid json");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn quick_taps_are_preserved_after_release() {
        let mut input = State::default().presses(State::default());
        input.press(Button::South);
        input.press(Button::East);
        input.press(Button::DPadDown);
        assert!(input.jump && input.dash && input.back && input.down);
        assert!(!State::default().presses(State::default()).jump);
    }

    #[test]
    fn stick_dead_zone_prevents_drift() {
        assert_eq!(stick(0.19), 0.0);
        assert_eq!(stick(-0.2), 0.0);
        assert_eq!(stick(1.0), 1.0);
        assert_eq!(stick(-1.0), -1.0);
        assert!((stick(0.6) - 0.5).abs() < 0.001);
    }

    #[test]
    fn held_buttons_only_trigger_once() {
        let state = State {
            jump: true,
            attack: true,
            dash: true,
            up: true,
            pause: true,
            ..State::default()
        };
        assert!(state.presses(State::default()).jump);
        let held = state.presses(state);
        assert!(!held.jump && !held.attack && !held.dash && !held.up && !held.pause);
        assert!(state.presses(State::default()).pause);
    }
}
