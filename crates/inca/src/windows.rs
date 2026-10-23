//! Floating editors: Script Editor, Hypershade, Graph Editor, Render View / Settings, dialogs.

use crate::app::IncaApp;
use crate::theme;
use egui::{Color32, RichText};
use inca_core::materials::{MAT_TYPES, TEX_TYPES};
use inca_core::render::{RenderScene, RenderSettings};
use inca_core::scene::{MsgKind, NodeRef, Scene};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct RenderJob {
    cancel: Arc<AtomicBool>,
    latest: Arc<Mutex<Option<(Vec<u8>, u32)>>>,
    pub size: [u32; 2],
    pub passes: u32,
    pub total: u32,
    pub started: std::time::Instant,
    pub tris: usize,
    pub camera: String,
}

#[derive(Default)]
pub struct WinState {
    /// dope sheet key drag: (node, time grabbed, current offset)
    pub ds_drag: Option<(Option<String>, f64, f64)>,
    pub job: Option<RenderJob>,
    pub image: Option<egui::TextureHandle>,
    pub pixels: Option<(Vec<u8>, [u32; 2])>,
    pub hs_selected: Option<String>,
    pub ge_curves: std::collections::HashSet<String>,
    pub ge_key: Option<(String, usize)>,
    pub ge_drag: bool,
}

