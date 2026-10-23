//! Maya-style dark theme (colours from the JS app's inca.css).

use egui::{Color32, CornerRadius, Frame, Margin, Stroke, Visuals};

pub const BG: Color32 = Color32::from_rgb(0x44, 0x44, 0x44);
pub const BG2: Color32 = Color32::from_rgb(0x3c, 0x3c, 0x3c);
pub const BG3: Color32 = Color32::from_rgb(0x37, 0x37, 0x37);
pub const BG_DARK: Color32 = Color32::from_rgb(0x2b, 0x2b, 0x2b);
pub const FIELD: Color32 = Color32::from_rgb(0x2b, 0x2b, 0x2b);
pub const LINE: Color32 = Color32::from_rgb(0x2a, 0x2a, 0x2a);
pub const TEXT: Color32 = Color32::from_rgb(0xcc, 0xcc, 0xcc);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x8f, 0x8f, 0x8f);
pub const HI: Color32 = Color32::from_rgb(0x52, 0x85, 0xa6);
pub const BTN: Color32 = Color32::from_rgb(0x5d, 0x5d, 0x5d);
pub const BTN_HOVER: Color32 = Color32::from_rgb(0x6f, 0x6f, 0x6f);
pub const KEYED: Color32 = Color32::from_rgb(0xdd, 0x72, 0x7a);
pub const KEY: Color32 = Color32::from_rgb(0xcd, 0x5c, 0x5c);
pub const LOCKED: Color32 = Color32::from_rgb(0x5c, 0x68, 0x74);
pub const DRIVEN: Color32 = Color32::from_rgb(0xe6, 0xc5, 0x4a);
pub const ACCENT: Color32 = Color32::from_rgb(0x48, 0xaa, 0xb5);

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.override_text_color = Some(TEXT);
    v.panel_fill = BG;
    v.window_fill = BG;
    v.extreme_bg_color = FIELD;
    v.faint_bg_color = BG2;
    v.code_bg_color = BG_DARK;
    v.window_stroke = Stroke::new(1.0, LINE);
    v.selection.bg_fill = HI;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.hyperlink_color = ACCENT;
    let r = CornerRadius::same(2);
    v.widgets.noninteractive.bg_fill = BG;
    v.widgets.noninteractive.weak_bg_fill = BG;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.bg_fill = BTN;
    v.widgets.inactive.weak_bg_fill = BTN;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.bg_fill = BTN_HOVER;
    v.widgets.hovered.weak_bg_fill = BTN_HOVER;
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.bg_fill = Color32::from_rgb(0x82, 0x82, 0x82);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0x82, 0x82, 0x82);
    v.widgets.active.corner_radius = r;
    v.widgets.open.weak_bg_fill = HI;
    v.menu_corner_radius = CornerRadius::same(2);
    v.window_corner_radius = CornerRadius::same(3);
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(4.0, 3.0);
        s.spacing.button_padding = egui::vec2(6.0, 2.0);
        s.spacing.interact_size.y = 18.0;
        s.spacing.menu_margin = Margin::same(3);
        for (_, f) in s.text_styles.iter_mut() {
            f.size = (f.size * 0.92).round();
        }
    });
}

pub fn bar_frame() -> Frame {
    Frame::NONE.fill(BG).inner_margin(Margin::symmetric(3, 2)).stroke(Stroke::new(1.0, LINE))
}

pub fn panel_frame() -> Frame {
    Frame::NONE.fill(BG).inner_margin(Margin::same(4)).stroke(Stroke::new(1.0, LINE))
}

/// A collapsible section header (Attribute Editor frames); returns whether it is open.
pub fn fold(ui: &mut egui::Ui, text: &str) -> bool {
    let id = ui.make_persistent_id(("fold", text));
    let mut open = ui.data_mut(|d| *d.get_persisted_mut_or(id, true));
    let (r, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 18.0), egui::Sense::click());
    ui.painter().rect_filled(r, 0.0, if resp.hovered() { Color32::from_gray(0x5a) } else { Color32::from_gray(0x52) });
    let c = egui::pos2(r.min.x + 9.0, r.center().y);
    let pts = if open {
        vec![c + egui::vec2(-3.5, -2.0), c + egui::vec2(3.5, -2.0), c + egui::vec2(0.0, 2.5)]
    } else {
        vec![c + egui::vec2(-2.0, -3.5), c + egui::vec2(2.5, 0.0), c + egui::vec2(-2.0, 3.5)]
    };
    ui.painter().add(egui::Shape::convex_polygon(pts, Color32::from_gray(0xdd), Stroke::NONE));
    ui.painter().text(egui::pos2(r.min.x + 18.0, r.center().y), egui::Align2::LEFT_CENTER, text, egui::FontId::proportional(12.0), Color32::from_gray(0xee));
    if resp.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_persisted(id, open));
    }
    open
}

pub fn section_header(ui: &mut egui::Ui, text: &str) {
    egui::Frame::NONE.fill(BG3).inner_margin(Margin::symmetric(4, 2)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(egui::RichText::new(text).strong().color(Color32::from_rgb(0xdd, 0xdd, 0xdd)));
    });
}
