//! Viewport panes: layout, panel menus, mouse interaction (navigation, selection, manipulators), HUD.

use crate::app::{IncaApp, Layout, Tool};
use crate::theme;
use crate::viewport::{self, CamView, Comp, LightMode, ShadeMode};
use egui::{pos2, vec2, Color32, Pos2, Rect, RichText, Sense, Stroke};
use glam::{DMat4, DQuat, DVec3};
use inca_core::mesh::epair;
use inca_core::scene::{CompMode, SelMode, Shape};
use std::collections::HashSet;
use transform_gizmo_egui::math::Transform;
use transform_gizmo_egui::prelude::*;

pub struct VpState {
    pub gizmos: Vec<Gizmo>,
    pub drag: Option<GizmoDrag>,
    pub marquee: Option<(usize, Pos2)>,
    pub nav: Option<(usize, egui::PointerButton)>,
    pub snap_step: f64,
    pub snap_angle: f64,
    /// points placed by the curve / polygon creation tools
    pub tool_pts: Vec<DVec3>,
}

impl Default for VpState {
    fn default() -> Self {
        VpState { gizmos: (0..4).map(|_| Gizmo::default()).collect(), drag: None, marquee: None, nav: None, snap_step: 1.0, snap_angle: 15.0, tool_pts: vec![] }
    }
}

pub struct GizmoDrag {
    pub vp: usize,
    pub start: Transform,
    pub cur: Transform,
    pub objects: Vec<(String, DMat4)>,
    pub comps: Vec<(String, Vec<u32>, inca_core::mesh::PolyMesh)>,
    pub tool: Tool,
    /// Edit Pivot drag: start world pivots of the objects
    pub pivots: Option<Vec<DVec3>>,
}

impl VpState {
    pub fn live_meshes(&self) -> HashSet<String> {
        self.drag.as_ref().map(|d| d.comps.iter().map(|c| c.0.clone()).collect()).unwrap_or_default()
    }
}

fn tf_matrix(t: &Transform) -> DMat4 {
    DMat4::from_scale_rotation_translation(DVec3::from(t.scale), DQuat::from(t.rotation), DVec3::from(t.translation))
}

pub fn viewports(app: &mut IncaApp, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
    let full = ui.available_rect_before_wrap();
    let panes: Vec<(usize, Rect)> = match app.layout {
        Layout::Single => vec![(app.active_vp, full)],
        Layout::Four => {
            let (w, h) = (full.width() / 2.0, full.height() / 2.0);
            let r = |x: f32, y: f32| Rect::from_min_size(pos2(full.min.x + x * w, full.min.y + y * h), vec2(w, h)).shrink(1.0);
            vec![(1, r(0.0, 0.0)), (0, r(1.0, 0.0)), (2, r(0.0, 1.0)), (3, r(1.0, 1.0))]
        }
        Layout::TwoSide => {
            let w = full.width() / 2.0;
            vec![(0, Rect::from_min_size(full.min, vec2(w, full.height())).shrink(1.0)), (1, Rect::from_min_size(pos2(full.min.x + w, full.min.y), vec2(w, full.height())).shrink(1.0))]
        }
    };
    ui.allocate_rect(full, Sense::hover());
    for (idx, rect) in panes {
        pane(app, ui, frame, idx, rect);
    }
    // drop GPU buffers of deleted meshes
    if let Some(g) = app.gpu.as_mut() {
        let keep: HashSet<String> = app.s.nodes.values().filter(|n| n.mesh().is_some()).map(|n| n.id.clone()).collect();
        g.retain_meshes(&keep);
    }
}

