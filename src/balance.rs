//! Parametres d'equilibrage modifiables sans recompiler : `balance.json` a cote du pack d'assets.
//!
//! Un fichier absent donne les valeurs par defaut ; un fichier invalide est refuse au demarrage.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlayerTuning {
    /// Pixels par seconde carree.
    pub gravity: f32,
    pub max_fall: f32,
    pub wall_slide_fall: f32,
    pub walk_speed: f32,
    pub acceleration: f32,
    pub jump_speed: f32,
    /// Vitesse horizontale puis verticale du saut mural.
    pub wall_jump: [f32; 2],
    pub wall_lock: f32,
    pub coyote: f32,
    pub jump_buffer: f32,
    pub dash_speed: f32,
    pub dash_time: f32,
    pub dash_cooldown: f32,
}

impl Default for PlayerTuning {
    fn default() -> Self {
        Self {
            gravity: 1200.0,
            max_fall: 700.0,
            wall_slide_fall: 120.0,
            walk_speed: 235.0,
            acceleration: 1800.0,
            jump_speed: 510.0,
            wall_jump: [290.0, 470.0],
            wall_lock: 0.16,
            coyote: 0.1,
            jump_buffer: 0.12,
            dash_speed: 620.0,
            dash_time: 0.16,
            dash_cooldown: 0.45,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnemyTuning {
    /// Bonus de vitesse cumule par rang de biome (0..9).
    pub speed_per_tier: f32,
    /// Reduction du delai d'attaque par rang de biome.
    pub pace_per_tier: f32,
    /// Un boss est enrage sous cette fraction de sa vitalite.
    pub enrage_threshold: f32,
    pub enrage_cooldown: f32,
    pub enrage_speed: f32,
}

impl Default for EnemyTuning {
    fn default() -> Self {
        Self {
            speed_per_tier: 0.025,
            pace_per_tier: 0.02,
            enrage_threshold: 0.5,
            enrage_cooldown: 0.7,
            enrage_speed: 1.2,
        }
    }
}

/// Reglages d'un niveau de difficulte : calme, normal puis epreuve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelTuning {
    pub bonus_hearts: i32,
    pub invincibility: f32,
    /// Multiplie le delai entre deux attaques ennemies.
    pub attack_pace: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tuning {
    pub version: u32,
    pub player: PlayerTuning,
    pub enemies: EnemyTuning,
    pub difficulty: [LevelTuning; 3],
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            version: 1,
            player: PlayerTuning::default(),
            enemies: EnemyTuning::default(),
            difficulty: [
                LevelTuning {
                    bonus_hearts: 2,
                    invincibility: 2.0,
                    attack_pace: 1.25,
                },
                LevelTuning {
                    bonus_hearts: 0,
                    invincibility: 1.4,
                    attack_pace: 1.0,
                },
                LevelTuning {
                    bonus_hearts: 0,
                    invincibility: 1.0,
                    attack_pace: 0.8,
                },
            ],
        }
    }
}

impl Tuning {
    /// Les bornes protegent la simulation d'un fichier edite a la main, pas l'equilibrage.
    pub fn validate(&self) -> Result<(), String> {
        let check = |name: &str, value: f32, min: f32, max: f32| {
            if value.is_finite() && (min..=max).contains(&value) {
                Ok(())
            } else {
                Err(format!(
                    "balance.json: {name} doit etre entre {min} et {max}"
                ))
            }
        };
        let player = &self.player;
        let enemies = &self.enemies;
        if self.version != 1 {
            return Err("balance.json: version 1 attendue".into());
        }
        check("gravity", player.gravity, 100.0, 5000.0)?;
        check("max_fall", player.max_fall, 100.0, 2000.0)?;
        check(
            "wall_slide_fall",
            player.wall_slide_fall,
            10.0,
            player.max_fall,
        )?;
        check("walk_speed", player.walk_speed, 50.0, 800.0)?;
        check("acceleration", player.acceleration, 100.0, 10000.0)?;
        check("jump_speed", player.jump_speed, 100.0, 1200.0)?;
        check("wall_jump[0]", player.wall_jump[0], 0.0, 800.0)?;
        check("wall_jump[1]", player.wall_jump[1], 100.0, 1200.0)?;
        check("wall_lock", player.wall_lock, 0.0, 1.0)?;
        check("coyote", player.coyote, 0.0, 0.5)?;
        check("jump_buffer", player.jump_buffer, 0.0, 0.5)?;
        check("dash_speed", player.dash_speed, 100.0, 1500.0)?;
        check("dash_time", player.dash_time, 0.02, 0.6)?;
        check("dash_cooldown", player.dash_cooldown, 0.05, 5.0)?;
        check("speed_per_tier", enemies.speed_per_tier, 0.0, 0.2)?;
        check("pace_per_tier", enemies.pace_per_tier, 0.0, 0.08)?;
        check("enrage_threshold", enemies.enrage_threshold, 0.0, 1.0)?;
        check("enrage_cooldown", enemies.enrage_cooldown, 0.2, 1.0)?;
        check("enrage_speed", enemies.enrage_speed, 1.0, 3.0)?;
        for level in &self.difficulty {
            if !(0..=4).contains(&level.bonus_hearts) {
                return Err("balance.json: bonus_hearts doit etre entre 0 et 4".into());
            }
            check("invincibility", level.invincibility, 0.2, 5.0)?;
            check("attack_pace", level.attack_pace, 0.3, 3.0)?;
        }
        Ok(())
    }

