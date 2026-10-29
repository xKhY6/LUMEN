//! Contrat versionne des ressources externes, partageable avec un futur editeur.

use crate::game::SoundCue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sprite {
    pub size: [u16; 2],
    pub frames: Vec<String>,
    pub fps: f32,
    pub walk: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoomPalette {
    pub background: [u8; 3],
    pub stone: [u8; 3],
    pub accent: [u8; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weapon {
    pub name: String,
    pub damage: i32,
    pub reach: f32,
    pub height: f32,
    pub duration: f32,
    pub cooldown: f32,
    pub color: [u8; 3],
    // L'absence de projectile conserve le combat de melee des anciens packs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projectile: Option<ProjectileSettings>,
    #[serde(default)]
    pub automatic: bool,
    // Le rendu laser reste explicite, independant du nom personnalise de l'arme.
    #[serde(default)]
    pub laser: bool,
    // Zero conserve l'acces immediat des armes des anciens packs.
    #[serde(default)]
    pub unlock_kills: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectileSettings {
    pub speed: f32,
    pub range: f32,
    pub radius: f32,
}

pub fn default_weapons() -> Vec<Weapon> {
    [
        ("Epee", 1, 62.0, 52.0, 0.18, 0.32, [239, 222, 160]),
        ("Dague", 1, 38.0, 34.0, 0.10, 0.18, [120, 220, 210]),
        ("Lance", 1, 110.0, 24.0, 0.16, 0.42, [155, 195, 245]),
        ("Marteau", 3, 50.0, 70.0, 0.28, 0.75, [245, 140, 115]),
    ]
    .into_iter()
    .map(
        |(name, damage, reach, height, duration, cooldown, color)| Weapon {
            name: name.into(),
            damage,
            reach,
            height,
            duration,
            cooldown,
            color,
            projectile: None,
            automatic: false,
            laser: false,
            unlock_kills: 0,
        },
    )
    .chain(
        [
            ("Arc", 1, 520.0, 600.0, 3.0, 0.4, [190, 225, 150]),
            ("Arbalete", 3, 800.0, 850.0, 3.0, 0.85, [235, 190, 110]),
            (
                "Baton d'etincelles",
                2,
                360.0,
                450.0,
                7.0,
                0.55,
                [100, 205, 245],
            ),
        ]
        .into_iter()
        .map(
            |(name, damage, speed, range, radius, cooldown, color)| Weapon {
                name: name.into(),
                damage,
                reach: 32.0,
                height: 32.0,
                duration: 0.12,
                cooldown,
                color,
                projectile: Some(ProjectileSettings {
                    speed,
                    range,
                    radius,
                }),
                automatic: false,
                laser: false,
                unlock_kills: 0,
            },
        ),
    )
    .chain(
        [
            (
                "Pistolet leger",
                1,
                720.0,
                650.0,
                0.25,
                false,
                2,
                [235, 210, 150],
            ),
            (
                "Pistolet lourd",
                3,
                780.0,
                700.0,
                0.6,
                false,
                5,
                [240, 155, 110],
            ),
            (
                "Fusil de combat",
                2,
                1000.0,
                1000.0,
                0.3,
                false,
                8,
                [160, 220, 175],
            ),
            (
                "Fusil de precision",
                5,
                1200.0,
                1200.0,
                1.0,
                false,
                12,
                [155, 205, 245],
            ),
            (
                "Mitrailleuse legere",
                1,
                850.0,
                750.0,
                0.10,
                true,
                16,
                [245, 205, 100],
            ),
            (
                "Mitrailleuse lourde",
                2,
                900.0,
                900.0,
                0.16,
                true,
                24,
                [245, 135, 130],
            ),
        ]
        .into_iter()
        .map(
            |(name, damage, speed, range, cooldown, automatic, unlock_kills, color)| Weapon {
                name: name.into(),
                damage,
                reach: 32.0,
                height: 32.0,
                duration: 0.06,
                cooldown,
                color,
                automatic,
                laser: false,
                unlock_kills,
                projectile: Some(ProjectileSettings {
                    speed,
                    range,
                    radius: 3.0,
                }),
            },
        ),
    )
    .chain(
        [
            ("Pistolet laser", 2, 0.22, false, 28, 800.0, [80, 235, 245]),
            (
                "Pistolet laser lourd",
                4,
                0.5,
                false,
                32,
                900.0,
                [245, 100, 160],
            ),
            ("Fusil laser", 3, 0.28, false, 38, 1100.0, [120, 245, 145]),
            (
                "Fusil laser de precision",
                7,
                0.9,
                false,
                44,
                1200.0,
                [105, 185, 255],
            ),
            (
                "Mitrailleuse laser",
                2,
                0.10,
                true,
                52,
                900.0,
                [245, 220, 90],
            ),
            (
                "Mitrailleuse laser lourde",
                3,
                0.16,
                true,
                64,
                1100.0,
                [255, 100, 105],
            ),
        ]
        .into_iter()
        .map(
            |(name, damage, cooldown, automatic, unlock_kills, range, color)| Weapon {
                name: name.into(),
                damage,
                reach: 32.0,
                height: 32.0,
                duration: 0.06,
                cooldown,
                automatic,
                laser: true,
                unlock_kills,
                color,
                projectile: Some(ProjectileSettings {
                    speed: 1200.0,
                    range,
                    radius: 3.0,
                }),
            },
        ),
    )
    .collect()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub font: String,
    pub sprites: BTreeMap<String, Sprite>,
    pub rooms: Vec<RoomPalette>,
    pub music: String,
    pub effects: BTreeMap<SoundCue, String>,
    // Les packs precedents conservent l'epee et recoivent le catalogue sans migration destructive.
    #[serde(default = "default_weapons")]
    pub weapons: Vec<Weapon>,
}

// Les chemins appartiennent au pack, jamais au repertoire de sauvegarde ni a la machine de l'auteur.
pub fn relative_path(root: &Path, name: &str) -> Result<PathBuf, String> {
    if name.is_empty()
        || name.contains('\\')
        || name.contains(':')
        || Path::new(name)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!(
            "Asset path must be relative and contained in the pack: {name}"
        ));
    }
    Ok(root.join(name))
}

impl Manifest {
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        let temporary = path.with_extension("tmp");
        // Ne remplacer le manifeste actif qu'une fois le document complet et valide ecrit.
        std::fs::write(&temporary, bytes)
            .map_err(|error| format!("{}: {error}", temporary.display()))?;
        std::fs::rename(&temporary, path).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.weapons.is_empty() || self.weapons.len() > 32 {
            return Err("Le catalogue doit contenir entre 1 et 32 armes".into());
        }
        if !self.weapons.iter().any(|weapon| weapon.unlock_kills == 0) {
            return Err("Au moins une arme doit etre disponible des le depart".into());
        }
        for (index, weapon) in self.weapons.iter().enumerate() {
            if weapon.unlock_kills > 200
                || ((weapon.automatic || weapon.laser) && weapon.projectile.is_none())
            {
                return Err(format!(
                    "Progression ou tir automatique invalide : {}",
                    weapon.name
                ));
            }
            if let Some(projectile) = &weapon.projectile {
                if !projectile.speed.is_finite()
                    || !(100.0..=1200.0).contains(&projectile.speed)
                    || !projectile.range.is_finite()
                    || !(64.0..=1200.0).contains(&projectile.range)
                    || !projectile.radius.is_finite()
                    || !(2.0..=12.0).contains(&projectile.radius)
                {
                    return Err(format!("Projectile invalide : {}", weapon.name));
                }
            }
            if weapon.name.trim().is_empty()
                || weapon.name.chars().count() > 32
                || self.weapons[..index]
                    .iter()
                    .any(|other| other.name == weapon.name)
                || !(1..=20).contains(&weapon.damage)
                || !weapon.reach.is_finite()
                || !(16.0..=160.0).contains(&weapon.reach)
                || !weapon.height.is_finite()
                || !(16.0..=100.0).contains(&weapon.height)
                || !weapon.duration.is_finite()
                || !(0.05..=0.5).contains(&weapon.duration)
                || !weapon.cooldown.is_finite()
                || !(weapon.duration + 0.02..=2.0).contains(&weapon.cooldown)
            {
                return Err(format!("Arme invalide : {}", weapon.name));
            }
        }
        if self.version != 1 {
            return Err(format!("Unsupported asset version: {}", self.version));
        }
        if self.rooms.len() != crate::game::NAMES.len() {
            return Err("Expected seven room palettes".into());
        }
        for (name, minimum) in [
            ("player", 6),
            ("enemy", 4),
            ("heart", 1),
            ("fern", 1),
            ("crystal", 1),
            ("glow", 1),
        ] {
            let sprite = self
                .sprites
                .get(name)
                .ok_or_else(|| format!("Missing sprite: {name}"))?;
            if sprite.frames.len() < minimum {
                return Err(format!("Not enough frames for {name}"));
            }
        }
        for room in 0..self.rooms.len() {
            if !self.sprites.contains_key(&format!("tile_{room}")) {
                return Err(format!("Missing tile for room {room}"));
            }
        }
        for (name, sprite) in &self.sprites {
            if sprite.size.contains(&0)
                || sprite.size.iter().any(|size| *size > 2048)
                || sprite.frames.is_empty()
                || sprite.frames.len() > 256
                || !sprite.fps.is_finite()
                || !(0.1..=120.0).contains(&sprite.fps)
                || sprite.walk.is_empty()
                || sprite
                    .walk
                    .iter()
                    .any(|index| *index >= sprite.frames.len())
            {
                return Err(format!(
                    "Invalid sprite dimensions, animation or frame index: {name}"
                ));
            }
            for path in &sprite.frames {
                relative_path(Path::new("."), path)?;
            }
        }
        relative_path(Path::new("."), &self.font)?;
        relative_path(Path::new("."), &self.music)?;
        for cue in SoundCue::ALL {
            let path = self
                .effects
                .get(&cue)
                .ok_or_else(|| format!("Missing sound: {cue:?}"))?;
            relative_path(Path::new("."), path)?;
        }
        Ok(())
    }
}

pub struct Content {
    pub root: PathBuf,
    pub manifest: Manifest,
}

impl Content {
    pub fn load(root: PathBuf) -> Result<Self, String> {
        let path = root.join("manifest.json");
        let bytes = read_bounded(&path, 1024 * 1024)?;
        let manifest: Manifest = serde_json::from_slice(&bytes)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        manifest.validate()?;
        Ok(Self { root, manifest })
    }

    pub fn read(&self, name: &str, maximum: u64) -> Result<Vec<u8>, String> {
        read_bounded(&relative_path(&self.root, name)?, maximum)
    }

    pub fn image(
        &self,
        sprite: &Sprite,
        frame: usize,
    ) -> Result<macroquad::prelude::Image, String> {
        let name = sprite
            .frames
            .get(frame)
            .ok_or_else(|| format!("Frame index out of range: {frame}"))?;
        let bytes = self.read(name, 16 * 1024 * 1024)?;
        let mut reader =
            image::io::Reader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
        let mut limits = image::io::Limits::default();
        limits.max_image_width = Some(sprite.size[0] as u32);
        limits.max_image_height = Some(sprite.size[1] as u32);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let decoded = reader
            .decode()
            .map_err(|error| format!("{name}: {error}"))?
            .to_rgba8();
        if [decoded.width() as u16, decoded.height() as u16] != sprite.size {
            return Err(format!("{name}: PNG dimensions do not match manifest"));
        }
        Ok(macroquad::prelude::Image {
            width: sprite.size[0],
            height: sprite.size[1],
            bytes: decoded.into_raw(),
        })
    }

    pub fn wav(&self, name: &str) -> Result<Vec<u8>, String> {
        let bytes = self.read(name, 64 * 1024 * 1024)?;
        let mut reader = hound::WavReader::new(std::io::Cursor::new(&bytes))
            .map_err(|error| format!("{name}: {error}"))?;
        let spec = reader.spec();
        if ![1, 2].contains(&spec.channels)
            || spec.bits_per_sample != 16
            || spec.sample_format != hound::SampleFormat::Int
            || !(8_000..=96_000).contains(&spec.sample_rate)
            || reader.duration() == 0
            || reader.duration() > spec.sample_rate * 300
        {
            return Err(format!(
                "{name}: expected PCM16 mono/stereo WAV, 8-96 kHz, at most 300 seconds"
            ));
        }
        // Parcourir le PCM detecte aussi les donnees tronquees, pas seulement un entete valide.
        for sample in reader.samples::<i16>() {
            sample.map_err(|error| format!("{name}: {error}"))?;
        }
        Ok(bytes)
    }

    pub fn validate_files(&self) -> Result<(), String> {
        for sprite in self.manifest.sprites.values() {
            for frame in 0..sprite.frames.len() {
                self.image(sprite, frame)?;
            }
        }
        self.read(&self.manifest.font, 16 * 1024 * 1024)?;
        self.wav(&self.manifest.music)?;
        for name in self.manifest.effects.values() {
            self.wav(name)?;
        }
        Ok(())
    }
}

// Borner les allocations avant le decodage : un fichier exporte incorrectement doit etre refuse.
fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if bytes.len() as u64 > maximum {
        return Err(format!("Asset exceeds size limit: {}", path.display()));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_unlock_rules_and_automatic_fire_are_validated() {
        let mut manifest = project_content().manifest;
        assert_eq!(manifest.weapons.len(), 19);
        assert_eq!(manifest.weapons, default_weapons());
        let mut old = serde_json::to_value(&manifest.weapons[0]).unwrap();
        old.as_object_mut().unwrap().remove("automatic");
        old.as_object_mut().unwrap().remove("laser");
        old.as_object_mut().unwrap().remove("unlock_kills");
        let old: Weapon = serde_json::from_value(old).unwrap();
        assert!(!old.automatic);
        assert!(!old.laser);
        assert_eq!(old.unlock_kills, 0);
        manifest.weapons[0].automatic = true;
        assert!(manifest.validate().is_err());
        manifest.weapons[0].automatic = false;
        manifest.weapons[0].laser = true;
        assert!(manifest.validate().is_err());
        manifest.weapons[0].laser = false;
        manifest.weapons[0].unlock_kills = 201;
        assert!(manifest.validate().is_err());
        for weapon in &mut manifest.weapons {
            weapon.unlock_kills = 1;
        }
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn ranged_settings_validate_and_old_weapons_remain_melee() {
        let mut manifest = project_content().manifest;
        let mut json = serde_json::to_value(&manifest.weapons[0]).unwrap();
        json.as_object_mut().unwrap().remove("projectile");
        let old_weapon: Weapon = serde_json::from_value(json).unwrap();
        assert!(old_weapon.projectile.is_none());
        for invalid in [f32::NAN, 0.0, 1500.0] {
            let settings = ProjectileSettings {
                speed: invalid,
                range: 600.0,
                radius: 3.0,
            };
            manifest.weapons[0].projectile = Some(settings);
            assert!(manifest.validate().is_err());
        }
        manifest.weapons[0].projectile = Some(ProjectileSettings {
            speed: 500.0,
            range: 0.0,
            radius: 3.0,
        });
        assert!(manifest.validate().is_err());
        manifest.weapons[0].projectile = Some(ProjectileSettings {
            speed: 500.0,
            range: 600.0,
            radius: 40.0,
        });
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn legacy_packs_receive_weapons_and_invalid_catalogs_are_rejected() {
        let manifest = project_content().manifest;
        let mut json = serde_json::to_value(&manifest).unwrap();
        json.as_object_mut().unwrap().remove("weapons");
        let mut loaded: Manifest = serde_json::from_value(json).unwrap();
        assert_eq!(loaded.weapons, default_weapons());
        loaded.validate().unwrap();
        loaded.weapons[0].reach = f32::NAN;
        assert!(loaded.validate().is_err());
        loaded.weapons = default_weapons();
        loaded.weapons[0].cooldown = 0.01;
        assert!(loaded.validate().is_err());
        loaded.weapons.clear();
        assert!(loaded.validate().is_err());
    }

    fn project_content() -> Content {
        Content::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")).unwrap()
    }

    fn temporary_directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "lumen-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn edited_png_and_wav_are_read_from_disk_without_embedded_fallback() {
        let root = temporary_directory("edited-assets");
        std::fs::create_dir_all(&root).unwrap();
        let content = Content {
            root: root.clone(),
            manifest: project_content().manifest,
        };
        let sprite = Sprite {
            size: [1, 1],
            frames: vec!["pixel.png".into()],
            fps: 1.0,
            walk: vec![0],
        };
        for rgba in [[255, 0, 0, 255], [0, 255, 0, 255]] {
            image::save_buffer_with_format(
                root.join("pixel.png"),
                &rgba,
                1,
                1,
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            )
            .unwrap();
            assert_eq!(content.image(&sprite, 0).unwrap().bytes, rgba);
        }
        for value in [1200_i16, -2400_i16] {
            let mut writer = hound::WavWriter::create(
                root.join("tone.wav"),
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 44_100,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            writer.write_sample(value).unwrap();
            writer.finalize().unwrap();
            let bytes = content.wav("tone.wav").unwrap();
            let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
            assert_eq!(reader.samples::<i16>().next().unwrap().unwrap(), value);
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn checked_in_pack_decodes_and_manifest_round_trips() {
        let content = project_content();
        content.validate_files().unwrap();
        let json = serde_json::to_vec(&content.manifest).unwrap();
        let restored: Manifest = serde_json::from_slice(&json).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.effects.len(), SoundCue::ALL.len());
        // Pack fourni : dix poses de joueur et cinq d'ennemi, en haute definition.
        assert_eq!(restored.sprites["player"].frames.len(), 10);
        assert_eq!(restored.sprites["enemy"].frames.len(), 5);
        assert_eq!(restored.sprites["player"].size, [32, 48]);
        assert_eq!(restored.sprites["enemy"].size, [32, 32]);
        // Quatre personnages jouables et leurs equivalents ennemis, haute definition.
        for character in &crate::characters::Character::ALL[1..] {
            let sprite = &restored.sprites[character.sprite()];
            assert_eq!((sprite.frames.len(), sprite.size), (10, [32, 48]));
        }
        for skin in &crate::characters::Skin::ALL[1..] {
            let sprite = &restored.sprites[skin.sprite()];
            assert_eq!((sprite.frames.len(), sprite.size), (5, [32, 32]));
        }
    }

    #[test]
    fn invalid_versions_references_and_animation_indices_are_rejected() {
        let mut manifest = project_content().manifest;
        manifest.version = 2;
        assert!(manifest.validate().is_err());
        manifest.version = 1;
        manifest.sprites.get_mut("player").unwrap().walk = vec![999];
        assert!(manifest.validate().is_err());
        manifest.sprites.get_mut("player").unwrap().walk = vec![0];
        manifest.effects.remove(&SoundCue::Jump);
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn missing_corrupt_and_mismatched_files_return_actionable_errors() {
        let content = project_content();
        assert!(content
            .read("missing.png", 1024)
            .unwrap_err()
            .contains("missing.png"));
        assert!(content
            .wav("sprites/player_0.png")
            .unwrap_err()
            .contains("player_0.png"));
        assert!(content
            .read(&content.manifest.music, 16)
            .unwrap_err()
            .contains("size limit"));
        let mut sprite = content.manifest.sprites["player"].clone();
        sprite.size = [32, 32];
        assert!(content.image(&sprite, 0).is_err());
        assert!(content.image(&sprite, 999).is_err());
    }

    #[test]
    fn invalid_manifest_does_not_replace_an_existing_file() {
        let temporary = temporary_directory("manifest");
        std::fs::create_dir_all(&temporary).unwrap();
        let path = temporary.join("manifest.json");
        let mut manifest = project_content().manifest;
        manifest.save(&path).unwrap();
        let original = std::fs::read(&path).unwrap();
        manifest.version = 99;
        assert!(manifest.save(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        manifest.version = 1;
        manifest.sprites.get_mut("player").unwrap().fps = 12.0;
        manifest.save(&path).unwrap();
        assert_eq!(
            Content::load(temporary.clone()).unwrap().manifest.sprites["player"].fps,
            12.0
        );
        std::fs::remove_dir_all(temporary).unwrap();
    }

    #[test]
    fn asset_paths_stay_relative_and_portable() {
        for name in [
            "",
            "../save.json",
            "/tmp/music.wav",
            "C:/music.wav",
            "sprites/../player.png",
            "sprites\\player.png",
        ] {
            assert!(relative_path(Path::new("assets"), name).is_err(), "{name}");
        }
        assert_eq!(
            relative_path(Path::new("assets"), "sprites/player.png").unwrap(),
            Path::new("assets").join("sprites/player.png")
        );
    }
}