fn pane(app: &mut IncaApp, ui: &mut egui::Ui, frame: &mut eframe::Frame, idx: usize, rect: Rect) {
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), 20.0));
    let img_rect = Rect::from_min_max(pos2(rect.min.x, bar.max.y), rect.max);
    // ---- panel menu bar
    let mut bar_ui = ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(egui::Layout::left_to_right(egui::Align::Center)));
    bar_ui.painter().rect_filled(bar, 0.0, theme::BG);
    panel_menus(app, &mut bar_ui, idx);
    app.vps[idx].rect = img_rect;
    let ppp = ui.ctx().pixels_per_point();
    let px = [(img_rect.width() * ppp).round().max(1.0) as u32, (img_rect.height() * ppp).round().max(1.0) as u32];
    let cv = viewport::cam_view(&app.s, &app.vps[idx].camera.clone(), [img_rect.width(), img_rect.height()]);
    // ---- render
    if let (Some(rs), Some(gpu)) = (frame.wgpu_render_state(), app.gpu.as_mut()) {
        let tex = {
            let mut r = rs.renderer.write();
            gpu.target(&mut r, idx, px)
        };
        let live = app.vp_state.live_meshes();
        let draws = viewport::build_draws(&app.s, &app.vps[idx], &cv, gpu, &mut app.smooth_cache, &live);
        let fu = viewport::frame_uniform(&app.s, &app.vps[idx], &cv);
        gpu.render(idx, &fu, &draws);
        ui.painter().image(tex, img_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    } else {
        ui.painter().rect_filled(img_rect, 0.0, Color32::from_rgb(60, 64, 70));
        ui.painter().text(img_rect.center(), egui::Align2::CENTER_CENTER, "wgpu renderer unavailable", egui::FontId::proportional(14.0), Color32::WHITE);
    }
    let resp = ui.interact(img_rect, egui::Id::new(("vp", idx)), Sense::click_and_drag());
    let painter = ui.painter_at(img_rect);
    viewport::draw_component_overlay(&app.s, &app.vps[idx], &cv, &painter);
    if resp.hovered() || resp.dragged() {
        if app.active_vp != idx && (resp.clicked() || resp.drag_started() || ui.input(|i| i.pointer.any_pressed())) {
            app.active_vp = idx;
        }
    }
    let active = app.active_vp == idx;
    if active && app.layout != Layout::Single {
        painter.rect_stroke(img_rect.shrink(0.5), 0.0, Stroke::new(2.0, theme::HI), egui::StrokeKind::Inside);
    }
    interact(app, ui, idx, &resp, &cv, &painter);
    hud(app, idx, &cv, &painter);
}

fn panel_menus(app: &mut IncaApp, ui: &mut egui::Ui, idx: usize) {
    ui.spacing_mut().button_padding = vec2(4.0, 0.0);
    let small = |t: &str| RichText::new(t).size(11.5);
    ui.menu_button(small("View"), |ui| {
        for c in ["persp", "top", "front", "side"] {
            if ui.selectable_label(app.vps[idx].camera == c, c).clicked() {
                app.vps[idx].camera = c.into();
                ui.close();
            }
        }
        let cams: Vec<String> = app.s.nodes.values().filter(|n| matches!(&n.shape, Shape::Camera(c) if !c.startup)).map(|n| n.name.clone()).collect();
        if !cams.is_empty() {
            ui.separator();
            for c in cams {
                if ui.selectable_label(app.vps[idx].camera == c, &c).clicked() {
                    app.vps[idx].camera = c;
                    ui.close();
                }
            }
        }
        ui.separator();
        if ui.button("Look Through Selected").clicked() {
            if let Some(l) = app.s.lead().cloned() {
                if matches!(app.s.node(&l).map(|n| &n.shape), Some(Shape::Camera(_))) {
                    app.vps[idx].camera = app.s.name_of(&l);
                }
            }
            ui.close();
        }
        if ui.button("Frame Selection    F").clicked() {
            app.active_vp = idx;
            app.action("frameSelected");
            ui.close();
        }
        if ui.button("Frame All    A").clicked() {
            app.active_vp = idx;
            app.action("frameAll");
            ui.close();
        }
    });
    ui.menu_button(small("Shading"), |ui| {
        let v = &mut app.vps[idx];
        ui.radio_value(&mut v.shade, ShadeMode::Wire, "Wireframe  4");
        ui.radio_value(&mut v.shade, ShadeMode::Shaded, "Smooth Shade All  5");
        ui.radio_value(&mut v.shade, ShadeMode::Textured, "Textured  6");
        ui.separator();
        ui.checkbox(&mut v.wire_on_shaded, "Wireframe on Shaded");
        ui.checkbox(&mut v.xray, "X-Ray");
        ui.checkbox(&mut v.backface, "Backface Culling");
    });
    ui.menu_button(small("Lighting"), |ui| {
        let v = &mut app.vps[idx];
        ui.radio_value(&mut v.lights, LightMode::Default, "Use Default Lighting");
        ui.radio_value(&mut v.lights, LightMode::All, "Use All Lights  7");
        ui.radio_value(&mut v.lights, LightMode::Flat, "Use Flat Lighting");
    });
    ui.menu_button(small("Show"), |ui| {
        let v = &mut app.vps[idx];
        ui.checkbox(&mut v.grid, "Grid");
        ui.checkbox(&mut v.hud, "HUD");
        ui.checkbox(&mut v.show_lights, "Lights");
        ui.checkbox(&mut v.show_cameras, "Cameras");
        ui.checkbox(&mut v.show_curves, "NURBS Curves");
        ui.separator();
        if ui.checkbox(&mut v.isolate.is_some(), "Isolate Select").clicked() {
            app.active_vp = idx;
            app.action("isolateSelect");
        }
    });
    ui.menu_button(small("Panels"), |ui| {
        for (l, lay) in [("Single Pane", Layout::Single), ("Four Panes", Layout::Four), ("Two Panes", Layout::TwoSide)] {
            if ui.selectable_label(app.layout == lay, l).clicked() {
                app.active_vp = idx;
                app.layout = lay;
                ui.close();
            }
        }
    });
    ui.separator();
    let v = &mut app.vps[idx];
    use crate::icons::{self, Icon};
    for (i, tip, on, which) in [(Icon::Wire, "Wireframe (4)", v.shade == ShadeMode::Wire, 0), (Icon::Shaded, "Smooth shade (5)", v.shade == ShadeMode::Shaded, 1), (Icon::Light, "Use all lights (7)", v.lights == LightMode::All, 2), (Icon::XRay, "X-Ray", v.xray, 3), (Icon::Grid, "Grid", v.grid, 4)] {
        if icons::button(ui, i, tip, on, 19.0).clicked() {
            match which {
                0 => v.shade = ShadeMode::Wire,
                1 => v.shade = ShadeMode::Shaded,
                2 => v.lights = if v.lights == LightMode::All { LightMode::Default } else { LightMode::All },
                3 => v.xray = !v.xray,
                _ => v.grid = !v.grid,
            }
        }
    }
}

