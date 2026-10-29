//! Couches 3D du decor : relief des plateformes, mur de fond, cristaux et debris au premier plan.
//!
//! Le plan z = 0 de la camera 3D coincide pixel pour pixel avec le monde 2D : sprites et
//! hitboxes restent inchanges, seul le decor gagne de la profondeur (z > 0 = vers le fond).

use crate::game::{Game, COLS, PLAYER_SIZE, TILE, WIDTH};
use crate::render::VIEW_HEIGHT;
use macroquad::prelude::*;
use macroquad_platformer::Tile;

const FOV: f32 = 40.0 * std::f32::consts::PI / 180.0;
// Le monde 2D est affiche dans le rectangle (0, -56, WIDTH, VIEW_HEIGHT).
const SCREEN_CENTER: Vec2 = Vec2::new(WIDTH * 0.5, -56.0 + VIEW_HEIGHT * 0.5);
const RELIEF_DEPTH: f32 = TILE * 2.0;
const WALL_DEPTH: f32 = 1300.0;
const WALL_SPACING: f32 = 160.0;
// Profondeurs des nappes de brume qui separent les plans du decor.
const HAZE_DEPTHS: [f32; 3] = [450.0, 800.0, 1100.0];
const CRYSTALS: usize = 9;
const SHARDS: usize = 4;
// Eclairage fixe (vers la source, camera du cote z < 0) pour des facettes lisibles.
const TO_LIGHT: Vec3 = Vec3::new(-0.40, -0.60, -0.69);
// Un lot reste sous la capacite d'un appel de dessin de Macroquad, sinon il est tronque.
const BATCH_VERTICES: usize = 1600;
const BATCH_INDICES: usize = 2400;
const _: () = assert!(BATCH_VERTICES < u16::MAX as usize && BATCH_INDICES < 5000);

/// Couleurs du biome courant, deja resolues par le rendu 2D.
#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Color,
    pub stone: Color,
    pub accent: Color,
    /// Choisit la famille de formes du fond (architecture du biome).
    pub theme: usize,
}

/// La cible doit porter un tampon de profondeur pour que les facettes se masquent entre elles.
pub fn target(width: u32, height: u32) -> RenderTarget {
    let target = render_target_ex(
        width,
        height,
        RenderTargetParams {
            depth: true,
            ..Default::default()
        },
    );
    target.texture.set_filter(FilterMode::Nearest);
    target
}

/// Secousse de la camera du monde, partagee par la vue 2D et les couches 3D.
pub fn shake_offset(game: &Game, motion: bool) -> Vec2 {
    if game.shake > 0.0 && motion {
        vec2((game.time * 110.0).sin(), (game.time * 90.0).cos()) * game.shake * 12.0
    } else {
        Vec2::ZERO
    }
}

fn eye_distance() -> f32 {
    VIEW_HEIGHT * 0.5 / (FOV * 0.5).tan()
}

/// Pseudo-hasard stable : une salle garde le meme decor d'une image a l'autre.
fn noise(seed: u32) -> f32 {
    let mut value = seed.wrapping_mul(0x9E37_79B1);
    value ^= value >> 15;
    value = value.wrapping_mul(0x85EB_CA6B);
    value ^= value >> 13;
    (value & 0xFFFF) as f32 / 65535.0
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color::new(color.r, color.g, color.b, alpha)
}

fn scaled(color: Color, factor: f32) -> Color {
    Color::new(
        color.r * factor,
        color.g * factor,
        color.b * factor,
        color.a,
    )
}

fn mixed(from: Color, to: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    Color::new(
        from.r + (to.r - from.r) * amount,
        from.g + (to.g - from.g) * amount,
        from.b + (to.b - from.b) * amount,
        from.a,
    )
}

