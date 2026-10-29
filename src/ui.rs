//! Systeme de design de l'interface : panneaux biseautes, degrades, jauges et logotype.
//!
//! Toutes les fonctions dessinent en coordonnees ecran et ne lisent aucun etat de jeu.

use crate::game::WIDTH;
use crate::render::{Art, INK, MINT, PAPER, VIEW_HEIGHT};
use macroquad::prelude::*;

pub const PANEL: Color = Color::new(0.075, 0.118, 0.106, 0.94);
pub const PANEL_ACTIVE: Color = Color::new(0.145, 0.235, 0.205, 0.97);
pub const EDGE: Color = Color::new(0.29, 0.40, 0.35, 1.0);
/// Texte secondaire et jauges vides.
pub const DIM: Color = Color::new(0.47, 0.55, 0.50, 1.0);
// Les panneaux restent quasi opaques pour garder le texte lisible sur n'importe quel decor.
const _: () = assert!(PANEL.a > 0.9 && PANEL_ACTIVE.a > 0.9);

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color::new(color.r, color.g, color.b, alpha)
}

/// Degrade vertical sans texture ; les sommets portent seuls la couleur.
pub fn gradient(rect: Rect, top: Color, bottom: Color) {
    let vertex = |x: f32, y: f32, color: Color| Vertex::new2(vec3(x, y, 0.0), Vec2::ZERO, color);
    draw_mesh(&Mesh {
        vertices: vec![
            vertex(rect.x, rect.y, top),
            vertex(rect.right(), rect.y, top),
            vertex(rect.right(), rect.bottom(), bottom),
            vertex(rect.x, rect.bottom(), bottom),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: None,
    });
}

/// Rectangle aux quatre coins coupes ; `cut` ne doit pas depasser la moitie du petit cote.
pub fn chamfer(rect: Rect, cut: f32, color: Color) {
    let cut = cut.min(rect.w.min(rect.h) * 0.5);
    draw_rectangle(rect.x + cut, rect.y, rect.w - cut * 2.0, rect.h, color);
    draw_rectangle(rect.x, rect.y + cut, rect.w, rect.h - cut * 2.0, color);
    let (x, y, r, b) = (rect.x, rect.y, rect.right(), rect.bottom());
    draw_triangle(
        vec2(x, y + cut),
        vec2(x + cut, y),
        vec2(x + cut, y + cut),
        color,
    );
    draw_triangle(
        vec2(r - cut, y),
        vec2(r, y + cut),
        vec2(r - cut, y + cut),
        color,
    );
    draw_triangle(
        vec2(x, b - cut),
        vec2(x + cut, b),
        vec2(x + cut, b - cut),
        color,
    );
    draw_triangle(
        vec2(r, b - cut),
        vec2(r - cut, b),
        vec2(r - cut, b - cut),
        color,
    );
}

pub fn chamfer_outline(rect: Rect, cut: f32, thickness: f32, color: Color) {
    let cut = cut.min(rect.w.min(rect.h) * 0.5);
    let (x, y, r, b) = (rect.x, rect.y, rect.right(), rect.bottom());
    let points = [
        vec2(x + cut, y),
        vec2(r - cut, y),
        vec2(r, y + cut),
        vec2(r, b - cut),
        vec2(r - cut, b),
        vec2(x + cut, b),
        vec2(x, b - cut),
        vec2(x, y + cut),
    ];
    for index in 0..points.len() {
        let (from, to) = (points[index], points[(index + 1) % points.len()]);
        draw_line(from.x, from.y, to.x, to.y, thickness, color);
    }
}

/// Panneau d'interface : ombre portee, fond biseaute, reflet haut et contour d'accent.
pub fn panel(rect: Rect, accent: Color, active: bool) {
    chamfer(
        Rect::new(rect.x, rect.y + 3.0, rect.w, rect.h),
        5.0,
        Color::new(0.0, 0.0, 0.0, 0.32),
    );
    chamfer(rect, 5.0, if active { PANEL_ACTIVE } else { PANEL });
    draw_line(
        rect.x + 6.0,
        rect.y + 1.5,
        rect.right() - 6.0,
        rect.y + 1.5,
        1.0,
        with_alpha(PAPER, if active { 0.22 } else { 0.10 }),
    );
    chamfer_outline(rect, 5.0, 1.0, if active { accent } else { EDGE });
}

/// Jauge horizontale ; `ratio` est borne pour rester dans le cadre.
pub fn bar(rect: Rect, ratio: f32, fill: Color) {
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, with_alpha(INK, 0.9));
    let width = rect.w * ratio.clamp(0.0, 1.0);
    if width > 0.0 {
        gradient(
            Rect::new(rect.x, rect.y, width, rect.h),
            fill,
            Color::new(fill.r * 0.6, fill.g * 0.6, fill.b * 0.6, fill.a),
        );
        draw_rectangle(rect.x, rect.y, width, 1.0, with_alpha(WHITE, 0.35));
    }
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, with_alpha(EDGE, 0.8));
}

