//! Edition native des donnees du pack ; aucune operation ne touche a la progression du joueur.

use crate::content::{relative_path, Content, Manifest, Sprite};
use crate::game::{Game, Input, Save, COLS, HEIGHT, NAMES, ROWS, STEP, TILE, WIDTH};
use crate::levels::{Levels, Room, Spawn};
use egui_macroquad::egui;
use macroquad::prelude::*;
use std::collections::{BTreeMap, VecDeque};
use std::io::Cursor;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Levels,
    Sprites,
    Audio,
    Weapons,
}

#[derive(Clone, Debug, PartialEq)]
struct Pixels {
    size: [u16; 2],
    bytes: Vec<u8>,
}

impl Pixels {
    fn paint(&mut self, column: usize, row: usize, rgba: [u8; 4]) {
        if column < self.size[0] as usize && row < self.size[1] as usize {
            let offset = (row * self.size[0] as usize + column) * 4;
            self.bytes[offset..offset + 4].copy_from_slice(&rgba);
        }
    }

    fn flood(&mut self, column: usize, row: usize, rgba: [u8; 4]) {
        if column >= self.size[0] as usize || row >= self.size[1] as usize {
            return;
        }
        let width = self.size[0] as usize;
        let offset = (row * width + column) * 4;
        let original: [u8; 4] = self.bytes[offset..offset + 4]
            .try_into()
            .expect("RGBA pixel");
        if original == rgba {
            return;
        }
        let mut queue = VecDeque::from([(column, row)]);
        self.paint(column, row, rgba);
        while let Some((horizontal, vertical)) = queue.pop_front() {
            for (next_column, next_row) in [
                (horizontal.wrapping_sub(1), vertical),
                (horizontal + 1, vertical),
                (horizontal, vertical.wrapping_sub(1)),
                (horizontal, vertical + 1),
            ] {
                if next_column >= width || next_row >= self.size[1] as usize {
                    continue;
                }
                let next = (next_row * width + next_column) * 4;
                if self.bytes[next..next + 4] == original {
                    self.paint(next_column, next_row, rgba);
                    queue.push_back((next_column, next_row));
                }
            }
        }
    }

    fn png(&self) -> Result<Vec<u8>, String> {
        let image = image::RgbaImage::from_raw(
            self.size[0] as u32,
            self.size[1] as u32,
            self.bytes.clone(),
        )
        .ok_or("Donnees RGBA invalides")?;
        let mut output = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut output, image::ImageOutputFormat::Png)
            .map_err(|error| error.to_string())?;
        Ok(output.into_inner())
    }
}

struct ImageEdit {
    pixels: Pixels,
    saved: Pixels,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct AudioSettings {
    start: f32,
    end: f32,
    gain: f32,
    fade_in: f32,
    fade_out: f32,
}

struct Clip {
    spec: hound::WavSpec,
    samples: Vec<i16>,
    settings: AudioSettings,
    imported: bool,
}

impl Clip {
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut reader =
            hound::WavReader::new(Cursor::new(bytes)).map_err(|error| error.to_string())?;
        let spec = reader.spec();
        if ![1, 2].contains(&spec.channels)
            || spec.bits_per_sample != 16
            || spec.sample_format != hound::SampleFormat::Int
            || !(8_000..=96_000).contains(&spec.sample_rate)
            || reader.duration() == 0
            || reader.duration() > spec.sample_rate * 300
        {
            return Err("WAV PCM16 mono/stereo requis, 8-96 kHz, duree maximale 300 s".into());
        }
        let samples = reader
            .samples::<i16>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        if !samples.len().is_multiple_of(spec.channels as usize) {
            return Err("WAV stereo incomplet".into());
        }
        let duration = samples.len() as f32 / spec.channels as f32 / spec.sample_rate as f32;
        Ok(Self {
            spec,
            samples,
            settings: Self::defaults(duration),
            imported: false,
        })
    }

    fn defaults(duration: f32) -> AudioSettings {
        AudioSettings {
            start: 0.0,
            end: duration,
            gain: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
        }
    }
    fn duration(&self) -> f32 {
        self.samples.len() as f32 / self.spec.channels as f32 / self.spec.sample_rate as f32
    }
    fn dirty(&self) -> bool {
        self.imported || self.settings != Self::defaults(self.duration())
    }

    fn render(&self) -> Result<Vec<u8>, String> {
        let settings = self.settings;
        if [
            settings.start,
            settings.end,
            settings.gain,
            settings.fade_in,
            settings.fade_out,
        ]
        .iter()
        .any(|value| !value.is_finite())
            || settings.start < 0.0
            || settings.end > self.duration()
            || settings.end <= settings.start
            || !(0.0..=2.0).contains(&settings.gain)
            || settings.fade_in < 0.0
            || settings.fade_out < 0.0
        {
            return Err("Selection ou gain audio invalide".into());
        }
        let channels = self.spec.channels as usize;
        let start = (settings.start * self.spec.sample_rate as f32).round() as usize;
        let end = ((settings.end * self.spec.sample_rate as f32).round() as usize)
            .min(self.samples.len() / channels);
        if end <= start {
            return Err("La selection audio est vide".into());
        }
        let frames = end - start;
        let mut output = Cursor::new(Vec::new());
        {
            let mut writer =
                hound::WavWriter::new(&mut output, self.spec).map_err(|error| error.to_string())?;
            // Les fondus portent sur les frames audio, pas sur les canaux : conserver l'image stereo.
            for frame in 0..frames {
                let fade_in = if settings.fade_in > 0.0 {
                    (frame as f32 / (settings.fade_in * self.spec.sample_rate as f32)).min(1.0)
                } else {
                    1.0
                };
                let fade_out = if settings.fade_out > 0.0 {
                    ((frames - 1 - frame) as f32
                        / (settings.fade_out * self.spec.sample_rate as f32))
                        .min(1.0)
                } else {
                    1.0
                };
                for channel in 0..channels {
                    let value = self.samples[(start + frame) * channels + channel] as f32
                        * settings.gain
                        * fade_in
                        * fade_out;
                    writer
                        .write_sample(value.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16)
                        .map_err(|error| error.to_string())?;
                }
            }
            writer.finalize().map_err(|error| error.to_string())?;
        }
        Ok(output.into_inner())
    }
}

enum Change {
    Portal {
        index: usize,
        before: crate::levels::Portal,
        after: crate::levels::Portal,
    },
    Biome {
        index: usize,
        before: crate::levels::Biome,
        after: crate::levels::Biome,
    },
    Weapons {
        before: Vec<crate::content::Weapon>,
        after: Vec<crate::content::Weapon>,
    },
    Levels {
        before: Levels,
        after: Levels,
    },
    Room {
        index: usize,
        before: Room,
        after: Room,
    },
    Pixels {
        path: String,
        before: Pixels,
        after: Pixels,
    },
    Animation {
        name: String,
        before: Sprite,
        after: Sprite,
    },
}

impl Change {
    fn cost(&self) -> usize {
        match self {
            Self::Levels { before, after } => (before.rooms.len() + after.rooms.len()) * 16 * 1024,
            Self::Pixels { before, after, .. } => before.bytes.len() + after.bytes.len(),
            _ => 16 * 1024,
        }
    }
}

struct Document {
    content: Content,
    levels: Levels,
    saved_levels: Levels,
    saved_manifest: Manifest,
    images: BTreeMap<String, ImageEdit>,
    clips: BTreeMap<String, Clip>,
    undo: VecDeque<Change>,
    redo: Vec<Change>,
}

impl Document {
    fn load(root: &Path) -> Result<Self, String> {
        let content = Content::load(root.canonicalize().map_err(|error| error.to_string())?)?;
        let levels = Levels::load(&content.root)?;
        Ok(Self {
            saved_manifest: content.manifest.clone(),
            content,
            saved_levels: levels.clone(),
            levels,
            images: BTreeMap::new(),
            clips: BTreeMap::new(),
            undo: VecDeque::new(),
            redo: Vec::new(),
        })
    }

    fn dirty(&self) -> bool {
        self.levels != self.saved_levels
            || self.content.manifest != self.saved_manifest
            || self
                .images
                .values()
                .any(|image| image.pixels != image.saved)
            || self.clips.values().any(Clip::dirty)
    }

    fn image(&mut self, name: &str, frame: usize) -> Result<String, String> {
        let sprite = &self.content.manifest.sprites[name];
        let path = sprite.frames.get(frame).ok_or("Frame introuvable")?.clone();
        if !self.images.contains_key(&path) {
            let image = self.content.image(sprite, frame)?;
            let pixels = Pixels {
                size: [image.width, image.height],
                bytes: image.bytes,
            };
            self.images.insert(
                path.clone(),
                ImageEdit {
                    saved: pixels.clone(),
                    pixels,
                },
            );
        }
        Ok(path)
    }

    fn clip(&mut self, path: &str) -> Result<(), String> {
        if !self.clips.contains_key(path) {
            let bytes = self.content.wav(path)?;
            self.clips.insert(path.to_string(), Clip::decode(&bytes)?);
        }
        Ok(())
    }

    fn record(&mut self, change: Change) {
        self.redo.clear();
        self.undo.push_back(change);
        // Borner l'historique meme pour les grandes textures importees.
        while self.undo.len() > 128
            || self.undo.iter().map(Change::cost).sum::<usize>() > 32 * 1024 * 1024
        {
            self.undo.pop_front();
        }
    }

