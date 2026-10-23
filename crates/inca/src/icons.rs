//! Small vector icons drawn with the egui painter (no icon font needed).

use egui::{pos2, vec2, Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Ui};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    New,
    Open,
    Save,
    Undo,
    Redo,
    Object,
    Component,
    Vertex,
    Edge,
    Face,
    Snap,
    World,
    History,
    Render,
    Settings,
    ChannelBox,
    AttrEditor,
    ToolSettings,
    Outliner,
    Select,
    Move,
    Rotate,
    Scale,
    Single,
    Four,
    Two,
    Script,
    Wire,
    Shaded,
    Light,
    XRay,
    Grid,
}

fn arc(c: Pos2, r: f32, a0: f32, a1: f32, n: usize) -> Vec<Pos2> {
    (0..=n).map(|i| {
        let a = a0 + (a1 - a0) * i as f32 / n as f32;
        c + vec2(a.cos(), a.sin()) * r
    }).collect()
}

fn arrow_head(p: &egui::Painter, tip: Pos2, dir: egui::Vec2, s: f32, c: Color32) {
    let d = dir.normalized();
    let n = vec2(-d.y, d.x);
    p.add(Shape::convex_polygon(vec![tip, tip - d * s + n * s * 0.6, tip - d * s - n * s * 0.6], c, Stroke::NONE));
}

