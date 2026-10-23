//! Time Slider, Range Slider, playback controls, command line and help line.

use crate::app::IncaApp;
use crate::icons;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, RichText, Sense, Stroke};

/// Frame rates with Maya's names (the JS FPS list).
const FPS: &[(&str, f64)] = &[("15 fps", 15.0), ("film (24 fps)", 24.0), ("PAL (25 fps)", 25.0), ("NTSC (30 fps)", 30.0), ("show (48 fps)", 48.0), ("PAL Field (50 fps)", 50.0), ("NTSC Field (60 fps)", 60.0), ("120 fps", 120.0)];

fn fps_label(f: f64) -> String {
    FPS.iter().find(|x| x.1 == f).map(|x| x.0.to_string()).unwrap_or_else(|| format!("{} fps", fmt(f)))
}

fn fmt(t: f64) -> String {
    inca_core::math::fmt_num_long(t)
}

pub fn time_slider(app: &mut IncaApp, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        let ctl_w = 250.0;
        let w = (ui.available_width() - ctl_w).max(100.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click_and_drag());
        slider_canvas(app, ui, rect, &resp);
        // current time field and transport buttons (JS order)
        let mut cur = app.s.time.current;
        if ui.add(egui::DragValue::new(&mut cur).speed(0.2).max_decimals(2)).on_hover_text("Current time").changed() {
            app.s.set_time(cur);
        }
        let ib = |ui: &mut egui::Ui, i: &str, tip: &str, on: bool| icons::button(ui, i, tip, on, 20.0).clicked();
        let (playing, dir) = (app.playing, app.play_dir);
        if ib(ui, "goStart", "Go to start of playback range (Shift+Alt+V)", false) {
            app.action("goToStart");
        }
        if ib(ui, "stepBack", "Step back one frame (Alt+,)", false) {
            app.action("prevFrame");
        }
        if ib(ui, "keyBack", "Step back one key (,)", false) {
            app.action("prevKey");
        }
        if ib(ui, if playing && dir < 0.0 { "stop" } else { "playBack" }, "Play backwards", playing && dir < 0.0) {
            app.action("playBackwards");
        }
        if ib(ui, if playing && dir > 0.0 { "stop" } else { "playFwd" }, "Play forwards (Alt+V). Press Esc to stop", playing && dir > 0.0) {
            app.action("playToggle");
        }
        if ib(ui, "keyFwd", "Step forward one key (.)", false) {
            app.action("nextKey");
        }
        if ib(ui, "stepFwd", "Step forward one frame (Alt+.)", false) {
            app.action("nextFrame");
        }
        if ib(ui, "goEnd", "Go to end of playback range", false) {
            app.action("goToEnd");
        }
    });
}

