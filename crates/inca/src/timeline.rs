//! Time Slider, Range Slider, playback controls, command line and help line.

use crate::app::IncaApp;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, RichText, Sense, Stroke};

pub fn time_slider(app: &mut IncaApp, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        let ctl_w = 260.0;
        let w = (ui.available_width() - ctl_w).max(100.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click_and_drag());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 0.0, theme::BG2);
        let (t0, t1) = (app.s.time.start, app.s.time.end.max(app.s.time.start + 1.0));
        let x_of = |t: f64| rect.min.x + 8.0 + ((t - t0) / (t1 - t0)) as f32 * (rect.width() - 16.0);
        let span = t1 - t0;
        let step = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0].into_iter().find(|s| (span / s) * 40.0 < rect.width() as f64).unwrap_or(1000.0);
        let mut t = (t0 / step).ceil() * step;
        while t <= t1 {
            let x = x_of(t);
            p.line_segment([pos2(x, rect.max.y - 10.0), pos2(x, rect.max.y)], Stroke::new(1.0, theme::TEXT_DIM));
            p.text(pos2(x + 2.0, rect.max.y - 11.0), egui::Align2::LEFT_BOTTOM, inca_core::math::fmt_num_long(t), egui::FontId::proportional(10.0), theme::TEXT_DIM);
            t += step;
        }
        // key ticks of the selection
        for k in app.s.anim.key_times(&app.s.sel) {
            if k >= t0 && k <= t1 {
                let x = x_of(k);
                p.rect_filled(Rect::from_min_max(pos2(x - 1.5, rect.min.y + 6.0), pos2(x + 1.5, rect.max.y - 2.0)), 0.0, theme::KEY);
            }
        }
        let cx = x_of(app.s.time.current);
        p.rect_filled(Rect::from_min_max(pos2(cx - 3.0, rect.min.y), pos2(cx + 3.0, rect.max.y)), 0.0, Color32::from_rgba_unmultiplied(220, 220, 220, 140));
        p.text(pos2(cx + 5.0, rect.min.y + 1.0), egui::Align2::LEFT_TOP, inca_core::math::fmt_num_long(app.s.time.current), egui::FontId::proportional(11.0), Color32::WHITE);
        if resp.dragged() || resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let t = (t0 + ((pos.x - rect.min.x - 8.0) / (rect.width() - 16.0)) as f64 * (t1 - t0)).round().clamp(t0, t1);
                if t != app.s.time.current {
                    app.s.set_time(t);
                }
            }
        }
        // playback controls
        let mut cur = app.s.time.current;
        if ui.add(egui::DragValue::new(&mut cur).speed(0.2).max_decimals(2)).changed() {
            app.s.set_time(cur);
        }
        for (g, a, tip) in [("⏮", "goStart", "Go to start"), ("◀|", "prevKey", "Previous key"), ("◀", "prevFrame", "Step back one frame")] {
            if ui.small_button(g).on_hover_text(tip).clicked() {
                app.action(a);
            }
        }
        if ui.small_button(if app.playing { "⏸" } else { "▶" }).on_hover_text("Play / Stop (Alt+V)").clicked() {
            app.action("play");
        }
        for (g, a, tip) in [("▶", "nextFrame", "Step forward one frame"), ("|▶", "nextKey", "Next key"), ("⏭", "goEnd", "Go to end")] {
            if ui.small_button(g).on_hover_text(tip).clicked() {
                app.action(a);
            }
        }
        let ak = app.s.time.auto_key;
        if ui.add(egui::Button::new(RichText::new("⏺").color(if ak { theme::KEY } else { theme::TEXT_DIM })).selected(ak)).on_hover_text("Auto Key").clicked() {
            app.s.time.auto_key = !ak;
        }
    });
}

pub fn range_slider(app: &mut IncaApp, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        let t = &mut app.s.time;
        ui.add(egui::DragValue::new(&mut t.anim_start).speed(0.5)).on_hover_text("Animation start");
        ui.add(egui::DragValue::new(&mut t.start).speed(0.5)).on_hover_text("Playback start");
        let w = (ui.available_width() - 260.0).max(80.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 18.0), Sense::drag());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 2.0, theme::BG_DARK);
        let span = (t.anim_end - t.anim_start).max(1.0);
        let xa = rect.min.x + ((t.start - t.anim_start) / span) as f32 * rect.width();
        let xb = rect.min.x + ((t.end - t.anim_start) / span) as f32 * rect.width();
        p.rect_filled(Rect::from_min_max(pos2(xa, rect.min.y + 2.0), pos2(xb.max(xa + 6.0), rect.max.y - 2.0)), 6.0, theme::BTN_HOVER);
        if resp.dragged() {
            let d = (resp.drag_delta().x / rect.width()) as f64 * span;
            let len = t.end - t.start;
            t.start = (t.start + d).clamp(t.anim_start, t.anim_end - len);
            t.end = t.start + len;
        }
        if resp.drag_stopped() {
            t.start = t.start.round();
            t.end = t.end.round();
        }
        ui.add(egui::DragValue::new(&mut t.end).speed(0.5)).on_hover_text("Playback end");
        ui.add(egui::DragValue::new(&mut t.anim_end).speed(0.5)).on_hover_text("Animation end");
        egui::ComboBox::from_id_salt("fps").width(70.0).selected_text(format!("{} fps", t.fps)).show_ui(ui, |ui| {
            for f in [12.0, 15.0, 24.0, 25.0, 30.0, 48.0, 50.0, 60.0] {
                ui.selectable_value(&mut t.fps, f, format!("{f} fps"));
            }
        });
        t.end = t.end.max(t.start + 1.0);
    });
}

pub fn command_line(app: &mut IncaApp, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        ui.label(RichText::new("MEL").strong());
        let w = ui.available_width() * 0.55;
        let r = ui.add(egui::TextEdit::singleline(&mut app.cmdline).desired_width(w).font(egui::TextStyle::Monospace).hint_text("type a MEL command, e.g. polyCube -w 2;"));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let src = std::mem::take(&mut app.cmdline);
            if !src.trim().is_empty() {
                app.cmd_history.push(src.clone());
                let _ = app.run_mel("command line", &src);
            }
            r.request_focus();
        }
        if r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
            if let Some(last) = app.cmd_history.last() {
                app.cmdline = last.clone();
            }
        }
        let color = if app.last_result.contains("Error") {
            Color32::from_rgb(240, 110, 110)
        } else if app.last_result.contains("Warning") {
            Color32::from_rgb(240, 200, 90)
        } else {
            theme::TEXT
        };
        egui::Frame::NONE.fill(theme::FIELD).inner_margin(egui::Margin::symmetric(4, 1)).show(ui, |ui| {
            ui.set_width(ui.available_width() - 30.0);
            ui.label(RichText::new(&app.last_result).monospace().color(color));
        });
        if ui.small_button("{}").on_hover_text("Script Editor").clicked() {
            app.win.script_editor = !app.win.script_editor;
        }
    });
}

pub fn help_line(app: &mut IncaApp, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        let (v, e, f, t) = app.s.poly_count();
        let txt = if app.help.is_empty() { "Hold Alt and drag in a viewport to tumble (LMB), track (MMB) or dolly (RMB)".to_string() } else { app.help.clone() };
        ui.label(RichText::new(txt).color(theme::TEXT_DIM).size(11.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(format!("Verts {v}  Edges {e}  Faces {f}  Tris {t}")).color(theme::TEXT_DIM).size(11.0));
        });
    });
}
