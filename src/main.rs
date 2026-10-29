//! Orchestre les entrees, les ecrans et les sauvegardes, sans definir la physique.

mod audio;
mod balance;
mod characters;
mod content;
mod controls;
mod debug;
mod editor;
mod game;
mod levels;
#[cfg(test)]
mod regression;
mod render;
mod render3d;
mod talents;
mod ui;

use controls::{Action, Bindings, Key};
use game::{Game, Input, Save, SoundCue, STEP, WIDTH};
use macroquad::prelude::*;
use render::{Art, VIEW_HEIGHT};
use std::path::{Path, PathBuf};

fn window_conf() -> Conf {
    let small = std::env::args().any(|argument| argument == "--small");
    Conf {
        window_title: "LUMEN - La citadelle silencieuse".to_string(),
        window_width: if small { 800 } else { 1280 },
        window_height: if small { 500 } else { 800 },
        window_resizable: true,
        high_dpi: true,
        ..Default::default()
    }
}

fn save_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Mvania").join("save.json")
}

// Le pack explicite prime ; sinon chercher a cote de l'executable puis dans le projet courant.
fn asset_path(arguments: &[String]) -> Result<PathBuf, String> {
    if let Some(index) = arguments.iter().position(|argument| argument == "--assets") {
        return arguments
            .get(index + 1)
            .filter(|value| !value.starts_with("--"))
            .map(PathBuf::from)
            .ok_or("--assets requires a directory".into());
    }
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let beside = executable
        .parent()
        .ok_or("Executable directory unavailable")?
        .join("assets");
    if beside.join("manifest.json").is_file() {
        return Ok(beside);
    }
    if Path::new("assets/manifest.json").is_file() {
        return Ok(PathBuf::from("assets"));
    }
    // Dans le depot, lancer target/release depuis un autre dossier doit retrouver le pack du projet.
    for parent in executable.ancestors().skip(1) {
        let assets = parent.join("assets");
        if parent.join("Cargo.toml").is_file() && assets.join("manifest.json").is_file() {
            return Ok(assets);
        }
    }
    Ok(PathBuf::from("assets"))
}

fn asset_error(error: impl std::fmt::Display) -> ! {
    eprintln!("Cannot load assets: {error}");
    std::process::exit(1)
}

fn load_save() -> Result<Save, String> {
    match std::fs::read(save_path()) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Save::default()),
        Err(error) => Err(error.to_string()),
    }
}