/// The Time Slider strip: ticks, keys, range selection, current time; scrub / Shift-select / MMB / RMB.
fn slider_canvas(app: &mut IncaApp, ui: &mut egui::Ui, rect: Rect, resp: &egui::Response) {
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, Color32::from_gray(0x52));
    let (t0, t1) = (app.s.time.start, app.s.time.end.max(app.s.time.start + 1.0));
    let pad = 8.0;
    let x_of = |t: f64| rect.min.x + pad + ((t - t0) / (t1 - t0)) as f32 * (rect.width() - 2.0 * pad);
    let t_of = |x: f32| (t0 + ((x - rect.min.x - pad) / (rect.width() - 2.0 * pad)) as f64 * (t1 - t0)).round().clamp(t0, t1);
    let px_per = (rect.width() - 2.0 * pad) as f64 / (t1 - t0);
    let major = [1.0, 2.0, 5.0, 10.0, 20.0, 25.0, 50.0, 100.0, 200.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0].into_iter().find(|s| s * px_per >= 45.0).unwrap_or(10000.0);
    let mut minor = (major / 5.0_f64).max(1.0);
    if minor * px_per < 4.0 {
        minor = major;
    }
    // range selection
    if let Some((a, b)) = app.range_sel {
        let (lo, hi) = (a.min(b), a.max(b));
        let (xa, xb) = (x_of(lo), x_of(hi + 1.0));
        p.rect_filled(Rect::from_min_max(pos2(xa, rect.min.y), pos2(xb.max(xa + 2.0), rect.max.y)), 0.0, Color32::from_rgba_unmultiplied(200, 50, 50, 140));
    }
    let mut t = (t0 / minor).ceil() * minor;
    while t <= t1 + 1e-6 {
        let x = x_of(t).round() + 0.5;
        let is_major = ((t / major) - (t / major).round()).abs() < 1e-6;
        p.line_segment([pos2(x, rect.max.y), pos2(x, rect.max.y - if is_major { 14.0 } else { 6.0 })], Stroke::new(1.0, Color32::from_gray(0x8a)));
        if is_major {
            p.text(pos2(x + 2.0, rect.max.y - 12.0), egui::Align2::LEFT_BOTTOM, fmt(t), egui::FontId::proportional(10.5), Color32::from_gray(0xd0));
        }
        t += minor;
    }
    // keys of the selection
    for k in app.s.anim.key_times(&app.s.sel) {
        if k >= t0 && k <= t1 {
            let x = x_of(k).round();
            p.rect_filled(Rect::from_min_size(pos2(x - 1.0, rect.max.y - 22.0), vec2((px_per as f32 * 0.18).max(2.0), 22.0)), 0.0, Color32::from_rgb(0xd7, 0x3b, 0x3e));
        }
    }
    // current time: grey frame band, white line, label box
    let cx = x_of(app.s.time.current);
    let band = (px_per as f32).max(2.0);
    p.rect_filled(Rect::from_min_size(pos2(cx - band / 2.0, rect.min.y), vec2(band, rect.height())), 0.0, Color32::from_rgba_unmultiplied(150, 150, 150, 140));
    p.rect_filled(Rect::from_min_size(pos2(cx.round() - 1.0, rect.min.y), vec2(2.0, rect.height())), 0.0, Color32::from_gray(0xe8));
    let lab = fmt(app.s.time.current);
    let font = egui::FontId::proportional(11.0);
    let tw = ui.fonts_mut(|f| f.layout_no_wrap(lab.clone(), font.clone(), Color32::BLACK).size().x) + 6.0;
    let lx = (cx + 2.0).min(rect.max.x - tw);
    p.rect_filled(Rect::from_min_size(pos2(lx, rect.min.y + 1.0), vec2(tw, 13.0)), 0.0, Color32::from_gray(0xd9));
    p.text(pos2(lx + 3.0, rect.min.y + 1.5), egui::Align2::LEFT_TOP, lab, font, Color32::from_gray(0x11));

    // ---- interaction
    let (mods, ptr) = ui.input(|i| (i.modifiers, i.pointer.clone()));
    let at = resp.interact_pointer_pos().map(|q| t_of(q.x));
    let primary = ptr.button_down(egui::PointerButton::Primary) || resp.clicked_by(egui::PointerButton::Primary);
    let middle = ptr.button_down(egui::PointerButton::Middle) || resp.clicked_by(egui::PointerButton::Middle);
    let state_id = egui::Id::new("ts-drag");
    if resp.drag_started() || resp.clicked() {
        if let Some(f) = at {
            let mode = if primary && mods.shift {
                app.range_sel = Some((f, f));
                1
            } else if middle && app.range_sel.is_some() {
                2
            } else if middle {
                3
            } else if primary {
                app.range_sel = None;
                if app.playing {
                    app.action("stopPlayback");
                }
                0
            } else {
                -1
            };
            ui.data_mut(|d| d.insert_temp(state_id, (mode, f, app.range_sel)));
        }
    }
    let state: Option<(i32, f64, Option<(f64, f64)>)> = ui.data(|d| d.get_temp(state_id));
    if let (Some((mode, f0, sel0)), Some(f)) = (state, at) {
        match mode {
            0 if f != app.s.time.current => app.s.set_time(f),
            1 => app.range_sel = app.range_sel.map(|(a, _)| (a, f)),
            2 => {
                if let Some((a, b)) = sel0 {
                    let d = f - f0;
                    app.range_sel = Some((a.min(b) + d, a.max(b) + d));
                }
            }
            // MMB: change time without updating the scene, to copy a pose
            3 => app.s.time.current = f,
            _ => {}
        }
    }
    if resp.drag_stopped() || resp.clicked() {
        if let Some((mode, f0, sel0)) = state {
            match mode {
                1 => {
                    if app.range_sel.is_some_and(|(a, b)| a == b) {
                        app.range_sel = None;
                    }
                }
                2 => {
                    if let (Some((a, b)), Some(f)) = (sel0, at) {
                        let d = f - f0;
                        if d != 0.0 {
                            move_keys(app, a.min(b), a.max(b), d);
                        }
                    }
                }
                3 => app.help = "Time changed without updating the scene. Set a key to copy the pose.".into(),
                _ => {}
            }
        }
        ui.data_mut(|d| d.remove::<(i32, f64, Option<(f64, f64)>)>(state_id));
    }
    resp.context_menu(|ui| time_menu(app, ui));
}