    fn apply(&mut self, change: &Change, forward: bool) {
        match change {
            Change::Portal {
                index,
                before,
                after,
            } => self.levels.portals[*index] = if forward { after } else { before }.clone(),
            Change::Biome {
                index,
                before,
                after,
            } => self.levels.biomes[*index] = if forward { after } else { before }.clone(),
            Change::Weapons { before, after } => {
                self.content.manifest.weapons = if forward { after } else { before }.clone();
            }
            Change::Levels { before, after } => {
                self.levels = if forward { after } else { before }.clone();
            }
            Change::Room {
                index,
                before,
                after,
            } => self.levels.rooms[*index] = if forward { after } else { before }.clone(),
            Change::Pixels {
                path,
                before,
                after,
            } => {
                self.images.get_mut(path).expect("cached image").pixels =
                    if forward { after } else { before }.clone()
            }
            Change::Animation {
                name,
                before,
                after,
            } => {
                self.content
                    .manifest
                    .sprites
                    .insert(name.clone(), if forward { after } else { before }.clone());
            }
        }
    }

    fn undo(&mut self) {
        if let Some(change) = self.undo.pop_back() {
            self.apply(&change, false);
            self.redo.push(change);
        }
    }
    fn redo(&mut self) {
        if let Some(change) = self.redo.pop() {
            self.apply(&change, true);
            self.undo.push_back(change);
        }
    }

    fn save(&mut self, tab: Tab, isolated: bool) -> Result<(), String> {
        if isolated {
            return Err("Enregistrement desactive en capture".into());
        }
        match tab {
            Tab::Levels => {
                self.levels.save(&self.content.root)?;
                self.saved_levels = self.levels.clone();
            }
            Tab::Sprites => {
                self.content.manifest.validate()?;
                for (path, image) in &mut self.images {
                    if image.pixels != image.saved {
                        write_asset(&self.content.root, path, &image.pixels.png()?)?;
                        image.saved = image.pixels.clone();
                    }
                }
                // Sauver l'onglet actif sans valider implicitement les retouches d'un autre onglet.
                let mut manifest = self.content.manifest.clone();
                manifest.weapons = self.saved_manifest.weapons.clone();
                manifest.save(&self.content.root.join("manifest.json"))?;
                self.saved_manifest = manifest;
            }
            Tab::Weapons => {
                let mut manifest = self.saved_manifest.clone();
                manifest.weapons = self.content.manifest.weapons.clone();
                manifest.save(&self.content.root.join("manifest.json"))?;
                self.saved_manifest = manifest;
            }
            Tab::Audio => {
                // Valider toutes les retouches avant de commencer a remplacer les WAV.
                let rendered = self
                    .clips
                    .iter()
                    .filter(|(_, clip)| clip.dirty())
                    .map(|(path, clip)| clip.render().map(|bytes| (path.clone(), bytes)))
                    .collect::<Result<Vec<_>, _>>()?;
                for (path, bytes) in rendered {
                    write_asset(&self.content.root, &path, &bytes)?;
                    self.clips.insert(path, Clip::decode(&bytes)?);
                }
            }
        }
        Ok(())
    }

    fn save_all(&mut self, isolated: bool) -> Result<(), String> {
        for tab in [Tab::Levels, Tab::Sprites, Tab::Audio, Tab::Weapons] {
            self.save(tab, isolated)?;
        }
        Ok(())
    }
}

fn write_asset(root: &Path, path: &str, bytes: &[u8]) -> Result<(), String> {
    let destination = relative_path(root, path)?;
    let temporary = destination.with_extension("editor.tmp");
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("{}: {error}", temporary.display()))?;
    std::fs::rename(&temporary, &destination)
        .map_err(|error| format!("{}: {error}", destination.display()))
}

#[derive(Clone, Copy, PartialEq)]
enum LevelTool {
    Solid,
    Erase,
    Enemy,
    Hazard,
    Shrine,
    Spawn,
}

fn paint_room(room: &mut Room, column: usize, row: usize, tool: LevelTool, boss: bool) {
    if column >= COLS || row >= ROWS {
        return;
    }
    let horizontal = column as f32 * TILE;
    let vertical = row as f32 * TILE;
    let in_cell = |point: [f32; 2]| {
        point[0] >= horizontal
            && point[0] < horizontal + TILE
            && point[1] >= vertical
            && point[1] < vertical + TILE
    };
    match tool {
        LevelTool::Solid => room.set_tile(column, row, true),
        LevelTool::Erase => {
            room.set_tile(column, row, false);
            room.enemies.retain(|enemy| !in_cell(enemy.position));
            room.hazards.retain(|rect| !in_cell([rect[0], rect[1]]));
        }
        LevelTool::Enemy => {
            if !room.enemies.iter().any(|enemy| in_cell(enemy.position)) && room.enemies.len() < 64
            {
                room.enemies.push(Spawn {
                    position: [
                        horizontal.min(WIDTH - if boss { 46.0 } else { 26.0 }),
                        vertical.min(HEIGHT - if boss { 60.0 } else { 28.0 }),
                    ],
                    boss,
                    species: room.biome.unwrap_or(0) as u8 * 3,
                    behavior: room.biome.map_or(crate::levels::Behavior::Legacy, |biome| {
                        crate::levels::Behavior::ALL[biome + 1]
                    }),
                    skin: 0,
                });
            }
        }
        LevelTool::Hazard => {
            if !room.hazards.iter().any(|rect| in_cell([rect[0], rect[1]])) {
                room.hazards.push([horizontal, vertical + 22.0, TILE, 10.0]);
            }
        }
        LevelTool::Shrine => room.shrine = Some([horizontal + 16.0, vertical + TILE]),
        LevelTool::Spawn => {
            room.spawn = [horizontal.min(WIDTH - 22.0), vertical.min(HEIGHT - 34.0)]
        }
    }
}

enum Stroke {
    Room(usize, Room),
    Pixels(String, Pixels),
}

#[derive(Clone, Copy, PartialEq)]
enum PixelTool {
    Brush,
    Erase,
    Fill,
    Pick,
}

struct CachedTexture {
    handle: egui::TextureHandle,
    pixels: Pixels,
}

struct Workspace {
    document: Document,
    tab: Tab,
    room: usize,
    sprite: String,
    frame: usize,
    level_tool: LevelTool,
    pixel_tool: PixelTool,
    boss: bool,
    grid: bool,
    generation_seed: u32,
    generation_difficulty: usize,
    color: [u8; 4],
    animation: bool,
    audio_index: usize,
    weapon_index: usize,
    loop_audio: bool,
    textures: BTreeMap<String, CachedTexture>,
    stroke: Option<Stroke>,
    last_cell: Option<(usize, usize)>,
    notice: String,
    preview: Option<Vec<u8>>,
    stop_audio: bool,
    request_test: bool,
    request_exit: bool,
    confirm_exit: bool,
    close_game: bool,
    isolated: bool,
}

impl Workspace {
    fn new(root: &Path, tab: Tab, room: usize, isolated: bool) -> Result<Self, String> {
        let document = Document::load(root)?;
        let room = room.min(document.levels.rooms.len() - 1);
        Ok(Self {
            document,
            tab,
            room,
            sprite: "player".into(),
            frame: 0,
            level_tool: LevelTool::Solid,
            pixel_tool: PixelTool::Brush,
            boss: false,
            grid: true,
            generation_seed: 1,
            generation_difficulty: 1,
            color: [97, 226, 198, 255],
            animation: false,
            audio_index: 0,
            weapon_index: 0,
            loop_audio: true,
            textures: BTreeMap::new(),
            stroke: None,
            last_cell: None,
            notice: String::new(),
            preview: None,
            stop_audio: false,
            request_test: false,
            request_exit: false,
            confirm_exit: false,
            close_game: false,
            isolated,
        })
    }

    fn generate_level(&mut self, play: bool) {
        self.finish_stroke();
        let before = self.document.levels.clone();
        match self.document.levels.generate(
            self.room,
            self.generation_seed,
            self.generation_difficulty,
        ) {
            Ok(index) => {
                let source_name = before.name(self.room);
                let after = self.document.levels.clone();
                self.document.record(Change::Levels { before, after });
                self.room = index;
                self.notice = format!("Salle creee et reliee a {source_name}");
                self.request_test = play;
                self.generation_seed = self.generation_seed.wrapping_add(1);
            }
            Err(error) => self.notice = error,
        }
    }

    fn finish_stroke(&mut self) {
        if let Some(stroke) = self.stroke.take() {
            match stroke {
                Stroke::Room(index, before) => {
                    let after = self.document.levels.rooms[index].clone();
                    if before != after {
                        self.document.record(Change::Room {
                            index,
                            before,
                            after,
                        });
                    }
                }
                Stroke::Pixels(path, before) => {
                    let after = self.document.images[&path].pixels.clone();
                    if before != after {
                        self.document.record(Change::Pixels {
                            path,
                            before,
                            after,
                        });
                    }
                }
            }
        }
        self.last_cell = None;
    }

    fn texture(
        &mut self,
        context: &egui::Context,
        name: &str,
        frame: usize,
    ) -> Result<egui::TextureId, String> {
        let path = self.document.image(name, frame)?;
        let pixels = &self.document.images[&path].pixels;
        if let Some(cached) = self.textures.get_mut(&path) {
            if cached.pixels != *pixels {
                cached.handle.set(
                    egui::ColorImage::from_rgba_unmultiplied(
                        [pixels.size[0] as usize, pixels.size[1] as usize],
                        &pixels.bytes,
                    ),
                    egui::TextureOptions::NEAREST,
                );
                cached.pixels = pixels.clone();
            }
            return Ok(cached.handle.id());
        }
        let handle = context.load_texture(
            &path,
            egui::ColorImage::from_rgba_unmultiplied(
                [pixels.size[0] as usize, pixels.size[1] as usize],
                &pixels.bytes,
            ),
            egui::TextureOptions::NEAREST,
        );
        let id = handle.id();
        self.textures.insert(
            path,
            CachedTexture {
                handle,
                pixels: pixels.clone(),
            },
        );
        Ok(id)
    }

