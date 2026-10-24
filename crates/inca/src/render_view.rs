//! Render View: display exposure / gamma / alpha, kept images to compare,
//! IPR, Render Sequence and the two renderers (Inca Path Tracer, Inca Hardware).

use crate::app::IncaApp;
use crate::theme;
use egui::RichText;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

pub const RENDERERS: [(&str, &str); 2] = [("path", "Inca Path Tracer"), ("hardware", "Inca Hardware")];

/// A snapshot in the kept-image strip.
pub struct Kept {
    px: Vec<u8>,
    alpha: Option<Vec<u8>>,
    size: [u32; 2],
    label: String,
    info: String,
    thumb: Option<egui::TextureHandle>,
}

/// Render Sequence in progress: one frame at a time, saved when its render finishes.
pub struct Batch {
    frames: Vec<i64>,
    next: usize,
    dir: PathBuf,
    name: String,
    restore: f64,
    saved: usize,
    waiting: bool,
}

pub struct RvState {
    pub exposure: f32,
    pub gamma: f32,
    pub show_alpha: bool,
    pub alpha: Option<Vec<u8>>,
    pub kept: Vec<Kept>,
    /// which kept image is shown (None = the live render)
    pub viewing: Option<usize>,
    pub ipr: bool,
    ipr_sig: u64,
    pub batch: Option<Batch>,
    pub dirty: bool,
    pub status: String,
}

impl Default for RvState {
    fn default() -> Self {
        RvState { exposure: 0.0, gamma: 1.0, show_alpha: false, alpha: None, kept: vec![], viewing: None, ipr: false, ipr_sig: 0, batch: None, dirty: false, status: String::new() }
    }
}

fn renderer(app: &IncaApp) -> &str {
    app.s.render.get("renderer").and_then(|v| v.as_str()).unwrap_or("path")
}

fn render_camera(app: &IncaApp) -> String {
    app.s.render.get("camera").and_then(|v| v.as_str()).and_then(|c| app.s.find(c)).map(|c| app.s.name_of(&c)).unwrap_or_else(|| app.vps[app.active_vp].camera.clone())
}

/// Render the current frame with the selected renderer.
pub fn render(app: &mut IncaApp) {
    let cam = render_camera(app);
    app.windows_state.rv.viewing = None;
    if renderer(app) == "hardware" {
        hardware(app, &cam);
    } else {
        app.windows_state.start_render(&app.s, &cam, 0);
    }
    let ipr = if app.windows_state.rv.ipr { " -ipr" } else { "" };
    app.echo(format!("render -camera {cam}{ipr};"));
}

/// Inca Hardware: the viewport rasterizer at render resolution, meshes only.
fn hardware(app: &mut IncaApp, cam: &str) {
    let settings = inca_core::render::RenderSettings::from_scene(&app.s);
    let size = [settings.width.max(1), settings.height.max(1)];
    let Some(gpu) = app.gpu.as_mut() else {
        app.s.msg(inca_core::scene::MsgKind::Warning, "// Warning: the hardware renderer needs a GPU viewport");
        return;
    };
    let mut vp = crate::viewport::Viewport::new(cam);
    vp.beauty = true;
    vp.grid = false;
    vp.show_sel_highlight = false;
    // like Maya: scene lights, or the default light when there are none
    let lit = app.s.nodes.values().any(|n| matches!(n.shape, inca_core::scene::Shape::Light(_)) && app.s.is_visible(&n.id));
    vp.lights = if lit { crate::viewport::LightMode::All } else { crate::viewport::LightMode::Default };
    vp.exposure = settings.exposure;
    let cv = crate::viewport::cam_view(&app.s, cam, [size[0] as f32, size[1] as f32]);
    let draws = crate::viewport::build_draws(&app.s, &vp, &cv, gpu, &mut app.smooth_cache, &Default::default());
    let fu = crate::viewport::frame_uniform(&app.s, &vp, &cv, 1);
    let px = gpu.render_offscreen(size, &fu, &draws);
    if px.len() != (size[0] * size[1] * 4) as usize {
        return;
    }
    let rs = inca_core::render::RenderScene::build(&app.s, cam, settings);
    let alpha = rs.coverage();
    let tris = rs.triangle_count();
    app.windows_state.set_image(px, alpha, size, cam, tris);
}

