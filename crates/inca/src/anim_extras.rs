//! Animation > Visualize (Ghost Selected, Motion Trail) and Playblast.

use crate::app::IncaApp;
use crate::viewport::gpu::{DrawCmd, DrawUniform, Geo, LineVertex, MeshVertex, Pass};
use crate::viewport::CamView;
use egui::{vec2, Color32, Rect};
use glam::{DVec3, Mat4};
use std::path::PathBuf;

/// A ghost: world-space triangles of a mesh at another frame.
pub struct Ghost {
    tris: Vec<MeshVertex>,
    alpha: f32,
}

#[derive(Default)]
pub struct AnimViz {
    pub ghosts: Vec<Ghost>,
    /// world positions per frame, one trail per object
    pub trails: Vec<Vec<DVec3>>,
}

/// Re-evaluate the scene at `t` (keys, constraints, rigs and the meshes that follow them).
fn goto(app: &mut IncaApp, t: f64) {
    app.s.set_time(t);
    let moved = app.s.eval_constraints();
    app.s.refresh_many(&moved);
}

/// Ghost Selected: translucent copies of the selected meshes at the neighbouring keys
/// (or every `every` frames), blue before the current frame and red after.
pub fn ghost(app: &mut IncaApp) {
    let steps = crate::options::get(app, "createGhost", "steps").max(1.0) as usize;
    let every = crate::options::get(app, "createGhost", "every").max(0.0);
    let objs: Vec<String> = app.s.sel.iter().flat_map(|i| app.s.subtree(i)).filter(|i| app.s.node(i).is_some_and(|n| n.mesh().is_some())).collect();
    if objs.is_empty() {
        app.last_result = "// Warning: Select animated objects to ghost".into();
        return;
    }
    let t0 = app.s.time.current;
    let mut times: Vec<f64> = vec![];
    if every > 0.0 {
        for i in 1..=steps {
            times.push(t0 - i as f64 * every);
            times.push(t0 + i as f64 * every);
        }
    } else {
        let keyed: Vec<String> = app.s.sel.clone();
        let ks = app.s.anim.key_times(&keyed);
        let before: Vec<f64> = ks.iter().copied().filter(|t| *t < t0).collect();
        times.extend(before.iter().rev().take(steps));
        times.extend(ks.iter().copied().filter(|t| *t > t0).take(steps));
    }
    app.viz.ghosts.clear();
    for t in times {
        goto(app, t);
        let col = if t < t0 { [0.33, 0.47, 1.0] } else { [1.0, 0.47, 0.33] };
        let alpha = 0.18 + 0.1 / (1.0 + (t - t0).abs() as f32 / 10.0);
        for o in &objs {
            let wm = app.s.world_matrix(o);
            let Some(pm) = app.s.node(o).and_then(|n| n.mesh()).map(|m| &m.mesh) else { continue };
            let mut tris = vec![];
            for fi in 0..pm.f.len() {
                for tr in pm.triangulate_face(fi) {
                    for &c in &tr {
                        let p = wm.transform_point3(pm.v[pm.f[fi][c] as usize]);
                        tris.push(MeshVertex { pos: [p.x as f32, p.y as f32, p.z as f32], normal: [0.0, 1.0, 0.0], color: col });
                    }
                }
            }
            app.viz.ghosts.push(Ghost { tris, alpha });
        }
    }
    goto(app, t0);
    app.echo("ghosting;");
}

pub fn unghost(app: &mut IncaApp) {
    app.viz.ghosts.clear();
    app.echo("unghostAll;");
}

/// Create Motion Trail: the selected objects' pivots over the playback range.
pub fn motion_trail(app: &mut IncaApp) {
    app.viz.trails.clear();
    let objs = app.s.sel.clone();
    if objs.is_empty() {
        app.last_result = "// Warning: Select animated objects".into();
        return;
    }
    let (t0, a, b) = (app.s.time.current, app.s.time.start, app.s.time.end);
    let mut pts: Vec<Vec<DVec3>> = vec![vec![]; objs.len()];
    let mut t = a;
    while t <= b {
        goto(app, t);
        for (i, o) in objs.iter().enumerate() {
            pts[i].push(app.s.world_pos(o));
        }
        t += 1.0;
    }
    goto(app, t0);
    app.viz.trails = pts;
    app.echo("snapshot -motionTrail 1 -increment 1;");
}

