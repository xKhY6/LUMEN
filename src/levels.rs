//! Donnees de salle editables ; les indices restent stables pour les sauvegardes du jeu.

use crate::game::{self, COLS, HEIGHT, NAMES, ROWS, WIDTH};
use macroquad_platformer::Tile;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spawn {
    pub position: [f32; 2],
    pub boss: bool,
    #[serde(default)]
    pub species: u8,
    #[serde(default)]
    pub behavior: Behavior,
    /// Apparence : 0 creature du biome, 1 a 4 equivalents humains (voir `characters::Skin`).
    #[serde(default)]
    pub skin: u8,
}

/// Environ quatre ennemis sur neuf adoptent un equivalent humain, repartis entre les quatre apparences.
pub fn campaign_skin(biome: usize, number: usize, species: usize) -> u8 {
    match (biome + number + species) % 9 {
        0 => 1,
        3 => 2,
        5 => 3,
        7 => 4,
        _ => 0,
    }
}

// Chaque profil associe un deplacement et une attaque ; Legacy conserve les anciens packs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Behavior {
    #[default]
    Legacy,
    Charge,
    Leap,
    Orbit,
    Volley,
    Wave,
    Ambush,
    Spiral,
    Burst,
    Sweep,
    Hunt,
}

impl Behavior {
    pub const ALL: [Self; 11] = [
        Self::Legacy,
        Self::Charge,
        Self::Leap,
        Self::Orbit,
        Self::Volley,
        Self::Wave,
        Self::Ambush,
        Self::Spiral,
        Self::Burst,
        Self::Sweep,
        Self::Hunt,
    ];
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Biome {
    pub name: String,
    pub boss: String,
    pub bestiary: [String; 3],
    pub background: [u8; 3],
    pub stone: [u8; 3],
    pub accent: [u8; 3],
    pub flora: u8,
    #[serde(default)]
    pub architecture: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Room {
    #[serde(default)]
    pub biome: Option<usize>,
    pub tiles: Vec<String>,
    pub spawn: [f32; 2],
    pub enemies: Vec<Spawn>,
    pub hazards: Vec<[f32; 4]>,
    pub shrine: Option<[f32; 2]>,
    /// Plateformes horizontales mobiles, facultatives dans les anciens packs.
    #[serde(default)]
    pub platforms: Vec<MovingPlatform>,
    /// Les piques de la salle alternent entre sortis, rentres et avertissement.
    #[serde(default)]
    pub pulsing: bool,
}

pub const PLATFORM_HEIGHT: f32 = 12.0;

/// Plateforme solide en va-et-vient ; seul le mouvement horizontal porte correctement les acteurs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovingPlatform {
    pub from: [f32; 2],
    pub to: [f32; 2],
    pub tiles: u8,
    /// Duree d'un trajet aller, en secondes.
    pub period: f32,
}

/// Gabarits de salle de campagne ; le couloir bas (ligne 14) reste toujours libre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Template {
    Classic,
    Stairs,
    Colonnade,
    Islands,
    Gantry,
    Pulse,
    Arena,
}

impl Template {
    /// Les salles de sanctuaire restent classiques ; la derniere salle d'un biome est l'arene du boss.
    pub fn for_room(biome: usize, number: usize) -> Self {
        if number == 49 {
            return Self::Arena;
        }
        if number.is_multiple_of(10) {
            return Self::Classic;
        }
        match (biome + number) % 6 {
            0 => Self::Classic,
            1 => Self::Stairs,
            2 => Self::Colonnade,
            3 => Self::Islands,
            4 => Self::Gantry,
            _ => Self::Pulse,
        }
    }

