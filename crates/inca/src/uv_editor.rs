//! UV Editor window (src/js/ui/uvEditor.js): select, move, rotate and scale UVs, flip / rotate /
//! normalize / layout / unfold, projections, grid / checker / shaded display.

use crate::app::IncaApp;
use egui::{pos2, vec2, Color32, Pos2, Rect, Sense, Stroke};
use glam::DVec2;
use inca_core::mesh::PolyMesh;
use inca_core::params::Params;
use inca_core::uv::{self, CornerIndex};
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Copy, PartialEq)]
pub enum UvTool {
    Select,
    Move,
    Rotate,
    Scale,
}

enum Drag {
    Pan { start: Pos2, c0: DVec2 },
    Zoom { start: Pos2, s0: f64, anchor: DVec2 },
    Marquee { start: Pos2, mode: SelMode },
    Xform { tool: UvTool, start: Pos2, pivot: DVec2, items: Vec<(String, PolyMesh, Vec<usize>)>, axis: Option<bool> },
}

#[derive(Clone, Copy, PartialEq)]
enum SelMode {
    Replace,
    Add,
    Toggle,
    Remove,
}

pub struct UvEditor {
    pub open: bool,
    center: DVec2,
    scale: f64,
    sel: HashMap<String, BTreeSet<usize>>,
    tool: UvTool,
    grid: bool,
    checker: bool,
    shade: bool,
    points: bool,
    drag: Option<Drag>,
    framed: bool,
    pending: Option<(DVec2, DVec2)>,
    /// the window's rect last frame (scene hotkeys stay out of it)
    win_rect: Option<Rect>,
    /// show the mesh's colour texture behind the UVs (Image > Display Image)
    image: bool,
    image_tex: Option<(u64, egui::TextureHandle)>,
    /// UV mode (F12): last synced component selection
    synced: u64,
}

impl Default for UvEditor {
    fn default() -> Self {
        UvEditor { open: false, center: DVec2::splat(0.5), scale: 400.0, sel: HashMap::new(), tool: UvTool::Move, grid: true, checker: false, shade: false, points: true, drag: None, framed: false, pending: None, win_rect: None, image: true, image_tex: None, synced: 0 }
    }
}

/// Meshes the editor shows: hilited (component mode) or selected meshes.
fn targets(app: &IncaApp) -> Vec<String> {
    let list = if app.s.comp_mode.is_some() && !app.s.hilite().is_empty() { app.s.hilite() } else { app.s.sel.clone() };
    list.into_iter().filter(|i| app.s.node(i).is_some_and(|n| n.mesh().is_some())).collect()
}

fn mesh(app: &IncaApp, id: &str) -> Option<PolyMesh> {
    app.s.node(id).and_then(|n| n.mesh()).map(|m| m.mesh.clone())
}

/// Record a mesh's UVs in history, merging into a trailing polyUVSet (one step per edit).
fn commit(app: &mut IncaApp, id: &str, pm: &PolyMesh) {
    let Some(uvs) = &pm.uv else { return };
    let v = serde_json::to_value(uvs).unwrap_or_default();
    let last = app.s.node(id).and_then(|n| n.mesh()).map(|m| (m.history.len(), m.history.last().map(|h| h.ty.clone())));
    match last {
        Some((n, Some(t))) if t == "polyUVSet" => app.s.set_history_param(id, n - 1, "uv", v),
        _ => {
            let mut p = Params::new();
            p.insert("uv".into(), v);
            app.s.apply_op(id, "polyUVSet", p, true);
        }
    }
}

pub fn open(app: &mut IncaApp) {
    app.uv_editor.open = true;
    app.uv_editor.framed = false;
}

impl UvEditor {
    fn to_screen(&self, r: Rect, p: DVec2) -> Pos2 {
        pos2(r.center().x + ((p.x - self.center.x) * self.scale) as f32, r.center().y - ((p.y - self.center.y) * self.scale) as f32)
    }
    fn to_uv(&self, r: Rect, q: Pos2) -> DVec2 {
        DVec2::new((q.x - r.center().x) as f64 / self.scale + self.center.x, self.center.y - (q.y - r.center().y) as f64 / self.scale)
    }
    fn frame(&mut self, r: Rect, b: Option<(DVec2, DVec2)>) {
        let (lo, hi) = b.unwrap_or((DVec2::ZERO, DVec2::ONE));
        let sz = (hi - lo).max(DVec2::splat(1e-3));
        self.scale = ((r.width() as f64 - 60.0) / sz.x).min((r.height() as f64 - 60.0) / sz.y).max(5.0);
        self.center = (lo + hi) * 0.5;
    }
    fn sel_count(&self) -> usize {
        self.sel.values().map(|s| s.len()).sum()
    }
}

fn bounds_all(app: &IncaApp) -> Option<(DVec2, DVec2)> {
    let mut b = None;
    for id in targets(app) {
        if let Some(pm) = mesh(app, &id) {
            let ci = CornerIndex::new(&pm);
            b = uv::merge_bounds(b, uv::uv_bounds(&pm, &ci, &uv::all_corners(&pm, &ci)));
        }
    }
    b
}

