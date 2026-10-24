//! Interactive rigging tools: Create IK Handle, Lattice Point and Paint Skin Weights.

use crate::app::{IncaApp, Tool};
use crate::viewport::CamView;
use egui::{vec2, Color32, Pos2, Rect, Stroke};
use glam::DVec3;
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Default)]
pub struct RigToolState {
    /// IK Handle tool: the start joint once it has been clicked
    pub ik_start: Option<String>,
    /// single-chain solver instead of rotate-plane
    pub ik_sc: bool,
    /// Lattice Point tool: the lattice, its selected points and the drag anchor (world)
    pub lattice: Option<String>,
    pub lat_sel: BTreeSet<usize>,
    lat_drag: Option<DVec3>,
    lat_box: Option<Pos2>,
    /// Paint Skin Weights: the influence being painted, and whether a stroke is open
    pub influence: usize,
    pub paint_value: f64,
    pub paint_add: bool,
    pub paint_radius: f32,
    stroke: bool,
    pub sdk: SdkState,
}

impl RigToolState {
    pub fn new() -> Self {
        RigToolState { paint_value: 1.0, paint_radius: 30.0, ..Default::default() }
    }
}

pub fn is_rig_tool(t: Tool) -> bool {
    matches!(t, Tool::IkHandle | Tool::LatticePoint | Tool::PaintWeights)
}

const YELLOW: Color32 = Color32::from_rgb(255, 225, 74);
const PURPLE: Color32 = Color32::from_rgb(200, 90, 230);

pub fn interact(app: &mut IncaApp, idx: usize, resp: &egui::Response, cv: &CamView, painter: &egui::Painter) {
    let rect = app.vps[idx].rect;
    match app.tool {
        Tool::IkHandle => ik_tool(app, resp, cv, rect, painter),
        Tool::LatticePoint => lattice_tool(app, resp, cv, rect, painter),
        Tool::PaintWeights => paint_tool(app, resp, cv, rect, painter),
        _ => {}
    }
}

// ---------------------------------------------------------------- IK handle

/// Nearest visible joint within 12 px of `p`.
fn pick_joint(app: &IncaApp, cv: &CamView, rect: Rect, p: Pos2) -> Option<String> {
    let mut best: Option<(f32, String)> = None;
    for id in app.s.all_dag(false) {
        if !app.s.node(&id).is_some_and(|n| n.kind == "joint") || !app.s.is_visible(&id) {
            continue;
        }
        if let Some(q) = cv.project(app.s.world_pos(&id), rect) {
            let d = q.distance(p);
            if d < 12.0 && best.as_ref().is_none_or(|b| d < b.0) {
                best = Some((d, id));
            }
        }
    }
    best.map(|b| b.1)
}

fn ik_tool(app: &mut IncaApp, resp: &egui::Response, cv: &CamView, rect: Rect, painter: &egui::Painter) {
    if let Some(sj) = app.rig_tool.ik_start.clone() {
        if let (Some(a), Some(h)) = (cv.project(app.s.world_pos(&sj), rect), resp.hover_pos()) {
            painter.line_segment([a, h], Stroke::new(1.0, YELLOW));
            painter.circle_stroke(a, 7.0, Stroke::new(2.0, YELLOW));
        }
    }
    if let Some(j) = resp.hover_pos().and_then(|p| pick_joint(app, cv, rect, p)) {
        if let Some(q) = cv.project(app.s.world_pos(&j), rect) {
            painter.circle_stroke(q, 9.0, Stroke::new(1.0, Color32::WHITE));
        }
    }
    if !resp.clicked_by(egui::PointerButton::Primary) {
        return;
    }
    let Some(j) = resp.interact_pointer_pos().and_then(|p| pick_joint(app, cv, rect, p)) else { return };
    match app.rig_tool.ik_start.take() {
        None => {
            app.select_names(&[app.s.name_of(&j)], inca_core::scene::SelMode::Replace);
            app.rig_tool.ik_start = Some(j);
            app.help = "Create IK Handle: click the end joint".into();
        }
        Some(sj) if sj != j => {
            let mel = format!("ikHandle -sj {} -ee {} -sol {};", app.s.name_of(&sj), app.s.name_of(&j), if app.rig_tool.ik_sc { "ikSCsolver" } else { "ikRPsolver" });
            let _ = app.run_mel("ik handle", &mel);
            app.help = "Create IK Handle: click the start joint".into();
        }
        Some(sj) => app.rig_tool.ik_start = Some(sj),
    }
}