    fn apply(self, room: &mut Room, biome: usize, number: usize) {
        let tile = game::TILE;
        let spikes = |room: &mut Room, column: usize, row: usize, width: usize| {
            room.hazards.push([
                column as f32 * tile,
                row as f32 * tile - 8.0,
                width as f32 * tile,
                8.0,
            ]);
        };
        match self {
            Self::Classic => {
                // Dix architectures, cinq variantes : le couloir bas et les entrees restent libres.
                for section in 0..4 {
                    let column = 7 + section * 4 + (number / 6 + section) % 2;
                    let row = 7 + (biome * 3 + number + section * (biome % 3 + 1) + number / 6) % 6;
                    for offset in 0..2 + biome % 3 {
                        room.set_tile(column + offset, row, true);
                    }
                    if biome % 3 == 1 {
                        for top in 2..row.saturating_sub(2) {
                            room.set_tile(column, top, true);
                        }
                    }
                    if !number.is_multiple_of(10) && number != 49 && section == number % 4 {
                        spikes(room, column, row, 1);
                    }
                }
            }
            Self::Stairs => {
                for step in 0..5 {
                    for offset in 0..3 {
                        room.set_tile(8 + step * 4 + offset, 12 - step, true);
                    }
                }
                if number.is_multiple_of(2) {
                    spikes(room, 16, 10, 2);
                }
            }
            Self::Colonnade => {
                for pillar in 0..4 {
                    let column = 9 + pillar * 5;
                    for top in 1..=4 + (pillar + number) % 3 * 2 {
                        room.set_tile(column, top, true);
                    }
                    for offset in 0..2 {
                        room.set_tile(column + 2 + offset, 10 + pillar % 2, true);
                    }
                }
            }
            Self::Islands => {
                for island in 0..5 {
                    let column = 8 + island * 4;
                    let row = [7, 10, 8, 11, 9][(island + number) % 5];
                    for offset in 0..2 {
                        room.set_tile(column + offset, row, true);
                    }
                    if island == number % 5 {
                        spikes(room, column, row, 2);
                    }
                }
            }
            Self::Gantry => {
                room.platforms.push(MovingPlatform {
                    from: [224.0, 384.0],
                    to: [608.0, 384.0],
                    tiles: 4,
                    period: 4.0 + (number % 3) as f32,
                });
                for offset in 0..4 {
                    room.set_tile(22 + offset, 8, true);
                    room.set_tile(8 + offset, 7, true);
                }
            }
            Self::Pulse => {
                room.pulsing = true;
                for offset in 0..4 {
                    room.set_tile(10 + offset, 11, true);
                    room.set_tile(18 + offset, 9, true);
                }
                spikes(room, 11, 11, 2);
                spikes(room, 19, 9, 2);
            }
            Self::Arena => {
                for offset in 0..4 {
                    room.set_tile(4 + offset, 10, true);
                    room.set_tile(22 + offset, 10, true);
                }
                for offset in 0..3 {
                    room.set_tile(14 + offset, 8, true);
                }
            }
        }
    }
}

impl Room {
    pub fn collision(&self) -> Vec<Tile> {
        self.tiles
            .iter()
            .flat_map(|row| {
                row.bytes().map(|pixel| {
                    if pixel == b'#' {
                        Tile::Solid
                    } else {
                        Tile::Empty
                    }
                })
            })
            .collect()
    }