fn key_range(app: &IncaApp) -> (f64, f64) {
    match app.range_sel {
        Some((a, b)) => (a.min(b), a.max(b)),
        None => (app.s.time.current, app.s.time.current),
    }
}

fn move_keys(app: &mut IncaApp, lo: f64, hi: f64, d: f64) {
    app.s.checkpoint("move keys");
    let sel = app.s.sel.clone();
    for c in app.s.anim.curves.values_mut().filter(|c| sel.contains(&c.node)) {
        let (moving, rest): (Vec<_>, Vec<_>) = c.keys.drain(..).partition(|k| k.t >= lo - 1e-4 && k.t <= hi + 1e-4);
        c.keys = rest.into_iter().filter(|k| !(k.t >= lo + d - 1e-4 && k.t <= hi + d + 1e-4)).collect();
        c.keys.extend(moving.into_iter().map(|mut k| {
            k.t += d;
            k
        }));
        c.sort();
    }
    app.s.apply_anim();
    app.s.rev += 1;
}

fn delete_keys(app: &mut IncaApp, lo: f64, hi: f64) {
    app.s.checkpoint("delete keys");
    let sel = app.s.sel.clone();
    for c in app.s.anim.curves.values_mut().filter(|c| sel.contains(&c.node)) {
        c.keys.retain(|k| !(k.t >= lo - 1e-4 && k.t <= hi + 1e-4));
    }
    app.s.anim.curves.retain(|_, c| !c.keys.is_empty());
    app.s.apply_anim();
    app.s.rev += 1;
}

pub fn copy_keys(app: &mut IncaApp, lo: f64, hi: f64) {
    app.key_clip = app
        .s
        .anim
        .curves
        .values()
        .filter(|c| app.s.sel.contains(&c.node))
        .map(|c| {
            let keys = c.keys.iter().filter(|k| k.t >= lo - 1e-4 && k.t <= hi + 1e-4).map(|k| inca_core::anim::Key { t: k.t - lo, ..k.clone() }).collect::<Vec<_>>();
            (c.node.clone(), c.attr.clone(), keys)
        })
        .filter(|c| !c.2.is_empty())
        .collect();
}

/// Paste copied keys at the current time (onto the same node, or by attribute onto the selection).
pub fn paste_keys(app: &mut IncaApp) {
    if app.key_clip.is_empty() {
        return;
    }
    app.s.checkpoint("paste keys");
    let t = app.s.time.current;
    let sel = app.s.sel.clone();
    let clip = app.key_clip.clone();
    let same_nodes = sel.iter().all(|n| clip.iter().any(|c| &c.0 == n));
    for n in &sel {
        for (cn, attr, keys) in &clip {
            if same_nodes && cn != n {
                continue;
            }
            if let Some(c) = app.s.anim.curve_mut(n, attr, true) {
                for k in keys {
                    c.keys.retain(|x| (x.t - (k.t + t)).abs() > 1e-4);
                    c.keys.push(inca_core::anim::Key { t: k.t + t, ..k.clone() });
                }
                c.sort();
            }
        }
    }
    app.s.apply_anim();
    app.s.rev += 1;
}