impl WinState {
    pub fn start_render(&mut self, s: &Scene, camera: &str, _frame: u32) {
        if let Some(j) = &self.job {
            j.cancel.store(true, Ordering::Relaxed);
        }
        let settings = RenderSettings::from_scene(s);
        let size = [settings.width.max(1), settings.height.max(1)];
        let total = settings.samples.max(1);
        let rs = Arc::new(RenderScene::build(s, camera, settings));
        let tris = rs.triangle_count();
        let cancel = Arc::new(AtomicBool::new(false));
        let latest = Arc::new(Mutex::new(None));
        let (c2, l2) = (cancel.clone(), latest.clone());
        std::thread::spawn(move || {
            let mut acc = vec![0f32; (size[0] * size[1] * 3) as usize];
            for pass in 0..total {
                if c2.load(Ordering::Relaxed) {
                    return;
                }
                rs.render_pass(&mut acc, pass);
                let px = rs.resolve(&acc, pass + 1);
                *l2.lock().unwrap() = Some((px, pass + 1));
            }
        });
        self.job = Some(RenderJob { cancel, latest, size, passes: 0, total, started: std::time::Instant::now(), tris, camera: camera.into() });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let Some(j) = &mut self.job else { return };
        let got = j.latest.lock().unwrap().take();
        if let Some((px, n)) = got {
            j.passes = n;
            let img = egui::ColorImage::from_rgba_unmultiplied([j.size[0] as usize, j.size[1] as usize], &px);
            match &mut self.image {
                Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                None => self.image = Some(ctx.load_texture("render", img, egui::TextureOptions::LINEAR)),
            }
            self.pixels = Some((px, j.size));
        }
        if j.passes < j.total {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
}

pub fn show(app: &mut IncaApp, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    app.windows_state.poll(ctx);
    if app.playblast.running.is_some() {
        // floating windows would end up in the captured frames
        return;
    }
    crate::rig_tools::sdk_window(app, ctx);
    crate::anim_extras::window(app, ctx);
    let mut open = app.win.script_editor;
    egui::Window::new("Script Editor").open(&mut open).default_size([640.0, 460.0]).show(ctx, |ui| script_editor(app, ui));
    app.win.script_editor = open;
    let mut open = app.win.hypershade;
    egui::Window::new("Hypershade").open(&mut open).default_size([620.0, 480.0]).show(ctx, |ui| hypershade(app, ui));
    app.win.hypershade = open;
    let mut open = app.win.graph_editor;
    egui::Window::new("Graph Editor").open(&mut open).default_size([760.0, 420.0]).show(ctx, |ui| graph_editor(app, ui));
    app.win.graph_editor = open;
    let mut open = app.win.dope_sheet;
    egui::Window::new("Dope Sheet").open(&mut open).default_size([760.0, 260.0]).show(ctx, |ui| dope_sheet(app, ui));
    app.win.dope_sheet = open;
    let mut open = app.win.render_view;
    egui::Window::new("Render View").open(&mut open).default_size([700.0, 560.0]).show(ctx, |ui| render_view(app, ui));
    app.win.render_view = open;
    let mut open = app.win.render_settings;
    egui::Window::new("Render Settings").open(&mut open).default_size([340.0, 260.0]).show(ctx, |ui| render_settings(app, ui));
    app.win.render_settings = open;
    let mut open = app.win.prefs;
    egui::Window::new("Preferences").open(&mut open).show(ctx, |ui| prefs(app, ui));
    app.win.prefs = open;
    let mut open = app.win.hotkeys;
    egui::Window::new("Hotkeys").open(&mut open).default_size([360.0, 500.0]).show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("hk").striped(true).show(ui, |ui| {
                for (id, l, hk) in crate::actions::ACTIONS {
                    if !hk.is_empty() {
                        ui.label(*l);
                        ui.label(RichText::new(*hk).monospace());
                        ui.label(RichText::new(*id).color(theme::TEXT_DIM));
                        ui.end_row();
                    }
                }
            });
        });
    });
    app.win.hotkeys = open;
    let mut open = app.win.command_list;
    egui::Window::new("MEL Command Reference").open(&mut open).default_size([380.0, 480.0]).show(ctx, |ui| {
        ui.label("Commands available to MEL scripts, the command line and the Script Editor:");
        egui::ScrollArea::vertical().show(ui, |ui| {
            for g in inca_core::primitives::GEN_INFO {
                let flags: Vec<String> = g.short.iter().map(|(s, l)| format!("-{s} ({l})")).collect();
                ui.label(RichText::new(g.ty).monospace().strong()).on_hover_text(flags.join("\n"));
            }
            for c in inca_core::commands::CORE_COMMANDS {
                ui.label(RichText::new(*c).monospace());
            }
            ui.separator();
            ui.label("Language: int/float/string/vector variables and arrays, procs, if/else, for, for-in, while, do-while, switch, backtick command capture, print, size, sin/cos/..., tokenize, substring, gmatch ...");
        });
    });
    app.win.command_list = open;
    let mut open = app.win.about;
    egui::Window::new("About Inca").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
        ui.heading("Inca");
        ui.label(format!("version {} (Rust)", env!("CARGO_PKG_VERSION")));
        ui.label("A free, open-source 3D modeling, animation and rendering application.");
        ui.label("Built with egui/eframe, wgpu, glam, transform-gizmo, boolmesh, tobj, gltf, stl_io, rayon and rhai.");
        ui.hyperlink("https://incacad.com");
    });
    app.win.about = open;
    // rename dialog
    if let Some(mut name) = app.win.rename.take() {
        let mut keep = true;
        egui::Window::new("Rename").collapsible(false).resizable(false).show(ctx, |ui| {
            let r = ui.text_edit_singleline(&mut name);
            r.request_focus();
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    let _ = app.run_mel("rename", &format!("rename \"{}\";", name.trim()));
                    keep = false;
                }
                if ui.button("Cancel").clicked() {
                    keep = false;
                }
            });
        });
        if keep {
            app.win.rename = Some(name);
        }
    }
}

// ------------------------------------------------------------------ script editor