/// Selection mode from Maya modifiers: Shift toggles, Ctrl removes, Ctrl+Shift adds.
fn sel_mode(m: egui::Modifiers) -> SelMode {
    let ctrl = m.ctrl || m.command;
    match (ctrl, m.shift) {
        (true, true) => SelMode::Add,
        (true, false) => SelMode::Remove,
        (false, true) => SelMode::Toggle,
        _ => SelMode::Replace,
    }
}

fn interact(app: &mut IncaApp, ui: &mut egui::Ui, idx: usize, resp: &egui::Response, cv: &CamView, painter: &egui::Painter) {
    let (mods, pointer, scroll) = ui.input(|i| (i.modifiers, i.pointer.clone(), i.smooth_scroll_delta.y));
    let cam = app.vps[idx].camera.clone();
    // ---- navigation (Alt + mouse, wheel)
    if resp.hovered() && scroll != 0.0 {
        viewport::dolly(&mut app.s, &cam, scroll as f64 * 0.0025);
    }
    if mods.alt && resp.drag_started() {
        let b = if pointer.button_down(egui::PointerButton::Middle) {
            egui::PointerButton::Middle
        } else if pointer.button_down(egui::PointerButton::Secondary) {
            egui::PointerButton::Secondary
        } else {
            egui::PointerButton::Primary
        };
        app.vp_state.nav = Some((idx, b));
    }
    if let Some((v, b)) = app.vp_state.nav {
        if v == idx {
            let d = pointer.delta();
            match b {
                egui::PointerButton::Primary => viewport::tumble(&mut app.s, &cam, d.x, d.y),
                egui::PointerButton::Middle => viewport::track(&mut app.s, cv, &cam, d.x, d.y),
                _ => viewport::dolly(&mut app.s, &cam, (d.x + d.y) as f64 * 0.01),
            }
            if !pointer.any_down() {
                app.vp_state.nav = None;
            }
            ui.ctx().request_repaint();
            return;
        }
    }
    if mods.alt {
        return;
    }
    // ---- creation tools take the clicks
    if app.tool.is_creation() {
        creation_tool(app, idx, resp, cv, painter);
        return;
    }
    // ---- manipulator
    let gizmo_used = gizmo(app, ui, idx, resp, cv, painter);
    if gizmo_used {
        return;
    }
    // ---- selection: click or marquee (LMB)
    if resp.drag_started_by(egui::PointerButton::Primary) {
        if let Some(p) = resp.interact_pointer_pos() {
            app.vp_state.marquee = Some((idx, p - resp.drag_delta()));
        }
    }
    if let Some((v, start)) = app.vp_state.marquee {
        if v == idx {
            if let Some(cur) = pointer.interact_pos() {
                let r = Rect::from_two_pos(start, cur);
                painter.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
                if resp.drag_stopped() || !pointer.primary_down() {
                    app.vp_state.marquee = None;
                    marquee_select(app, idx, cv, r, sel_mode(mods));
                }
            }
        }
    }
    if resp.clicked_by(egui::PointerButton::Primary) {
        if let Some(p) = resp.interact_pointer_pos() {
            click_select(app, idx, cv, p, sel_mode(mods));
        }
    }
    // ---- RMB marking menu
    resp.context_menu(|ui| marking_menu(app, ui));
}