    pub fn set_tile(&mut self, column: usize, row: usize, solid: bool) {
        if column < COLS && row < self.tiles.len() {
            let mut bytes = self.tiles[row].as_bytes().to_vec();
            if column < bytes.len() {
                bytes[column] = if solid { b'#' } else { b'.' };
                self.tiles[row] = String::from_utf8(bytes).expect("validated ASCII grid");
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Levels {
    pub version: u32,
    pub rooms: Vec<Room>,
    #[serde(default)]
    pub portals: Vec<Portal>,
    #[serde(default)]
    pub biomes: Vec<Biome>,
}

// Une liaison contient ses deux entrees : le retour ne depend pas de la progression.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Portal {
    pub from: usize,
    pub to: usize,
    pub entrance: [f32; 2],
    pub exit: [f32; 2],
    #[serde(default)]
    pub requires_boss: Option<usize>,
}

impl Levels {
    pub const MAX_ROOMS: usize = 600;

    pub fn name(&self, index: usize) -> String {
        if let Some(biome) = self.rooms.get(index).and_then(|room| room.biome) {
            let number = self.rooms[..=index]
                .iter()
                .filter(|room| room.biome == Some(biome))
                .count();
            return format!("{} / {:02}", self.biomes[biome].name, number);
        }
        NAMES.get(index).map_or_else(
            || format!("EXPEDITION {}", index + 1 - NAMES.len()),
            |name| name.to_string(),
        )
    }

    pub fn can_extend(&self, source: usize) -> bool {
        source < self.rooms.len()
            && self.rooms.len() < Self::MAX_ROOMS
            && !self.portals.iter().any(|portal| portal.from == source)
    }

    pub fn generate(
        &mut self,
        source: usize,
        seed: u32,
        difficulty: usize,
    ) -> Result<usize, String> {
        if !self.can_extend(source) {
            return Err(
                "Cette salle a deja une extension ou la limite de 600 salles est atteinte".into(),
            );
        }
        let difficulty = difficulty.clamp(1, 3);
        let mut room = Room {
            biome: self.rooms[source].biome,
            tiles: vec![".".repeat(COLS); ROWS],
            spawn: [96.0, 446.0],
            enemies: Vec::new(),
            hazards: Vec::new(),
            shrine: Some([192.0, 480.0]),
            platforms: Vec::new(),
            pulsing: false,
        };
        // Le chemin bas reste praticable sans double saut ; les plateformes sont facultatives.
        for row in 15..ROWS {
            room.tiles[row] = "#".repeat(COLS);
        }
        for row in 0..15 {
            room.set_tile(0, row, true);
            room.set_tile(COLS - 1, row, true);
        }
        for section in 0..5u32 {
            let mut value = seed.wrapping_add(section.wrapping_mul(0x9e3779b9));
            value = (value ^ (value >> 16)).wrapping_mul(0x85ebca6b);
            value = (value ^ (value >> 13)).wrapping_mul(0xc2b2ae35);
            value ^= value >> 16;
            let column = 13 + section as usize * 3;
            let row = 10 + (value % 3) as usize;
            for offset in 0..2 + (value % 2) as usize {
                room.set_tile(column + offset, row, true);
            }
            if section < difficulty as u32 {
                room.enemies.push(Spawn {
                    position: [column as f32 * game::TILE, 452.0],
                    boss: false,
                    species: room
                        .biome
                        .map_or(0, |biome| (biome * 3 + section as usize % 3) as u8),
                    behavior: room.biome.map_or(Behavior::Legacy, |biome| {
                        Behavior::ALL[1 + (biome + section as usize * 3) % 10]
                    }),
                    skin: 0,
                });
            }
        }
        let index = self.rooms.len();
        let entrance = [320.0, 446.0];
        // Degager uniquement l'entree ajoutee, sans remplacer la salle de rattachement.
        let source_room = &mut self.rooms[source];
        for row in 12..15 {
            for column in 9..13 {
                source_room.set_tile(column, row, false);
            }
        }
        for column in 9..13 {
            source_room.set_tile(column, 15, true);
        }
        source_room.enemies.retain(|enemy| {
            !(enemy.position[0] >= 270.0 && enemy.position[0] < 416.0 && enemy.position[1] > 350.0)
        });
        source_room.hazards.retain(|hazard| {
            !(hazard[0] < 416.0 && hazard[0] + hazard[2] > 288.0 && hazard[1] + hazard[3] > 384.0)
        });
        self.portals.push(Portal {
            from: source,
            to: index,
            entrance,
            exit: room.spawn,
            requires_boss: None,
        });
        self.rooms.push(room);
        Ok(index)
    }

    // Migration des packs sans levels.json ; un fichier present mais invalide n'est jamais ignore.
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = root.join("levels.json");
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut levels = Self::original();
                levels.add_campaign()?;
                return Ok(levels);
            }
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("levels.json exceeds size limit".into());
        }
        let mut levels: Self = serde_json::from_slice(&bytes)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        levels.validate()?;
        // Migration en memoire seulement : ne jamais ecraser les salles personnalisees.
        if levels.biomes.is_empty() {
            levels.add_campaign()?;
        }
        Ok(levels)
    }

    pub fn original() -> Self {
        let enemies = [
            vec![],
            vec![(192.0, 452.0, false), (740.0, 452.0, false)],
            vec![(420.0, 452.0, false)],
            vec![(732.0, 452.0, false)],
            vec![(620.0, 420.0, true)],
            vec![(740.0, 452.0, false)],
            vec![(420.0, 452.0, false)],
        ];
        let rooms = (0..NAMES.len())
            .map(|index| Room {
                biome: None,
                tiles: game::layout(index)
                    .chunks(COLS)
                    .map(|row| {
                        row.iter()
                            .map(|tile| if *tile == Tile::Solid { '#' } else { '.' })
                            .collect()
                    })
                    .collect(),
                spawn: [96.0, 446.0],
                enemies: enemies[index]
                    .iter()
                    .map(|(horizontal, vertical, boss)| Spawn {
                        position: [*horizontal, *vertical],
                        boss: *boss,
                        species: 0,
                        behavior: Behavior::Legacy,
                        skin: 0,
                    })
                    .collect(),
                hazards: game::hazards(index)
                    .iter()
                    .map(|rect| [rect.x, rect.y, rect.w, rect.h])
                    .collect(),
                shrine: game::shrine(index).map(|point| [point.x, point.y]),
                platforms: Vec::new(),
                pulsing: false,
            })
            .collect();
        Self {
            version: 1,
            rooms,
            portals: Vec::new(),
            biomes: Vec::new(),
        }
    }