// ---------------------------------------------------------------- lattice points

fn lattice_pts(app: &IncaApp, id: &str) -> Vec<DVec3> {
    let wm = app.s.world_matrix(id);
    app.s.rig(id)["pts"].as_array().map(|a| a.iter().map(|p| wm.transform_point3(DVec3::new(p[0].as_f64().unwrap_or(0.0), p[1].as_f64().unwrap_or(0.0), p[2].as_f64().unwrap_or(0.0)))).collect()).unwrap_or_default()
}

/// The lattice the tool edits: the selected one (or the last one used).
fn active_lattice(app: &mut IncaApp) -> Option<String> {
    let sel = app.s.sel.iter().find(|i| app.s.node(i).is_some_and(|n| n.kind == "lattice")).cloned();
    if sel.is_some() && sel != app.rig_tool.lattice {
        app.rig_tool.lattice = sel;
        app.rig_tool.lat_sel.clear();
    }
    app.rig_tool.lattice.clone().filter(|l| app.s.node(l).is_some())
}

/// Regenerate a lattice's points when its divisions no longer match them.
pub fn sync_lattice_divisions(app: &mut IncaApp) {
    let ids: Vec<String> = app.s.nodes.values().filter(|n| n.kind == "lattice").map(|n| n.id.clone()).collect();
    for id in ids {
        let div = crate::viewport::lattice_div(&app.s, &id);
        let n = app.s.rig(&id)["pts"].as_array().map(|a| a.len()).unwrap_or(0);
        if n != div[0] * div[1] * div[2] {
            let mut r = app.s.rig(&id);
            r["pts"] = json!(inca_core::deformers::lattice_grid(div));
            app.s.set_rig(&id, r);
            app.s.refresh_dependents(&id);
            app.rig_tool.lat_sel.clear();
        }
    }
}

fn lattice_tool(app: &mut IncaApp, resp: &egui::Response, cv: &CamView, rect: Rect, painter: &egui::Painter) {
    let Some(lat) = active_lattice(app) else {
        app.help = "Lattice Point Tool: select a lattice first".into();
        return;
    };
    let pts = lattice_pts(app, &lat);
    let scr: Vec<Option<Pos2>> = pts.iter().map(|p| cv.project(*p, rect)).collect();
    for (i, q) in scr.iter().enumerate() {
        if let Some(q) = q {
            let c = if app.rig_tool.lat_sel.contains(&i) { YELLOW } else { PURPLE };
            painter.rect_filled(Rect::from_center_size(*q, vec2(6.0, 6.0)), 1.0, c);
        }
    }
    let mods = resp.ctx.input(|i| i.modifiers);
    let nearest = |p: Pos2| scr.iter().enumerate().filter_map(|(i, q)| q.map(|q| (i, q.distance(p)))).filter(|x| x.1 < 8.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|x| x.0);
    if resp.drag_started_by(egui::PointerButton::Primary) {
        // where the button went down, not where the drag threshold was crossed
        let p = resp.ctx.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos()).unwrap_or_default();
        match nearest(p) {
            Some(i) => {
                if !app.rig_tool.lat_sel.contains(&i) {
                    if !mods.shift {
                        app.rig_tool.lat_sel.clear();
                    }
                    app.rig_tool.lat_sel.insert(i);
                }
                app.s.checkpoint("move lattice points");
                app.rig_tool.lat_drag = Some(pts[i]);
            }
            None => app.rig_tool.lat_box = Some(p),
        }
    }
    if let (Some(start), Some(q)) = (app.rig_tool.lat_box, resp.interact_pointer_pos()) {
        let r = Rect::from_two_pos(start, q);
        painter.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
        if resp.drag_stopped() {
            if !mods.shift {
                app.rig_tool.lat_sel.clear();
            }
            for (i, s) in scr.iter().enumerate() {
                if s.is_some_and(|s| r.contains(s)) {
                    app.rig_tool.lat_sel.insert(i);
                }
            }
            app.rig_tool.lat_box = None;
        }
        return;
    }
    if let (Some(anchor), Some(q)) = (app.rig_tool.lat_drag, resp.interact_pointer_pos()) {
        // drag in the view plane through the grabbed point
        let ray = cv.ray(q, rect);
        let nrm = if cv.ortho { -cv.fwd } else { (cv.eye - anchor).normalize() };
        let denom = ray.dir.dot(nrm);
        if denom.abs() > 1e-9 {
            let t = (anchor - ray.origin).dot(nrm) / denom;
            let delta = ray.at(t) - anchor;
            if delta.length_squared() > 0.0 {
                let local = app.s.world_matrix(&lat).inverse().transform_vector3(delta);
                let mut r = app.s.rig(&lat);
                if let Some(a) = r["pts"].as_array_mut() {
                    for i in &app.rig_tool.lat_sel {
                        if let Some(p) = a.get_mut(*i) {
                            *p = json!([p[0].as_f64().unwrap_or(0.0) + local.x, p[1].as_f64().unwrap_or(0.0) + local.y, p[2].as_f64().unwrap_or(0.0) + local.z]);
                        }
                    }
                }
                app.s.set_rig(&lat, r);
                app.s.refresh_dependents(&lat);
                app.rig_tool.lat_drag = Some(anchor + delta);
            }
        }
        if resp.drag_stopped() {
            app.rig_tool.lat_drag = None;
            app.s.touch();
        }
        return;
    }
    if resp.clicked_by(egui::PointerButton::Primary) {
        let p = resp.interact_pointer_pos().unwrap_or_default();
        if !mods.shift {
            app.rig_tool.lat_sel.clear();
        }
        if let Some(i) = nearest(p) {
            if !app.rig_tool.lat_sel.remove(&i) {
                app.rig_tool.lat_sel.insert(i);
            }
        }
    }
    app.help = format!("Lattice Point Tool: {} of {} points selected. Drag points to move them, drag empty space to box select, Shift adds", app.rig_tool.lat_sel.len(), pts.len());
}