pub fn paint(p: &egui::Painter, r: Rect, icon: Icon, c: Color32) {
    let st = Stroke::new(1.4, c);
    let cn = r.center();
    let s = r.width().min(r.height()) * 0.5 - 2.0;
    let rr = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(cn + vec2(x0, y0) * s, cn + vec2(x1, y1) * s);
    match icon {
        Icon::New => {
            p.rect_stroke(rr(-0.6, -0.8, 0.6, 0.8), 1.0, st, egui::StrokeKind::Middle);
            p.line_segment([cn + vec2(0.2, -0.8) * s, cn + vec2(0.6, -0.4) * s], st);
        }
        Icon::Open => {
            p.add(Shape::line(vec![cn + vec2(-0.8, 0.7) * s, cn + vec2(-0.8, -0.6) * s, cn + vec2(-0.3, -0.6) * s, cn + vec2(-0.1, -0.4) * s, cn + vec2(0.6, -0.4) * s, cn + vec2(0.6, -0.1) * s], st));
            p.add(Shape::convex_polygon(vec![cn + vec2(-0.8, 0.7) * s, cn + vec2(-0.5, -0.1) * s, cn + vec2(0.9, -0.1) * s, cn + vec2(0.6, 0.7) * s], c.gamma_multiply(0.6), st));
        }
        Icon::Save => {
            p.rect_stroke(rr(-0.75, -0.75, 0.75, 0.75), 1.5, st, egui::StrokeKind::Middle);
            p.rect_filled(rr(-0.45, -0.75, 0.45, -0.25), 0.0, c);
            p.rect_stroke(rr(-0.45, 0.1, 0.45, 0.75), 0.0, st, egui::StrokeKind::Middle);
        }
        Icon::Undo | Icon::Redo => {
            let flip = if icon == Icon::Undo { 1.0 } else { -1.0 };
            let pts: Vec<Pos2> = arc(cn + vec2(0.0, 0.15) * s, 0.6 * s, std::f32::consts::PI * 1.1, std::f32::consts::PI * 2.05, 12).into_iter().map(|q| pos2(cn.x + (q.x - cn.x) * flip, q.y)).collect();
            let tip = pts[0];
            p.add(Shape::line(pts, st));
            arrow_head(p, tip + vec2(0.0, 0.25 * s), vec2(0.0, 1.0), 0.45 * s, c);
        }
        Icon::Object => {
            let f = [cn + vec2(-0.6, -0.3) * s, cn + vec2(0.0, -0.65) * s, cn + vec2(0.6, -0.3) * s, cn + vec2(0.6, 0.4) * s, cn + vec2(0.0, 0.75) * s, cn + vec2(-0.6, 0.4) * s];
            p.add(Shape::convex_polygon(f.to_vec(), c.gamma_multiply(0.5), st));
            p.line_segment([cn + vec2(0.0, 0.05) * s, cn + vec2(0.0, 0.75) * s], st);
            p.line_segment([cn + vec2(-0.6, -0.3) * s, cn + vec2(0.0, 0.05) * s], st);
            p.line_segment([cn + vec2(0.6, -0.3) * s, cn + vec2(0.0, 0.05) * s], st);
        }
        Icon::Component => {
            paint(p, r, Icon::Object, c.gamma_multiply(0.5));
            for v in [vec2(-0.6, -0.3), vec2(0.0, -0.65), vec2(0.6, -0.3), vec2(0.6, 0.4), vec2(0.0, 0.75), vec2(-0.6, 0.4), vec2(0.0, 0.05)] {
                p.circle_filled(cn + v * s, 1.8, Color32::from_rgb(220, 80, 230));
            }
        }
        Icon::Vertex => {
            for v in [vec2(-0.55, -0.55), vec2(0.55, -0.55), vec2(0.55, 0.55), vec2(-0.55, 0.55)] {
                p.circle_filled(cn + v * s, 2.2, c);
            }
        }
        Icon::Edge => {
            p.rect_stroke(rr(-0.55, -0.55, 0.55, 0.55), 0.0, Stroke::new(1.0, c.gamma_multiply(0.5)), egui::StrokeKind::Middle);
            p.line_segment([cn + vec2(-0.55, 0.55) * s, cn + vec2(0.55, -0.55) * s], Stroke::new(2.2, c));
        }
        Icon::Face => {
            p.rect_filled(rr(-0.6, -0.6, 0.6, 0.6), 0.0, c);
        }
        Icon::Snap | Icon::Grid => {
            for k in [-0.35f32, 0.35] {
                p.line_segment([cn + vec2(k, -0.8) * s, cn + vec2(k, 0.8) * s], st);
                p.line_segment([cn + vec2(-0.8, k) * s, cn + vec2(0.8, k) * s], st);
            }
            if icon == Icon::Snap {
                p.circle_filled(cn + vec2(0.35, 0.35) * s, 2.5, Color32::from_rgb(255, 220, 80));
            }
        }
        Icon::World => {
            p.circle_stroke(cn, 0.75 * s, st);
            p.line_segment([cn + vec2(-0.75, 0.0) * s, cn + vec2(0.75, 0.0) * s], st);
            p.add(Shape::line(arc(cn, 0.75 * s, -1.57, 1.57, 10).into_iter().map(|q| pos2(cn.x + (q.x - cn.x) * 0.45, q.y)).collect(), st));
        }
        Icon::History => {
            p.circle_stroke(cn, 0.72 * s, st);
            p.line_segment([cn, cn + vec2(0.0, -0.5) * s], st);
            p.line_segment([cn, cn + vec2(0.35, 0.2) * s], st);
        }
        Icon::Render => {
            p.rect_stroke(rr(-0.75, -0.3, 0.75, 0.7), 1.0, st, egui::StrokeKind::Middle);
            p.add(Shape::convex_polygon(vec![cn + vec2(-0.75, -0.35) * s, cn + vec2(0.65, -0.75) * s, cn + vec2(0.75, -0.45) * s, cn + vec2(-0.65, -0.05) * s], c.gamma_multiply(0.7), Stroke::NONE));
        }
        Icon::Settings => {
            p.circle_stroke(cn, 0.35 * s, st);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let d = vec2(a.cos(), a.sin());
                p.line_segment([cn + d * 0.5 * s, cn + d * 0.85 * s], Stroke::new(2.0, c));
            }
        }
        Icon::ChannelBox | Icon::AttrEditor | Icon::Outliner | Icon::ToolSettings => {
            p.rect_stroke(rr(-0.8, -0.75, 0.8, 0.75), 1.0, st, egui::StrokeKind::Middle);
            let rows = [-0.4f32, 0.0, 0.4];
            for y in rows {
                let x0 = match icon {
                    Icon::Outliner => -0.55 + (y + 0.4) * 0.4,
                    _ => -0.55,
                };
                p.line_segment([cn + vec2(x0, y) * s, cn + vec2(0.55, y) * s], Stroke::new(1.0, c));
            }
            if icon == Icon::ChannelBox {
                p.line_segment([cn + vec2(0.0, -0.6) * s, cn + vec2(0.0, 0.6) * s], Stroke::new(1.0, c));
            }
            if icon == Icon::ToolSettings {
                p.circle_filled(cn + vec2(-0.3, 0.0) * s, 2.5, c);
            }
        }
        Icon::Select => {
            let pts = vec![cn + vec2(-0.55, -0.8) * s, cn + vec2(0.55, 0.15) * s, cn + vec2(0.05, 0.2) * s, cn + vec2(0.35, 0.8) * s, cn + vec2(0.15, 0.85) * s, cn + vec2(-0.15, 0.3) * s, cn + vec2(-0.55, 0.6) * s];
            p.add(Shape::closed_line(pts, Stroke::new(1.3, c)));
        }
        Icon::Move => {
            for d in [vec2(1.0, 0.0), vec2(-1.0, 0.0), vec2(0.0, 1.0), vec2(0.0, -1.0)] {
                p.line_segment([cn, cn + d * 0.6 * s], Stroke::new(1.6, c));
                arrow_head(p, cn + d * 0.9 * s, d, 0.35 * s, c);
            }
        }
        Icon::Rotate => {
            let pts = arc(cn, 0.65 * s, -0.4, 4.8, 20);
            let tip = *pts.last().unwrap();
            let prev = pts[pts.len() - 2];
            p.add(Shape::line(pts, Stroke::new(1.6, c)));
            arrow_head(p, tip + (tip - prev).normalized() * 0.2 * s, tip - prev, 0.4 * s, c);
        }
        Icon::Scale => {
            p.rect_stroke(rr(-0.7, -0.1, 0.1, 0.7), 0.0, st, egui::StrokeKind::Middle);
            p.line_segment([cn + vec2(0.1, -0.1) * s, cn + vec2(0.65, -0.65) * s], Stroke::new(1.6, c));
            p.rect_filled(rr(0.45, -0.85, 0.85, -0.45), 0.0, c);
        }
        Icon::Single => {
            p.rect_stroke(rr(-0.75, -0.65, 0.75, 0.65), 0.0, st, egui::StrokeKind::Middle);
        }
        Icon::Four => {
            p.rect_stroke(rr(-0.75, -0.65, 0.75, 0.65), 0.0, st, egui::StrokeKind::Middle);
            p.line_segment([cn + vec2(0.0, -0.65) * s, cn + vec2(0.0, 0.65) * s], st);
            p.line_segment([cn + vec2(-0.75, 0.0) * s, cn + vec2(0.75, 0.0) * s], st);
        }
        Icon::Two => {
            p.rect_stroke(rr(-0.75, -0.65, 0.75, 0.65), 0.0, st, egui::StrokeKind::Middle);
            p.line_segment([cn + vec2(0.0, -0.65) * s, cn + vec2(0.0, 0.65) * s], st);
        }
        Icon::Script => {
            p.text(cn, egui::Align2::CENTER_CENTER, "{ }", egui::FontId::monospace(s * 1.3), c);
        }
        Icon::Wire => {
            p.rect_stroke(rr(-0.65, -0.35, 0.35, 0.65), 0.0, Stroke::new(1.0, c), egui::StrokeKind::Middle);
            p.rect_stroke(rr(-0.35, -0.65, 0.65, 0.35), 0.0, Stroke::new(1.0, c), egui::StrokeKind::Middle);
            for (a, b) in [(vec2(-0.65, -0.35), vec2(-0.35, -0.65)), (vec2(0.35, -0.35), vec2(0.65, -0.65)), (vec2(0.35, 0.65), vec2(0.65, 0.35)), (vec2(-0.65, 0.65), vec2(-0.35, 0.35))] {
                p.line_segment([cn + a * s, cn + b * s], Stroke::new(1.0, c));
            }
        }
        Icon::Shaded => {
            p.circle_filled(cn, 0.72 * s, c.gamma_multiply(0.75));
            p.circle_filled(cn + vec2(-0.25, -0.25) * s, 0.25 * s, Color32::from_white_alpha(120));
        }
        Icon::Light => {
            p.circle_stroke(cn + vec2(0.0, -0.15) * s, 0.5 * s, st);
            p.line_segment([cn + vec2(-0.2, 0.55) * s, cn + vec2(0.2, 0.55) * s], st);
            p.line_segment([cn + vec2(-0.15, 0.8) * s, cn + vec2(0.15, 0.8) * s], st);
        }
        Icon::XRay => {
            p.circle_filled(cn + vec2(-0.2, 0.0) * s, 0.55 * s, c.gamma_multiply(0.4));
            p.circle_stroke(cn + vec2(0.2, 0.0) * s, 0.55 * s, st);
        }
    }
}

/// A square tool button with a drawn icon.
pub fn button(ui: &mut Ui, icon: Icon, tip: &str, on: bool, size: f32) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(size, size - 2.0), Sense::click());
    let v = ui.visuals();
    let bg = if on {
        crate::theme::HI
    } else if resp.hovered() {
        v.widgets.hovered.bg_fill
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(r, 3.0, bg);
    let c = if on { Color32::WHITE } else { crate::theme::TEXT };
    paint(ui.painter(), r.shrink(2.0), icon, c);
    resp.on_hover_text(tip)
}