fn click_select(app: &mut IncaApp, idx: usize, cv: &CamView, p: Pos2, mode: SelMode) {
    let vp = app.vps[idx].clone();
    if let Some(cm) = app.s.comp_mode {
        let hil = app.s.hilite();
        let mut best: Option<(String, Comp)> = None;
        for id in &hil {
            if let Some(c) = viewport::pick_component(&app.s, &vp, cv, id, cm, p) {
                best = Some((id.clone(), c));
                break;
            }
        }
        match best {
            Some((id, c)) => {
                let name = app.s.name_of(&id);
                let topo = app.s.node(&id).unwrap().mesh().unwrap().mesh.topo();
                let comp = match c {
                    Comp::V(v) => format!("{name}.vtx[{v}]"),
                    Comp::E(a, b) => format!("{name}.e[{}]", topo.edge(a, b).unwrap_or(0)),
                    Comp::F(f) => format!("{name}.f[{f}]"),
                };
                let flag = match mode {
                    SelMode::Replace => "-r",
                    SelMode::Add => "-add",
                    SelMode::Toggle => "-tgl",
                    SelMode::Remove => "-d",
                };
                if mode == SelMode::Toggle {
                    // toggle a single component
                    let cs = app.s.comp.entry(id.clone()).or_default();
                    let on = match c {
                        Comp::V(v) => cs.v.contains(&v),
                        Comp::E(a, b) => cs.e.contains(&epair(a, b)),
                        Comp::F(f) => cs.f.contains(&f),
                    };
                    let _ = app.exec_mel(&format!("select {} {comp};", if on { "-d" } else { "-add" }), true);
                } else {
                    let _ = app.exec_mel(&format!("select {flag} {comp};", ), true);
                }
            }
            None => {
                if mode == SelMode::Replace {
                    // clicking empty space clears components; clicking another mesh hilites it
                    if let Some(o) = viewport::pick_object(&app.s, &vp, cv, p) {
                        if !hil.contains(&o) && app.s.node(&o).is_some_and(|n| n.mesh().is_some()) {
                            app.select_names(&[o], SelMode::Replace);
                            return;
                        }
                    }
                    for id in hil {
                        app.s.comp.remove(&id);
                    }
                    app.s.rev += 1;
                }
            }
        }
        return;
    }
    let hit = viewport::pick_object(&app.s, &vp, cv, p);
    // clicking a child selects its top-most ancestor group? Maya selects the object itself.
    match hit {
        Some(id) => app.select_names(&[id], mode),
        None => {
            if mode == SelMode::Replace {
                app.select_names(&[], SelMode::Replace);
            }
        }
    }
}

fn marquee_select(app: &mut IncaApp, idx: usize, cv: &CamView, r: Rect, mode: SelMode) {
    if r.width() < 3.0 && r.height() < 3.0 {
        return;
    }
    let vp = app.vps[idx].clone();
    if let Some(cm) = app.s.comp_mode {
        let mut parts = vec![];
        for id in app.s.hilite() {
            let name = app.s.name_of(&id);
            let topo = app.s.node(&id).unwrap().mesh().unwrap().mesh.topo();
            for c in viewport::marquee_components(&app.s, &vp, cv, &id, cm, r) {
                parts.push(match c {
                    Comp::V(v) => format!("{name}.vtx[{v}]"),
                    Comp::E(a, b) => format!("{name}.e[{}]", topo.edge(a, b).unwrap_or(0)),
                    Comp::F(f) => format!("{name}.f[{f}]"),
                });
            }
        }
        let flag = match mode {
            SelMode::Replace => "-r",
            SelMode::Remove => "-d",
            _ => "-add",
        };
        if parts.is_empty() {
            if mode == SelMode::Replace {
                for id in app.s.hilite() {
                    app.s.comp.remove(&id);
                }
                app.s.rev += 1;
            }
            return;
        }
        // keep the echo short for big selections
        let mel = format!("select {flag} {};", parts.join(" "));
        let _ = app.exec_mel(&mel, parts.len() < 200);
        return;
    }
    let ids = viewport::marquee_objects(&app.s, &vp, cv, r);
    app.select_names(&ids, if mode == SelMode::Toggle && ids.is_empty() { SelMode::Add } else { mode });
}

