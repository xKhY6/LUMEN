//! Dessine l'etat du jeu sans modifier la simulation ni les zones de collision.

use crate::characters::{Character, Skin};
use crate::content::{Content, Manifest, RoomPalette, Sprite};
use crate::game::*;
use crate::ui;
use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation};
use macroquad::prelude::*;
use macroquad_platformer::Tile;
use serde::{Deserialize, Serialize};

// Les 56 pixels supplementaires sont reserves au HUD au-dessus du monde de 544 pixels.
pub const VIEW_HEIGHT: f32 = 600.0;
pub const INK: Color = Color::new(0.043, 0.067, 0.063, 1.0);
pub const PAPER: Color = Color::new(0.886, 0.914, 0.827, 1.0);
pub const MINT: Color = Color::new(0.380, 0.886, 0.776, 1.0);
pub const GOLD: Color = Color::new(0.965, 0.757, 0.443, 1.0);
pub const CORAL: Color = Color::new(0.937, 0.431, 0.443, 1.0);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EffectsProfile {
    Clean,
    #[default]
    Cinematic,
    Retro,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VisualSettings {
    pub profile: EffectsProfile,
    pub reduced_motion: bool,
    /// Desactive les couches 3D sans toucher aux autres effets.
    pub flat: bool,
}

impl VisualSettings {
    pub fn from_arguments(arguments: &[String]) -> Result<Self, String> {
        Self::default().with_arguments(arguments)
    }

    /// Les options de ligne de commande ne remplacent que ce qu'elles mentionnent.
    pub fn with_arguments(mut self, arguments: &[String]) -> Result<Self, String> {
        if let Some(index) = arguments.iter().position(|argument| argument == "--fx") {
            self.profile = match arguments.get(index + 1).map(String::as_str) {
                Some("clean") => EffectsProfile::Clean,
                Some("cinematic") => EffectsProfile::Cinematic,
                Some("retro") => EffectsProfile::Retro,
                _ => return Err("--fx attend clean, cinematic ou retro".into()),
            };
        }
        self.reduced_motion |= arguments.iter().any(|argument| argument == "--reduced-fx");
        self.flat |= arguments.iter().any(|argument| argument == "--no-3d");
        Ok(self)
    }

    pub fn motion(&self) -> bool {
        self.profile != EffectsProfile::Clean && !self.reduced_motion
    }

    pub fn depth(&self) -> bool {
        self.profile != EffectsProfile::Clean && !self.flat
    }
}

/// Frames et animation d'un heros ou d'un ennemi humain fournis par le pack.
struct SpriteSet {
    frames: Vec<Texture2D>,
    animation: Sprite,
}

/// Ressources GPU creees une seule fois et reutilisees a chaque image.
pub struct Art {
    pub font: Font,
    pub player: Texture2D,
    player_frames: Vec<Texture2D>,
    enemy_frames: Vec<Texture2D>,
    // Un emplacement par personnage ou apparence ; None retombe sur Lumen ou la creature.
    hero_sets: Vec<Option<SpriteSet>>,
    enemy_sets: Vec<Option<SpriteSet>>,
    pub heart: Texture2D,
    pub fern: Texture2D,
    pub crystal: Texture2D,
    pub tiles: Vec<Texture2D>,
    glow: Texture2D,
    light_material: Material,
    post_material: Material,
    pub effects: VisualSettings,
    palettes: Vec<RoomPalette>,
    player_animation: Sprite,
    enemy_animation: Sprite,
}

// Le masque s'annule progressivement au bord pour eviter un disque ou un rectangle visible.
fn light_falloff(distance_squared: f32) -> f32 {
    (1.0 - distance_squared).clamp(0.0, 1.0).powi(3)
}

fn glow_image() -> Image {
    let mut image = Image::gen_image_color(128, 128, WHITE);
    for row in 0..128 {
        for column in 0..128 {
            let offset = vec2(
                (column as f32 + 0.5) / 64.0 - 1.0,
                (row as f32 + 0.5) / 64.0 - 1.0,
            );
            image.set_pixel(
                column,
                row,
                Color::new(1.0, 1.0, 1.0, light_falloff(offset.length_squared())),
            );
        }
    }
    image
}

// Les attributs et matrices suivent le format de dessin standard de Macroquad.
const LIGHT_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
uniform mat4 Model;
uniform mat4 Projection;
varying lowp vec4 tint;
varying mediump vec2 uv;
void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    uv = texcoord;
    tint = color0 / 255.0;
}
"#;

const LIGHT_FRAGMENT: &str = r#"#version 100
precision mediump float;
uniform sampler2D Texture;
varying lowp vec4 tint;
varying mediump vec2 uv;
void main() {
    gl_FragColor = texture2D(Texture, uv) * tint;
}
"#;

// La cible contient deja le melange alpha des sprites et glyphes : la presenter opaque,
// sans recomposer leurs bords. Les UV sont inverses et le HUD contourne tous les filtres.
const POST_FRAGMENT: &str = r#"#version 100
precision mediump float;
uniform sampler2D Texture;
uniform vec4 Frame;
uniform vec4 Shock;
uniform vec4 World;
uniform vec2 UiBounds;
varying lowp vec4 tint;
varying mediump vec2 uv;
vec3 scene_sample(vec2 point) {
    return texture2D(Texture, clamp(point, vec2(0.001), vec2(0.999, 0.905))).rgb;
}
vec3 bright(vec2 point) {
    vec3 value = scene_sample(point);
    return value * smoothstep(0.55, 0.95, max(value.r, max(value.g, value.b)));
}
void main() {
    vec4 original = texture2D(Texture, uv);
    if (World.w < 0.5 || uv.y > 1.0 - UiBounds.x || uv.y < UiBounds.y) { gl_FragColor = vec4(original.rgb, 1.0) * tint; return; }
    vec2 point = uv;
    float motion = 1.0 - Frame.w;
    if (World.x == 2.0) {
        point.x += sin(uv.y * 95.0 + Frame.x * 3.0) * 0.0013 * motion;
    } else if (World.x == 6.0) {
        point.x += sin(uv.y * 70.0 + Frame.x * 2.0) * 0.0009 * motion;
    }
    vec2 delta = (point - Shock.xy) * vec2(1.6, 1.0);
    float distance_from_hit = length(delta);
    float ring = exp(-pow((distance_from_hit - Shock.z * 0.23) * 45.0, 2.0));
    point += delta / max(distance_from_hit, 0.001) * ring * Shock.w * (1.0 - Shock.z) * 0.003 * motion;
    float fringe = (0.0007 + Frame.y * 0.002) * motion;
    vec3 base = scene_sample(point);
    base.r = scene_sample(point + vec2(fringe, 0.0)).r;
    base.b = scene_sample(point - vec2(fringe, 0.0)).b;
    vec2 travel = vec2(Frame.z * 0.002 * motion, 0.0);
    base = base * 0.88 + (scene_sample(point + travel) + scene_sample(point - travel)) * 0.06;
    vec2 step_size = vec2(0.0021, 0.0033);
    vec3 bloom = bright(point + step_size) + bright(point - step_size);
    bloom += bright(point + vec2(step_size.x, -step_size.y)) + bright(point + vec2(-step_size.x, step_size.y));
    bloom += bright(point + vec2(step_size.x * 2.0, 0.0)) + bright(point - vec2(step_size.x * 2.0, 0.0));
    bloom += bright(point + vec2(0.0, step_size.y * 2.0)) + bright(point - vec2(0.0, step_size.y * 2.0));
    base += bloom * 0.018;
    vec2 centered = vec2(uv.x, (1.0 - uv.y - 0.0933333) / 0.9066667) * 2.0 - 1.0;
    base *= 1.0 - smoothstep(0.35, 1.6, dot(centered, centered)) * 0.16;
    float luminance = dot(base, vec3(0.299, 0.587, 0.114));
    float danger = 1.0 - smoothstep(0.15, 0.45, World.z);
    base = mix(base, vec3(luminance), danger * 0.22);
    base.r += danger * smoothstep(0.4, 1.4, dot(centered, centered)) * 0.035;
    base = max((base - 0.5) * 1.025 + 0.5, vec3(0.0));
    if (World.y > 0.5) {
        base = floor(base * 15.0 + 0.5) / 15.0;
        base *= 0.96 + 0.04 * sin(uv.y * 600.0 * 3.14159265);
    }
    float noise = fract(sin(dot(floor(uv * vec2(960.0, 600.0)), vec2(12.9898, 78.233))) * 4375.8);
    base += (noise - 0.5) * 0.006;
    gl_FragColor = vec4(clamp(base, 0.0, 1.0), 1.0) * tint;
}
"#;