fn bounds_sel(app: &IncaApp) -> Option<(DVec2, DVec2)> {
    let mut b = None;
    for (id, s) in &app.uv_editor.sel {
        if let Some(pm) = mesh(app, id) {
            let ci = CornerIndex::new(&pm);
            b = uv::merge_bounds(b, uv::uv_bounds(&pm, &ci, &s.iter().copied().collect::<Vec<_>>()));
        }
    }
    b
}

fn apply_sel(app: &mut IncaApp, entries: Vec<(String, Vec<usize>)>, mode: SelMode) {
    let sel = &mut app.uv_editor.sel;
    if mode == SelMode::Replace {
        sel.clear();
    }
    for (id, ids) in entries {
        let s = sel.entry(id.clone()).or_default();
        for i in ids {
            match mode {
                SelMode::Remove => {
                    s.remove(&i);
                }
                SelMode::Toggle => {
                    if !s.remove(&i) {
                        s.insert(i);
                    }
                }
                _ => {
                    s.insert(i);
                }
            }
        }
        if s.is_empty() {
            sel.remove(&id);
        }
    }
}

fn select_all(app: &mut IncaApp) {
    let e: Vec<(String, Vec<usize>)> = targets(app).into_iter().filter_map(|id| mesh(app, &id).map(|pm| (id, uv::all_corners(&pm, &CornerIndex::new(&pm))))).collect();
    apply_sel(app, e, SelMode::Replace);
}

fn select_shell(app: &mut IncaApp) {
    let mut e = vec![];
    for (id, s) in app.uv_editor.sel.clone() {
        let Some(pm) = mesh(app, &id) else { continue };
        let ci = CornerIndex::new(&pm);
        let (shell_of, shells) = uv::uv_shells(&pm, &ci);
        let mut want: Vec<usize> = s.iter().filter_map(|&c| usize::try_from(shell_of[ci.cf[c]]).ok()).collect();
        want.sort();
        want.dedup();
        let faces: Vec<usize> = want.iter().flat_map(|&w| shells[w].clone()).collect();
        e.push((id, uv::corners_of_faces(&pm, &ci, &faces)));
    }
    apply_sel(app, e, SelMode::Add);
}

/// Select > Convert Scene Selection to UVs.
fn from_scene(app: &mut IncaApp) {
    let mut e = vec![];
    for id in targets(app) {
        let Some(pm) = mesh(app, &id) else { continue };
        let ci = CornerIndex::new(&pm);
        let cs = app.s.comp_sel(&id);
        let ids: Vec<usize> = if app.s.comp_mode == Some(inca_core::scene::CompMode::Face) {
            uv::corners_of_faces(&pm, &ci, &cs.f.iter().copied().collect::<Vec<_>>())
        } else {
            app.s.comp_verts(&id).iter().flat_map(|v| ci.by_vert.get(v).cloned().unwrap_or_default()).filter(|&c| uv::corner_uv(&pm, &ci, c).is_some()).collect()
        };
        if !ids.is_empty() {
            e.push((id, ids));
        }
    }
    apply_sel(app, e, SelMode::Replace);
}

/// Modify menu ops: on the selected UVs, or everything when nothing is selected.
fn op(app: &mut IncaApp, kind: &str) {
    let ts: Vec<String> = targets(app).into_iter().filter(|i| mesh(app, i).is_some_and(|m| m.uv.is_some())).collect();
    if ts.is_empty() {
        app.help = "Select a mesh with UVs".into();
        return;
    }
    app.s.checkpoint(&format!("UV {kind}"));
    let any = app.uv_editor.sel_count() > 0;
    for id in ts {
        let ids: Vec<usize> = if any { app.uv_editor.sel.get(&id).map(|s| s.iter().copied().collect()).unwrap_or_default() } else { vec![] };
        if any && ids.is_empty() {
            continue;
        }
        let pm = uv::uv_op(&mesh(app, &id).unwrap(), kind, &ids);
        commit(app, &id, &pm);
    }
    let echo = match kind {
        "flipU" => "polyFlipUV -flipType 0;",
        "flipV" => "polyFlipUV -flipType 1;",
        "rot90" => "polyRotateUVs 90;",
        "rot-90" => "polyRotateUVs -90;",
        "normalize" => "polyNormalizeUV -normalizeType 1 -preserveAspectRatio on;",
        "layout" => "polyMultiLayoutUV -layoutMethod 1;",
        _ => "u3dUnfold -ite 1;",
    };
    app.echo(echo);
}

fn project(app: &mut IncaApp, action: &str) {
    app.action(action);
    app.uv_editor.sel.clear();
    app.uv_editor.framed = false;
}