    fn save(&mut self, all: bool) -> bool {
        self.finish_stroke();
        let result = if all {
            self.document.save_all(self.isolated)
        } else {
            self.document.save(self.tab, self.isolated)
        };
        match result {
            Ok(()) => {
                self.notice = "Enregistre".into();
                true
            }
            Err(error) => {
                self.notice = error;
                false
            }
        }
    }

    fn ui(&mut self, context: &egui::Context) {
        // Annuler une generation retire sa salle ; borner la selection avant tout affichage.
        self.room = self.room.min(self.document.levels.rooms.len() - 1);
        self.weapon_index = self
            .weapon_index
            .min(self.document.content.manifest.weapons.len() - 1);
        if !context.input(|input| input.pointer.any_down()) {
            self.finish_stroke();
        }
        // Une confirmation est exclusive : ne pas modifier le document derriere le dialogue.
        if !self.confirm_exit && !context.wants_keyboard_input() {
            if context.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::Z)) {
                self.finish_stroke();
                self.document.undo();
            }
            if context.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::Y)) {
                self.document.redo();
            }
            if context.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::S)) {
                self.save(false);
            }
        }
        egui::TopBottomPanel::top("editor_toolbar").show(context, |ui| {
            if self.confirm_exit {
                ui.disable();
            }
            ui.horizontal_wrapped(|ui| {
                ui.strong("LUMEN / ATELIER");
                ui.separator();
                for (tab, label) in [
                    (Tab::Levels, "Niveaux"),
                    (Tab::Sprites, "Sprites"),
                    (Tab::Audio, "Audio"),
                    (Tab::Weapons, "Armes"),
                ] {
                    if ui.selectable_label(self.tab == tab, label).clicked() {
                        self.finish_stroke();
                        self.tab = tab;
                        self.stop_audio = true;
                    }
                }
                ui.separator();
                if ui
                    .add_enabled(
                        !self.document.undo.is_empty() && self.tab != Tab::Audio,
                        egui::Button::new("<"),
                    )
                    .on_hover_text("Annuler (Ctrl+Z)")
                    .clicked()
                {
                    self.document.undo();
                }
                if ui
                    .add_enabled(
                        !self.document.redo.is_empty() && self.tab != Tab::Audio,
                        egui::Button::new(">"),
                    )
                    .on_hover_text("Retablir (Ctrl+Y)")
                    .clicked()
                {
                    self.document.redo();
                }
                if ui
                    .add_enabled(!self.isolated, egui::Button::new("Enregistrer"))
                    .on_hover_text("Enregistrer l'onglet actif (Ctrl+S)")
                    .clicked()
                {
                    self.save(false);
                }
                if ui.button("Tester").clicked() {
                    self.finish_stroke();
                    self.request_test = true;
                }
                if ui.button("Retour").clicked() {
                    self.finish_stroke();
                    self.request_exit = true;
                }
                if self.document.dirty() {
                    ui.colored_label(egui::Color32::from_rgb(244, 193, 113), "Modifie");
                }
            });
        });
        egui::TopBottomPanel::bottom("editor_status").show(context, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::Label::new(self.document.content.root.display().to_string()).truncate(),
                )
                .on_hover_text(self.document.content.root.display().to_string());
                ui.separator();
                ui.label(&self.notice);
            });
        });
        let side_width = if screen_width() < 1000.0 {
            175.0
        } else {
            225.0
        };
        egui::SidePanel::left("editor_properties")
            .exact_width(side_width)
            .resizable(false)
            .show(context, |ui| {
                if self.confirm_exit {
                    ui.disable();
                }
                egui::ScrollArea::vertical().show(ui, |ui| match self.tab {
                    Tab::Levels => self.level_properties(ui),
                    Tab::Sprites => self.sprite_properties(ui),
                    Tab::Audio => self.audio_properties(ui),
                    Tab::Weapons => self.weapon_properties(ui),
                });
            });
        egui::CentralPanel::default().show(context, |ui| {
            if self.confirm_exit {
                ui.disable();
            }
            let result = match self.tab {
                Tab::Levels => self.level_canvas(ui),
                Tab::Sprites => self.sprite_canvas(ui),
                Tab::Audio => self.audio_canvas(ui),
                Tab::Weapons => self.weapon_canvas(ui),
            };
            if let Err(error) = result {
                self.notice = error;
            }
        });
    }

    fn replace_weapons(&mut self, after: Vec<crate::content::Weapon>) {
        let before = self.document.content.manifest.weapons.clone();
        let mut manifest = self.document.content.manifest.clone();
        manifest.weapons = after.clone();
        match manifest.validate() {
            Ok(()) if before != after => {
                self.document.content.manifest.weapons = after.clone();
                self.document.record(Change::Weapons { before, after });
            }
            Ok(()) => {}
            Err(error) => self.notice = error,
        }
    }

    fn weapon_properties(&mut self, ui: &mut egui::Ui) {
        ui.heading("Armes");
        let before = self.document.content.manifest.weapons.clone();
        // Le catalogue ne doit pas repousser les reglages hors de la fenetre compacte.
        egui::ScrollArea::vertical()
            .id_salt("weapon_catalog")
            .max_height(160.0)
            .show(ui, |ui| {
                for (index, weapon) in before.iter().enumerate() {
                    if ui
                        .selectable_label(self.weapon_index == index, &weapon.name)
                        .clicked()
                    {
                        self.weapon_index = index;
                    }
                }
            });
        ui.separator();
        let mut create = false;
        let mut duplicate = false;
        ui.horizontal_wrapped(|ui| {
            create = ui
                .add_enabled(before.len() < 32, egui::Button::new("+"))
                .on_hover_text("Creer une arme")
                .clicked();
            duplicate = ui
                .add_enabled(before.len() < 32, egui::Button::new("Dupliquer"))
                .clicked();
        });
        if create || duplicate {
            let mut after = before.clone();
            let mut weapon = if duplicate {
                before[self.weapon_index].clone()
            } else {
                crate::content::default_weapons()[0].clone()
            };
            let mut number = 1;
            while after
                .iter()
                .any(|entry| entry.name == format!("Arme {number}"))
            {
                number += 1;
            }
            weapon.name = format!("Arme {number}");
            after.push(weapon);
            self.replace_weapons(after);
            self.weapon_index = before.len();
            return;
        }
        if ui
            .add_enabled(before.len() > 1, egui::Button::new("Supprimer"))
            .clicked()
        {
            let mut after = before.clone();
            after.remove(self.weapon_index);
            self.weapon_index = self.weapon_index.min(after.len() - 1);
            self.replace_weapons(after);
            return;
        }
        ui.separator();
        let mut after = before.clone();
        let weapon = &mut after[self.weapon_index];
        ui.label("Nom");
        ui.add(egui::TextEdit::singleline(&mut weapon.name).char_limit(32));
        let mut ranged = weapon.projectile.is_some();
        if ui.checkbox(&mut ranged, "Distance").changed() {
            if !ranged {
                weapon.automatic = false;
                weapon.laser = false;
            }
            weapon.projectile = ranged.then_some(crate::content::ProjectileSettings {
                speed: 520.0,
                range: 600.0,
                radius: 3.0,
            });
        }
        if ranged {
            ui.checkbox(&mut weapon.automatic, "Automatique");
            ui.checkbox(&mut weapon.laser, "Laser");
        }
        ui.label("Deblocage (ennemis)")
            .on_hover_text("0 : disponible des le depart");
        ui.add(egui::Slider::new(&mut weapon.unlock_kills, 0..=200));
        // Separer les libelles des curseurs pour garder la colonne compacte lisible.
        ui.label("Degats");
        ui.add(egui::Slider::new(&mut weapon.damage, 1..=20));
        if let Some(projectile) = &mut weapon.projectile {
            ui.label("Vitesse (px/s)");
            ui.add(egui::Slider::new(&mut projectile.speed, 100.0..=1200.0));
            ui.label("Portee du tir (px)");
            ui.add(egui::Slider::new(&mut projectile.range, 64.0..=1200.0));
            ui.label("Rayon du tir (px)");
            ui.add(egui::Slider::new(&mut projectile.radius, 2.0..=12.0));
        } else {
            ui.label("Portee (px)");
            ui.add(egui::Slider::new(&mut weapon.reach, 16.0..=160.0));
            ui.label("Hauteur (px)");
            ui.add(egui::Slider::new(&mut weapon.height, 16.0..=100.0));
        }
        ui.label("Frappe (s)");
        ui.add(egui::Slider::new(&mut weapon.duration, 0.05..=0.5));
        // La recharge doit depasser la frappe, y compris apres avoir allonge celle-ci.
        weapon.cooldown = weapon.cooldown.max(weapon.duration + 0.02);
        ui.label("Recharge (s)");
        ui.add(egui::Slider::new(
            &mut weapon.cooldown,
            weapon.duration + 0.02..=2.0,
        ));
        ui.color_edit_button_srgb(&mut weapon.color);
        if after != before {
            self.replace_weapons(after);
        }
    }

    fn weapon_canvas(&mut self, ui: &mut egui::Ui) -> Result<(), String> {
        let weapon = &self.document.content.manifest.weapons[self.weapon_index];
        ui.heading(&weapon.name);
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 260.0),
            egui::Sense::hover(),
        );
        let painter = ui.painter_at(rect);
        if let Some(projectile) = &weapon.projectile {
            let color = egui::Color32::from_rgb(weapon.color[0], weapon.color[1], weapon.color[2]);
            let length = projectile.range.min(rect.width() - 40.0);
            let start = rect.center() - egui::vec2(length * 0.5, 0.0);
            let phase = (get_time() as f32 * projectile.speed / projectile.range).fract();
            let center = start + egui::vec2(length * phase, 0.0);
            painter.line_segment(
                [start, start + egui::vec2(length, 0.0)],
                egui::Stroke::new(1.0_f32, color.gamma_multiply(0.3)),
            );
            let tail = center - egui::vec2(if weapon.laser { 44.0 } else { 20.0 }, 0.0);
            if weapon.laser {
                painter.line_segment(
                    [tail, center],
                    egui::Stroke::new(9.0_f32, color.gamma_multiply(0.22)),
                );
            }
            painter.line_segment([tail, center], egui::Stroke::new(3.0_f32, color));
            if weapon.laser {
                painter.line_segment(
                    [tail, center],
                    egui::Stroke::new(1.5_f32, egui::Color32::WHITE),
                );
            }
            painter.circle_filled(center, projectile.radius, color);
            ui.ctx().request_repaint();
            return Ok(());
        }
        let origin = rect.center() - egui::vec2(weapon.reach, 0.0);
        let tip = origin + egui::vec2(weapon.reach * 2.0, 0.0);
        let color = egui::Color32::from_rgb(weapon.color[0], weapon.color[1], weapon.color[2]);
        let bounds = egui::Rect::from_center_size(
            rect.center(),
            egui::vec2(weapon.reach * 2.0, weapon.height * 2.0),
        );
        painter.rect_filled(bounds, 0.0, color.gamma_multiply(0.12));
        painter.line_segment([origin, tip], egui::Stroke::new(6.0_f32, color));
        if weapon.damage >= 3 {
            painter.rect_filled(
                egui::Rect::from_center_size(
                    tip - egui::vec2(12.0, 0.0),
                    egui::vec2(24.0, weapon.height),
                ),
                2.0,
                color,
            );
        } else {
            painter.line_segment(
                [tip - egui::vec2(18.0, 12.0), tip],
                egui::Stroke::new(4.0_f32, color),
            );
            painter.line_segment(
                [tip - egui::vec2(18.0, -12.0), tip],
                egui::Stroke::new(4.0_f32, color),
            );
        }
        Ok(())
    }

    fn level_properties(&mut self, ui: &mut egui::Ui) {
        self.room = self.room.min(self.document.levels.rooms.len() - 1);
        let current = self.document.levels.rooms[self.room].biome;
        let mut selected = current;
        egui::ComboBox::from_id_salt("biome_select")
            .selected_text(current.map_or("Citadelle / extensions", |index| {
                self.document.levels.biomes[index].name.as_str()
            }))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut selected, None, "Citadelle / extensions");
                for (index, biome) in self.document.levels.biomes.iter().enumerate() {
                    ui.selectable_value(&mut selected, Some(index), &biome.name);
                }
            });
        if selected != current {
            self.finish_stroke();
            if let Some(index) = self
                .document
                .levels
                .rooms
                .iter()
                .position(|room| room.biome == selected)
            {
                self.room = index;
            }
        }
        ui.heading("Salle");
        egui::ComboBox::from_id_salt("room_select")
            .selected_text(self.document.levels.name(self.room))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for index in 0..self.document.levels.rooms.len() {
                    if self.document.levels.rooms[index].biome != selected {
                        continue;
                    }
                    if ui
                        .selectable_value(&mut self.room, index, self.document.levels.name(index))
                        .changed()
                    {
                        self.finish_stroke();
                    }
                }
            });
        if let Some(index) = selected {
            let before = self.document.levels.biomes[index].clone();
            egui::CollapsingHeader::new("Biome et bestiaire").show(ui, |ui| {
                let biome = &mut self.document.levels.biomes[index];
                ui.label("Nom du niveau");
                ui.add(egui::TextEdit::singleline(&mut biome.name).char_limit(64));
                ui.label("Boss");
                ui.add(egui::TextEdit::singleline(&mut biome.boss).char_limit(64));
                for name in &mut biome.bestiary {
                    ui.add(egui::TextEdit::singleline(name).char_limit(64));
                }
                for (label, color) in [
                    ("Fond", &mut biome.background),
                    ("Architecture", &mut biome.stone),
                    ("Flore", &mut biome.accent),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        ui.color_edit_button_srgb(color);
                    });
                }
                egui::ComboBox::from_id_salt("flora_select")
                    .selected_text(format!("Flore {}", biome.flora + 1))
                    .show_ui(ui, |ui| {
                        for (flora, name) in [
                            "Fougeres",
                            "Lichens",
                            "Braises",
                            "Coraux",
                            "Cristaux",
                            "Fleurs d'ambre",
                            "Roseaux",
                            "Champignons",
                            "Ronces",
                            "Fleurs solaires",
                        ]
                        .iter()
                        .enumerate()
                        {
                            ui.selectable_value(&mut biome.flora, flora as u8, *name);
                        }
                    });
                egui::ComboBox::from_id_salt("architecture_select")
                    .selected_text(format!("Architecture {}", biome.architecture + 1))
                    .show_ui(ui, |ui| {
                        for (architecture, name) in [
                            "Foret",
                            "Cryptes",
                            "Fonderie",
                            "Anneaux coralliens",
                            "Cimes de verre",
                            "Ruche",
                            "Observatoire",
                            "Mycelium",
                            "Citadelle",
                            "Sanctuaire",
                        ]
                        .iter()
                        .enumerate()
                        {
                            ui.selectable_value(&mut biome.architecture, architecture as u8, *name);
                        }
                    });
            });
            let after = self.document.levels.biomes[index].clone();
            if before != after {
                self.document.record(Change::Biome {
                    index,
                    before,
                    after,
                });
            }
        }
        ui.separator();
        egui::CollapsingHeader::new("Generation procedurale")
            .default_open(true)
            .show(ui, |ui| {
                ui.add(egui::DragValue::new(&mut self.generation_seed).prefix("Graine "));
                ui.label("Difficulte");
                ui.add(egui::Slider::new(&mut self.generation_difficulty, 1..=3));
                let available = self.document.levels.can_extend(self.room);
                let generate = ui
                    .add_enabled(available, egui::Button::new("Generer"))
                    .clicked();
                let play = ui
                    .add_enabled(available, egui::Button::new("Generer et jouer"))
                    .clicked();
                if generate || play {
                    self.generate_level(play);
                }
                for portal in &self.document.levels.portals {
                    if portal.from == self.room || portal.to == self.room {
                        let target = if portal.from == self.room {
                            portal.to
                        } else {
                            portal.from
                        };
                        ui.label(format!("Passage : {}", self.document.levels.name(target)));
                    }
                }
            });
        ui.separator();
        ui.heading("Outil");
        for (tool, label) in [
            (LevelTool::Solid, "Sol"),
            (LevelTool::Erase, "Gomme"),
            (LevelTool::Enemy, "Ennemi"),
            (LevelTool::Hazard, "Piques"),
            (LevelTool::Shrine, "Sanctuaire"),
            (LevelTool::Spawn, "Depart du test"),
        ] {
            ui.radio_value(&mut self.level_tool, tool, label);
        }
        if self.level_tool == LevelTool::Enemy {
            ui.checkbox(&mut self.boss, "Gardien");
        }
        ui.checkbox(&mut self.grid, "Grille");
        egui::CollapsingHeader::new("Portails").show(ui, |ui| {
            let mut changes = Vec::new();
            let rooms = &self.document.levels.rooms;
            for (index, portal) in self.document.levels.portals.iter_mut().enumerate() {
                if portal.from != self.room && portal.to != self.room {
                    continue;
                }
                let before = portal.clone();
                ui.push_id(("portal", index), |ui| {
                    ui.label(format!("Salle {} -> {}", portal.from + 1, portal.to + 1));
                    let point = if portal.from == self.room {
                        &mut portal.entrance
                    } else {
                        &mut portal.exit
                    };
                    ui.add(
                        egui::DragValue::new(&mut point[0])
                            .range(0.0..=WIDTH - 22.0)
                            .prefix("X "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut point[1])
                            .range(0.0..=HEIGHT - 34.0)
                            .prefix("Y "),
                    );
                    let mut gated = portal.requires_boss.is_some();
                    let boss = rooms[portal.from].enemies.iter().any(|enemy| enemy.boss);
                    if ui
                        .add_enabled(
                            boss,
                            egui::Checkbox::new(&mut gated, "Exiger la victoire du boss"),
                        )
                        .changed()
                    {
                        portal.requires_boss = gated.then_some(portal.from);
                    }
                });
                if *portal != before {
                    changes.push(Change::Portal {
                        index,
                        before,
                        after: portal.clone(),
                    });
                }
            }
            for change in changes {
                self.document.record(change);
            }
        });
        ui.separator();
        let before = self.document.levels.rooms[self.room].clone();
        egui::CollapsingHeader::new("Objets").show(ui, |ui| {
            let mut remove = None;
            for (index, enemy) in self.document.levels.rooms[self.room]
                .enemies
                .iter_mut()
                .enumerate()
            {
                ui.push_id(index, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("#{}", index + 1));
                        if ui
                            .button("\u{00d7}")
                            .on_hover_text("Supprimer cet ennemi")
                            .clicked()
                        {
                            remove = Some(index);
                        }
                    });
                    ui.add(
                        egui::DragValue::new(&mut enemy.position[0])
                            .range(0.0..=WIDTH - 46.0)
                            .prefix("X "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut enemy.position[1])
                            .range(0.0..=HEIGHT - 60.0)
                            .prefix("Y "),
                    );
                    ui.checkbox(&mut enemy.boss, "Gardien");
                    let species_names =
                        selected.map(|index| self.document.levels.biomes[index].bestiary.clone());
                    if let Some(names) = species_names {
                        let base = selected.unwrap() as u8 * 3;
                        egui::ComboBox::from_id_salt(("species", index))
                            .selected_text(&names[enemy.species as usize % 3])
                            .show_ui(ui, |ui| {
                                for (species, name) in names.iter().enumerate() {
                                    ui.selectable_value(
                                        &mut enemy.species,
                                        base + species as u8,
                                        name,
                                    );
                                }
                            });
                    }
                    egui::ComboBox::from_id_salt(("skin", index))
                        .selected_text(
                            crate::characters::Skin::from_index(enemy.skin)
                                .map_or("?", |skin| skin.name()),
                        )
                        .show_ui(ui, |ui| {
                            for skin in crate::characters::Skin::ALL {
                                ui.selectable_value(&mut enemy.skin, skin.index(), skin.name());
                            }
                        });
                    egui::ComboBox::from_id_salt(("behavior", index))
                        .selected_text(format!("{:?}", enemy.behavior))
                        .show_ui(ui, |ui| {
                            for behavior in crate::levels::Behavior::ALL {
                                ui.selectable_value(
                                    &mut enemy.behavior,
                                    behavior,
                                    format!("{behavior:?}"),
                                );
                            }
                        });
                });
            }
            if let Some(index) = remove {
                self.document.levels.rooms[self.room].enemies.remove(index);
            }
        });
        let after = self.document.levels.rooms[self.room].clone();
        if before != after {
            self.document.record(Change::Room {
                index: self.room,
                before,
                after,
            });
        }
        ui.separator();
        ui.label(format!(
            "{} ennemis / {} dangers",
            self.document.levels.rooms[self.room].enemies.len(),
            self.document.levels.rooms[self.room].hazards.len()
        ));
    }

    fn level_canvas(&mut self, ui: &mut egui::Ui) -> Result<(), String> {
        ui.heading(self.document.levels.name(self.room));
        // Les biomes teintent les ressources du pack ; le test utilise le rendu complet du jeu.
        let theme = self.room % NAMES.len();
        let tile_texture = self.texture(ui.ctx(), &format!("tile_{theme}"), 0)?;
        let player_texture = self.texture(ui.ctx(), "player", 0)?;
        let enemy_texture = self.texture(ui.ctx(), "enemy", 0)?;
        let scale = (ui.available_width() / WIDTH)
            .min(ui.available_height() / HEIGHT)
            .max(0.01);
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(WIDTH, HEIGHT) * scale,
            egui::Sense::click_and_drag(),
        );
        let painter = ui.painter_at(rect);
        let palette = &self.document.content.manifest.rooms[theme];
        let biome = self.document.levels.rooms[self.room]
            .biome
            .map(|index| &self.document.levels.biomes[index]);
        let background = biome.map_or(palette.background, |biome| biome.background);
        let tint = biome.map_or(egui::Color32::WHITE, |biome| {
            egui::Color32::from_rgb(biome.stone[0], biome.stone[1], biome.stone[2])
        });
        let accent = biome.map_or(egui::Color32::WHITE, |biome| {
            egui::Color32::from_rgb(biome.accent[0], biome.accent[1], biome.accent[2])
        });
        painter.rect_filled(
            rect,
            0.0,
            egui::Color32::from_rgb(background[0], background[1], background[2]),
        );
        let room = &self.document.levels.rooms[self.room];
        let world_rect = |horizontal: f32, vertical: f32, width: f32, height: f32| {
            egui::Rect::from_min_size(
                rect.min + egui::vec2(horizontal, vertical) * scale,
                egui::vec2(width, height) * scale,
            )
        };
        let uv = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
        for (row, pixels) in room.tiles.iter().enumerate() {
            for (column, pixel) in pixels.bytes().enumerate() {
                if pixel == b'#' {
                    painter.image(
                        tile_texture,
                        world_rect(column as f32 * TILE, row as f32 * TILE, TILE, TILE),
                        uv,
                        tint,
                    );
                }
            }
        }
        for enemy in &room.enemies {
            painter.image(
                enemy_texture,
                world_rect(
                    enemy.position[0],
                    enemy.position[1],
                    if enemy.boss { 46.0 } else { 26.0 },
                    if enemy.boss { 60.0 } else { 28.0 },
                ),
                uv,
                accent,
            );
        }
        for hazard in &room.hazards {
            let hazard_rect = world_rect(hazard[0], hazard[1], hazard[2], hazard[3]);
            painter.rect_filled(hazard_rect, 0.0, egui::Color32::from_rgb(239, 110, 113));
        }
        if let Some(shrine) = room.shrine {
            painter.rect_filled(
                world_rect(shrine[0] - 8.0, shrine[1] - 38.0, 16.0, 38.0),
                2.0,
                egui::Color32::from_rgb(97, 226, 198),
            );
        }
        painter.image(
            player_texture,
            world_rect(room.spawn[0], room.spawn[1], 22.0, 34.0),
            uv,
            egui::Color32::WHITE,
        );
        for portal in &self.document.levels.portals {
            let point = if portal.from == self.room {
                portal.entrance
            } else if portal.to == self.room {
                portal.exit
            } else {
                continue;
            };
            painter.rect_stroke(
                world_rect(point[0] - 8.0, point[1] - 14.0, 38.0, 48.0),
                3.0,
                egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(244, 193, 113)),
                egui::StrokeKind::Inside,
            );
        }
        if self.grid {
            draw_grid(&painter, rect, COLS, ROWS);
        }
        if response.is_pointer_button_down_on() {
            if let Some(pointer) = response
                .interact_pointer_pos()
                .filter(|point| rect.contains(*point))
            {
                let cell = (
                    ((pointer.x - rect.min.x) / (TILE * scale)) as usize,
                    ((pointer.y - rect.min.y) / (TILE * scale)) as usize,
                );
                if self.last_cell != Some(cell) {
                    if self.stroke.is_none() {
                        self.stroke = Some(Stroke::Room(
                            self.room,
                            self.document.levels.rooms[self.room].clone(),
                        ));
                    }
                    let erase = ui.input(|input| input.pointer.secondary_down());
                    paint_room(
                        &mut self.document.levels.rooms[self.room],
                        cell.0,
                        cell.1,
                        if erase {
                            LevelTool::Erase
                        } else {
                            self.level_tool
                        },
                        self.boss,
                    );
                    self.last_cell = Some(cell);
                }
            }
        }
        Ok(())
    }

    fn sprite_properties(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sprite");
        let names: Vec<_> = self
            .document
            .content
            .manifest
            .sprites
            .keys()
            .cloned()
            .collect();
        egui::ComboBox::from_id_salt("sprite_select")
            .selected_text(&self.sprite)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for name in names {
                    if ui
                        .selectable_value(&mut self.sprite, name.clone(), name)
                        .changed()
                    {
                        self.finish_stroke();
                        self.frame = 0;
                    }
                }
            });
        let before = self.document.content.manifest.sprites[&self.sprite].clone();
        let sprite = self
            .document
            .content
            .manifest
            .sprites
            .get_mut(&self.sprite)
            .expect("sprite role");
        ui.label(format!("{} x {} pixels", sprite.size[0], sprite.size[1]));
        ui.horizontal(|ui| {
            if ui.button("<").on_hover_text("Frame precedente").clicked() {
                self.frame = (self.frame + sprite.frames.len() - 1) % sprite.frames.len();
            }
            ui.label(format!("{} / {}", self.frame + 1, sprite.frames.len()));
            if ui.button(">").on_hover_text("Frame suivante").clicked() {
                self.frame = (self.frame + 1) % sprite.frames.len();
            }
        });
        ui.checkbox(&mut self.animation, "Lecture animation");
        ui.add(
            egui::DragValue::new(&mut sprite.fps)
                .range(0.1..=120.0)
                .speed(0.1)
                .suffix(" fps"),
        );
        egui::CollapsingHeader::new("Sequence de marche").show(ui, |ui| {
            for (index, frame) in sprite.walk.iter_mut().enumerate() {
                ui.add(
                    egui::DragValue::new(frame)
                        .range(0..=sprite.frames.len() - 1)
                        .prefix(format!("{} : ", index + 1)),
                );
            }
            ui.horizontal(|ui| {
                if sprite.walk.len() < 64
                    && ui.button("+").on_hover_text("Ajouter un slot").clicked()
                {
                    sprite.walk.push(0);
                }
                if sprite.walk.len() > 1
                    && ui
                        .button("-")
                        .on_hover_text("Retirer le dernier slot")
                        .clicked()
                {
                    sprite.walk.pop();
                }
            });
        });
        let after = sprite.clone();
        if before != after {
            self.document.record(Change::Animation {
                name: self.sprite.clone(),
                before,
                after,
            });
        }
        ui.separator();
        ui.heading("Outil");
        for (tool, label) in [
            (PixelTool::Brush, "Crayon"),
            (PixelTool::Erase, "Gomme"),
            (PixelTool::Fill, "Remplissage"),
            (PixelTool::Pick, "Pipette"),
        ] {
            ui.radio_value(&mut self.pixel_tool, tool, label);
        }
        ui.color_edit_button_srgba_unmultiplied(&mut self.color);
        ui.horizontal_wrapped(|ui| {
            for color in [
                [97, 226, 198, 255],
                [226, 233, 211, 255],
                [246, 193, 113, 255],
                [239, 110, 113, 255],
                [57, 72, 71, 255],
                [0, 0, 0, 0],
            ] {
                if ui
                    .add(
                        egui::Button::new(if color[3] == 0 { "/" } else { " " })
                            .fill(egui::Color32::from_rgba_unmultiplied(
                                color[0], color[1], color[2], color[3],
                            ))
                            .min_size(egui::vec2(22.0, 22.0)),
                    )
                    .on_hover_text(format!("RGBA {color:?}"))
                    .clicked()
                {
                    self.color = color;
                }
            }
        });
        ui.checkbox(&mut self.grid, "Grille");
        if ui
            .add_enabled(!self.isolated, egui::Button::new("Importer PNG"))
            .clicked()
        {
            self.finish_stroke();
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG", &["png"])
                .pick_file()
            {
                let result = (|| -> Result<(), String> {
                    let key = self.document.image(&self.sprite, self.frame)?;
                    let sprite = &self.document.content.manifest.sprites[&self.sprite];
                    let imported = Content {
                        root: path
                            .parent()
                            .ok_or("Dossier PNG introuvable")?
                            .to_path_buf(),
                        manifest: self.document.content.manifest.clone(),
                    };
                    let spec = Sprite {
                        size: sprite.size,
                        frames: vec![path
                            .file_name()
                            .ok_or("Nom PNG introuvable")?
                            .to_string_lossy()
                            .into_owned()],
                        fps: 1.0,
                        walk: vec![0],
                    };
                    let image = imported.image(&spec, 0)?;
                    let before = self.document.images[&key].pixels.clone();
                    let after = Pixels {
                        size: spec.size,
                        bytes: image.bytes,
                    };
                    self.document
                        .images
                        .get_mut(&key)
                        .expect("image cache")
                        .pixels = after.clone();
                    self.document.record(Change::Pixels {
                        path: key,
                        before,
                        after,
                    });
                    Ok(())
                })();
                self.notice = result.err().unwrap_or_else(|| "PNG importe".into());
            }
        }
    }

    fn sprite_canvas(&mut self, ui: &mut egui::Ui) -> Result<(), String> {
        let sprite = &self.document.content.manifest.sprites[&self.sprite];
        let frame = if self.animation {
            sprite.walk[(get_time() as f32 * sprite.fps) as usize % sprite.walk.len()]
        } else {
            self.frame
        };
        let size = sprite.size;
        let name = self.sprite.clone();
        let path = self.document.image(&name, frame)?;
        let texture = self.texture(ui.ctx(), &name, frame)?;
        ui.heading(format!("{} / frame {}", self.sprite, frame));
        let scale = (ui.available_width() / size[0] as f32)
            .min(ui.available_height() / size[1] as f32)
            .max(0.01);
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(size[0] as f32, size[1] as f32) * scale,
            egui::Sense::click_and_drag(),
        );
        let painter = ui.painter_at(rect);
        checkerboard(&painter, rect);
        painter.image(
            texture,
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        if self.grid && scale >= 6.0 {
            draw_grid(&painter, rect, size[0] as usize, size[1] as usize);
        }
        if !self.animation && response.is_pointer_button_down_on() {
            if let Some(pointer) = response
                .interact_pointer_pos()
                .filter(|point| rect.contains(*point))
            {
                let cell = (
                    ((pointer.x - rect.min.x) / scale) as usize,
                    ((pointer.y - rect.min.y) / scale) as usize,
                );
                if self.last_cell != Some(cell) {
                    let pixels = &mut self
                        .document
                        .images
                        .get_mut(&path)
                        .expect("image cache")
                        .pixels;
                    if cell.0 < size[0] as usize && cell.1 < size[1] as usize {
                        if self.pixel_tool == PixelTool::Pick {
                            let offset = (cell.1 * size[0] as usize + cell.0) * 4;
                            self.color
                                .copy_from_slice(&pixels.bytes[offset..offset + 4]);
                        } else {
                            if self.stroke.is_none() {
                                self.stroke = Some(Stroke::Pixels(path.clone(), pixels.clone()));
                            }
                            let rgba = if self.pixel_tool == PixelTool::Erase
                                || ui.input(|input| input.pointer.secondary_down())
                            {
                                [0; 4]
                            } else {
                                self.color
                            };
                            if self.pixel_tool == PixelTool::Fill {
                                pixels.flood(cell.0, cell.1, rgba);
                            } else {
                                pixels.paint(cell.0, cell.1, rgba);
                            }
                        }
                        self.last_cell = Some(cell);
                    }
                }
            }
        }
        Ok(())
    }

    fn audio_targets(&self) -> Vec<(String, String)> {
        let mut targets = vec![(
            "Musique".into(),
            self.document.content.manifest.music.clone(),
        )];
        for cue in crate::game::SoundCue::ALL {
            targets.push((
                format!("{cue:?}"),
                self.document.content.manifest.effects[&cue].clone(),
            ));
        }
        targets
    }

    fn audio_properties(&mut self, ui: &mut egui::Ui) {
        ui.heading("Banque audio");
        for (index, (label, _)) in self.audio_targets().iter().enumerate() {
            if ui
                .selectable_value(&mut self.audio_index, index, label)
                .changed()
            {
                self.stop_audio = true;
                self.loop_audio = index == 0;
            }
        }
        ui.separator();
        ui.checkbox(&mut self.loop_audio, "Boucle");
        if ui
            .add_enabled(!self.isolated, egui::Button::new("Importer WAV"))
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("WAV PCM16", &["wav"])
                .pick_file()
            {
                let result = (|| -> Result<(), String> {
                    let imported = Content {
                        root: path
                            .parent()
                            .ok_or("Dossier WAV introuvable")?
                            .to_path_buf(),
                        manifest: self.document.content.manifest.clone(),
                    };
                    let bytes = imported.wav(
                        &path
                            .file_name()
                            .ok_or("Nom WAV introuvable")?
                            .to_string_lossy(),
                    )?;
                    let mut clip = Clip::decode(&bytes)?;
                    clip.imported = true;
                    let key = self.audio_targets()[self.audio_index].1.clone();
                    self.document.clips.insert(key, clip);
                    self.stop_audio = true;
                    Ok(())
                })();
                self.notice = result.err().unwrap_or_else(|| "WAV importe".into());
            }
        }
    }

    fn audio_canvas(&mut self, ui: &mut egui::Ui) -> Result<(), String> {
        let (label, path) = self.audio_targets()[self.audio_index].clone();
        self.document.clip(&path)?;
        ui.heading(label);
        ui.add(egui::Label::new(&path).truncate());
        let clip = self.document.clips.get_mut(&path).expect("audio cache");
        ui.label(format!(
            "{} Hz / {} canal(aux) / {:.3} s",
            clip.spec.sample_rate,
            clip.spec.channels,
            clip.duration()
        ));
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(
                ui.available_width(),
                (ui.available_height() * 0.45).clamp(70.0, 230.0),
            ),
            egui::Sense::hover(),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(14, 22, 22));
        let bins = (rect.width() as usize).clamp(1, 1000);
        let frames = clip.samples.len() / clip.spec.channels as usize;
        for index in 0..bins {
            let start = index * frames / bins;
            let end = ((index + 1) * frames / bins).max(start + 1).min(frames);
            let peak = clip.samples
                [start * clip.spec.channels as usize..end * clip.spec.channels as usize]
                .iter()
                .map(|sample| (*sample as f32).abs())
                .fold(0.0, f32::max)
                / 32768.0;
            let horizontal = rect.min.x + index as f32 / bins as f32 * rect.width();
            painter.line_segment(
                [
                    egui::pos2(horizontal, rect.center().y - peak * rect.height() * 0.45),
                    egui::pos2(horizontal, rect.center().y + peak * rect.height() * 0.45),
                ],
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(112, 205, 225)),
            );
        }
        let duration = clip.duration();
        let selected = egui::Rect::from_min_max(
            egui::pos2(
                rect.min.x + clip.settings.start / duration * rect.width(),
                rect.min.y,
            ),
            egui::pos2(
                rect.min.x + clip.settings.end / duration * rect.width(),
                rect.max.y,
            ),
        );
        painter.rect_filled(
            selected,
            0.0,
            egui::Color32::from_rgba_unmultiplied(97, 226, 198, 35),
        );
        ui.horizontal_wrapped(|ui| {
            ui.label("Debut");
            ui.add(
                egui::DragValue::new(&mut clip.settings.start)
                    .range(0.0..=duration)
                    .speed(0.01)
                    .suffix(" s"),
            );
            ui.label("Fin");
            ui.add(
                egui::DragValue::new(&mut clip.settings.end)
                    .range(0.0..=duration)
                    .speed(0.01)
                    .suffix(" s"),
            );
        });
        ui.add(egui::Slider::new(&mut clip.settings.gain, 0.0..=2.0).text("Gain"));
        ui.horizontal_wrapped(|ui| {
            ui.label("Fondu entree");
            ui.add(
                egui::DragValue::new(&mut clip.settings.fade_in)
                    .range(0.0..=duration)
                    .speed(0.01)
                    .suffix(" s"),
            );
            ui.label("Fondu sortie");
            ui.add(
                egui::DragValue::new(&mut clip.settings.fade_out)
                    .range(0.0..=duration)
                    .speed(0.01)
                    .suffix(" s"),
            );
        });
        let mut reset = false;
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!self.isolated, egui::Button::new("\u{25b6}"))
                .on_hover_text("Ecouter la selection retouchee")
                .clicked()
            {
                match clip.render() {
                    Ok(bytes) => self.preview = Some(bytes),
                    Err(error) => self.notice = error,
                }
            }
            if ui
                .button("\u{25a0}")
                .on_hover_text("Arreter la lecture")
                .clicked()
            {
                self.stop_audio = true;
            }
            if ui.button("Reinitialiser").clicked() {
                reset = true;
            }
        });
        if reset {
            self.document.clips.remove(&path);
            self.stop_audio = true;
        }
        Ok(())
    }
}