/// Faces d'une plateforme visibles depuis l'oeil : gauche, droite, haut, bas.
fn visible_sides(eye: Vec2, rect: Rect, exposed: [bool; 4]) -> [bool; 4] {
    [
        exposed[0] && eye.x < rect.x,
        exposed[1] && eye.x > rect.right(),
        exposed[2] && eye.y < rect.y,
        exposed[3] && eye.y > rect.bottom(),
    ]
}

/// Octaedre allonge : huit facettes triangulaires autour de `center` (y vers le bas).
fn crystal_faces(center: Vec3, radius: f32, height: f32, angle: f32) -> [[Vec3; 3]; 8] {
    let (sin, cos) = angle.sin_cos();
    let ring = [(radius, 0.0), (0.0, radius), (-radius, 0.0), (0.0, -radius)]
        .map(|(x, z)| center + vec3(x * cos - z * sin, 0.0, x * sin + z * cos));
    let top = center - vec3(0.0, height, 0.0);
    let bottom = center + vec3(0.0, height, 0.0);
    std::array::from_fn(|face| {
        let (a, b) = (ring[face % 4], ring[(face + 1) % 4]);
        if face < 4 {
            [top, a, b]
        } else {
            [bottom, b, a]
        }
    })
}

/// Facettes d'un decor du fond selon son type : 0 cristal, 1 pilier, 2 pyramide.
fn shape_faces(kind: usize, center: Vec3, radius: f32, angle: f32) -> Vec<[Vec3; 3]> {
    if kind.is_multiple_of(3) {
        return crystal_faces(center, radius, radius * 1.9, angle).to_vec();
    }
    let (sin, cos) = angle.sin_cos();
    let corner = |x: f32, z: f32, y: f32| center + vec3(x * cos - z * sin, y, x * sin + z * cos);
    if kind % 3 == 1 {
        // Pilier carre, plus etroit que haut : le sommet et les quatre flancs.
        let (r, h) = (radius * 0.6, radius * 3.2);
        let top = [(r, r), (-r, r), (-r, -r), (r, -r)].map(|(x, z)| corner(x, z, -h));
        let bottom = [(r, r), (-r, r), (-r, -r), (r, -r)].map(|(x, z)| corner(x, z, h));
        let mut faces = vec![[top[0], top[1], top[2]], [top[0], top[2], top[3]]];
        for side in 0..4 {
            let next = (side + 1) % 4;
            faces.push([top[side], top[next], bottom[next]]);
            faces.push([top[side], bottom[next], bottom[side]]);
        }
        faces
    } else {
        // Pyramide a base carree, pointe vers le haut.
        let apex = center - vec3(0.0, radius * 2.2, 0.0);
        let base = [
            (radius, radius),
            (-radius, radius),
            (-radius, -radius),
            (radius, -radius),
        ]
        .map(|(x, z)| corner(x, z, radius * 0.8));
        (0..4)
            .map(|side| [apex, base[side], base[(side + 1) % 4]])
            .collect()
    }
}

/// Luminosite d'une facette, orientee vers l'exterieur du cristal.
fn facet_light(face: &[Vec3; 3], center: Vec3) -> f32 {
    let normal = (face[1] - face[0])
        .cross(face[2] - face[0])
        .normalize_or_zero();
    let middle = (face[0] + face[1] + face[2]) / 3.0;
    let outward = if normal.dot(middle - center) < 0.0 {
        -normal
    } else {
        normal
    };
    0.25 + 0.75 * outward.dot(TO_LIGHT).max(0.0)
}

/// Position apparente (pixels), profondeur et rayon apparent d'un element du decor.
struct Placement {
    apparent: Vec2,
    depth: f32,
    radius: f32,
    phase: f32,
}