fn marking_menu(app: &mut IncaApp, ui: &mut egui::Ui) {
    let has_mesh = app.s.sel.iter().any(|i| app.s.node(i).is_some_and(|n| n.mesh().is_some()));
    if has_mesh {
        for (l, m) in [("Object Mode", None), ("Vertex", Some(CompMode::Vertex)), ("Edge", Some(CompMode::Edge)), ("Face", Some(CompMode::Face))] {
            if ui.selectable_label(app.s.comp_mode == m, l).clicked() {
                app.set_comp_mode(m);
                ui.close();
            }
        }
        ui.separator();
        ui.menu_button("Assign New Material", |ui| {
            for (l, a) in [("Lambert", "matLambert"), ("Blinn", "matBlinn"), ("Phong", "matPhong"), ("Standard Surface", "matStandard")] {
                if ui.button(l).clicked() {
                    app.action(a);
                    ui.close();
                }
            }
        });
        ui.menu_button("Assign Existing Material", |ui| {
            let mats: Vec<(String, String)> = app.s.mats.values().map(|m| (m.id.clone(), m.name.clone())).collect();
            for (_, n) in mats {
                if ui.button(&n).clicked() {
                    let _ = app.run_mel("assign material", &format!("hyperShade -assign {n};"));
                    ui.close();
                }
            }
        });
        ui.separator();
    }
    for a in ["selectAll", "frameSelected", "frameAll", "duplicate", "deleteSel", "centerPivot", "freezeTransformations"] {
        if ui.button(crate::actions::label(a)).clicked() {
            app.action(a);
            ui.close();
        }
    }
}