fn script_editor(app: &mut IncaApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui.selectable_label(!app.script_lang_rhai, "MEL").clicked() {
            app.script_lang_rhai = false;
        }
        if ui.selectable_label(app.script_lang_rhai, "Rhai").on_hover_text("Rhai scripting: mel(\"polyCube\"), cmd(\"move\", [1, 2, 3]), getAttr(\"pCube1.tx\"), ls()").clicked() {
            app.script_lang_rhai = true;
        }
        ui.separator();
        if ui.button("▶ Execute").on_hover_text("Ctrl+Enter").clicked() {
            run_script(app);
        }
        if ui.button("Clear History").clicked() {
            app.script_log.clear();
        }
        if !app.script_lang_rhai && ui.button("Save to Shelf").on_hover_text("Add the script as a button on the Custom shelf").clicked() {
            let src = app.script_text.trim().to_string();
            if !src.is_empty() {
                let l: String = src.lines().find(|l| !l.trim_start().starts_with("//")).unwrap_or("script").split(|c: char| !c.is_alphanumeric()).find(|w| !w.is_empty()).unwrap_or("script").chars().take(8).collect();
                app.custom_shelf.push((l, src));
            }
        }
        if ui.button("Load...").clicked() {
            if let Some(p) = rfd::FileDialog::new().add_filter("Scripts", &["mel", "rhai", "txt"]).pick_file() {
                if let Ok(t) = std::fs::read_to_string(&p) {
                    app.script_lang_rhai = p.extension().is_some_and(|e| e == "rhai");
                    app.script_text = t;
                }
            }
        }
        if ui.button("Save...").clicked() {
            if let Some(p) = rfd::FileDialog::new().set_file_name(if app.script_lang_rhai { "script.rhai" } else { "script.mel" }).save_file() {
                let _ = std::fs::write(p, &app.script_text);
            }
        }
    });
    let h = ui.available_height();
    egui::Frame::NONE.fill(theme::BG_DARK).show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("selog").max_height(h * 0.55).stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
            for (k, l) in &app.script_log {
                let c = match k {
                    MsgKind::Error => Color32::from_rgb(240, 110, 110),
                    MsgKind::Warning => Color32::from_rgb(240, 200, 90),
                    MsgKind::Result => Color32::from_rgb(160, 200, 255),
                    MsgKind::Info => Color32::from_gray(230),
                    MsgKind::Echo => theme::TEXT,
                };
                ui.label(RichText::new(l).monospace().color(c));
            }
        });
    });
    ui.separator();
    let r = egui::ScrollArea::vertical()
        .id_salt("seinput")
        .show(ui, |ui| ui.add(egui::TextEdit::multiline(&mut app.script_text).code_editor().desired_rows(10).desired_width(f32::INFINITY)))
        .inner;
    if r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && (i.modifiers.ctrl || i.modifiers.command)) {
        run_script(app);
    }
}

fn run_script(app: &mut IncaApp) {
    let src = app.script_text.clone();
    if app.script_lang_rhai {
        app.s.checkpoint("rhai script");
        match crate::scripting::run_rhai(app, &src) {
            Ok(r) => {
                if !r.is_empty() {
                    app.script_log.push((MsgKind::Result, format!("// Result: {r}")));
                }
            }
            Err(e) => app.script_log.push((MsgKind::Error, format!("// Error: {e}"))),
        }
    } else {
        let _ = app.run_mel("script", &src);
    }
}

// ------------------------------------------------------------------ hypershade

pub fn material_editor(app: &mut IncaApp, ui: &mut egui::Ui, mid: &str) {
    let Some(m) = app.s.mats.get(mid).cloned() else { return };
    ui.horizontal(|ui| {
        let p = m.pbr();
        let (r, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
        ui.painter().circle_filled(r.center(), 13.0, Color32::from(egui::Rgba::from_rgb(p.base[0], p.base[1], p.base[2])));
        let mut name = m.name.clone();
        let resp = ui.text_edit_singleline(&mut name);
        if resp.lost_focus() && name != m.name && !name.trim().is_empty() {
            app.s.checkpoint("rename material");
            let old = m.name.clone();
            app.s.release_name(&old);
            let nn = app.s.unique_name(name.trim());
            app.s.mats.get_mut(mid).unwrap().name = nn;
            app.s.touch();
        }
        ui.label(RichText::new(inca_core::materials::mat_type(&m.ty).label).color(theme::TEXT_DIM));
    });
    theme::section_header(ui, "Common Material Attributes");
    let keys: Vec<String> = m.attrs.keys().cloned().collect();
    crate::panels::attr_map_editor(app, ui, &NodeRef::Material(mid.to_string()), &keys);
    theme::section_header(ui, "Texture Maps");
    for slot in inca_core::materials::mat_type(&m.ty).mappable {
        ui.horizontal(|ui| {
            ui.label(inca_core::ops::nice_name(slot));
            let cur = m.maps.get(*slot).and_then(|v| v.as_str()).map(String::from);
            let cur_name = cur.as_ref().and_then(|t| app.s.texs.get(t)).map(|t| t.name.clone()).unwrap_or("None".into());
            egui::ComboBox::from_id_salt(("map", mid, *slot)).selected_text(cur_name).show_ui(ui, |ui| {
                if ui.selectable_label(cur.is_none(), "None").clicked() {
                    app.s.checkpoint("disconnect texture");
                    app.s.mats.get_mut(mid).unwrap().maps.remove(*slot);
                    app.s.touch();
                }
                for t in TEX_TYPES {
                    if ui.button(format!("New {}", t.label)).clicked() {
                        app.s.checkpoint("connect texture");
                        let tid = app.s.create_texture(t.ty, None);
                        app.s.mats.get_mut(mid).unwrap().maps.insert(slot.to_string(), json!(tid));
                        app.s.touch();
                    }
                }
                let texs: Vec<(String, String)> = app.s.texs.values().map(|t| (t.id.clone(), t.name.clone())).collect();
                for (tid, tn) in texs {
                    if ui.selectable_label(cur.as_deref() == Some(tid.as_str()), &tn).clicked() {
                        app.s.checkpoint("connect texture");
                        app.s.mats.get_mut(mid).unwrap().maps.insert(slot.to_string(), json!(tid));
                        app.s.touch();
                    }
                }
            });
        });
        if let Some(tid) = m.maps.get(*slot).and_then(|v| v.as_str()) {
            if let Some(t) = app.s.texs.get(tid).cloned() {
                ui.indent(("tex", tid), |ui| {
                    ui.label(RichText::new(format!("{} ({})", t.name, t.ty)).color(theme::TEXT_DIM));
                    let keys: Vec<String> = t.attrs.keys().filter(|k| *k != "colors").cloned().collect();
                    crate::panels::attr_map_editor(app, ui, &NodeRef::Texture(tid.to_string()), &keys);
                    if t.ty == "file" && ui.button("Browse image...").clicked() {
                        if let Some(p) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "hdr"]).pick_file() {
                            app.s.set_attr(&NodeRef::Texture(tid.to_string()), "fileTextureName", json!(p.to_string_lossy()));
                        }
                    }
                });
            }
        }
    }
}

