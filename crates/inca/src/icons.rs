//! The JS app's SVG icon set (icons_svg.rs, generated from src/js/ui/icons.js), rasterized with
//! resvg into cached egui textures.

use crate::icons_svg::{CMD_ICONS, ICONS};
use egui::{vec2, Color32, Rect, Response, Sense, TextureHandle, Ui};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Icon name for an action id: the JS command's icon, an icon of the same name, or `help`.
pub fn for_action(id: &str) -> &'static str {
    if let Some((_, i)) = CMD_ICONS.iter().find(|(c, _)| *c == id) {
        return i;
    }
    if let Some((n, _)) = ICONS.iter().find(|(n, _)| *n == id) {
        return n;
    }
    // a few Rust action ids without a JS twin
    match id {
        "createDirectionalEmitter" | "createEmitter" | "emitFromObject" => "pointLight",
        "gravityField" | "dragField" | "turbulenceField" | "radialField" | "vortexField" | "airField" => "move",
        _ if id.starts_with("assignNew_") => match &id[10..] {
            "lambert" => "lambert",
            "blinn" => "blinn",
            "phong" => "phong",
            "surfaceShader" => "surfaceShader",
            _ => "standardSurface",
        },
        _ => "help",
    }
}

fn fontdb() -> Arc<resvg::usvg::fontdb::Database> {
    static DB: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        db.set_sans_serif_family("DejaVu Sans");
        Arc::new(db)
    })
    .clone()
}

fn rasterize(svg: &str, px: u32) -> Option<egui::ColorImage> {
    let opt = resvg::usvg::Options { fontdb: fontdb(), ..Default::default() };
    let tree = resvg::usvg::Tree::from_str(svg, &opt).ok()?;
    let mut pm = resvg::tiny_skia::Pixmap::new(px, px)?;
    let s = tree.size();
    let k = px as f32 / s.width().max(s.height());
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(k, k), &mut pm.as_mut());
    // tiny-skia pixels are premultiplied
    Some(egui::ColorImage::from_rgba_premultiplied([px as usize, px as usize], pm.data()))
}

#[derive(Clone, Default)]
struct Cache(Arc<Mutex<HashMap<(&'static str, u32), TextureHandle>>>);

/// Texture for an icon at `size` points (rendered at the screen's pixel density).
pub fn texture(ctx: &egui::Context, name: &str, size: f32) -> Option<egui::TextureId> {
    let (name, svg) = ICONS.iter().find(|(n, _)| *n == name).or_else(|| ICONS.iter().find(|(n, _)| *n == "help"))?;
    let px = (size * ctx.pixels_per_point()).round().max(8.0) as u32;
    let cache = ctx.data_mut(|d| d.get_temp_mut_or_default::<Cache>(egui::Id::new("inca-icons")).clone());
    let mut map = cache.0.lock().ok()?;
    if let Some(t) = map.get(&(*name, px)) {
        return Some(t.id());
    }
    let img = rasterize(svg, px)?;
    let t = ctx.load_texture(format!("icon-{name}-{px}"), img, egui::TextureOptions::LINEAR);
    let id = t.id();
    map.insert((*name, px), t);
    Some(id)
}

pub fn paint(ui: &Ui, rect: Rect, name: &str, tint: Color32) {
    if let Some(t) = texture(ui.ctx(), name, rect.width()) {
        ui.painter().image(t, rect, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), tint);
    }
}

/// A square icon button like the JS `.ib` (hover grey, blue-grey when on).
pub fn button(ui: &mut Ui, name: &str, tip: &str, on: bool, size: f32) -> Response {
    button_ex(ui, name, tip, on, size, size - 4.0, true)
}

pub fn button_ex(ui: &mut Ui, name: &str, tip: &str, on: bool, size: f32, icon: f32, enabled: bool) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(size, size), if enabled { Sense::click() } else { Sense::hover() });
    if on {
        ui.painter().rect_filled(r, 3.0, Color32::from_rgb(0x5d, 0x7a, 0x8f));
        ui.painter().rect_stroke(r.shrink(0.5), 3.0, egui::Stroke::new(1.0, Color32::from_rgb(0x81, 0xa9, 0xc4)), egui::StrokeKind::Inside);
    } else if resp.hovered() && enabled {
        ui.painter().rect_filled(r, 3.0, Color32::from_rgb(0x57, 0x57, 0x57));
    }
    let tint = if enabled { Color32::WHITE } else { Color32::from_white_alpha(100) };
    paint(ui, Rect::from_center_size(r.center(), vec2(icon, icon)), name, tint);
    if tip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tip)
    }
}

/// Dotted double-line separator between status line groups.
pub fn separator(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(8.0, 22.0), Sense::hover());
    let c = Color32::from_gray(0x77);
    for x in [r.min.x + 3.0, r.min.x + 5.0] {
        let mut y = r.min.y + 3.0;
        while y < r.max.y - 3.0 {
            ui.painter().line_segment([egui::pos2(x, y), egui::pos2(x, y + 1.0)], egui::Stroke::new(1.0, c));
            y += 2.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_rasterizes() {
        for (n, svg) in ICONS {
            assert!(rasterize(svg, 24).is_some(), "{n}");
        }
        assert_eq!(for_action("polyCube"), "polyCube");
        assert_eq!(for_action("moveTool"), "move");
    }
}