/// Move / rotate / scale manipulator. Returns true while it owns the mouse.
fn gizmo(app: &mut IncaApp, ui: &mut egui::Ui, idx: usize, resp: &egui::Response, cv: &CamView, painter: &egui::Painter) -> bool {
    if !app.tool.is_manip() {
        return false;
    }
    if let Some(d) = &app.vp_state.drag {
        if d.vp != idx {
            return false;
        }
    }
    // target frame: pivot + orientation
    let comp_targets: Vec<(String, Vec<u32>)> = if app.s.comp_mode.is_some() { app.s.hilite().into_iter().map(|id| { let v = app.s.comp_verts(&id); (id, v) }).filter(|(_, v)| !v.is_empty()).collect() } else { vec![] };
    let objects: Vec<String> = if app.s.comp_mode.is_some() { vec![] } else { app.s.sel.iter().filter(|i| app.s.node(i).is_some_and(|n| !n.startup())).cloned().collect() };
    if comp_targets.is_empty() && objects.is_empty() && app.vp_state.drag.is_none() {
        return false;
    }
    let (pivot, rot) = if let Some(d) = &app.vp_state.drag {
        (DVec3::from(d.cur.translation), DQuat::from(d.cur.rotation))
    } else if !comp_targets.is_empty() {
        let mut c = DVec3::ZERO;
        let mut n = 0;
        for (id, vs) in &comp_targets {
            let wm = app.s.world_matrix(id);
            let pm = &app.s.node(id).unwrap().mesh().unwrap().mesh;
            for v in vs {
                c += wm.transform_point3(pm.v[*v as usize]);
                n += 1;
            }
        }
        (c / n.max(1) as f64, DQuat::IDENTITY)
    } else {
        let lead = objects.last().unwrap();
        let rot = if app.world_space && app.tool != Tool::Scale { DQuat::IDENTITY } else { app.s.world_matrix(lead).to_scale_rotation_translation().1 };
        (app.s.world_pivot(lead), rot)
    };
    let pivot_mode = app.pivot_mode && app.s.comp_mode.is_none();
    let modes = match app.tool {
        _ if pivot_mode => GizmoMode::all_translate(),
        Tool::Move => GizmoMode::all_translate(),
        Tool::Rotate => GizmoMode::all_rotate(),
        _ => GizmoMode::all_scale(),
    };
    let ppp = ui.ctx().pixels_per_point();
    let g = &mut app.vp_state.gizmos[idx];
    let mut visuals = GizmoVisuals { gizmo_size: 75.0 * app.manip_size, stroke_width: 3.0, ..Default::default() };
    visuals.x_color = Color32::from_rgb(230, 57, 62);
    visuals.y_color = Color32::from_rgb(92, 214, 92);
    visuals.z_color = Color32::from_rgb(74, 132, 255);
    visuals.s_color = Color32::from_rgb(240, 225, 80);
    g.update_config(GizmoConfig {
        view_matrix: cv.view.into(),
        projection_matrix: cv.proj.into(),
        viewport: resp.rect,
        modes,
        orientation: if app.world_space && app.tool != Tool::Scale && app.s.comp_mode.is_none() { GizmoOrientation::Global } else { GizmoOrientation::Local },
        snapping: app.snap_grid,
        snap_distance: app.vp_state.snap_step as f32,
        snap_angle: (app.vp_state.snap_angle as f32).to_radians(),
        snap_scale: 0.1,
        visuals,
        pixels_per_point: ppp,
        ..Default::default()
    });
    let tf = Transform::from_scale_rotation_translation(DVec3::ONE, rot, pivot);
    let pointer = ui.input(|i| i.pointer.clone());
    let cursor = pointer.hover_pos().unwrap_or_default();
    let interaction = GizmoInteraction {
        cursor_pos: (cursor.x, cursor.y),
        hovered: resp.hovered() || app.vp_state.drag.is_some(),
        drag_started: resp.hovered() && pointer.button_pressed(egui::PointerButton::Primary),
        dragging: pointer.button_down(egui::PointerButton::Primary),
    };
    let focused_before = g.is_focused();
    let result = g.update(interaction, &[tf]);
    let draw = g.draw();
    painter.add(egui::Mesh {
        indices: draw.indices,
        vertices: draw.vertices.into_iter().zip(draw.colors).map(|(p, [r, gg, b, a])| egui::epaint::Vertex { pos: p.into(), uv: Pos2::ZERO, color: egui::Rgba::from_rgba_premultiplied(r, gg, b, a).into() }).collect(),
        ..Default::default()
    });
    match result {
        Some((_, new_tfs)) => {
            if app.vp_state.drag.is_none() {
                // drag start: snapshot what we move (one undo step per drag)
                let label = match app.tool {
                    Tool::Move => "move",
                    Tool::Rotate => "rotate",
                    _ => "scale",
                };
                app.s.checkpoint(label);
                let objs: Vec<(String, DMat4)> = objects.iter().filter(|o| !objects.iter().any(|p| p != *o && app.s.is_ancestor(p, o))).map(|o| (o.clone(), app.s.world_matrix(o))).collect();
                let comps = comp_targets.iter().map(|(id, v)| (id.clone(), v.clone(), app.s.node(id).unwrap().mesh().unwrap().mesh.clone())).collect();
                let pivots = pivot_mode.then(|| objs.iter().map(|(o, _)| app.s.world_pivot(o)).collect());
                app.vp_state.drag = Some(GizmoDrag { vp: idx, start: tf, cur: tf, objects: objs, comps, tool: if pivot_mode { Tool::Move } else { app.tool }, pivots });
            }
            let d = app.vp_state.drag.as_mut().unwrap();
            if let Some(n) = new_tfs.first() {
                d.cur = *n;
            }
            let delta = tf_matrix(&d.cur) * tf_matrix(&d.start).inverse();
            let objs = d.objects.clone();
            let pivots = d.pivots.clone();
            let comps: Vec<(String, Vec<u32>, inca_core::mesh::PolyMesh)> = d.comps.iter().map(|(a, b, c)| (a.clone(), b.clone(), c.clone())).collect();
            match pivots {
                Some(pv) => {
                    for ((id, _), p0) in objs.iter().zip(pv) {
                        app.s.set_pivot_world(id, delta.transform_point3(p0));
                    }
                }
                None => {
                    for (id, start) in objs {
                        app.s.set_world_matrix(&id, delta * start);
                    }
                }
            }
            for (id, verts, orig) in comps {
                let wm = app.s.world_matrix(&id);
                let local = wm.inverse() * delta * wm;
                if let Some(m) = app.s.nodes.get_mut(&id).and_then(|n| n.mesh_mut()) {
                    for v in &verts {
                        m.mesh.v[*v as usize] = local.transform_point3(orig.v[*v as usize]);
                    }
                }
                if let Some(n) = app.s.nodes.get_mut(&id) {
                    n.rev += 1;
                }
                app.s.refresh_dependents(&id);
            }
            app.s.touch();
            ui.ctx().request_repaint();
            true
        }
        None => {
            if let Some(d) = app.vp_state.drag.take() {
                finish_drag(app, d);
                return true;
            }
            // a press on a handle belongs to the gizmo
            focused_before && pointer.primary_down()
        }
    }
}