pub fn window(app: &mut IncaApp, ctx: &egui::Context) {
    if !app.uv_editor.open {
        return;
    }
    // UV mode (F12): the viewport's picks select the matching UVs here
    if app.uv_mode {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let mut keys: Vec<String> = app.s.comp.iter().map(|(k, v)| format!("{k}{v:?}")).collect();
        keys.sort();
        keys.hash(&mut h);
        let sig = h.finish();
        if sig != app.uv_editor.synced {
            app.uv_editor.synced = sig;
            from_scene(app);
        }
    }
    let mut open = true;
    let shown = egui::Window::new("UV Editor").open(&mut open).default_size([760.0, 620.0]).show(ctx, |ui| {
        menus(app, ui);
        toolbar(app, ui);
        let avail = ui.available_size() - vec2(0.0, 20.0);
        let (rect, resp) = ui.allocate_exact_size(avail.max(vec2(100.0, 100.0)), Sense::click_and_drag());
        if !app.uv_editor.framed {
            let b = uv::merge_bounds(bounds_all(app), Some((DVec2::ZERO, DVec2::ONE)));
            app.uv_editor.frame(rect, b);
            app.uv_editor.framed = true;
        }
        canvas(app, ui, rect, &resp);
        keys(app, ui, rect, &resp);
        let n_sel = app.uv_editor.sel_count();
        let n_all: usize = targets(app).iter().filter_map(|i| mesh(app, i)).map(|pm| uv::all_corners(&pm, &CornerIndex::new(&pm)).len()).sum();
        ui.label(egui::RichText::new(format!("{} mesh(es)   UV corners: {n_sel} / {n_all} selected   Q/W/E/R tools · Alt+MMB pan · Alt+RMB / wheel zoom · Shift constrains · F/A frame", targets(app).len())).size(11.0).color(crate::theme::TEXT_DIM));
    });
    app.uv_editor.win_rect = shown.map(|r| r.response.rect);
    if !open {
        app.uv_editor.open = false;
        app.uv_editor.win_rect = None;
    }
}

fn menus(app: &mut IncaApp, ui: &mut egui::Ui) {
    egui::MenuBar::new().ui(ui, |ui| {
        let item = |ui: &mut egui::Ui, l: &str| {
            let c = ui.button(l).clicked();
            if c {
                ui.close();
            }
            c
        };
        ui.menu_button("Edit", |ui| {
            if item(ui, "Undo") {
                app.action("undo");
            }
            if item(ui, "Redo") {
                app.action("redo");
            }
        });
        ui.menu_button("Select", |ui| {
            if item(ui, "Select All UVs") {
                select_all(app);
            }
            if item(ui, "Select Shell") {
                select_shell(app);
            }
            if item(ui, "Convert Scene Selection to UVs") {
                from_scene(app);
            }
            if item(ui, "Clear Selection") {
                app.uv_editor.sel.clear();
            }
        });
        ui.menu_button("Create", |ui| {
            for (l, a) in [("Planar", "planarMap"), ("Cylindrical", "cylindricalMap"), ("Spherical", "sphericalMap"), ("Automatic", "automaticMap")] {
                if item(ui, l) {
                    project(app, a);
                }
            }
        });
        ui.menu_button("Modify", |ui| {
            for (l, k) in [("Flip U", "flipU"), ("Flip V", "flipV"), ("Rotate 90° CCW", "rot90"), ("Rotate 90° CW", "rot-90")] {
                if item(ui, l) {
                    op(app, k);
                }
            }
            ui.separator();
            for (l, k) in [("Normalize", "normalize"), ("Layout", "layout"), ("Unfold (Relax)", "relax")] {
                if item(ui, l) {
                    op(app, k);
                }
            }
        });
        ui.menu_button("Tool", |ui| {
            for (l, t) in [("Select Tool  Q", UvTool::Select), ("Move Tool  W", UvTool::Move), ("Rotate Tool  E", UvTool::Rotate), ("Scale Tool  R", UvTool::Scale)] {
                if ui.radio(app.uv_editor.tool == t, l).clicked() {
                    app.uv_editor.tool = t;
                    ui.close();
                }
            }
        });
        ui.menu_button("View", |ui| {
            if item(ui, "Frame All  A") {
                app.uv_editor.framed = false;
            }
            ui.separator();
            let e = &mut app.uv_editor;
            ui.checkbox(&mut e.grid, "Grid");
            ui.checkbox(&mut e.checker, "Checker Map");
            ui.checkbox(&mut e.image, "Display Image");
            ui.checkbox(&mut e.shade, "Shade UVs");
            ui.checkbox(&mut e.points, "UV Points");
        });
        ui.menu_button("Help", |ui| {
            if item(ui, "Help on UV Editor") {
                app.help = "UV Editor: click/marquee UVs (Shift toggle, Ctrl deselect, Ctrl+Shift add) · W/E/R + LMB drag on the selection (or MMB anywhere) to move/rotate/scale · Shift constrains, Ctrl snaps · Alt+MMB pan · wheel zoom".into();
            }
        });
    });
}