/// RMB on the Time Slider (the JS ctxMenu).
fn time_menu(app: &mut IncaApp, ui: &mut egui::Ui) {
    let (lo, hi) = key_range(app);
    let act = |app: &mut IncaApp, ui: &mut egui::Ui, l: &str, a: &str| {
        if ui.button(l).clicked() {
            app.action(a);
            ui.close();
        }
    };
    act(app, ui, "Undo", "undo");
    act(app, ui, "Redo", "redo");
    ui.separator();
    if ui.button("Cut").clicked() {
        copy_keys(app, lo, hi);
        delete_keys(app, lo, hi);
        ui.close();
    }
    if ui.button("Copy").clicked() {
        copy_keys(app, lo, hi);
        ui.close();
    }
    ui.menu_button("Paste", |ui| {
        if ui.add_enabled(!app.key_clip.is_empty(), egui::Button::new("Paste")).clicked() {
            paste_keys(app);
            ui.close();
        }
    });
    if ui.button("Delete").clicked() {
        delete_keys(app, lo, hi);
        ui.close();
    }
    ui.separator();
    for (l, a) in [("Set Key", "setKey"), ("Key Translate", "keyTranslate"), ("Key Rotate", "keyRotate"), ("Key Scale", "keyScale")] {
        act(app, ui, l, a);
    }
    ui.separator();
    ui.menu_button("Tangents", |ui| {
        for t in inca_core::anim::TANGENTS.iter().filter(|t| **t != "fixed") {
            let mut label = t.to_string();
            label[..1].make_ascii_uppercase();
            if ui.button(label).clicked() {
                app.s.checkpoint("tangents");
                let sel = app.s.sel.clone();
                for c in app.s.anim.curves.values_mut().filter(|c| sel.contains(&c.node)) {
                    for k in c.keys.iter_mut().filter(|k| k.t >= lo - 1e-4 && k.t <= hi + 1e-4) {
                        k.it = t.to_string();
                        k.ot = t.to_string();
                    }
                }
                app.s.apply_anim();
                app.s.rev += 1;
                ui.close();
            }
        }
    });
    ui.separator();
    ui.menu_button("Playback Looping", |ui| {
        for (l, v) in [("Once", "once"), ("Oscillate", "oscillate"), ("Continuous", "continuous")] {
            if ui.radio(app.s.time.r#loop == v, l).clicked() {
                app.s.time.r#loop = v.into();
                ui.close();
            }
        }
    });
    ui.menu_button("Set Range To", |ui| {
        if ui.add_enabled(app.range_sel.is_some(), egui::Button::new("Selected")).clicked() {
            app.s.time.start = lo;
            app.s.time.end = hi.max(lo + 1.0);
            ui.close();
        }
        ui.add_enabled(false, egui::Button::new("Sound Length"));
        if ui.button("Keys of selected").clicked() {
            let ks = app.s.anim.key_times(&app.s.sel);
            if let (Some(a), Some(b)) = (ks.first(), ks.last()) {
                app.s.time.start = *a;
                app.s.time.end = b.max(a + 1.0);
            }
            ui.close();
        }
    });
    ui.add_enabled(false, egui::Button::new("Playblast... (planned)"));
}

pub fn range_slider(app: &mut IncaApp, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        let t = &mut app.s.time;
        let fld = |ui: &mut egui::Ui, v: &mut f64, tip: &str| ui.add(egui::DragValue::new(v).speed(0.5).max_decimals(2)).on_hover_text(tip).changed();
        if fld(ui, &mut t.anim_start, "Animation start time") && t.start < t.anim_start {
            t.start = t.anim_start;
        }
        if fld(ui, &mut t.start, "Playback start time") {
            t.start = t.start.min(t.end);
            t.anim_start = t.anim_start.min(t.start);
        }
        let w = (ui.available_width() - 520.0).max(80.0);
        let (bar, _) = ui.allocate_exact_size(vec2(w, 20.0), Sense::hover());
        range_bar(app, ui, bar);
        let t = &mut app.s.time;
        if fld(ui, &mut t.end, "Playback end time") {
            t.end = t.end.max(t.start);
            t.anim_end = t.anim_end.max(t.end);
        }
        if fld(ui, &mut t.anim_end, "Animation end time") && t.end > t.anim_end {
            t.end = t.anim_end;
        }
        egui::ComboBox::from_id_salt("fps").width(118.0).selected_text(fps_label(t.fps)).show_ui(ui, |ui| {
            for (l, f) in FPS {
                ui.selectable_value(&mut t.fps, *f, *l);
            }
        });
        let lp = t.r#loop.clone();
        if icons::button(ui, "loop", &format!("Looping: {lp} (continuous / once / oscillate)"), false, 20.0).clicked() {
            t.r#loop = match lp.as_str() {
                "continuous" => "once",
                "once" => "oscillate",
                _ => "continuous",
            }
            .into();
            app.help = format!("Playback looping: {}", app.s.time.r#loop);
        }
        egui::ComboBox::from_id_salt("charset").width(112.0).selected_text("No Character Set").show_ui(ui, |ui| {
            let _ = ui.selectable_label(true, "No Character Set");
        });
        egui::ComboBox::from_id_salt("animlayer").width(100.0).selected_text("No Anim Layer").show_ui(ui, |ui| {
            let _ = ui.selectable_label(true, "No Anim Layer");
        });
        let ak = app.s.time.auto_key;
        let r = icons::button_ex(ui, "autoKey", "Toggle auto keyframe", ak, 20.0, 16.0, true);
        if ak {
            ui.painter().rect_stroke(r.rect, 3.0, Stroke::new(1.0, Color32::from_rgb(0xff, 0x55, 0x55)), egui::StrokeKind::Inside);
        }
        if r.clicked() {
            app.action("toggleAutoKey");
        }
        if icons::button(ui, "speaker", "Sound (no audio in scene)", false, 20.0).clicked() {
            app.help = "Import audio is not supported yet".into();
        }
        if icons::button(ui, "playPrefs", "Animation preferences", false, 20.0).clicked() {
            app.action("preferences");
        }
    });
}