    /// Fichier absent : valeurs par defaut. Present mais invalide : erreur explicite.
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = root.join("balance.json");
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default())
            }
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        if bytes.len() > 64 * 1024 {
            return Err("balance.json depasse 64 Kio".into());
        }
        let tuning: Self = serde_json::from_slice(&bytes)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        tuning.validate()?;
        Ok(tuning)
    }
}

static ACTIVE: OnceLock<Tuning> = OnceLock::new();

/// Installe les reglages du processus ; seul le premier appel compte.
pub fn install(tuning: Tuning) {
    let _ = ACTIVE.set(tuning);
}

/// Reglages actifs, ou valeurs par defaut quand rien n'a ete installe (tests, outils).
pub fn active() -> &'static Tuning {
    ACTIVE.get_or_init(Tuning::default)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_dir(name: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("lumen-balance-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn defaults_are_valid_and_match_the_checked_in_file() {
        Tuning::default().validate().unwrap();
        let shipped = Tuning::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")).unwrap();
        assert_eq!(shipped, Tuning::default());
    }

    #[test]
    fn partial_files_keep_defaults_and_invalid_ones_are_rejected() {
        let missing = unique_dir("missing");
        assert_eq!(Tuning::load(&missing).unwrap(), Tuning::default());
        std::fs::write(
            missing.join("balance.json"),
            r#"{"player":{"jump_speed":600.0}}"#,
        )
        .unwrap();
        let tuned = Tuning::load(&missing).unwrap();
        assert_eq!(tuned.player.jump_speed, 600.0);
        assert_eq!(tuned.player.gravity, 1200.0);
        for bad in [
            r#"{"player":{"gravity":-1.0}}"#,
            r#"{"player":{"unknown":1}}"#,
            r#"{"version":2}"#,
            r#"{"difficulty":[{"bonus_hearts":9,"invincibility":1.0,"attack_pace":1.0},{"bonus_hearts":0,"invincibility":1.0,"attack_pace":1.0},{"bonus_hearts":0,"invincibility":1.0,"attack_pace":1.0}]}"#,
            "not json",
        ] {
            std::fs::write(missing.join("balance.json"), bad).unwrap();
            assert!(Tuning::load(&missing).is_err(), "{bad}");
        }
        let _ = std::fs::remove_dir_all(&missing);
    }

    #[test]
    fn jump_and_dash_defaults_stay_within_designed_reach() {
        let player = Tuning::default().player;
        // Hauteur du saut simple : v^2 / 2g, a conserver sous quatre tuiles.
        let height = player.jump_speed.powi(2) / (2.0 * player.gravity);
        assert!((96.0..128.0).contains(&height), "{height}");
        let dash = player.dash_speed * player.dash_time;
        assert!((80.0..160.0).contains(&dash), "{dash}");
    }
}