fn toolbar(app: &mut IncaApp, ui: &mut egui::Ui) {
    use crate::icons::{self, separator};
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (i, tip, t) in [("select", "Select UVs (Q)", UvTool::Select), ("move", "Move UVs (W)", UvTool::Move), ("rotate", "Rotate UVs (E)", UvTool::Rotate), ("scale", "Scale UVs (R)", UvTool::Scale)] {
            if icons::button(ui, i, tip, app.uv_editor.tool == t, 22.0).clicked() {
                app.uv_editor.tool = t;
            }
        }
        separator(ui);
        for (l, tip, a) in [("Planar", "Planar mapping", "planarMap"), ("Cyl", "Cylindrical mapping", "cylindricalMap"), ("Sph", "Spherical mapping", "sphericalMap")] {
            if ui.small_button(l).on_hover_text(tip).clicked() {
                project(app, a);
            }
        }
        if icons::button(ui, "autoMap", "Automatic mapping", false, 22.0).clicked() {
            project(app, "automaticMap");
        }
        separator(ui);
        for (l, tip, k) in [("Flip U", "Flip selected UVs in U", "flipU"), ("Flip V", "Flip selected UVs in V", "flipV"), ("⟲90", "Rotate UVs 90° counter-clockwise", "rot90"), ("⟳90", "Rotate UVs 90° clockwise", "rot-90")] {
            if ui.small_button(l).on_hover_text(tip).clicked() {
                op(app, k);
            }
        }
        separator(ui);
        for (l, tip, k) in [("Normalize", "Fit selected (or all) UVs into 0..1", "normalize"), ("Layout", "Lay out UV shells in 0..1", "layout"), ("Unfold", "Relax UVs (borders pinned)", "relax")] {
            if ui.small_button(l).on_hover_text(tip).clicked() {
                op(app, k);
            }
        }
        separator(ui);
        let e = &mut app.uv_editor;
        if icons::button(ui, "grid", "Toggle grid", e.grid, 22.0).clicked() {
            e.grid = !e.grid;
        }
        if icons::button(ui, "checker", "Checker tile", e.checker, 22.0).clicked() {
            e.checker = !e.checker;
        }
        if icons::button(ui, "smoothShade", "Shade UVs (blue = front, red = flipped)", e.shade, 22.0).clicked() {
            e.shade = !e.shade;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icons::button(ui, "frameSel", "Frame selection (F)", false, 22.0).clicked() {
                app.uv_editor.framed = true;
                let b = bounds_sel(app);
                app.uv_editor.frame_pending(b);
            }
            if icons::button(ui, "frameAll", "Frame all (A)", false, 22.0).clicked() {
                app.uv_editor.framed = false;
            }
        });
    });
}

impl UvEditor {
    /// Frame on the next canvas draw (the rect isn't known in the toolbar).
    fn frame_pending(&mut self, b: Option<(DVec2, DVec2)>) {
        if let Some((lo, hi)) = b {
            let pad = DVec2::splat(0.02);
            self.pending = Some((lo - pad, hi + pad));
        } else {
            self.framed = false;
        }
    }
}

fn nice_step(x: f64) -> f64 {
    let p = 10f64.powf(x.log10().floor());
    for m in [1.0, 2.0, 5.0, 10.0] {
        if m * p >= x {
            return m * p;
        }
    }
    10.0 * p
}

/// The texture on the colour slot of the first shown mesh's material (cached until it changes).
fn image_texture(app: &mut IncaApp, ctx: &egui::Context) -> Option<egui::TextureId> {
    let id = targets(app).into_iter().next()?;
    let mid = app.s.node(&id)?.mesh()?.material.clone();
    let m = app.s.mats.get(&mid)?;
    let tid = m.maps.get(m.color_attr())?.as_str()?;
    let t = app.s.texs.get(tid)?;
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (tid, serde_json::to_string(&t.attrs).unwrap_or_default()).hash(&mut h);
    let key = h.finish();
    if let Some((k, tex)) = &app.uv_editor.image_tex {
        if *k == key {
            return Some(tex.id());
        }
    }
    let size = 512usize;
    let px = match inca_core::materials::texture_swatch(t, size) {
        Some(p) => p,
        None => {
            let path = t.attrs.get("fileTextureName").and_then(|v| v.as_str()).unwrap_or("");
            let img = image::open(path).ok()?.to_rgba8();
            image::imageops::resize(&img, size as u32, size as u32, image::imageops::FilterType::Triangle).into_raw()
        }
    };
    let tex = ctx.load_texture("uvImage", egui::ColorImage::from_rgba_unmultiplied([size, size], &px), egui::TextureOptions::LINEAR);
    let tid = tex.id();
    app.uv_editor.image_tex = Some((key, tex));
    Some(tid)
}

