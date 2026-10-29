//! Personnages jouables et apparences ennemies : regles de deblocage et effets de jeu.
//!
//! Aucun sprite n'est dessine dans le code : l'art de chaque personnage est un ensemble de
//! PNG du pack (`assets/sprites`), decrit par le manifeste sous le nom retourne par `sprite`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Character {
    #[default]
    Lumen,
    Ninja,
    Samurai,
    Soldier,
    Goddess,
    Stalker,
}

/// Condition de deblocage, evaluee depuis la progression sauvegardee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Requirement {
    Free,
    Kills(u32),
    Bosses(usize),
}

impl Requirement {
    pub fn met(self, kills: u32, bosses: usize) -> bool {
        match self {
            Self::Free => true,
            Self::Kills(needed) => kills >= needed,
            Self::Bosses(needed) => bosses >= needed,
        }
    }

    /// Libelle avec la progression courante, par exemple `30 ENNEMIS (12/30)`.
    pub fn label(self, kills: u32, bosses: usize) -> String {
        match self {
            Self::Free => "DISPONIBLE".into(),
            Self::Kills(needed) => format!("{needed} ENNEMIS ({}/{needed})", kills.min(needed)),
            Self::Bosses(needed) => format!("{needed} BOSS ({}/{needed})", bosses.min(needed)),
        }
    }
}

/// Multiplicateurs et bonus appliques par un personnage ; 1,0 et 0 sont neutres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterStats {
    pub walk: f32,
    pub jump: f32,
    pub dash_cooldown: f32,
    pub reach: f32,
    pub fall: f32,
    pub hearts: i32,
    pub melee_damage: i32,
    pub ranged_damage: i32,
    pub invincibility: f32,
}

const NEUTRAL: CharacterStats = CharacterStats {
    walk: 1.0,
    jump: 1.0,
    dash_cooldown: 1.0,
    reach: 1.0,
    fall: 1.0,
    hearts: 0,
    melee_damage: 0,
    ranged_damage: 0,
    invincibility: 0.0,
};

impl Character {
    pub const ALL: [Self; 6] = [
        Self::Lumen,
        Self::Ninja,
        Self::Samurai,
        Self::Soldier,
        Self::Goddess,
        Self::Stalker,
    ];

    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|other| *other == self)
            .unwrap_or(0)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Lumen => "LUMEN",
            Self::Ninja => "NINJA",
            Self::Samurai => "SAMOURAI",
            Self::Soldier => "SOLDAT",
            Self::Goddess => "DEESSE",
            Self::Stalker => "TRAQUEUSE",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Lumen => "Equilibre : aucun bonus, aucun malus",
            Self::Ninja => "+12% vitesse, ruee 30% plus rapide, -1 coeur",
            Self::Samurai => "+20% portee et +1 degat en melee, -5% vitesse",
            Self::Soldier => "+2 coeurs, +1 degat a distance, -8% vitesse",
            Self::Goddess => "+10% saut, chute lente, +0,4 s d'invulnerabilite",
            Self::Stalker => "+1 degat en melee, +8% vitesse, ruee 15% plus rapide, -10% portee",
        }
    }

    /// Nom du sprite dans le manifeste ; Lumen utilise le sprite historique `player`.
    pub fn sprite(self) -> &'static str {
        match self {
            Self::Lumen => "player",
            Self::Ninja => "ninja",
            Self::Samurai => "samurai",
            Self::Soldier => "soldier",
            Self::Goddess => "goddess",
            Self::Stalker => "stalker",
        }
    }

    pub fn requirement(self) -> Requirement {
        match self {
            Self::Lumen => Requirement::Free,
            Self::Ninja => Requirement::Kills(30),
            Self::Samurai => Requirement::Bosses(3),
            Self::Soldier => Requirement::Kills(80),
            Self::Goddess => Requirement::Bosses(6),
            Self::Stalker => Requirement::Kills(150),
        }
    }

    pub fn unlocked(self, kills: u32, bosses: usize) -> bool {
        self.requirement().met(kills, bosses)
    }

    pub fn stats(self) -> CharacterStats {
        match self {
            Self::Lumen => NEUTRAL,
            Self::Ninja => CharacterStats {
                walk: 1.12,
                dash_cooldown: 0.7,
                hearts: -1,
                ..NEUTRAL
            },
            Self::Samurai => CharacterStats {
                walk: 0.95,
                reach: 1.2,
                melee_damage: 1,
                ..NEUTRAL
            },
            Self::Soldier => CharacterStats {
                walk: 0.92,
                hearts: 2,
                ranged_damage: 1,
                ..NEUTRAL
            },
            Self::Goddess => CharacterStats {
                jump: 1.1,
                fall: 0.8,
                invincibility: 0.4,
                ..NEUTRAL
            },
            Self::Stalker => CharacterStats {
                walk: 1.08,
                dash_cooldown: 0.85,
                reach: 0.9,
                melee_damage: 1,
                ..NEUTRAL
            },
        }
    }
}

/// Apparence d'un ennemi : creature du biome ou equivalent humain des personnages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Skin {
    #[default]
    Creature,
    Ninja,
    Samurai,
    Soldier,
    Goddess,
}

impl Skin {
    pub const ALL: [Self; 5] = [
        Self::Creature,
        Self::Ninja,
        Self::Samurai,
        Self::Soldier,
        Self::Goddess,
    ];

    pub fn from_index(index: u8) -> Option<Self> {
        Self::ALL.get(usize::from(index)).copied()
    }

