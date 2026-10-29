//! Regressions de parties completes : simulation sans fenetre, sans GPU et sans sauvegarde personnelle.

use crate::game::{Game, Input, Save, HEIGHT, STEP, WIDTH};
use crate::levels::Levels;
use macroquad::prelude::*;

fn campaign() -> (Levels, usize) {
    let mut levels = Levels::original();
    let start = levels.add_campaign().unwrap();
    (levels, start)
}

/// Empreinte de l'etat observable : suffit a detecter toute divergence de simulation.
fn fingerprint(game: &Game) -> Vec<i64> {
    let position = game.position();
    let mut values = vec![
        game.room as i64,
        (position.x * 100.0) as i64,
        (position.y * 100.0) as i64,
        (game.velocity.x * 100.0) as i64,
        (game.velocity.y * 100.0) as i64,
        i64::from(game.health),
        (game.time * 1000.0) as i64,
        i64::from(game.save.shards),
        game.enemies.len() as i64,
    ];
    for enemy in &game.enemies {
        let enemy_position = game.world.actor_pos(enemy.actor);
        values.push(i64::from(enemy.health));
        values.push((enemy_position.x * 100.0) as i64);
    }
    values
}

#[test]
fn every_room_survives_idle_and_walking_simulation_inside_its_bounds() {
    let (levels, _) = campaign();
    let mut game = Game::with_levels(Save::default(), levels.clone());
    // Un joueur invulnerable evite que la mort renvoie au sanctuaire pendant la mesure.
    game.debug.invulnerable = true;
    for (index, room) in levels.rooms.iter().enumerate() {
        game.enter(index, vec2(room.spawn[0], room.spawn[1]));
        for frame in 0..180 {
            let input = Input {
                direction: if frame < 90 { 0.0 } else { 1.0 },
                jump: frame % 60 == 0,
                attack: frame % 45 == 0,
                ..Input::default()
            };
            game.update(input, STEP);
        }
        let position = game.position();
        assert!(
            position.x.is_finite() && position.y.is_finite(),
            "room {index}: position {position}"
        );
        assert!(
            (-40.0..=WIDTH + 40.0).contains(&position.x) && position.y < HEIGHT + 40.0,
            "room {index}: player escaped to {position}"
        );
        assert!(game.health > 0, "room {index}");
    }
}

#[test]
fn identical_inputs_produce_identical_runs() {
    let (levels, start) = campaign();
    let run = || {
        let mut game = Game::with_levels(Save::default(), levels.clone());
        game.enter(start + 4, vec2(96.0, 446.0));
        for frame in 0..900 {
            game.update(
                Input {
                    direction: ((frame / 100) % 3) as f32 - 1.0,
                    jump: frame % 70 == 0,
                    attack: frame % 33 == 0,
                    dash: frame % 211 == 0,
                    ..Input::default()
                },
                STEP,
            );
        }
        fingerprint(&game)
    };
    assert_eq!(run(), run());
}

#[test]
fn ground_route_crosses_the_whole_campaign_in_order() {
    let (levels, start) = campaign();
    let mut game = Game::with_levels(Save::default(), levels.clone());
    game.debug.invulnerable = true;
    let end = start + 500;
    for room in start..end {
        // Les combats de boss sont couverts par d'autres tests : ici, seule la route est mesuree.
        if levels.rooms[room].enemies.iter().any(|enemy| enemy.boss) {
            game.save.defeated_bosses.push(room);
        }
        game.enter(room, vec2(96.0, 446.0));
        let mut crossed = room + 1 == end;
        for _ in 0..1200 {
            if crossed {
                break;
            }
            let portal = game
                .portals()
                .into_iter()
                .find(|(_, target, _)| *target == room + 1)
                .map(|(point, _, _)| point);
            let Some(point) = portal else {
                panic!("room {room}: no forward portal");
            };
            let near = (game.position() - point).length() < 40.0;
            game.update(
                Input {
                    direction: if near { 0.0 } else { 1.0 },
                    interact: near,
                    ..Input::default()
                },
                STEP,
            );
            crossed = game.room == room + 1;
        }
        assert!(crossed, "room {room}: ground route is blocked");
    }
}

#[test]
fn every_biome_keeps_text_and_platforms_readable() {
    let (levels, _) = campaign();
    let luminance = |rgb: [u8; 3]| {
        (0.299 * f32::from(rgb[0]) + 0.587 * f32::from(rgb[1]) + 0.114 * f32::from(rgb[2])) / 255.0
    };
    for (index, biome) in levels.biomes.iter().enumerate() {
        let background = luminance(biome.background);
        // Les plateformes et l'accent doivent se detacher du fond sans calcul de couleur complet.
        assert!(
            luminance(biome.stone) - background >= 0.15,
            "biome {index}: platforms blend into the background"
        );
        assert!(
            luminance(biome.accent) - background >= 0.35,
            "biome {index}: accent is too dim"
        );
    }
}

#[test]
fn legacy_saves_load_with_every_new_field_defaulted() {
    let save: Save = serde_json::from_str(
        r#"{"checkpoint":2,"double_jump":true,"won":false,"life_shard":true,"visited":[true,true,true,false,false]}"#,
    )
    .unwrap();
    let save = save.sanitize();
    assert_eq!((save.shards, save.deaths, save.talents.len()), (0, 0, 0));
    let game = Game::new(save);
    assert_eq!(game.max_health(), 6);
    assert!(game.save.visited.len() >= 7);
}