fn hypershade(app: &mut IncaApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label("Create:");
        for t in MAT_TYPES {
            if ui.button(t.label).clicked() {
                let _ = app.run_mel("create material", &format!("shadingNode -asShader {};", t.ty));
                app.windows_state.hs_selected = app.s.mats.keys().last().cloned();
            }
        }
    });
    ui.separator();
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        ui.label(RichText::new("Materials").strong());
        egui::ScrollArea::vertical().id_salt("hsmats").show(ui, |ui| {
            let mats: Vec<(String, String, [f32; 3])> = app.s.mats.values().map(|m| (m.id.clone(), m.name.clone(), m.pbr().base)).collect();
            egui::Grid::new("hsgrid").num_columns(3).show(ui, |ui| {
                for (i, (id, name, c)) in mats.iter().enumerate() {
                    let sel = app.windows_state.hs_selected.as_deref() == Some(id.as_str());
                    ui.vertical(|ui| {
                        let (r, resp) = ui.allocate_exact_size(egui::vec2(56.0, 56.0), egui::Sense::click());
                        ui.painter().rect_filled(r, 2.0, if sel { theme::HI } else { theme::BG_DARK });
                        ui.painter().circle_filled(r.center(), 22.0, Color32::from(egui::Rgba::from_rgb(c[0], c[1], c[2])));
                        ui.painter().circle_filled(r.center() + egui::vec2(-7.0, -7.0), 5.0, Color32::from_white_alpha(60));
                        ui.label(RichText::new(name).size(10.5));
                        if resp.clicked() {
                            app.windows_state.hs_selected = Some(id.clone());
                        }
                        if resp.double_clicked() {
                            let _ = app.run_mel("assign material", &format!("hyperShade -assign {name};"));
                        }
                        resp.on_hover_text("Click to edit, double-click to assign to the selection");
                    });
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
        });
        let ui = &mut cols[1];
        match app.windows_state.hs_selected.clone().filter(|m| app.s.mats.contains_key(m)) {
            Some(mid) => {
                ui.horizontal(|ui| {
                    if ui.button("Assign to Selection").clicked() {
                        let n = app.s.mats[&mid].name.clone();
                        let _ = app.run_mel("assign material", &format!("hyperShade -assign {n};"));
                    }
                    if mid != app.s.default_mat && ui.button("Delete").clicked() {
                        app.s.checkpoint("delete material");
                        app.s.delete_material(&mid);
                        app.windows_state.hs_selected = None;
                    }
                });
                egui::ScrollArea::vertical().id_salt("hsattr").show(ui, |ui| material_editor(app, ui, &mid));
            }
            None => {
                ui.label(RichText::new("Select a material to edit it").color(theme::TEXT_DIM));
            }
        }
    });
}