fn placement(room: usize, index: usize, foreground: bool) -> Placement {
    let seed = room as u32 * 131 + index as u32 * 17 + if foreground { 5000 } else { 0 };
    let unit = |channel: u32| noise(seed + channel * 977);
    if foreground {
        // Les debris restent sur les bords pour ne jamais masquer le joueur.
        let edge = unit(0) * 130.0;
        Placement {
            apparent: vec2(
                if index.is_multiple_of(2) {
                    edge
                } else {
                    WIDTH - edge
                },
                60.0 + unit(1) * 420.0,
            ),
            depth: -420.0 + unit(2) * 220.0,
            radius: 26.0 + unit(3) * 34.0,
            phase: unit(4) * std::f32::consts::TAU,
        }
    } else {
        Placement {
            apparent: vec2(unit(0) * WIDTH, 40.0 + unit(1) * 400.0),
            depth: 260.0 + unit(2) * 840.0,
            radius: 12.0 + unit(3) * 26.0,
            phase: unit(4) * std::f32::consts::TAU,
        }
    }
}

/// L'oeil se decale avec le joueur : le plan z = 0 reste fixe, les autres plans glissent.
struct Stage {
    center: Vec2,
    eye: Vec2,
}

impl Stage {
    fn new(game: &Game, motion: bool) -> Self {
        let eye = if motion {
            let player = game.position() + PLAYER_SIZE * 0.5;
            ((player - SCREEN_CENTER) * 0.35).clamp(vec2(-170.0, -100.0), vec2(170.0, 100.0))
        } else {
            Vec2::ZERO
        };
        Self {
            center: SCREEN_CENTER + shake_offset(game, motion),
            eye,
        }
    }

    fn eye_position(&self) -> Vec2 {
        self.center + self.eye
    }

    /// Equivalent d'une camera decentree : un point a la profondeur z glisse de eye * z / d.
    fn lift(&self, point: Vec2, depth: f32) -> Vec3 {
        let shift = self.eye * (depth / eye_distance());
        vec3(point.x + shift.x, point.y + shift.y, depth)
    }
}

/// Accumule des triangles et les envoie par lots bornes.
struct Batch {
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
}

impl Batch {
    fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn reserve(&mut self, vertices: usize) {
        if self.vertices.len() + vertices > BATCH_VERTICES || self.indices.len() + 6 > BATCH_INDICES
        {
            self.flush();
        }
    }

    fn triangle(&mut self, corners: [Vec3; 3], color: Color) {
        self.reserve(3);
        let base = self.vertices.len() as u16;
        for corner in corners {
            self.vertices.push(Vertex::new2(corner, Vec2::ZERO, color));
        }
        self.indices.extend([base, base + 1, base + 2]);
    }

    fn quad(&mut self, corners: [Vec3; 4], colors: [Color; 4]) {
        self.reserve(4);
        let base = self.vertices.len() as u16;
        for (corner, color) in corners.into_iter().zip(colors) {
            self.vertices.push(Vertex::new2(corner, Vec2::ZERO, color));
        }
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn flush(&mut self) {
        if self.vertices.is_empty() {
            return;
        }
        draw_mesh(&Mesh {
            vertices: std::mem::take(&mut self.vertices),
            indices: std::mem::take(&mut self.indices),
            texture: None,
        });
    }
}

/// Dessine les couches 3D d'une image sur la cible du monde, sans toucher aux sprites 2D.
pub struct Layers<'a> {
    target: &'a RenderTarget,
    game: &'a Game,
    palette: Palette,
    stage: Stage,
    motion: bool,
}

impl<'a> Layers<'a> {
    pub fn new(target: &'a RenderTarget, game: &'a Game, palette: Palette, motion: bool) -> Self {
        Self {
            target,
            game,
            palette,
            stage: Stage::new(game, motion),
            motion,
        }
    }

    fn camera(&self) -> Camera3D {
        Camera3D {
            position: vec3(self.stage.center.x, self.stage.center.y, -eye_distance()),
            target: vec3(self.stage.center.x, self.stage.center.y, 0.0),
            // Axe y vers le bas et oeil du cote z < 0 : meme orientation que la camera 2D.
            up: vec3(0.0, -1.0, 0.0),
            fovy: FOV,
            aspect: None,
            projection: Projection::Perspective,
            render_target: Some(self.target.clone()),
            viewport: None,
            z_near: 1.0,
            z_far: 4000.0,
        }
    }