fn write_save(save: &Save) -> Result<(), Box<dyn std::error::Error>> {
    let path = save_path();
    std::fs::create_dir_all(path.parent().ok_or("invalid save path")?)?;
    let temporary = path.with_extension("tmp");
    // Ecrire le JSON complet avant de remplacer la sauvegarde existante.
    std::fs::write(&temporary, serde_json::to_vec_pretty(save)?)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

fn pressed(keys: &[KeyCode]) -> bool {
    keys.iter().any(|key| is_key_pressed(*key))
}

// Une session de test reste isolee meme apres fermeture du panneau ou retour a l'accueil.
fn automatic_save_allowed(isolated: bool, debug_session: bool) -> bool {
    !isolated && !debug_session
}
// La configuration ne devient active qu'apres son enregistrement ; les captures restent isolees.
fn apply_bindings(
    current: &mut Bindings,
    updated: Bindings,
    path: &Path,
    isolated: bool,
) -> Result<(), String> {
    updated.validate().map_err(str::to_string)?;
    if !isolated {
        updated.save(path).map_err(|error| error.to_string())?;
    }
    *current = updated;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Screen {
    Title,
    Playing,
    Pause,
    Victory,
    // Garder l'origine de la confirmation pour revenir au bon menu en annulant.
    ConfirmTitle,
    ConfirmPause,
    // L'origine des reglages permet de revenir a l'accueil ou a la pause sans relancer le jeu.
    SettingsTitle,
    SettingsPause,
    AudioTitle,
    AudioPause,
    Talents,
    Characters,
    Debug,
}

#[derive(Debug, PartialEq)]
enum MenuAction {
    Resume,
    RequestNew,
    StartNew,
    Title,
    Cancel,
    Settings,
    Audio,
    Talents,
    Characters,
    Editor,
    Debug,
    Quit,
}

impl Screen {
    // L'ordre des libelles doit rester identique aux indices interpretes par choose.
    fn labels(self, has_save: bool) -> &'static [&'static str] {
        match self {
            Self::Title if has_save => &[
                "CONTINUER",
                "NOUVELLE PARTIE",
                "TOUCHES",
                "AUDIO ET FX",
                "ATELIER",
                "QUITTER",
            ],
            Self::Title => &["JOUER", "TOUCHES", "AUDIO ET FX", "ATELIER", "QUITTER"],
            Self::Pause | Self::Victory => &[
                "REPRENDRE",
                "NOUVELLE PARTIE",
                "TOUCHES",
                "AUDIO ET FX",
                "TALENTS",
                "PERSONNAGES",
                "ACCUEIL",
                "ATELIER",
                "DEBUG",
                "QUITTER",
            ],
            Self::ConfirmTitle | Self::ConfirmPause => &["ANNULER", "NOUVELLE PARTIE"],
            Self::Playing
            | Self::SettingsTitle
            | Self::SettingsPause
            | Self::AudioTitle
            | Self::AudioPause
            | Self::Talents
            | Self::Characters
            | Self::Debug => &[],
        }
    }

    // Decider sans effets de bord permet de tester la protection des sauvegardes.
    fn choose(self, index: usize, has_save: bool) -> MenuAction {
        match self {
            Self::Title if !has_save => match index {
                0 => MenuAction::StartNew,
                1 => MenuAction::Settings,
                2 => MenuAction::Audio,
                3 => MenuAction::Editor,
                _ => MenuAction::Quit,
            },
            Self::Title => match index {
                0 => MenuAction::Resume,
                1 => MenuAction::RequestNew,
                2 => MenuAction::Settings,
                3 => MenuAction::Audio,
                4 => MenuAction::Editor,
                _ => MenuAction::Quit,
            },
            Self::Pause | Self::Victory => match index {
                0 => MenuAction::Resume,
                1 => MenuAction::RequestNew,
                2 => MenuAction::Settings,
                3 => MenuAction::Audio,
                4 => MenuAction::Talents,
                5 => MenuAction::Characters,
                6 => MenuAction::Title,
                7 => MenuAction::Editor,
                8 => MenuAction::Debug,
                _ => MenuAction::Quit,
            },
            Self::ConfirmTitle | Self::ConfirmPause => {
                if index == 0 {
                    MenuAction::Cancel
                } else {
                    MenuAction::StartNew
                }
            }
            Self::Playing
            | Self::SettingsTitle
            | Self::SettingsPause
            | Self::AudioTitle
            | Self::AudioPause
            | Self::Talents
            | Self::Characters
            | Self::Debug => MenuAction::Resume,
        }
    }

    fn cancel(self) -> Self {
        match self {
            Self::ConfirmTitle => Self::Title,
            Self::ConfirmPause => Self::Pause,
            Self::SettingsTitle => Self::Title,
            Self::SettingsPause => Self::Pause,
            Self::AudioTitle => Self::Title,
            Self::AudioPause => Self::Pause,
            Self::Talents => Self::Pause,
            Self::Characters => Self::Pause,
            Self::Pause | Self::Victory | Self::Debug => Self::Playing,
            other => other,
        }
    }

    fn is_settings(self) -> bool {
        matches!(
            self,
            Self::SettingsTitle | Self::SettingsPause | Self::AudioTitle | Self::AudioPause
        )
    }

    fn is_audio(self) -> bool {
        matches!(self, Self::AudioTitle | Self::AudioPause)
    }
}

fn navigate(selection: usize, direction: i32, count: usize) -> usize {
    (selection as i32 + direction).rem_euclid(count as i32) as usize
}

// Les menus plus longs laissent de la place au personnage de l'accueil et au bas de l'ecran.
fn menu_rect(index: usize, count: usize) -> Rect {
    // Au-dela de huit entrees (pause complete), les boutons se resserrent pour tenir sous le HUD.
    if count > 8 {
        return Rect::new(320.0, 252.0 + index as f32 * 33.0, 320.0, 30.0);
    }
    let top = if count > 4 {
        256.0
    } else if count > 3 {
        252.0
    } else {
        280.0
    };
    let spacing = if count > 4 { 38.0 } else { 50.0 };
    Rect::new(
        320.0,
        top + index as f32 * spacing,
        320.0,
        if count > 4 { 34.0 } else { 44.0 },
    )
}

#[macroquad::main(window_conf)]
async fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    let visual_settings = render::VisualSettings::from_arguments(&arguments)
        .unwrap_or_else(|error| asset_error(error));
    let capture = arguments
        .iter()
        .position(|argument| argument == "--capture")
        .and_then(|index| arguments.get(index + 1))
        .cloned();
    let capture_room = arguments
        .iter()
        .position(|argument| argument == "--room")
        .and_then(|index| arguments.get(index + 1))
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let capture_dash = capture.is_some() && arguments.iter().any(|argument| argument == "--dash");
    let capture_effects =
        capture.is_some() && arguments.iter().any(|argument| argument == "--effects");
    // Les captures sont reproductibles et n'utilisent ni manette ni sauvegarde personnelle.
    let mut controller = controls::Controller::new(capture.is_some());
    if arguments
        .iter()
        .any(|argument| argument == "--check-controller")
    {
        controller.poll();
        if let Some(error) = &controller.error {
            eprintln!("Controller initialization failed: {error}");
            std::process::exit(1);
        }
        match controller.name() {
            Some(name) => println!("Controller connected: {name}"),
            None => println!("Controller backend ready; no controller connected."),
        }
        return;
    }
    if let Some(index) = arguments
        .iter()
        .position(|argument| argument == "--export-assets")
    {
        // Ne jamais ecraser un pack edite : l'export demande un nouveau repertoire.
        let destination = arguments
            .get(index + 1)
            .filter(|value| !value.starts_with("--"))
            .map(PathBuf::from)
            .unwrap_or_else(|| asset_error("--export-assets requires a new directory"));
        if destination.exists() {
            asset_error("Export directory already exists; choose a new directory");
        }
        std::fs::create_dir_all(&destination).unwrap_or_else(|error| asset_error(error));
        let mut manifest = Art::export(&destination).unwrap_or_else(|error| asset_error(error));
        audio::Audio::export(&destination, &mut manifest)
            .unwrap_or_else(|error| asset_error(error));
        let mut exported_levels = levels::Levels::original();
        exported_levels
            .add_campaign()
            .unwrap_or_else(|error| asset_error(error));
        exported_levels
            .save(&destination)
            .unwrap_or_else(|error| asset_error(error));
        manifest
            .save(&destination.join("manifest.json"))
            .unwrap_or_else(|error| asset_error(error));
        println!("Assets exported to {}", destination.display());
        return;
    }
    let mut content =
        content::Content::load(asset_path(&arguments).unwrap_or_else(|error| asset_error(error)))
            .unwrap_or_else(|error| asset_error(error));
    // L'equilibrage est charge une fois, avant toute simulation ; un fichier invalide arrete le jeu.
    balance::install(
        balance::Tuning::load(&content.root).unwrap_or_else(|error| asset_error(error)),
    );
    if arguments
        .iter()
        .any(|argument| argument == "--save-campaign")
    {
        let levels = levels::Levels::load(&content.root).unwrap_or_else(|error| asset_error(error));
        levels
            .save(&content.root)
            .unwrap_or_else(|error| asset_error(error));
        println!(
            "Campaign saved: {} biomes, {} rooms",
            levels.biomes.len(),
            levels.rooms.len()
        );
        return;
    }
    if arguments
        .iter()
        .any(|argument| argument == "--check-assets")
    {
        content
            .validate_files()
            .unwrap_or_else(|error| asset_error(error));
        levels::Levels::load(&content.root).unwrap_or_else(|error| asset_error(error));
        let _art = Art::load(&content).unwrap_or_else(|error| asset_error(error));
        println!("Assets ready: sprites, animations, palettes, font and WAV files validated.");
        return;
    }
    if arguments.iter().any(|argument| argument == "--check-audio") {
        // Diagnostic reel sans lire ou ecrire les fichiers personnels.
        let mut audio = audio::Audio::load(&content, false)
            .await
            .unwrap_or_else(|error| asset_error(error));
        if !audio.ready() {
            eprintln!("Audio loading failed");
            std::process::exit(1);
        }
        for frame in 0..210 {
            audio.update(0.45, false, 1.0 / 60.0);
            if frame % 14 == 0 {
                if let Some(cue) = SoundCue::ALL.get(frame / 14) {
                    audio.play(*cue, 0.6);
                }
            }
            next_frame().await;
        }
        audio.stop();
        println!("Audio ready: 14 sound effects and looping music played.");
        return;
    }
    // L'ouverture directe ne lit ni n'ecrit la progression ou la configuration personnelle.
    if arguments.iter().any(|argument| argument == "--editor") {
        let tab = match arguments
            .iter()
            .position(|argument| argument == "--editor-tab")
            .and_then(|index| arguments.get(index + 1))
            .map(String::as_str)
        {
            None | Some("levels") => editor::Tab::Levels,
            Some("sprites") => editor::Tab::Sprites,
            Some("audio") => editor::Tab::Audio,
            Some("weapons") => editor::Tab::Weapons,
            Some(value) => asset_error(format!("Onglet d'atelier inconnu: {value}")),
        };
        editor::run(
            &content.root,
            editor::Options {
                tab,
                weapon: arguments
                    .iter()
                    .position(|argument| argument == "--editor-weapon")
                    .map(|index| {
                        arguments
                            .get(index + 1)
                            .and_then(|value| value.parse::<usize>().ok())
                            .unwrap_or_else(|| asset_error("--editor-weapon requires an index"))
                    })
                    .unwrap_or(0),
                room: capture_room,
                capture: capture.map(PathBuf::from),
                test: arguments.iter().any(|argument| argument == "--editor-test"),
                generation_seed: arguments
                    .iter()
                    .position(|argument| argument == "--editor-generate")
                    .map(|index| {
                        arguments
                            .get(index + 1)
                            .and_then(|value| value.parse::<u32>().ok())
                            .unwrap_or_else(|| asset_error("--editor-generate requires a u32 seed"))
                    }),
                visuals: visual_settings,
            },
            &Bindings::default(),
        )
        .await
        .unwrap_or_else(|error| asset_error(error));
        return;
    }
    let config_path = save_path().with_file_name("config.json");
    let mut binding_notice = String::new();
    let mut bindings = if capture.is_some() {
        Bindings::default()
    } else {
        match Bindings::load(&config_path) {
            Ok(bindings) => bindings,
            Err(error) => {
                // Garder un fichier illisible intact jusqu'a une modification volontaire des reglages.
                eprintln!("Cannot load controls: {error}");
                binding_notice = "CONFIGURATION ILLISIBLE / VALEURS PAR DEFAUT".to_string();
                Bindings::default()
            }
        }
    };
    if capture.is_none() && !config_path.exists() {
        if let Err(error) = bindings.save(&config_path) {
            eprintln!("Cannot create controls: {error}");
            binding_notice = "ECRITURE DE LA CONFIGURATION IMPOSSIBLE".to_string();
        }
    }
    let mut has_save = if capture.is_some() {
        arguments.iter().any(|argument| argument == "--saved-menu")
    } else {
        save_path().is_file()
    };
    let mut art = Art::load(&content).unwrap_or_else(|error| asset_error(error));
    // La configuration fournit la base ; les options de ligne de commande la surchargent.
    let mut visual_settings = bindings
        .visuals()
        .with_arguments(&arguments)
        .unwrap_or_else(|error| asset_error(error));
    art.effects = visual_settings;
    let mut audio = audio::Audio::load(&content, capture.is_some())
        .await
        .unwrap_or_else(|error| asset_error(error));
    let mut levels = levels::Levels::load(&content.root).unwrap_or_else(|error| asset_error(error));
    let capture_room = capture_room.min(levels.rooms.len() - 1);
    let mut game = if capture.is_some() {
        Game::with_levels(Save::default(), levels.clone())
    } else {
        match load_save() {
            Ok(save) => Game::with_levels(save, levels.clone()),
            Err(error) => {
                has_save = false;
                eprintln!("Cannot load save: {error}");
                let mut game = Game::with_levels(Save::default(), levels.clone());
                game.message("Sauvegarde illisible. Nouvelle exploration.");
                game
            }
        }
    };
    if capture.is_some() {
        game.enter(capture_room, vec2(96.0, 446.0));
        if capture_dash {
            game.save.double_jump = true;
        }
    }
    // Game::new marque la salle visitee ; ouvrir l'accueil ne doit pas ecrire de sauvegarde.
    game.set_weapons(content.manifest.weapons.clone());
    game.set_difficulty(bindings.difficulty);
    game.save_dirty = false;
    let target = render3d::target(WIDTH as u32, VIEW_HEIGHT as u32);
    let mut accumulator = 0.0;
    let mut pending = Input::default();
    let mut screen = if capture.is_some() && !arguments.iter().any(|argument| argument == "--title")
    {
        Screen::Playing
    } else {
        Screen::Title
    };
    if capture.is_some() && arguments.iter().any(|argument| argument == "--pause") {
        screen = Screen::Pause;
    }
    if capture.is_some() && arguments.iter().any(|argument| argument == "--confirm") {
        screen = if screen == Screen::Title {
            Screen::ConfirmTitle
        } else {
            Screen::ConfirmPause
        };
    }
    if capture.is_some() && arguments.iter().any(|argument| argument == "--settings") {
        screen = if screen == Screen::Pause {
            Screen::SettingsPause
        } else {
            Screen::SettingsTitle
        };
    }
    if capture.is_some() && arguments.iter().any(|argument| argument == "--audio-menu") {
        screen = if screen == Screen::Pause {
            Screen::AudioPause
        } else {
            Screen::AudioTitle
        };
    }
    if capture.is_some() && arguments.iter().any(|argument| argument == "--talents") {
        screen = Screen::Talents;
        game.save.shards = 24;
    }
    if capture.is_some() && arguments.iter().any(|argument| argument == "--characters") {
        screen = Screen::Characters;
        game.all_weapons = true;
    }
    // Capture d'un personnage precis en jeu : --hero ninja|samurai|soldier|goddess.
    if capture.is_some() {
        if let Some(name) = arguments
            .iter()
            .position(|argument| argument == "--hero")
            .and_then(|index| arguments.get(index + 1))
        {
            game.all_weapons = true;
            if let Some(hero) = characters::Character::ALL
                .iter()
                .find(|character| character.sprite() == name)
            {
                let _ = game.select_character(*hero);
            }
        }
    }
    let mut rebinding = None;
    let mut debug_menu = debug::DebugMenu::default();
    let mut debug_session = arguments.iter().any(|argument| argument == "--debug");
    if debug_session {
        screen = Screen::Debug;
    }
    let mut selection = 0;
    let mut menu_time = 0.0;
    let mut last_mouse = vec2(-1.0, -1.0);
    let mut show_map = capture.is_some() && arguments.iter().any(|argument| argument == "--map");
    let mut fullscreen = false;
    let mut frame = 0;
    let mut previous_pad_direction = 0.0;
    loop {
        // L'atelier protege ses documents ; le jeu reprend ensuite la fermeture de la fenetre.
        if is_quit_requested() {
            break;
        }
        let previous_screen = screen;
        let previous_selection = selection;
        // Une action qui ferme un menu ou la carte ne doit pas aussi agir sur le personnage.
        let was_active = screen == Screen::Playing && !show_map;
        // Le raccourci consomme l'image d'ouverture ; aucun clic du panneau ne devient une attaque.
        if is_key_pressed(KeyCode::F3)
            && matches!(
                screen,
                Screen::Playing | Screen::Pause | Screen::Victory | Screen::Debug
            )
        {
            screen = if screen == Screen::Debug {
                Screen::Playing
            } else {
                Screen::Debug
            };
            debug_session = true;
            show_map = false;
            selection = 0;
            game.finished = false;
        }
        let pad = controller.poll();
        // Un mouvement horizontal maintenu ne doit pas ecrire la configuration a chaque image.
        let pad_left = pad.direction < -0.55 && previous_pad_direction >= -0.55;
        let pad_right = pad.direction > 0.55 && previous_pad_direction <= 0.55;
        previous_pad_direction = pad.direction;
        let back = is_key_pressed(KeyCode::Escape) || pad.back;
        let was_binding = rebinding.is_some();
        if let Some(action) = rebinding {
            // Consommer la touche dans les reglages, pas aussi comme validation ou raccourci global.
            if back {
                rebinding = None;
                binding_notice.clear();
            } else if let Some(code) = get_last_key_pressed() {
                if let Some(key) = Key::from_code(code) {
                    let mut updated = bindings.clone();
                    match updated.assign(action, key) {
                        Ok(()) => match apply_bindings(
                            &mut bindings,
                            updated,
                            &config_path,
                            capture.is_some(),
                        ) {
                            Ok(()) => {
                                rebinding = None;
                                binding_notice = "CONFIGURATION ENREGISTREE".to_string();
                            }
                            Err(error) => {
                                eprintln!("Cannot save controls: {error}");
                                binding_notice =
                                    "ECRITURE IMPOSSIBLE / MODIFICATION NON APPLIQUEE".to_string();
                            }
                        },
                        Err(message) => binding_notice = message.to_string(),
                    }
                } else {
                    binding_notice = "TOUCHE NON PRISE EN CHARGE".to_string();
                }
            }
        }
        if !was_binding && !screen.is_settings() && bindings.pressed(Action::Fullscreen) {
            fullscreen = !fullscreen;
            set_fullscreen(fullscreen);
        }
        if controller.disconnected && screen == Screen::Playing {
            screen = Screen::Pause;
            show_map = false;
            selection = 0;
        }
        if screen == Screen::Playing
            && (is_key_pressed(KeyCode::Escape) || bindings.pressed(Action::Pause) || pad.pause)
        {
            if show_map {
                show_map = false;
            } else {
                screen = Screen::Pause;
                selection = 0;
            }
        } else if screen != Screen::Playing
            && !was_binding
            && (back || (screen == Screen::Pause && (pad.pause || bindings.pressed(Action::Pause))))
        {
            screen = screen.cancel();
            selection = 0;
            game.finished = false;
        }
        if screen == Screen::Playing && show_map && pad.back {
            show_map = false;
        }
        if (bindings.pressed(Action::Map) || pad.map) && screen == Screen::Playing && !game.finished
        {
            show_map = !show_map;
        }
        let delta = if capture.is_some() {
            1.0 / 60.0
        } else {
            get_frame_time().min(0.1)
        };
        menu_time += delta;
        audio.update(
            bindings.music_volume,
            (screen != Screen::Playing && !screen.is_audio()) || show_map,
            delta,
        );
        if screen == Screen::Playing && !show_map && !game.finished && was_active {
            pending.direction = if bindings.down(Action::Right) {
                1.0
            } else {
                0.0
            } - if bindings.down(Action::Left) {
                1.0
            } else {
                0.0
            };
            if pending.direction == 0.0 {
                pending.direction = pad.direction;
            }
            // Conserver les pressions tant qu'aucun pas de physique ne les a consommees.
            pending.jump |= bindings.pressed(Action::Jump) || pad.jump;
            pending.attack |= bindings.pressed(Action::Attack)
                || is_mouse_button_pressed(MouseButton::Left)
                || pad.attack;
            pending.attack_held = bindings.down(Action::Attack)
                || is_mouse_button_down(MouseButton::Left)
                || pad.attack_held;
            pending.dash |= bindings.pressed(Action::Dash) || pad.dash;
            pending.next_weapon |= bindings.pressed(Action::NextWeapon) || pad.next_weapon;
            pending.interact |= bindings.pressed(Action::Interact) || pad.interact;
            if capture.is_some() {
                pending.attack_held = false;
                pending.direction = if frame < 60 { 1.0 } else { 0.0 };
                pending.jump = frame == 20;
                pending.dash = capture_dash && frame == 85;
                if capture_effects {
                    pending.attack = frame == 85;
                    if frame == 85 {
                        game.damage(-1.0);
                    }
                }
            }
            accumulator += delta * debug_menu.speed;
            // Plusieurs pas peuvent etre executes par image, mais chaque action ne part qu'une fois.
            while accumulator >= STEP {
                game.update(pending, STEP);
                pending.jump = false;
                pending.attack = false;
                pending.next_weapon = false;
                pending.dash = false;
                pending.interact = false;
                accumulator -= STEP;
            }
        } else {
            // Ne pas rattraper le temps passe en pause ni rejouer les anciennes entrees.
            accumulator = 0.0;
            pending = Input::default();
        }
        if game.finished && screen == Screen::Playing {
            screen = Screen::Victory;
            selection = 0;
        }
        // Consommer tous les evenements du pas fixe une seule fois par image, meme en capture muette.
        let mut played = Vec::new();
        for cue in game.sounds.drain(..) {
            if !played.contains(&cue) {
                audio.play(cue, bindings.sfx_volume);
                played.push(cue);
            }
        }
        // Persister aussi la victoire, meme si elle vient de quitter l'etat Playing.
        if game.save_dirty && !matches!(screen, Screen::Title | Screen::ConfirmTitle) {
            game.save_dirty = false;
            // Une session debug ne remplace jamais la sauvegarde par une ecriture automatique.
            if automatic_save_allowed(capture.is_some(), debug_session) {
                if let Err(error) = write_save(&game.save) {
                    eprintln!("Cannot save: {error}");
                    game.message("Ecriture de la sauvegarde impossible.");
                } else {
                    has_save = true;
                }
            }
        }
        // Le jeu garde sa resolution logique ; le pointeur doit suivre les memes bandes noires.
        let scale = (screen_width() / WIDTH).min(screen_height() / VIEW_HEIGHT);
        let offset = vec2(
            (screen_width() - WIDTH * scale) * 0.5,
            (screen_height() - VIEW_HEIGHT * scale) * 0.5,
        );
        let mouse = (vec2(mouse_position().0, mouse_position().1) - offset) / scale;
        let mouse_moved = (mouse - last_mouse).length_squared() > 0.25;
        last_mouse = mouse;
        let mut world_camera =
            Camera2D::from_display_rect(Rect::new(0.0, -56.0, WIDTH, VIEW_HEIGHT));
        world_camera.render_target = Some(target.clone());
        world_camera.target += render3d::shake_offset(&game, art.effects.motion());
        set_camera(&world_camera);
        let title_background = matches!(
            screen,
            Screen::Title | Screen::ConfirmTitle | Screen::SettingsTitle | Screen::AudioTitle
        );
        if title_background {
            render::title(&art, menu_time);
        } else {
            render::scene(&art, &game, &target);
        }
        // Une camera distincte garde le HUD immobile pendant les secousses du monde.
        let mut ui_camera = Camera2D::from_display_rect(Rect::new(0.0, 0.0, WIDTH, VIEW_HEIGHT));
        ui_camera.render_target = Some(target.clone());
        set_camera(&ui_camera);
        if !title_background {
            render::hud(&art, &game);
        }
        if show_map && screen == Screen::Playing {
            render::map(&art, &game);
        }
        if screen.is_audio() {
            let rows = render::AUDIO_ROWS;
            if pressed(&[KeyCode::Up, KeyCode::W, KeyCode::Z]) || pad.up {
                selection = navigate(selection, -1, rows + 1);
            }
            if pressed(&[KeyCode::Down, KeyCode::S]) || pad.down {
                selection = navigate(selection, 1, rows + 1);
            }
            if mouse_moved {
                for index in 0..=rows {
                    if render::audio_rect(index).contains(mouse) {
                        selection = index;
                    }
                }
            }
            let mut change =
                render::audio_controls(&art, &bindings, selection, mouse, &binding_notice);
            if selection < rows {
                let step = if selection < 2 { 0.1 } else { 1.0 };
                if is_key_pressed(KeyCode::Left) || pad_left {
                    change = Some((selection, -step));
                }
                if is_key_pressed(KeyCode::Right) || pad_right {
                    change = Some((selection, step));
                }
            }
            // Le clic porte son propre indice, independamment d'un focus clavier precedent.
            if let Some((index, change)) = change {
                selection = index;
                let mut updated = bindings.clone();
                match index {
                    0 | 1 => updated.adjust_volume(index == 0, change),
                    2 => updated.cycle_fx(change > 0.0),
                    3 => updated.reduced_motion = !updated.reduced_motion,
                    4 => updated.flat = !updated.flat,
                    _ => updated.cycle_difficulty(change > 0.0),
                }
                match apply_bindings(&mut bindings, updated, &config_path, capture.is_some()) {
                    Ok(()) => {
                        binding_notice = "CONFIGURATION ENREGISTREE".to_string();
                        if index == 1 {
                            audio.play(SoundCue::Attack, bindings.sfx_volume);
                        }
                        // Les effets s'appliquent immediatement, sans redemarrage.
                        visual_settings = bindings.visuals();
                        art.effects = visual_settings;
                        game.set_difficulty(bindings.difficulty);
                    }
                    Err(error) => {
                        eprintln!("Cannot save audio settings: {error}");
                        binding_notice =
                            "ECRITURE IMPOSSIBLE / MODIFICATION NON APPLIQUEE".to_string();
                    }
                }
            } else if selection == rows
                && (pressed(&[KeyCode::Enter, KeyCode::Space])
                    || pad.jump
                    || (is_mouse_button_pressed(MouseButton::Left)
                        && render::audio_rect(rows).contains(mouse)))
            {
                // Le retour est valide seulement depuis le bouton dedie.
                screen = screen.cancel();
                selection = 0;
            }
        } else if screen == Screen::Characters {
            let rows = render::CHARACTER_ROWS;
            if pressed(&[KeyCode::Up, KeyCode::W, KeyCode::Z]) || pad.up {
                selection = navigate(selection, -1, rows + 1);
            }
            if pressed(&[KeyCode::Down, KeyCode::S]) || pad.down {
                selection = navigate(selection, 1, rows + 1);
            }
            if mouse_moved {
                for index in 0..=rows {
                    if render::character_rect(index).contains(mouse) {
                        selection = index;
                    }
                }
            }
            let clicked = render::characters(&art, &game, selection, mouse, &binding_notice);
            let activate = pressed(&[KeyCode::Enter, KeyCode::Space]) || pad.jump;
            let back_clicked = is_mouse_button_pressed(MouseButton::Left)
                && render::character_rect(rows).contains(mouse);
            if let Some(index) = clicked.or((activate && selection < rows).then_some(selection)) {
                selection = index;
                match game.select_character(characters::Character::ALL[index]) {
                    Ok(()) => binding_notice = "PERSONNAGE CHOISI".to_string(),
                    Err(message) => binding_notice = message.to_string(),
                }
            } else if selection == rows && (activate || back_clicked) {
                screen = screen.cancel();
                selection = 0;
            }
        } else if screen == Screen::Talents {
            let rows = render::TALENT_ROWS;
            if pressed(&[KeyCode::Up, KeyCode::W, KeyCode::Z]) || pad.up {
                selection = navigate(selection, -1, rows + 1);
            }
            if pressed(&[KeyCode::Down, KeyCode::S]) || pad.down {
                selection = navigate(selection, 1, rows + 1);
            }
            if mouse_moved {
                for index in 0..=rows {
                    if render::talent_rect(index).contains(mouse) {
                        selection = index;
                    }
                }
            }
            let clicked = render::talents(&art, &game, selection, mouse, &binding_notice);
            let activate = pressed(&[KeyCode::Enter, KeyCode::Space]) || pad.jump;
            let back_clicked = is_mouse_button_pressed(MouseButton::Left)
                && render::talent_rect(rows).contains(mouse);
            if let Some(index) = clicked.or((activate && selection < rows).then_some(selection)) {
                selection = index;
                match game.buy_talent(talents::Talent::ALL[index]) {
                    Ok(()) => binding_notice = "TALENT ACQUIS".to_string(),
                    Err(message) => binding_notice = message.to_string(),
                }
            } else if selection == rows && (activate || back_clicked) {
                screen = screen.cancel();
                selection = 0;
            }
        } else if screen.is_settings() {
            let count = Action::ALL.len() + 2;
            if !was_binding {
                if pressed(&[KeyCode::Up, KeyCode::W, KeyCode::Z]) || pad.up {
                    selection = navigate(selection, -1, count);
                }
                if pressed(&[KeyCode::Down, KeyCode::S]) || pad.down {
                    selection = navigate(selection, 1, count);
                }
                if mouse_moved {
                    for index in 0..count {
                        if render::binding_rect(index).contains(mouse) {
                            selection = index;
                        }
                    }
                }
            }
            render::bindings(&art, &bindings, selection, rebinding, &binding_notice);
            if !was_binding {
                let clicked = is_mouse_button_pressed(MouseButton::Left)
                    && render::binding_rect(selection).contains(mouse);
                if clicked || pressed(&[KeyCode::Enter, KeyCode::Space]) || pad.jump {
                    if let Some(action) = Action::ALL.get(selection) {
                        rebinding = Some(*action);
                        binding_notice.clear();
                    } else if selection == Action::ALL.len() {
                        // Restaurer les touches ne doit pas reinitialiser audio ni affichage.
                        let defaults = Bindings {
                            music_volume: bindings.music_volume,
                            sfx_volume: bindings.sfx_volume,
                            fx: bindings.fx,
                            reduced_motion: bindings.reduced_motion,
                            flat: bindings.flat,
                            difficulty: bindings.difficulty,
                            ..Bindings::default()
                        };
                        match apply_bindings(
                            &mut bindings,
                            defaults,
                            &config_path,
                            capture.is_some(),
                        ) {
                            Ok(()) => binding_notice = "CONFIGURATION ENREGISTREE".to_string(),
                            Err(error) => {
                                eprintln!("Cannot reset controls: {error}");
                                binding_notice =
                                    "ECRITURE IMPOSSIBLE / MODIFICATION NON APPLIQUEE".to_string();
                            }
                        }
                    } else {
                        screen = screen.cancel();
                        selection = 0;
                    }
                }
            }
        } else if screen != Screen::Playing && screen != Screen::Debug {
            if !title_background {
                ui::overlay();
            }
            let confirmation = matches!(screen, Screen::ConfirmTitle | Screen::ConfirmPause);
            if screen == Screen::Title {
                ui::wordmark(&art, vec2(480.0, 140.0), "LUMEN", 72);
            } else {
                art.centered(
                    if screen == Screen::Victory {
                        "LUMEN RESTAURE"
                    } else if confirmation {
                        "NOUVELLE PARTIE ?"
                    } else {
                        "CITADELLE EN SOMMEIL"
                    },
                    vec2(480.0, 210.0),
                    30,
                    render::PAPER,
                );
            }
            if screen == Screen::Title {
                art.centered(
                    "LA CITADELLE SILENCIEUSE",
                    vec2(480.0, 184.0),
                    18,
                    render::GOLD,
                );
                art.centered(
                    if controller.error.is_some() {
                        "MANETTE INDISPONIBLE"
                    } else if controller.name().is_some() {
                        "MANETTE CONNECTEE"
                    } else {
                        "CLAVIER / SOURIS"
                    },
                    vec2(480.0, 555.0),
                    12,
                    render::PAPER,
                );
            }
            if screen == Screen::Victory {
                art.centered(
                    "La lumiere revient entre les pierres.",
                    vec2(480.0, 247.0),
                    14,
                    render::MINT,
                );
            }
            if confirmation {
                art.centered(
                    "La progression sauvegardee sera effacee.",
                    vec2(480.0, 247.0),
                    14,
                    render::CORAL,
                );
            }
            let labels = screen.labels(has_save);
            if pressed(&[KeyCode::Up, KeyCode::W, KeyCode::Z]) || pad.up {
                selection = navigate(selection, -1, labels.len());
            }
            if pressed(&[KeyCode::Down, KeyCode::S]) || pad.down {
                selection = navigate(selection, 1, labels.len());
            }
            for index in 0..labels.len() {
                let rect = menu_rect(index, labels.len());
                // Un curseur immobile ne doit pas reprendre le focus au clavier ou a la manette.
                if mouse_moved && rect.contains(mouse) {
                    selection = index;
                }
            }
            let mut chosen = None;
            for (index, label) in labels.iter().enumerate() {
                let rect = menu_rect(index, labels.len());
                if render::button(&art, label, rect, mouse, selection == index) {
                    chosen = Some(index);
                }
            }
            if pressed(&[KeyCode::Enter, KeyCode::Space]) || pad.jump {
                chosen = Some(selection);
            }
            if let Some(index) = chosen {
                match screen.choose(index, has_save) {
                    MenuAction::Resume => {
                        screen = Screen::Playing;
                        game.finished = false;
                        game.save_dirty = true;
                    }
                    MenuAction::RequestNew => {
                        screen = if screen == Screen::Title {
                            Screen::ConfirmTitle
                        } else {
                            Screen::ConfirmPause
                        };
                    }
                    MenuAction::StartNew => {
                        game = Game::with_levels(Save::default(), levels.clone());
                        debug_session = false;
                        debug_menu = debug::DebugMenu::default();
                        game.set_weapons(content.manifest.weapons.clone());
                        game.set_difficulty(bindings.difficulty);
                        screen = Screen::Playing;
                        show_map = false;
                    }
                    MenuAction::Title => {
                        screen = Screen::Title;
                        show_map = false;
                    }
                    MenuAction::Cancel => screen = screen.cancel(),
                    MenuAction::Settings => {
                        screen = if screen == Screen::Title {
                            Screen::SettingsTitle
                        } else {
                            Screen::SettingsPause
                        };
                        rebinding = None;
                        show_map = false;
                    }
                    MenuAction::Characters => {
                        screen = Screen::Characters;
                        binding_notice.clear();
                        show_map = false;
                    }
                    MenuAction::Talents => {
                        screen = Screen::Talents;
                        binding_notice.clear();
                        show_map = false;
                    }
                    MenuAction::Audio => {
                        screen = if screen == Screen::Title {
                            Screen::AudioTitle
                        } else {
                            Screen::AudioPause
                        };
                        show_map = false;
                    }
                    MenuAction::Editor => {
                        audio.stop();
                        if editor::run(
                            &content.root,
                            editor::Options {
                                tab: editor::Tab::Levels,
                                weapon: game.weapon_index,
                                room: game.room,
                                capture: None,
                                test: false,
                                generation_seed: None,
                                visuals: visual_settings,
                            },
                            &bindings,
                        )
                        .await
                        .unwrap_or_else(|error| asset_error(error))
                        {
                            break;
                        }
                        // Recharger le pack sans modifier les acquis ni enregistrer la session de test.
                        content = content::Content::load(content.root.clone())
                            .unwrap_or_else(|error| asset_error(error));
                        art = Art::load(&content).unwrap_or_else(|error| asset_error(error));
                        art.effects = visual_settings;
                        audio = audio::Audio::load(&content, capture.is_some())
                            .await
                            .unwrap_or_else(|error| asset_error(error));
                        levels = levels::Levels::load(&content.root)
                            .unwrap_or_else(|error| asset_error(error));
                        let room = if game.room < levels.rooms.len() {
                            game.room
                        } else {
                            0
                        };
                        let position = game.position();
                        let health = game.health;
                        let debug_options = game.debug.clone();
                        // Revalider sanctuaire et carte apres l'ajout ou le retrait d'extensions.
                        game = Game::with_levels(game.save.clone(), levels.clone());
                        game.set_weapons(content.manifest.weapons.clone());
                        game.set_difficulty(bindings.difficulty);
                        game.enter(room, position);
                        game.health = health;
                        game.debug = debug_options;
                        game.save_dirty = false;
                        // Le rendu de l'atelier a change de camera ; reprendre a l'image suivante.
                        selection = 0;
                        pending = Input::default();
                        accumulator = 0.0;
                        next_frame().await;
                        continue;
                    }
                    MenuAction::Debug => {
                        screen = Screen::Debug;
                        debug_session = true;
                        show_map = false;
                        game.finished = false;
                    }
                    MenuAction::Quit => break,
                }
                selection = 0;
                pending = Input::default();
                accumulator = 0.0;
            }
        }
        if screen != previous_screen || selection != previous_selection {
            audio.play(SoundCue::Menu, bindings.sfx_volume * 0.5);
        }
        set_default_camera();
        clear_background(render::INK);
        // Exclure les menus et la carte du post-traitement pour garder textes et controles lisibles.
        art.present(
            &target.texture,
            Rect::new(offset.x, offset.y, WIDTH * scale, VIEW_HEIGHT * scale),
            if screen == Screen::Playing && !show_map {
                Some(&game)
            } else {
                None
            },
        );
        if screen == Screen::Debug {
            let response = debug_menu.draw(&mut game, capture.is_some());
            if response.save && capture.is_none() {
                match write_save(&game.save) {
                    Ok(()) => {
                        has_save = true;
                        game.save_dirty = false;
                        debug_menu.notice = "PROGRESSION ENREGISTREE".into();
                    }
                    Err(error) => debug_menu.notice = format!("Ecriture impossible : {error}"),
                }
            }
            if response.resume {
                screen = Screen::Playing;
                pending = Input::default();
                accumulator = 0.0;
            }
        } else if debug_session && screen == Screen::Playing {
            // Le statut reste visible hors du menu pour signaler l'absence de sauvegarde automatique.
            let status_y = offset.y + VIEW_HEIGHT * scale - 24.0;
            draw_rectangle(offset.x + 8.0, status_y, 234.0, 20.0, render::INK);
            draw_text(
                "DEBUG / AUTO-SAVE OFF",
                offset.x + 12.0,
                status_y + 15.0,
                16.0,
                render::GOLD,
            );
        }
        if frame == 90 {
            if let Some(path) = &capture {
                get_screen_data().export_png(path);
                println!("Captured room {} to {path}", game.room);
                break;
            }
        }
        frame += 1;
        next_frame().await;
    }
    // Couper explicitement les boucles et les effets lors de la fermeture du jeu.
    audio.stop();
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad_platformer::{Tile, World};

    #[test]
    fn debug_session_never_automatically_writes_personal_save() {
        assert!(automatic_save_allowed(false, false));
        assert!(!automatic_save_allowed(false, true));
        assert!(!automatic_save_allowed(true, false));
        assert!(!automatic_save_allowed(true, true));
    }

    #[test]
    fn menu_protects_existing_save_and_can_return_home() {
        assert_eq!(Screen::Title.choose(0, false), MenuAction::StartNew);
        assert_eq!(Screen::Title.choose(0, true), MenuAction::Resume);
        assert_eq!(Screen::Title.choose(1, true), MenuAction::RequestNew);
        assert_eq!(Screen::ConfirmTitle.choose(0, true), MenuAction::Cancel);
        assert_eq!(Screen::ConfirmTitle.choose(1, true), MenuAction::StartNew);
        assert_eq!(Screen::ConfirmTitle.cancel(), Screen::Title);
        assert_eq!(Screen::ConfirmPause.cancel(), Screen::Pause);
        assert_eq!(Screen::Pause.choose(4, true), MenuAction::Talents);
        assert_eq!(Screen::Talents.cancel(), Screen::Pause);
        assert_eq!(Screen::Pause.choose(5, true), MenuAction::Characters);
        assert_eq!(Screen::Characters.cancel(), Screen::Pause);
        assert_eq!(Screen::Pause.choose(6, true), MenuAction::Title);
        assert_eq!(Screen::Title.choose(1, false), MenuAction::Settings);
        assert_eq!(Screen::Title.choose(2, true), MenuAction::Settings);
        assert_eq!(Screen::Pause.choose(2, true), MenuAction::Settings);
        assert_eq!(Screen::SettingsTitle.cancel(), Screen::Title);
        assert_eq!(Screen::SettingsPause.cancel(), Screen::Pause);
        assert_eq!(Screen::Title.choose(2, false), MenuAction::Audio);
        assert_eq!(Screen::Title.choose(3, true), MenuAction::Audio);
        assert_eq!(Screen::AudioTitle.cancel(), Screen::Title);
        assert_eq!(Screen::AudioPause.cancel(), Screen::Pause);
        assert_eq!(Screen::Title.choose(3, false), MenuAction::Editor);
        assert_eq!(Screen::Title.choose(4, true), MenuAction::Editor);
        assert_eq!(Screen::Pause.choose(7, true), MenuAction::Editor);
        assert_eq!(Screen::Pause.choose(8, true), MenuAction::Debug);
        assert_eq!(Screen::Debug.cancel(), Screen::Playing);
        assert!(menu_rect(9, 10).bottom() < VIEW_HEIGHT);
        assert_eq!(Screen::Pause.labels(true).len(), 10);
        assert!(menu_rect(7, 8).bottom() < VIEW_HEIGHT);
        assert!(menu_rect(6, 7).bottom() < VIEW_HEIGHT);
        assert_eq!(navigate(0, -1, 4), 3);
        assert_eq!(navigate(3, 1, 4), 0);
    }

    #[test]
    fn invalid_bindings_are_not_applied_even_in_isolated_mode() {
        let mut current = Bindings::default();
        let original = current.clone();
        let mut invalid = current.clone();
        invalid.jump.clear();
        assert!(
            apply_bindings(&mut current, invalid, Path::new("unused-config.json"), true).is_err()
        );
        assert_eq!(current, original);
        assert!(menu_rect(4, 5).bottom() < VIEW_HEIGHT);
    }

    #[test]
    fn config_write_failure_keeps_active_bindings() {
        // Un fichier a la place du dossier simule un refus d'ecriture sans toucher au profil du joueur.
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "lumen-config-error-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let blocked = directory.join("blocked");
        std::fs::write(&blocked, b"not a directory").unwrap();
        let mut current = Bindings::default();
        let original = current.clone();
        let mut updated = current.clone();
        updated.assign(Action::Jump, Key::B).unwrap();
        assert!(
            apply_bindings(&mut current, updated, &blocked.join("config.json"), false).is_err()
        );
        assert_eq!(current, original);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn actor_stops_on_the_floor() {
        let mut world = World::new();
        world.add_static_tiled_layer(vec![Tile::Empty, Tile::Solid], 32.0, 32.0, 1, 1);
        let actor = world.add_actor(vec2(2.0, 0.0), 16, 24);
        world.move_v(actor, 50.0);
        assert_eq!(world.actor_pos(actor).y, 8.0);
        assert!(world.collide_check(actor, world.actor_pos(actor) + vec2(0.0, 1.0)));
    }
}