/// What IPR watches: scene edits, the time, the render camera and the render settings.
fn ipr_signature(app: &IncaApp) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    app.s.rev.hash(&mut h);
    let cam = render_camera(app);
    if let Some(c) = app.s.find(&cam) {
        for x in app.s.world_matrix(&c).to_cols_array() {
            x.to_bits().hash(&mut h);
        }
    }
    serde_json::to_string(&app.s.render).unwrap_or_default().hash(&mut h);
    h.finish()
}

fn job_done(app: &IncaApp) -> bool {
    app.windows_state.job.as_ref().is_none_or(|j| j.passes >= j.total)
}

/// Per frame: collect render results, restart IPR after changes, advance a Render Sequence.
pub fn tick(app: &mut IncaApp, ctx: &egui::Context) {
    app.windows_state.poll(ctx);
    if app.windows_state.rv.ipr && app.windows_state.rv.batch.is_none() {
        let sig = ipr_signature(app);
        if sig != app.windows_state.rv.ipr_sig {
            app.windows_state.rv.ipr_sig = sig;
            render(app);
        }
    }
    batch_tick(app);
    if app.windows_state.rv.dirty {
        app.windows_state.rv.dirty = false;
        refresh_texture(app, ctx);
    }
}

/// Display adjustments: exposure (stops) and gamma on the sRGB image, or the alpha channel.
fn adjusted(px: &[u8], alpha: Option<&[u8]>, show_alpha: bool, exposure: f32, gamma: f32) -> Vec<u8> {
    if show_alpha {
        return match alpha {
            Some(a) => a.iter().flat_map(|&v| [v, v, v, 255]).collect(),
            None => px.chunks(4).flat_map(|_| [255, 255, 255, 255]).collect(),
        };
    }
    if exposure == 0.0 && (gamma - 1.0).abs() < 1e-6 {
        return px.to_vec();
    }
    let k = 2f32.powf(exposure);
    let lut: Vec<u8> = (0..256).map(|i| ((i as f32 / 255.0 * k).clamp(0.0, 1.0).powf(1.0 / gamma.max(0.01)) * 255.0 + 0.5) as u8).collect();
    px.chunks(4).flat_map(|c| [lut[c[0] as usize], lut[c[1] as usize], lut[c[2] as usize], c[3]]).collect()
}

fn refresh_texture(app: &mut IncaApp, ctx: &egui::Context) {
    let ws = &mut app.windows_state;
    let (px, alpha, size) = match ws.rv.viewing.and_then(|i| ws.rv.kept.get(i)) {
        Some(k) => (k.px.clone(), k.alpha.clone(), k.size),
        None => match &ws.pixels {
            Some((p, s)) => (p.clone(), ws.rv.alpha.clone(), *s),
            None => return,
        },
    };
    let out = adjusted(&px, alpha.as_deref(), ws.rv.show_alpha, ws.rv.exposure, ws.rv.gamma);
    let img = egui::ColorImage::from_rgba_unmultiplied([size[0] as usize, size[1] as usize], &out);
    match &mut ws.image {
        Some(t) => t.set(img, egui::TextureOptions::LINEAR),
        None => ws.image = Some(ctx.load_texture("render", img, egui::TextureOptions::LINEAR)),
    }
}

fn status_line(app: &IncaApp) -> String {
    match &app.windows_state.job {
        Some(j) => format!("{}{} — {}/{} samples, {}×{}, {} triangles, {:.1}s", if app.windows_state.rv.ipr { "IPR  " } else { "" }, j.camera, j.passes, j.total, j.size[0], j.size[1], j.tris, j.elapsed.unwrap_or_else(|| j.started.elapsed().as_secs_f32())),
        None => String::new(),
    }
}