    pub fn index(self) -> u8 {
        Self::ALL
            .iter()
            .position(|other| *other == self)
            .unwrap_or(0) as u8
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Creature => "Creature",
            Self::Ninja => "Kunoichi",
            Self::Samurai => "Samourai",
            Self::Soldier => "Soldat",
            Self::Goddess => "Deesse dechue",
        }
    }

    /// Nom du sprite dans le manifeste ; la creature garde le sprite historique `enemy`.
    pub fn sprite(self) -> &'static str {
        match self {
            Self::Creature => "enemy",
            Self::Ninja => "enemy_ninja",
            Self::Samurai => "enemy_samurai",
            Self::Soldier => "enemy_soldier",
            Self::Goddess => "enemy_goddess",
        }
    }

    /// Points de vie ajoutes ou retires a un ennemi ordinaire.
    pub fn health_bonus(self) -> i32 {
        match self {
            Self::Creature | Self::Soldier => 0,
            Self::Ninja => -1,
            Self::Samurai => 1,
            Self::Goddess => 2,
        }
    }

    pub fn speed(self) -> f32 {
        match self {
            Self::Ninja => 1.25,
            Self::Goddess => 0.9,
            _ => 1.0,
        }
    }

    /// Multiplie le delai entre deux attaques : plus petit = plus agressif.
    pub fn pace(self) -> f32 {
        match self {
            Self::Soldier => 0.85,
            Self::Goddess => 0.9,
            _ => 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Content;
    use std::path::Path;

    /// Nombre de pixels opaques de chaque ligne de la frame 0 d'un sprite du pack.
    fn row_widths(content: &Content, name: &str) -> Vec<usize> {
        let sprite = &content.manifest.sprites[name];
        let image = content.image(sprite, 0).unwrap();
        (0..u32::from(image.height))
            .map(|y| {
                (0..u32::from(image.width))
                    .filter(|x| image.get_pixel(*x, y).a > 0.0)
                    .count()
            })
            .collect()
    }

    fn pack() -> Content {
        Content::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")).unwrap()
    }

    #[test]
    fn every_character_and_skin_comes_from_png_files_of_the_pack() {
        let content = pack();
        for character in &Character::ALL[1..] {
            let sprite = &content.manifest.sprites[character.sprite()];
            assert!(sprite
                .frames
                .iter()
                .all(|path| path.starts_with("sprites/") && path.ends_with(".png")));
            assert!(row_widths(&content, character.sprite())
                .iter()
                .any(|width| *width > 0));
        }
        for skin in &Skin::ALL[1..] {
            let sprite = &content.manifest.sprites[skin.sprite()];
            assert!(sprite.frames.iter().all(|path| path.ends_with(".png")));
        }
    }

    #[test]
    fn female_sprites_have_an_hourglass_silhouette() {
        let content = pack();
        // Zones de la frame de repos (joueur 32x48, ennemi 32x32) : buste, taille puis hanches,
        // et ecart minimal de pixels opaques (la Traqueuse a des bras detaches du buste).
        for (name, bust, waist, hips, margin) in [
            ("ninja", 16..24, 24..30, 30..38, 6),
            ("goddess", 16..24, 24..30, 30..38, 6),
            ("stalker", 15..21, 22..26, 27..31, 4),
            ("enemy_ninja", 10..16, 16..20, 20..24, 6),
            ("enemy_goddess", 10..16, 16..20, 20..26, 6),
        ] {
            let widths = row_widths(&content, name);
            let max = |range: std::ops::Range<usize>| widths[range].iter().copied().max().unwrap();
            let narrowest = widths[waist].iter().copied().min().unwrap();
            assert!(max(bust) >= narrowest + margin, "{name}: buste trop etroit");
            assert!(
                max(hips) >= narrowest + margin,
                "{name}: hanches trop etroites"
            );
        }
    }

    #[test]
    fn unlocks_follow_progress_and_stats_stay_balanced() {
        assert!(Character::Lumen.unlocked(0, 0));
        assert!(!Character::Ninja.unlocked(29, 0) && Character::Ninja.unlocked(30, 0));
        assert!(!Character::Samurai.unlocked(200, 2) && Character::Samurai.unlocked(0, 3));
        assert!(!Character::Soldier.unlocked(79, 9) && Character::Soldier.unlocked(80, 0));
        assert!(!Character::Goddess.unlocked(200, 5) && Character::Goddess.unlocked(0, 6));
        assert!(!Character::Stalker.unlocked(149, 9) && Character::Stalker.unlocked(150, 0));
        assert_eq!(Requirement::Kills(30).label(12, 0), "30 ENNEMIS (12/30)");
        assert_eq!(Requirement::Bosses(3).label(0, 9), "3 BOSS (3/3)");
        for character in Character::ALL {
            let stats = character.stats();
            assert!((0.8..=1.25).contains(&stats.walk), "{character:?}");
            assert!((-1..=2).contains(&stats.hearts));
            assert_eq!(Character::ALL[character.index()], character);
            assert!(!character.name().is_empty() && !character.description().is_empty());
        }
        assert_eq!(Character::Lumen.stats(), NEUTRAL);
        let json = serde_json::to_string(&Character::Goddess).unwrap();
        assert_eq!(json, "\"goddess\"");
        assert!(serde_json::from_str::<Character>("\"dragon\"").is_err());
    }

    #[test]
    fn skins_round_trip_and_keep_enemies_within_reasonable_stats() {
        for (index, skin) in Skin::ALL.into_iter().enumerate() {
            assert_eq!(Skin::from_index(index as u8), Some(skin));
            assert_eq!(usize::from(skin.index()), index);
            assert!((0.8..=1.3).contains(&skin.speed()));
            assert!((0.8..=1.0).contains(&skin.pace()));
            assert!((-1..=2).contains(&skin.health_bonus()));
        }
        assert_eq!(Skin::from_index(5), None);
        assert_eq!(Skin::Creature.sprite(), "enemy");
    }
}