fn draw_grid(painter: &egui::Painter, rect: egui::Rect, columns: usize, rows: usize) {
    let stroke = egui::Stroke::new(
        0.5_f32,
        egui::Color32::from_rgba_unmultiplied(180, 200, 190, 60),
    );
    for column in 0..=columns {
        let horizontal = rect.min.x + column as f32 / columns as f32 * rect.width();
        painter.line_segment(
            [
                egui::pos2(horizontal, rect.min.y),
                egui::pos2(horizontal, rect.max.y),
            ],
            stroke,
        );
    }
    for row in 0..=rows {
        let vertical = rect.min.y + row as f32 / rows as f32 * rect.height();
        painter.line_segment(
            [
                egui::pos2(rect.min.x, vertical),
                egui::pos2(rect.max.x, vertical),
            ],
            stroke,
        );
    }
}

fn checkerboard(painter: &egui::Painter, rect: egui::Rect) {
    let cell = 16.0;
    for row in 0..(rect.height() / cell).ceil() as usize {
        for column in 0..(rect.width() / cell).ceil() as usize {
            let tile = egui::Rect::from_min_size(
                rect.min + egui::vec2(column as f32 * cell, row as f32 * cell),
                egui::vec2(cell, cell),
            )
            .intersect(rect);
            painter.rect_filled(
                tile,
                0.0,
                if (column + row).is_multiple_of(2) {
                    egui::Color32::from_gray(34)
                } else {
                    egui::Color32::from_gray(44)
                },
            );
        }
    }
}