pub fn keep_image(app: &mut IncaApp) {
    let info = status_line(app);
    let ws = &mut app.windows_state;
    let Some((px, size)) = ws.pixels.clone() else {
        app.last_result = "// Warning: Render an image first.".into();
        return;
    };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let label = format!("{:02}:{:02}:{:02}", (now / 3600) % 24, (now / 60) % 60, now % 60);
    ws.rv.kept.push(Kept { px, alpha: ws.rv.alpha.clone(), size, label, info, thumb: None });
    if ws.rv.kept.len() > 24 {
        ws.rv.kept.remove(0);
    }
    app.help = "Image kept. Click a thumbnail in the strip to compare.".into();
}

fn remove_image(app: &mut IncaApp) {
    let rv = &mut app.windows_state.rv;
    match rv.viewing.take() {
        Some(i) if i < rv.kept.len() => {
            rv.kept.remove(i);
        }
        _ => {
            rv.kept.pop();
        }
    }
    rv.dirty = true;
}

fn save_image(app: &mut IncaApp) {
    let ws = &app.windows_state;
    let (px, size) = match ws.rv.viewing.and_then(|i| ws.rv.kept.get(i)) {
        Some(k) => (k.px.clone(), k.size),
        None => match &ws.pixels {
            Some(p) => p.clone(),
            None => return,
        },
    };
    let name = app.s.render.get("filename").and_then(|v| v.as_str()).unwrap_or("image").to_string();
    let dir = crate::project::default_dir(app, "images");
    let _ = std::fs::create_dir_all(&dir);
    if let Some(p) = rfd::FileDialog::new().add_filter("PNG Image", &["png"]).set_directory(&dir).set_file_name(format!("{name}.png")).save_file() {
        match image::save_buffer(&p, &px, size[0], size[1], image::ColorType::Rgba8) {
            Ok(_) => app.last_result = format!("// Result: {}", p.display()),
            Err(e) => app.s.msg(inca_core::scene::MsgKind::Error, format!("// Error: Could not save image: {e}")),
        }
    }
}

// ---------------------------------------------------------------- Render Sequence

pub fn render_sequence(app: &mut IncaApp) {
    if app.windows_state.rv.batch.is_some() {
        app.last_result = "// Warning: A batch render is already running.".into();
        return;
    }
    let g = |k: &str, d: f64| app.s.render.get(k).and_then(|v| v.as_f64()).unwrap_or(d);
    let (a, b, by) = (g("start", app.s.time.start).round() as i64, g("end", app.s.time.end).round() as i64, g("by", 1.0).round().max(1.0) as usize);
    let frames: Vec<i64> = (a..=b).step_by(by).collect();
    if frames.is_empty() {
        app.last_result = "// Warning: The frame range is empty.".into();
        return;
    }
    let dir = crate::project::default_dir(app, "images");
    let _ = std::fs::create_dir_all(&dir);
    let name = app.s.render.get("filename").and_then(|v| v.as_str()).unwrap_or("image").to_string();
    app.echo(format!("// Batch render: {} frames → {}", frames.len(), dir.display()));
    app.win.render_view = true;
    app.windows_state.rv.ipr = false;
    app.windows_state.rv.batch = Some(Batch { frames, next: 0, dir, name, restore: app.s.time.current, saved: 0, waiting: false });
}

fn goto(app: &mut IncaApp, t: f64) {
    app.s.set_time(t);
    let moved = app.s.eval_constraints();
    app.s.refresh_many(&moved);
}