    // La camera 2D du monde est restauree, secousse comprise, des que la couche est dessinee.
    fn draw(&self, content: impl FnOnce(&mut Batch)) {
        push_camera_state();
        set_camera(&self.camera());
        let mut batch = Batch::new();
        content(&mut batch);
        batch.flush();
        pop_camera_state();
    }

    /// Mur de fond quadrille, tres loin derriere tout le decor 2D.
    pub fn wall(&self) {
        let Palette { accent, .. } = self.palette;
        let color = Color::new(accent.r, accent.g, accent.b, 0.11);
        push_camera_state();
        set_camera(&self.camera());
        let mut x = -1000.0;
        while x <= 2000.0 {
            draw_line_3d(
                self.stage.lift(vec2(x, -1000.0), WALL_DEPTH),
                self.stage.lift(vec2(x, 1500.0), WALL_DEPTH),
                color,
            );
            x += WALL_SPACING;
        }
        let mut y = -1000.0;
        while y <= 1500.0 {
            draw_line_3d(
                self.stage.lift(vec2(-1000.0, y), WALL_DEPTH),
                self.stage.lift(vec2(2000.0, y), WALL_DEPTH),
                color,
            );
            y += WALL_SPACING;
        }
        pop_camera_state();
    }

    /// Decors flottants du biome a plusieurs profondeurs, separes par des nappes de brume.
    pub fn crystals(&self) {
        let Palette {
            background,
            stone,
            accent,
            theme,
        } = self.palette;
        let time = if self.motion { self.game.time } else { 0.0 };
        let distance = eye_distance();
        // Chaque element porte sa profondeur ; le tri garantit un melange alpha coherent.
        let mut items: Vec<(f32, Option<Placement>)> = HAZE_DEPTHS
            .iter()
            .map(|depth| (*depth, None))
            .chain((0..CRYSTALS).map(|index| {
                let item = placement(self.game.room, index, false);
                (item.depth, Some(item))
            }))
            .collect();
        items.sort_by(|first, second| second.0.total_cmp(&first.0));
        self.draw(|batch| {
            for (depth, item) in items {
                let scale = (distance + depth) / distance;
                let Some(item) = item else {
                    // Nappe de brume : transparente en haut, plus dense vers le sol.
                    let mist = mixed(background, accent, 0.25);
                    let (clear, dense) = (with_alpha(mist, 0.0), with_alpha(mist, 0.10));
                    let corner = |x: f32, y: f32| {
                        self.stage
                            .lift(SCREEN_CENTER + (vec2(x, y) - SCREEN_CENTER) * scale, depth)
                    };
                    batch.quad(
                        [
                            corner(-150.0, -60.0),
                            corner(WIDTH + 150.0, -60.0),
                            corner(WIDTH + 150.0, 700.0),
                            corner(-150.0, 700.0),
                        ],
                        [clear, clear, dense, dense],
                    );
                    continue;
                };
                let bob = (time * 0.8 + item.phase).sin() * 6.0;
                let apparent = item.apparent + vec2(0.0, bob);
                let position = SCREEN_CENTER + (apparent - SCREEN_CENTER) * scale;
                let center = self.stage.lift(position, item.depth);
                // Un element sur trois reprend la forme voisine pour varier les silhouettes.
                let kind = theme + usize::from(item.phase.to_bits() % 3 == 0);
                let faces = shape_faces(kind, center, item.radius * scale, time * 0.4 + item.phase);
                let base = mixed(stone, accent, 0.65);
                let fog = (item.depth / 1300.0).clamp(0.0, 0.75);
                for face in &faces {
                    let color = mixed(scaled(base, facet_light(face, center)), background, fog);
                    batch.triangle(*face, color);
                }
            }
        });
    }