/// Fond des ecrans plein cadre, sous le HUD haut de 56 pixels.
pub fn overlay() {
    gradient(
        Rect::new(0.0, 56.0, WIDTH, VIEW_HEIGHT - 56.0),
        Color::new(0.035, 0.055, 0.05, 0.97),
        Color::new(0.07, 0.11, 0.10, 0.98),
    );
    draw_rectangle(0.0, 56.0, WIDTH, 1.0, with_alpha(MINT, 0.35));
}

/// Texte avec ombre portee, pour rester lisible sur les decors clairs.
pub fn shadowed(art: &Art, text: &str, position: Vec2, size: u16, color: Color) {
    art.text(text, position + vec2(2.0, 2.0), size, with_alpha(INK, 0.85));
    art.text(text, position, size, color);
}

/// Texte reduit si besoin pour tenir dans `max_width`, sans jamais l'agrandir.
pub fn fitted(art: &Art, text: &str, position: Vec2, size: u16, color: Color, max_width: f32) {
    let measured = measure_text(text, Some(&art.font), size, 1.0).width;
    draw_text_ex(
        text,
        position.x,
        position.y,
        TextParams {
            font: Some(&art.font),
            font_size: size,
            font_scale: (max_width / measured.max(1.0)).min(1.0),
            color,
            ..Default::default()
        },
    );
}

/// Logotype espace du titre, avec halo et filet d'accent.
pub fn wordmark(art: &Art, center: Vec2, text: &str, size: u16) {
    let spacing = f32::from(size) * 0.18;
    let widths: Vec<f32> = text
        .chars()
        .map(|letter| {
            measure_text(letter.encode_utf8(&mut [0; 4]), Some(&art.font), size, 1.0).width
        })
        .collect();
    let total = widths.iter().sum::<f32>() + spacing * (widths.len().saturating_sub(1)) as f32;
    let mut x = center.x - total * 0.5;
    for (letter, width) in text.chars().zip(&widths) {
        let letter = letter.to_string();
        let position = vec2(x, center.y);
        art.text(
            &letter,
            position + vec2(-2.0, 0.0),
            size,
            with_alpha(MINT, 0.18),
        );
        art.text(
            &letter,
            position + vec2(2.0, 0.0),
            size,
            with_alpha(MINT, 0.18),
        );
        art.text(
            &letter,
            position + vec2(3.0, 4.0),
            size,
            with_alpha(INK, 0.9),
        );
        art.text(&letter, position, size, PAPER);
        x += width + spacing;
    }
    let half = total * 0.5 + 18.0;
    let y = center.y + 16.0;
    gradient(
        Rect::new(center.x - half, y, half, 2.0),
        with_alpha(MINT, 0.0),
        with_alpha(MINT, 0.8),
    );
    draw_triangle(
        vec2(center.x, y - 5.0),
        vec2(center.x + 6.0, y + 1.0),
        vec2(center.x, y + 7.0),
        MINT,
    );
    draw_triangle(
        vec2(center.x, y - 5.0),
        vec2(center.x - 6.0, y + 1.0),
        vec2(center.x, y + 7.0),
        MINT,
    );
    // Le degrade de droite est inverse : opaque au centre, transparent au bord.
    let right = Rect::new(center.x, y, half, 2.0);
    let vertex = |x: f32, y: f32, alpha: f32| {
        Vertex::new2(vec3(x, y, 0.0), Vec2::ZERO, with_alpha(MINT, alpha))
    };
    draw_mesh(&Mesh {
        vertices: vec![
            vertex(right.x, right.y, 0.8),
            vertex(right.right(), right.y, 0.0),
            vertex(right.right(), right.bottom(), 0.0),
            vertex(right.x, right.bottom(), 0.8),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: None,
    });
}

/// Vignette douce du monde : assombrit les bords haut et bas pour cadrer l'action.
pub fn world_vignette(top: f32, height: f32) {
    let shade = Color::new(INK.r, INK.g, INK.b, 0.38);
    let clear = Color::new(INK.r, INK.g, INK.b, 0.0);
    gradient(Rect::new(0.0, top, WIDTH, 64.0), shade, clear);
    gradient(
        Rect::new(0.0, top + height - 72.0, WIDTH, 72.0),
        clear,
        shade,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_keeps_text_and_panels_distinct_and_opaque_enough() {
        // Le texte clair doit rester nettement plus lumineux que les panneaux.
        let luminance = |color: Color| color.r * 0.299 + color.g * 0.587 + color.b * 0.114;
        assert!(luminance(PAPER) - luminance(PANEL) > 0.6);
        assert!(luminance(DIM) > luminance(PANEL_ACTIVE));
    }

    #[test]
    fn alpha_helper_only_changes_opacity() {
        let faded = with_alpha(MINT, 0.25);
        assert_eq!(
            (faded.r, faded.g, faded.b, faded.a),
            (MINT.r, MINT.g, MINT.b, 0.25)
        );
    }
}