/// Ghost and trail geometry for a viewport.
pub fn draws(viz: &AnimViz) -> Vec<DrawCmd> {
    let mut d = vec![];
    for g in &viz.ghosts {
        d.push(DrawCmd { pass: Pass::MeshBlend, geo: Geo::TransientTris(g.tris.clone()), u: DrawUniform::new(Mat4::IDENTITY).unlit().alpha(g.alpha) });
    }
    let col = [0.55, 0.85, 1.0, 1.0];
    for tr in &viz.trails {
        let mut l = vec![];
        for w in tr.windows(2) {
            l.push(LineVertex { pos: w[0].as_vec3().to_array(), color: col });
            l.push(LineVertex { pos: w[1].as_vec3().to_array(), color: col });
        }
        d.push(DrawCmd { pass: Pass::Lines, geo: Geo::TransientLines(l), u: DrawUniform::new(Mat4::IDENTITY) });
    }
    d
}

/// Frame dots along the motion trails.
pub fn overlay(app: &IncaApp, cv: &CamView, rect: Rect, painter: &egui::Painter) {
    for tr in &app.viz.trails {
        for p in tr {
            if let Some(q) = cv.project(*p, rect) {
                painter.rect_filled(Rect::from_center_size(q, vec2(3.0, 3.0)), 0.0, Color32::from_rgb(140, 215, 255));
            }
        }
    }
}

// ---------------------------------------------------------------- Playblast

#[derive(Clone, PartialEq)]
pub struct PlayblastOpts {
    pub slider: bool,
    pub start: i64,
    pub end: i64,
    pub scale: u32,
    pub path: String,
}

impl Default for PlayblastOpts {
    fn default() -> Self {
        PlayblastOpts { slider: true, start: 1, end: 120, scale: 100, path: String::new() }
    }
}

/// A running playblast: one screenshot per frame, cropped to the active viewport.
pub struct Playblast {
    frame: i64,
    end: i64,
    restore: f64,
    out: PathBuf,
    scale: u32,
    waiting: bool,
    saved: usize,
}

/// Marks screenshots taken for a playblast (the test hook ignores them).
pub struct PlayblastShot;

#[derive(Default)]
pub struct PlayblastUi {
    pub open: bool,
    pub opts: PlayblastOpts,
    pub running: Option<Playblast>,
    pub status: String,
}

fn frame_path(out: &std::path::Path, f: i64) -> PathBuf {
    let stem = out.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or("playblast".into());
    let stem = stem.trim_end_matches(".####").to_string();
    out.with_file_name(format!("{stem}.{f:04}.png"))
}

pub fn start(app: &mut IncaApp) {
    let o = app.playblast.opts.clone();
    let (a, b) = if o.slider { (app.s.time.start as i64, app.s.time.end as i64) } else { (o.start.min(o.end), o.start.max(o.end)) };
    let out = if o.path.trim().is_empty() {
        crate::project::default_dir(app, "movies").join("playblast.png")
    } else {
        let p = PathBuf::from(o.path.trim());
        if p.is_absolute() { p } else { crate::project::default_dir(app, "movies").join(p) }
    };
    if let Some(d) = out.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    app.echo(format!("playblast -startTime {a} -endTime {b} -format image -percent {} -filename \"{}\";", o.scale, out.display()));
    app.playblast.running = Some(Playblast { frame: a, end: b, restore: app.s.time.current, out, scale: o.scale, waiting: false, saved: 0 });
}