pub struct Options {
    pub tab: Tab,
    pub weapon: usize,
    pub room: usize,
    pub capture: Option<PathBuf>,
    pub test: bool,
    pub generation_seed: Option<u32>,
    pub visuals: crate::render::VisualSettings,
}

// Renvoie true si l'utilisateur a demande de fermer le jeu, et non seulement l'atelier.
pub async fn run(
    root: &Path,
    options: Options,
    bindings: &crate::controls::Bindings,
) -> Result<bool, String> {
    use macroquad::audio::{load_sound_from_bytes, play_sound, stop_sound, PlaySoundParams, Sound};
    let mut workspace = Workspace::new(root, options.tab, options.room, options.capture.is_some())?;
    if options.weapon >= workspace.document.content.manifest.weapons.len() {
        return Err("Indice d'arme introuvable dans le pack".into());
    }
    workspace.weapon_index = options.weapon;
    // Le diagnostic emprunte la meme action que les boutons, sans enregistrer le document.
    if let Some(seed) = options.generation_seed {
        workspace.generation_seed = seed;
        workspace.generate_level(options.test);
    }
    workspace.request_test = options.test;
    let mut sound: Option<Sound> = None;
    let mut sandbox: Option<Game> = None;
    let mut art = crate::render::Art::load(&workspace.document.content)?;
    art.effects = options.visuals;
    let target = crate::render3d::target(WIDTH as u32, crate::render::VIEW_HEIGHT as u32);
    let mut accumulator = 0.0;
    let mut pending = Input::default();
    let mut frame = 0;
    prevent_quit();
    loop {
        set_default_camera();
        clear_background(crate::render::INK);
        let close_requested = is_quit_requested();
        if close_requested {
            workspace.close_game = true;
            workspace.request_exit = true;
        }
        if workspace.request_test {
            workspace.request_test = false;
            // Une annulation peut avoir retire la salle selectionnee juste avant le test.
            workspace.room = workspace
                .room
                .min(workspace.document.levels.rooms.len() - 1);
            match workspace
                .document
                .levels
                .validate()
                .and_then(|()| workspace.document.content.manifest.validate())
            {
                Ok(()) => {
                    art = crate::render::Art::load(&workspace.document.content)?;
                    art.effects = options.visuals;
                    for (name, sprite) in &workspace.document.content.manifest.sprites {
                        for (index, path) in sprite.frames.iter().enumerate() {
                            if let Some(image) = workspace.document.images.get(path) {
                                art.update_image(
                                    name,
                                    index,
                                    &Image {
                                        width: image.pixels.size[0],
                                        height: image.pixels.size[1],
                                        bytes: image.pixels.bytes.clone(),
                                    },
                                );
                            }
                        }
                    }
                    let mut game = Game::with_levels(
                        Save {
                            double_jump: true,
                            ..Save::default()
                        },
                        workspace.document.levels.clone(),
                    );
                    let spawn = workspace.document.levels.rooms[workspace.room].spawn;
                    game.all_weapons = true;
                    game.set_weapons(workspace.document.content.manifest.weapons.clone());
                    game.equip(workspace.weapon_index);
                    game.enter(workspace.room, vec2(spawn[0], spawn[1]));
                    game.save_dirty = false;
                    sandbox = Some(game);
                    workspace.stop_audio = true;
                    accumulator = 0.0;
                    pending = Input::default();
                }
                Err(error) => workspace.notice = error,
            }
        }
        if let Some(game) = &mut sandbox {
            if is_key_pressed(KeyCode::Escape) {
                sandbox = None;
                pending = Input::default();
                accumulator = 0.0;
            } else {
                pending.direction = if bindings.down(crate::controls::Action::Right) {
                    1.0
                } else {
                    0.0
                } - if bindings.down(crate::controls::Action::Left) {
                    1.0
                } else {
                    0.0
                };
                pending.jump |= bindings.pressed(crate::controls::Action::Jump);
                pending.attack |= bindings.pressed(crate::controls::Action::Attack);
                pending.attack_held = bindings.down(crate::controls::Action::Attack)
                    || is_mouse_button_down(MouseButton::Left);
                pending.attack |= is_mouse_button_pressed(MouseButton::Left);
                pending.next_weapon |= bindings.pressed(crate::controls::Action::NextWeapon);
                pending.dash |= bindings.pressed(crate::controls::Action::Dash);
                pending.interact |= bindings.pressed(crate::controls::Action::Interact);
                // La capture teste aussi le rendu du projectile sans entree utilisateur ni sauvegarde.
                if options.capture.is_some() && game.weapon().projectile.is_some() {
                    pending.attack_held = game.weapon().automatic && (70..=90).contains(&frame);
                    pending.attack |= frame == 80;
                }
                accumulator += if options.capture.is_some() {
                    1.0 / 60.0
                } else {
                    get_frame_time().min(0.1)
                };
                while accumulator >= STEP {
                    game.update(pending, STEP);
                    pending.jump = false;
                    pending.attack = false;
                    pending.next_weapon = false;
                    pending.dash = false;
                    pending.interact = false;
                    accumulator -= STEP;
                }
                game.sounds.clear();
                game.save_dirty = false;
                let mut camera = Camera2D::from_display_rect(Rect::new(
                    0.0,
                    -56.0,
                    WIDTH,
                    crate::render::VIEW_HEIGHT,
                ));
                camera.render_target = Some(target.clone());
                // Le confort visuel et les secousses suivent les memes regles que dans le jeu.
                camera.target += crate::render3d::shake_offset(game, art.effects.motion());
                set_camera(&camera);
                crate::render::scene(&art, game, &target);
                let mut camera = Camera2D::from_display_rect(Rect::new(
                    0.0,
                    0.0,
                    WIDTH,
                    crate::render::VIEW_HEIGHT,
                ));
                camera.render_target = Some(target.clone());
                set_camera(&camera);
                crate::render::hud(&art, game);
                set_default_camera();
                let scale = (screen_width() / WIDTH)
                    .min((screen_height() - 40.0) / crate::render::VIEW_HEIGHT);
                art.present(
                    &target.texture,
                    Rect::new(
                        (screen_width() - WIDTH * scale) * 0.5,
                        40.0 + (screen_height() - 40.0 - crate::render::VIEW_HEIGHT * scale) * 0.5,
                        WIDTH * scale,
                        crate::render::VIEW_HEIGHT * scale,
                    ),
                    Some(game),
                );
            }
        }
        let testing = sandbox.is_some();
        let mut exit = false;
        egui_macroquad::ui(|context| {
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = egui::Color32::from_rgb(22, 32, 29);
            visuals.selection.bg_fill = egui::Color32::from_rgb(44, 97, 80);
            visuals.selection.stroke.color = egui::Color32::from_rgb(226, 233, 211);
            context.set_visuals(visuals);
            if testing {
                egui::TopBottomPanel::top("sandbox_bar").show(context, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong("TEST / ");
                        let game = sandbox.as_ref().expect("sandbox");
                        ui.label(game.levels.name(game.room));
                        if ui.button("Retour a l'edition").clicked() {
                            sandbox = None;
                        }
                    });
                });
            } else {
                if is_key_pressed(KeyCode::Escape) && !context.wants_keyboard_input() {
                    workspace.request_exit = true;
                }
                workspace.ui(context);
                if workspace.request_exit {
                    workspace.request_exit = false;
                    workspace.finish_stroke();
                    if workspace.document.dirty() {
                        workspace.confirm_exit = true;
                    } else {
                        exit = true;
                    }
                }
            }
            if workspace.confirm_exit || (close_requested && workspace.document.dirty()) {
                workspace.confirm_exit = true;
                egui::Window::new("Modifications non enregistrees")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    .show(context, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add_enabled(
                                    !workspace.isolated,
                                    egui::Button::new("Enregistrer tout et quitter"),
                                )
                                .clicked()
                                && workspace.save(true)
                            {
                                exit = true;
                            }
                            if ui.button("Quitter sans enregistrer").clicked() {
                                exit = true;
                            }
                            if ui.button("Annuler").clicked() {
                                workspace.confirm_exit = false;
                                workspace.close_game = false;
                            }
                        });
                    });
            } else if close_requested {
                exit = true;
            }
        });
        egui_macroquad::draw();
        if workspace.stop_audio {
            if let Some(sound) = sound.take() {
                stop_sound(&sound);
            }
            workspace.stop_audio = false;
        }
        if let Some(bytes) = workspace.preview.take() {
            if let Some(previous) = sound.take() {
                stop_sound(&previous);
            }
            match load_sound_from_bytes(&bytes).await {
                Ok(loaded) => {
                    play_sound(
                        &loaded,
                        PlaySoundParams {
                            looped: workspace.loop_audio,
                            volume: bindings.sfx_volume,
                        },
                    );
                    sound = Some(loaded);
                }
                Err(error) => workspace.notice = error.to_string(),
            }
        }
        if frame == 90 {
            if let Some(path) = &options.capture {
                get_screen_data().export_png(path.to_str().ok_or("Chemin de capture invalide")?);
                println!("Editor captured to {}", path.display());
                exit = true;
            }
        }
        if exit {
            if let Some(sound) = sound {
                stop_sound(&sound);
            }
            return Ok(workspace.close_game);
        }
        frame += 1;
        next_frame().await;
    }
}