// ------------------------------------------------------------------ graph editor

fn graph_editor(app: &mut IncaApp, ui: &mut egui::Ui) {
    use egui_plot::{Line, Plot, PlotPoints, Points};
    let curves: Vec<(String, String, String)> = app.s.anim.curves.iter().filter(|(_, c)| app.s.sel.contains(&c.node) && !c.keys.is_empty()).map(|(k, c)| (k.clone(), app.s.name_of(&c.node), c.attr.clone())).collect();
    ui.horizontal(|ui| {
        if let Some((ck, ki)) = app.windows_state.ge_key.clone() {
            if let Some(c) = app.s.anim.curves.get(&ck).cloned() {
                if let Some(k) = c.keys.get(ki).cloned() {
                    ui.label(format!("Key  t={}  v={}", inca_core::math::fmt_num(k.t), inca_core::math::fmt_num(k.v)));
                    let mut tan = k.ot.clone();
                    egui::ComboBox::from_id_salt("getan").selected_text(&tan).show_ui(ui, |ui| {
                        for t in inca_core::anim::TANGENTS {
                            ui.selectable_value(&mut tan, t.to_string(), *t);
                        }
                    });
                    if tan != k.ot {
                        app.s.checkpoint("key tangents");
                        let key = &mut app.s.anim.curves.get_mut(&ck).unwrap().keys[ki];
                        key.it = tan.clone();
                        key.ot = tan;
                        app.s.touch();
                    }
                }
                let (mut pre, mut post) = (c.pre.clone(), c.post.clone());
                ui.separator();
                egui::ComboBox::from_id_salt("gepre").selected_text(format!("pre: {pre}")).show_ui(ui, |ui| {
                    for t in inca_core::anim::INFINITY {
                        ui.selectable_value(&mut pre, t.to_string(), *t);
                    }
                });
                egui::ComboBox::from_id_salt("gepost").selected_text(format!("post: {post}")).show_ui(ui, |ui| {
                    for t in inca_core::anim::INFINITY {
                        ui.selectable_value(&mut post, t.to_string(), *t);
                    }
                });
                let c = app.s.anim.curves.get_mut(&ck).unwrap();
                if pre != c.pre || post != c.post {
                    c.pre = pre;
                    c.post = post;
                    app.s.touch();
                }
            }
        } else {
            ui.label(RichText::new("Select an animated object; click a key to edit it, drag to move it").color(theme::TEXT_DIM));
        }
    });
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(160.0);
            for (k, n, a) in &curves {
                let mut on = !app.windows_state.ge_curves.contains(k);
                if ui.checkbox(&mut on, format!("{n}.{a}")).changed() {
                    if on {
                        app.windows_state.ge_curves.remove(k);
                    } else {
                        app.windows_state.ge_curves.insert(k.clone());
                    }
                }
            }
        });
        let colors = [Color32::from_rgb(230, 80, 80), Color32::from_rgb(90, 210, 90), Color32::from_rgb(90, 140, 255), Color32::from_rgb(230, 200, 80), Color32::from_rgb(200, 120, 230)];
        let t0 = app.s.time.start;
        let t1 = app.s.time.end;
        let cur = app.s.time.current;
        let shown: Vec<String> = curves.iter().filter(|c| !app.windows_state.ge_curves.contains(&c.0)).map(|c| c.0.clone()).collect();
        let selected_key = app.windows_state.ge_key.clone();
        let resp = Plot::new("graph").allow_drag(false).allow_double_click_reset(true).show(ui, |pu| {
            for (i, k) in shown.iter().enumerate() {
                let c = &app.s.anim.curves[k];
                let col = match c.attr.chars().last() {
                    Some('X') => colors[0],
                    Some('Y') => colors[1],
                    Some('Z') => colors[2],
                    _ => colors[3 + i % 2],
                };
                let (a, b) = (c.keys[0].t.min(t0) - 10.0, c.keys.last().unwrap().t.max(t1) + 10.0);
                let pts: PlotPoints = (0..=400).map(|s| {
                    let t = a + (b - a) * s as f64 / 400.0;
                    [t, c.eval(t)]
                }).collect();
                pu.line(Line::new(c.attr.clone(), pts).color(col).width(1.5));
                let kp: PlotPoints = c.keys.iter().map(|k| [k.t, k.v]).collect();
                pu.points(Points::new(format!("{} keys", c.attr), kp).radius(3.5).color(Color32::BLACK));
                if let Some((sk, si)) = &selected_key {
                    if sk == k {
                        if let Some(key) = c.keys.get(*si) {
                            pu.points(Points::new("selected", vec![[key.t, key.v]]).radius(5.0).color(Color32::from_rgb(255, 230, 0)));
                        }
                    }
                }
            }
            pu.vline(egui_plot::VLine::new("time", cur).color(Color32::from_rgb(200, 60, 60)));
            pu.pointer_coordinate()
        });
        let pointer = resp.inner;
        let r = resp.response;
        let tf = resp.transform;
        if r.drag_started() || r.clicked() {
            // pick the nearest key in screen space
            if let Some(pp) = r.interact_pointer_pos() {
                let mut best: Option<(f32, String, usize)> = None;
                for k in &shown {
                    for (i, key) in app.s.anim.curves[k].keys.iter().enumerate() {
                        let sp = tf.position_from_point(&egui_plot::PlotPoint::new(key.t, key.v));
                        let d = sp.distance(pp);
                        if d < 10.0 && best.as_ref().is_none_or(|b| d < b.0) {
                            best = Some((d, k.clone(), i));
                        }
                    }
                }
                app.windows_state.ge_key = best.map(|b| (b.1, b.2));
                if app.windows_state.ge_key.is_some() && r.drag_started() {
                    app.s.checkpoint("move key");
                    app.windows_state.ge_drag = true;
                }
            }
        }
        if r.dragged() && app.windows_state.ge_drag {
            if let (Some((ck, ki)), Some(p)) = (app.windows_state.ge_key.clone(), pointer) {
                if let Some(c) = app.s.anim.curves.get_mut(&ck) {
                    let n = c.keys.len();
                    let lo = if ki > 0 { c.keys[ki - 1].t + 1.0 } else { f64::MIN };
                    let hi = if ki + 1 < n { c.keys[ki + 1].t - 1.0 } else { f64::MAX };
                    c.keys[ki].t = p.x.round().clamp(lo, hi);
                    c.keys[ki].v = p.y;
                }
                app.s.apply_anim();
                app.s.touch();
            }
        }
        if r.drag_stopped() {
            app.windows_state.ge_drag = false;
        }
    });
}