/// Advance a running playblast (called once per frame, before the viewports draw).
pub fn tick(app: &mut IncaApp, ctx: &egui::Context) {
    if app.playblast.running.is_none() {
        return;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel(app);
        return;
    }
    let pb = app.playblast.running.as_mut().unwrap();
    if pb.waiting {
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, user_data, .. } if user_data.data.as_ref().is_some_and(|d| d.is::<PlayblastShot>()) => Some(image.clone()),
                _ => None,
            })
        });
        let Some(img) = shot else {
            ctx.request_repaint();
            return;
        };
        let rect = app.vps[app.active_vp].rect;
        let ppp = ctx.pixels_per_point();
        let [w, h] = img.size;
        let x0 = ((rect.min.x * ppp) as usize).min(w);
        let y0 = ((rect.min.y * ppp) as usize).min(h);
        let x1 = ((rect.max.x * ppp) as usize).min(w);
        let y1 = ((rect.max.y * ppp) as usize).min(h);
        let pb = app.playblast.running.as_mut().unwrap();
        if x1 > x0 && y1 > y0 {
            let mut px = Vec::with_capacity((x1 - x0) * (y1 - y0) * 4);
            for y in y0..y1 {
                for x in x0..x1 {
                    px.extend_from_slice(&img.pixels[y * w + x].to_array());
                }
            }
            let mut buf = image::RgbaImage::from_raw((x1 - x0) as u32, (y1 - y0) as u32, px).unwrap();
            if pb.scale < 100 {
                let k = pb.scale as f32 / 100.0;
                let (nw, nh) = (((x1 - x0) as f32 * k).max(2.0) as u32, ((y1 - y0) as f32 * k).max(2.0) as u32);
                buf = image::imageops::resize(&buf, nw, nh, image::imageops::FilterType::Triangle);
            }
            if buf.save(frame_path(&pb.out, pb.frame)).is_ok() {
                pb.saved += 1;
            }
        }
        pb.waiting = false;
        pb.frame += 1;
    }
    let pb = app.playblast.running.as_mut().unwrap();
    if pb.frame > pb.end {
        let (restore, saved, out) = (pb.restore, pb.saved, pb.out.clone());
        app.playblast.running = None;
        goto(app, restore);
        app.playblast.status = format!("Saved {saved} frames");
        app.last_result = format!("// Result: playblast wrote {saved} frames to {}", frame_path(&out, 0).with_file_name("").display());
        return;
    }
    let (f, end) = (pb.frame, pb.end);
    pb.waiting = true;
    app.playblast.status = format!("Frame {f}");
    app.help = format!("Playblast: frame {f} of {end} (Esc cancels)");
    goto(app, f as f64);
    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(PlayblastShot)));
    ctx.request_repaint();
}

pub fn cancel(app: &mut IncaApp) {
    if let Some(pb) = app.playblast.running.take() {
        goto(app, pb.restore);
        app.playblast.status = format!("Cancelled after {} frames", pb.saved);
    }
}

pub fn window(app: &mut IncaApp, ctx: &egui::Context) {
    let mut open = app.playblast.open;
    if !open {
        return;
    }
    let (ts, te) = (app.s.time.start as i64, app.s.time.end as i64);
    let mut go = false;
    let mut stop = false;
    egui::Window::new("Playblast Options").open(&mut open).resizable(false).show(ctx, |ui| {
        let o = &mut app.playblast.opts;
        egui::Grid::new("pb").num_columns(2).show(ui, |ui| {
            ui.label("Time range");
            ui.horizontal(|ui| {
                ui.radio_value(&mut o.slider, true, "Time Slider");
                ui.radio_value(&mut o.slider, false, "Start/End");
            });
            ui.end_row();
            if o.slider {
                o.start = ts;
                o.end = te;
            }
            ui.label("Start time");
            ui.add_enabled(!o.slider, egui::DragValue::new(&mut o.start));
            ui.end_row();
            ui.label("End time");
            ui.add_enabled(!o.slider, egui::DragValue::new(&mut o.end));
            ui.end_row();
            ui.label("Format");
            ui.label("PNG image sequence");
            ui.end_row();
            ui.label("Scale");
            egui::ComboBox::from_id_salt("pbscale").selected_text(format!("{}%", o.scale)).show_ui(ui, |ui| {
                for s in [25, 50, 75, 100] {
                    ui.selectable_value(&mut o.scale, s, format!("{s}%"));
                }
            });
            ui.end_row();
            ui.label("File");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut o.path).hint_text("<project>/movies/playblast.####.png").desired_width(220.0));
                if ui.button("Browse…").clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("PNG Image", &["png"]).set_file_name("playblast.png").save_file() {
                        o.path = p.display().to_string();
                    }
                }
            });
            ui.end_row();
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&app.playblast.status).color(crate::theme::TEXT_DIM));
            if app.playblast.running.is_some() {
                stop = ui.button("Cancel").clicked();
            } else {
                go = ui.button("Playblast").clicked();
            }
        });
    });
    if go {
        start(app);
    }
    if stop || !open {
        cancel(app);
    }
    app.playblast.open = open;
}