/// Where a click lands: the ground grid in perspective, the view plane in orthographic views.
fn place_point(cv: &CamView, rect: Rect, p: Pos2, snap: Option<f64>) -> Option<DVec3> {
    let r = cv.ray(p, rect);
    let n = if cv.ortho { -cv.fwd } else { DVec3::Y };
    let d = r.dir.dot(n);
    if d.abs() < 1e-9 {
        return None;
    }
    let t = -r.origin.dot(n) / d;
    if t < 0.0 && !cv.ortho {
        return None;
    }
    let mut w = r.at(t);
    if let Some(st) = snap {
        w = (w / st).round() * st;
    }
    // keep exactly on the plane
    w -= n * w.dot(n);
    Some(w)
}

fn creation_tool(app: &mut IncaApp, idx: usize, resp: &egui::Response, cv: &CamView, painter: &egui::Painter) {
    let rect = app.vps[idx].rect;
    let snap = app.snap_grid.then_some(app.vp_state.snap_step);
    if resp.clicked_by(egui::PointerButton::Primary) {
        if let Some(p) = resp.interact_pointer_pos().and_then(|p| place_point(cv, rect, p, snap)) {
            app.vp_state.tool_pts.push(p);
            if app.tool == Tool::Arc && app.vp_state.tool_pts.len() == 3 {
                finish_creation(app);
            }
        }
    }
    if resp.clicked_by(egui::PointerButton::Secondary) {
        finish_creation(app);
    }
    // preview
    let pts = app.vp_state.tool_pts.clone();
    let mut preview = pts.clone();
    if let Some(h) = resp.hover_pos().and_then(|p| place_point(cv, rect, p, snap)) {
        preview.push(h);
    }
    let line: Vec<DVec3> = match app.tool {
        Tool::CvCurve if preview.len() >= 2 => inca_core::curves::Curve::new(preview.clone(), 3.min(preview.len() - 1), "open", "cv").sample(12),
        Tool::EpCurve if preview.len() >= 2 => inca_core::curves::Curve::new(preview.clone(), 3, "open", "ep").sample(12),
        Tool::Arc if preview.len() == 3 => inca_core::curves::arc_through_points(preview[0], preview[2], preview[1], 0.05).unwrap_or(preview.clone()),
        Tool::Polygon if preview.len() >= 2 => {
            let mut l = preview.clone();
            l.push(preview[0]);
            l
        }
        _ => preview.clone(),
    };
    let sp: Vec<Pos2> = line.iter().filter_map(|p| cv.project(*p, rect)).collect();
    painter.add(egui::Shape::line(sp, Stroke::new(1.5, Color32::from_rgb(255, 225, 74))));
    for p in &pts {
        if let Some(q) = cv.project(*p, rect) {
            painter.rect_filled(Rect::from_center_size(q, vec2(6.0, 6.0)), 0.0, Color32::from_rgb(92, 255, 92));
        }
    }
}

/// Complete the active creation tool (Enter).
pub fn finish_creation(app: &mut IncaApp) {
    let pts = std::mem::take(&mut app.vp_state.tool_pts);
    let f = |p: &DVec3| format!("-p {} {} {}", inca_core::math::fmt_num(p.x), inca_core::math::fmt_num(p.y), inca_core::math::fmt_num(p.z));
    let flags = |pts: &[DVec3]| pts.iter().map(f).collect::<Vec<_>>().join(" ");
    let mel = match app.tool {
        Tool::CvCurve if pts.len() >= 2 => format!("curve -d {} {};", 3.min(pts.len() - 1), flags(&pts)),
        Tool::EpCurve if pts.len() >= 2 => format!("curve -d 3 {};", pts.iter().map(|p| f(p).replacen("-p", "-ep", 1)).collect::<Vec<_>>().join(" ")),
        Tool::Arc if pts.len() == 3 => format!("curve -arc {};", flags(&pts)),
        Tool::Polygon if pts.len() >= 3 => {
            // face the camera that drew it
            let cam = viewport::cam_view(&app.s, &app.vps[app.active_vp].camera.clone(), [1.0, 1.0]);
            let n = inca_core::mesh::newell(&pts);
            let mut p = pts.clone();
            if n.dot(-cam.fwd) < 0.0 {
                p.reverse();
            }
            format!("polyCreateFacet {};", flags(&p))
        }
        _ => return,
    };
    let _ = app.run_mel("create", &mel);
}