/// The range bar: drag the handle to slide the playback range, its grips to resize, double-click for the full range.
fn range_bar(app: &mut IncaApp, ui: &mut egui::Ui, bar: Rect) {
    let t = &app.s.time;
    let span = (t.anim_end - t.anim_start).max(1.0);
    let xa = bar.min.x + ((t.start - t.anim_start) / span) as f32 * bar.width();
    let xb = bar.min.x + ((t.end - t.anim_start) / span) as f32 * bar.width();
    let hnd = Rect::from_min_max(pos2(xa, bar.min.y + 2.0), pos2(xb.max(xa + 24.0), bar.max.y - 2.0));
    let p = ui.painter_at(bar);
    p.rect_filled(bar, 2.0, theme::BG_DARK);
    p.rect_filled(hnd, 3.0, Color32::from_gray(0x8a));
    let grip = |r: Rect| r.width() > 0.0;
    let gl = Rect::from_min_size(hnd.min, vec2(7.0, hnd.height()));
    let gr = Rect::from_min_size(pos2(hnd.max.x - 7.0, hnd.min.y), vec2(7.0, hnd.height()));
    for g in [gl, gr] {
        if grip(g) {
            p.rect_filled(g, 3.0, Color32::from_gray(0xaa));
        }
    }
    let small = egui::FontId::proportional(10.5);
    p.text(pos2(gl.max.x + 2.0, hnd.center().y), egui::Align2::LEFT_CENTER, fmt(t.start), small.clone(), Color32::from_gray(0x1a));
    p.text(pos2(gr.min.x - 2.0, hnd.center().y), egui::Align2::RIGHT_CENTER, fmt(t.end), small, Color32::from_gray(0x1a));
    let id = egui::Id::new("rs-bar");
    let resp = ui.interact(hnd, id, Sense::click_and_drag());
    let mode_id = id.with("mode");
    if resp.drag_started() {
        let x = resp.interact_pointer_pos().map(|q| q.x).unwrap_or(hnd.center().x);
        let mode = if gl.contains(pos2(x, gl.center().y)) { 1 } else if gr.contains(pos2(x, gr.center().y)) { 2 } else { 0 };
        ui.data_mut(|d| d.insert_temp(mode_id, (mode, x, t.start, t.end)));
    }
    if let Some((mode, x0, s0, e0)) = ui.data(|d| d.get_temp::<(i32, f32, f64, f64)>(mode_id)) {
        if let Some(x) = resp.interact_pointer_pos() {
            let d = (((x.x - x0) / bar.width()) as f64 * span).round();
            let t = &mut app.s.time;
            match mode {
                1 => t.start = (s0 + d).clamp(t.anim_start, e0 - 1.0),
                2 => t.end = (e0 + d).clamp(s0 + 1.0, t.anim_end),
                _ => {
                    let (mut ns, mut ne) = (s0 + d, e0 + d);
                    if ns < t.anim_start {
                        ne += t.anim_start - ns;
                        ns = t.anim_start;
                    }
                    if ne > t.anim_end {
                        ns -= ne - t.anim_end;
                        ne = t.anim_end;
                    }
                    t.start = ns;
                    t.end = ne;
                }
            }
        }
        if resp.drag_stopped() {
            ui.data_mut(|d| d.remove::<(i32, f32, f64, f64)>(mode_id));
        }
    }
    if resp.double_clicked() {
        let t = &mut app.s.time;
        if t.start == t.anim_start && t.end == t.anim_end {
            if let Some((a, b)) = app.prev_range {
                t.start = a;
                t.end = b;
            }
        } else {
            app.prev_range = Some((t.start, t.end));
            t.start = t.anim_start;
            t.end = t.anim_end;
        }
    }
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
