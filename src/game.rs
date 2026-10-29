//! Simulation a pas fixe : collisions, combat, progression et effets temporaires.

use macroquad::prelude::*;
use macroquad_platformer::{Actor, Solid, Tile, World};
use serde::{Deserialize, Serialize};

pub const WIDTH: f32 = 960.0;
pub const HEIGHT: f32 = 544.0;
pub const TILE: f32 = 32.0;
pub const COLS: usize = 30;
pub const ROWS: usize = 17;
// Toutes les durees et vitesses sont en secondes et pixels logiques, pas en images.
pub const STEP: f32 = 1.0 / 120.0;
pub const PLAYER_SIZE: Vec2 = Vec2::new(22.0, 34.0);
// Ces indices sont sauvegardes : ajouter les salles a la fin sans renumeroter les anciennes.
pub const NAMES: [&str; 7] = [
    "LE REFUGE",
    "LES GALERIES",
    "LA FORGE",
    "LA TOUR",
    "LE COEUR",
    "LA SERRE",
    "LES SOURCES",
];
pub const LINKS: [(usize, usize); 6] = [(0, 1), (1, 2), (0, 3), (3, 4), (2, 5), (5, 6)];
// Duree de l'ecrasement visuel apres un atterrissage, partagee avec le rendu.
pub const LANDING_SQUASH_TIME: f32 = 0.12;
// Duree du flash et de l'etourdissement d'un ennemi touche.
const STAGGER_TIME: f32 = 0.24;

/// Cycle des piques pulsants : sortis, rentres, puis avertissement avant de ressortir.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HazardPhase {
    Armed,
    Retracted,
    Warning,
}

const HAZARD_CYCLE: f32 = 3.0;

pub fn hazard_phase(time: f32) -> HazardPhase {
    let cycle = time.rem_euclid(HAZARD_CYCLE);
    if cycle < 1.6 {
        HazardPhase::Armed
    } else if cycle < 2.6 {
        HazardPhase::Retracted
    } else {
        HazardPhase::Warning
    }
}

/// Position d'une plateforme en va-et-vient, adoucie aux extremites ; deterministe selon le temps.
pub fn platform_position(from: Vec2, to: Vec2, period: f32, time: f32) -> Vec2 {
    let phase = (time / period).rem_euclid(2.0);
    let linear = if phase < 1.0 { phase } else { 2.0 - phase };
    from + (to - from) * (linear * linear * (3.0 - 2.0 * linear))
}

pub struct PlatformState {
    pub solid: Solid,
    pub from: Vec2,
    pub to: Vec2,
    pub size: Vec2,
    pub period: f32,
}

/// Reglage de confort : il ne touche ni la sauvegarde ni les collisions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Calm,
    #[default]
    Normal,
    Hard,
}

impl Difficulty {
    fn level(self) -> &'static crate::balance::LevelTuning {
        let levels = &crate::balance::active().difficulty;
        match self {
            Self::Calm => &levels[0],
            Self::Normal => &levels[1],
            Self::Hard => &levels[2],
        }
    }

    fn bonus_hearts(self) -> i32 {
        self.level().bonus_hearts
    }

    fn invincibility(self) -> f32 {
        self.level().invincibility
    }

    /// Multiplie le delai entre deux attaques ennemies : plus petit = plus agressif.
    fn attack_pace(self) -> f32 {
        self.level().attack_pace
    }
}

/// Seule la progression durable est persistee, pas les ennemis ou la physique courante.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
// Les champs ajoutes restent facultatifs dans les sauvegardes des versions precedentes.
#[serde(default)]
pub struct Save {
    pub checkpoint: usize,
    pub double_jump: bool,
    pub won: bool,
    pub life_shard: bool,
    pub visited: Vec<bool>,
    pub defeated_enemies: u32,
    pub equipped_weapon: Option<String>,
    pub defeated_bosses: Vec<usize>,
    /// Compteurs locaux de morts, utiles pour reperer les salles trop dures.
    pub deaths: u32,
    pub death_rooms: Vec<u32>,
    /// Monnaie des talents, gagnee en vainquant des ennemis.
    pub shards: u32,
    pub talents: Vec<crate::talents::Talent>,
    /// Heros actif ; un heros non debloque est remplace par Lumen au chargement.
    pub character: crate::characters::Character,
}

impl Save {
    pub fn has_talent(&self, talent: crate::talents::Talent) -> bool {
        self.talents.contains(&talent)
    }

    /// Vie de base avant difficulte et aides de debug : eclat de vie et talent Vigueur.
    pub fn base_health(&self) -> i32 {
        let base = if self.life_shard { 6 } else { 5 };
        (base
            + i32::from(self.has_talent(crate::talents::Talent::Vigor))
            + self.character.stats().hearts)
            .max(1)
    }

    pub fn record_death(&mut self, room: usize) {
        self.deaths = self.deaths.saturating_add(1).min(9999);
        if room < crate::levels::Levels::MAX_ROOMS {
            if self.death_rooms.len() <= room {
                self.death_rooms.resize(room + 1, 0);
            }
            self.death_rooms[room] = self.death_rooms[room].saturating_add(1).min(999);
        }
    }
    pub fn sanitize(mut self) -> Self {
        self.defeated_bosses
            .retain(|room| *room < crate::levels::Levels::MAX_ROOMS);
        self.defeated_bosses.sort_unstable();
        self.defeated_bosses.dedup();
        self.defeated_enemies = self.defeated_enemies.min(200);
        self.deaths = self.deaths.min(9999);
        self.shards = self.shards.min(crate::talents::MAX_SHARDS);
        if !self
            .character
            .unlocked(self.defeated_enemies, self.defeated_bosses.len())
        {
            self.character = crate::characters::Character::default();
        }
        // Ne garder que des talents connus, uniques et dont le prerequis est present.
        let mut owned: Vec<crate::talents::Talent> = Vec::new();
        for talent in crate::talents::Talent::ALL {
            if self.talents.contains(&talent)
                && talent
                    .requires()
                    .is_none_or(|required| owned.contains(&required))
            {
                owned.push(talent);
            }
        }
        self.talents = owned;
        self.death_rooms.truncate(crate::levels::Levels::MAX_ROOMS);
        for count in &mut self.death_rooms {
            *count = (*count).min(999);
        }
        if self
            .equipped_weapon
            .as_ref()
            .is_some_and(|name| name.trim().is_empty() || name.chars().count() > 32)
        {
            self.equipped_weapon = None;
        }
        // Un point de retour doit correspondre a une salle qui contient un sanctuaire.
        if ![0, 2, 6].contains(&self.checkpoint)
            && !(NAMES.len()..crate::levels::Levels::MAX_ROOMS).contains(&self.checkpoint)
        {
            self.checkpoint = 0;
        }
        // Adapter la carte avant tout acces par indice, y compris aux anciens tableaux de cinq salles.
        self.visited.resize(
            self.visited
                .len()
                .clamp(NAMES.len(), crate::levels::Levels::MAX_ROOMS),
            false,
        );
        self.visited[0] = true;
        self
    }
}

#[derive(Clone, Copy, Default)]
pub struct Input {
    pub direction: f32,
    pub jump: bool,
    pub attack: bool,
    pub attack_held: bool,
    pub next_weapon: bool,
    pub dash: bool,
    pub interact: bool,
}

pub struct Enemy {
    pub actor: Actor,
    pub home: Vec2,
    pub health: i32,
    pub direction: f32,
    pub hurt: f32,
    pub cooldown: f32,
    pub boss: bool,
    pub behavior: crate::levels::Behavior,
    pub species: u8,
    pub skin: crate::characters::Skin,
}

impl Enemy {
    /// Rang du biome (0..9) : trois especes par biome.
    pub fn tier(&self) -> u8 {
        self.species / 3
    }

    /// Les ennemis ordinaires gagnent un point de vie tous les trois biomes.
    pub fn max_health(&self) -> i32 {
        if self.behavior == crate::levels::Behavior::Legacy {
            if self.boss {
                12
            } else {
                2
            }
        } else if self.boss {
            18 + i32::from(self.tier()) * 2
        } else {
            (2 + i32::from(self.species % 3)
                + i32::from(self.tier() / 3)
                + self.skin.health_bonus())
            .max(1)
        }
    }
    /// Un coup interrompt l'attaque en cours des ennemis ordinaires ; les boss gardent leur rythme.
    fn stagger(&mut self) {
        self.hurt = STAGGER_TIME;
        if !self.boss {
            self.cooldown = self.cooldown.max(0.7);
        }
    }
    pub fn size(&self) -> Vec2 {
        if self.boss {
            vec2(46.0, 60.0)
        } else {
            vec2(26.0, 28.0)
        }
    }
}

pub struct Particle {
    pub position: Vec2,
    pub velocity: Vec2,
    pub remaining: f32,
    pub color: Color,
}

pub struct Ripple {
    pub position: Vec2,
    pub remaining: f32,
    pub radius: f32,
    pub color: Color,
}

// Ces instantanes n'ont ni acteur ni collision ; leur cadence suit le pas fixe.
pub struct Afterimage {
    pub position: Vec2,
    pub facing: f32,
    pub remaining: f32,
    pub dash: bool,
}

pub struct Impact {
    pub position: Vec2,
    pub remaining: f32,
    pub strength: f32,
    pub color: Color,
}

pub struct CombatText {
    pub position: Vec2,
    pub remaining: f32,
    pub amount: i32,
    pub color: Color,
}

fn push_effect<T>(effects: &mut Vec<T>, effect: T, limit: usize) {
    if effects.len() >= limit {
        effects.remove(0);
    }
    effects.push(effect);
}

/// Evenements factuels de simulation ; la lecture audio appartient a la boucle principale.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundCue {
    Jump,
    DoubleJump,
    Land,
    Attack,
    Dash,
    Hurt,
    Death,
    Hit,
    Defeat,
    Pickup,
    Shrine,
    Portal,
    Victory,
    Menu,
}

impl SoundCue {
    pub const ALL: [Self; 14] = [
        Self::Jump,
        Self::DoubleJump,
        Self::Land,
        Self::Attack,
        Self::Dash,
        Self::Hurt,
        Self::Death,
        Self::Hit,
        Self::Defeat,
        Self::Pickup,
        Self::Shrine,
        Self::Portal,
        Self::Victory,
        Self::Menu,
    ];
}

pub struct Projectile {
    pub position: Vec2,
    pub velocity: Vec2,
    pub remaining: f32,
    pub radius: f32,
    pub damage: i32,
    pub color: Color,
    pub laser: bool,
}

pub struct HostileShot {
    pub position: Vec2,
    pub velocity: Vec2,
    pub remaining: f32,
    pub radius: f32,
}

/// Un boss sous le seuil de vitalite attaque plus souvent et se deplace plus vite.
fn enrage_scales(boss: bool, health: i32, max_health: i32) -> (f32, f32) {
    let tuning = &crate::balance::active().enemies;
    if boss && health as f32 <= max_health as f32 * tuning.enrage_threshold {
        (tuning.enrage_cooldown, tuning.enrage_speed)
    } else {
        (1.0, 1.0)
    }
}

/// Anneau lent ajoute par un boss enrage ; le decalage fait tourner les breches entre deux salves.
fn enraged_ring(species: u8, offset: f32) -> Vec<Vec2> {
    let speed = (130.0 + f32::from(species) * 3.0) * 0.6;
    (0..6)
        .map(|index| Vec2::from_angle(offset + index as f32 * std::f32::consts::TAU / 6.0) * speed)
        .collect()
}

// Les profils sont purs pour tester les attaques sans contexte graphique ni aleatoire.
pub fn attack_vectors(
    behavior: crate::levels::Behavior,
    aim: Vec2,
    boss: bool,
    species: u8,
) -> Vec<Vec2> {
    use crate::levels::Behavior;
    let aim = aim.normalize_or_zero();
    let speed = 130.0 + f32::from(species) * 3.0;
    let count = if boss { 5 } else { 2 };
    match behavior {
        Behavior::Legacy => vec![],
        Behavior::Charge => vec![vec2(aim.x.signum(), 0.0) * speed],
        Behavior::Leap => vec![vec2(-0.7, -0.7) * speed, vec2(0.7, -0.7) * speed],
        Behavior::Orbit => (0..count)
            .map(|index| {
                Vec2::from_angle(index as f32 * std::f32::consts::TAU / count as f32) * speed
            })
            .collect(),
        Behavior::Volley => (0..count)
            .map(|index| {
                aim.rotate(Vec2::from_angle(
                    (index as f32 - (count - 1) as f32 / 2.0) * 0.22,
                )) * speed
            })
            .collect(),
        Behavior::Wave => vec![vec2(-1.0, 0.0) * speed, vec2(1.0, 0.0) * speed],
        Behavior::Ambush => vec![aim * speed * 1.7],
        Behavior::Spiral => (0..count)
            .map(|index| aim.rotate(Vec2::from_angle(index as f32 * 0.9)) * speed)
            .collect(),
        Behavior::Burst => (0..count)
            .map(|index| aim * speed * (0.65 + index as f32 * 0.2))
            .collect(),
        Behavior::Sweep => (0..count)
            .map(|index| vec2(aim.x.signum(), -0.7 + index as f32 * 0.3).normalize() * speed)
            .collect(),
        Behavior::Hunt => vec![aim * speed, aim.rotate(Vec2::from_angle(0.5)) * speed * 0.8],
    }
}