// ------------------------------------------------------------------ dope sheet

fn dope_sheet(app: &mut IncaApp, ui: &mut egui::Ui) {
    use egui::{pos2, vec2, Rect, Sense, Stroke};
    let nodes: Vec<String> = app.s.sel.clone();
    let (t0, t1) = (app.s.time.start, app.s.time.end.max(app.s.time.start + 1.0));
    ui.label(RichText::new("Drag keys to retime them (the summary row moves every key). Delete removes the keys at the current time.").color(theme::TEXT_DIM));
    let rows: Vec<(Option<String>, String, Vec<f64>)> = std::iter::once((None, "Dope Sheet Summary".to_string(), app.s.anim.key_times(&nodes)))
        .chain(nodes.iter().map(|n| (Some(n.clone()), app.s.name_of(n), app.s.anim.key_times(std::slice::from_ref(n)))))
        .collect();
    let label_w = 170.0;
    let row_h = 20.0;
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, row_h * rows.len().max(1) as f32 + 18.0), Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, theme::BG_DARK);
    let track = Rect::from_min_max(pos2(rect.min.x + label_w, rect.min.y + 18.0), rect.max);
    let x_of = |t: f64| track.min.x + 6.0 + ((t - t0) / (t1 - t0)) as f32 * (track.width() - 12.0);
    let t_of = |x: f32| t0 + ((x - track.min.x - 6.0) / (track.width() - 12.0)) as f64 * (t1 - t0);
    // frame ruler
    let span = t1 - t0;
    let step = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0].into_iter().find(|s| (span / s) * 36.0 < track.width() as f64).unwrap_or(100.0);
    let mut t = (t0 / step).ceil() * step;
    while t <= t1 {
        let x = x_of(t);
        p.line_segment([pos2(x, track.min.y), pos2(x, rect.max.y)], Stroke::new(1.0, theme::BG2));
        p.text(pos2(x + 2.0, rect.min.y + 2.0), egui::Align2::LEFT_TOP, inca_core::math::fmt_num_long(t), egui::FontId::proportional(10.0), theme::TEXT_DIM);
        t += step;
    }
    let drag = app.windows_state.ds_drag.clone();
    for (i, (node, name, times)) in rows.iter().enumerate() {
        let y = track.min.y + i as f32 * row_h;
        p.text(pos2(rect.min.x + 6.0, y + row_h / 2.0), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(12.0), if node.is_none() { theme::TEXT_DIM } else { theme::TEXT });
        p.line_segment([pos2(track.min.x, y + row_h), pos2(track.max.x, y + row_h)], Stroke::new(1.0, theme::BG2));
        for &k in times {
            let moving = drag.as_ref().is_some_and(|(n, grab, _)| (n.is_none() || n == node) && (k - grab).abs() < 1e-6);
            let off = if moving { drag.as_ref().map(|d| d.2).unwrap_or(0.0) } else { 0.0 };
            let x = x_of(k + off);
            let c = if moving { Color32::from_rgb(255, 230, 0) } else if node.is_none() { Color32::from_rgb(170, 170, 170) } else { theme::KEY };
            p.rect_filled(Rect::from_center_size(pos2(x, y + row_h / 2.0), vec2(6.0, 12.0)), 1.0, c);
        }
    }
    let cx = x_of(app.s.time.current);
    p.line_segment([pos2(cx, rect.min.y), pos2(cx, rect.max.y)], Stroke::new(1.5, Color32::from_rgb(200, 60, 60)));
    if resp.drag_started() {
        if let Some(pp) = resp.interact_pointer_pos() {
            let row = ((pp.y - track.min.y) / row_h).floor() as isize;
            if row >= 0 && (row as usize) < rows.len() {
                let (node, _, times) = &rows[row as usize];
                if let Some(&k) = times.iter().min_by(|a, b| (x_of(**a) - pp.x).abs().partial_cmp(&(x_of(**b) - pp.x).abs()).unwrap()) {
                    if (x_of(k) - pp.x).abs() < 8.0 {
                        app.windows_state.ds_drag = Some((node.clone(), k, 0.0));
                    }
                }
            }
        }
    }
    if let (Some(d), Some(pp)) = (app.windows_state.ds_drag.as_mut(), resp.interact_pointer_pos()) {
        if resp.dragged() {
            d.2 = (t_of(pp.x) - d.1).round();
        }
    }
    if resp.drag_stopped() {
        if let Some((node, grab, off)) = app.windows_state.ds_drag.take() {
            if off != 0.0 {
                app.s.checkpoint("move keys");
                let ids: Vec<String> = match &node {
                    Some(n) => vec![n.clone()],
                    None => nodes.clone(),
                };
                for c in app.s.anim.curves.values_mut().filter(|c| ids.contains(&c.node)) {
                    if c.keys.iter().any(|k| (k.t - (grab + off)).abs() < 1e-6) {
                        continue; // don't stack two keys on one frame
                    }
                    for k in c.keys.iter_mut() {
                        if (k.t - grab).abs() < 1e-6 {
                            k.t = grab + off;
                        }
                    }
                    c.sort();
                }
                app.s.touch();
                app.s.apply_anim();
            }
        }
    }
    if resp.clicked() {
        if let Some(pp) = resp.interact_pointer_pos() {
            if track.contains(pp) {
                let t = t_of(pp.x).round().clamp(t0, t1);
                app.s.set_time(t);
            }
        }
    }
    if ui.input(|i| i.key_pressed(egui::Key::Delete)) && resp.hovered() {
        let names: Vec<String> = nodes.iter().map(|n| app.s.name_of(n)).collect();
        if !names.is_empty() {
            let t = app.s.time.current;
            let _ = app.run_mel("cut keys", &format!("cutKey -t {t} {};", names.join(" ")));
        }
    }
}