    /// Extrusion des plateformes vers le fond : seules les faces exposees et visibles sont tracees.
    pub fn relief(&self) {
        let Palette {
            background, stone, ..
        } = self.palette;
        let eye = self.stage.eye_position();
        let tiles = &self.game.tiles;
        // Hors de la salle, on suppose du plein : les bords de l'ecran n'ont pas de flanc.
        let solid = |col: i32, row: i32| {
            col < 0
                || row < 0
                || col >= COLS as i32
                || matches!(
                    tiles.get(row as usize * COLS + col as usize),
                    Some(Tile::Solid) | None
                )
        };
        self.draw(|batch| {
            for (index, tile) in tiles.iter().enumerate() {
                if *tile != Tile::Solid {
                    continue;
                }
                let (col, row) = ((index % COLS) as i32, (index / COLS) as i32);
                let rect = Rect::new(col as f32 * TILE, row as f32 * TILE, TILE, TILE);
                let visible = visible_sides(
                    eye,
                    rect,
                    [
                        !solid(col - 1, row),
                        !solid(col + 1, row),
                        !solid(col, row - 1),
                        !solid(col, row + 1),
                    ],
                );
                // Arete avant, arete arriere et luminosite de chaque face (haut eclaire, bas sombre).
                let sides = [
                    (rect.point(), vec2(rect.x, rect.bottom()), 0.7),
                    (
                        vec2(rect.right(), rect.y),
                        vec2(rect.right(), rect.bottom()),
                        0.55,
                    ),
                    (rect.point(), vec2(rect.right(), rect.y), 1.0),
                    (
                        vec2(rect.x, rect.bottom()),
                        vec2(rect.right(), rect.bottom()),
                        0.4,
                    ),
                ];
                for (side, (from, to, light)) in sides.into_iter().enumerate() {
                    if !visible[side] {
                        continue;
                    }
                    let near = scaled(stone, light);
                    let far = mixed(scaled(stone, light * 0.45), background, 0.5);
                    batch.quad(
                        [
                            self.stage.lift(from, 0.0),
                            self.stage.lift(to, 0.0),
                            self.stage.lift(to, RELIEF_DEPTH),
                            self.stage.lift(from, RELIEF_DEPTH),
                        ],
                        [near, near, far, far],
                    );
                }
            }
        });
    }

