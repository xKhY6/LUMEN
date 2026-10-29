//! Lecture des WAV externes ; la synthese originale sert uniquement a l'export de reference.

use crate::content::{Content, Manifest};
use crate::game::SoundCue;
use macroquad::audio::{
    load_sound_from_bytes, play_sound, set_sound_volume, stop_sound, PlaySoundParams, Sound,
};
use std::f32::consts::TAU;
use std::io::Cursor;

const SAMPLE_RATE: u32 = 44_100;
const MUSIC_SECONDS: f32 = 24.0;

fn tone(frequency: f32, time: f32) -> f32 {
    (TAU * frequency * time).sin()
}

// Une enveloppe douce a chaque extremite evite les clics de debut et de fin.
fn envelope(time: f32, duration: f32, attack: f32, release: f32) -> f32 {
    (time / attack).clamp(0.0, 1.0) * ((duration - time) / release).clamp(0.0, 1.0)
}

fn music_sample(time: f32) -> f32 {
    let time = time.rem_euclid(MUSIC_SECONDS);
    let chords = [
        [110.0, 164.81, 220.0, 261.63],
        [87.31, 130.81, 174.61, 220.0],
        [130.81, 196.0, 261.63, 329.63],
        [98.0, 146.83, 196.0, 246.94],
    ];
    let mut sample = 0.0;
    for (index, chord) in chords.iter().enumerate() {
        // Les nappes se chevauchent, y compris a la jonction de la boucle de 24 secondes.
        let local = (time - index as f32 * 6.0).rem_euclid(MUSIC_SECONDS);
        if local < 8.0 {
            let gain = envelope(local, 8.0, 1.5, 2.0);
            for frequency in chord {
                sample += gain
                    * (tone(*frequency, local) * 0.024 + tone(*frequency * 1.002, local) * 0.016);
            }
            sample += gain * tone(chord[0] * 0.5, local) * 0.035;
        }
    }
    for (index, frequency) in [440.0, 523.25, 659.25, 587.33, 440.0, 392.0, 329.63, 392.0]
        .iter()
        .enumerate()
    {
        let local = (time - index as f32 * 3.0).rem_euclid(MUSIC_SECONDS);
        if local < 2.8 {
            let gain = envelope(local, 2.8, 0.02, 0.5) * (-local * 1.5).exp();
            sample +=
                gain * (tone(*frequency, local) + tone(*frequency * 2.0, local) * 0.25) * 0.07;
        }
    }
    sample
}

fn cue_duration(cue: SoundCue) -> f32 {
    match cue {
        SoundCue::Menu => 0.07,
        SoundCue::Land | SoundCue::Hit => 0.12,
        SoundCue::Jump | SoundCue::Attack => 0.18,
        SoundCue::Dash | SoundCue::DoubleJump => 0.25,
        SoundCue::Hurt => 0.3,
        SoundCue::Defeat => 0.4,
        SoundCue::Portal => 0.6,
        SoundCue::Death | SoundCue::Pickup => 0.9,
        SoundCue::Shrine => 1.2,
        SoundCue::Victory => 1.6,
    }
}

fn effect_sample(cue: SoundCue, time: f32) -> f32 {
    let duration = cue_duration(cue);
    let progress = time / duration;
    // Bruit deterministe : les memes evenements produisent les memes fichiers lors des tests.
    let index = (time * SAMPLE_RATE as f32) as u32;
    let hash = index.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    let noise = ((hash ^ (hash >> 16)) & 0xffff) as f32 / 32767.5 - 1.0;
    let (start, end) = match cue {
        SoundCue::Jump => (220.0, 700.0),
        SoundCue::DoubleJump => (440.0, 1100.0),
        SoundCue::Land => (90.0, 45.0),
        SoundCue::Attack => (500.0, 140.0),
        SoundCue::Dash => (1200.0, 200.0),
        SoundCue::Hurt => (160.0, 70.0),
        SoundCue::Death => (330.0, 45.0),
        SoundCue::Hit => (190.0, 90.0),
        SoundCue::Defeat => (240.0, 50.0),
        SoundCue::Portal => (180.0, 880.0),
        _ => (660.0, 660.0),
    };
    let sweep = (TAU * (start * time + (end - start) * time * time / (2.0 * duration))).sin();
    let signal = match cue {
        SoundCue::Attack | SoundCue::Dash => noise * 0.6 + sweep * 0.2,
        SoundCue::Land | SoundCue::Hit | SoundCue::Defeat => noise * 0.25 + sweep * 0.6,
        SoundCue::Hurt | SoundCue::Death => {
            sweep * 0.55 + noise * 0.2 + tone(start * 1.45, time) * 0.15
        }
        SoundCue::Pickup | SoundCue::Victory => {
            let notes = if cue == SoundCue::Pickup {
                [440.0, 554.37, 659.25, 880.0]
            } else {
                [261.63, 329.63, 392.0, 523.25]
            };
            let note = (progress * 4.0).floor().min(3.0) as usize;
            let local = time - note as f32 * duration / 4.0;
            tone(notes[note], local) * envelope(local, duration / 4.0, 0.01, 0.04) * 0.65
        }
        SoundCue::Shrine => (tone(220.0, time) + tone(330.0, time) + tone(440.0, time)) * 0.22,
        _ => sweep * 0.65 + tone(start * 2.0, time) * 0.12,
    };
    signal * envelope(time, duration, 0.006, 0.04) * (1.0 - progress).max(0.0)
}