    pub fn add_campaign(&mut self) -> Result<usize, String> {
        if !self.biomes.is_empty() || self.rooms.len() + 500 > Self::MAX_ROOMS {
            return Err("Campagne deja presente ou capacite insuffisante".into());
        }
        let source = (0..self.rooms.len())
            .find(|index| !self.portals.iter().any(|p| p.from == *index))
            .ok_or("Aucune entree libre pour la campagne")?;
        let start = self.rooms.len();
        let definitions = [
            (
                "BOIS DES MURMURES",
                "LA MATRIARCHE",
                ["Ronceur", "Luciole", "Sylphe"],
                [12, 35, 24],
                [64, 100, 65],
                [110, 235, 135],
            ),
            (
                "CRYPTES DE CRAIE",
                "LE FOSSOYEUR",
                ["Scarabee", "Spectre", "Osselet"],
                [28, 28, 35],
                [140, 145, 152],
                [220, 230, 235],
            ),
            (
                "FONDERIES DE BRAISE",
                "LE COLOSSE",
                ["Scorie", "Salamandre", "Fumee"],
                [45, 18, 20],
                [128, 65, 70],
                [255, 100, 55],
            ),
            (
                "JARDINS DES MAREES",
                "LA REINE ABYSSALE",
                ["Crabe", "Meduse", "Anguille"],
                [8, 38, 45],
                [45, 125, 130],
                [75, 235, 230],
            ),
            (
                "CIMES DE VERRE",
                "LE PRISME",
                ["Eclat", "Phalene", "Givre"],
                [32, 42, 48],
                [125, 185, 200],
                [200, 250, 255],
            ),
            (
                "RUCHES D'AMBRE",
                "LE MONARQUE",
                ["Ouvriere", "Frelon", "Larve"],
                [40, 32, 12],
                [150, 125, 45],
                [250, 220, 70],
            ),
            (
                "OBSERVATOIRES DU VENT",
                "L'ASTROMANCIEN",
                ["Rafale", "Corbeau", "Automate"],
                [30, 40, 55],
                [100, 130, 155],
                [145, 205, 255],
            ),
            (
                "MARAIS DES SPORES",
                "LE MYCELIUM",
                ["Mycose", "Spore", "Limace"],
                [30, 35, 18],
                [90, 110, 50],
                [205, 235, 65],
            ),
            (
                "CITADELLES CARMIN",
                "LA SENTINELLE",
                ["Duelliste", "Gargouille", "Arbaletrier"],
                [42, 15, 30],
                [135, 55, 85],
                [255, 95, 155],
            ),
            (
                "SANCTUAIRE DE L'AUBE",
                "L'ECLIPSE",
                ["Rayon", "Seraphin", "Echo"],
                [35, 38, 32],
                [160, 170, 140],
                [255, 245, 165],
            ),
        ];
        self.biomes = definitions
            .into_iter()
            .enumerate()
            .map(
                |(flora, (name, boss, bestiary, background, stone, accent))| Biome {
                    name: name.into(),
                    boss: boss.into(),
                    bestiary: bestiary.map(String::from),
                    background,
                    stone,
                    accent,
                    flora: flora as u8,
                    architecture: flora as u8,
                },
            )
            .collect();
        for biome in 0..10 {
            for number in 0..50 {
                let mut room = Room {
                    biome: Some(biome),
                    tiles: vec![".".repeat(COLS); ROWS],
                    spawn: [96.0, 446.0],
                    enemies: vec![],
                    hazards: vec![],
                    shrine: (number % 10 == 0 || number == 48).then_some([192.0, 480.0]),
                    platforms: vec![],
                    pulsing: false,
                };
                for row in 15..ROWS {
                    room.tiles[row] = "#".repeat(COLS);
                }
                for column in 0..COLS {
                    room.set_tile(column, 0, true);
                }
                for row in 0..15 {
                    room.set_tile(0, row, true);
                    room.set_tile(COLS - 1, row, true);
                }
                Template::for_room(biome, number).apply(&mut room, biome, number);
                if number == 49 {
                    room.enemies.push(Spawn {
                        position: [640.0, 420.0],
                        boss: true,
                        species: (biome * 3) as u8,
                        behavior: Behavior::ALL[biome + 1],
                        skin: 0,
                    });
                } else if number % 10 != 0 {
                    for species in 0..3 {
                        room.enemies.push(Spawn {
                            position: [400.0 + species as f32 * 145.0, 452.0],
                            boss: false,
                            species: (biome * 3 + species) as u8,
                            behavior: Behavior::ALL[1 + (biome + species * 3) % 10],
                            skin: campaign_skin(biome, number, species),
                        });
                    }
                }
                let index = self.rooms.len();
                let previous = if index == start { source } else { index - 1 };
                self.portals.push(Portal {
                    from: previous,
                    to: index,
                    entrance: if index == start {
                        [320.0, 446.0]
                    } else {
                        [856.0, 446.0]
                    },
                    exit: [96.0, 446.0],
                    requires_boss: (number == 0 && biome > 0).then_some(previous),
                });
                self.rooms.push(room);
            }
        }
        // L'entree de campagne ne doit pas se retrouver incrustee dans un ancien decor.
        for row in 12..15 {
            for column in 9..13 {
                self.rooms[source].set_tile(column, row, false);
            }
        }
        for column in 9..13 {
            self.rooms[source].set_tile(column, 15, true);
        }
        self.rooms[source].enemies.retain(|enemy| {
            !(enemy.position[0] >= 270.0 && enemy.position[0] < 416.0 && enemy.position[1] > 350.0)
        });
        self.rooms[source].hazards.retain(|hazard| {
            !(hazard[0] < 416.0 && hazard[0] + hazard[2] > 288.0 && hazard[1] + hazard[3] > 384.0)
        });
        Ok(start)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || !(NAMES.len()..=Self::MAX_ROOMS).contains(&self.rooms.len()) {
            return Err("Expected levels version 1 and 7 to 600 stable room indices".into());
        }
        if self.biomes.len() > 10
            || self.biomes.iter().any(|b| {
                b.flora > 9
                    || b.architecture > 9
                    || b.name.is_empty()
                    || b.boss.is_empty()
                    || b.name.len() > 64
                    || b.boss.len() > 64
                    || b.bestiary
                        .iter()
                        .any(|name| name.is_empty() || name.len() > 64)
            })
        {
            return Err("Biomes invalides".into());
        }
        let point_valid = |point: [f32; 2], width: f32, height: f32| {
            point.iter().all(|value| value.is_finite())
                && point[0] >= 0.0
                && point[1] >= 0.0
                && point[0] + width <= WIDTH
                && point[1] + height <= HEIGHT
        };
        if self.portals.len() != self.rooms.len() - NAMES.len() {
            return Err("Chaque extension doit avoir une liaison vers une salle precedente".into());
        }
        for (offset, portal) in self.portals.iter().enumerate() {
            if portal.to != NAMES.len() + offset
                || portal.from >= portal.to
                || !point_valid(portal.entrance, 22.0, 34.0)
                || !point_valid(portal.exit, 22.0, 34.0)
                || self.portals[..offset]
                    .iter()
                    .any(|other| other.from == portal.from)
                || portal.requires_boss.is_some_and(|room| {
                    room != portal.from || !self.rooms[room].enemies.iter().any(|enemy| enemy.boss)
                })
            {
                return Err("Liaison d'extension invalide ou dupliquee".into());
            }
        }
        for (index, room) in self.rooms.iter().enumerate() {
            if room.biome.is_some_and(|biome| biome >= self.biomes.len()) {
                return Err(format!("Room {index}: invalid biome"));
            }
            if room.tiles.len() != ROWS
                || room.tiles.iter().any(|row| {
                    row.len() != COLS || row.bytes().any(|pixel| !b".#".contains(&pixel))
                })
            {
                return Err(format!(
                    "Room {index}: expected a {COLS}x{ROWS} ASCII collision grid"
                ));
            }
            if !point_valid(room.spawn, 22.0, 34.0)
                || room.enemies.len() > 64
                || room.hazards.len() > COLS * ROWS
            {
                return Err(format!("Room {index}: invalid spawn or too many entities"));
            }
            for enemy in &room.enemies {
                let (width, height) = if enemy.boss {
                    (46.0, 60.0)
                } else {
                    (26.0, 28.0)
                };
                if enemy.species >= 30
                    || enemy.skin > 4
                    || !point_valid(enemy.position, width, height)
                {
                    return Err(format!("Room {index}: enemy outside room"));
                }
            }
            for hazard in &room.hazards {
                if !hazard.iter().all(|value| value.is_finite())
                    || hazard[2] <= 0.0
                    || hazard[3] <= 0.0
                    || !point_valid([hazard[0], hazard[1]], hazard[2], hazard[3])
                {
                    return Err(format!("Room {index}: invalid hazard"));
                }
            }
            // Seul le mouvement horizontal porte correctement les acteurs de la bibliotheque de collisions.
            if room.platforms.len() > 8
                || room.platforms.iter().any(|platform| {
                    let width = f32::from(platform.tiles) * game::TILE;
                    !platform
                        .from
                        .iter()
                        .chain(&platform.to)
                        .all(|v| v.is_finite())
                        || platform.from[1] != platform.to[1]
                        || !(1..=8).contains(&platform.tiles)
                        || !(1.0..=30.0).contains(&platform.period)
                        || !point_valid(platform.from, width, PLATFORM_HEIGHT)
                        || !point_valid(platform.to, width, PLATFORM_HEIGHT)
                })
            {
                return Err(format!("Room {index}: invalid moving platform"));
            }
            if room
                .shrine
                .is_some_and(|point| !point_valid(point, 0.0, 0.0))
                || ([0, 2, 6].contains(&index) && room.shrine.is_none())
            {
                return Err(format!(
                    "Room {index}: invalid or missing checkpoint shrine"
                ));
            }
        }
        Ok(())
    }