    /// Eclats sombres et translucides tres proches de la camera, en bordure d'ecran.
    pub fn foreground(&self) {
        let Palette { accent, .. } = self.palette;
        let time = if self.motion { self.game.time } else { 0.0 };
        let distance = eye_distance();
        let mut placements: Vec<_> = (0..SHARDS)
            .map(|index| placement(self.game.room, index, true))
            .collect();
        placements.sort_by(|first, second| second.depth.total_cmp(&first.depth));
        self.draw(|batch| {
            for item in placements {
                let scale = (distance + item.depth) / distance;
                let drift = vec2(0.0, (time * 0.5 + item.phase).sin() * 10.0);
                let position = SCREEN_CENTER + (item.apparent + drift - SCREEN_CENTER) * scale;
                let center = self.stage.lift(position, item.depth);
                let faces = crystal_faces(
                    center,
                    item.radius * scale * 0.5,
                    item.radius * scale * 1.6,
                    time * 0.25 + item.phase,
                );
                for face in &faces {
                    let light = facet_light(face, center);
                    batch.triangle(
                        *face,
                        Color::new(
                            accent.r * light * 0.5,
                            accent.g * light * 0.5,
                            accent.b * light * 0.5,
                            0.22,
                        ),
                    );
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stage(eye: Vec2) -> Stage {
        Stage {
            center: SCREEN_CENTER,
            eye,
        }
    }

    #[test]
    fn the_world_plane_never_moves_with_the_eye() {
        let point = vec2(321.0, 123.0);
        let lifted = stage(vec2(120.0, -80.0)).lift(point, 0.0);
        assert_eq!((lifted.x, lifted.y, lifted.z), (point.x, point.y, 0.0));
    }

    #[test]
    fn deeper_points_slide_toward_the_eye_and_nearer_ones_away() {
        let stage = stage(vec2(100.0, 0.0));
        let deep = stage.lift(vec2(0.0, 0.0), RELIEF_DEPTH);
        let near = stage.lift(vec2(0.0, 0.0), -RELIEF_DEPTH);
        assert!(deep.x > 0.0 && near.x < 0.0);
        assert_eq!(deep.y, 0.0);
    }

    #[test]
    fn only_exposed_sides_facing_the_eye_are_visible() {
        let tile = Rect::new(100.0, 100.0, 32.0, 32.0);
        let all = [true; 4];
        assert_eq!(
            visible_sides(vec2(0.0, 0.0), tile, all),
            [true, false, true, false]
        );
        assert_eq!(
            visible_sides(vec2(500.0, 500.0), tile, all),
            [false, true, false, true]
        );
        assert_eq!(visible_sides(vec2(0.0, 0.0), tile, [false; 4]), [false; 4]);
        // Un oeil face a la plateforme ne voit aucun flanc.
        assert_eq!(visible_sides(vec2(116.0, 116.0), tile, all), [false; 4]);
    }

    #[test]
    fn crystals_have_eight_lit_facets_around_their_center() {
        let center = vec3(10.0, 20.0, 300.0);
        let faces = crystal_faces(center, 10.0, 20.0, 0.7);
        assert_eq!(faces.len(), 8);
        for face in &faces {
            let light = facet_light(face, center);
            assert!((0.25..=1.0).contains(&light));
            for corner in face {
                assert!((*corner - center).length() <= 20.01);
            }
        }
    }

    #[test]
    fn decor_placement_is_stable_and_stays_inside_its_bounds() {
        for room in [0, 7, 506] {
            for index in 0..CRYSTALS {
                let first = placement(room, index, false);
                let second = placement(room, index, false);
                assert_eq!(first.apparent, second.apparent);
                assert!((0.0..=WIDTH).contains(&first.apparent.x));
                assert!((260.0..=1100.0).contains(&first.depth));
            }
            for index in 0..SHARDS {
                let shard = placement(room, index, true);
                assert!(shard.depth < 0.0 && shard.depth > -eye_distance());
                // Les debris restent sur les bords, hors de la zone de jeu centrale.
                assert!(shard.apparent.x <= 130.0 || shard.apparent.x >= WIDTH - 130.0);
            }
        }
    }

    #[test]
    fn background_shapes_have_expected_facets_and_stay_near_their_center() {
        let center = vec3(0.0, 0.0, 500.0);
        let counts = [8, 10, 4];
        for (kind, count) in counts.into_iter().enumerate() {
            let faces = shape_faces(kind, center, 20.0, 0.4);
            assert_eq!(faces.len(), count);
            for corner in faces.iter().flatten() {
                assert!((*corner - center).length() <= 80.0);
            }
        }
        // Les themes au-dela de trois reprennent les formes dans l'ordre.
        assert_eq!(shape_faces(4, center, 20.0, 0.0).len(), 10);
    }

    #[test]
    fn haze_layers_sit_between_the_wall_and_the_foreground() {
        assert!(HAZE_DEPTHS
            .iter()
            .all(|depth| *depth > 0.0 && *depth < WALL_DEPTH));
        assert!(HAZE_DEPTHS.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn camera_distance_maps_the_world_plane_onto_the_view_height() {
        let height = 2.0 * eye_distance() * (FOV * 0.5).tan();
        assert!((height - VIEW_HEIGHT).abs() < 0.01);
    }
}