#[cfg(test)]
mod tests {
    // Copier seulement les fichiers utilises : les tests ne touchent jamais au pack du joueur.
    fn temporary_document() -> Document {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("lumen-editor-{}-{unique}", std::process::id()));
        let source =
            Content::load(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")).unwrap();
        std::fs::create_dir_all(&root).unwrap();
        source.manifest.save(&root.join("manifest.json")).unwrap();
        for path in [
            &source.manifest.sprites["player"].frames[0],
            &source.manifest.effects[&crate::game::SoundCue::Jump],
        ] {
            let destination = root.join(path);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            std::fs::copy(source.root.join(path), destination).unwrap();
        }
        Document::load(&root).unwrap()
    }

    #[test]
    fn weapons_are_undoable_saved_independently_and_available_in_sandbox() {
        let document = temporary_document();
        let root = document.content.root.clone();
        let mut workspace = Workspace::new(&root, Tab::Weapons, 0, false).unwrap();
        let original = workspace.document.content.manifest.weapons.clone();
        let mut created = original.clone();
        let mut weapon = original[0].clone();
        weapon.name = "Lame du test".into();
        weapon.damage = 5;
        weapon.automatic = true;
        weapon.laser = true;
        weapon.unlock_kills = 10;
        weapon.projectile = Some(crate::content::ProjectileSettings {
            speed: 650.0,
            range: 700.0,
            radius: 4.0,
        });
        created.push(weapon);
        workspace.replace_weapons(created.clone());
        assert!(workspace.document.dirty());
        workspace.document.undo();
        assert_eq!(workspace.document.content.manifest.weapons, original);
        assert!(!workspace.document.dirty());
        workspace.document.redo();
        let saved_fps = workspace.document.saved_manifest.sprites["player"].fps;
        workspace
            .document
            .content
            .manifest
            .sprites
            .get_mut("player")
            .unwrap()
            .fps = saved_fps + 1.0;
        workspace.document.save(Tab::Weapons, false).unwrap();
        let reloaded = Document::load(&root).unwrap();
        assert_eq!(reloaded.content.manifest.weapons, created);
        assert_eq!(reloaded.content.manifest.sprites["player"].fps, saved_fps);
        assert!(workspace.document.dirty());
        let mut game = Game::with_levels(Save::default(), reloaded.levels);
        game.all_weapons = true;
        game.weapons = reloaded.content.manifest.weapons;
        game.equip(created.len() - 1);
        assert_eq!(game.weapon().damage, 5);
        assert!(game.weapon().automatic);
        assert!(game.weapon().laser);
        assert_eq!(game.weapon().unlock_kills, 10);
        assert_eq!(game.weapon().projectile.as_ref().unwrap().speed, 650.0);
        let mut invalid = created.clone();
        invalid[0].name = invalid[1].name.clone();
        workspace.replace_weapons(invalid);
        assert_eq!(workspace.document.content.manifest.weapons, created);
        assert!(workspace.document.save(Tab::Weapons, true).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generation_history_restores_connections_and_saved_pack_is_playable() {
        let mut document = temporary_document();
        let root = document.content.root.clone();
        let before = document.levels.clone();
        let index = document.levels.generate(1, 42, 2).unwrap();
        let after = document.levels.clone();
        document.record(Change::Levels {
            before: before.clone(),
            after: after.clone(),
        });
        assert!(document.dirty());
        document.undo();
        assert_eq!(document.levels, before);
        assert!(!document.dirty());
        document.redo();
        assert_eq!(document.levels, after);
        document.save(Tab::Levels, false).unwrap();
        let reloaded = Document::load(&root).unwrap();
        assert_eq!(reloaded.levels, after);
        let mut game = Game::with_levels(Save::default(), reloaded.levels);
        game.enter(index, vec2(96.0, 446.0));
        assert_eq!(game.portals()[0].1, 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn edited_pack_saves_and_reloads_levels_pixels_and_audio() {
        let mut document = temporary_document();
        let root = document.content.root.clone();
        document.levels.rooms[0].set_tile(2, 3, true);
        let image_path = document.image("player", 0).unwrap();
        document
            .images
            .get_mut(&image_path)
            .unwrap()
            .pixels
            .paint(0, 0, [241, 32, 60, 255]);
        let audio_path = document.content.manifest.effects[&crate::game::SoundCue::Jump].clone();
        document.clip(&audio_path).unwrap();
        let original_audio = document.content.wav(&audio_path).unwrap();
        document.clips.get_mut(&audio_path).unwrap().settings.gain = 0.5;
        assert!(document.dirty());
        document.save(Tab::Levels, false).unwrap();
        assert!(document.dirty());
        document.save(Tab::Sprites, false).unwrap();
        assert!(document.dirty());
        document.save(Tab::Audio, false).unwrap();
        assert!(!document.dirty());
        let mut reloaded = Document::load(&root).unwrap();
        assert_eq!(reloaded.levels.rooms[0].tiles[3].as_bytes()[2], b'#');
        // Verifier le consommateur reel, pas seulement le JSON ou la grille de l'atelier.
        let game = Game::with_levels(Save::default(), reloaded.levels.clone());
        assert!(game.world.collide_check(game.actor, vec2(64.0, 96.0)));
        let path = reloaded.image("player", 0).unwrap();
        assert_eq!(
            &reloaded.images[&path].pixels.bytes[..4],
            &[241, 32, 60, 255]
        );
        assert_ne!(reloaded.content.wav(&audio_path).unwrap(), original_audio);
        assert!(!reloaded.dirty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_save_preserves_dirty_state_and_previous_file() {
        let mut document = temporary_document();
        let root = document.content.root.clone();
        document.save(Tab::Levels, false).unwrap();
        let original = std::fs::read(root.join("levels.json")).unwrap();
        document.levels.rooms[0].set_tile(2, 3, true);
        std::fs::create_dir(root.join("levels.tmp")).unwrap();
        assert!(document.save(Tab::Levels, false).is_err());
        assert!(document.dirty());
        assert_eq!(std::fs::read(root.join("levels.json")).unwrap(), original);
        std::fs::remove_dir_all(root).unwrap();
    }
    use super::*;

    #[test]
    fn biome_and_gate_edits_are_undoable_saved_and_used_by_the_game() {
        let mut document = temporary_document();
        let root = document.content.root.clone();
        let before = document.levels.biomes[0].clone();
        let mut after = before.clone();
        after.name = "FORET EDITEe".into();
        after.accent = [90, 220, 140];
        after.architecture = 7;
        after.flora = 3;
        document.levels.biomes[0] = after.clone();
        document.record(Change::Biome {
            index: 0,
            before: before.clone(),
            after: after.clone(),
        });
        document.undo();
        assert_eq!(document.levels.biomes[0], before);
        document.redo();
        assert_eq!(document.levels.biomes[0], after);
        let index = document
            .levels
            .portals
            .iter()
            .position(|portal| portal.requires_boss.is_some())
            .unwrap();
        let before = document.levels.portals[index].clone();
        let mut after = before.clone();
        after.entrance = [820.0, 446.0];
        after.requires_boss = None;
        document.levels.portals[index] = after.clone();
        document.record(Change::Portal {
            index,
            before: before.clone(),
            after: after.clone(),
        });
        document.undo();
        assert_eq!(document.levels.portals[index], before);
        document.redo();
        assert_eq!(document.levels.portals[index], after);
        document.save(Tab::Levels, false).unwrap();
        let reloaded = Document::load(&root).unwrap();
        assert_eq!(reloaded.levels, document.levels);
        let mut game = Game::with_levels(Save::default(), reloaded.levels);
        game.enter(after.from, vec2(820.0, 446.0));
        assert!(game
            .portals()
            .iter()
            .any(|(_, target, _)| *target == after.to));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pixel_paint_and_fill_preserve_disconnected_regions() {
        let mut pixels = Pixels {
            size: [3, 2],
            bytes: vec![0; 24],
        };
        pixels.paint(1, 0, [255, 0, 0, 255]);
        pixels.paint(1, 1, [255, 0, 0, 255]);
        pixels.flood(0, 0, [0, 255, 0, 255]);
        assert_eq!(&pixels.bytes[0..4], &[0, 255, 0, 255]);
        assert_eq!(&pixels.bytes[8..12], &[0, 0, 0, 0]);
        pixels.paint(999, 999, [0; 4]);
        assert_eq!(pixels.bytes.len(), 24);
    }

    #[test]
    fn level_history_undoes_redoes_and_tracks_unsaved_state() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let mut document = Document::load(&root).unwrap();
        let before = document.levels.rooms[0].clone();
        paint_room(&mut document.levels.rooms[0], 2, 2, LevelTool::Solid, false);
        let after = document.levels.rooms[0].clone();
        document.record(Change::Room {
            index: 0,
            before,
            after,
        });
        assert!(document.dirty());
        document.undo();
        assert!(!document.dirty());
        document.redo();
        assert!(document.dirty());
        assert!(document.save(Tab::Levels, true).is_err());
    }

    #[test]
    fn audio_trimming_keeps_stereo_pairs_and_fades_endpoints() {
        let clip = Clip {
            spec: hound::WavSpec {
                channels: 2,
                sample_rate: 8_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
            samples: [20_000, -10_000].repeat(80),
            settings: AudioSettings {
                start: 0.002,
                end: 0.008,
                gain: 2.0,
                fade_in: 0.001,
                fade_out: 0.001,
            },
            imported: false,
        };
        let processed = Clip::decode(&clip.render().unwrap()).unwrap();
        assert_eq!(processed.samples.len(), 48 * 2);
        assert_eq!(&processed.samples[..2], &[0, 0]);
        assert_eq!(&processed.samples[processed.samples.len() - 2..], &[0, 0]);
        assert_eq!(processed.samples[30], i16::MAX);
        assert_eq!(processed.samples[31], -20_000);
    }
}