fn canvas(app: &mut IncaApp, ui: &mut egui::Ui, rect: Rect, resp: &egui::Response) {
    if let Some(b) = app.uv_editor.pending.take() {
        app.uv_editor.frame(rect, Some(b));
    }
    let p = ui.painter_at(rect);
    let e = &app.uv_editor;
    p.rect_filled(rect, 0.0, Color32::from_gray(0x32));
    let (t0, t1) = (e.to_screen(rect, DVec2::new(0.0, 1.0)), e.to_screen(rect, DVec2::new(1.0, 0.0)));
    let tile = Rect::from_two_pos(t0, t1);
    p.rect_filled(tile, 0.0, Color32::from_gray(0x3c));
    // the colour texture of the first mesh's material behind the 0-1 tile
    if e.image && !e.checker {
        if let Some(t) = image_texture(app, ui.ctx()) {
            p.image(t, tile, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::from_white_alpha(200));
        }
    }
    let e = &app.uv_editor;
    if e.checker {
        let n = 8;
        let c = tile.width() / n as f32;
        for j in 0..n {
            for i in 0..n {
                let col = if (i + j) % 2 == 1 { Color32::from_gray(0x6d) } else { Color32::from_gray(0xb4) };
                p.rect_filled(Rect::from_min_size(tile.min + vec2(i as f32 * c, j as f32 * c), vec2(c, c)), 0.0, col);
            }
        }
    }
    if e.grid {
        let step = nice_step(40.0 / e.scale);
        let lo = e.to_uv(rect, rect.left_bottom());
        let hi = e.to_uv(rect, rect.right_top());
        let gc = if e.checker { Color32::from_black_alpha(64) } else { Color32::from_gray(0x46) };
        let mut u = (lo.x / step).floor() * step;
        while u <= hi.x {
            let x = e.to_screen(rect, DVec2::new(u, 0.0)).x;
            p.line_segment([pos2(x, rect.min.y), pos2(x, rect.max.y)], Stroke::new(1.0, gc));
            u += step;
        }
        let mut v = (lo.y / step).floor() * step;
        while v <= hi.y {
            let y = e.to_screen(rect, DVec2::new(0.0, v)).y;
            p.line_segment([pos2(rect.min.x, y), pos2(rect.max.x, y)], Stroke::new(1.0, gc));
            v += step;
        }
        p.rect_stroke(tile, 0.0, Stroke::new(1.0, Color32::from_gray(0x7d)), egui::StrokeKind::Inside);
        let ls = nice_step(70.0 / e.scale);
        let mut u = (lo.x / ls).floor() * ls;
        while u <= hi.x {
            let q = e.to_screen(rect, DVec2::new(u, 0.0));
            p.text(pos2(q.x + 2.0, (q.y + 2.0).clamp(rect.min.y + 2.0, rect.max.y - 12.0)), egui::Align2::LEFT_TOP, inca_core::math::fmt_num(u), egui::FontId::proportional(10.0), Color32::from_gray(0x9a));
            u += ls;
        }
    }
    // shells, points, selection
    let face_sel = app.s.comp_mode == Some(inca_core::scene::CompMode::Face);
    for id in targets(app) {
        let Some(pm) = mesh(app, &id) else { continue };
        if pm.uv.is_none() {
            continue;
        }
        let fsel = app.s.comp_sel(&id).f.clone();
        let e = &app.uv_editor;
        for f in 0..pm.f.len() {
            let Some(u) = pm.face_uv(f) else { continue };
            let pts: Vec<Pos2> = u.iter().map(|q| e.to_screen(rect, *q)).collect();
            let picked = face_sel && fsel.contains(&f);
            if e.shade || picked {
                let col = if picked {
                    Color32::from_rgba_unmultiplied(255, 140, 40, 90)
                } else if uv::face_uv_area(u) >= 0.0 {
                    Color32::from_rgba_unmultiplied(70, 110, 220, 72)
                } else {
                    Color32::from_rgba_unmultiplied(230, 60, 60, 90)
                };
                // fan fill (faces are near-convex)
                for i in 1..pts.len().saturating_sub(1) {
                    p.add(egui::Shape::convex_polygon(vec![pts[0], pts[i], pts[i + 1]], col, Stroke::NONE));
                }
            }
            let mut l = pts.clone();
            l.push(pts[0]);
            p.add(egui::Shape::line(l, Stroke::new(1.0, Color32::from_rgb(0x8f, 0xb4, 0xef))));
        }
        let ci = CornerIndex::new(&pm);
        let ss = e.sel.get(&id);
        if e.points && ci.len() < 200_000 {
            for c in 0..ci.len() {
                if ss.is_some_and(|s| s.contains(&c)) {
                    continue;
                }
                if let Some(q) = uv::corner_uv(&pm, &ci, c) {
                    p.rect_filled(Rect::from_center_size(e.to_screen(rect, q), vec2(3.0, 3.0)), 0.0, Color32::from_rgb(0xc8, 0x56, 0xc8));
                }
            }
        }
        for &c in ss.into_iter().flatten() {
            if let Some(q) = uv::corner_uv(&pm, &ci, c) {
                p.rect_filled(Rect::from_center_size(e.to_screen(rect, q), vec2(4.0, 4.0)), 0.0, Color32::from_rgb(0x3c, 0xff, 0x3c));
            }
        }
    }
    if targets(app).is_empty() {
        p.text(pos2(rect.center().x, rect.min.y + 24.0), egui::Align2::CENTER_CENTER, "Select a polygon mesh to edit its UVs", egui::FontId::proportional(12.0), Color32::from_gray(0x9a));
    }
    draw_manip(app, &p, rect);
    interact(app, ui, rect, resp, &p);
}