fn batch_tick(app: &mut IncaApp) {
    let Some(b) = app.windows_state.rv.batch.as_ref() else { return };
    if b.waiting {
        if !job_done(app) {
            return;
        }
        let b = app.windows_state.rv.batch.as_mut().unwrap();
        let f = b.frames[b.next];
        if let Some((px, size)) = &app.windows_state.pixels {
            let p = b.dir.join(format!("{}.{f:04}.png", b.name));
            if image::save_buffer(&p, px, size[0], size[1], image::ColorType::Rgba8).is_ok() {
                b.saved += 1;
            }
        }
        b.waiting = false;
        b.next += 1;
    }
    let b = app.windows_state.rv.batch.as_ref().unwrap();
    if b.next >= b.frames.len() {
        let b = app.windows_state.rv.batch.take().unwrap();
        goto(app, b.restore);
        app.windows_state.rv.status = format!("Rendered {} of {} frames to {}", b.saved, b.frames.len(), b.dir.display());
        app.last_result = format!("// {}", app.windows_state.rv.status);
        return;
    }
    let f = b.frames[b.next];
    let n = b.next + 1;
    let total = b.frames.len();
    goto(app, f as f64);
    render(app);
    let b = app.windows_state.rv.batch.as_mut().unwrap();
    b.waiting = true;
    app.windows_state.rv.status = format!("Batch: frame {f} ({n}/{total})");
}

fn cancel_batch(app: &mut IncaApp) {
    if let Some(b) = app.windows_state.rv.batch.take() {
        if let Some(j) = &app.windows_state.job {
            j.cancel.store(true, Ordering::Relaxed);
        }
        goto(app, b.restore);
        app.windows_state.rv.status = format!("Batch cancelled after {} frames", b.saved);
    }
}