/// Aides temporaires : elles ne font jamais partie du format de sauvegarde.
#[derive(Clone, Default)]
pub struct DebugOptions {
    pub invulnerable: bool,
    pub infinite_jumps: bool,
    pub no_cooldowns: bool,
    pub health_bonus: i32,
}

pub struct Game {
    pub debug: DebugOptions,
    pub projectiles: Vec<Projectile>,
    pub hostile_shots: Vec<HostileShot>,
    pub weapons: Vec<crate::content::Weapon>,
    pub weapon_index: usize,
    pub all_weapons: bool,
    queued_weapon: Option<usize>,
    struck: Vec<usize>,
    pub levels: crate::levels::Levels,
    pub world: World,
    pub actor: Actor,
    pub tiles: Vec<Tile>,
    pub room: usize,
    pub save: Save,
    pub velocity: Vec2,
    pub health: i32,
    pub facing: f32,
    pub grounded: bool,
    pub jumps: u8,
    /// Tolerance apres avoir quitte une plateforme, pour ne pas perdre un saut tardif.
    pub coyote: f32,
    /// Memorise un saut demande juste avant l'atterrissage.
    pub jump_buffer: f32,
    pub attack_time: f32,
    pub attack_cooldown: f32,
    pub dash_time: f32,
    pub dash_cooldown: f32,
    pub dash_available: bool,
    pub invincible: f32,
    /// Fige la simulation quelques instants pour donner du poids aux coups.
    pub hitstop: f32,
    /// Bloque le controle horizontal apres un saut mural pour garder l'impulsion.
    pub wall_lock: f32,
    /// Temps restant de l'ecrasement visuel apres un atterrissage.
    pub landing_squash: f32,
    /// Vrai pendant une glissade contre un mur ; limite la chute.
    pub wall_slide: bool,
    pub difficulty: Difficulty,
    pub platforms: Vec<PlatformState>,
    pub enemies: Vec<Enemy>,
    pub particles: Vec<Particle>,
    pub ripples: Vec<Ripple>,
    pub afterimages: Vec<Afterimage>,
    pub impacts: Vec<Impact>,
    pub combat_text: Vec<CombatText>,
    afterimage_cooldown: f32,
    pub damage_flash: f32,
    pub sounds: Vec<SoundCue>,
    pub notice: String,
    pub notice_time: f32,
    pub room_time: f32,
    pub shake: f32,
    pub save_dirty: bool,
    pub time: f32,
    pub finished: bool,
}

/// Grille de collision en ordre ligne/colonne, independante des decorations dessinees.
pub fn layout(room: usize) -> Vec<Tile> {
    let mut tiles = vec![Tile::Empty; COLS * ROWS];
    let mut fill = |left: usize, top: usize, width: usize, height: usize| {
        for row in top..(top + height).min(ROWS) {
            for col in left..(left + width).min(COLS) {
                tiles[row * COLS + col] = Tile::Solid;
            }
        }
    };
    fill(0, 15, COLS, 2);
    fill(0, 0, COLS, 1);
    match room {
        0 => {
            fill(0, 0, 1, ROWS);
            fill(4, 12, 5, 1);
            fill(12, 9, 5, 1);
            fill(4, 5, 6, 1);
            fill(21, 12, 5, 1);
        }
        1 => {
            fill(7, 13, 4, 2);
            fill(12, 10, 5, 1);
            fill(20, 12, 5, 1);
        }
        2 => {
            fill(29, 0, 1, ROWS);
            fill(7, 12, 4, 1);
            fill(13, 10, 4, 1);
            fill(19, 8, 6, 1);
        }
        3 => {
            fill(0, 0, 1, ROWS);
            fill(5, 12, 4, 1);
            fill(12, 10, 4, 1);
            fill(21, 12, 4, 1);
        }
        4 => {
            fill(29, 0, 1, ROWS);
            fill(5, 12, 4, 1);
            fill(21, 12, 4, 1);
        }
        5 => {
            fill(0, 0, 1, ROWS);
            fill(6, 12, 4, 1);
            fill(13, 10, 4, 1);
            fill(20, 12, 4, 1);
        }
        6 => {
            fill(29, 0, 1, ROWS);
            fill(5, 12, 4, 1);
            fill(12, 10, 4, 1);
            fill(19, 8, 6, 1);
        }
        _ => {}
    }
    tiles
}

pub fn shrine(room: usize) -> Option<Vec2> {
    match room {
        0 => Some(vec2(96.0, 480.0)),
        2 => Some(vec2(856.0, 480.0)),
        6 => Some(vec2(856.0, 480.0)),
        _ => None,
    }
}

pub fn hazards(room: usize) -> Vec<Rect> {
    match room {
        1 => vec![Rect::new(352.0, 470.0, 224.0, 10.0)],
        3 => vec![Rect::new(304.0, 470.0, 304.0, 10.0)],
        5 => vec![Rect::new(320.0, 470.0, 320.0, 10.0)],
        _ => vec![],
    }
}

impl Game {
    #[cfg(test)]
    pub fn new(save: Save) -> Self {
        Self::with_levels(save, crate::levels::Levels::original())
    }

    // Le bac a sable et le jeu normal consomment les memes donnees de collision.
    pub fn with_levels(save: Save, levels: crate::levels::Levels) -> Self {
        let mut save = save.sanitize();
        // Un pack retire peut rendre un ancien sanctuaire inconnu ; revenir au refuge.
        if levels
            .rooms
            .get(save.checkpoint)
            .is_none_or(|room| room.shrine.is_none())
        {
            save.checkpoint = 0;
        }
        save.visited.resize(levels.rooms.len(), false);
        let mut world = World::new();
        let actor = world.add_actor(Vec2::ZERO, 22, 34);
        let mut game = Self {
            debug: DebugOptions::default(),
            projectiles: vec![],
            hostile_shots: vec![],
            weapons: crate::content::default_weapons(),
            weapon_index: 0,
            all_weapons: false,
            queued_weapon: None,
            struck: vec![],
            levels,
            world,
            actor,
            tiles: vec![],
            room: 0,
            health: save.base_health(),
            save,
            velocity: Vec2::ZERO,
            facing: 1.0,
            grounded: false,
            jumps: 0,
            coyote: 0.0,
            jump_buffer: 0.0,
            attack_time: 0.0,
            attack_cooldown: 0.0,
            dash_time: 0.0,
            dash_cooldown: 0.0,
            dash_available: true,
            invincible: 0.0,
            hitstop: 0.0,
            wall_lock: 0.0,
            landing_squash: 0.0,
            wall_slide: false,
            difficulty: Difficulty::default(),
            platforms: vec![],
            enemies: vec![],
            particles: vec![],
            ripples: vec![],
            afterimages: vec![],
            impacts: vec![],
            combat_text: vec![],
            afterimage_cooldown: 0.0,
            damage_flash: 0.0,
            sounds: vec![],
            notice: String::new(),
            notice_time: 0.0,
            room_time: 0.0,
            shake: 0.0,
            save_dirty: false,
            time: 0.0,
            finished: false,
        };
        game.enter(game.save.checkpoint, game.checkpoint_position());
        // Ne pas remplacer un nom absent ici : le pack personnalise sera charge par l'appelant.
        game.weapon_index = game
            .weapons
            .iter()
            .position(|weapon| {
                game.save.equipped_weapon.as_ref() == Some(&weapon.name)
                    && weapon.unlock_kills <= game.save.defeated_enemies
            })
            .unwrap_or(0);
        game
    }

    pub fn position(&self) -> Vec2 {
        self.world.actor_pos(self.actor)
    }

    pub fn max_health(&self) -> i32 {
        self.save.base_health()
            + self.difficulty.bonus_hearts()
            + self.debug.health_bonus.clamp(0, 4)
    }

    pub fn talent_state(&self, talent: crate::talents::Talent) -> crate::talents::TalentState {
        use crate::talents::TalentState;
        if self.save.has_talent(talent) {
            TalentState::Owned
        } else if talent
            .requires()
            .is_some_and(|required| !self.save.has_talent(required))
        {
            TalentState::Locked
        } else if self.save.shards < talent.cost() {
            TalentState::TooExpensive
        } else {
            TalentState::Available
        }
    }