fn pivot(app: &IncaApp) -> Option<DVec2> {
    bounds_sel(app).map(|(lo, hi)| (lo + hi) * 0.5)
}

fn draw_manip(app: &IncaApp, p: &egui::Painter, rect: Rect) {
    let e = &app.uv_editor;
    if e.tool == UvTool::Select || e.sel_count() == 0 {
        return;
    }
    let pv = match &e.drag {
        Some(Drag::Xform { pivot, .. }) => Some(*pivot),
        _ => pivot(app),
    };
    let Some(pv) = pv else { return };
    let c = e.to_screen(rect, pv);
    let (r, g, y) = (Color32::from_rgb(0xe2, 0x40, 0x40), Color32::from_rgb(0x40, 0xd0, 0x40), Color32::from_rgb(0xe8, 0xe0, 0x40));
    match e.tool {
        UvTool::Move => {
            p.arrow(c, vec2(56.0, 0.0), Stroke::new(2.0, r));
            p.arrow(c, vec2(0.0, -56.0), Stroke::new(2.0, g));
            p.rect_stroke(Rect::from_center_size(c, vec2(10.0, 10.0)), 0.0, Stroke::new(1.5, y), egui::StrokeKind::Inside);
        }
        UvTool::Rotate => {
            p.circle_stroke(c, 45.0, Stroke::new(2.0, Color32::from_rgb(0x40, 0x80, 0xff)));
            p.rect_filled(Rect::from_center_size(c, vec2(4.0, 4.0)), 0.0, y);
        }
        _ => {
            p.line_segment([c, c + vec2(50.0, 0.0)], Stroke::new(2.0, r));
            p.rect_filled(Rect::from_center_size(c + vec2(50.0, 0.0), vec2(8.0, 8.0)), 0.0, r);
            p.line_segment([c, c - vec2(0.0, 50.0)], Stroke::new(2.0, g));
            p.rect_filled(Rect::from_center_size(c - vec2(0.0, 50.0), vec2(8.0, 8.0)), 0.0, g);
            p.rect_filled(Rect::from_center_size(c, vec2(10.0, 10.0)), 0.0, y);
        }
    }
}

fn hit_point(app: &IncaApp, rect: Rect, q: Pos2) -> Option<(String, usize)> {
    let mut best: Option<(f32, String, usize)> = None;
    for id in targets(app) {
        let Some(pm) = mesh(app, &id) else { continue };
        let ci = CornerIndex::new(&pm);
        for c in 0..ci.len() {
            if let Some(u) = uv::corner_uv(&pm, &ci, c) {
                let d = app.uv_editor.to_screen(rect, u).distance(q);
                if d < 6.0 && best.as_ref().is_none_or(|b| d < b.0) {
                    best = Some((d, id.clone(), c));
                }
            }
        }
    }
    best.map(|b| (b.1, b.2))
}

fn coincident(app: &IncaApp, id: &str, c: usize) -> Vec<usize> {
    mesh(app, id).map(|pm| uv::coincident(&pm, &CornerIndex::new(&pm), c)).unwrap_or_default()
}

fn inside_sel_box(app: &IncaApp, rect: Rect, q: Pos2) -> bool {
    bounds_sel(app).is_some_and(|(lo, hi)| {
        let e = &app.uv_editor;
        Rect::from_two_pos(e.to_screen(rect, lo), e.to_screen(rect, hi)).expand(10.0).contains(q)
    })
}