// ---------------------------------------------------------------- paint skin weights

/// The selected skinned mesh and the index of its skinCluster.
pub fn skinned(app: &IncaApp) -> Option<(String, usize)> {
    app.s.sel.iter().flat_map(|i| app.s.subtree(i)).find_map(|i| {
        let h = app.s.node(&i)?.mesh()?.history.iter().position(|h| h.ty == "skinCluster")?;
        Some((i, h))
    })
}

pub fn influences(app: &IncaApp) -> Vec<String> {
    let Some((m, h)) = skinned(app) else { return vec![] };
    app.s.node(&m).and_then(|n| n.mesh()).and_then(|x| x.history[h].params.get("joints")).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|j| j.as_str().map(String::from)).collect()).unwrap_or_default()
}

pub fn weight_of(w: &Value, j: usize) -> f64 {
    w.as_array().and_then(|a| a.iter().find(|e| e[0].as_u64() == Some(j as u64))).and_then(|e| e[1].as_f64()).unwrap_or(0.0)
}

/// Set influence `j` of one vertex to `v`, scaling the others so the weights sum to 1.
pub fn set_weight(w: &mut Value, j: usize, v: f64) {
    let v = v.clamp(0.0, 1.0);
    let mut list: Vec<(usize, f64)> = w.as_array().map(|a| a.iter().filter_map(|e| Some((e[0].as_u64()? as usize, e[1].as_f64()?))).collect()).unwrap_or_default();
    list.retain(|e| e.0 != j);
    let rest: f64 = list.iter().map(|e| e.1).sum();
    if rest > 1e-9 {
        for e in list.iter_mut() {
            e.1 *= (1.0 - v) / rest;
        }
    } else if v < 1.0 && list.is_empty() {
        // nothing else to give the weight to: keep this influence at 1
        list.clear();
        list.push((j, 1.0));
        *w = json!(list.iter().map(|(i, x)| json!([i, x])).collect::<Vec<_>>());
        return;
    }
    list.push((j, v));
    list.retain(|e| e.1 > 1e-5);
    *w = json!(list.iter().map(|(i, x)| json!([i, (x * 1e4).round() / 1e4])).collect::<Vec<_>>());
}

