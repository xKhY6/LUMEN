//! Menu natif de debug ; main garde le controle des ecritures personnelles.

use crate::characters::Character;
use crate::game::{DebugOptions, Game};
use egui_macroquad::egui;

pub struct DebugMenu {
    tab: usize,
    search: String,
    pub speed: f32,
    pub notice: String,
    confirm_save: bool,
}

impl Default for DebugMenu {
    fn default() -> Self {
        Self {
            tab: 0,
            search: String::new(),
            speed: 1.0,
            notice: String::new(),
            confirm_save: false,
        }
    }
}

#[derive(Default)]
pub struct Response {
    pub resume: bool,
    pub save: bool,
}

impl DebugMenu {
    pub fn draw(&mut self, game: &mut Game, isolated: bool) -> Response {
        let mut response = Response::default();
        egui_macroquad::ui(|context| {
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = egui::Color32::from_rgb(22, 32, 29);
            visuals.selection.bg_fill = egui::Color32::from_rgb(44, 97, 80);
            context.set_visuals(visuals);
            egui::CentralPanel::default().show(context, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.heading("DEBUG");
                    if ui.button("Reprendre").clicked() {
                        response.resume = true;
                    }
                    if ui
                        .add_enabled(
                            !isolated,
                            egui::Button::new("Enregistrer la progression..."),
                        )
                        .clicked()
                    {
                        self.confirm_save = true;
                    }
                });
                ui.label("SESSION DEBUG / SAUVEGARDE AUTO SUSPENDUE");
                ui.label(format!(
                    "{} | Vie {}/{} | Position {:.0}, {:.0}",
                    game.levels.name(game.room),
                    game.health,
                    game.max_health(),
                    game.position().x,
                    game.position().y
                ));
                if !self.notice.is_empty() {
                    ui.label(&self.notice);
                }
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    for (index, label) in ["Joueur", "Armes", "Niveaux", "Progression"]
                        .iter()
                        .enumerate()
                    {
                        ui.selectable_value(&mut self.tab, index, *label);
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.tab {
                        0 => self.player(ui, game),
                        1 => self.weapons(ui, game),
                        2 => self.levels(ui, game),
                        _ => self.progress(ui, game),
                    });
            });
            if self.confirm_save {
                egui::Window::new("Remplacer la sauvegarde ?")
                    .collapsible(false).resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    .show(context, |ui| {
                        ui.set_max_width((context.screen_rect().width() - 48.0).max(100.0));
                        ui.label("Les pouvoirs, armes, visites et boss modifies remplaceront la progression personnelle. Les aides temporaires ne seront pas enregistrees.");
                        ui.horizontal_wrapped(|ui| {
                            if ui.add_enabled(!isolated, egui::Button::new("Enregistrer")).clicked() { response.save = true; self.confirm_save = false; }
                            if ui.button("Annuler").clicked() { self.confirm_save = false; }
                        });
                    });
            }
        });
        egui_macroquad::draw();
        response
    }

    fn player(&mut self, ui: &mut egui::Ui, game: &mut Game) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("+1 vie").clicked() {
                game.health = (game.health + 1).min(game.max_health());
            }
            if ui.button("Soigner completement").clicked() {
                game.health = game.max_health();
            }
            if ui.button("Retour au sanctuaire").clicked() {
                game.debug_respawn();
            }
        });
        ui.add(
            egui::Slider::new(&mut game.debug.health_bonus, 0..=4)
                .text("Vies maximales supplementaires"),
        );
        game.health = game.health.min(game.max_health());
        ui.checkbox(&mut game.debug.invulnerable, "Invincible");
        ui.checkbox(&mut game.debug.infinite_jumps, "Sauts illimites");
        ui.checkbox(
            &mut game.debug.no_cooldowns,
            "Attaque et ruee sans recharge",
        );
        if ui
            .checkbox(&mut game.save.double_jump, "Double saut et ruee")
            .changed()
        {
            game.save_dirty = true;
        }
        ui.add(egui::Slider::new(&mut self.speed, 0.1..=3.0).text("Vitesse de simulation"));
        ui.separator();
        ui.strong("Personnage");
        ui.horizontal_wrapped(|ui| {
            for character in Character::ALL {
                let label = if game.character_unlocked(character) {
                    character.name().to_string()
                } else {
                    format!("{} (verrouille)", character.name())
                };
                if ui
                    .add(egui::Button::new(label).selected(game.save.character == character))
                    .clicked()
                {
                    game.debug_select_character(character);
                }
            }
        });
        ui.label(game.save.character.description());
        if ui.button("Retirer les aides temporaires").clicked() {
            game.debug = DebugOptions::default();
            game.health = game.health.min(game.max_health());
            self.speed = 1.0;
        }
    }

    fn weapons(&mut self, ui: &mut egui::Ui, game: &mut Game) {
        if ui.button("Debloquer toutes les armes").clicked() {
            game.debug_unlock_weapons();
        }
        let mut kills = game.save.defeated_enemies;
        if ui
            .add(egui::Slider::new(&mut kills, 0..=200).text("Ennemis vaincus"))
            .changed()
        {
            game.save.defeated_enemies = kills;
            game.set_weapons(game.weapons.clone());
            game.save_dirty = true;
        }
        for index in 0..game.weapons.len() {
            let weapon = &game.weapons[index];
            let label = format!(
                "{} | {} degats | seuil {}",
                weapon.name, weapon.damage, weapon.unlock_kills
            );
            if ui
                .add_enabled(
                    game.weapon_available(index),
                    egui::Button::new(label).selected(index == game.weapon_index),
                )
                .clicked()
            {
                game.debug_equip(index);
            }
        }
    }

    fn levels(&mut self, ui: &mut egui::Ui, game: &mut Game) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("Recharger la salle").clicked() {
                game.debug_enter(game.room);
            }
            if ui.button("Vider la salle (sans butin)").clicked() {
                // Garder les indices des acteurs stables pendant la frappe en cours.
                for enemy in &mut game.enemies {
                    enemy.health = 0;
                }
                game.hostile_shots.clear();
            }
            if ui.button("Retirer les projectiles").clicked() {
                game.projectiles.clear();
                game.hostile_shots.clear();
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Rechercher");
            ui.text_edit_singleline(&mut self.search);
        });
        let query = self.search.trim().to_lowercase();
        ui.collapsing("Entrees des biomes", |ui| {
            for biome in 0..game.levels.biomes.len() {
                let name = game.levels.biomes[biome].name.clone();
                if ui.button(name).clicked() {
                    if let Some(room) = game
                        .levels
                        .rooms
                        .iter()
                        .position(|room| room.biome == Some(biome))
                    {
                        game.debug_enter(room);
                    }
                }
            }
        });
        for room in 0..game.levels.rooms.len() {
            let label = format!("{} / {}", room + 1, game.levels.name(room));
            if (query.is_empty() || label.to_lowercase().contains(&query))
                && ui.selectable_label(room == game.room, label).clicked()
            {
                game.debug_enter(room);
            }
        }
    }

    fn progress(&mut self, ui: &mut egui::Ui, game: &mut Game) {
        if ui
            .checkbox(&mut game.save.life_shard, "Eclat de vie permanent")
            .changed()
        {
            game.health = game.health.min(game.max_health());
            game.save_dirty = true;
        }
        if ui.button("Reveler toute la carte").clicked() {
            game.save.visited.fill(true);
            game.save_dirty = true;
        }
        if ui
            .add_enabled(
                game.shrine().is_some(),
                egui::Button::new("Sanctuaire actuel comme point de retour"),
            )
            .clicked()
        {
            game.save.checkpoint = game.room;
            game.save_dirty = true;
        }
        if ui
            .checkbox(&mut game.save.won, "Victoire du coeur / campagne")
            .changed()
        {
            game.finished = false;
            game.save_dirty = true;
        }
        ui.separator();
        ui.strong("Boss et verrous de portails");
        for room in 0..game.levels.rooms.len() {
            if !game.levels.rooms[room]
                .enemies
                .iter()
                .any(|enemy| enemy.boss)
            {
                continue;
            }
            let mut defeated = game.save.defeated_bosses.contains(&room);
            if ui.checkbox(&mut defeated, game.levels.name(room)).changed() {
                if defeated {
                    game.save.defeated_bosses.push(room);
                } else {
                    game.save.defeated_bosses.retain(|index| *index != room);
                }
                // Reconstruire la salle active pour rafraichir son boss sans deplacer le joueur.
                if game.room == room {
                    game.enter(room, game.position());
                }
                game.save_dirty = true;
            }
        }
    }
}