fn interact(app: &mut IncaApp, ui: &egui::Ui, rect: Rect, resp: &egui::Response, p: &egui::Painter) {
    let (mods, ptr, scroll) = ui.input(|i| (i.modifiers, i.pointer.clone(), i.smooth_scroll_delta.y));
    if resp.hovered() && scroll != 0.0 {
        if let Some(q) = resp.hover_pos() {
            let e = &mut app.uv_editor;
            let anchor = e.to_uv(rect, q);
            e.scale = (e.scale * 1.0015f64.powf(scroll as f64)).clamp(5.0, 1e6);
            let after = e.to_uv(rect, q);
            e.center += anchor - after;
        }
    }
    let mode = match (mods.ctrl || mods.command, mods.shift) {
        (true, true) => SelMode::Add,
        (true, false) => SelMode::Remove,
        (false, true) => SelMode::Toggle,
        _ => SelMode::Replace,
    };
    if resp.drag_started() {
        let q = resp.interact_pointer_pos().unwrap_or_default() - resp.drag_delta();
        let e = &app.uv_editor;
        let mid = ptr.button_down(egui::PointerButton::Middle);
        let right = ptr.button_down(egui::PointerButton::Secondary);
        app.uv_editor.drag = if mods.alt && right {
            Some(Drag::Zoom { start: q, s0: e.scale, anchor: e.to_uv(rect, q) })
        } else if mods.alt {
            Some(Drag::Pan { start: q, c0: e.center })
        } else if mid || (e.tool != UvTool::Select && mode == SelMode::Replace && (hit_point(app, rect, q).is_some() || inside_sel_box(app, rect, q))) {
            if !mid {
                if let Some((id, c)) = hit_point(app, rect, q) {
                    if !app.uv_editor.sel.get(&id).is_some_and(|s| s.contains(&c)) {
                        let ids = coincident(app, &id, c);
                        apply_sel(app, vec![(id, ids)], SelMode::Replace);
                    }
                }
            }
            start_xform(app, q, if mid && app.uv_editor.tool == UvTool::Select { UvTool::Move } else { app.uv_editor.tool })
        } else {
            Some(Drag::Marquee { start: q, mode })
        };
    }
    let cur = ptr.interact_pos().or(resp.hover_pos()).unwrap_or_default();
    match app.uv_editor.drag.take() {
        Some(Drag::Pan { start, c0 }) => {
            let e = &mut app.uv_editor;
            e.center = c0 - DVec2::new((cur.x - start.x) as f64, -(cur.y - start.y) as f64) / e.scale;
            app.uv_editor.drag = Some(Drag::Pan { start, c0 });
        }
        Some(Drag::Zoom { start, s0, anchor }) => {
            let e = &mut app.uv_editor;
            let f = (((cur.x - start.x) - (cur.y - start.y)) as f64 * 0.006).exp();
            e.scale = (s0 * f).clamp(5.0, 1e6);
            e.center = anchor - DVec2::new((start.x - rect.center().x) as f64, -(start.y - rect.center().y) as f64) / e.scale;
            app.uv_editor.drag = Some(Drag::Zoom { start, s0, anchor });
        }
        Some(Drag::Marquee { start, mode }) => {
            p.rect_stroke(Rect::from_two_pos(start, cur), 0.0, Stroke::new(1.0, Color32::from_gray(0xee)), egui::StrokeKind::Inside);
            app.uv_editor.drag = Some(Drag::Marquee { start, mode });
        }
        Some(Drag::Xform { tool, start, pivot, items, mut axis }) => {
            let e = &app.uv_editor;
            let (dx, dy) = ((cur.x - start.x) as f64, (cur.y - start.y) as f64);
            if mods.shift && axis.is_none() && dx.hypot(dy) > 5.0 {
                axis = Some(dx.abs() > dy.abs());
            }
            if !mods.shift {
                axis = None;
            }
            let f: Box<dyn Fn(DVec2) -> DVec2> = match tool {
                UvTool::Rotate => {
                    let (m0, m) = (e.to_uv(rect, start), e.to_uv(rect, cur));
                    let mut a = (m.y - pivot.y).atan2(m.x - pivot.x) - (m0.y - pivot.y).atan2(m0.x - pivot.x);
                    if mods.shift {
                        let st = std::f64::consts::PI / 12.0;
                        a = (a / st).round() * st;
                    }
                    Box::new(uv::rotate_about(a, pivot))
                }
                UvTool::Scale => {
                    let (fu, fv) = match axis {
                        Some(true) => ((dx * 0.01).exp(), 1.0),
                        Some(false) => (1.0, (-dy * 0.01).exp()),
                        None => {
                            let k = ((dx - dy) * 0.006).exp();
                            (k, k)
                        }
                    };
                    Box::new(move |q: DVec2| pivot + (q - pivot) * DVec2::new(fu, fv))
                }
                _ => {
                    let (mut du, mut dv) = (dx / e.scale, -dy / e.scale);
                    match axis {
                        Some(true) => dv = 0.0,
                        Some(false) => du = 0.0,
                        None => {}
                    }
                    if mods.ctrl || mods.command {
                        let g = nice_step(10.0 / e.scale);
                        du = (du / g).round() * g;
                        dv = (dv / g).round() * g;
                    }
                    Box::new(move |q: DVec2| q + DVec2::new(du, dv))
                }
            };
            for (id, orig, ids) in &items {
                let mut pm = orig.clone();
                let ci = CornerIndex::new(&pm);
                uv::xform_corners(&mut pm, &ci, ids, &f);
                if let Some(n) = app.s.nodes.get_mut(id) {
                    if let Some(m) = n.mesh_mut() {
                        m.mesh.uv = pm.uv;
                    }
                    n.rev += 1;
                }
            }
            app.s.rev += 1;
            app.uv_editor.drag = Some(Drag::Xform { tool, start, pivot, items, axis });
        }
        None => {}
    }
    if resp.drag_stopped() || (app.uv_editor.drag.is_some() && !ptr.any_down()) {
        match app.uv_editor.drag.take() {
            Some(Drag::Marquee { start, mode }) => {
                let r = Rect::from_two_pos(start, cur);
                let e = &app.uv_editor;
                let (lo, hi) = (e.to_uv(rect, r.left_bottom()), e.to_uv(rect, r.right_top()));
                let mut entries = vec![];
                for id in targets(app) {
                    let Some(pm) = mesh(app, &id) else { continue };
                    let ci = CornerIndex::new(&pm);
                    let ids: Vec<usize> = (0..ci.len()).filter(|&c| uv::corner_uv(&pm, &ci, c).is_some_and(|u| u.x >= lo.x && u.x <= hi.x && u.y >= lo.y && u.y <= hi.y)).collect();
                    if !ids.is_empty() {
                        entries.push((id, ids));
                    }
                }
                apply_sel(app, entries, mode);
            }
            Some(Drag::Xform { tool, items, pivot, .. }) => {
                // commit: one undo step for the whole drag
                app.s.checkpoint(&format!("UV {}", match tool { UvTool::Rotate => "rotate", UvTool::Scale => "scale", _ => "move" }));
                let names: Vec<String> = items.iter().map(|i| app.s.name_of(&i.0)).collect();
                for (id, orig, _) in &items {
                    let cur = mesh(app, id).unwrap_or_default();
                    if let Some(m) = app.s.nodes.get_mut(id).and_then(|n| n.mesh_mut()) {
                        m.mesh.uv = orig.uv.clone();
                    }
                    commit(app, id, &cur);
                }
                let f = inca_core::math::fmt_num;
                app.echo(match tool {
                    UvTool::Rotate => format!("polyEditUV -pu {} -pv {} -a ... {};", f(pivot.x), f(pivot.y), names.join(" ")),
                    UvTool::Scale => format!("polyEditUV -su ... -sv ... {};", names.join(" ")),
                    _ => format!("polyEditUV -u ... -v ... {};", names.join(" ")),
                });
            }
            _ => {}
        }
    }
    if resp.clicked_by(egui::PointerButton::Primary) && !mods.alt {
        let q = resp.interact_pointer_pos().unwrap_or_default();
        match hit_point(app, rect, q) {
            Some((id, c)) => {
                let ids = coincident(app, &id, c);
                apply_sel(app, vec![(id, ids)], mode);
            }
            None if mode == SelMode::Replace => app.uv_editor.sel.clear(),
            None => {}
        }
    }
    resp.context_menu(|ui| {
        for (l, k) in [("Select All UVs", "all"), ("Select Shell", "shell"), ("Clear Selection", "clear")] {
            if ui.button(l).clicked() {
                match k {
                    "all" => select_all(app),
                    "shell" => select_shell(app),
                    _ => app.uv_editor.sel.clear(),
                }
                ui.close();
            }
        }
        ui.separator();
        for (l, k) in [("Flip U", "flipU"), ("Flip V", "flipV"), ("Rotate 90° CCW", "rot90"), ("Rotate 90° CW", "rot-90")] {
            if ui.button(l).clicked() {
                op(app, k);
                ui.close();
            }
        }
        ui.separator();
        for (l, k) in [("Normalize", "normalize"), ("Layout", "layout"), ("Unfold (Relax)", "relax")] {
            if ui.button(l).clicked() {
                op(app, k);
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Frame All").clicked() {
            app.uv_editor.framed = false;
            ui.close();
        }
    });
}

fn start_xform(app: &IncaApp, start: Pos2, tool: UvTool) -> Option<Drag> {
    if app.uv_editor.sel_count() == 0 {
        return None;
    }
    let pivot = pivot(app)?;
    let items = app.uv_editor.sel.iter().filter_map(|(id, s)| mesh(app, id).map(|pm| (id.clone(), pm, s.iter().copied().collect()))).collect();
    Some(Drag::Xform { tool, start, pivot, items, axis: None })
}

/// Q/W/E/R tools, F/A framing while the pointer is over the editor.
fn keys(app: &mut IncaApp, ui: &egui::Ui, rect: Rect, resp: &egui::Response) {
    if !resp.hovered() || ui.ctx().egui_wants_keyboard_input() {
        return;
    }
    let (q, w, e, r, f, a) = ui.input(|i| (i.key_pressed(egui::Key::Q), i.key_pressed(egui::Key::W), i.key_pressed(egui::Key::E), i.key_pressed(egui::Key::R), i.key_pressed(egui::Key::F), i.key_pressed(egui::Key::A)));
    let ed = &mut app.uv_editor;
    for (k, t) in [(q, UvTool::Select), (w, UvTool::Move), (e, UvTool::Rotate), (r, UvTool::Scale)] {
        if k {
            ed.tool = t;
        }
    }
    if a {
        ed.framed = false;
    }
    if f {
        let b = bounds_sel(app);
        match b {
            Some((lo, hi)) => app.uv_editor.frame(rect, Some((lo - DVec2::splat(0.02), hi + DVec2::splat(0.02)))),
            None => app.uv_editor.framed = false,
        }
    }
}

/// The UV Editor takes Q/W/E/R/F/A while the pointer is over it, so the scene hotkeys skip them.
pub fn wants_keys(app: &IncaApp, ctx: &egui::Context) -> bool {
    app.uv_editor.win_rect.zip(ctx.pointer_hover_pos()).is_some_and(|(r, p)| r.contains(p))
}