pub fn toggle_ipr(app: &mut IncaApp) {
    let rv = &mut app.windows_state.rv;
    rv.ipr = !rv.ipr;
    if rv.ipr {
        rv.ipr_sig = 0; // renders on the next frame
        app.win.render_view = true;
    } else if let Some(j) = &app.windows_state.job {
        j.cancel.store(true, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------- window

pub fn render_view(app: &mut IncaApp, ui: &mut egui::Ui) {
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Save Image...").clicked() {
                save_image(app);
                ui.close();
            }
            if ui.button("Keep Image").clicked() {
                keep_image(app);
                ui.close();
            }
            if ui.button("Remove Image").clicked() {
                remove_image(app);
                ui.close();
            }
        });
        ui.menu_button("Display", |ui| {
            if ui.radio(!app.windows_state.rv.show_alpha, "Display RGB Channels").clicked() {
                app.windows_state.rv.show_alpha = false;
                app.windows_state.rv.dirty = true;
                ui.close();
            }
            if ui.radio(app.windows_state.rv.show_alpha, "Display Alpha Channel").clicked() {
                app.windows_state.rv.show_alpha = true;
                app.windows_state.rv.dirty = true;
                ui.close();
            }
            if ui.button("Reset Exposure / Gamma").clicked() {
                app.windows_state.rv.exposure = 0.0;
                app.windows_state.rv.gamma = 1.0;
                app.windows_state.rv.dirty = true;
                ui.close();
            }
        });
        ui.menu_button("Render", |ui| {
            if ui.button("Render Current Frame").clicked() {
                render(app);
                ui.close();
            }
            if ui.radio(app.windows_state.rv.ipr, "IPR").clicked() {
                toggle_ipr(app);
                ui.close();
            }
            if ui.button("Render Sequence").clicked() {
                render_sequence(app);
                ui.close();
            }
            ui.separator();
            for (k, l) in RENDERERS {
                if ui.radio(renderer(app) == k, l).clicked() {
                    app.s.render.insert("renderer".into(), serde_json::json!(k));
                    ui.close();
                }
            }
        });
    });
    ui.horizontal(|ui| {
        if crate::icons::button(ui, "renderFrame", "Render the current frame", false, 22.0).clicked() {
            app.windows_state.rv.ipr = false;
            render(app);
        }
        let ipr_on = app.windows_state.rv.ipr;
        if ui.selectable_label(ipr_on, "IPR").on_hover_text("IPR render: updates as the scene changes").clicked() {
            toggle_ipr(app);
        }
        let running = app.windows_state.job.as_ref().is_some_and(|j| j.passes < j.total) || app.windows_state.rv.batch.is_some();
        if ui.add_enabled(running, egui::Button::new("Stop")).clicked() {
            app.windows_state.rv.ipr = false;
            if let Some(j) = &app.windows_state.job {
                j.cancel.store(true, Ordering::Relaxed);
            }
            cancel_batch(app);
        }
        ui.separator();
        if ui.button("Keep").on_hover_text("Keep image (snapshot)").clicked() {
            keep_image(app);
        }
        if ui.button("Remove").on_hover_text("Remove the shown kept image").clicked() {
            remove_image(app);
        }
        ui.separator();
        let rv = &mut app.windows_state.rv;
        if ui.selectable_label(!rv.show_alpha, "RGB").clicked() {
            rv.show_alpha = false;
            rv.dirty = true;
        }
        if ui.selectable_label(rv.show_alpha, "A").on_hover_text("Display alpha channel").clicked() {
            rv.show_alpha = true;
            rv.dirty = true;
        }
        ui.label("Exp");
        rv.dirty |= ui.add(egui::DragValue::new(&mut rv.exposure).speed(0.05).range(-10.0..=10.0).max_decimals(2)).on_hover_text("Display exposure (stops)").changed();
        ui.label("γ");
        rv.dirty |= ui.add(egui::DragValue::new(&mut rv.gamma).speed(0.01).range(0.1..=5.0).max_decimals(2)).on_hover_text("Display gamma").changed();
        ui.separator();
        egui::ComboBox::from_id_salt("rvRenderer").selected_text(RENDERERS.iter().find(|r| r.0 == renderer(app)).map(|r| r.1).unwrap_or("")).show_ui(ui, |ui| {
            for (k, l) in RENDERERS {
                if ui.selectable_label(renderer(app) == k, l).clicked() {
                    app.s.render.insert("renderer".into(), serde_json::json!(k));
                }
            }
        });
    });
    let st = if app.windows_state.rv.batch.is_some() || !app.windows_state.rv.status.is_empty() && app.windows_state.job.as_ref().is_none_or(|j| j.passes >= j.total) { app.windows_state.rv.status.clone() } else { status_line(app) };
    let st = match app.windows_state.rv.viewing.and_then(|i| app.windows_state.rv.kept.get(i)) {
        Some(k) => format!("Kept {} — {}", k.label, k.info),
        None => st,
    };
    ui.label(RichText::new(st).color(theme::TEXT_DIM));
    // kept-image strip
    if !app.windows_state.rv.kept.is_empty() {
        let ctx = ui.ctx().clone();
        egui::ScrollArea::horizontal().id_salt("kept").show(ui, |ui| {
            ui.horizontal(|ui| {
                let rv = &mut app.windows_state.rv;
                if ui.add(egui::Button::new("Live").min_size(egui::vec2(50.0, 50.0)).selected(rv.viewing.is_none())).clicked() {
                    rv.viewing = None;
                    rv.dirty = true;
                }
                for i in 0..rv.kept.len() {
                    let k = &mut rv.kept[i];
                    let th = k.thumb.get_or_insert_with(|| {
                        let img = egui::ColorImage::from_rgba_unmultiplied([k.size[0] as usize, k.size[1] as usize], &k.px);
                        ctx.load_texture(format!("kept{i}{}", k.label), img, egui::TextureOptions::LINEAR)
                    });
                    let h = 50.0;
                    let w = h * k.size[0] as f32 / k.size[1].max(1) as f32;
                    let r = ui.add(egui::Button::image((th.id(), egui::vec2(w, h))).selected(rv.viewing == Some(i))).on_hover_text(format!("{}\n{}", k.label, k.info));
                    if r.clicked() {
                        rv.viewing = Some(i);
                        rv.dirty = true;
                    }
                }
            });
        });
    }
    ui.separator();
    if let Some(t) = &app.windows_state.image {
        let size = t.size_vec2();
        let avail = ui.available_size();
        let k = (avail.x / size.x).min(avail.y / size.y).min(1.0);
        ui.centered_and_justified(|ui| ui.image((t.id(), size * k)));
    } else {
        ui.label(RichText::new(format!("{} — press Render", RENDERERS.iter().find(|r| r.0 == renderer(app)).map(|r| r.1).unwrap_or(""))).color(theme::TEXT_DIM));
    }
}