fn paint_tool(app: &mut IncaApp, resp: &egui::Response, cv: &CamView, rect: Rect, painter: &egui::Painter) {
    let Some((m, h)) = skinned(app) else {
        app.help = "Paint Skin Weights: select a skinned mesh".into();
        return;
    };
    let wm = app.s.world_matrix(&m);
    let verts: Vec<DVec3> = app.s.node(&m).and_then(|n| n.mesh()).map(|x| x.mesh.v.iter().map(|p| wm.transform_point3(*p)).collect()).unwrap_or_default();
    let j = app.rig_tool.influence;
    let weights = app.s.node(&m).and_then(|n| n.mesh()).and_then(|x| x.history[h].params.get("weights")).cloned().unwrap_or(json!([]));
    // weight display: black (0) to white (1) dots
    for (i, p) in verts.iter().enumerate() {
        if let Some(q) = cv.project(*p, rect) {
            let w = weight_of(&weights[i], j);
            let g = (w * 255.0) as u8;
            painter.rect_filled(Rect::from_center_size(q, vec2(4.0, 4.0)), 0.0, Color32::from_rgb(g, g, g));
        }
    }
    let r = app.rig_tool.paint_radius.max(2.0);
    if let Some(hp) = resp.hover_pos() {
        painter.circle_stroke(hp, r, Stroke::new(1.0, Color32::from_rgb(255, 64, 64)));
    }
    if resp.drag_started_by(egui::PointerButton::Primary) || resp.clicked_by(egui::PointerButton::Primary) {
        app.s.checkpoint("paint skin weights");
        app.rig_tool.stroke = true;
    }
    if !app.rig_tool.stroke {
        return;
    }
    if let Some(p) = resp.interact_pointer_pos() {
        let mut w = weights;
        let mut changed = false;
        for (i, v) in verts.iter().enumerate() {
            let Some(q) = cv.project(*v, rect) else { continue };
            let d = q.distance(p);
            if d > r {
                continue;
            }
            let fall = 1.0 - (d / r) as f64;
            let cur = weight_of(&w[i], j);
            let target = if app.rig_tool.paint_add { cur + app.rig_tool.paint_value * fall * 0.2 } else { cur + (app.rig_tool.paint_value - cur) * fall };
            if (target - cur).abs() > 1e-4 {
                set_weight(&mut w[i], j, target);
                changed = true;
            }
        }
        if changed {
            app.s.set_history_param(&m, h, "weights", w);
        }
    }
    if resp.drag_stopped() || resp.clicked() || !resp.is_pointer_button_down_on() {
        app.rig_tool.stroke = false;
        app.s.touch();
    }
}

pub fn settings(app: &mut IncaApp, ui: &mut egui::Ui) {
    match app.tool {
        Tool::IkHandle => {
            ui.label("Click the start joint, then the end joint of the chain.");
            ui.horizontal(|ui| {
                ui.label("Current solver");
                ui.radio_value(&mut app.rig_tool.ik_sc, false, "ikRPsolver");
                ui.radio_value(&mut app.rig_tool.ik_sc, true, "ikSCsolver");
            });
        }
        Tool::LatticePoint => {
            ui.label("Select a lattice, then click or box-select its points and drag them.");
            if ui.button("Reset Lattice").clicked() {
                if let Some(l) = app.rig_tool.lattice.clone() {
                    app.s.checkpoint("reset lattice");
                    let mut r = app.s.rig(&l);
                    r["pts"] = json!(inca_core::deformers::lattice_grid(crate::viewport::lattice_div(&app.s, &l)));
                    app.s.set_rig(&l, r);
                    app.s.refresh_dependents(&l);
                    app.s.touch();
                }
            }
        }
        Tool::PaintWeights => {
            let inf = influences(app);
            if inf.is_empty() {
                ui.label("Select a skinned mesh.");
                return;
            }
            ui.label("Influences");
            egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                for (i, j) in inf.iter().enumerate() {
                    ui.selectable_value(&mut app.rig_tool.influence, i, app.s.name_of(j));
                }
            });
            ui.horizontal(|ui| {
                ui.label("Paint operation");
                ui.radio_value(&mut app.rig_tool.paint_add, false, "Replace");
                ui.radio_value(&mut app.rig_tool.paint_add, true, "Add");
            });
            ui.horizontal(|ui| {
                ui.label("Value");
                ui.add(egui::Slider::new(&mut app.rig_tool.paint_value, 0.0..=1.0));
            });
            ui.horizontal(|ui| {
                ui.label("Radius (px)");
                ui.add(egui::Slider::new(&mut app.rig_tool.paint_radius, 4.0..=200.0));
            });
        }
        _ => {}
    }
}

// ---------------------------------------------------------------- Set Driven Key window

#[derive(Default)]
pub struct SdkState {
    pub driver: Option<String>,
    pub driver_attr: Option<String>,
    pub driven: Vec<String>,
    pub driven_attrs: BTreeSet<String>,
}