fn wav(duration: f32, sample: impl Fn(f32) -> f32) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    {
        // Hound gere l'entete RIFF et les tailles ; aucune manipulation manuelle du format WAV.
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(&mut cursor, spec).expect("in-memory wav");
        for index in 0..(duration * SAMPLE_RATE as f32) as usize {
            let value = sample(index as f32 / SAMPLE_RATE as f32).clamp(-0.95, 0.95);
            writer
                .write_sample((value * i16::MAX as f32) as i16)
                .expect("wav sample");
        }
        writer.finalize().expect("wav header");
    }
    cursor.into_inner()
}

pub struct Audio {
    effects: Vec<(SoundCue, Sound)>,
    music: Option<Sound>,
    started: bool,
    music_gain: f32,
}

impl Audio {
    pub async fn load(content: &Content, disabled: bool) -> Result<Self, String> {
        let mut audio = Self {
            effects: vec![],
            music: None,
            started: false,
            music_gain: 0.0,
        };
        if disabled {
            return Ok(audio);
        }
        // Une reference invalide bloque le chargement, pour ne pas masquer une erreur d'edition.
        for cue in SoundCue::ALL {
            let path = &content.manifest.effects[&cue];
            let bytes = content.wav(path)?;
            let sound = load_sound_from_bytes(&bytes)
                .await
                .map_err(|error| format!("{path}: {error}"))?;
            audio.effects.push((cue, sound));
        }
        let path = &content.manifest.music;
        let bytes = content.wav(path)?;
        audio.music = Some(
            load_sound_from_bytes(&bytes)
                .await
                .map_err(|error| format!("{path}: {error}"))?,
        );
        Ok(audio)
    }

    pub fn export(
        root: &std::path::Path,
        manifest: &mut Manifest,
    ) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::create_dir_all(root.join("audio"))?;
        std::fs::write(root.join(&manifest.music), wav(MUSIC_SECONDS, music_sample))?;
        for cue in SoundCue::ALL {
            let name = serde_json::to_value(cue)?;
            let path = format!("audio/{}.wav", name.as_str().ok_or("invalid sound id")?);
            std::fs::write(
                root.join(&path),
                wav(cue_duration(cue), |time| effect_sample(cue, time)),
            )?;
            manifest.effects.insert(cue, path);
        }
        Ok(())
    }

    pub fn play(&self, cue: SoundCue, volume: f32) {
        if volume <= 0.0 {
            return;
        }
        if let Some((_, sound)) = self.effects.iter().find(|(kind, _)| *kind == cue) {
            play_sound(
                sound,
                PlaySoundParams {
                    looped: false,
                    volume,
                },
            );
        }
    }

    pub fn update(&mut self, volume: f32, quiet: bool, dt: f32) {
        let Some(music) = &self.music else { return };
        if !self.started {
            play_sound(
                music,
                PlaySoundParams {
                    looped: true,
                    volume: 0.0,
                },
            );
            self.started = true;
        }
        let target = volume * if quiet { 0.4 } else { 1.0 };
        // Les changements de menu et de volume se font par fondu, sans redemarrer la boucle.
        let next = self.music_gain + (target - self.music_gain).clamp(-dt * 0.6, dt * 0.6);
        if (next - self.music_gain).abs() > f32::EPSILON {
            self.music_gain = next;
            set_sound_volume(music, next);
        }
    }

    pub fn stop(&self) {
        if let Some(music) = &self.music {
            stop_sound(music);
        }
        for (_, sound) in &self.effects {
            stop_sound(sound);
        }
    }

    pub fn ready(&self) -> bool {
        self.music.is_some() && self.effects.len() == SoundCue::ALL.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_effects_are_valid_non_silent_pcm() {
        for cue in SoundCue::ALL {
            let bytes = wav(cue_duration(cue), |time| effect_sample(cue, time));
            let mut reader = hound::WavReader::new(Cursor::new(bytes)).unwrap();
            assert_eq!(reader.spec().sample_rate, SAMPLE_RATE);
            assert_eq!(reader.spec().bits_per_sample, 16);
            let samples: Vec<i16> = reader.samples().map(Result::unwrap).collect();
            assert_eq!(samples[0], 0);
            assert!(
                samples.iter().any(|value| value.abs() > 1000),
                "silent {cue:?}"
            );
            assert!(samples.iter().all(|value| value.abs() < i16::MAX));
        }
    }

    #[test]
    fn music_is_non_silent_and_loops_without_a_click() {
        assert_eq!(music_sample(0.0), music_sample(MUSIC_SECONDS));
        assert!(
            (music_sample(MUSIC_SECONDS - 1.0 / SAMPLE_RATE as f32) - music_sample(0.0)).abs()
                < 0.02
        );
        let bytes = wav(MUSIC_SECONDS, music_sample);
        let mut reader = hound::WavReader::new(Cursor::new(bytes)).unwrap();
        assert_eq!(
            reader.duration(),
            (MUSIC_SECONDS * SAMPLE_RATE as f32) as u32
        );
        let samples: Vec<i16> = reader.samples().map(Result::unwrap).collect();
        assert!(samples.iter().any(|value| value.abs() > 1000));
        assert!(samples.iter().all(|value| value.abs() < 20_000));
    }
}