fn post_material() -> Result<Material, String> {
    load_material(
        ShaderSource::Glsl {
            vertex: LIGHT_VERTEX,
            fragment: POST_FRAGMENT,
        },
        MaterialParams {
            uniforms: vec![
                UniformDesc::new("Frame", UniformType::Float4),
                UniformDesc::new("Shock", UniformType::Float4),
                UniformDesc::new("World", UniformType::Float4),
                UniformDesc::new("UiBounds", UniformType::Float2),
            ],
            pipeline_params: PipelineParams {
                color_blend: None,
                alpha_blend: None,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .map_err(|error| format!("Post-traitement: {error}"))
}

// Limites dans les UV inverses de la cible ; le bandeau inferieur n'existe que pendant un message.
fn hud_uv_bounds(game: Option<&Game>) -> (f32, f32) {
    (
        56.0 / VIEW_HEIGHT,
        if game.is_some_and(|game| game.notice_time > 0.0) {
            43.0 / VIEW_HEIGHT
        } else {
            0.0
        },
    )
}

// Les lignes ASCII doivent avoir la meme largeur ; les symboles hors palette sont transparents.
fn bitmap_image(rows: &[&str], palette: &[(u8, Color)]) -> Image {
    let mut image = Image::gen_image_color(
        rows[0].len() as u16,
        rows.len() as u16,
        Color::new(0.0, 0.0, 0.0, 0.0),
    );
    for (row_index, row) in rows.iter().enumerate() {
        for (col_index, symbol) in row.bytes().enumerate() {
            if let Some((_, color)) = palette.iter().find(|(key, _)| *key == symbol) {
                image.set_pixel(col_index as u32, row_index as u32, *color);
            }
        }
    }
    image
}

fn bitmap(rows: &[&str], palette: &[(u8, Color)]) -> Texture2D {
    pixel_texture(&bitmap_image(rows, palette))
}

fn pixel_texture(image: &Image) -> Texture2D {
    let texture = Texture2D::from_image(image);
    // Eviter le lissage des pixels lors de l'agrandissement des sprites.
    texture.set_filter(FilterMode::Nearest);
    texture
}

// Deformer les pixels une seule fois au chargement ; les hitboxes restent celles de la simulation.
fn animated_image(base: &Image, frame: usize, player: bool) -> Image {
    let mut image = Image::gen_image_color(base.width, base.height, Color::new(0.0, 0.0, 0.0, 0.0));
    let height = base.height as u32;
    let width = base.width as u32;
    for row in 0..height {
        for column in 0..width {
            let side = if column < width / 2 { -1 } else { 1 };
            let shift = match frame {
                1 | 2 if row >= height * 3 / 4 => side * if frame == 1 { 1 } else { -1 },
                1 | 2 if player && row >= height / 2 && column < 5 => {
                    if frame == 1 {
                        -1
                    } else {
                        1
                    }
                }
                3 if row >= height * 3 / 4 => -side,
                // Saut montant : jambes ramenees sous le corps.
                6 if row >= height * 3 / 4 => -side,
                // Chute : jambes et bras ecartes pour amortir.
                7 if row >= height * 3 / 4 => side,
                7 if player
                    && row >= height / 3
                    && row < height / 2
                    && (column < 3 || column >= width - 3) =>
                {
                    side
                }
                // Glissade murale : buste colle au mur, genoux flechis.
                8 if row >= height * 3 / 4 => side,
                8 if row < height * 3 / 4 => -1,
                4 if row >= height / 3 && row < height * 3 / 4 => 1,
                5 if row < height * 3 / 4 => 1,
                _ => 0,
            };
            let target = column as i32 + shift;
            let lift = match frame {
                3 if row >= height * 3 / 4 => 1,
                6 if row >= height * 3 / 4 => 2,
                _ => 0,
            };
            // Reception / armement : le haut du corps s'affaisse de deux pixels.
            let drop = if frame == 9 && row < height * 3 / 4 {
                2
            } else {
                0
            };
            let target_row = row as i32 - lift + drop;
            if (0..width as i32).contains(&target) && (0..height as i32).contains(&target_row) {
                let color = base.get_pixel(column, row);
                if color.a > 0.0 {
                    image.set_pixel(target as u32, target_row as u32, color);
                }
            }
        }
    }
    image
}

#[derive(Debug)]
struct Pose {
    frame: usize,
    size: Vec2,
    rotation: f32,
}

// La priorite des actions evite de faire marcher les jambes pendant une ruee ou une frappe.
fn player_pose(game: &Game, animation: &Sprite) -> Pose {
    let (frame, scale, rotation) = if game.dash_time > 0.0 {
        (5, vec2(1.15, 0.82), game.facing * 0.22)
    } else if game.damage_flash > 0.0 {
        (3, vec2(0.9, 1.05), -game.facing * 0.16)
    } else if game.attack_time > 0.0 {
        let progress = (1.0 - game.attack_time / game.weapon().duration).clamp(0.0, 1.0);
        (
            4,
            vec2(1.06, 0.95),
            game.facing * (progress * std::f32::consts::PI).sin() * 0.18,
        )
    } else if game.wall_slide {
        // Le joueur s'appuie contre le mur : silhouette etiree et legere inclinaison vers la paroi.
        (8, vec2(0.92, 1.06), game.facing * 0.08)
    } else if !game.grounded {
        // Montee, apex puis chute : trois poses pour lire la trajectoire du saut.
        let frame = if game.velocity.y < -20.0 {
            6
        } else if game.velocity.y > 20.0 {
            7
        } else {
            3
        };
        (
            frame,
            if game.velocity.y < 0.0 {
                vec2(0.92, 1.08)
            } else {
                vec2(1.04, 0.96)
            },
            0.0,
        )
    } else if game.landing_squash > 0.0 {
        (9, vec2(1.0, 1.0), 0.0)
    } else if game.velocity.x.abs() > 20.0 {
        let phase = (game.time * animation.fps) as usize % animation.walk.len();
        (
            animation.walk[phase],
            vec2(1.0, 1.0 - (game.time * 20.0).sin().abs() * 0.035),
            game.facing * 0.04,
        )
    } else {
        (0, vec2(1.0, 1.0 + (game.time * 2.5).sin() * 0.018), 0.0)
    };
    // L'ecrasement s'estompe pendant LANDING_SQUASH_TIME, puis la pose normale reprend.
    let landing = (game.landing_squash / crate::game::LANDING_SQUASH_TIME).clamp(0.0, 1.0);
    let scale = scale * vec2(1.0 + landing * 0.12, 1.0 - landing * 0.18);
    Pose {
        frame,
        size: vec2(32.0, 48.0) * scale,
        rotation,
    }
}

/// Retourne le fond, la pierre et l'accent communs au decor et aux cartes.
pub fn room_colors(room: usize) -> (Color, Color, Color) {
    match room {
        0 => (
            Color::from_rgba(22, 34, 29, 255),
            Color::from_rgba(45, 62, 47, 255),
            MINT,
        ),
        1 => (
            Color::from_rgba(32, 28, 34, 255),
            Color::from_rgba(64, 49, 58, 255),
            CORAL,
        ),
        2 => (
            Color::from_rgba(38, 27, 25, 255),
            Color::from_rgba(78, 51, 42, 255),
            GOLD,
        ),
        3 => (
            Color::from_rgba(29, 35, 38, 255),
            Color::from_rgba(55, 68, 71, 255),
            PAPER,
        ),
        5 => (
            Color::from_rgba(23, 39, 36, 255),
            Color::from_rgba(62, 88, 69, 255),
            Color::from_rgba(189, 218, 117, 255),
        ),
        6 => (
            Color::from_rgba(27, 35, 49, 255),
            Color::from_rgba(66, 72, 97, 255),
            Color::from_rgba(112, 205, 225, 255),
        ),
        _ => (
            Color::from_rgba(24, 33, 34, 255),
            Color::from_rgba(47, 61, 57, 255),
            MINT,
        ),
    }
}

impl Art {
    // Les generateurs historiques sont reserves a l'export explicite des assets originaux.
    fn original() -> Self {
        let mut font = load_ttf_font_from_bytes(include_bytes!("../assets/Silkscreen-Regular.ttf"))
            .expect("embedded font");
        font.set_filter(FilterMode::Nearest);
        let player_image = bitmap_image(
            &[
                "................",
                "......pppp......",
                ".....pppppp.....",
                "....ppddddpp....",
                "....pddddddp....",
                "....pdeeedep....",
                "....pddddddp....",
                ".....pddddp.....",
                "....cccccccc....",
                "...ccccggcccc...",
                "...ccggggggcc...",
                "..cccggggggcc...",
                "..cccggggggcc...",
                ".ccccggggggcc...",
                ".ccccggggggcc...",
                ".ccccggggggcc...",
                ".ccccddggddcc...",
                "..cccddddddcc...",
                "..cccddddddcc...",
                "...ccdd..ddc....",
                "....ddd..ddd....",
                "....ddd..ddd....",
                "....ppp..ppp....",
                "................",
            ],
            &[
                (b'p', PAPER),
                (b'd', Color::from_rgba(57, 72, 71, 255)),
                (b'e', GOLD),
                (b'c', MINT),
                (b'g', Color::from_rgba(107, 131, 116, 255)),
            ],
        );
        let player = pixel_texture(&player_image);
        let enemy_image = bitmap_image(
            &[
                "................",
                "...dd......dd...",
                "...ddd....ddd...",
                "....dddddddd....",
                "...dddddddddd...",
                "...ddeeddeedd...",
                "...dddddddddd...",
                "....ddggggdd....",
                "...ddddggdddd...",
                "..dddddddddddd..",
                "..ddggddddggdd..",
                "..ddggddddggdd..",
                "...dddddddddd...",
                "...ddd....ddd...",
                "...dd......dd...",
                "................",
            ],
            &[
                (b'd', Color::from_rgba(148, 112, 116, 255)),
                (b'e', GOLD),
                (b'g', CORAL),
            ],
        );
        // Preparer les variantes en memoire CPU avant leur transfert unique vers le GPU.
        let player_frames = (0..PLAYER_FRAME_CODES)
            .map(|frame| pixel_texture(&animated_image(&player_image, frame, true)))
            .collect();
        let enemy_frames = ENEMY_FRAME_CODES
            .iter()
            .map(|frame| pixel_texture(&animated_image(&enemy_image, *frame, false)))
            .collect();
        let heart = bitmap(
            &[
                ".hh.hh.", "hhhhhhh", "hhhhhhh", ".hhhhh.", "..hhh..", "...h...",
            ],
            &[(b'h', CORAL)],
        );
        let fern = bitmap(
            &[
                ".......s........",
                "...ll..s..ll....",
                "....ll.s.ll.....",
                "..lll..s..lll...",
                "....ll.s.ll.....",
                ".llll..s..llll..",
                "...lll.s.lll....",
                ".....llsll......",
                "..llll.s.llll...",
                "....lllslll.....",
                "......sss.......",
                ".......s........",
            ],
            &[
                (b'l', Color::from_rgba(117, 160, 83, 255)),
                (b's', Color::from_rgba(61, 105, 69, 255)),
            ],
        );
        let crystal = bitmap(
            &[
                "......p.........",
                ".....pap........",
                ".....paap.......",
                "..p..paap.......",
                ".pap.paap..p....",
                ".paapaaaap.pap..",
                ".paaapaaaappap..",
                "..aaapaaaappap..",
                "..aaaapaaaapa...",
                "...aaaapaaaa....",
                "...aaaaaaaap....",
                "....aaaaaap.....",
            ],
            &[(b'p', PAPER), (b'a', Color::from_rgba(112, 205, 225, 255))],
        );
        let tiles = (0..NAMES.len())
            .map(|room| {
                let (_, stone, accent) = room_colors(room);
                let mut image = Image::gen_image_color(32, 32, stone);
                for row in 0..32 {
                    for col in 0..32 {
                        let hash = (row * 31 + col * 17 + room * 13) % 29;
                        let color = if row == 0 {
                            Color::new(stone.r + 0.14, stone.g + 0.14, stone.b + 0.12, 1.0)
                        } else if row == 15 || col == (if row < 15 { 1 } else { 17 }) {
                            Color::new(stone.r * 0.55, stone.g * 0.55, stone.b * 0.55, 1.0)
                        } else if hash < 2 {
                            Color::new(stone.r + 0.035, stone.g + 0.035, stone.b + 0.035, 1.0)
                        } else {
                            stone
                        };
                        image.set_pixel(col as u32, row as u32, color);
                    }
                }
                for col in (3..28).step_by(7) {
                    image.set_pixel(
                        col,
                        2,
                        Color::new(accent.r * 0.4, accent.g * 0.4, accent.b * 0.4, 1.0),
                    );
                }
                let texture = Texture2D::from_image(&image);
                texture.set_filter(FilterMode::Nearest);
                texture
            })
            .collect();
        // Creer le masque et le pipeline une fois, jamais dans la boucle de rendu.
        let glow = Texture2D::from_image(&glow_image());
        glow.set_filter(FilterMode::Linear);
        let light_material = light_material();
        Self {
            font,
            player,
            player_frames,
            enemy_frames,
            hero_sets: (0..Character::ALL.len()).map(|_| None).collect(),
            enemy_sets: (0..Skin::ALL.len()).map(|_| None).collect(),
            heart,
            fern,
            crystal,
            tiles,
            glow,
            light_material,
            post_material: post_material().expect("post material"),
            effects: VisualSettings::default(),
            palettes: (0..NAMES.len())
                .map(|room| {
                    let (background, stone, accent) = room_colors(room);
                    let rgb = |color: Color| {
                        [
                            (color.r * 255.0).round() as u8,
                            (color.g * 255.0).round() as u8,
                            (color.b * 255.0).round() as u8,
                        ]
                    };
                    RoomPalette {
                        background: rgb(background),
                        stone: rgb(stone),
                        accent: rgb(accent),
                    }
                })
                .collect(),
            player_animation: Sprite {
                size: [16, 24],
                frames: vec![],
                fps: 10.0,
                walk: vec![1, 0, 2, 0],
            },
            enemy_animation: Sprite {
                size: [16, 16],
                frames: vec![],
                fps: 8.0,
                walk: vec![0, 1, 0, 2],
            },
        }
    }

    pub fn load(content: &Content) -> Result<Self, String> {
        content.manifest.validate()?;
        let bytes = content.read(&content.manifest.font, 16 * 1024 * 1024)?;
        let mut font = load_ttf_font_from_bytes(&bytes)
            .map_err(|error| format!("{}: {error}", content.manifest.font))?;
        font.set_filter(FilterMode::Nearest);
        let frames = |name: &str| -> Result<Vec<Texture2D>, String> {
            let sprite = &content.manifest.sprites[name];
            (0..sprite.frames.len())
                .map(|index| {
                    content
                        .image(sprite, index)
                        .map(|image| pixel_texture(&image))
                })
                .collect()
        };
        let player_frames = frames("player")?;
        let enemy_frames = frames("enemy")?;
        // Un pack ancien sans ces sprites reste jouable : les emplacements restent vides.
        let optional = |name: &str| -> Result<Option<SpriteSet>, String> {
            match content.manifest.sprites.get(name) {
                Some(sprite) => Ok(Some(SpriteSet {
                    frames: frames(name)?,
                    animation: sprite.clone(),
                })),
                None => Ok(None),
            }
        };
        let hero_sets = Character::ALL
            .iter()
            .map(|character| {
                if *character == Character::Lumen {
                    Ok(None)
                } else {
                    optional(character.sprite())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let enemy_sets = Skin::ALL
            .iter()
            .map(|skin| {
                if *skin == Skin::Creature {
                    Ok(None)
                } else {
                    optional(skin.sprite())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let glow = frames("glow")?.remove(0);
        glow.set_filter(FilterMode::Linear);
        Ok(Self {
            player: player_frames[0].clone(),
            player_frames,
            enemy_frames,
            hero_sets,
            enemy_sets,
            font,
            heart: frames("heart")?.remove(0),
            fern: frames("fern")?.remove(0),
            crystal: frames("crystal")?.remove(0),
            tiles: (0..NAMES.len())
                .map(|room| frames(&format!("tile_{room}")).map(|mut textures| textures.remove(0)))
                .collect::<Result<_, _>>()?,
            glow,
            light_material: light_material(),
            post_material: post_material()?,
            effects: VisualSettings::default(),
            palettes: content.manifest.rooms.clone(),
            player_animation: content.manifest.sprites["player"].clone(),
            enemy_animation: content.manifest.sprites["enemy"].clone(),
        })
    }

    // Presentation commune au jeu et au bac a sable ; menus et HUD restent nets et non deformes.
    pub fn present(&self, texture: &Texture2D, destination: Rect, game: Option<&Game>) {
        self.post_material
            .set_uniform("UiBounds", hud_uv_bounds(game));
        if let Some(game) = game {
            let impact = game.impacts.iter().max_by(|first, second| {
                (first.strength * first.remaining).total_cmp(&(second.strength * second.remaining))
            });
            let shock = impact
                .map(|impact| {
                    (
                        impact.position.x / WIDTH,
                        1.0 - (impact.position.y + 56.0) / VIEW_HEIGHT,
                        1.0 - impact.remaining / 0.35,
                        impact.strength,
                    )
                })
                .unwrap_or((0.0, 0.0, 0.0, 0.0));
            self.post_material.set_uniform("Shock", shock);
            self.post_material.set_uniform(
                "Frame",
                (
                    game.time,
                    game.damage_flash / 0.18,
                    game.velocity.x / 620.0,
                    if self.effects.reduced_motion {
                        1.0f32
                    } else {
                        0.0f32
                    },
                ),
            );
            self.post_material.set_uniform(
                "World",
                (
                    game.room as f32,
                    if self.effects.profile == EffectsProfile::Retro {
                        1.0f32
                    } else {
                        0.0f32
                    },
                    game.health as f32 / game.max_health() as f32,
                    if self.effects.profile == EffectsProfile::Clean {
                        0.0f32
                    } else {
                        1.0f32
                    },
                ),
            );
        } else {
            self.post_material
                .set_uniform("World", (0.0f32, 0.0f32, 1.0f32, 0.0f32));
            self.post_material
                .set_uniform("Frame", (0.0f32, 0.0f32, 0.0f32, 1.0f32));
            self.post_material
                .set_uniform("Shock", (0.0f32, 0.0f32, 0.0f32, 0.0f32));
        }
        // Meme copie opaque pour les menus et le profil clean : une seule composition par pixel.
        gl_use_material(&self.post_material);
        draw_texture_ex(
            texture,
            destination.x,
            destination.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(destination.w, destination.h)),
                flip_y: true,
                ..Default::default()
            },
        );
        gl_use_default_material();
    }

    /// Frames et animation du heros, ou de Lumen si le pack ne fournit pas ce personnage.
    fn hero(&self, character: Character) -> (&[Texture2D], &Sprite) {
        match self
            .hero_sets
            .get(character.index())
            .and_then(Option::as_ref)
        {
            Some(set) => (&set.frames, &set.animation),
            None => (&self.player_frames, &self.player_animation),
        }
    }

    fn enemy_art(&self, skin: Skin) -> (&[Texture2D], &Sprite) {
        match self
            .enemy_sets
            .get(usize::from(skin.index()))
            .and_then(Option::as_ref)
        {
            Some(set) => (&set.frames, &set.animation),
            None => (&self.enemy_frames, &self.enemy_animation),
        }
    }

    /// Image de repos d'un personnage, pour l'ecran de selection.
    pub fn hero_preview(&self, character: Character) -> &Texture2D {
        &self.hero(character).0[0]
    }

    // Le test de l'atelier applique les pixels en memoire sans enregistrer les PNG du pack.
    pub fn update_image(&mut self, name: &str, frame: usize, image: &Image) {
        let texture = pixel_texture(image);
        if let Some(character) = Character::ALL[1..]
            .iter()
            .find(|character| character.sprite() == name)
        {
            if let Some(Some(set)) = self.hero_sets.get_mut(character.index()) {
                if let Some(slot) = set.frames.get_mut(frame) {
                    *slot = texture;
                }
            }
            return;
        }
        if let Some(skin) = Skin::ALL[1..].iter().find(|skin| skin.sprite() == name) {
            if let Some(Some(set)) = self.enemy_sets.get_mut(usize::from(skin.index())) {
                if let Some(slot) = set.frames.get_mut(frame) {
                    *slot = texture;
                }
            }
            return;
        }
        match name {
            "player" => {
                self.player_frames[frame] = texture.clone();
                if frame == 0 {
                    self.player = texture;
                }
            }
            "enemy" => self.enemy_frames[frame] = texture,
            "heart" if frame == 0 => self.heart = texture,
            "fern" if frame == 0 => self.fern = texture,
            "crystal" if frame == 0 => self.crystal = texture,
            "glow" if frame == 0 => {
                texture.set_filter(FilterMode::Linear);
                self.glow = texture;
            }
            _ => {
                if frame == 0 {
                    if let Some(index) = name
                        .strip_prefix("tile_")
                        .and_then(|index| index.parse::<usize>().ok())
                        .filter(|index| *index < self.tiles.len())
                    {
                        self.tiles[index] = texture;
                    }
                }
            }
        }
    }

    pub fn export(root: &std::path::Path) -> Result<Manifest, Box<dyn std::error::Error>> {
        let art = Self::original();
        std::fs::create_dir_all(root.join("sprites"))?;
        std::fs::write(
            root.join("Silkscreen-Regular.ttf"),
            include_bytes!("../assets/Silkscreen-Regular.ttf"),
        )?;
        std::fs::write(root.join("OFL.txt"), include_bytes!("../assets/OFL.txt"))?;
        let mut sprites = std::collections::BTreeMap::new();
        let mut export = |name: &str,
                          textures: &[Texture2D],
                          fps: f32,
                          walk: Vec<usize>|
         -> Result<(), Box<dyn std::error::Error>> {
            let mut frames = Vec::new();
            let size = [textures[0].width() as u16, textures[0].height() as u16];
            for (index, texture) in textures.iter().enumerate() {
                let path = format!("sprites/{name}_{index}.png");
                let image = texture.get_texture_data();
                image::save_buffer_with_format(
                    root.join(&path),
                    &image.bytes,
                    image.width as u32,
                    image.height as u32,
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )?;
                frames.push(path);
            }
            sprites.insert(
                name.to_owned(),
                Sprite {
                    size,
                    frames,
                    fps,
                    walk,
                },
            );
            Ok(())
        };
        export("player", &art.player_frames, 10.0, vec![1, 0, 2, 0])?;
        export("enemy", &art.enemy_frames, 8.0, vec![0, 1, 0, 2])?;
        for (name, texture) in [
            ("heart", &art.heart),
            ("fern", &art.fern),
            ("crystal", &art.crystal),
            ("glow", &art.glow),
        ] {
            export(name, std::slice::from_ref(texture), 1.0, vec![0])?;
        }
        for (room, texture) in art.tiles.iter().enumerate() {
            export(
                &format!("tile_{room}"),
                std::slice::from_ref(texture),
                1.0,
                vec![0],
            )?;
        }
        Ok(Manifest {
            version: 1,
            font: "Silkscreen-Regular.ttf".into(),
            sprites,
            rooms: art.palettes,
            music: "audio/music.wav".into(),
            effects: std::collections::BTreeMap::new(),
            weapons: crate::content::default_weapons(),
        })
    }

    fn room_colors(&self, room: usize) -> (Color, Color, Color) {
        // Les salles ajoutees reutilisent les themes du pack graphique existant.
        let palette = &self.palettes[room % self.palettes.len()];
        let color = |rgb: [u8; 3]| Color::from_rgba(rgb[0], rgb[1], rgb[2], 255);
        (
            color(palette.background),
            color(palette.stone),
            color(palette.accent),
        )
    }

    pub fn text(&self, text: &str, position: Vec2, size: u16, color: Color) {
        draw_text_ex(
            text,
            position.x,
            position.y,
            TextParams {
                font: Some(&self.font),
                font_size: size,
                color,
                ..Default::default()
            },
        );
    }

    pub fn centered(&self, text: &str, center: Vec2, size: u16, color: Color) {
        let measure = measure_text(text, Some(&self.font), size, 1.0);
        self.text(text, center - vec2(measure.width * 0.5, 0.0), size, color);
    }

    // Appeler uniquement dans une passe utilisant light_material ; aucune collision n'est modifiee.
    fn light(&self, center: Vec2, radius: f32, color: Color, intensity: f32) {
        draw_texture_ex(
            &self.glow,
            center.x - radius,
            center.y - radius,
            Color::new(color.r, color.g, color.b, intensity.clamp(0.0, 1.0)),
            DrawTextureParams {
                dest_size: Some(vec2(radius * 2.0, radius * 2.0)),
                ..Default::default()
            },
        );
    }
}

// Codes de deformation : joueur 0..9 (marche, saut, chute, glissade, reception), ennemi avec annonce.
const PLAYER_FRAME_CODES: usize = 10;
const ENEMY_FRAME_CODES: [usize; 5] = [0, 1, 2, 3, 9];

/// Les anciens packs n'ont que six frames de joueur : les poses etendues retombent sur les classiques.
fn usable_frame(available: usize, frame: usize) -> usize {
    let fallback = match frame {
        6..=8 => 3,
        9 => 0,
        other => other,
    };
    if frame < available {
        frame
    } else {
        fallback.min(available.saturating_sub(1))
    }
}

// Le pipeline d'eclairage appartient au moteur ; son masque est un asset externe.
fn light_material() -> Material {
    load_material(
        ShaderSource::Glsl {
            vertex: LIGHT_VERTEX,
            fragment: LIGHT_FRAGMENT,
        },
        MaterialParams {
            pipeline_params: PipelineParams {
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::One,
                )),
                alpha_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Zero,
                    BlendFactor::One,
                )),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .expect("light material")
}

fn portal_light(art: &Art, position: Vec2, active: bool, pulse: f32) {
    art.light(
        position + vec2(26.0, 30.0),
        if active { 115.0 } else { 75.0 },
        if active { MINT } else { GOLD },
        if active { 0.28 * pulse } else { 0.09 * pulse },
    );
}

/// Passe additive liee aux sources du monde, avant les fondus et sans affecter l'interface.
fn lighting(art: &Art, game: &Game) {
    gl_use_material(&art.light_material);
    let pulse = 0.94 + (game.time * 2.0).sin() * 0.06;
    let center = game.position() + PLAYER_SIZE * 0.5;
    art.light(
        center,
        if game.dash_time > 0.0 { 150.0 } else { 110.0 },
        MINT,
        if game.dash_time > 0.0 {
            0.32
        } else {
            0.18 * pulse
        },
    );
    if let Some(point) = game.shrine() {
        art.light(point - vec2(0.0, 40.0), 145.0, MINT, 0.30 * pulse);
    }
    match game.room {
        0 => portal_light(art, vec2(174.0, 106.0), game.save.double_jump, pulse),
        2 => {
            for furnace in 0..4 {
                let flicker = 0.30 + (game.time * 5.0 + furnace as f32 * 1.7).sin() * 0.025;
                art.light(
                    vec2(144.0 + furnace as f32 * 216.0, 446.0),
                    150.0,
                    GOLD,
                    flicker,
                );
            }
            portal_light(art, vec2(754.0, 202.0), game.save.double_jump, pulse);
            if game.save.double_jump {
                portal_light(art, vec2(794.0, 426.0), true, pulse);
            } else {
                art.light(vec2(704.0, 230.0), 120.0, MINT, 0.32 * pulse);
            }
        }
        3 | 5 => portal_light(art, vec2(74.0, 426.0), true, pulse),
        4 => art.light(
            vec2(852.0, 406.0),
            180.0,
            if game.save.won { MINT } else { CORAL },
            0.32 * pulse,
        ),
        6 => {
            // Les halos sont attaches aux cristaux reellement dessines sur les surfaces exposees.
            for (index, tile) in game.tiles.iter().enumerate() {
                if *tile == Tile::Solid
                    && index >= COLS
                    && index % 3 == 0
                    && game.tiles[index - COLS] == Tile::Empty
                {
                    let point = vec2(
                        (index % COLS) as f32 * TILE + 22.0,
                        (index / COLS) as f32 * TILE - 15.0,
                    );
                    art.light(point, 66.0, art.room_colors(6).2, 0.11 * pulse);
                }
            }
            if !game.save.life_shard {
                art.light(vec2(704.0, 230.0), 115.0, CORAL, 0.24 * pulse);
            }
        }
        _ => {}
    }
    if game.attack_time > 0.0 {
        art.light(
            center + vec2(game.facing * 40.0, 0.0),
            85.0,
            PAPER,
            game.attack_time * 1.25,
        );
    }
    for ripple in &game.ripples {
        art.light(ripple.position, 70.0, ripple.color, ripple.remaining * 0.4);
    }
    // Ne pas laisser le melange additif contaminer les particules, fondus, textes ou menus.
    gl_use_default_material();
}

fn diamond(center: Vec2, size: f32, color: Color) {
    draw_triangle(
        center - vec2(0.0, size),
        center + vec2(size * 0.65, 0.0),
        center + vec2(0.0, size),
        color,
    );
    draw_triangle(
        center - vec2(0.0, size),
        center - vec2(size * 0.65, 0.0),
        center + vec2(0.0, size),
        color,
    );
}

/// Colonne de lumiere et anneau de points : repere un sanctuaire ou une recompense a distance.
fn beacon(base: Vec2, color: Color, time: f32) {
    ui::gradient(
        Rect::new(base.x - 5.0, base.y - 150.0, 10.0, 150.0),
        Color::new(color.r, color.g, color.b, 0.0),
        Color::new(color.r, color.g, color.b, 0.22),
    );
    for dash in 0..8 {
        let angle = time * 1.2 + dash as f32 * std::f32::consts::TAU / 8.0;
        let point = base + vec2(angle.cos() * 26.0, angle.sin() * 8.0);
        draw_rectangle(
            point.x - 1.5,
            point.y - 1.5,
            3.0,
            3.0,
            Color::new(color.r, color.g, color.b, 0.7),
        );
    }
}

fn arch(position: Vec2, width: f32, height: f32, stone: Color, inner: Color) {
    draw_rectangle(position.x, position.y + 20.0, width, height - 20.0, stone);
    draw_rectangle(
        position.x + 8.0,
        position.y + 8.0,
        width - 16.0,
        height - 8.0,
        stone,
    );
    draw_rectangle(position.x + 20.0, position.y, width - 40.0, height, stone);
    draw_rectangle(
        position.x + 12.0,
        position.y + 36.0,
        width - 24.0,
        height - 36.0,
        inner,
    );
    draw_rectangle(
        position.x + 20.0,
        position.y + 20.0,
        width - 40.0,
        height - 20.0,
        inner,
    );
}

fn portal(position: Vec2, active: bool, time: f32) {
    arch(
        position,
        52.0,
        54.0,
        Color::from_rgba(95, 109, 94, 255),
        INK,
    );
    let accent = if active { MINT } else { GOLD };
    for index in 0..3 {
        let height = 14.0 + (time * 2.0 + index as f32).sin() * 5.0;
        draw_rectangle(
            position.x + 17.0 + index as f32 * 7.0,
            position.y + 32.0 - height * 0.5,
            3.0,
            height,
            accent,
        );
    }
}

// L'ambiance est calculee depuis le temps de simulation : deterministe en capture et figee en pause.
fn atmosphere(art: &Art, game: &Game, foreground: bool) {
    let time = if art.effects.motion() { game.time } else { 0.0 };
    if foreground {
        if [1, 3, 6].contains(&game.room) {
            for layer in 0..5 {
                let top = 425.0 + layer as f32 * 10.0 + (time * 0.3 + layer as f32).sin() * 6.0;
                draw_rectangle(0.0, top, WIDTH, 55.0, Color::new(0.45, 0.62, 0.59, 0.012));
            }
        }
        if game.room == 6 {
            for fall in 0..4 {
                let left = 140.0 + fall as f32 * 206.0;
                for drop in 0..8 {
                    let phase = (time * 1.4 + drop as f32 / 8.0).fract();
                    let horizontal = left + 18.0 + (drop as f32 * 2.4).sin() * phase * 28.0;
                    let vertical = 478.0 - (phase * std::f32::consts::PI).sin() * 12.0;
                    draw_rectangle(
                        horizontal,
                        vertical,
                        2.0,
                        2.0,
                        Color::new(0.62, 0.86, 0.91, (1.0 - phase) * 0.3),
                    );
                }
                draw_line(
                    left - 7.0,
                    478.0,
                    left + 43.0,
                    478.0,
                    2.0,
                    Color::new(0.62, 0.86, 0.91, 0.3),
                );
            }
        }
        return;
    }
    if [0, 3, 5].contains(&game.room) {
        for shaft in 0..4 {
            let horizontal = 60.0 + shaft as f32 * 248.0;
            let sway = (time * 0.4 + shaft as f32).sin() * 14.0;
            draw_triangle(
                vec2(horizontal, 30.0),
                vec2(horizontal + 30.0, 30.0),
                vec2(horizontal + 140.0 + sway, 440.0),
                Color::new(0.78, 0.88, 0.72, 0.025),
            );
        }
    }
    match game.room {
        1 | 3 => {
            for drop in 0..18 {
                let horizontal = 50.0 + (drop as f32 * 137.0).rem_euclid(WIDTH - 100.0);
                let vertical = (drop as f32 * 73.0 + time * 130.0).rem_euclid(440.0);
                draw_line(
                    horizontal,
                    vertical,
                    horizontal - 2.0,
                    vertical + 8.0,
                    1.0,
                    Color::new(0.55, 0.77, 0.86, 0.14),
                );
            }
        }
        2 => {
            for ember in 0..32 {
                let furnace = ember % 4;
                let phase = (time * 0.35 + ember as f32 * 0.13).fract();
                let horizontal =
                    144.0 + furnace as f32 * 216.0 + (ember as f32 * 2.1 + time).sin() * 16.0;
                draw_rectangle(
                    horizontal,
                    450.0 - phase * 150.0,
                    2.0,
                    3.0,
                    Color::new(GOLD.r, GOLD.g, GOLD.b, (1.0 - phase) * 0.7),
                );
            }
        }
        5 => {
            for leaf in 0..16 {
                let horizontal = (leaf as f32 * 139.0 + time * 18.0).rem_euclid(WIDTH);
                let vertical = (leaf as f32 * 73.0 + time * 24.0).rem_euclid(460.0);
                let sway = (time + leaf as f32).sin() * 12.0;
                diamond(
                    vec2(horizontal + sway, vertical),
                    2.5,
                    Color::new(0.68, 0.82, 0.42, 0.3),
                );
            }
        }
        _ => {}
    }
}

fn combat_effects(art: &Art, game: &Game) {
    if art.effects.motion() {
        for impact in &game.impacts {
            let alpha = (impact.remaining / 0.35).powi(2);
            let size = 5.0 + (1.0 - alpha) * impact.strength * 30.0;
            if impact.remaining > 0.24 {
                let color = Color::new(impact.color.r, impact.color.g, impact.color.b, alpha * 0.7);
                draw_line(
                    impact.position.x - size,
                    impact.position.y,
                    impact.position.x + size,
                    impact.position.y,
                    2.0,
                    color,
                );
                draw_line(
                    impact.position.x,
                    impact.position.y - size,
                    impact.position.x,
                    impact.position.y + size,
                    2.0,
                    color,
                );
            }
        }
        if game.dash_time > 0.0 {
            let center = game.position() + PLAYER_SIZE * 0.5;
            for line in 0..7 {
                let vertical = center.y + (line as f32 - 3.0) * 9.0;
                let horizontal = center.x - game.facing * (18.0 + line as f32 * 11.0);
                draw_line(
                    horizontal,
                    vertical,
                    horizontal - game.facing * 30.0,
                    vertical,
                    1.0,
                    Color::new(MINT.r, MINT.g, MINT.b, 0.15),
                );
            }
        }
    }
    for text in &game.combat_text {
        let alpha = (text.remaining / 0.3).min(1.0);
        art.centered(
            &text.amount.to_string(),
            text.position - vec2(0.0, 5.0),
            16,
            Color::new(text.color.r, text.color.g, text.color.b, alpha),
        );
    }
}

/// Dessine du fond vers le premier plan, dans les coordonnees de la camera du monde.
pub fn scene(art: &Art, game: &Game, target: &RenderTarget) {
    let biome = game.levels.rooms[game.room]
        .biome
        .map(|index| &game.levels.biomes[index]);
    let color = |rgb: [u8; 3]| Color::from_rgba(rgb[0], rgb[1], rgb[2], 255);
    let (background, stone, accent) = biome.map_or_else(
        || art.room_colors(game.room),
        |biome| {
            (
                color(biome.background),
                color(biome.stone),
                color(biome.accent),
            )
        },
    );
    clear_background(background);
    for band in 0..34 {
        let shade = 1.0 - band as f32 * 0.01;
        draw_rectangle(
            0.0,
            band as f32 * 16.0,
            WIDTH,
            16.0,
            Color::new(
                background.r * shade,
                background.g * shade,
                background.b * shade,
                1.0,
            ),
        );
    }
    // Le profil clean reste purement 2D ; sinon les couches 3D s'intercalent dans le decor.
    let depth = art.effects.depth().then(|| {
        crate::render3d::Layers::new(
            target,
            game,
            crate::render3d::Palette {
                background,
                stone,
                accent,
                theme: biome.map_or(game.room, |biome| usize::from(biome.architecture)),
            },
            art.effects.motion(),
        )
    });
    if let Some(layers) = &depth {
        layers.wall();
    }
    let far = Color::new(stone.r * 0.5, stone.g * 0.5, stone.b * 0.5, 1.0);
    let parallax = if art.effects.motion() {
        (game.position().x - WIDTH * 0.5) * -0.025
    } else {
        0.0
    };
    if game.room == 5 {
        for panel in 0..6 {
            let left = 34.0 + panel as f32 * 158.0;
            draw_rectangle(left, 36.0, 142.0, 192.0, Color::from_rgba(37, 67, 58, 255));
            draw_line(left, 36.0, left + 142.0, 228.0, 3.0, far);
            draw_line(left + 142.0, 36.0, left, 228.0, 3.0, far);
            draw_rectangle(left + 68.0, 36.0, 4.0, 192.0, stone);
        }
    } else if game.room == 6 {
        for fall in 0..4 {
            let left = 140.0 + fall as f32 * 206.0;
            draw_rectangle(left, 48.0, 36.0, 428.0, Color::from_rgba(43, 81, 106, 255));
            for ripple in 0..18 {
                let top = 48.0 + (ripple as f32 * 24.0 + game.time * 52.0) % 420.0;
                draw_rectangle(
                    left + 4.0,
                    top,
                    28.0,
                    2.0,
                    Color::from_rgba(83, 136, 156, 255),
                );
            }
        }
    }
    for index in 0..6 {
        let position = vec2(
            60.0 + index as f32 * 164.0 + parallax,
            96.0 + (index % 2) as f32 * 28.0,
        );
        // La silhouette d'architecture reste lisible derriere les plateformes jouables.
        match biome.map(|biome| biome.architecture) {
            Some(0) => {
                draw_rectangle(position.x + 35.0, 70.0, 18.0, 410.0, far);
                draw_circle(position.x + 44.0, 120.0, 55.0, far);
            }
            Some(1) => {
                arch(position, 100.0, 384.0 - position.y, far, background);
                draw_rectangle(position.x + 20.0, 350.0, 60.0, 60.0, far);
            }
            Some(2) => {
                draw_rectangle(position.x, 90.0, 65.0, 390.0, far);
                for top in 0..8 {
                    draw_line(
                        position.x,
                        110.0 + top as f32 * 40.0,
                        position.x + 65.0,
                        110.0 + top as f32 * 40.0,
                        4.0,
                        stone,
                    );
                }
            }
            Some(3) => {
                draw_circle_lines(position.x + 45.0, 220.0, 55.0, 8.0, far);
                draw_rectangle(position.x + 40.0, 270.0, 10.0, 210.0, far);
            }
            Some(4) => {
                draw_triangle(
                    vec2(position.x, 480.0),
                    vec2(position.x + 50.0, 80.0),
                    vec2(position.x + 100.0, 480.0),
                    far,
                );
            }
            Some(5) => {
                for cell in 0..4 {
                    draw_poly_lines(
                        position.x + 45.0,
                        130.0 + cell as f32 * 75.0,
                        6,
                        42.0,
                        30.0,
                        5.0,
                        far,
                    );
                }
            }
            Some(6) => {
                arch(position, 100.0, 384.0 - position.y, far, background);
                draw_circle_lines(position.x + 50.0, 130.0, 60.0, 3.0, stone);
                draw_line(position.x, 130.0, position.x + 100.0, 130.0, 3.0, far);
            }
            Some(7) => {
                draw_rectangle(position.x + 40.0, 240.0, 15.0, 240.0, far);
                draw_ellipse(position.x + 48.0, 240.0, 65.0, 25.0, 0.0, far);
            }
            Some(8) => {
                draw_rectangle(position.x, 160.0, 90.0, 320.0, far);
                for tooth in 0..3 {
                    draw_rectangle(position.x + tooth as f32 * 35.0, 130.0, 20.0, 35.0, far);
                }
            }
            Some(9) => {
                arch(position, 100.0, 384.0 - position.y, far, background);
                draw_triangle(
                    vec2(position.x, 95.0),
                    vec2(position.x + 50.0, 55.0),
                    vec2(position.x + 100.0, 95.0),
                    stone,
                );
            }
            _ => arch(position, 100.0, 384.0 - position.y, far, background),
        }
        draw_rectangle(
            position.x + 40.0,
            position.y + 40.0,
            20.0,
            132.0,
            Color::new(accent.r * 0.09, accent.g * 0.09, accent.b * 0.09, 1.0),
        );
        draw_rectangle(position.x - 18.0, 64.0, 12.0, 416.0, far);
        draw_rectangle(position.x - 22.0, 68.0, 20.0, 10.0, stone);
    }
    for index in 0..12 {
        let col = 40.0 + index as f32 * 83.0;
        let length = 28.0 + ((index * 17 + game.room * 9) % 65) as f32;
        draw_rectangle(col, 30.0, 2.0, length, Color::from_rgba(60, 78, 47, 255));
        for leaf in 0..(length as i32 / 12) {
            draw_rectangle(
                col - (leaf % 2) as f32 * 6.0,
                38.0 + leaf as f32 * 12.0,
                7.0,
                3.0,
                Color::from_rgba(76, 93, 51, 255),
            );
        }
    }
    if let Some(layers) = &depth {
        layers.crystals();
    }
    for index in 0..38 {
        let phase = game.time * (5.0 + (index % 3) as f32) + index as f32 * 31.0;
        let position = vec2(
            (index as f32 * 127.0 + game.time * 8.0) % WIDTH,
            64.0 + phase % 360.0,
        );
        let alpha = 0.15 + (game.time + index as f32).sin().abs() * 0.3;
        draw_rectangle(
            position.x.floor(),
            position.y.floor(),
            2.0,
            2.0,
            Color::new(accent.r, accent.g, accent.b, alpha),
        );
    }
    if game.room == 2 {
        for index in 0..4 {
            let position = vec2(104.0 + index as f32 * 216.0, 378.0);
            arch(position, 80.0, 102.0, stone, INK);
            draw_rectangle(
                position.x + 24.0,
                430.0,
                32.0,
                32.0,
                Color::from_rgba(107, 46, 38, 255),
            );
            for flame in 0..4 {
                let height = 12.0 + (game.time * 8.0 + flame as f32).sin().abs() * 13.0;
                draw_rectangle(
                    position.x + 28.0 + flame as f32 * 6.0,
                    460.0 - height,
                    4.0,
                    height,
                    GOLD,
                );
            }
        }
    }
    if art.effects.profile != EffectsProfile::Clean {
        atmosphere(art, game, false);
    }
    // Les flancs des plateformes sont derriere le plan 2D : ils passent avant les tuiles.
    if let Some(layers) = &depth {
        layers.relief();
    }
    for (index, tile) in game.tiles.iter().enumerate() {
        if *tile != Tile::Solid {
            continue;
        }
        let position = vec2((index % COLS) as f32 * TILE, (index / COLS) as f32 * TILE);
        draw_texture(
            &art.tiles[game.room % art.tiles.len()],
            position.x,
            position.y,
            if biome.is_some() { stone } else { WHITE },
        );
        // Seules les faces superieures exposees recoivent une bordure et des decorations.
        if index >= COLS && game.tiles[index - COLS] == Tile::Empty {
            draw_rectangle(
                position.x,
                position.y,
                TILE,
                3.0,
                Color::new(accent.r * 0.45, accent.g * 0.45, accent.b * 0.45, 1.0),
            );
            if let Some(biome) = biome {
                if index % 3 == 0 {
                    let center = position + vec2(16.0, -8.0);
                    // Flore par biome : fougeres, lichens, braises, coraux, cristaux, fleurs,
                    // roseaux, champignons, ronces et fleurs solaires.
                    match biome.flora {
                        0 => draw_texture_ex(
                            &art.fern,
                            position.x,
                            position.y - 30.0,
                            accent,
                            DrawTextureParams {
                                dest_size: Some(vec2(32.0, 30.0)),
                                ..Default::default()
                            },
                        ),
                        1 => {
                            draw_ellipse(center.x, position.y - 2.0, 14.0, 3.0, 0.0, accent);
                        }
                        2 => {
                            draw_triangle(
                                center + vec2(-6.0, 8.0),
                                center + vec2(0.0, -15.0),
                                center + vec2(6.0, 8.0),
                                accent,
                            );
                        }
                        3 => {
                            for branch in [-1.0, 1.0] {
                                draw_line(
                                    center.x,
                                    position.y,
                                    center.x,
                                    position.y - 22.0,
                                    3.0,
                                    accent,
                                );
                                draw_line(
                                    center.x,
                                    position.y - 8.0,
                                    center.x + branch * 10.0,
                                    position.y - 20.0,
                                    3.0,
                                    accent,
                                );
                            }
                        }
                        4 => draw_texture_ex(
                            &art.crystal,
                            position.x,
                            position.y - 30.0,
                            accent,
                            DrawTextureParams {
                                dest_size: Some(vec2(32.0, 30.0)),
                                ..Default::default()
                            },
                        ),
                        5 | 9 => {
                            draw_line(center.x, position.y, center.x, center.y - 8.0, 2.0, accent);
                            for petal in 0..6 {
                                let point = center + Vec2::from_angle(petal as f32 * 1.047) * 7.0;
                                draw_circle(point.x, point.y, 4.0, accent);
                            }
                            draw_circle(center.x, center.y, 3.0, PAPER);
                        }
                        6 => {
                            for stem in 0..3 {
                                draw_line(
                                    position.x + 8.0 + stem as f32 * 7.0,
                                    position.y,
                                    position.x + stem as f32 * 7.0,
                                    position.y - 24.0,
                                    2.0,
                                    accent,
                                );
                            }
                        }
                        7 => {
                            draw_rectangle(center.x - 2.0, center.y, 4.0, 10.0, accent);
                            draw_ellipse(center.x, center.y, 12.0, 5.0, 0.0, accent);
                            draw_circle(center.x - 4.0, center.y, 2.0, PAPER);
                        }
                        8 => {
                            draw_line(
                                position.x,
                                position.y - 4.0,
                                position.x + 32.0,
                                position.y - 4.0,
                                2.0,
                                accent,
                            );
                            for thorn in 0..3 {
                                draw_triangle(
                                    position + vec2(thorn as f32 * 10.0, -4.0),
                                    position + vec2(thorn as f32 * 10.0 + 4.0, -13.0),
                                    position + vec2(thorn as f32 * 10.0 + 7.0, -4.0),
                                    accent,
                                );
                            }
                        }
                        _ => {}
                    }
                }
            } else if game.room == 5 || game.room == 6 {
                if index % 3 == 0 {
                    draw_texture_ex(
                        if game.room == 5 {
                            &art.fern
                        } else {
                            &art.crystal
                        },
                        position.x + 2.0,
                        position.y - 30.0,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some(vec2(40.0, 30.0)),
                            rotation: if game.room == 5 && art.effects.motion() {
                                (game.time * 1.7 + position.x * 0.01).sin() * 0.045
                            } else {
                                0.0
                            },
                            ..Default::default()
                        },
                    );
                }
            } else if game.room == 0 || game.room == 3 {
                for blade in 0..4 {
                    draw_rectangle(
                        position.x + blade as f32 * 8.0 + 2.0,
                        position.y - 4.0,
                        2.0,
                        5.0,
                        Color::from_rgba(105, 120, 62, 255),
                    );
                }
            }
        }
    }
    // Les piques pulsants rentrent, clignotent en avertissement, puis ressortent.
    let phase = game.hazard_phase();
    let blink = (game.time * 12.0) as i32 % 2 == 0;
    let (rise, spike_color) = match phase {
        HazardPhase::Armed => (1.0, CORAL),
        HazardPhase::Retracted => (0.2, ui::DIM),
        HazardPhase::Warning => (if blink { 0.2 } else { 0.6 }, GOLD),
    };
    // Les points d'interet restent reperables de loin ; le mouvement reduit fige l'anneau.
    let beacon_time = if art.effects.motion() { game.time } else { 0.0 };
    for hazard in game.hazards() {
        for index in 0..(hazard.w / 16.0) as i32 {
            let base = vec2(hazard.x + index as f32 * 16.0, hazard.y + hazard.h);
            let height = 14.0 * rise;
            draw_triangle(
                base,
                base + vec2(14.0, 0.0),
                base + vec2(7.0, -height),
                spike_color,
            );
            if phase == HazardPhase::Armed {
                draw_line(
                    base.x + 7.0,
                    base.y - 13.0,
                    base.x + 12.0,
                    base.y - 2.0,
                    2.0,
                    PAPER,
                );
            }
        }
    }
    for platform in &game.platforms {
        let position = game.world.solid_pos(platform.solid);
        draw_rectangle(
            position.x,
            position.y,
            platform.size.x,
            platform.size.y,
            stone,
        );
        draw_rectangle(position.x, position.y, platform.size.x, 3.0, accent);
        draw_rectangle(
            position.x,
            position.y + platform.size.y - 2.0,
            platform.size.x,
            2.0,
            Color::new(INK.r, INK.g, INK.b, 0.6),
        );
        // Rails et veilleuses : la trajectoire reste lisible avant d'embarquer.
        let (left, right) = (
            platform.from.x.min(platform.to.x),
            platform.from.x.max(platform.to.x),
        );
        draw_line(
            left,
            platform.from.y + 6.0,
            right + platform.size.x,
            platform.from.y + 6.0,
            1.0,
            Color::new(accent.r, accent.g, accent.b, 0.18),
        );
    }
    if let Some(point) = game.shrine() {
        // Dore tant que le sanctuaire n'est pas le point de retour actuel, menthe une fois actif.
        let active = game.save.checkpoint == game.room;
        let tint = if active { MINT } else { GOLD };
        beacon(point - vec2(0.0, 10.0), tint, beacon_time);
        draw_rectangle(point.x - 20.0, point.y - 10.0, 40.0, 10.0, stone);
        draw_rectangle(point.x - 12.0, point.y - 24.0, 24.0, 14.0, PAPER);
        diamond(
            point - vec2(0.0, 40.0 + (game.time * 2.0).sin() * 3.0),
            10.0,
            tint,
        );
        if (game.position() - (point - vec2(0.0, 34.0))).length() < 64.0 {
            ui::panel(
                Rect::new(point.x - 22.0, point.y - 98.0, 44.0, 22.0),
                tint,
                true,
            );
            art.centered("E", vec2(point.x, point.y - 82.0), 14, PAPER);
        }
    }
    for (point, _, _) in game.portals() {
        portal(point - vec2(8.0, 14.0), true, game.time);
    }
    match game.room {
        0 => portal(vec2(174.0, 106.0), game.save.double_jump, game.time),
        2 => {
            if !game.save.double_jump {
                let center = vec2(704.0, 230.0 + (game.time * 2.5).sin() * 5.0);
                beacon(vec2(704.0, 256.0), MINT, beacon_time);
                diamond(center, 16.0, MINT);
                diamond(center - vec2(0.0, 2.0), 6.0, PAPER);
                draw_line(
                    center.x - 35.0,
                    center.y,
                    center.x - 19.0,
                    center.y - 8.0,
                    3.0,
                    MINT,
                );
                draw_line(
                    center.x + 35.0,
                    center.y,
                    center.x + 19.0,
                    center.y - 8.0,
                    3.0,
                    MINT,
                );
            }
            if game.save.double_jump {
                portal(vec2(794.0, 426.0), true, game.time);
            }
            portal(vec2(754.0, 202.0), game.save.double_jump, game.time);
        }
        3 => portal(vec2(74.0, 426.0), true, game.time),
        5 => portal(vec2(74.0, 426.0), true, game.time),
        6 => {
            if !game.save.life_shard {
                let center = vec2(704.0, 230.0 + (game.time * 2.5).sin() * 5.0);
                beacon(vec2(704.0, 256.0), CORAL, beacon_time);
                diamond(center, 22.0, accent);
                draw_texture_ex(
                    &art.heart,
                    center.x - 10.5,
                    center.y - 9.0,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(21.0, 18.0)),
                        ..Default::default()
                    },
                );
            }
        }
        4 => {
            arch(vec2(808.0, 342.0), 88.0, 138.0, stone, INK);
            diamond(
                vec2(852.0, 406.0 + (game.time * 2.0).sin() * 4.0),
                30.0,
                if game.save.won { MINT } else { CORAL },
            );
            diamond(vec2(852.0, 404.0), 12.0, GOLD);
        }
        _ => {}
    }
    for enemy in &game.enemies {
        if enemy.health <= 0 {
            continue;
        }
        let position = game.world.actor_pos(enemy.actor);
        let size = enemy.size();
        let tint = biome.map_or(WHITE, |biome| color(biome.accent));
        // Les equivalents humains gardent leurs couleurs d'origine et n'ont pas les ornements de creature.
        let human = enemy.skin != Skin::Creature;
        let tint = if human { WHITE } else { tint };
        let (enemy_frames, enemy_animation) = art.enemy_art(enemy.skin);
        if enemy.behavior != crate::levels::Behavior::Legacy && enemy.cooldown < 0.65 {
            draw_circle_lines(
                position.x + size.x * 0.5,
                position.y + size.y * 0.5,
                30.0 + size.x * 0.3,
                2.0,
                CORAL,
            );
        }
        // Les especes partagent l'atlas du pack, avec ailes, carapace ou crete distinctes.
        if biome.is_some() && !human {
            match enemy.species % 3 {
                0 => {
                    draw_ellipse(
                        position.x + size.x * 0.5,
                        position.y + size.y * 0.7,
                        size.x * 0.7,
                        10.0,
                        0.0,
                        tint,
                    );
                }
                1 => {
                    for side in [-1.0, 1.0] {
                        draw_ellipse(
                            position.x + size.x * 0.5 + side * size.x * 0.6,
                            position.y + 8.0,
                            12.0,
                            5.0,
                            side * (game.time * 9.0).sin() * 15.0,
                            tint,
                        );
                    }
                }
                _ => {
                    draw_triangle(
                        position + vec2(0.0, 5.0),
                        position + vec2(size.x * 0.5, -12.0),
                        position + vec2(size.x, 5.0),
                        tint,
                    );
                }
            }
        }
        // Decaler la phase selon le point de patrouille pour ne pas synchroniser tous les ennemis.
        let phase = ((game.time * enemy_animation.fps * if enemy.boss { 0.625 } else { 1.0 }
            + enemy.home.x * 0.01) as usize)
            % enemy_animation.walk.len();
        // Annonce de l'attaque : frame dediee quand le pack la fournit, sinon l'animation de marche.
        let telegraph = (enemy.behavior != crate::levels::Behavior::Legacy
            && enemy.cooldown < 0.65)
            || (enemy.boss && enemy.cooldown < 0.9);
        let frame = if enemy.hurt > 0.0 {
            3
        } else if telegraph && enemy_frames.len() > 4 {
            4
        } else {
            enemy_animation.walk[phase]
        };
        let crouch = if enemy.boss && enemy.cooldown < 0.9 {
            0.9
        } else {
            1.0
        };
        let draw_size = (size + vec2(6.0, 4.0)) * vec2(1.0, crouch);
        let pulse = if enemy.boss && enemy.cooldown < 0.9 {
            CORAL
        } else {
            tint
        };
        draw_texture_ex(
            &enemy_frames[frame.min(enemy_frames.len() - 1)],
            position.x - 3.0,
            position.y + size.y - draw_size.y,
            if enemy.hurt > 0.0 { PAPER } else { pulse },
            DrawTextureParams {
                dest_size: Some(draw_size),
                rotation: if enemy.hurt > 0.0 {
                    -enemy.direction * 0.15
                } else {
                    (game.time * 8.0 + enemy.home.x).sin() * 0.025
                },
                flip_x: enemy.direction > 0.0,
                ..Default::default()
            },
        );
        if enemy.boss {
            draw_rectangle(340.0, 78.0, 280.0, 6.0, INK);
            draw_rectangle(
                340.0,
                78.0,
                280.0 * enemy.health as f32 / enemy.max_health() as f32,
                6.0,
                CORAL,
            );
            art.centered(
                biome.map_or("LE VEILLEUR", |biome| biome.boss.as_str()),
                vec2(480.0, 69.0),
                14,
                PAPER,
            );
        }
    }
    for shot in &game.hostile_shots {
        let tail = shot.position - shot.velocity * 0.04;
        draw_line(
            tail.x,
            tail.y,
            shot.position.x,
            shot.position.y,
            shot.radius,
            CORAL,
        );
        draw_circle(shot.position.x, shot.position.y, shot.radius, PAPER);
    }
    let position = game.position();
    let (hero_frames, hero_animation) = art.hero(game.save.character);
    let pose = player_pose(game, hero_animation);
    if game.grounded && art.effects.profile != EffectsProfile::Clean {
        draw_ellipse(
            position.x + 11.0,
            position.y + PLAYER_SIZE.y,
            15.0,
            3.0,
            0.0,
            Color::new(INK.r, INK.g, INK.b, 0.4),
        );
    }
    // Dessiner les silhouettes derriere le joueur, sans creer d'acteurs de collision.
    if art.effects.motion() {
        for echo in &game.afterimages {
            let frame = if echo.dash { 5 } else { 3 };
            draw_texture_ex(
                &hero_frames[usable_frame(hero_frames.len(), frame)],
                echo.position.x + 11.0 - pose.size.x * 0.5,
                echo.position.y + 38.0 - pose.size.y,
                Color::new(MINT.r, MINT.g, MINT.b, echo.remaining / 0.22 * 0.24),
                DrawTextureParams {
                    dest_size: Some(pose.size),
                    rotation: pose.rotation,
                    flip_x: echo.facing < 0.0,
                    ..Default::default()
                },
            );
        }
    }
    if game.invincible <= 0.0 || art.effects.reduced_motion || (game.time * 16.0) as i32 % 2 == 0 {
        draw_texture_ex(
            &hero_frames[usable_frame(hero_frames.len(), pose.frame)],
            position.x + 11.0 - pose.size.x * 0.5,
            position.y + 38.0 - pose.size.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(pose.size),
                rotation: pose.rotation,
                flip_x: game.facing < 0.0,
                ..Default::default()
            },
        );
    }
    for projectile in &game.projectiles {
        let tail = projectile.position
            - projectile.velocity.normalize_or_zero()
                * (if projectile.laser { 42.0 } else { 12.0 } + projectile.radius);
        if projectile.laser {
            draw_line(
                tail.x,
                tail.y,
                projectile.position.x,
                projectile.position.y,
                projectile.radius * 3.0,
                Color::new(
                    projectile.color.r,
                    projectile.color.g,
                    projectile.color.b,
                    0.22,
                ),
            );
        }
        draw_line(
            tail.x,
            tail.y,
            projectile.position.x,
            projectile.position.y,
            projectile.radius,
            projectile.color,
        );
        if projectile.laser {
            draw_line(
                tail.x,
                tail.y,
                projectile.position.x,
                projectile.position.y,
                projectile.radius * 0.55,
                WHITE,
            );
        }
        draw_circle(
            projectile.position.x,
            projectile.position.y,
            projectile.radius,
            projectile.color,
        );
        draw_circle(
            projectile.position.x,
            projectile.position.y,
            projectile.radius * 0.45,
            PAPER,
        );
    }
    if game.attack_time > 0.0 && game.weapon().projectile.is_some() {
        let color = game.weapon().color;
        let tint = Color::from_rgba(color[0], color[1], color[2], 255);
        let hand = position + vec2(11.0, 18.0);
        if game.weapon().laser {
            draw_rectangle(hand.x - 8.0, hand.y - 4.0, 25.0, 8.0, tint);
            draw_circle(hand.x + game.facing * 20.0, hand.y, 4.0, WHITE);
        }
        draw_line(
            hand.x,
            hand.y - 12.0,
            hand.x + game.facing * 14.0,
            hand.y,
            3.0,
            tint,
        );
        draw_line(
            hand.x + game.facing * 14.0,
            hand.y,
            hand.x,
            hand.y + 12.0,
            3.0,
            tint,
        );
    }
    if let Some(sword) = game.sword() {
        let weapon = game.weapon();
        let tint = Color::from_rgba(weapon.color[0], weapon.color[1], weapon.color[2], 255);
        let progress = 1.0 - game.attack_time / weapon.duration;
        for segment in 0..8 {
            let angle = -1.2 + segment as f32 * 0.3;
            let center = position + vec2(11.0, 18.0);
            let inner =
                center + vec2(angle.cos() * game.facing, angle.sin()) * (26.0 + progress * 8.0);
            let outer = center
                + vec2(angle.cos() * game.facing, angle.sin())
                    * weapon.reach
                    * (0.6 + progress * 0.25);
            draw_line(
                inner.x,
                inner.y,
                outer.x,
                outer.y,
                3.0,
                Color::new(tint.r, tint.g, tint.b, (1.0 - progress) * 0.65),
            );
        }
        let center = vec2(
            if game.facing > 0.0 {
                sword.x + sword.w - 12.0
            } else {
                sword.x + 12.0
            },
            sword.y + sword.h * 0.5,
        );
        draw_line(
            position.x + 11.0,
            position.y + 18.0,
            center.x,
            center.y,
            4.0,
            tint,
        );
        // Les armes lourdes ont une tete pleine ; les lames suivent la hauteur de leur frappe.
        if weapon.damage >= 3 {
            draw_rectangle(
                center.x - 8.0,
                center.y - weapon.height * 0.3,
                16.0,
                weapon.height * 0.6,
                tint,
            );
        } else {
            for vertical in [-1.0, 1.0] {
                draw_line(
                    center.x,
                    center.y + weapon.height * 0.3 * vertical,
                    center.x + game.facing * 12.0,
                    center.y,
                    4.0,
                    tint,
                );
            }
        }
    }
    for ripple in &game.ripples {
        // La duree de 0,45 s doit rester synchronisee avec Game::burst.
        let progress = 1.0 - ripple.remaining / 0.45;
        draw_circle_lines(
            ripple.position.x,
            ripple.position.y,
            4.0 + progress * ripple.radius,
            1.5,
            Color::new(
                ripple.color.r,
                ripple.color.g,
                ripple.color.b,
                (1.0 - progress) * 0.55,
            ),
        );
    }
    for particle in &game.particles {
        let alpha = (particle.remaining * 2.0).min(1.0);
        if art.effects.profile != EffectsProfile::Clean {
            let tail = particle.position - particle.velocity * 0.025;
            draw_line(
                tail.x,
                tail.y,
                particle.position.x,
                particle.position.y,
                1.0,
                Color::new(
                    particle.color.r,
                    particle.color.g,
                    particle.color.b,
                    alpha * 0.4,
                ),
            );
        }
        draw_rectangle(
            particle.position.x,
            particle.position.y,
            3.0,
            3.0,
            Color::new(particle.color.r, particle.color.g, particle.color.b, alpha),
        );
    }
    if art.effects.profile != EffectsProfile::Clean {
        atmosphere(art, game, true);
        combat_effects(art, game);
    }
    if let Some(layers) = &depth {
        layers.foreground();
    }
    if art.effects.profile != EffectsProfile::Clean {
        ui::world_vignette(0.0, HEIGHT);
    }
    lighting(art, game);
    if game.damage_flash > 0.0 && !art.effects.reduced_motion {
        draw_rectangle(
            0.0,
            0.0,
            WIDTH,
            HEIGHT,
            Color::new(CORAL.r, CORAL.g, CORAL.b, game.damage_flash * 0.65),
        );
    }
    // Le fondu occupe les 0,4 premieres secondes des 2,4 secondes du titre de salle.
    if game.room_time > 2.0 {
        draw_rectangle(
            0.0,
            0.0,
            WIDTH,
            HEIGHT,
            Color::new(INK.r, INK.g, INK.b, ((game.room_time - 2.0) / 0.4).min(1.0)),
        );
    }
    if game.room_time > 0.0 {
        art.centered(
            &game.levels.name(game.room),
            vec2(480.0, 130.0),
            24,
            Color::new(PAPER.r, PAPER.g, PAPER.b, game.room_time.min(1.0)),
        );
    }
}

/// Interface en coordonnees ecran, independante des secousses de la camera du monde.
pub fn hud(art: &Art, game: &Game) {
    ui::gradient(
        Rect::new(0.0, 0.0, WIDTH, 56.0),
        Color::from_rgba(16, 25, 23, 255),
        INK,
    );
    draw_rectangle(
        0.0,
        55.0,
        WIDTH,
        1.0,
        Color::new(MINT.r, MINT.g, MINT.b, 0.35),
    );
    ui::shadowed(art, "LUMEN", vec2(22.0, 29.0), 24, PAPER);
    ui::fitted(
        art,
        &game.levels.name(game.room),
        vec2(24.0, 47.0),
        11,
        GOLD,
        116.0,
    );
    ui::panel(Rect::new(150.0, 8.0, 176.0, 40.0), CORAL, false);
    // Les vies de debug gardent la largeur historique du HUD, sans recouvrir le pouvoir.
    let heart_spacing = 24.0_f32.min(152.0 / game.max_health() as f32);
    for index in 0..game.max_health() {
        draw_texture_ex(
            &art.heart,
            162.0 + index as f32 * heart_spacing,
            19.0,
            if index < game.health {
                WHITE
            } else {
                Color::from_rgba(54, 57, 49, 255)
            },
            DrawTextureParams {
                dest_size: Some(vec2(heart_spacing - 3.0, 18.0)),
                ..Default::default()
            },
        );
    }
    let has_power = game.save.double_jump;
    ui::panel(Rect::new(336.0, 8.0, 152.0, 40.0), MINT, has_power);
    diamond(
        vec2(353.0, 28.0),
        8.0,
        if has_power { MINT } else { ui::DIM },
    );
    art.text(
        if has_power {
            "AILE D'ECHO"
        } else {
            "ECLAT ERRANT"
        },
        vec2(367.0, if has_power { 26.0 } else { 33.0 }),
        12,
        if has_power { MINT } else { PAPER },
    );
    if has_power {
        // La jauge se remplit pendant la recharge du dash et reste pleine quand il est pret.
        ui::bar(
            Rect::new(367.0, 33.0, 108.0, 5.0),
            if game.dash_available {
                1.0 - game.dash_cooldown / crate::balance::active().player.dash_cooldown
            } else {
                0.0
            },
            MINT,
        );
    }
    ui::panel(Rect::new(496.0, 8.0, 296.0, 40.0), GOLD, false);
    let owned = (0..game.weapons.len())
        .filter(|index| game.weapon_available(*index))
        .count();
    let weapon_name = format!("{} [{owned}/{}]", game.weapon().name, game.weapons.len());
    ui::fitted(art, &weapon_name, vec2(508.0, 25.0), 14, PAPER, 180.0);
    art.text(
        &format!("ECLATS {}", game.save.shards),
        vec2(704.0, 25.0),
        11,
        MINT,
    );
    if !game.all_weapons {
        if let Some(next) = game
            .weapons
            .iter()
            .filter(|weapon| weapon.unlock_kills > game.save.defeated_enemies)
            .min_by_key(|weapon| weapon.unlock_kills)
        {
            art.text(
                &format!("BUTIN {}/{}", game.save.defeated_enemies, next.unlock_kills),
                vec2(508.0, 41.0),
                10,
                GOLD,
            );
            ui::bar(
                Rect::new(600.0, 34.0, 176.0, 5.0),
                game.save.defeated_enemies as f32 / next.unlock_kills.max(1) as f32,
                GOLD,
            );
        }
    }
    ui::panel(Rect::new(800.0, 6.0, 152.0, 44.0), MINT, false);
    // La mini-carte partage la page de la carte : jamais plus de cinquante noeuds dans le HUD.
    let original_nodes = [
        vec2(818.0, 36.0),
        vec2(850.0, 36.0),
        vec2(882.0, 36.0),
        vec2(818.0, 18.0),
        vec2(850.0, 18.0),
        vec2(914.0, 36.0),
        vec2(914.0, 18.0),
    ];
    let indices = if game.levels.rooms.len() > 28 {
        room_page(game)
    } else {
        (0..game.levels.rooms.len()).collect()
    };
    let mut nodes = vec![None; game.levels.rooms.len()];
    for (offset, index) in indices.iter().enumerate() {
        nodes[*index] = Some(if game.levels.rooms.len() == NAMES.len() {
            original_nodes[*index]
        } else if game.levels.rooms.len() > 28 {
            vec2(
                818.0 + (offset % 10) as f32 * 11.0,
                10.0 + (offset / 10) as f32 * 8.0,
            )
        } else {
            vec2(
                818.0 + (offset % 7) as f32 * 16.0,
                12.0 + (offset / 7) as f32 * 11.0,
            )
        });
    }
    for (from, to) in LINKS.into_iter().chain(
        game.levels
            .portals
            .iter()
            .map(|portal| (portal.from, portal.to)),
    ) {
        if let (Some(from), Some(to)) = (nodes[from], nodes[to]) {
            draw_line(
                from.x,
                from.y,
                to.x,
                to.y,
                2.0,
                Color::from_rgba(85, 87, 67, 255),
            );
        }
    }
    for (index, point) in nodes.iter().enumerate() {
        let Some(point) = point else {
            continue;
        };
        draw_rectangle(
            point.x - 6.0,
            point.y - 3.0,
            if game.levels.rooms.len() > 28 {
                8.0
            } else {
                12.0
            },
            6.0,
            if index == game.room {
                MINT
            } else if game.save.visited[index] {
                PAPER
            } else {
                Color::from_rgba(54, 60, 49, 255)
            },
        );
    }
    if game.notice_time > 0.0 {
        ui::gradient(
            Rect::new(0.0, 557.0, WIDTH, 43.0),
            Color::from_rgba(18, 28, 26, 255),
            INK,
        );
        draw_rectangle(
            0.0,
            557.0,
            WIDTH,
            1.0,
            Color::new(GOLD.r, GOLD.g, GOLD.b, 0.45),
        );
        art.centered(&game.notice, vec2(480.0, 583.0), 14, PAPER);
    }
}

fn stone_dim() -> Color {
    Color::from_rgba(70, 77, 62, 255)
}

fn room_page(game: &Game) -> Vec<usize> {
    let biome = game.levels.rooms[game.room].biome;
    let indices: Vec<_> = game
        .levels
        .rooms
        .iter()
        .enumerate()
        .filter(|(_, room)| room.biome == biome)
        .map(|(index, _)| index)
        .collect();
    let page = indices
        .iter()
        .position(|index| *index == game.room)
        .unwrap_or(0)
        / 50;
    indices.into_iter().skip(page * 50).take(50).collect()
}

pub fn map(art: &Art, game: &Game) {
    ui::overlay();
    art.text(
        &format!("MORTS {}", game.save.deaths),
        vec2(20.0, 80.0),
        12,
        PAPER,
    );
    if game.levels.rooms.len() > 28 {
        let biome = game.levels.rooms[game.room].biome;
        let title = biome.map_or("LA CITADELLE SILENCIEUSE", |index| {
            game.levels.biomes[index].name.as_str()
        });
        art.centered(title, vec2(480.0, 132.0), 22, PAPER);
        for (index, zone) in game.levels.biomes.iter().enumerate() {
            let position = vec2(120.0 + index as f32 * 80.0, 166.0);
            let defeated = game.levels.rooms.iter().enumerate().any(|(room, data)| {
                data.biome == Some(index)
                    && data.enemies.iter().any(|enemy| enemy.boss)
                    && game.save.defeated_bosses.contains(&room)
            });
            art.centered(
                &format!("{:02}", index + 1),
                position,
                14,
                if biome == Some(index) {
                    MINT
                } else if defeated {
                    GOLD
                } else {
                    PAPER
                },
            );
            draw_line(
                position.x - 20.0,
                position.y + 8.0,
                position.x + 20.0,
                position.y + 8.0,
                3.0,
                Color::from_rgba(zone.accent[0], zone.accent[1], zone.accent[2], 255),
            );
        }
        let mut nodes = vec![None; game.levels.rooms.len()];
        let rooms = room_page(game);
        let page_start = game.levels.rooms[..rooms[0]]
            .iter()
            .filter(|room| room.biome == biome)
            .count();
        // Une page de cinquante salles tient dans le viewport, sans noms longs dans les cases.
        for (offset, index) in rooms.iter().enumerate() {
            nodes[*index] = Some(vec2(
                120.0 + (offset % 10) as f32 * 80.0,
                225.0 + (offset / 10) as f32 * 48.0,
            ));
        }
        for portal in &game.levels.portals {
            if let (Some(from), Some(to)) = (nodes[portal.from], nodes[portal.to]) {
                draw_line(from.x, from.y, to.x, to.y, 2.0, stone_dim());
            }
        }
        for (offset, index) in rooms.iter().enumerate() {
            let point = nodes[*index].unwrap();
            draw_rectangle(point.x - 30.0, point.y - 17.0, 60.0, 34.0, stone_dim());
            draw_rectangle_lines(
                point.x - 30.0,
                point.y - 17.0,
                60.0,
                34.0,
                2.0,
                if *index == game.room { MINT } else { INK },
            );
            let label = if game.save.visited[*index] {
                format!("{:02}", page_start + offset + 1)
            } else {
                "??".into()
            };
            art.centered(&label, point + vec2(0.0, 5.0), 14, PAPER);
            if game.levels.rooms[*index]
                .enemies
                .iter()
                .any(|enemy| enemy.boss)
            {
                diamond(
                    point + vec2(22.0, 0.0),
                    4.0,
                    if game.save.defeated_bosses.contains(index) {
                        GOLD
                    } else {
                        CORAL
                    },
                );
            }
            if game.levels.rooms[*index].shrine.is_some() {
                draw_rectangle(point.x - 28.0, point.y - 3.0, 6.0, 6.0, MINT);
            }
            if *index == game.room {
                diamond(point - vec2(0.0, 23.0), 4.0, MINT);
            }
        }
        // Legende : les reperes de la carte doivent se comprendre sans lire le manuel.
        diamond(vec2(300.0, 528.0), 4.0, CORAL);
        art.text("BOSS", vec2(312.0, 532.0), 12, PAPER);
        draw_rectangle(397.0, 525.0, 6.0, 6.0, MINT);
        art.text("SANCTUAIRE", vec2(410.0, 532.0), 12, PAPER);
        diamond(vec2(560.0, 528.0), 4.0, GOLD);
        art.text("BOSS VAINCU", vec2(572.0, 532.0), 12, PAPER);
        return;
    }
    art.centered("LA CITADELLE SILENCIEUSE", vec2(480.0, 142.0), 24, PAPER);
    let expanded = game.levels.rooms.len() > NAMES.len();
    let nodes = if !expanded {
        vec![
            vec2(150.0, 365.0),
            vec2(370.0, 365.0),
            vec2(590.0, 365.0),
            vec2(150.0, 230.0),
            vec2(370.0, 230.0),
            vec2(810.0, 365.0),
            vec2(810.0, 230.0),
        ]
    } else {
        (0..game.levels.rooms.len())
            .map(|index| {
                vec2(
                    84.0 + (index % 7) as f32 * 132.0,
                    218.0 + (index / 7) as f32 * 86.0,
                )
            })
            .collect()
    };
    for (from, to) in LINKS.into_iter().chain(
        game.levels
            .portals
            .iter()
            .map(|portal| (portal.from, portal.to)),
    ) {
        draw_line(
            nodes[from].x,
            nodes[from].y,
            nodes[to].x,
            nodes[to].y,
            2.0,
            Color::from_rgba(74, 89, 69, 255),
        );
    }
    if game.save.double_jump && !expanded {
        draw_line(590.0, 395.0, 590.0, 440.0, 2.0, MINT);
        draw_line(150.0, 440.0, 590.0, 440.0, 2.0, MINT);
        draw_line(150.0, 395.0, 150.0, 440.0, 2.0, MINT);
    }
    for (index, point) in nodes.iter().enumerate() {
        let half_width = if expanded { 58.0 } else { 84.0 };
        draw_rectangle(
            point.x - half_width,
            point.y - 30.0,
            half_width * 2.0,
            60.0,
            art.room_colors(index).1,
        );
        draw_rectangle_lines(
            point.x - half_width,
            point.y - 30.0,
            half_width * 2.0,
            60.0,
            2.0,
            if game.room == index {
                MINT
            } else {
                Color::from_rgba(94, 104, 83, 255)
            },
        );
        let name = game.levels.name(index);
        art.centered(
            if game.save.visited[index] {
                &name
            } else {
                "???"
            },
            *point + vec2(0.0, 5.0),
            if expanded { 11 } else { 14 },
            PAPER,
        );
        if game.room == index {
            diamond(*point - vec2(0.0, 43.0), 6.0, MINT);
        }
    }
}

pub fn button(art: &Art, label: &str, rect: Rect, mouse: Vec2, selected: bool) -> bool {
    // Le focus visuel vient du menu ; le clic utilise toujours la position reelle du pointeur.
    let hover = selected;
    ui::panel(rect, MINT, hover);
    if hover {
        // Barre d'accent a gauche, lisible meme sans la couleur du contour.
        draw_rectangle(rect.x + 4.0, rect.y + 6.0, 3.0, rect.h - 12.0, MINT);
        diamond(vec2(rect.x + 20.0, rect.y + rect.h * 0.5), 5.0, MINT);
    }
    // Centrer aussi le texte des boutons compacts utilises par les menus plus longs.
    art.centered(
        label,
        vec2(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5 + 6.0),
        18,
        if hover { PAPER } else { ui::DIM },
    );
    rect.contains(mouse) && is_mouse_button_pressed(MouseButton::Left)
}

/// Zones stables des six reglages et du bouton retour, partagees avec la navigation.
pub const AUDIO_ROWS: usize = 6;

pub fn audio_rect(index: usize) -> Rect {
    if index < AUDIO_ROWS {
        Rect::new(200.0, 160.0 + index as f32 * 48.0, 560.0, 42.0)
    } else {
        Rect::new(320.0, 464.0, 320.0, 44.0)
    }
}

/// Retourne (ligne, sens) : un pas de volume pour les deux premieres lignes, +-1 pour les autres.
pub fn audio_controls(
    art: &Art,
    config: &crate::controls::Bindings,
    selection: usize,
    mouse: Vec2,
    notice: &str,
) -> Option<(usize, f32)> {
    ui::overlay();
    art.centered("AUDIO ET EFFETS", vec2(480.0, 128.0), 30, PAPER);
    let yes_no = |on: bool| if on { "OUI" } else { "NON" }.to_string();
    let rows = [
        ("MUSIQUE", format!("{:.0}%", config.music_volume * 100.0)),
        ("BRUITAGES", format!("{:.0}%", config.sfx_volume * 100.0)),
        (
            "EFFETS",
            match config.fx {
                EffectsProfile::Cinematic => "CINEMA",
                EffectsProfile::Retro => "RETRO",
                EffectsProfile::Clean => "SIMPLE",
            }
            .to_string(),
        ),
        ("MOUVEMENT REDUIT", yes_no(config.reduced_motion)),
        ("PROFONDEUR 3D", yes_no(!config.flat)),
        (
            "DIFFICULTE",
            match config.difficulty {
                Difficulty::Calm => "CALME",
                Difficulty::Normal => "NORMAL",
                Difficulty::Hard => "EPREUVE",
            }
            .to_string(),
        ),
    ];
    let mut change = None;
    for (index, (label, value)) in rows.iter().enumerate() {
        let rect = audio_rect(index);
        art.text(label, vec2(rect.x + 22.0, rect.y + 28.0), 18, PAPER);
        if selection == index {
            diamond(vec2(rect.x + 6.0, rect.y + 22.0), 5.0, MINT);
        }
        art.centered(value, vec2(650.0, rect.y + 28.0), 18, MINT);
        // Les boutons +/- sont de vrais steppers ; l'appelant gere la persistance du changement.
        let step = if index < 2 { 0.1 } else { 1.0 };
        if button(
            art,
            "-",
            Rect::new(560.0, rect.y, 44.0, rect.h),
            mouse,
            false,
        ) {
            change = Some((index, -step));
        }
        if button(
            art,
            "+",
            Rect::new(716.0, rect.y, 44.0, rect.h),
            mouse,
            false,
        ) {
            change = Some((index, step));
        }
    }
    button(
        art,
        "RETOUR",
        audio_rect(AUDIO_ROWS),
        vec2(-1.0, -1.0),
        selection == AUDIO_ROWS,
    );
    art.centered(
        notice,
        vec2(480.0, 535.0),
        12,
        if notice == "CONFIGURATION ENREGISTREE" {
            MINT
        } else {
            CORAL
        },
    );
    change
}

pub const TALENT_ROWS: usize = 6;

pub fn talent_rect(index: usize) -> Rect {
    if index < TALENT_ROWS {
        Rect::new(120.0, 150.0 + index as f32 * 52.0, 720.0, 46.0)
    } else {
        Rect::new(320.0, 478.0, 320.0, 44.0)
    }
}

/// Retourne le talent clique ; l'achat et la sauvegarde restent a l'appelant.
pub fn talents(
    art: &Art,
    game: &Game,
    selection: usize,
    mouse: Vec2,
    notice: &str,
) -> Option<usize> {
    use crate::talents::{Talent, TalentState};
    ui::overlay();
    art.centered("TALENTS", vec2(480.0, 108.0), 30, PAPER);
    art.centered(
        &format!("ECLATS {}", game.save.shards),
        vec2(480.0, 134.0),
        14,
        GOLD,
    );
    let mut clicked = None;
    for (index, talent) in Talent::ALL.iter().enumerate() {
        let rect = talent_rect(index);
        let (label, tint) = match game.talent_state(*talent) {
            TalentState::Owned => ("ACQUIS", MINT),
            TalentState::Available => ("ACHETER", GOLD),
            TalentState::Locked => ("VERROUILLE", ui::DIM),
            TalentState::TooExpensive => ("TROP CHER", CORAL),
        };
        ui::panel(rect, tint, selection == index);
        art.text(talent.name(), vec2(rect.x + 20.0, rect.y + 20.0), 16, PAPER);
        let requirement = talent
            .requires()
            .map(|required| format!("  (APRES {})", required.name()))
            .unwrap_or_default();
        art.text(
            &format!("{}{requirement}", talent.description()),
            vec2(rect.x + 20.0, rect.y + 37.0),
            11,
            ui::DIM,
        );
        art.text(
            &format!("{} ECLATS", talent.cost()),
            vec2(rect.right() - 250.0, rect.y + 20.0),
            12,
            GOLD,
        );
        art.centered(label, vec2(rect.right() - 70.0, rect.y + 28.0), 14, tint);
        if rect.contains(mouse) && is_mouse_button_pressed(MouseButton::Left) {
            clicked = Some(index);
        }
    }
    button(
        art,
        "RETOUR",
        talent_rect(TALENT_ROWS),
        vec2(-1.0, -1.0),
        selection == TALENT_ROWS,
    );
    art.centered(
        notice,
        vec2(480.0, 548.0),
        12,
        if notice == "TALENT ACQUIS" {
            MINT
        } else {
            CORAL
        },
    );
    clicked
}

pub const CHARACTER_ROWS: usize = Character::ALL.len();

pub fn character_rect(index: usize) -> Rect {
    if index < CHARACTER_ROWS {
        Rect::new(120.0, 136.0 + index as f32 * 52.0, 720.0, 50.0)
    } else {
        Rect::new(320.0, 474.0, 320.0, 44.0)
    }
}

/// Retourne le personnage clique ; le choix et la sauvegarde restent a l'appelant.
pub fn characters(
    art: &Art,
    game: &Game,
    selection: usize,
    mouse: Vec2,
    notice: &str,
) -> Option<usize> {
    ui::overlay();
    art.centered("PERSONNAGES", vec2(480.0, 108.0), 30, PAPER);
    art.centered(
        &format!(
            "ENNEMIS {}   BOSS {}",
            game.save.defeated_enemies,
            game.save.defeated_bosses.len()
        ),
        vec2(480.0, 130.0),
        12,
        GOLD,
    );
    let mut clicked = None;
    for (index, character) in Character::ALL.iter().enumerate() {
        let rect = character_rect(index);
        let unlocked = game.character_unlocked(*character);
        let chosen = game.save.character == *character;
        let (label, tint) = if chosen {
            ("CHOISI", MINT)
        } else if unlocked {
            ("CHOISIR", GOLD)
        } else {
            ("VERROUILLE", ui::DIM)
        };
        ui::panel(rect, tint, selection == index);
        // Un personnage verrouille reste visible mais assombri.
        draw_texture_ex(
            art.hero_preview(*character),
            rect.x + 12.0,
            rect.y + 1.0,
            if unlocked {
                WHITE
            } else {
                Color::new(0.25, 0.25, 0.3, 1.0)
            },
            DrawTextureParams {
                dest_size: Some(vec2(32.0, 48.0)),
                ..Default::default()
            },
        );
        art.text(
            character.name(),
            vec2(rect.x + 62.0, rect.y + 20.0),
            16,
            PAPER,
        );
        art.text(
            character.description(),
            vec2(rect.x + 62.0, rect.y + 38.0),
            11,
            ui::DIM,
        );
        if !unlocked {
            art.text(
                &character
                    .requirement()
                    .label(game.save.defeated_enemies, game.save.defeated_bosses.len()),
                vec2(rect.right() - 290.0, rect.y + 20.0),
                11,
                GOLD,
            );
        }
        art.centered(label, vec2(rect.right() - 70.0, rect.y + 30.0), 14, tint);
        if rect.contains(mouse) && is_mouse_button_pressed(MouseButton::Left) {
            clicked = Some(index);
        }
    }
    button(
        art,
        "RETOUR",
        character_rect(CHARACTER_ROWS),
        vec2(-1.0, -1.0),
        selection == CHARACTER_ROWS,
    );
    art.centered(
        notice,
        vec2(480.0, 548.0),
        12,
        if notice == "PERSONNAGE CHOISI" {
            MINT
        } else {
            CORAL
        },
    );
    clicked
}

/// Geometrie partagee par le rendu, la selection et les clics des reglages.
pub fn binding_rect(index: usize) -> Rect {
    if index < crate::controls::Action::ALL.len() {
        // Dix actions doivent tenir au-dessus des boutons fixes de retour et de reinitialisation.
        Rect::new(110.0, 168.0 + index as f32 * 30.0, 740.0, 28.0)
    } else {
        Rect::new(
            150.0 + (index - crate::controls::Action::ALL.len()) as f32 * 360.0,
            478.0,
            300.0,
            40.0,
        )
    }
}

/// Les valeurs viennent des bindings actifs ; cet ecran ne touche ni le disque ni la simulation.
pub fn bindings(
    art: &Art,
    config: &crate::controls::Bindings,
    selection: usize,
    waiting: Option<crate::controls::Action>,
    notice: &str,
) {
    use crate::controls::Action;
    ui::overlay();
    art.centered("TOUCHES CLAVIER", vec2(480.0, 118.0), 24, PAPER);
    art.text("ACTION", vec2(132.0, 151.0), 12, GOLD);
    art.text("TOUCHES", vec2(390.0, 151.0), 12, GOLD);
    for (index, action) in Action::ALL.iter().enumerate() {
        let rect = binding_rect(index);
        if selection == index {
            draw_rectangle(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                Color::from_rgba(40, 61, 51, 255),
            );
            diamond(vec2(rect.x + 10.0, rect.y + 14.0), 4.0, MINT);
        }
        art.text(
            action.label(),
            vec2(rect.x + 22.0, rect.y + 20.0),
            14,
            PAPER,
        );
        let label = if waiting == Some(*action) {
            "...".to_string()
        } else {
            config
                .keys(*action)
                .iter()
                .map(|key| key.label())
                .collect::<Vec<_>>()
                .join(" / ")
        };
        // Les alias edites a la main doivent eux aussi tenir dans la colonne fixe.
        let mut size = 14;
        while size > 8 && measure_text(&label, Some(&art.font), size, 1.0).width > 440.0 {
            size -= 1;
        }
        art.text(
            &label,
            vec2(390.0, rect.y + 20.0),
            size,
            if waiting == Some(*action) { GOLD } else { MINT },
        );
        draw_line(
            rect.x,
            rect.y + rect.h,
            rect.x + rect.w,
            rect.y + rect.h,
            1.0,
            Color::from_rgba(42, 56, 50, 255),
        );
    }
    for (offset, label) in ["PAR DEFAUT", "RETOUR"].iter().enumerate() {
        let index = Action::ALL.len() + offset;
        let rect = binding_rect(index);
        // La boucle principale gere l'activation ; le faux pointeur evite les effets de clic ici.
        button(art, label, rect, vec2(-1.0, -1.0), selection == index);
    }
    art.centered(
        notice,
        vec2(480.0, 553.0),
        12,
        if notice == "CONFIGURATION ENREGISTREE" {
            MINT
        } else {
            CORAL
        },
    );
}

/// Le temps du menu anime l'accueil meme quand la simulation du jeu est arretee.
pub fn title(art: &Art, time: f32) {
    clear_background(INK);
    for layer in 0..3 {
        let color = art.room_colors(layer).1;
        for tower in 0..9 {
            let left = tower as f32 * 126.0 - 42.0 + layer as f32 * 30.0;
            let top = 210.0 + layer as f32 * 68.0 + ((tower * 47) % 90) as f32;
            draw_rectangle(
                left,
                top,
                90.0,
                HEIGHT - top,
                Color::new(color.r * 0.55, color.g * 0.55, color.b * 0.55, 1.0),
            );
            for tooth in 0..3 {
                draw_rectangle(left + tooth as f32 * 32.0, top - 12.0, 24.0, 16.0, color);
            }
            arch(vec2(left + 24.0, top + 30.0), 44.0, 120.0, color, INK);
            draw_rectangle(left + 42.0, top + 66.0, 4.0, 36.0, GOLD);
        }
    }
    draw_rectangle(
        0.0,
        0.0,
        WIDTH,
        HEIGHT,
        Color::new(INK.r, INK.g, INK.b, 0.55),
    );
    // Ancrer les pieds pendant la respiration, independamment du temps de simulation arrete.
    let breathing_height = 84.0 * (1.0 + (time * 2.5).sin() * 0.018);
    draw_texture_ex(
        &art.player,
        450.0,
        482.0 - breathing_height,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(56.0, breathing_height)),
            ..Default::default()
        },
    );
    for mote in 0..48 {
        let horizontal = (mote as f32 * 137.0 + time * 9.0) % WIDTH;
        let vertical = 24.0 + (mote as f32 * 73.0 - time * 13.0).rem_euclid(490.0);
        draw_rectangle(
            horizontal.floor(),
            vertical.floor(),
            2.0,
            2.0,
            Color::new(
                MINT.r,
                MINT.g,
                MINT.b,
                0.2 + (time + mote as f32).sin().abs() * 0.35,
            ),
        );
    }
    // L'accueil utilise son propre temps ; son halo reste anime sans faire avancer le jeu.
    gl_use_material(&art.light_material);
    art.light(
        vec2(478.0, 440.0 + (time * 1.5).sin() * 2.0),
        135.0,
        MINT,
        0.19 + (time * 2.0).sin() * 0.012,
    );
    gl_use_default_material();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campaign_maps_include_current_room_and_never_exceed_fifty_nodes() {
        let mut levels = crate::levels::Levels::original();
        let start = levels.add_campaign().unwrap();
        let mut game = Game::with_levels(Save::default(), levels);
        for room in 0..507 {
            game.room = room;
            let page = room_page(&game);
            assert!(page.contains(&room));
            assert!(page.len() <= 50);
            assert!(page
                .iter()
                .all(|index| game.levels.rooms[*index].biome == game.levels.rooms[room].biome));
        }
        // Les extensions d'un biome occupent une nouvelle page, sans deborder sur le monde.
        let mut source = start + 499;
        for seed in 0..12 {
            source = game.levels.generate(source, seed, 1).unwrap();
        }
        game.room = source;
        let page = room_page(&game);
        assert_eq!(page.len(), 12);
        assert!(page.contains(&source));
    }

    #[test]
    fn hud_bounds_protect_header_and_active_notification() {
        let mut game = Game::new(Save::default());
        assert_eq!(hud_uv_bounds(Some(&game)), (56.0 / VIEW_HEIGHT, 0.0));
        game.message("Sanctuaire active.");
        assert_eq!(
            hud_uv_bounds(Some(&game)),
            (56.0 / VIEW_HEIGHT, 43.0 / VIEW_HEIGHT)
        );
        game.notice_time = 0.0;
        assert_eq!(hud_uv_bounds(Some(&game)).1, 0.0);
        assert_eq!(hud_uv_bounds(None).1, 0.0);
    }

    #[test]
    fn visual_settings_select_profiles_and_protect_motion() {
        let arguments = |values: &[&str]| {
            values
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
        };
        assert!(VisualSettings::from_arguments(&[]).unwrap().motion());
        let retro = VisualSettings::from_arguments(&arguments(&["--fx", "retro"])).unwrap();
        assert_eq!(retro.profile, EffectsProfile::Retro);
        assert!(
            !VisualSettings::from_arguments(&arguments(&["--fx", "clean"]))
                .unwrap()
                .motion()
        );
        assert!(
            !VisualSettings::from_arguments(&arguments(&["--reduced-fx"]))
                .unwrap()
                .motion()
        );
        assert!(VisualSettings::from_arguments(&arguments(&["--fx"])).is_err());
        assert!(VisualSettings::from_arguments(&arguments(&["--fx", "unknown"])).is_err());
    }

    #[test]
    fn action_poses_take_priority_over_walk_and_keep_feet_anchored() {
        let mut animation = Sprite {
            size: [16, 24],
            frames: vec![],
            fps: 10.0,
            walk: vec![1, 0, 2, 0],
        };
        let mut game = Game::new(Save::default());
        game.grounded = true;
        game.velocity.x = 120.0;
        game.time = 0.0;
        assert_eq!(player_pose(&game, &animation).frame, 1);
        game.time = 0.2;
        assert_eq!(player_pose(&game, &animation).frame, 2);
        animation.fps = 20.0;
        assert_eq!(player_pose(&game, &animation).frame, 1);
        animation.walk = vec![2];
        assert_eq!(player_pose(&game, &animation).frame, 2);
        game.grounded = false;
        assert_eq!(player_pose(&game, &animation).frame, 3);
        game.attack_time = 0.1;
        assert_eq!(player_pose(&game, &animation).frame, 4);
        game.damage_flash = 0.1;
        assert_eq!(player_pose(&game, &animation).frame, 3);
        game.dash_time = 0.1;
        let pose = player_pose(&game, &animation);
        assert_eq!(pose.frame, 5);
        assert!(pose.size.x > 32.0 && pose.size.y < 48.0);
    }

    #[test]
    fn extended_poses_follow_the_jump_wall_and_landing_states() {
        let animation = Sprite {
            size: [16, 24],
            frames: vec![],
            fps: 10.0,
            walk: vec![1, 0, 2, 0],
        };
        let mut game = Game::new(Save::default());
        game.grounded = false;
        game.velocity.y = -300.0;
        assert_eq!(player_pose(&game, &animation).frame, 6);
        game.velocity.y = 300.0;
        assert_eq!(player_pose(&game, &animation).frame, 7);
        game.velocity.y = 0.0;
        assert_eq!(player_pose(&game, &animation).frame, 3);
        game.wall_slide = true;
        assert_eq!(player_pose(&game, &animation).frame, 8);
        game.wall_slide = false;
        game.grounded = true;
        game.landing_squash = 0.05;
        assert_eq!(player_pose(&game, &animation).frame, 9);
        // Un ancien pack de six frames retombe sur les poses d'origine.
        assert_eq!(usable_frame(6, 6), 3);
        assert_eq!(usable_frame(6, 8), 3);
        assert_eq!(usable_frame(6, 9), 0);
        assert_eq!(usable_frame(10, 9), 9);
        assert_eq!(usable_frame(6, 4), 4);
    }

    #[test]
    fn extra_frames_change_pixels_without_changing_the_canvas() {
        let base = bitmap_image(
            &[
                "........", "..pppp..", "..pppp..", "..pppp..", "..p..p..", "........",
            ],
            &[(b'p', MINT)],
        );
        let neutral = animated_image(&base, 0, true).bytes;
        for code in [6, 7, 8, 9] {
            let frame = animated_image(&base, code, true);
            assert_eq!((frame.width, frame.height), (base.width, base.height));
            assert_ne!(frame.bytes, neutral, "frame {code}");
        }
    }

    #[test]
    fn sprite_frames_change_pixels_without_changing_canvas() {
        let base = bitmap_image(
            &["........", "..pppp..", "..pppp..", "..pppp.."],
            &[(b'p', MINT)],
        );
        let first = animated_image(&base, 1, true);
        let second = animated_image(&base, 2, true);
        assert_eq!((first.width, first.height), (base.width, base.height));
        assert_ne!(first.bytes, second.bytes);
        assert!(first
            .bytes
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] > 0));
    }

    #[test]
    fn light_mask_fades_without_hard_edges() {
        assert_eq!(light_falloff(0.0), 1.0);
        assert_eq!(light_falloff(1.0), 0.0);
        assert_eq!(light_falloff(2.0), 0.0);
        let image = glow_image();
        assert!(image.get_pixel(64, 64).a > 0.98);
        assert_eq!(image.get_pixel(0, 0).a, 0.0);
        assert_eq!(image.get_pixel(127, 64).a, 0.0);
        for column in 64..127 {
            assert!(image.get_pixel(column + 1, 64).a <= image.get_pixel(column, 64).a);
        }
    }
}