/// Keyable attributes of a node: the transform channels and its numeric added attributes.
fn keyable(app: &IncaApp, id: &str) -> Vec<String> {
    let mut v: Vec<String> = inca_core::scene::CHANNELS.iter().map(|c| c.to_string()).collect();
    if let Some(e) = app.s.node(id).and_then(|n| n.extra.as_object()) {
        v.extend(e.iter().filter(|(_, x)| x.is_number() || x.is_boolean()).map(|(k, _)| k.clone()));
    }
    v
}

fn attr_list(ui: &mut egui::Ui, salt: &str, attrs: &[String], sel: &mut dyn FnMut(&str) -> bool, toggle: &mut dyn FnMut(&str)) {
    egui::ScrollArea::vertical().id_salt(salt).max_height(180.0).show(ui, |ui| {
        for a in attrs {
            if ui.selectable_label(sel(a), a).clicked() {
                toggle(a);
            }
        }
    });
}

pub fn sdk_window(app: &mut IncaApp, ctx: &egui::Context) {
    let mut open = app.win.sdk;
    if !open {
        return;
    }
    egui::Window::new("Set Driven Key").open(&mut open).default_size([460.0, 420.0]).show(ctx, |ui| {
        let mut st = std::mem::take(&mut app.rig_tool.sdk);
        st.driver = st.driver.filter(|d| app.s.node(d).is_some());
        st.driven.retain(|d| app.s.node(d).is_some());
        ui.columns(2, |cols| {
            cols[0].strong("Driver");
            match &st.driver {
                Some(d) => {
                    cols[0].label(app.s.name_of(d));
                    let attrs = keyable(app, d);
                    let cur = st.driver_attr.clone();
                    let mut pick: Option<String> = None;
                    attr_list(&mut cols[0], "sdkDriver", &attrs, &mut |a| cur.as_deref() == Some(a), &mut |a| pick = Some(a.to_string()));
                    if pick.is_some() {
                        st.driver_attr = pick;
                    }
                }
                None => {
                    cols[0].label(dim("(none)"));
                }
            }
            cols[1].strong("Driven");
            if let Some(d) = st.driven.first() {
                let names: Vec<String> = st.driven.iter().map(|d| app.s.name_of(d)).collect();
                cols[1].label(names.join(", "));
                let attrs = keyable(app, d);
                let cur = st.driven_attrs.clone();
                let mut flip: Vec<String> = vec![];
                attr_list(&mut cols[1], "sdkDriven", &attrs, &mut |a| cur.contains(a), &mut |a| flip.push(a.to_string()));
                for a in flip {
                    if !st.driven_attrs.remove(&a) {
                        st.driven_attrs.insert(a);
                    }
                }
            } else {
                cols[1].label(dim("(none)"));
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Load Driver").clicked() {
                st.driver = app.s.lead().cloned();
                st.driver_attr = None;
            }
            if ui.button("Load Driven").clicked() {
                st.driven = app.s.sel.clone();
                st.driven_attrs.clear();
            }
            let ready = st.driver.is_some() && st.driver_attr.is_some() && !st.driven.is_empty() && !st.driven_attrs.is_empty();
            if ui.add_enabled(ready, egui::Button::new("Key")).clicked() {
                let drv = format!("{}.{}", app.s.name_of(st.driver.as_ref().unwrap()), st.driver_attr.as_ref().unwrap());
                let targets: Vec<String> = st.driven.iter().flat_map(|d| st.driven_attrs.iter().map(move |a| (d.clone(), a.clone()))).map(|(d, a)| format!("{}.{}", app.s.name_of(&d), a)).collect();
                let _ = app.run_mel("set driven key", &format!("setDrivenKeyframe -cd {drv} {};", targets.join(" ")));
            }
            if ui.button("Select Driver").clicked() {
                if let Some(d) = &st.driver {
                    app.select_names(&[app.s.name_of(d)], inca_core::scene::SelMode::Replace);
                }
            }
            if ui.button("Select Driven").clicked() {
                let n: Vec<String> = st.driven.iter().map(|d| app.s.name_of(d)).collect();
                app.select_names(&n, inca_core::scene::SelMode::Replace);
            }
        });
        ui.label(dim("Pick the driver attribute and the driven attributes, pose both, then Key. Repeat for each pose."));
        app.rig_tool.sdk = st;
    });
    app.win.sdk = open;
}

fn dim(s: &str) -> egui::RichText {
    egui::RichText::new(s).color(crate::theme::TEXT_DIM)
}