fn finish_drag(app: &mut IncaApp, d: GizmoDrag) {
    let delta = tf_matrix(&d.cur) * tf_matrix(&d.start).inverse();
    let (s, r, t) = delta.to_scale_rotation_translation();
    let names: Vec<String> = d.objects.iter().map(|o| app.s.name_of(&o.0)).collect();
    let f = inca_core::math::fmt_num;
    // components: commit as a polyTweak history node
    for (id, verts, orig) in &d.comps {
        let cur = app.s.node(id).unwrap().mesh().unwrap().mesh.clone();
        let deltas: Vec<[f64; 3]> = verts.iter().map(|&v| (cur.v[v as usize] - orig.v[v as usize]).to_array()).collect();
        if let Some(m) = app.s.nodes.get_mut(id).and_then(|n| n.mesh_mut()) {
            m.mesh = orig.clone();
        }
        let mut p = inca_core::params::Params::new();
        p.insert("verts".into(), serde_json::json!(verts));
        p.insert("deltas".into(), serde_json::json!(deltas));
        app.s.apply_op(id, "polyTweak", p, true);
    }
    if d.pivots.is_some() {
        for (id, _) in &d.objects {
            let p = app.s.world_pivot(id);
            let n = app.s.name_of(id);
            app.echo(format!("xform -ws -piv {} {} {} {n};", f(p.x), f(p.y), f(p.z)));
        }
        return;
    }
    let echo = match d.tool {
        Tool::Move => format!("move -r {} {} {}{};", f(t.x), f(t.y), f(t.z), if names.is_empty() { String::new() } else { format!(" {}", names.join(" ")) }),
        Tool::Rotate => {
            let e = inca_core::math::euler_deg_from_quat(r, "xyz");
            format!("rotate -r -ws {} {} {};", f(e[0]), f(e[1]), f(e[2]))
        }
        _ => format!("scale -r {} {} {};", f(s.x), f(s.y), f(s.z)),
    };
    app.echo(echo);
    if app.s.time.auto_key {
        for (id, _) in &d.objects {
            let attrs: Vec<String> = inca_core::scene::CHANNELS.iter().filter(|c| app.s.anim.is_keyed(id, c)).map(|c| c.to_string()).collect();
            for a in attrs {
                app.s.set_key(id, &a, None, None);
            }
        }
    }
}

fn hud(app: &IncaApp, idx: usize, cv: &CamView, p: &egui::Painter) {
    let vp = &app.vps[idx];
    let r = vp.rect;
    // axis triad
    let o = pos2(r.min.x + 32.0, r.max.y - 32.0);
    for (ax, c, l) in [(DVec3::X, Color32::from_rgb(224, 57, 62), "x"), (DVec3::Y, Color32::from_rgb(92, 214, 92), "y"), (DVec3::Z, Color32::from_rgb(74, 132, 255), "z")] {
        let v = cv.view.transform_vector3(ax);
        let e = o + vec2(v.x as f32, -v.y as f32) * 20.0;
        p.line_segment([o, e], Stroke::new(2.0, c));
        p.text(e, egui::Align2::CENTER_CENTER, l, egui::FontId::proportional(11.0), c);
    }
    if !vp.hud {
        return;
    }
    p.text(pos2(r.center().x, r.max.y - 12.0), egui::Align2::CENTER_CENTER, &vp.camera, egui::FontId::proportional(12.0), Color32::from_gray(220));
    let (v, e, f, t) = app.s.poly_count();
    let txt = format!("Verts: {v}\nEdges: {e}\nFaces: {f}\nTris:  {t}");
    p.text(pos2(r.min.x + 8.0, r.min.y + 6.0), egui::Align2::LEFT_TOP, txt, egui::FontId::monospace(11.0), Color32::from_gray(210));
    if app.s.comp_mode.is_some() {
        let m = match app.s.comp_mode {
            Some(CompMode::Vertex) => "Vertex",
            Some(CompMode::Edge) => "Edge",
            _ => "Face",
        };
        p.text(pos2(r.max.x - 8.0, r.min.y + 6.0), egui::Align2::RIGHT_TOP, format!("{m} mode"), egui::FontId::proportional(12.0), Color32::from_rgb(255, 200, 120));
    }
    if app.pivot_mode {
        p.text(pos2(r.max.x - 8.0, r.min.y + 38.0), egui::Align2::RIGHT_TOP, "Edit Pivot", egui::FontId::proportional(12.0), Color32::from_rgb(95, 211, 255));
    }
    if vp.isolate.is_some() {
        p.text(pos2(r.max.x - 8.0, r.min.y + 22.0), egui::Align2::RIGHT_TOP, "Isolate Select", egui::FontId::proportional(12.0), Color32::from_rgb(255, 230, 120));
    }
    p.text(pos2(r.max.x - 8.0, r.max.y - 12.0), egui::Align2::RIGHT_CENTER, format!("frame {}", inca_core::math::fmt_num_long(app.s.time.current)), egui::FontId::proportional(11.0), Color32::from_gray(200));
}