    /// Depense les eclats et enregistre le talent ; Vigueur soigne du coeur gagne.
    pub fn buy_talent(&mut self, talent: crate::talents::Talent) -> Result<(), &'static str> {
        use crate::talents::TalentState;
        match self.talent_state(talent) {
            TalentState::Owned => return Err("DEJA ACQUIS"),
            TalentState::Locked => return Err("PREREQUIS MANQUANT"),
            TalentState::TooExpensive => return Err("ECLATS INSUFFISANTS"),
            TalentState::Available => {}
        }
        self.save.shards -= talent.cost();
        self.save.talents.push(talent);
        self.save_dirty = true;
        if talent == crate::talents::Talent::Vigor {
            self.health = (self.health + 1).min(self.max_health());
        }
        self.sound(SoundCue::Pickup);
        Ok(())
    }

    fn gain_shards(&mut self, base: u32) {
        let gained = crate::talents::shards_gained(
            base,
            self.save.has_talent(crate::talents::Talent::Magnet),
        );
        self.save.shards = (self.save.shards + gained).min(crate::talents::MAX_SHARDS);
        self.save_dirty = true;
    }

    /// Bonus de degats : Tranchant pour tout, plus le bonus du heros selon l'arme (distance ou melee).
    fn damage_bonus(&self, ranged: bool) -> i32 {
        let hero = self.save.character.stats();
        i32::from(self.save.has_talent(crate::talents::Talent::Edge))
            + if ranged {
                hero.ranged_damage
            } else {
                hero.melee_damage
            }
    }

    pub fn character_unlocked(&self, character: crate::characters::Character) -> bool {
        self.all_weapons
            || character.unlocked(self.save.defeated_enemies, self.save.defeated_bosses.len())
    }

    /// Change de heros ; les coeurs gagnes ou perdus suivent le maximum, sans jamais tuer.
    pub fn select_character(
        &mut self,
        character: crate::characters::Character,
    ) -> Result<(), &'static str> {
        if !self.character_unlocked(character) {
            return Err("VERROUILLE");
        }
        if self.save.character == character {
            return Err("DEJA SELECTIONNE");
        }
        self.switch_character(character);
        Ok(())
    }

    fn switch_character(&mut self, character: crate::characters::Character) {
        let before = self.max_health();
        self.save.character = character;
        self.health = (self.health + self.max_health() - before).clamp(1, self.max_health());
        self.save_dirty = true;
        self.sound(SoundCue::Pickup);
    }

    /// Le debug choisit n'importe quel heros ; une sauvegarde qui le garderait verrouille revient a Lumen au chargement.
    pub fn debug_select_character(&mut self, character: crate::characters::Character) {
        if self.save.character != character {
            self.switch_character(character);
        }
    }

    /// Annonce les heros devenus disponibles depuis l'etat donne.
    fn announce_characters(&mut self, kills_before: u32, bosses_before: usize) {
        if self.all_weapons {
            return;
        }
        let (kills, bosses) = (self.save.defeated_enemies, self.save.defeated_bosses.len());
        let unlocked: Vec<_> = crate::characters::Character::ALL
            .into_iter()
            .filter(|character| {
                !character.unlocked(kills_before, bosses_before)
                    && character.unlocked(kills, bosses)
            })
            .map(|character| character.name())
            .collect();
        if !unlocked.is_empty() {
            self.sound(SoundCue::Pickup);
            self.message(&format!(
                "Personnage debloque : {} (Pause > Personnages)",
                unlocked.join(", ")
            ));
        }
    }

    /// Les coeurs gagnes ou perdus par le changement suivent le maximum, sans jamais tuer.
    pub fn set_difficulty(&mut self, difficulty: Difficulty) {
        let before = self.max_health();
        self.difficulty = difficulty;
        self.health = (self.health + self.max_health() - before).clamp(1, self.max_health());
    }

    /// Le point de test du pack est prefere aux coordonnees fixes, notamment pour les extensions.
    pub fn debug_enter(&mut self, room: usize) {
        if let Some(level) = self.levels.rooms.get(room) {
            let point = level.spawn;
            self.finished = false;
            self.enter(room, vec2(point[0], point[1]));
            self.invincible = 2.0;
        }
    }

    pub fn debug_unlock_weapons(&mut self) {
        self.save.defeated_enemies = 200;
        self.save_dirty = true;
    }

    /// Une commande explicite de debug remplace aussi un changement d'arme deja en attente.
    pub fn debug_equip(&mut self, index: usize) {
        self.attack_time = 0.0;
        self.queued_weapon = None;
        self.equip(index);
    }

    /// Revenir au sanctuaire conserve les aides de session mais reconstruit tous les acteurs.
    pub fn debug_respawn(&mut self) {
        self.health = self.max_health();
        self.enter(self.save.checkpoint, self.checkpoint_position());
        self.invincible = 2.0;
    }

    pub fn bounds(&self) -> Rect {
        let position = self.position();
        Rect::new(position.x, position.y, PLAYER_SIZE.x, PLAYER_SIZE.y)
    }

    /// Retourne -1 ou 1 si le joueur touche un mur en l'air ; l'Aile d'echo debloque ce mouvement.
    fn wall_side(&self) -> f32 {
        if self.grounded || !self.save.double_jump {
            return 0.0;
        }
        let position = self.position();
        if self
            .world
            .collide_check(self.actor, position - vec2(1.0, 0.0))
        {
            -1.0
        } else if self
            .world
            .collide_check(self.actor, position + vec2(1.0, 0.0))
        {
            1.0
        } else {
            0.0
        }
    }

    fn checkpoint_position(&self) -> Vec2 {
        self.levels.rooms[self.save.checkpoint]
            .shrine
            .map(|point| vec2(point[0], point[1]))
            .unwrap_or(vec2(96.0, 480.0))
            - vec2(0.0, 34.0)
    }

    pub fn shrine(&self) -> Option<Vec2> {
        self.levels.rooms[self.room]
            .shrine
            .map(|point| vec2(point[0], point[1]))
    }

    /// Les piques fixes restent toujours sortis ; seules les salles pulsantes alternent.
    pub fn hazard_phase(&self) -> HazardPhase {
        if self.levels.rooms[self.room].pulsing {
            hazard_phase(self.time)
        } else {
            HazardPhase::Armed
        }
    }

    fn move_platforms(&mut self) {
        for index in 0..self.platforms.len() {
            let platform = &self.platforms[index];
            let target = platform_position(platform.from, platform.to, platform.period, self.time);
            let current = self.world.solid_pos(platform.solid);
            self.world
                .solid_move(platform.solid, target.x - current.x, 0.0);
        }
    }

    pub fn hazards(&self) -> Vec<Rect> {
        self.levels.rooms[self.room]
            .hazards
            .iter()
            .map(|rect| Rect::new(rect[0], rect[1], rect[2], rect[3]))
            .collect()
    }

    pub fn portals(&self) -> Vec<(Vec2, usize, Vec2)> {
        let mut portals: Vec<_> = self
            .levels
            .portals
            .iter()
            .filter_map(|portal| {
                if portal
                    .requires_boss
                    .is_some_and(|room| !self.save.defeated_bosses.contains(&room))
                {
                    return None;
                }
                let (point, target, arrival) = if portal.from == self.room {
                    (portal.entrance, portal.to, portal.exit)
                } else if portal.to == self.room {
                    (portal.exit, portal.from, portal.entrance)
                } else {
                    return None;
                };
                Some((
                    vec2(point[0], point[1]),
                    target,
                    vec2(arrival[0], arrival[1]),
                ))
            })
            .collect();
        // Le dernier gardien ouvre le retour au refuge, plutot qu'une onzieme zone.
        if self.levels.rooms[self.room].biome == Some(9)
            && self.levels.rooms[self.room]
                .enemies
                .iter()
                .any(|enemy| enemy.boss)
            && self.save.defeated_bosses.contains(&self.room)
        {
            portals.push((vec2(856.0, 446.0), 0, vec2(96.0, 446.0)));
        }
        portals
    }

    fn use_generated_portal(&mut self) -> bool {
        let position = self.position();
        let portal = self
            .portals()
            .into_iter()
            .find(|(point, _, _)| (position - *point).length() < 48.0);
        if let Some((_, target, arrival)) = portal {
            self.enter(target, arrival);
            true
        } else {
            false
        }
    }

    /// Reconstruit la salle et ses acteurs ; room doit etre un indice valide du pack.
    pub fn enter(&mut self, room: usize, position: Vec2) {
        if self.time > 0.0 {
            self.sound(SoundCue::Portal);
        }
        self.room = room;
        self.tiles = self.levels.rooms[room].collision();
        // Les identifiants Actor appartiennent a leur World : recreer aussi joueur et ennemis.
        self.world = World::new();
        self.world
            .add_static_tiled_layer(self.tiles.clone(), TILE, TILE, COLS, 1);
        self.actor = self.world.add_actor(position, 22, 34);
        // Les solides appartiennent au World recree : reposer les plateformes a l'instant courant.
        self.platforms = self.levels.rooms[room]
            .platforms
            .iter()
            .map(|platform| {
                let from = vec2(platform.from[0], platform.from[1]);
                let to = vec2(platform.to[0], platform.to[1]);
                let size = vec2(
                    f32::from(platform.tiles) * TILE,
                    crate::levels::PLATFORM_HEIGHT,
                );
                let start = platform_position(from, to, platform.period, self.time);
                PlatformState {
                    solid: self.world.add_solid(start, size.x as i32, size.y as i32),
                    from,
                    to,
                    size,
                    period: platform.period,
                }
            })
            .collect();
        self.velocity = Vec2::ZERO;
        self.jumps = 0;
        self.coyote = 0.0;
        self.jump_buffer = 0.0;
        self.grounded = false;
        self.attack_time = 0.0;
        self.queued_weapon = None;
        self.dash_time = 0.0;
        self.dash_cooldown = 0.0;
        self.dash_available = true;
        self.enemies.clear();
        self.projectiles.clear();
        self.hostile_shots.clear();
        self.particles.clear();
        self.ripples.clear();
        self.afterimages.clear();
        self.impacts.clear();
        self.combat_text.clear();
        self.afterimage_cooldown = 0.0;
        self.damage_flash = 0.0;
        self.room_time = 2.4;
        self.hitstop = 0.0;
        self.wall_lock = 0.0;
        self.wall_slide = false;
        self.save.visited[room] = true;
        self.save_dirty = true;
        // Les ennemis ordinaires reapparaissent ; la victoire du gardien reste permanente.
        for spawn in &self.levels.rooms[room].enemies {
            let boss = spawn.boss;
            if boss
                && (self.save.defeated_bosses.contains(&room)
                    || (self.levels.rooms[room].biome.is_none() && self.save.won))
            {
                continue;
            }
            let home = vec2(spawn.position[0], spawn.position[1]);
            let size = if boss {
                vec2(46.0, 60.0)
            } else {
                vec2(26.0, 28.0)
            };
            let actor = self.world.add_actor(home, size.x as i32, size.y as i32);
            let mut enemy = Enemy {
                actor,
                home,
                boss,
                health: if boss { 12 } else { 2 },
                direction: -1.0,
                hurt: 0.0,
                cooldown: 1.6,
                species: spawn.species,
                skin: crate::characters::Skin::from_index(spawn.skin).unwrap_or_default(),
                behavior: spawn.behavior,
            };
            enemy.health = enemy.max_health();
            self.enemies.push(enemy);
        }
    }

    // Borner la file si un appelant simule longtemps sans consommer les evenements audio.
    pub fn sound(&mut self, cue: SoundCue) {
        if self.sounds.len() < 32 {
            self.sounds.push(cue);
        }
    }

    pub fn message(&mut self, message: &str) {
        self.notice = message.to_string();
        self.notice_time = 3.2;
    }

    pub fn burst(&mut self, position: Vec2, color: Color, amount: usize) {
        // Garder les effets les plus recents, meme lors de frappes sur plusieurs cibles.
        push_effect(
            &mut self.ripples,
            Ripple {
                position,
                remaining: 0.45,
                radius: 18.0 + amount as f32,
                color,
            },
            32,
        );
        if amount >= 8 {
            push_effect(
                &mut self.impacts,
                Impact {
                    position,
                    remaining: 0.35,
                    strength: (amount as f32 / 36.0).min(1.0),
                    color,
                },
                8,
            );
        }
        for index in 0..amount.min(256) {
            // L'angle d'or disperse les particules sans ajouter d'aleatoire aux captures.
            let angle = index as f32 * 2.399_963 + self.time;
            push_effect(
                &mut self.particles,
                Particle {
                    position,
                    velocity: vec2(angle.cos(), angle.sin()) * (60.0 + (index % 5) as f32 * 22.0),
                    remaining: 0.5 + (index % 3) as f32 * 0.12,
                    color,
                },
                256,
            );
        }
    }

    pub fn damage(&mut self, direction: f32) {
        if self.debug.invulnerable || self.invincible > 0.0 || self.dash_time > 0.0 {
            return;
        }
        self.health -= 1;
        self.hitstop = self.hitstop.max(0.06);
        self.sound(SoundCue::Hurt);
        self.invincible = self.difficulty.invincibility()
            + self.save.character.stats().invincibility
            + if self.save.has_talent(crate::talents::Talent::Reflex) {
                0.5
            } else {
                0.0
            };
        self.velocity = vec2(direction * 250.0, -260.0);
        self.shake = 0.25;
        self.damage_flash = 0.18;
        let position = self.position();
        push_effect(
            &mut self.combat_text,
            CombatText {
                position,
                remaining: 0.8,
                amount: -1,
                color: Color::from_rgba(239, 110, 113, 255),
            },
            32,
        );
        self.burst(
            self.position() + PLAYER_SIZE * 0.5,
            Color::from_rgba(239, 110, 113, 255),
            14,
        );
        if self.health <= 0 {
            self.save.record_death(self.room);
            self.save_dirty = true;
            // Revenir au sanctuaire sans effacer les pouvoirs ni les ameliorations de vie.
            self.health = self.max_health();
            self.enter(self.save.checkpoint, self.checkpoint_position());
            // La mort remplace les sons de blessure et de portail du retour au sanctuaire.
            self.sounds.clear();
            self.sound(SoundCue::Death);
            self.invincible = 2.0;
            self.message("La lumiere vous ramene au refuge.");
        }
    }

    pub fn sword(&self) -> Option<Rect> {
        if self.attack_time <= 0.0 || self.weapon().projectile.is_some() {
            return None;
        }
        let position = self.position();
        let weapon = self.weapon();
        let reach = weapon.reach
            * self.save.character.stats().reach
            * if self.save.has_talent(crate::talents::Talent::Reach) {
                1.15
            } else {
                1.0
            };
        Some(Rect::new(
            if self.facing > 0.0 {
                position.x + 14.0
            } else {
                position.x + 8.0 - reach
            },
            position.y + 18.0 - weapon.height * 0.5,
            reach,
            weapon.height,
        ))
    }

    pub fn weapon(&self) -> &crate::content::Weapon {
        &self.weapons[self.weapon_index]
    }

    pub fn equip(&mut self, index: usize) {
        // Une frappe garde ses caracteristiques jusqu'a sa fin ; la recharge ne peut etre contournee.
        let index = index % self.weapons.len();
        if self.attack_time <= 0.0 && self.weapon_available(index) {
            self.weapon_index = index;
            let name = self.weapon().name.clone();
            if self.save.equipped_weapon.as_ref() != Some(&name) {
                self.save.equipped_weapon = Some(name);
                self.save_dirty = true;
            }
        }
    }

    pub fn weapon_available(&self, index: usize) -> bool {
        self.all_weapons || self.save.defeated_enemies >= self.weapons[index].unlock_kills
    }

    pub fn set_weapons(&mut self, weapons: Vec<crate::content::Weapon>) {
        // Les indices du pack peuvent changer : restaurer par nom, sinon choisir une arme accessible.
        self.weapons = weapons;
        self.queued_weapon = None;
        self.weapon_index = self
            .weapons
            .iter()
            .enumerate()
            .find(|(index, weapon)| {
                self.weapon_available(*index)
                    && self.save.equipped_weapon.as_ref() == Some(&weapon.name)
            })
            .map(|(index, _)| index)
            .unwrap_or_else(|| {
                (0..self.weapons.len())
                    .find(|index| self.weapon_available(*index))
                    .expect("validated starting weapon")
            });
        self.equip(self.weapon_index);
    }

    fn cycle_weapon(&mut self) {
        let current = self.queued_weapon.unwrap_or(self.weapon_index);
        if let Some(next) = (1..=self.weapons.len())
            .map(|offset| (current + offset) % self.weapons.len())
            .find(|index| self.weapon_available(*index))
        {
            self.queued_weapon = Some(next);
        }
    }

    fn advance_projectiles(&mut self, dt: f32) -> Vec<(Vec2, i32, bool, bool)> {
        let mut impacts = Vec::new();
        for projectile in &mut self.projectiles {
            let distance = (projectile.velocity.length() * dt).min(projectile.remaining);
            // Sous-pas courts : un projectile rapide ne traverse ni mur ni cible entre deux images.
            let steps = (distance / 4.0).ceil().max(1.0) as usize;
            let motion = projectile.velocity.normalize_or_zero() * distance / steps as f32;
            for _ in 0..steps {
                projectile.position += motion;
                projectile.remaining -= distance / steps as f32;
                let bounds = Rect::new(
                    projectile.position.x - projectile.radius,
                    projectile.position.y - projectile.radius,
                    projectile.radius * 2.0,
                    projectile.radius * 2.0,
                );
                let blocked = projectile.position.x < 0.0
                    || projectile.position.x > WIDTH
                    || projectile.position.y < 0.0
                    || projectile.position.y > HEIGHT
                    || [
                        vec2(bounds.x, bounds.y),
                        vec2(bounds.x + bounds.w, bounds.y),
                        vec2(bounds.x, bounds.y + bounds.h),
                        vec2(bounds.x + bounds.w, bounds.y + bounds.h),
                    ]
                    .iter()
                    .any(|point| self.world.solid_at(*point));
                if blocked {
                    projectile.remaining = 0.0;
                    break;
                }
                if let Some(enemy) = self.enemies.iter_mut().find(|enemy| {
                    let position = self.world.actor_pos(enemy.actor);
                    let size = enemy.size();
                    enemy.health > 0
                        && bounds.overlaps(&Rect::new(position.x, position.y, size.x, size.y))
                }) {
                    enemy.health -= projectile.damage;
                    enemy.stagger();
                    impacts.push((
                        projectile.position,
                        projectile.damage,
                        enemy.health <= 0,
                        enemy.boss,
                    ));
                    projectile.remaining = 0.0;
                    break;
                }
            }
        }
        self.projectiles
            .retain(|projectile| projectile.remaining > 0.0);
        impacts
    }

    /// Execute un pas de simulation ; l'appelant fournit normalement dt = STEP.
    pub fn update(&mut self, input: Input, dt: f32) {
        let tune = crate::balance::active();
        let hero = self.save.character.stats();
        // Pendant l'arret sur impact, seul le compte a rebours avance ; le monde reste fige.
        if self.hitstop > 0.0 {
            self.hitstop = (self.hitstop - dt).max(0.0);
            return;
        }
        if self.debug.no_cooldowns {
            self.attack_cooldown = 0.0;
            self.dash_cooldown = 0.0;
            self.dash_available = true;
        }
        self.time += dt;
        self.move_platforms();
        self.damage_flash = (self.damage_flash - dt).max(0.0);
        self.invincible = (self.invincible - dt).max(0.0);
        self.attack_time = (self.attack_time - dt).max(0.0);
        self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);
        self.dash_time = (self.dash_time - dt).max(0.0);
        self.dash_cooldown = (self.dash_cooldown - dt).max(0.0);
        self.notice_time = (self.notice_time - dt).max(0.0);
        self.room_time = (self.room_time - dt).max(0.0);
        self.shake = (self.shake - dt).max(0.0);
        self.jump_buffer = (self.jump_buffer - dt).max(0.0);
        self.wall_lock = (self.wall_lock - dt).max(0.0);
        self.landing_squash = (self.landing_squash - dt).max(0.0);
        let was_grounded = self.grounded;
        self.grounded = self
            .world
            .collide_check(self.actor, self.position() + vec2(0.0, 1.0));
        if self.grounded {
            if !was_grounded {
                self.landing_squash = LANDING_SQUASH_TIME;
                if self.room_time < 2.3 {
                    self.sound(SoundCue::Land);
                }
                self.burst(
                    self.position() + vec2(PLAYER_SIZE.x * 0.5, PLAYER_SIZE.y),
                    Color::from_rgba(167, 186, 156, 255),
                    6,
                );
            }
            // Le sol recharge la ruee ; le cooldown reste une contrainte distincte.
            self.dash_available = true;
            self.jumps = 0;
            self.coyote = tune.player.coyote;
        } else {
            self.coyote = (self.coyote - dt).max(0.0);
        }
        if input.jump {
            self.jump_buffer = tune.player.jump_buffer;
        }
        let can_ground_jump = self.grounded || self.coyote > 0.0;
        let can_air_jump = self.debug.infinite_jumps || (self.save.double_jump && self.jumps < 2);
        let wall = self.wall_side();
        // Glisser seulement en poussant vers le mur : sans direction, le joueur tombe normalement.
        self.wall_slide = wall != 0.0 && input.direction * wall > 0.0 && self.velocity.y > 0.0;
        if wall != 0.0 && self.dash_time <= 0.0 && self.jump_buffer > 0.0 {
            // Le saut mural est prioritaire : il pousse a l'oppose du mur et bloque brievement le controle.
            self.velocity = vec2(-wall * tune.player.wall_jump[0], -tune.player.wall_jump[1]);
            self.facing = -wall;
            self.jumps = 1;
            self.coyote = 0.0;
            self.jump_buffer = 0.0;
            self.wall_lock = tune.player.wall_lock;
            self.sound(SoundCue::DoubleJump);
            self.burst(
                self.position()
                    + vec2(
                        if wall < 0.0 { 0.0 } else { PLAYER_SIZE.x },
                        PLAYER_SIZE.y * 0.5,
                    ),
                Color::from_rgba(97, 226, 198, 255),
                14,
            );
        } else if self.dash_time <= 0.0
            && self.jump_buffer > 0.0
            && (can_ground_jump || can_air_jump)
        {
            self.velocity.y = -tune.player.jump_speed * hero.jump;
            self.jumps = if can_ground_jump { 1 } else { 2 };
            self.coyote = 0.0;
            self.jump_buffer = 0.0;
            if can_ground_jump {
                self.sound(SoundCue::Jump);
                self.burst(
                    self.position() + PLAYER_SIZE,
                    Color::from_rgba(167, 186, 156, 255),
                    8,
                );
            } else {
                self.sound(SoundCue::DoubleJump);
                self.burst(
                    self.position() + PLAYER_SIZE,
                    Color::from_rgba(97, 226, 198, 255),
                    18,
                );
            }
        }
        if input.direction != 0.0 && self.dash_time <= 0.0 && self.wall_lock <= 0.0 {
            self.facing = input.direction.signum();
        }
        if input.dash && self.save.double_jump && self.dash_available && self.dash_cooldown <= 0.0 {
            self.dash_time = tune.player.dash_time;
            self.dash_cooldown = tune.player.dash_cooldown
                * hero.dash_cooldown
                * if self.save.has_talent(crate::talents::Talent::Breath) {
                    0.75
                } else {
                    1.0
                };
            self.dash_available = false;
            self.sound(SoundCue::Dash);
            self.burst(
                self.position() + PLAYER_SIZE * 0.5,
                Color::from_rgba(97, 226, 198, 255),
                12,
            );
        }
        if self.dash_time > 0.0 {
            // La ruee verrouille la direction et suspend la gravite, sans ignorer les murs.
            self.velocity = vec2(self.facing * tune.player.dash_speed, 0.0);
        } else {
            if self.wall_lock <= 0.0 {
                let target = input.direction.clamp(-1.0, 1.0) * tune.player.walk_speed * hero.walk;
                let step = tune.player.acceleration * dt;
                self.velocity.x += (target - self.velocity.x).clamp(-step, step);
            }
            let max_fall = if self.wall_slide {
                tune.player.wall_slide_fall
            } else {
                tune.player.max_fall * hero.fall
            };
            self.velocity.y = (self.velocity.y + tune.player.gravity * dt).min(max_fall);
        }
        if !self.world.move_h(self.actor, self.velocity.x * dt) {
            self.velocity.x = 0.0;
            self.dash_time = 0.0;
        }
        if !self.world.move_v(self.actor, self.velocity.y * dt) {
            self.velocity.y = 0.0;
        }
        self.afterimage_cooldown = (self.afterimage_cooldown - dt).max(0.0);
        if (self.dash_time > 0.0 || self.velocity.y.abs() > 420.0)
            && self.afterimage_cooldown <= 0.0
        {
            let position = self.position();
            push_effect(
                &mut self.afterimages,
                Afterimage {
                    position,
                    facing: self.facing,
                    remaining: 0.22,
                    dash: self.dash_time > 0.0,
                },
                24,
            );
            self.afterimage_cooldown = 0.025;
        }
        if input.next_weapon {
            self.cycle_weapon();
        }
        if self.attack_time <= 0.0 {
            if let Some(index) = self.queued_weapon.take() {
                self.equip(index);
            }
        }
        if (input.attack || (input.attack_held && self.weapon().automatic))
            && self.attack_cooldown <= 0.0
        {
            self.attack_time = self.weapon().duration;
            self.attack_cooldown = self.weapon().cooldown;
            self.struck.clear();
            if let Some(settings) = self.weapon().projectile.clone() {
                let weapon = self.weapon();
                let projectile = Projectile {
                    // Capturer le type visuel pour conserver le laser apres un changement d'arme.
                    laser: weapon.laser,
                    position: self.position() + vec2(11.0, 18.0),
                    velocity: vec2(self.facing * settings.speed, 0.0),
                    remaining: settings.range,
                    radius: settings.radius,
                    damage: weapon.damage + self.damage_bonus(true),
                    color: Color::from_rgba(weapon.color[0], weapon.color[1], weapon.color[2], 255),
                };
                push_effect(&mut self.projectiles, projectile, 64);
            }
            self.sound(SoundCue::Attack);
        }
        let damage = self.weapon().damage + self.damage_bonus(false);
        let sword = self.sword();
        let player_center = self.position() + PLAYER_SIZE * 0.5;
        let player_bounds = self.bounds();
        let mut hit = None;
        let projectile_impacts = self.advance_projectiles(dt);
        let mut defeated_boss = projectile_impacts
            .iter()
            .any(|(_, _, dead, boss)| *dead && *boss);
        let mut effects: Vec<_> = projectile_impacts
            .into_iter()
            .map(|(position, amount, dead, _)| (position, dead, amount))
            .collect();
        let mut shots = Vec::new();
        let attack_pace = self.difficulty.attack_pace();
        for (index, enemy) in self.enemies.iter_mut().enumerate() {
            if enemy.health <= 0 {
                continue;
            }
            enemy.hurt = (enemy.hurt - dt).max(0.0);
            enemy.cooldown -= dt;
            let position = self.world.actor_pos(enemy.actor);
            let size = enemy.size();
            let enemy_rect = Rect::new(position.x, position.y, size.x, size.y);
            // Une cible ne recoit qu'un coup par frappe, meme si l'arme reste active longtemps.
            if sword.is_some_and(|sword| sword.overlaps(&enemy_rect))
                && enemy.hurt <= 0.0
                && !self.struck.contains(&index)
            {
                self.struck.push(index);
                enemy.health -= damage;
                enemy.stagger();
                effects.push((position + size * 0.5, enemy.health <= 0, damage));
                if enemy.boss && enemy.health <= 0 {
                    defeated_boss = true;
                }
            }
            if enemy.health <= 0 {
                continue;
            }
            let mut vertical = 650.0;
            let mut custom_speed = None;
            if enemy.behavior != crate::levels::Behavior::Legacy {
                use crate::levels::Behavior;
                let period = (2.0 + f32::from(enemy.species % 3) * 0.3)
                    * (1.0 - f32::from(enemy.tier()) * tune.enemies.pace_per_tier)
                    * enemy.skin.pace();
                enemy.direction = (player_center.x - position.x).signum();
                let phase = self.time * (1.5 + f32::from(enemy.species) * 0.04);
                let speed = match enemy.behavior {
                    Behavior::Charge => {
                        if enemy.cooldown < 0.5 {
                            240.0
                        } else {
                            30.0
                        }
                    }
                    Behavior::Leap => {
                        if enemy.cooldown < 0.6 {
                            vertical = -270.0;
                        }
                        90.0
                    }
                    Behavior::Orbit => {
                        vertical = (enemy.home.y - position.y) * 3.0 + phase.sin() * 90.0;
                        80.0 * phase.cos()
                    }
                    Behavior::Volley => 0.0,
                    Behavior::Wave => {
                        enemy.direction = phase.sin().signum();
                        65.0
                    }
                    Behavior::Ambush => {
                        if enemy.cooldown < 0.35 {
                            300.0
                        } else {
                            0.0
                        }
                    }
                    Behavior::Spiral => {
                        vertical = (enemy.home.y - position.y) * 2.0 + phase.cos() * 120.0;
                        100.0 * phase.sin()
                    }
                    Behavior::Burst => {
                        if enemy.cooldown > 1.0 {
                            -45.0
                        } else {
                            65.0
                        }
                    }
                    Behavior::Sweep => {
                        enemy.direction =
                            (enemy.home.x - position.x + phase.sin() * 120.0).signum();
                        100.0
                    }
                    Behavior::Hunt => {
                        if (player_center.y - position.y).abs() > 60.0 {
                            vertical = (player_center.y - position.y) * 2.0;
                        }
                        125.0
                    }
                    Behavior::Legacy => 55.0,
                };
                let (cooldown_scale, speed_scale) =
                    enrage_scales(enemy.boss, enemy.health, enemy.max_health());
                custom_speed = Some(
                    speed
                        * speed_scale
                        * (1.0 + f32::from(enemy.tier()) * tune.enemies.speed_per_tier)
                        * enemy.skin.speed(),
                );
                if enemy.cooldown <= 0.0 {
                    enemy.cooldown = period * cooldown_scale * attack_pace;
                    let origin = position + size * 0.5;
                    let mut volley = attack_vectors(
                        enemy.behavior,
                        player_center - origin,
                        enemy.boss,
                        enemy.species,
                    );
                    if enemy.boss && cooldown_scale < 1.0 {
                        volley.extend(enraged_ring(enemy.species, self.time));
                    }
                    for velocity in volley {
                        shots.push(HostileShot {
                            position: origin,
                            velocity,
                            remaining: 4.0,
                            radius: if enemy.boss { 7.0 } else { 4.0 },
                        });
                    }
                }
            } else if enemy.boss {
                enemy.direction = (player_center.x - position.x).signum();
                if enemy.cooldown <= 0.0 {
                    enemy.cooldown = 2.4;
                }
            } else if (position.x - enemy.home.x).abs() > 78.0 {
                enemy.direction = (enemy.home.x - position.x).signum();
            }
            // Les ennemis ordinaires sont etourdis et repousses ; les boss ne reculent pas.
            let staggered = enemy.hurt > 0.0 && !enemy.boss;
            let speed = if staggered {
                0.0
            } else {
                custom_speed.unwrap_or(if enemy.boss {
                    if enemy.cooldown < 0.65 {
                        225.0
                    } else {
                        55.0
                    }
                } else {
                    55.0
                })
            };
            let ahead = position
                + vec2(
                    if enemy.direction > 0.0 {
                        size.x + 8.0
                    } else {
                        -8.0
                    },
                    size.y + 4.0,
                );
            if vertical == 650.0 && !self.world.solid_at(ahead) {
                enemy.direction *= -1.0;
            }
            if !self.world.move_h(enemy.actor, enemy.direction * speed * dt) {
                enemy.direction *= -1.0;
            }
            if staggered {
                let recoil =
                    -(player_center.x - position.x).signum() * 170.0 * (enemy.hurt / STAGGER_TIME);
                let edge_x = if recoil > 0.0 {
                    position.x + size.x + 8.0
                } else {
                    position.x - 8.0
                };
                // Un marcheur ne doit pas etre projete dans le vide par un recul.
                if vertical != 650.0 || self.world.solid_at(vec2(edge_x, position.y + size.y + 4.0))
                {
                    self.world.move_h(enemy.actor, recoil * dt);
                }
            }
            self.world.move_v(enemy.actor, vertical * dt);
            let moved = self.world.actor_pos(enemy.actor);
            if Rect::new(moved.x, moved.y, size.x, size.y).overlaps(&player_bounds) {
                hit = Some(if player_center.x < moved.x { -1.0 } else { 1.0 });
            }
        }
        for shot in shots {
            push_effect(&mut self.hostile_shots, shot, 128);
        }
        // Sous-pas courts : une balle ne traverse ni mur ni joueur a grande vitesse.
        for shot in &mut self.hostile_shots {
            shot.remaining -= dt;
            let steps = ((shot.velocity.length() * dt / 4.0).ceil() as usize).max(1);
            for _ in 0..steps {
                shot.position += shot.velocity * dt / steps as f32;
                if self.world.solid_at(shot.position) {
                    shot.remaining = 0.0;
                    break;
                }
                if Rect::new(
                    shot.position.x - shot.radius,
                    shot.position.y - shot.radius,
                    shot.radius * 2.0,
                    shot.radius * 2.0,
                )
                .overlaps(&player_bounds)
                {
                    hit = Some((player_center.x - shot.position.x).signum());
                    shot.remaining = 0.0;
                    break;
                }
            }
        }
        self.hostile_shots.retain(|shot| shot.remaining > 0.0);
        // Creer les effets apres l'emprunt mutable des ennemis pour pouvoir modifier Game.
        let previous_kills = self.save.defeated_enemies;
        let previous_bosses = self.save.defeated_bosses.len();
        for (position, dead, amount) in effects {
            if dead {
                self.save.defeated_enemies = self.save.defeated_enemies.saturating_add(1).min(200);
                self.gain_shards(crate::talents::SHARDS_PER_KILL);
                self.save_dirty = true;
            }
            push_effect(
                &mut self.combat_text,
                CombatText {
                    position,
                    remaining: 0.8,
                    amount,
                    color: Color::from_rgba(246, 193, 113, 255),
                },
                32,
            );
            self.hitstop = self.hitstop.max(if dead { 0.09 } else { 0.05 });
            self.sound(if dead {
                SoundCue::Defeat
            } else {
                SoundCue::Hit
            });
            self.burst(
                position,
                Color::from_rgba(246, 193, 113, 255),
                if dead { 20 } else { 8 },
            );
            self.shake = 0.1;
        }
        let earned: Vec<_> = self
            .weapons
            .iter()
            .filter(|weapon| {
                weapon.unlock_kills > previous_kills
                    && weapon.unlock_kills <= self.save.defeated_enemies
            })
            .map(|weapon| weapon.name.clone())
            .collect();
        if !earned.is_empty() && !self.all_weapons {
            self.sound(SoundCue::Pickup);
            self.message(&format!("Arme gagnee : {}", earned.join(", ")));
        }
        if defeated_boss {
            if !self.save.defeated_bosses.contains(&self.room) {
                self.save.defeated_bosses.push(self.room);
                self.gain_shards(crate::talents::SHARDS_PER_BOSS);
            }
            let biome = self.levels.rooms[self.room].biome;
            if biome.is_none() || biome == Some(9) {
                self.save.won = true;
            }
            self.sound(SoundCue::Victory);
            self.save_dirty = true;
            self.health = self.max_health();
            self.message(if biome == Some(9) {
                "L'Eclipse est tombee. Campagne terminee !"
            } else if biome.is_some() {
                "Boss vaincu ! Le portail du niveau suivant est ouvert."
            } else {
                "Le gardien est tombe. Le coeur peut renaitre."
            });
        }
        self.announce_characters(previous_kills, previous_bosses);
        if let Some(direction) = hit {
            self.damage(direction);
        }
        if self.hazard_phase() == HazardPhase::Armed
            && self
                .hazards()
                .iter()
                .any(|hazard| hazard.overlaps(&self.bounds()))
        {
            self.damage(-self.facing);
        }
        let position = self.position();
        if self.room == 2
            && !self.save.double_jump
            && self.bounds().overlaps(&Rect::new(684.0, 208.0, 40.0, 48.0))
        {
            self.save.double_jump = true;
            self.sound(SoundCue::Pickup);
            self.save_dirty = true;
            self.burst(vec2(704.0, 230.0), Color::from_rgba(97, 226, 198, 255), 36);
            self.message("AILE D'ECHO  /  Double saut et ruee obtenus");
        }
        if self.room == 6
            && !self.save.life_shard
            && self.bounds().overlaps(&Rect::new(684.0, 208.0, 40.0, 48.0))
        {
            self.save.life_shard = true;
            self.sound(SoundCue::Pickup);
            self.health = self.max_health();
            self.save_dirty = true;
            self.burst(vec2(704.0, 230.0), Color::from_rgba(239, 110, 113, 255), 36);
            self.message("ECLAT DE VIE  /  Vitalite augmentee");
        }
        // Les portails exigent une interaction, contrairement aux sorties laterales ci-dessous.
        if input.interact {
            // Une seule destination par pression, meme si l'entree est proche d'un sanctuaire.
            if self.use_generated_portal() {
                return;
            }
            if let Some(point) = self.shrine() {
                if (position + PLAYER_SIZE * 0.5 - (point - vec2(0.0, 20.0))).length() < 65.0 {
                    self.save.checkpoint = self.room;
                    self.sound(SoundCue::Shrine);
                    self.health = self.max_health();
                    self.save_dirty = true;
                    self.message("Sanctuaire active. Votre voyage est sauvegarde.");
                    self.burst(
                        point - vec2(0.0, 24.0),
                        Color::from_rgba(97, 226, 198, 255),
                        24,
                    );
                }
            }
            if self.room == 2
                && self.save.double_jump
                && self.bounds().overlaps(&Rect::new(754.0, 202.0, 52.0, 54.0))
            {
                self.enter(5, vec2(80.0, 446.0));
            } else if self.room == 5 && position.x < 150.0 && position.y > 360.0 {
                self.enter(2, vec2(754.0, 222.0));
            } else if self.room == 0
                && position.y + PLAYER_SIZE.y <= 161.0
                && self.bounds().overlaps(&Rect::new(168.0, 106.0, 64.0, 54.0))
            {
                self.enter(3, vec2(80.0, 446.0));
            } else if self.room == 3 && position.x < 150.0 && position.y > 360.0 {
                self.enter(0, vec2(188.0, 126.0));
            } else if self.room == 2
                && self.save.double_jump
                && position.x > 790.0
                && position.y > 360.0
            {
                self.enter(0, vec2(280.0, 446.0));
            } else if self.room == 4
                && self.save.won
                && self
                    .bounds()
                    .overlaps(&Rect::new(824.0, 380.0, 60.0, 100.0))
            {
                self.finished = true;
                self.sound(SoundCue::Victory);
            }
        }
        let position = self.position();
        if position.x > WIDTH - PLAYER_SIZE.x {
            let next = match self.room {
                0 => Some(1),
                1 => Some(2),
                3 => Some(4),
                5 => Some(6),
                _ => None,
            };
            if let Some(next) = next {
                self.enter(next, vec2(8.0, position.y.clamp(40.0, 446.0)));
            }
        } else if position.x < 0.0 {
            let next = match self.room {
                1 => Some(0),
                2 => Some(1),
                4 => Some(3),
                6 => Some(5),
                _ => None,
            };
            if let Some(next) = next {
                self.enter(next, vec2(928.0, position.y.clamp(40.0, 446.0)));
            }
        }
        if position.y > HEIGHT + 80.0 {
            self.damage(-self.facing);
            self.enter(self.save.checkpoint, self.checkpoint_position());
        }
        // Leur duree de vie suit la physique : les effets se figent donc aussi en pause.
        for particle in &mut self.particles {
            particle.remaining -= dt;
            particle.velocity.y += 180.0 * dt;
            particle.position += particle.velocity * dt;
        }
        self.particles.retain(|particle| particle.remaining > 0.0);
        for ripple in &mut self.ripples {
            ripple.remaining -= dt;
        }
        self.ripples.retain(|ripple| ripple.remaining > 0.0);
        for echo in &mut self.afterimages {
            echo.remaining -= dt;
        }
        self.afterimages.retain(|echo| echo.remaining > 0.0);
        for impact in &mut self.impacts {
            impact.remaining -= dt;
        }
        self.impacts.retain(|impact| impact.remaining > 0.0);
        for text in &mut self.combat_text {
            text.remaining -= dt;
            text.position.y -= 36.0 * dt;
        }
        self.combat_text.retain(|text| text.remaining > 0.0);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn hitstop_freezes_simulation_until_it_expires() {
        let mut game = super::Game::new(super::Save::default());
        game.enter(0, macroquad::prelude::vec2(96.0, 80.0));
        game.hitstop = 0.05;
        let before = game.position();
        game.update(
            super::Input {
                direction: 1.0,
                ..Default::default()
            },
            super::STEP,
        );
        assert_eq!(game.position(), before);
        assert!(game.hitstop < 0.05);
    }

    #[test]
    fn wall_jump_requires_air_upgrade_and_kicks_away() {
        let mut locked = super::Game::new(super::Save::default());
        locked.enter(0, macroquad::prelude::vec2(32.0, 300.0));
        locked.velocity = macroquad::prelude::vec2(0.0, 200.0);
        locked.update(
            super::Input {
                direction: -1.0,
                jump: true,
                ..Default::default()
            },
            super::STEP,
        );
        assert!(locked.velocity.y > 0.0);

        let mut game = super::Game::new(super::Save {
            double_jump: true,
            ..Default::default()
        });
        game.enter(0, macroquad::prelude::vec2(32.0, 300.0));
        game.velocity = macroquad::prelude::vec2(0.0, 200.0);
        game.update(
            super::Input {
                direction: -1.0,
                jump: true,
                ..Default::default()
            },
            super::STEP,
        );
        assert!(game.velocity.x > 0.0);
        assert!(game.velocity.y < 0.0);
        assert_eq!(game.facing, 1.0);
    }

    #[test]
    fn struck_enemies_recoil_and_lose_their_attack_but_bosses_hold_ground() {
        let mut game = ranged_game(1.0);
        let start = game.world.actor_pos(game.enemies[0].actor).x;
        game.enemies[0].cooldown = 0.1;
        game.enemies[0].stagger();
        assert!(game.enemies[0].cooldown >= 0.7);
        simulate(&mut game, 6, Input::default());
        assert!(game.world.actor_pos(game.enemies[0].actor).x > start + 5.0);

        let mut game = ranged_game(1.0);
        game.enemies[0].boss = true;
        game.enemies[0].cooldown = 0.1;
        let start = game.world.actor_pos(game.enemies[0].actor).x;
        game.enemies[0].stagger();
        assert!(game.enemies[0].cooldown <= 0.1);
        simulate(&mut game, 6, Input::default());
        // Le boss continue de charger vers le joueur au lieu de reculer.
        assert!(game.world.actor_pos(game.enemies[0].actor).x <= start);
    }

    #[test]
    fn talents_are_bought_in_order_and_change_stats() {
        use crate::talents::{Talent, TalentState};
        let mut game = Game::new(Save::default());
        game.save.shards = 30;
        assert_eq!(game.talent_state(Talent::Edge), TalentState::Locked);
        assert_eq!(game.buy_talent(Talent::Edge), Err("PREREQUIS MANQUANT"));
        game.buy_talent(Talent::Vigor).unwrap();
        assert_eq!((game.max_health(), game.save.shards), (6, 20));
        assert_eq!(game.buy_talent(Talent::Vigor), Err("DEJA ACQUIS"));
        game.buy_talent(Talent::Reach).unwrap();
        assert_eq!(game.damage_bonus(false), 0);
        assert_eq!(game.buy_talent(Talent::Edge), Err("ECLATS INSUFFISANTS"));
        game.save.shards = 16;
        game.buy_talent(Talent::Edge).unwrap();
        assert_eq!(game.damage_bonus(true), 1);
        assert_eq!(game.save.shards, 0);
        assert!(game.save_dirty);
        // Les talents survivent a la serialisation, a la restauration et a la mort.
        let restored: Save =
            serde_json::from_str(&serde_json::to_string(&game.save).unwrap()).unwrap();
        assert_eq!(Game::new(restored.sanitize()).max_health(), 6);
    }

    #[test]
    fn invalid_talents_are_dropped_when_a_save_is_sanitized() {
        use crate::talents::Talent;
        let broken = Save {
            talents: vec![Talent::Magnet, Talent::Edge, Talent::Magnet],
            shards: 1_000_000,
            ..Save::default()
        }
        .sanitize();
        assert!(broken.talents.is_empty());
        assert_eq!(broken.shards, crate::talents::MAX_SHARDS);
        let valid = Save {
            talents: vec![Talent::Magnet, Talent::Vigor, Talent::Vigor],
            ..Save::default()
        }
        .sanitize();
        assert_eq!(valid.talents, vec![Talent::Vigor, Talent::Magnet]);
        assert!(serde_json::from_str::<Save>(r#"{"talents":["unknown"]}"#).is_err());
        assert!(serde_json::from_str::<Save>(r#"{"checkpoint":0}"#)
            .unwrap()
            .talents
            .is_empty());
    }

    #[test]
    fn defeating_enemies_earns_shards_with_the_magnet_bonus() {
        for (magnet, expected) in [(false, 2), (true, 3)] {
            let mut game = Game::new(Save::default());
            if magnet {
                game.save.talents = vec![
                    crate::talents::Talent::Vigor,
                    crate::talents::Talent::Magnet,
                ];
            }
            game.enter(1, vec2(144.0, 446.0));
            game.enemies[0].health = 1;
            simulate(
                &mut game,
                20,
                Input {
                    attack: true,
                    ..Input::default()
                },
            );
            assert_eq!(game.save.shards, expected);
            assert_eq!(game.save.defeated_enemies, 1);
        }
    }

    fn flat_game() -> Game {
        let mut game = flat_game_with_platform();
        game.levels.rooms[0].platforms.clear();
        game.enter(0, vec2(100.0, 446.0));
        game
    }

    #[test]
    fn debug_can_play_any_character_without_unlocking_it() {
        use crate::characters::Character;
        let mut game = Game::new(Save::default());
        assert_eq!(game.select_character(Character::Goddess), Err("VERROUILLE"));
        game.debug_select_character(Character::Soldier);
        assert_eq!(game.save.character, Character::Soldier);
        assert!(!game.character_unlocked(Character::Soldier));
        assert_eq!((game.max_health(), game.health), (7, 7));
        game.debug_select_character(Character::Ninja);
        assert_eq!((game.max_health(), game.health), (4, 4));
        // Une sauvegarde enregistree depuis le debug ne garde pas un heros reste verrouille.
        assert_eq!(game.save.clone().sanitize().character, Character::Lumen);
    }

    #[test]
    fn characters_unlock_with_progress_and_change_stats() {
        use crate::characters::Character;
        let mut game = Game::new(Save::default());
        assert_eq!(game.select_character(Character::Ninja), Err("VERROUILLE"));
        assert_eq!(
            game.select_character(Character::Lumen),
            Err("DEJA SELECTIONNE")
        );
        game.save.defeated_enemies = 30;
        game.select_character(Character::Ninja).unwrap();
        // Le ninja est fragile : un coeur de moins, et la vie courante suit sans jamais tuer.
        assert_eq!((game.max_health(), game.health), (4, 4));
        assert!(game.save_dirty);
        game.save.defeated_bosses = vec![1, 2, 3];
        game.select_character(Character::Samurai).unwrap();
        assert_eq!((game.damage_bonus(false), game.damage_bonus(true)), (1, 0));
        game.all_weapons = true;
        game.select_character(Character::Soldier).unwrap();
        assert_eq!(game.max_health(), 7);
        assert_eq!((game.damage_bonus(false), game.damage_bonus(true)), (0, 1));
        // Une sauvegarde qui reference un personnage verrouille revient a Lumen.
        let cheating = Save {
            character: Character::Goddess,
            ..Save::default()
        }
        .sanitize();
        assert_eq!(cheating.character, Character::Lumen);
        let kept = Save {
            character: Character::Ninja,
            defeated_enemies: 40,
            ..Save::default()
        }
        .sanitize();
        assert_eq!(kept.character, Character::Ninja);
        assert_eq!(
            serde_json::from_str::<Save>(r#"{"checkpoint":0}"#)
                .unwrap()
                .character,
            Character::Lumen
        );
    }

    #[test]
    fn character_stats_change_speed_reach_and_falling() {
        use crate::characters::Character;
        let speed_after_walking = |character: Character| {
            let mut game = flat_game();
            game.save.character = character;
            simulate(
                &mut game,
                60,
                Input {
                    direction: 1.0,
                    ..Input::default()
                },
            );
            game.velocity.x
        };
        let lumen = speed_after_walking(Character::Lumen);
        assert!((lumen - 235.0).abs() < 1.0);
        assert!(speed_after_walking(Character::Ninja) > lumen * 1.1);
        assert!(speed_after_walking(Character::Samurai) < lumen);
        let reach = |character: Character| {
            let mut game = flat_game();
            game.save.character = character;
            game.attack_time = 0.1;
            game.sword().unwrap().w
        };
        assert!((reach(Character::Samurai) / reach(Character::Lumen) - 1.2).abs() < 0.01);
        let fall_speed = |character: Character| {
            let mut game = flat_game();
            game.save.character = character;
            game.world.set_actor_position(game.actor, vec2(100.0, 20.0));
            simulate(&mut game, 60, Input::default());
            game.velocity.y
        };
        assert!(fall_speed(Character::Goddess) < fall_speed(Character::Lumen));
    }

    #[test]
    fn new_characters_are_announced_when_progress_unlocks_them() {
        let mut game = Game::new(Save::default());
        game.save.defeated_enemies = 29;
        game.enter(1, vec2(144.0, 446.0));
        game.enemies[0].health = 1;
        simulate(
            &mut game,
            20,
            Input {
                attack: true,
                ..Input::default()
            },
        );
        assert_eq!(game.save.defeated_enemies, 30);
        assert!(game.notice.contains("NINJA"), "{}", game.notice);
    }

    #[test]
    fn human_enemy_skins_change_toughness_but_never_below_one_health() {
        use crate::characters::Skin;
        let game = Game::new(Save::default());
        let health = |skin: Skin, species: u8| {
            Enemy {
                actor: game.actor,
                home: Vec2::ZERO,
                health: 0,
                direction: 1.0,
                hurt: 0.0,
                cooldown: 0.0,
                boss: false,
                behavior: crate::levels::Behavior::Charge,
                species,
                skin,
            }
            .max_health()
        };
        assert_eq!(health(Skin::Creature, 0), 2);
        assert_eq!(health(Skin::Ninja, 0), 1);
        assert_eq!(health(Skin::Samurai, 0), 3);
        assert_eq!(health(Skin::Goddess, 0), 4);
        assert!(Skin::ALL.iter().all(|skin| health(*skin, 0) >= 1));
    }

    fn flat_game_with_platform() -> Game {
        let mut levels = crate::levels::Levels::original();
        let room = &mut levels.rooms[0];
        room.tiles = (0..ROWS)
            .map(|row| if row >= 15 { "#" } else { "." }.repeat(COLS))
            .collect();
        room.enemies.clear();
        room.hazards.clear();
        room.platforms.push(crate::levels::MovingPlatform {
            from: [200.0, 400.0],
            to: [600.0, 400.0],
            tiles: 4,
            period: 4.0,
        });
        let mut game = Game::with_levels(Save::default(), levels);
        game.enter(0, vec2(210.0, 340.0));
        game
    }

    #[test]
    fn moving_platform_follows_its_path_and_carries_the_player() {
        let mut game = flat_game_with_platform();
        assert_eq!(game.platforms.len(), 1);
        simulate(&mut game, 240, Input::default());
        // Deux secondes plus tard, la plateforme a parcouru la moitie du trajet et porte le joueur.
        let platform = game.world.solid_pos(game.platforms[0].solid);
        assert!(platform.x > 350.0 && platform.x < 450.0, "{platform}");
        assert!(game.grounded);
        let player = game.position();
        assert!(player.x > platform.x && player.x < platform.x + 64.0);
        assert!(player.y + PLAYER_SIZE.y <= platform.y + 1.0);
        let bounds = (
            platform_position(vec2(0.0, 5.0), vec2(100.0, 5.0), 4.0, 0.0),
            platform_position(vec2(0.0, 5.0), vec2(100.0, 5.0), 4.0, 4.0),
        );
        assert_eq!(bounds, (vec2(0.0, 5.0), vec2(100.0, 5.0)));
        // Le retour est periodique : apres un aller-retour complet, retour au depart.
        assert_eq!(
            platform_position(vec2(0.0, 0.0), vec2(100.0, 0.0), 4.0, 8.0),
            vec2(0.0, 0.0)
        );
    }

    #[test]
    fn pulsing_spikes_only_hurt_while_armed() {
        assert_eq!(hazard_phase(0.0), HazardPhase::Armed);
        assert_eq!(hazard_phase(1.7), HazardPhase::Retracted);
        assert_eq!(hazard_phase(2.8), HazardPhase::Warning);
        assert_eq!(hazard_phase(3.0), HazardPhase::Armed);
        let mut game = Game::new(Save::default());
        game.enter(1, vec2(144.0, 446.0));
        game.levels.rooms[1].hazards = vec![[0.0, 440.0, 960.0, 40.0]];
        game.levels.rooms[1].pulsing = true;
        game.time = 1.7;
        let health = game.health;
        game.update(Input::default(), STEP);
        assert_eq!(game.health, health);
        game.time = 3.1;
        game.update(Input::default(), STEP);
        assert_eq!(game.health, health - 1);
    }

    #[test]
    fn difficulty_changes_hearts_invincibility_and_attack_pace_only() {
        let mut game = Game::new(Save::default());
        game.enter(1, vec2(144.0, 446.0));
        assert_eq!(game.max_health(), 5);
        game.set_difficulty(Difficulty::Calm);
        assert_eq!((game.max_health(), game.health), (7, 7));
        game.damage(1.0);
        assert_eq!(game.invincible, 2.0);
        game.set_difficulty(Difficulty::Hard);
        assert_eq!((game.max_health(), game.health), (5, 4));
        game.health = 1;
        game.set_difficulty(Difficulty::Calm);
        game.set_difficulty(Difficulty::Hard);
        assert_eq!(game.health, 1);
        assert!(Difficulty::Hard.attack_pace() < Difficulty::Normal.attack_pace());
        assert!(Difficulty::Calm.attack_pace() > Difficulty::Normal.attack_pace());
    }

    #[test]
    fn deaths_are_counted_per_room_saved_and_bounded() {
        let mut game = Game::new(Save::default());
        game.enter(1, vec2(144.0, 446.0));
        game.health = 1;
        game.damage(1.0);
        assert_eq!(game.save.deaths, 1);
        assert_eq!(game.save.death_rooms[1], 1);
        assert!(game.save_dirty);
        let json = serde_json::to_string(&game.save).unwrap();
        let restored: Save = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.deaths, 1);
        let legacy: Save = serde_json::from_str(r#"{"checkpoint":0}"#).unwrap();
        assert_eq!(legacy.deaths, 0);
        let mut crowded = Save::default();
        crowded.record_death(usize::MAX);
        assert!(crowded.death_rooms.is_empty());
    }

    #[test]
    fn enraged_ring_is_slow_even_and_rotates() {
        let ring = super::enraged_ring(0, 0.0);
        assert_eq!(ring.len(), 6);
        assert!(ring.iter().all(|shot| (shot.length() - 78.0).abs() < 0.01));
        assert_ne!(ring[0], super::enraged_ring(0, 0.5)[0]);
    }

    #[test]
    fn enemy_toughness_never_drops_from_one_biome_to_the_next() {
        let game = Game::new(Save::default());
        let enemy = |species: u8, boss: bool| Enemy {
            actor: game.actor,
            home: Vec2::ZERO,
            health: 0,
            direction: 1.0,
            hurt: 0.0,
            cooldown: 0.0,
            boss,
            behavior: crate::levels::Behavior::Charge,
            species,
            skin: crate::characters::Skin::default(),
        };
        for boss in [false, true] {
            let tiers: Vec<_> = (0..10u8).map(|tier| enemy(tier * 3, boss)).collect();
            assert!(tiers
                .windows(2)
                .all(|pair| pair[0].max_health() <= pair[1].max_health()));
            assert!(tiers[9].max_health() > tiers[0].max_health());
        }
    }

    #[test]
    fn enraged_boss_attacks_faster_only_below_half_health() {
        assert_eq!(super::enrage_scales(true, 9, 18), (0.7, 1.2));
        assert_eq!(super::enrage_scales(true, 10, 18), (1.0, 1.0));
        assert_eq!(super::enrage_scales(false, 1, 18), (1.0, 1.0));
    }

    #[test]
    fn debug_equipment_replaces_queued_switch() {
        let mut game = super::Game::new(super::Save::default());
        game.attack_time = 0.5;
        game.queued_weapon = Some(1);
        game.debug_equip(2);
        game.update(super::Input::default(), super::STEP);
        assert_eq!(game.weapon_index, 2);
        assert!(game.queued_weapon.is_none());
    }

    #[test]
    fn debug_jumps_and_cooldowns_bypass_only_enabled_limits() {
        let mut game = super::Game::new(super::Save::default());
        game.enter(0, macroquad::prelude::vec2(96.0, 80.0));
        game.jumps = 2;
        game.debug.infinite_jumps = true;
        game.debug.no_cooldowns = true;
        game.attack_cooldown = 10.0;
        game.update(
            super::Input {
                jump: true,
                attack: true,
                ..Default::default()
            },
            super::STEP,
        );
        assert!(game.velocity.y < 0.0);
        assert!(game.attack_time > 0.0);
    }

    #[test]
    fn debug_aids_are_temporary_and_damage_is_blocked() {
        let mut game = super::Game::new(super::Save::default());
        let save = game.save.clone();
        game.debug.invulnerable = true;
        game.debug.health_bonus = 100;
        game.health = game.max_health();
        game.damage(1.0);
        assert_eq!(game.health, 9);
        assert_eq!(game.save, save);
        let restored = super::Game::new(game.save.clone());
        assert_eq!(restored.max_health(), 5);
        assert!(!restored.debug.invulnerable);
    }

    #[test]
    fn debug_travel_checks_bounds_and_unlocks_catalog() {
        let mut game = super::Game::new(super::Save::default());
        game.debug_enter(usize::MAX);
        assert_eq!(game.room, 0);
        game.debug_enter(2);
        assert_eq!(game.room, 2);
        assert!(game.save.visited[2]);
        game.debug_unlock_weapons();
        assert!((0..game.weapons.len()).all(|index| game.weapon_available(index)));
        game.health = 1;
        game.debug_respawn();
        assert_eq!(game.room, game.save.checkpoint);
        assert_eq!(game.health, game.max_health());
    }

    #[test]
    fn every_original_room_accepts_a_safe_bidirectional_extension() {
        for source in 0..NAMES.len() {
            let mut levels = crate::levels::Levels::original();
            let target = levels.generate(source, 42, 1).unwrap();
            let mut game = Game::with_levels(Save::default(), levels);
            game.enter(source, vec2(320.0, 446.0));
            assert!(!game.world.collide_check(game.actor, game.position()));
            assert!(!game
                .hazards()
                .iter()
                .any(|hazard| hazard.overlaps(&game.bounds())));
            assert!(game.use_generated_portal());
            assert_eq!(game.room, target);
            assert!(game.use_generated_portal());
            assert_eq!(game.room, source);
        }
    }

    #[test]
    fn generated_portals_work_both_ways_without_abilities_and_restore_checkpoints() {
        let mut levels = crate::levels::Levels::original();
        let extension = levels.generate(0, 42, 1).unwrap();
        let mut game = Game::with_levels(Save::default(), levels.clone());
        game.enter(0, vec2(320.0, 446.0));
        game.update(
            Input {
                interact: true,
                ..Input::default()
            },
            STEP,
        );
        assert_eq!(game.room, extension);
        assert!(!game.save.double_jump);
        assert!(game.save.visited[extension]);
        assert!(!game.world.collide_check(game.actor, game.position()));
        game.update(
            Input {
                interact: true,
                ..Input::default()
            },
            STEP,
        );
        assert_eq!(game.room, 0);
        game.enter(extension, vec2(192.0, 446.0));
        game.update(
            Input {
                interact: true,
                ..Input::default()
            },
            STEP,
        );
        assert_eq!(game.save.checkpoint, extension);
        let reloaded = Game::with_levels(game.save.clone(), levels);
        assert_eq!(reloaded.room, extension);
        let removed = Game::new(game.save);
        assert_eq!(removed.room, 0);
    }

    use super::*;

    #[test]
    fn ten_bosses_unlock_only_their_portal_and_stay_defeated_after_reload() {
        let mut levels = crate::levels::Levels::original();
        let start = levels.add_campaign().unwrap();
        let mut game = Game::with_levels(Save::default(), levels.clone());
        for biome in 0..10 {
            let room = start + biome * 50 + 49;
            game.enter(room, vec2(856.0, 446.0));
            assert_eq!(game.enemies.len(), 1);
            let target = if biome == 9 { 0 } else { room + 1 };
            assert!(!game
                .portals()
                .iter()
                .any(|(_, destination, _)| *destination == target));
            assert!(!game.use_generated_portal());
            let center = game.world.actor_pos(game.enemies[0].actor) + game.enemies[0].size() * 0.5;
            game.projectiles.push(Projectile {
                position: center,
                velocity: Vec2::ZERO,
                remaining: 1.0,
                radius: 3.0,
                damage: 100,
                laser: false,
                color: WHITE,
            });
            game.update(Input::default(), STEP);
            assert!(game.save.defeated_bosses.contains(&room));
            assert_eq!(game.save.won, biome == 9);
            assert!(game.use_generated_portal());
            assert_eq!(game.room, target);
            let save: Save =
                serde_json::from_slice(&serde_json::to_vec(&game.save).unwrap()).unwrap();
            let mut restored = Game::with_levels(save, levels.clone());
            restored.enter(room, vec2(856.0, 446.0));
            assert!(restored.enemies.is_empty());
            assert!(restored
                .portals()
                .iter()
                .any(|(_, destination, _)| *destination == target));
        }
        let mut old_victory = Game::with_levels(
            Save {
                won: true,
                ..Save::default()
            },
            levels,
        );
        old_victory.enter(start + 49, vec2(96.0, 446.0));
        assert_eq!(old_victory.enemies.len(), 1);
    }

    #[test]
    fn all_attack_profiles_are_distinct_and_hostile_shots_are_cleared() {
        let signatures: Vec<_> = crate::levels::Behavior::ALL[1..]
            .iter()
            .map(|behavior| attack_vectors(*behavior, vec2(1.0, 0.3), true, 0))
            .collect();
        for (index, vectors) in signatures.iter().enumerate() {
            assert!(!vectors.is_empty() && vectors.len() <= 5);
            assert!(vectors.iter().all(|vector| vector.is_finite()));
            assert!(!signatures[..index].contains(vectors));
        }
        let mut levels = crate::levels::Levels::original();
        let start = levels.add_campaign().unwrap();
        let mut game = Game::with_levels(Save::default(), levels);
        for biome in 0..10 {
            game.enter(start + biome * 50 + 49, vec2(96.0, 446.0));
            game.invincible = 10.0;
            for _ in 0..205 {
                game.update(Input::default(), STEP);
            }
            assert!(!game.hostile_shots.is_empty(), "profil {biome}");
            assert!(game.hostile_shots.len() <= 128);
            game.enter(0, vec2(96.0, 446.0));
            assert!(game.hostile_shots.is_empty());
        }
    }

    #[test]
    fn extra_visual_effects_are_bounded_expire_and_clear() {
        let mut game = Game::new(Save::default());
        for _ in 0..40 {
            game.burst(vec2(300.0, 300.0), WHITE, 500);
        }
        assert_eq!(game.particles.len(), 256);
        assert_eq!(game.ripples.len(), 32);
        assert_eq!(game.impacts.len(), 8);
        game.damage(-1.0);
        assert_eq!(game.combat_text.len(), 1);
        simulate(&mut game, 240, Input::default());
        assert!(game.impacts.is_empty());
        assert!(game.combat_text.is_empty());
        game.save.double_jump = true;
        game.update(
            Input {
                dash: true,
                ..Input::default()
            },
            STEP,
        );
        assert!(!game.afterimages.is_empty());
        assert!(game.afterimages.len() <= 24);
        game.enter(0, vec2(96.0, 446.0));
        assert!(game.afterimages.is_empty());
        assert!(game.impacts.is_empty());
        assert!(game.combat_text.is_empty());
    }

    #[test]
    fn visual_emitters_do_not_change_physics_or_progress() {
        let save = Save {
            double_jump: true,
            ..Save::default()
        };
        let mut reference = Game::new(save.clone());
        let mut decorated = Game::new(save);
        for frame in 0..180 {
            let input = Input {
                direction: 1.0,
                jump: frame == 30,
                dash: frame == 90,
                attack: frame == 120,
                ..Input::default()
            };
            if frame % 7 == 0 {
                decorated.burst(vec2(300.0, 200.0), WHITE, 40);
            }
            reference.update(input, STEP);
            decorated.update(input, STEP);
            assert_eq!(reference.position(), decorated.position());
            assert_eq!(reference.velocity, decorated.velocity);
            assert_eq!(reference.health, decorated.health);
            assert_eq!(reference.save, decorated.save);
        }
    }

    fn simulate(game: &mut Game, frames: usize, input: Input) {
        for frame in 0..frames {
            game.update(
                Input {
                    jump: input.jump && frame == 0,
                    attack: input.attack && frame == 0,
                    dash: input.dash && frame == 0,
                    ..input
                },
                STEP,
            );
        }
    }

    #[test]
    fn sound_events_follow_successful_actions_and_damage() {
        let mut game = Game::new(Save::default());
        simulate(&mut game, 120, Input::default());
        game.sounds.clear();
        game.update(
            Input {
                jump: true,
                attack: true,
                ..Input::default()
            },
            STEP,
        );
        assert!(game.sounds.contains(&SoundCue::Jump));
        assert!(game.sounds.contains(&SoundCue::Attack));
        game.sounds.clear();
        game.update(
            Input {
                jump: true,
                attack: true,
                dash: true,
                ..Input::default()
            },
            STEP,
        );
        assert!(game.sounds.is_empty());
        game.damage(-1.0);
        assert_eq!(game.sounds, vec![SoundCue::Hurt]);
        game.sounds.clear();
        game.damage(-1.0);
        assert!(game.sounds.is_empty());
        game.invincible = 0.0;
        game.health = 1;
        game.damage(-1.0);
        assert_eq!(game.sounds, vec![SoundCue::Death]);
    }

    #[test]
    fn visual_effects_expire_and_clear_on_room_change() {
        let mut game = Game::new(Save::default());
        simulate(&mut game, 120, Input::default());
        game.burst(vec2(100.0, 420.0), WHITE, 12);
        assert_eq!(game.ripples.len(), 1);
        simulate(&mut game, 120, Input::default());
        assert!(game.ripples.is_empty());
        assert!(game.particles.is_empty());
        game.damage(-1.0);
        assert!(game.damage_flash > 0.0);
        game.enter(2, vec2(80.0, 446.0));
        assert!(game.ripples.is_empty());
        assert_eq!(game.damage_flash, 0.0);
    }

    #[test]
    fn player_lands_and_can_jump() {
        let mut game = Game::new(Save::default());
        simulate(&mut game, 120, Input::default());
        assert_eq!(game.position().y, 446.0);
        simulate(
            &mut game,
            20,
            Input {
                jump: true,
                ..Input::default()
            },
        );
        assert!(game.position().y < 400.0);
    }

    #[test]
    fn ability_unlocks_second_jump_but_not_third() {
        let mut game = Game::new(Save::default());
        simulate(&mut game, 1, Input::default());
        simulate(
            &mut game,
            20,
            Input {
                jump: true,
                ..Input::default()
            },
        );
        game.update(
            Input {
                jump: true,
                ..Input::default()
            },
            STEP,
        );
        assert_eq!(game.jumps, 1);
        game.save.double_jump = true;
        game.update(
            Input {
                jump: true,
                ..Input::default()
            },
            STEP,
        );
        assert_eq!(game.jumps, 2);
        simulate(&mut game, 20, Input::default());
        let velocity = game.velocity.y;
        game.update(
            Input {
                jump: true,
                ..Input::default()
            },
            STEP,
        );
        assert!(game.velocity.y > velocity);
    }

    #[test]
    fn lethal_damage_respawns_and_keeps_ability() {
        let mut game = Game::new(Save {
            double_jump: true,
            checkpoint: 2,
            ..Save::default()
        });
        game.enter(1, vec2(80.0, 446.0));
        game.health = 1;
        game.damage(-1.0);
        assert_eq!(game.room, 2);
        assert_eq!(game.health, 5);
        assert!(game.save.double_jump);
    }

    #[test]
    fn weapon_unlocks_are_earned_saved_and_preserved_after_death() {
        let mut game = Game::new(Save {
            defeated_enemies: 1,
            ..Save::default()
        });
        game.enter(1, vec2(170.0, 446.0));
        game.invincible = 2.0;
        assert!(!game.weapon_available(7));
        game.equip(7);
        assert_eq!(game.weapon_index, 0);
        game.enemies[0].health = 1;
        simulate(
            &mut game,
            20,
            Input {
                attack: true,
                ..Input::default()
            },
        );
        assert_eq!(game.save.defeated_enemies, 2);
        assert!(game.weapon_available(7));
        assert!(game.notice.contains("Pistolet leger"));
        simulate(&mut game, 15, Input::default());
        game.equip(7);
        let json = serde_json::to_string(&game.save).unwrap();
        let mut reloaded = Game::new(serde_json::from_str(&json).unwrap());
        reloaded.set_weapons(crate::content::default_weapons());
        assert_eq!(reloaded.weapon_index, 7);
        reloaded.health = 1;
        reloaded.invincible = 0.0;
        reloaded.damage(1.0);
        assert_eq!(reloaded.save.defeated_enemies, 2);
        assert_eq!(reloaded.weapon_index, 7);
        let mut reordered = reloaded.weapons.clone();
        reordered.swap(0, 7);
        reloaded.set_weapons(reordered);
        assert_eq!(reloaded.weapon_index, 0);
        assert_eq!(reloaded.weapon().name, "Pistolet leger");
    }

    #[test]
    fn weapon_cycle_queues_during_attack_and_skips_locked_weapons() {
        let mut game = Game::new(Save::default());
        game.equip(6);
        game.attack_time = 0.1;
        game.attack_cooldown = 0.4;
        game.update(
            Input {
                next_weapon: true,
                ..Input::default()
            },
            STEP,
        );
        assert_eq!(game.weapon_index, 6);
        simulate(&mut game, 15, Input::default());
        assert_eq!(game.weapon_index, 0);
        assert!(game.attack_cooldown > 0.0);
        assert_eq!(game.save.equipped_weapon.as_deref(), Some("Epee"));
    }

    #[test]
    fn weapon_automatic_fire_repeats_only_while_held() {
        for index in [7, 11, 12, 17, 18] {
            let mut game = Game::new(Save {
                defeated_enemies: 64,
                ..Save::default()
            });
            game.equip(index);
            let mut shots = 0;
            for frame in 0..120 {
                game.update(
                    Input {
                        attack: frame == 0,
                        attack_held: true,
                        ..Input::default()
                    },
                    STEP,
                );
                shots += game
                    .sounds
                    .iter()
                    .filter(|cue| **cue == SoundCue::Attack)
                    .count();
                game.sounds.clear();
            }
            if index == 7 {
                assert_eq!(shots, 1);
            } else {
                assert!(shots >= 6);
            }
            for _ in 0..120 {
                game.update(Input::default(), STEP);
                assert!(!game.sounds.contains(&SoundCue::Attack));
                game.sounds.clear();
            }
        }
    }

    #[test]
    fn laser_weapons_unlock_and_keep_their_projectile_type_and_damage() {
        for index in 13..19 {
            let mut game = ranged_game(1.0);
            let weapon = game.weapons[index].clone();
            assert!(weapon.laser);
            game.save.defeated_enemies = weapon.unlock_kills - 1;
            assert!(!game.weapon_available(index));
            game.save.defeated_enemies += 1;
            game.equip(index);
            assert_eq!(game.weapon_index, index);
            game.update(
                Input {
                    attack: true,
                    ..Input::default()
                },
                STEP,
            );
            assert!(game.sword().is_none());
            assert_eq!(game.projectiles.len(), 1);
            assert!(game.projectiles[0].laser);
            game.attack_time = 0.0;
            game.equip(0);
            assert!(game.projectiles[0].laser);
            let hits = game.advance_projectiles(1.0);
            assert_eq!(hits.len(), 1);
            assert_eq!(hits[0].1, weapon.damage);
            assert_eq!(game.enemies[0].health, 10 - weapon.damage);
        }
    }

    #[test]
    fn sword_hits_each_enemy_once_per_swing() {
        let mut game = Game::new(Save::default());
        game.enter(1, vec2(144.0, 446.0));
        simulate(
            &mut game,
            20,
            Input {
                attack: true,
                ..Input::default()
            },
        );
        assert_eq!(game.enemies[0].health, 1);
    }

    fn ranged_game(facing: f32) -> Game {
        let mut game = Game::new(Save::default());
        game.enter(0, vec2(if facing > 0.0 { 96.0 } else { 400.0 }, 446.0));
        game.facing = facing;
        game.equip(5);
        let home = vec2(300.0, 446.0);
        let actor = game.world.add_actor(home, 26, 28);
        game.enemies.push(Enemy {
            actor,
            home,
            health: 10,
            direction: 0.0,
            hurt: 0.0,
            cooldown: 1.0,
            boss: false,
            behavior: crate::levels::Behavior::Legacy,
            species: 0,
            skin: crate::characters::Skin::default(),
        });
        game
    }

    #[test]
    fn ranged_projectiles_hit_once_in_both_directions_and_keep_their_damage() {
        for facing in [-1.0, 1.0] {
            let mut game = ranged_game(facing);
            game.update(
                Input {
                    attack: true,
                    ..Input::default()
                },
                STEP,
            );
            assert!(game.sword().is_none());
            assert_eq!(game.projectiles.len(), 1);
            game.update(
                Input {
                    attack: true,
                    ..Input::default()
                },
                STEP,
            );
            assert_eq!(game.projectiles.len(), 1);
            game.attack_time = 0.0;
            game.equip(0);
            let impacts = game.advance_projectiles(1.0);
            assert_eq!(impacts.len(), 1);
            assert_eq!(impacts[0].1, 3);
            assert_eq!(game.enemies[0].health, 7);
            assert!(game.projectiles.is_empty());
            assert!(game.advance_projectiles(1.0).is_empty());
        }
    }

    #[test]
    fn ranged_projectiles_stop_at_walls_expire_and_clear_on_room_change() {
        let mut game = ranged_game(1.0);
        let mut wall = vec![Tile::Empty; COLS * ROWS];
        wall[14 * COLS + 6] = Tile::Solid;
        game.world.add_static_tiled_layer(wall, TILE, TILE, COLS, 1);
        game.update(
            Input {
                attack: true,
                ..Input::default()
            },
            STEP,
        );
        assert!(game.advance_projectiles(1.0).is_empty());
        assert_eq!(game.enemies[0].health, 10);
        assert!(game.projectiles.is_empty());

        let mut game = ranged_game(1.0);
        game.weapons[5].projectile.as_mut().unwrap().range = 64.0;
        game.update(
            Input {
                attack: true,
                ..Input::default()
            },
            STEP,
        );
        assert!(game.advance_projectiles(1.0).is_empty());
        assert_eq!(game.enemies[0].health, 10);
        assert!(game.projectiles.is_empty());
        game.attack_cooldown = 0.0;
        game.update(
            Input {
                attack: true,
                ..Input::default()
            },
            STEP,
        );
        assert_eq!(game.projectiles.len(), 1);
        game.enter(1, vec2(96.0, 446.0));
        assert!(game.projectiles.is_empty());
    }

    #[test]
    fn ranged_projectiles_defeat_guardian_and_report_actual_damage() {
        let mut game = Game::new(Save::default());
        game.enter(4, vec2(480.0, 446.0));
        game.invincible = 2.0;
        game.equip(5);
        game.enemies[0].health = 3;
        simulate(
            &mut game,
            100,
            Input {
                attack: true,
                ..Input::default()
            },
        );
        assert!(game.save.won);
        assert!(game.save_dirty);
        assert!(game.combat_text.iter().any(|text| text.amount == 3));
    }

    #[test]
    fn weapons_control_damage_cadence_and_only_hit_once() {
        for index in 0..4 {
            let mut game = Game::new(Save::default());
            game.enter(1, vec2(170.0, 446.0));
            game.invincible = 2.0;
            game.equip(index);
            let weapon = game.weapon().clone();
            game.enemies[0].health = 20;
            game.update(
                Input {
                    attack: true,
                    ..Input::default()
                },
                STEP,
            );
            assert_eq!(game.attack_time, weapon.duration);
            assert_eq!(game.attack_cooldown, weapon.cooldown);
            game.equip(index + 1);
            assert_eq!(game.weapon_index, index);
            simulate(
                &mut game,
                (weapon.duration / STEP).ceil() as usize,
                Input::default(),
            );
            assert_eq!(game.enemies[0].health, 20 - weapon.damage);
            assert_eq!(game.combat_text[0].amount, weapon.damage);
            let cooldown = game.attack_cooldown;
            game.equip(index + 1);
            assert_eq!(game.attack_cooldown, cooldown);
        }
    }

    #[test]
    fn weapon_reach_is_mirrored_and_selection_wraps() {
        let mut game = Game::new(Save::default());
        game.all_weapons = true;
        game.equip(2);
        game.attack_time = game.weapon().duration;
        let right = game.sword().unwrap();
        game.facing = -1.0;
        let left = game.sword().unwrap();
        assert_eq!(right.w, 110.0);
        assert_eq!(left.w, right.w);
        assert!((left.x + left.w - (game.position().x + 8.0)).abs() < 0.01);
        game.attack_time = 0.0;
        game.equip(game.weapons.len() - 1);
        game.update(
            Input {
                next_weapon: true,
                ..Input::default()
            },
            STEP,
        );
        assert_eq!(game.weapon_index, 0);
    }

    #[test]
    fn forge_pickup_and_shortcut_work() {
        let mut game = Game::new(Save::default());
        game.enter(2, vec2(688.0, 222.0));
        game.update(Input::default(), STEP);
        assert!(game.save.double_jump);
        game.world
            .set_actor_position(game.actor, vec2(824.0, 446.0));
        game.update(
            Input {
                interact: true,
                ..Input::default()
            },
            STEP,
        );
        assert_eq!(game.room, 0);
    }

    #[test]
    fn tower_requires_double_jump_and_is_reachable_with_it() {
        for double_jump in [false, true] {
            let mut game = Game::new(Save {
                double_jump,
                ..Save::default()
            });
            game.enter(0, vec2(400.0, 254.0));
            for frame in 0..240 {
                let direction = if game.position().x > 185.0 { -1.0 } else { 0.0 };
                game.update(
                    Input {
                        direction,
                        jump: frame == 0 || (double_jump && frame == 30),
                        interact: true,
                        ..Input::default()
                    },
                    STEP,
                );
                if game.room == 3 {
                    break;
                }
            }
            assert_eq!(game.room == 3, double_jump);
        }
    }

    #[test]
    fn defeating_guardian_unlocks_the_ending() {
        let mut game = Game::new(Save::default());
        game.enter(4, vec2(560.0, 446.0));
        game.enemies[0].health = 1;
        simulate(
            &mut game,
            20,
            Input {
                attack: true,
                ..Input::default()
            },
        );
        assert!(game.save.won);
        assert!(game.save_dirty);
        game.world
            .set_actor_position(game.actor, vec2(824.0, 446.0));
        game.update(
            Input {
                interact: true,
                ..Input::default()
            },
            STEP,
        );
        assert!(game.finished);
    }

    #[test]
    fn save_round_trip_and_invalid_checkpoint() {
        let save = Save {
            checkpoint: 2,
            double_jump: true,
            won: true,
            visited: vec![true; NAMES.len()],
            life_shard: true,
            ..Save::default()
        };
        let json = serde_json::to_string(&save).unwrap();
        assert_eq!(serde_json::from_str::<Save>(&json).unwrap(), save);
        assert_eq!(
            Save {
                checkpoint: 999,
                ..Save::default()
            }
            .sanitize()
            .checkpoint,
            0
        );
    }

    #[test]
    fn legacy_save_keeps_progress_and_extends_map() {
        let save: Save = serde_json::from_str(r#"{"checkpoint":2,"double_jump":true,"won":false,"visited":[true,true,true,false,false]}"#).unwrap();
        let game = Game::new(save);
        assert_eq!(game.room, 2);
        assert!(game.save.double_jump);
        assert_eq!(game.save.visited.len(), NAMES.len());
        assert!(!game.save.visited[6]);
        assert_eq!(game.max_health(), 5);
    }

    #[test]
    fn dash_requires_wing_and_recharges_on_landing() {
        let mut game = Game::new(Save::default());
        game.update(
            Input {
                dash: true,
                ..Input::default()
            },
            STEP,
        );
        assert_eq!(game.dash_time, 0.0);
        game.save.double_jump = true;
        game.enter(0, vec2(480.0, 180.0));
        let start = game.position();
        simulate(
            &mut game,
            12,
            Input {
                dash: true,
                ..Input::default()
            },
        );
        assert!(game.position().x - start.x > 55.0);
        assert_eq!(game.position().y, start.y);
        game.damage(-1.0);
        assert_eq!(game.health, 5);
        simulate(&mut game, 60, Input::default());
        game.update(
            Input {
                dash: true,
                ..Input::default()
            },
            STEP,
        );
        assert_eq!(game.dash_time, 0.0);
        simulate(&mut game, 150, Input::default());
        game.update(
            Input {
                dash: true,
                ..Input::default()
            },
            STEP,
        );
        assert!(game.dash_time > 0.0);
    }

    #[test]
    fn dash_stops_at_solid_wall() {
        let mut game = Game::new(Save {
            double_jump: true,
            ..Save::default()
        });
        game.enter(6, vec2(895.0, 446.0));
        simulate(
            &mut game,
            24,
            Input {
                dash: true,
                ..Input::default()
            },
        );
        assert!(game.position().x <= 906.0);
        assert_eq!(game.dash_time, 0.0);
    }

    #[test]
    fn new_platform_routes_are_reachable_without_teleporting() {
        for room in [2, 6] {
            let mut game = Game::new(Save {
                double_jump: true,
                ..Save::default()
            });
            game.enter(room, vec2(96.0, 446.0));
            for target in [240.0, 448.0, 704.0] {
                for frame in 0..180 {
                    let distance = target - game.position().x;
                    game.update(
                        Input {
                            direction: if distance.abs() > 3.0 {
                                distance.signum()
                            } else {
                                0.0
                            },
                            jump: frame == 0 || frame == 30,
                            ..Input::default()
                        },
                        STEP,
                    );
                }
            }
            assert!(
                game.position().y <= 222.0,
                "room {room}: {:?}",
                game.position()
            );
            if room == 6 {
                assert!(game.save.life_shard);
            } else {
                simulate(
                    &mut game,
                    26,
                    Input {
                        direction: 1.0,
                        ..Input::default()
                    },
                );
                game.update(
                    Input {
                        interact: true,
                        ..Input::default()
                    },
                    STEP,
                );
                assert_eq!(game.room, 5);
            }
        }
    }

    #[test]
    fn optional_branch_returns_and_saves_reward() {
        let mut game = Game::new(Save::default());
        game.enter(2, vec2(760.0, 222.0));
        let interact = Input {
            interact: true,
            ..Input::default()
        };
        game.update(interact, STEP);
        assert_eq!(game.room, 2);
        game.save.double_jump = true;
        game.update(interact, STEP);
        assert_eq!(game.room, 5);
        game.update(interact, STEP);
        assert_eq!(game.room, 2);
        game.enter(5, vec2(940.0, 446.0));
        game.update(Input::default(), STEP);
        assert_eq!(game.room, 6);
        game.world
            .set_actor_position(game.actor, vec2(688.0, 222.0));
        game.update(Input::default(), STEP);
        assert!(game.save.life_shard);
        assert_eq!(game.health, 6);
        game.world
            .set_actor_position(game.actor, vec2(850.0, 446.0));
        game.update(interact, STEP);
        assert_eq!(game.save.checkpoint, 6);
        let restored = Game::new(game.save.clone());
        assert_eq!(restored.room, 6);
        assert_eq!(restored.health, 6);
        game.enter(6, vec2(-1.0, 446.0));
        game.update(Input::default(), STEP);
        assert_eq!(game.room, 5);
    }
}