// ------------------------------------------------------------------ rendering

fn render_view(app: &mut IncaApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let cam = app.vps[app.active_vp].camera.clone();
        if ui.button("🎬 Render").on_hover_text(format!("Render the current frame from {cam}")).clicked() {
            app.windows_state.start_render(&app.s, &cam, 0);
        }
        if let Some(j) = &app.windows_state.job {
            if j.passes < j.total && ui.button("Stop").clicked() {
                j.cancel.store(true, Ordering::Relaxed);
            }
        }
        if ui.button("Save Image...").clicked() {
            if let Some((px, size)) = &app.windows_state.pixels {
                if let Some(p) = rfd::FileDialog::new().add_filter("PNG", &["png"]).set_file_name("render.png").save_file() {
                    let _ = image::save_buffer(p, px, size[0], size[1], image::ColorType::Rgba8);
                }
            }
        }
        if let Some(j) = &app.windows_state.job {
            ui.label(RichText::new(format!("{} — {}/{} samples, {} triangles, {:.1}s", j.camera, j.passes, j.total, j.tris, j.started.elapsed().as_secs_f32())).color(theme::TEXT_DIM));
        }
    });
    ui.separator();
    if let Some(t) = &app.windows_state.image {
        let size = t.size_vec2();
        let avail = ui.available_size();
        let k = (avail.x / size.x).min(avail.y / size.y).min(1.0);
        ui.centered_and_justified(|ui| ui.image((t.id(), size * k)));
    } else {
        ui.label(RichText::new("Inca Path Tracer — press Render").color(theme::TEXT_DIM));
    }
}