    pub fn save(&self, root: &Path) -> Result<(), String> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        let temporary = root.join("levels.tmp");
        std::fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
        std::fs::rename(temporary, root.join("levels.json")).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campaign_enemies_use_every_human_skin_in_each_biome() {
        let mut levels = Levels::original();
        let start = levels.add_campaign().unwrap();
        levels.validate().unwrap();
        for biome in 0..10 {
            let skins: std::collections::HashSet<u8> = levels.rooms
                [start + biome * 50..start + (biome + 1) * 50]
                .iter()
                .flat_map(|room| room.enemies.iter().map(|enemy| enemy.skin))
                .collect();
            assert_eq!(
                skins,
                [0, 1, 2, 3, 4].into_iter().collect(),
                "biome {biome}"
            );
        }
        // Les bosses gardent leur creature ; un pack ancien sans champ donne des creatures.
        assert!(levels
            .rooms
            .iter()
            .flat_map(|room| &room.enemies)
            .filter(|enemy| enemy.boss)
            .all(|enemy| enemy.skin == 0));
        let legacy: Spawn = serde_json::from_str(r#"{"position":[1.0,2.0],"boss":false}"#).unwrap();
        assert_eq!(legacy.skin, 0);
        levels.rooms[start]
            .enemies
            .push(Spawn { skin: 9, ..legacy });
        assert!(levels.validate().is_err());
    }

    #[test]
    fn campaign_has_ten_connected_biomes_and_gated_bosses() {
        let mut levels = Levels::original();
        let start = levels.add_campaign().unwrap();
        levels.validate().unwrap();
        assert_eq!(levels.rooms.len(), 507);
        for biome in 0..10 {
            let rooms = &levels.rooms[start + biome * 50..start + (biome + 1) * 50];
            assert!(rooms.iter().all(|room| room.biome == Some(biome)));
            assert_eq!(
                rooms
                    .iter()
                    .flat_map(|r| &r.enemies)
                    .filter(|e| e.boss)
                    .count(),
                1
            );
            assert_eq!(rooms[49].enemies[0].behavior, Behavior::ALL[biome + 1]);
            for room in rooms {
                for column in 1..COLS - 1 {
                    assert_eq!(room.tiles[14].as_bytes()[column], b'.');
                    assert_eq!(room.tiles[15].as_bytes()[column], b'#');
                }
            }
        }
        assert_eq!(
            levels
                .portals
                .iter()
                .filter(|p| p.requires_boss.is_some())
                .count(),
            9
        );
        let bytes = serde_json::to_vec_pretty(&levels).unwrap();
        assert!(bytes.len() < 8 * 1024 * 1024);
        assert_eq!(serde_json::from_slice::<Levels>(&bytes).unwrap(), levels);
    }

    #[test]
    fn campaign_uses_every_template_in_each_biome_and_keeps_the_ground_route() {
        let mut levels = Levels::original();
        let start = levels.add_campaign().unwrap();
        levels.validate().unwrap();
        for biome in 0..10 {
            let used: std::collections::HashSet<_> = (0..50)
                .map(|number| format!("{:?}", Template::for_room(biome, number)))
                .collect();
            assert_eq!(used.len(), 7, "biome {biome}");
            for number in 0..50 {
                let room = &levels.rooms[start + biome * 50 + number];
                match Template::for_room(biome, number) {
                    Template::Gantry => assert_eq!(room.platforms.len(), 1),
                    Template::Pulse => assert!(room.pulsing && !room.hazards.is_empty()),
                    _ => assert!(room.platforms.is_empty() && !room.pulsing),
                }
                // Aucun piege ni plateforme ne bloque la route au sol vers la sortie.
                assert!(room
                    .hazards
                    .iter()
                    .all(|hazard| hazard[1] + hazard[3] <= 14.0 * game::TILE));
            }
        }
        let mut broken = levels.clone();
        broken.rooms[start].platforms.push(MovingPlatform {
            from: [100.0, 300.0],
            to: [200.0, 340.0],
            tiles: 2,
            period: 3.0,
        });
        assert!(broken.validate().is_err());
    }

    #[test]
    fn generation_limit_and_all_seeds_keep_a_clear_ground_route() {
        for seed in 0..128 {
            for difficulty in 1..=3 {
                let mut levels = Levels::original();
                let index = levels.generate(0, seed, difficulty).unwrap();
                levels.validate().unwrap();
                let room = &levels.rooms[index];
                assert_eq!(room.enemies.len(), difficulty);
                for column in 1..COLS - 1 {
                    assert_eq!(room.tiles[14].as_bytes()[column], b'.');
                    assert_eq!(room.tiles[15].as_bytes()[column], b'#');
                }
            }
        }
        let mut levels = Levels::original();
        let mut source = 0;
        while levels.rooms.len() < Levels::MAX_ROOMS {
            source = levels.generate(source, source as u32, 3).unwrap();
        }
        levels.validate().unwrap();
        let before = levels.clone();
        assert!(levels.generate(source, 0, 1).is_err());
        assert_eq!(levels, before);
    }

    #[test]
    fn generated_rooms_are_deterministic_connected_and_serializable() {
        let mut levels = Levels::original();
        let original = levels.clone();
        let index = levels.generate(0, 42, 2).unwrap();
        assert_eq!(index, NAMES.len());
        levels.validate().unwrap();
        assert_eq!(levels.rooms[1..NAMES.len()], original.rooms[1..]);
        let mut repeated = original.clone();
        repeated.generate(0, 42, 2).unwrap();
        assert_eq!(levels, repeated);
        let mut different = original;
        different.generate(0, 43, 2).unwrap();
        assert_ne!(levels.rooms[index], different.rooms[index]);
        assert!(levels.generate(0, 1, 1).is_err());
        levels.generate(index, 1, 3).unwrap();
        levels.validate().unwrap();
        assert_eq!(
            serde_json::from_slice::<Levels>(&serde_json::to_vec(&levels).unwrap()).unwrap(),
            levels
        );
        levels.portals[0].from = levels.portals[0].to;
        assert!(levels.validate().is_err());
    }

    #[test]
    fn original_levels_match_game_and_round_trip() {
        let levels = Levels::original();
        levels.validate().unwrap();
        for index in 0..NAMES.len() {
            assert_eq!(levels.rooms[index].collision(), game::layout(index));
        }
        let bytes = serde_json::to_vec(&levels).unwrap();
        assert_eq!(serde_json::from_slice::<Levels>(&bytes).unwrap(), levels);
    }

    #[test]
    fn paint_changes_only_selected_tile_and_invalid_data_is_rejected() {
        let mut levels = Levels::original();
        levels.rooms[0].set_tile(2, 2, true);
        assert_eq!(levels.rooms[0].collision()[2 * COLS + 2], Tile::Solid);
        levels.rooms[0].set_tile(2, 2, false);
        assert_eq!(levels, Levels::original());
        levels.rooms[0].tiles[0] = "bad".into();
        assert!(levels.validate().is_err());
        levels = Levels::original();
        levels.rooms[0].shrine = None;
        assert!(levels.validate().is_err());
    }
}