fn render_settings(app: &mut IncaApp, ui: &mut egui::Ui) {
    let r = &mut app.s.render;
    let get = |k: &str, d: f64| r.get(k).and_then(|v| v.as_f64()).unwrap_or(d);
    let (mut w, mut h, mut spp, mut b, mut ex) = (get("width", 640.0), get("height", 480.0), get("samples", 64.0), get("bounces", 4.0), get("exposure", 0.0));
    egui::Grid::new("rs").num_columns(2).show(ui, |ui| {
        ui.label("Renderer");
        ui.label("Inca Path Tracer (CPU)");
        ui.end_row();
        ui.label("Presets");
        ui.horizontal(|ui| {
            for (l, pw, ph) in [("320x240", 320.0, 240.0), ("640x480", 640.0, 480.0), ("HD 720", 1280.0, 720.0), ("HD 1080", 1920.0, 1080.0)] {
                if ui.small_button(l).clicked() {
                    w = pw;
                    h = ph;
                }
            }
        });
        ui.end_row();
        ui.label("Width");
        ui.add(egui::DragValue::new(&mut w).range(16.0..=8192.0));
        ui.end_row();
        ui.label("Height");
        ui.add(egui::DragValue::new(&mut h).range(16.0..=8192.0));
        ui.end_row();
        ui.label("Samples");
        ui.add(egui::DragValue::new(&mut spp).range(1.0..=10000.0));
        ui.end_row();
        ui.label("Bounces");
        ui.add(egui::DragValue::new(&mut b).range(0.0..=32.0));
        ui.end_row();
        ui.label("Exposure");
        ui.add(egui::DragValue::new(&mut ex).speed(0.05).range(-10.0..=10.0));
        ui.end_row();
    });
    for (k, v) in [("width", w), ("height", h), ("samples", spp), ("bounces", b), ("exposure", ex)] {
        r.insert(k.into(), json!(v));
    }
}

fn prefs(app: &mut IncaApp, ui: &mut egui::Ui) {
    theme::section_header(ui, "Undo");
    ui.add(egui::Slider::new(&mut app.s.undo_limit, 10..=500).text("queue size"));
    theme::section_header(ui, "Manipulators");
    ui.add(egui::Slider::new(&mut app.manip_size, 0.2..=5.0).text("size"));
    theme::section_header(ui, "Modeling");
    ui.checkbox(&mut app.history_on, "Construction history on by default");
    theme::section_header(ui, "Display");
    egui::ComboBox::from_label("Background color").selected_text(crate::viewport::BG_MODES[app.bg_mode]).show_ui(ui, |ui| {
        for (i, m) in crate::viewport::BG_MODES.iter().enumerate() {
            ui.selectable_value(&mut app.bg_mode, i, *m);
        }
    });
    theme::section_header(ui, "Files");
    ui.horizontal(|ui| {
        ui.checkbox(&mut app.autosave.0, "Autosave every");
        ui.add(egui::DragValue::new(&mut app.autosave.1).speed(0.2).range(1.0..=240.0).suffix(" min"));
    });
    theme::section_header(ui, "Interface");
    for (k, l) in crate::workspace::ELEMENTS {
        let mut on = app.shown(k);
        if ui.checkbox(&mut on, *l).changed() {
            app.set_shown(k, on);
        }
    }
}
