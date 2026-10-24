//! Interactive Mesh Tools: Insert Edge Loop, Multi-Cut, Target Weld,
//! Sculpt brushes, Quad Draw and Make Live.

use crate::app::{IncaApp, Tool};
use crate::viewport::{self, CamView};
use egui::{vec2, Color32, Pos2, Rect, Stroke};
use glam::{DMat4, DVec3};
use inca_core::mesh::{edge_ring, PolyMesh};
use inca_core::params::Params;
use inca_core::scene::{NodeRef, SelMode};
use serde_json::json;
use std::collections::HashSet;

/// Sculpt brushes, as the JS Tool Settings list them.
pub const BRUSHES: &[&str] = &["sculpt", "smooth", "relax", "grab", "pinch", "flatten", "inflate"];

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MeshToolOpts {
    pub edge_loops: u32,
    pub brush: usize,
    pub size: f64,
    pub strength: f64,
    pub mirror: bool,
}

impl Default for MeshToolOpts {
    fn default() -> Self {
        MeshToolOpts { edge_loops: 1, brush: 0, size: 1.0, strength: 0.3, mirror: false }
    }
}

/// A Multi-Cut point: a vertex, or a parameter along an edge (with its world position for drawing).
#[derive(Clone)]
pub struct CutPt {
    vertex: Option<u32>,
    edge: Option<(u32, u32, f64)>,
    w: DVec3,
}

pub struct SculptStroke {
    id: String,
    orig: PolyMesh,
    touched: HashSet<u32>,
    brush: usize,
    sign: f64,
    grab: Option<(DVec3, Vec<(u32, f64)>)>,
}

#[derive(Default)]
pub struct MeshToolState {
    pub cut: Vec<CutPt>,
    pub cut_obj: Option<String>,
    /// Insert Edge Loop drag: mesh, history index, screen ends of the clicked edge
    loop_drag: Option<(String, usize, Pos2, Pos2)>,
    /// the ring of edges (world ends) the dragged loop slides along, and whether it closes
    loop_ring: Option<(Vec<(DVec3, DVec3)>, bool)>,
    /// Target Weld drag: mesh, source vertex
    weld: Option<(String, u32)>,
    sculpt: Option<SculptStroke>,
    /// Quad Draw dots (world) and the mesh faces go into
    pub dots: Vec<DVec3>,
    pub quad_target: Option<String>,
    quad_drag: Option<QuadPick>,
}

pub fn is_mesh_tool(t: Tool) -> bool {
    matches!(t, Tool::EdgeLoop | Tool::MultiCut | Tool::TargetWeld | Tool::Sculpt | Tool::QuadDraw)
}

fn meshes(app: &IncaApp) -> Vec<String> {
    let hil = app.s.hilite();
    if !hil.is_empty() {
        return hil;
    }
    let sel: Vec<String> = app.s.sel.iter().filter(|i| app.s.node(i).is_some_and(|n| n.mesh().is_some())).cloned().collect();
    if !sel.is_empty() {
        return sel;
    }
    app.s.all_dag(false).into_iter().filter(|i| app.s.is_visible(i) && app.s.node(i).is_some_and(|n| n.mesh().is_some())).collect()
}

fn world_verts(app: &IncaApp, id: &str) -> Vec<DVec3> {
    let wm = app.s.world_matrix(id);
    app.s.node(id).and_then(|n| n.mesh()).map(|m| m.mesh.v.iter().map(|p| wm.transform_point3(*p)).collect()).unwrap_or_default()
}

struct EdgeHit {
    id: String,
    a: u32,
    b: u32,
    t: f64,
    w: Vec<DVec3>,
    d: f32,
}

/// Nearest edge within 8 px; `t` runs from `a` to `b`.
fn pick_edge(app: &IncaApp, cv: &CamView, rect: Rect, p: Pos2, objs: &[String]) -> Option<EdgeHit> {
    let mut best: Option<EdgeHit> = None;
    for id in objs {
        let Some(pm) = app.s.node(id).and_then(|n| n.mesh()).map(|m| &m.mesh) else { continue };
        let w = world_verts(app, id);
        let scr: Vec<Option<Pos2>> = w.iter().map(|q| cv.project(*q, rect)).collect();
        for &(a, b) in &pm.topo().edges {
            let (Some(sa), Some(sb)) = (scr[a as usize], scr[b as usize]) else { continue };
            let ab = sb - sa;
            let t = ((p - sa).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
            let d = (sa + ab * t).distance(p);
            if d < 8.0 && best.as_ref().is_none_or(|x| d < x.d) {
                best = Some(EdgeHit { id: id.clone(), a, b, t: t as f64, w: w.clone(), d });
            }
        }
    }
    best
}

/// Nearest vertex within `r` px: (mesh, index, world position, distance).
fn pick_vertex(app: &IncaApp, cv: &CamView, rect: Rect, p: Pos2, objs: &[String], r: f32) -> Option<(String, u32, DVec3, f32)> {
    let mut best: Option<(String, u32, DVec3, f32)> = None;
    for id in objs {
        for (i, w) in world_verts(app, id).into_iter().enumerate() {
            if let Some(s) = cv.project(w, rect) {
                let d = s.distance(p);
                if d < r && best.as_ref().is_none_or(|b| d < b.3) {
                    best = Some((id.clone(), i as u32, w, d));
                }
            }
        }
    }
    best
}

/// Ray hit on meshes: (mesh, world point, world face normal).
fn hit_mesh(app: &IncaApp, cv: &CamView, rect: Rect, p: Pos2, objs: &[String]) -> Option<(String, DVec3, DVec3)> {
    let ray = cv.ray(p, rect);
    let mut best: Option<(f64, String, DVec3, DVec3)> = None;
    for id in objs {
        let Some(pm) = app.s.node(id).and_then(|n| n.mesh()).map(|m| &m.mesh) else { continue };
        let wm = app.s.world_matrix(id);
        let lr = ray.transformed(&wm.inverse());
        if let Some((t, f)) = pm.raycast(&lr, false) {
            let w = wm.transform_point3(lr.at(t));
            let d = (w - ray.origin).length();
            if best.as_ref().is_none_or(|b| d < b.0) {
                let n = wm.inverse().transpose().transform_vector3(pm.face_normal(f)).normalize_or_zero();
                best = Some((d, id.clone(), w, n));
            }
        }
    }
    best.map(|b| (b.1, b.2, b.3))
}

fn line(p: &egui::Painter, cv: &CamView, rect: Rect, pts: &[DVec3], closed: bool, c: Color32) {
    let mut s: Vec<Pos2> = pts.iter().filter_map(|q| cv.project(*q, rect)).collect();
    if closed && s.len() > 2 {
        s.push(s[0]);
    }
    p.add(egui::Shape::line(s, Stroke::new(1.5, c)));
}

fn dot(p: &egui::Painter, cv: &CamView, rect: Rect, w: DVec3, size: f32, c: Color32) {
    if let Some(q) = cv.project(w, rect) {
        p.rect_filled(Rect::from_center_size(q, vec2(size, size)), 1.0, c);
    }
}

const GREEN: Color32 = Color32::from_rgb(92, 255, 92);
const YELLOW: Color32 = Color32::from_rgb(255, 225, 74);
const BLUE: Color32 = Color32::from_rgb(63, 169, 255);
const RED: Color32 = Color32::from_rgb(255, 64, 64);

/// Viewport input for the active mesh tool.
pub fn interact(app: &mut IncaApp, idx: usize, resp: &egui::Response, cv: &CamView, painter: &egui::Painter) {
    let rect = app.vps[idx].rect;
    match app.tool {
        Tool::EdgeLoop => edge_loop(app, resp, cv, rect, painter),
        Tool::MultiCut => multi_cut(app, resp, cv, rect, painter),
        Tool::TargetWeld => target_weld(app, resp, cv, rect, painter),
        Tool::Sculpt => sculpt(app, resp, cv, rect, painter),
        Tool::QuadDraw => quad_draw(app, resp, cv, rect, painter),
        _ => {}
    }
}

// ---------------------------------------------------------------- insert edge loop

fn loop_ts(n: u32, t: f64) -> Vec<f64> {
    if n <= 1 {
        vec![t]
    } else {
        (0..n).map(|i| (i + 1) as f64 / (n + 1) as f64).collect()
    }
}

/// The edge ring a loop through the hit edge crosses, as world-space edge ends in a consistent
/// direction (so one t lines up across the loop), and whether the ring closes.
fn ring_of(app: &IncaApp, h: &EdgeHit) -> (Vec<(DVec3, DVec3)>, bool) {
    let pm = &app.s.node(&h.id).unwrap().mesh().unwrap().mesh;
    let topo = pm.topo();
    let ring = edge_ring(pm, &topo, h.a, h.b);
    let closed = ring.len() > 2 && {
        let (f0, f1) = (ring[0], ring[ring.len() - 1]);
        let fa = topo.edge(f0.0, f0.1).map(|e| topo.edge_faces[e].clone()).unwrap_or_default();
        let fb = topo.edge(f1.0, f1.1).map(|e| topo.edge_faces[e].clone()).unwrap_or_default();
        fa.iter().any(|f| fb.contains(f))
    };
    (ring.iter().map(|&(a, b)| (h.w[a as usize], h.w[b as usize])).collect(), closed)
}

fn draw_loops(painter: &egui::Painter, cv: &CamView, rect: Rect, ring: &(Vec<(DVec3, DVec3)>, bool), ts: &[f64]) {
    for &t in ts {
        let pts: Vec<DVec3> = ring.0.iter().map(|(a, b)| a.lerp(*b, t)).collect();
        line(painter, cv, rect, &pts, ring.1, GREEN);
    }
}

fn edge_loop(app: &mut IncaApp, resp: &egui::Response, cv: &CamView, rect: Rect, painter: &egui::Painter) {
    let n = app.mesh_opts.edge_loops.max(1);
    if let Some((id, hi, sa, sb)) = app.mesh_tool.loop_drag.clone() {
        // drag slides a single new loop along the edge; the loop stays highlighted while it moves
        if n == 1 {
            if let Some(q) = resp.interact_pointer_pos() {
                let ab = sb - sa;
                let t = ((q - sa).dot(ab) / ab.length_sq().max(1.0)).clamp(0.01, 0.99) as f64;
                app.s.set_attr(&NodeRef::History(id.clone(), hi), "t", json!(t));
            }
        }
        if let Some(ring) = &app.mesh_tool.loop_ring {
            let t = app.s.node(&id).and_then(|n| n.mesh()).and_then(|m| m.history.get(hi)).and_then(|h| h.params.get("t")).and_then(|v| v.as_f64()).unwrap_or(0.5);
            draw_loops(painter, cv, rect, ring, &loop_ts(n, t));
        }
        if resp.drag_stopped() || resp.clicked() || !resp.is_pointer_button_down_on() {
            let t = app.s.node(&id).and_then(|n| n.mesh()).and_then(|m| m.history.get(hi)).and_then(|h| h.params.get("t")).and_then(|v| v.as_f64()).unwrap_or(0.5);
            app.echo(format!("polySplitRing -ch on -splitType 1 -weight {t:.4} -smoothingAngle 30 -fixQuads 1 -insertWithEdgeFlow 0;"));
            app.mesh_tool.loop_drag = None;
            app.mesh_tool.loop_ring = None;
        }
        return;
    }
    let objs = meshes(app);
    let hover = resp.hover_pos().and_then(|p| pick_edge(app, cv, rect, p, &objs));
    if let Some(h) = &hover {
        draw_loops(painter, cv, rect, &ring_of(app, h), &loop_ts(n, h.t));
    }
    let press = resp.drag_started_by(egui::PointerButton::Primary) || resp.clicked_by(egui::PointerButton::Primary);
    // the edge under the press, not where the drag has already moved to
    let at_press = resp.interact_pointer_pos().map(|p| if resp.drag_started() { p - resp.drag_delta() } else { p }).and_then(|p| pick_edge(app, cv, rect, p, &objs));
    if let (true, Some(h)) = (press, at_press) {
        app.s.checkpoint("insert edge loop");
        app.mesh_tool.loop_ring = Some(ring_of(app, &h));
        if app.s.comp_mode.is_none() && !app.s.sel.contains(&h.id) {
            app.select_names(std::slice::from_ref(&h.id), SelMode::Replace);
        }
        let mut p = Params::new();
        p.insert("edge".into(), json!([h.a, h.b]));
        p.insert("t".into(), json!(h.t.clamp(0.01, 0.99)));
        p.insert("multiple".into(), json!(n));
        app.s.apply_op(&h.id, "polySplitRing", p, app.history_on);
        let hi = app.s.node(&h.id).and_then(|n| n.mesh()).map(|m| m.history.len().saturating_sub(1)).unwrap_or(0);
        let (sa, sb) = (cv.project(h.w[h.a as usize], rect), cv.project(h.w[h.b as usize], rect));
        if let (Some(sa), Some(sb), true) = (sa, sb, app.history_on) {
            app.mesh_tool.loop_drag = Some((h.id.clone(), hi, sa, sb));
        }
        if resp.clicked() || !app.history_on {
            app.echo(format!("polySplitRing -ch on -splitType 1 -weight {:.4} -smoothingAngle 30 -fixQuads 1 -insertWithEdgeFlow 0;", h.t));
            app.mesh_tool.loop_drag = None;
            app.mesh_tool.loop_ring = None;
        }
    }
}

// ---------------------------------------------------------------- multi-cut

fn cut_pick(app: &IncaApp, cv: &CamView, rect: Rect, p: Pos2, shift: bool) -> Option<(String, CutPt)> {
    let objs = match &app.mesh_tool.cut_obj {
        Some(o) => vec![o.clone()],
        None => meshes(app),
    };
    let r = if app.snap_point { 40.0 } else { 8.0 };
    // the cut's own points snap like vertices (click the first one again to close a loop)
    let own = app.mesh_tool.cut.iter().filter(|c| c.vertex.is_none()).filter_map(|c| cv.project(c.w, rect).map(|s| (c, s.distance(p)))).filter(|x| x.1 < r).min_by(|a, b| a.1.total_cmp(&b.1));
    let v = pick_vertex(app, cv, rect, p, &objs, r);
    if let Some((c, d)) = own {
        if v.as_ref().is_none_or(|v| d <= v.3) {
            return Some((app.mesh_tool.cut_obj.clone()?, c.clone()));
        }
    }
    if let Some((id, i, w, _)) = v {
        return Some((id, CutPt { vertex: Some(i), edge: None, w }));
    }
    let e = pick_edge(app, cv, rect, p, &objs)?;
    let mut t = e.t;
    if shift {
        t = (t * 10.0).round() / 10.0;
    }
    let t = t.clamp(0.001, 0.999);
    Some((e.id, CutPt { vertex: None, edge: Some((e.a, e.b, t)), w: e.w[e.a as usize].lerp(e.w[e.b as usize], t) }))
}

pub fn multi_cut_complete(app: &mut IncaApp) {
    let pts = std::mem::take(&mut app.mesh_tool.cut);
    let obj = app.mesh_tool.cut_obj.take();
    let Some(id) = obj.filter(|_| pts.len() >= 2) else { return };
    app.s.checkpoint("multi-cut");
    let list: Vec<serde_json::Value> = pts
        .iter()
        .map(|c| match (c.vertex, c.edge) {
            (Some(v), _) => json!({ "vertex": v }),
            (_, Some((a, b, t))) => json!({ "edge": [a, b], "t": t }),
            _ => json!({}),
        })
        .collect();
    let mut p = Params::new();
    p.insert("points".into(), serde_json::Value::Array(list));
    app.s.apply_op(&id, "polySplit", p, app.history_on);
    app.s.comp.remove(&id);
    app.echo("polySplit -ch 1 -sma 180 -ep ...;");
}

fn multi_cut(app: &mut IncaApp, resp: &egui::Response, cv: &CamView, rect: Rect, painter: &egui::Painter) {
    let mods = resp.ctx.input(|i| i.modifiers);
    let hover = resp.hover_pos().and_then(|p| cut_pick(app, cv, rect, p, mods.shift));
    let ctrl = mods.ctrl || mods.command;
    // Ctrl: the edge loop a click would insert
    if ctrl {
        let objs = meshes(app);
        if let Some(h) = resp.hover_pos().and_then(|p| pick_edge(app, cv, rect, p, &objs)) {
            draw_loops(painter, cv, rect, &ring_of(app, &h), &[h.t.clamp(0.001, 0.999)]);
        }
    }
    let mut pts: Vec<DVec3> = app.mesh_tool.cut.iter().map(|c| c.w).collect();
    if let Some((_, h)) = hover.as_ref().filter(|_| !ctrl) {
        pts.push(h.w);
    }
    line(painter, cv, rect, &pts, false, YELLOW);
    for c in &app.mesh_tool.cut {
        dot(painter, cv, rect, c.w, 6.0, BLUE);
    }
    if let Some((_, h)) = &hover {
        dot(painter, cv, rect, h.w, 8.0, RED);
    }
    if resp.clicked_by(egui::PointerButton::Secondary) {
        multi_cut_complete(app);
        return;
    }
    if !resp.clicked_by(egui::PointerButton::Primary) {
        return;
    }
    let Some((id, c)) = hover else {
        multi_cut_complete(app);
        return;
    };
    if mods.ctrl || mods.command {
        // Ctrl+click on an edge inserts an edge loop
        if let Some((a, b, t)) = c.edge {
            app.s.checkpoint("insert edge loop");
            let mut p = Params::new();
            p.insert("edge".into(), json!([a, b]));
            p.insert("t".into(), json!(t));
            p.insert("multiple".into(), json!(1));
            app.s.apply_op(&id, "polySplitRing", p, app.history_on);
        }
        return;
    }
    if app.mesh_tool.cut_obj.as_ref().is_some_and(|o| *o != id) {
        return;
    }
    app.mesh_tool.cut_obj = Some(id);
    app.mesh_tool.cut.push(c);
}

// ---------------------------------------------------------------- target weld

fn target_weld(app: &mut IncaApp, resp: &egui::Response, cv: &CamView, rect: Rect, painter: &egui::Painter) {
    let objs = meshes(app);
    if let Some((id, src)) = app.mesh_tool.weld.clone() {
        let ws = world_verts(app, &id);
        let q = resp.interact_pointer_pos().or(resp.hover_pos());
        let tgt = q.and_then(|q| pick_vertex(app, cv, rect, q, std::slice::from_ref(&id), 10.0)).filter(|t| t.1 != src);
        let end = tgt.as_ref().map(|t| t.2).unwrap_or(ws[src as usize]);
        line(painter, cv, rect, &[ws[src as usize], end], false, YELLOW);
        dot(painter, cv, rect, ws[src as usize], 7.0, BLUE);
        if let Some(t) = &tgt {
            dot(painter, cv, rect, t.2, 9.0, RED);
        }
        if resp.drag_stopped() || !resp.is_pointer_button_down_on() {
            app.mesh_tool.weld = None;
            if let Some((_, ti, _, _)) = tgt {
                let pm = app.s.node(&id).unwrap().mesh().unwrap().mesh.clone();
                let d = pm.v[ti as usize] - pm.v[src as usize];
                app.s.checkpoint("target weld");
                let mut p = Params::new();
                p.insert("verts".into(), json!([src]));
                p.insert("deltas".into(), json!([d.to_array()]));
                app.s.apply_op(&id, "polyTweak", p, app.history_on);
                let mut p = Params::new();
                p.insert("verts".into(), json!([src, ti]));
                p.insert("threshold".into(), json!(1e-4));
                app.s.apply_op(&id, "polyMergeVert", p, app.history_on);
                app.s.comp.remove(&id);
                let n = app.s.name_of(&id);
                app.echo(format!("polyMergeVertex -d 0.0001 -am 1 -ch 1 {n}.vtx[{src}] {n}.vtx[{ti}];"));
            }
        }
        return;
    }
    let hover = resp.hover_pos().and_then(|p| pick_vertex(app, cv, rect, p, &objs, 10.0));
    if let Some(h) = &hover {
        dot(painter, cv, rect, h.2, 9.0, RED);
    }
    if resp.drag_started_by(egui::PointerButton::Primary) {
        if let Some(p) = resp.interact_pointer_pos() {
            if let Some((id, i, _, _)) = pick_vertex(app, cv, rect, p - resp.drag_delta(), &objs, 10.0) {
                app.mesh_tool.weld = Some((id, i));
            }
        }
    }
}

// ---------------------------------------------------------------- sculpt

fn ring(painter: &egui::Painter, cv: &CamView, rect: Rect, c: DVec3, n: DVec3, r: f64) {
    let u = n.any_orthonormal_vector();
    let v = n.cross(u);
    let pts: Vec<DVec3> = (0..=32).map(|i| {
        let a = std::f64::consts::TAU * i as f64 / 32.0;
        c + (u * a.cos() + v * a.sin()) * r
    }).collect();
    line(painter, cv, rect, &pts, false, RED);
}

fn sculpt_targets(app: &IncaApp) -> Vec<String> {
    app.s.all_dag(false).into_iter().filter(|i| app.s.is_visible(i) && app.s.node(i).is_some_and(|n| n.mesh().is_some())).collect()
}

fn sculpt(app: &mut IncaApp, resp: &egui::Response, cv: &CamView, rect: Rect, painter: &egui::Painter) {
    let size = app.mesh_opts.size;
    let objs = sculpt_targets(app);
    let q = resp.interact_pointer_pos().or(resp.hover_pos());
    let hit = q.and_then(|q| hit_mesh(app, cv, rect, q, &objs));
    if let Some((_, w, n)) = &hit {
        ring(painter, cv, rect, *w, *n, size);
    }
    let mods = resp.ctx.input(|i| i.modifiers);
    if resp.drag_started_by(egui::PointerButton::Primary) || resp.clicked_by(egui::PointerButton::Primary) {
        if let Some((id, w, _)) = &hit {
            if let Some(m) = app.s.nodes.get_mut(id).and_then(|n| n.mesh_mut()) {
                m.smooth_level = 0;
            }
            let orig = app.s.node(id).unwrap().mesh().unwrap().mesh.clone();
            let brush = if mods.shift { 1 } else { app.mesh_opts.brush };
            let grab = (BRUSHES[brush] == "grab").then(|| (*w, vec![]));
            app.mesh_tool.sculpt = Some(SculptStroke { id: id.clone(), orig, touched: HashSet::new(), brush, sign: if mods.ctrl || mods.command { -1.0 } else { 1.0 }, grab });
        }
    }
    let Some(st) = app.mesh_tool.sculpt.as_mut() else { return };
    let id = st.id.clone();
    let wm = app.s.world_matrix(&id);
    let inv = wm.inverse();
    if BRUSHES[st.brush] == "grab" {
        if let (Some(q), Some((start, _))) = (q, st.grab.clone()) {
            // drag in the view plane through the grab point
            let ray = cv.ray(q, rect);
            let nrm = if cv.ortho { -cv.fwd } else { (cv.eye - start).normalize() };
            let denom = ray.dir.dot(nrm);
            if denom.abs() > 1e-9 {
                let t = (start - ray.origin).dot(nrm) / denom;
                let delta = ray.at(t) - start;
                stroke(app, &wm, &inv, start, nrm, Some(delta));
            }
        }
    } else if let Some((hid, w, n)) = &hit {
        if *hid == id && (resp.dragged() || resp.drag_started() || resp.clicked()) {
            stroke(app, &wm, &inv, *w, *n, None);
        }
    }
    if resp.drag_stopped() || resp.clicked() || !resp.is_pointer_button_down_on() {
        finish_sculpt(app);
    }
}

fn stroke(app: &mut IncaApp, wm: &DMat4, inv: &DMat4, center: DVec3, normal: DVec3, grab_delta: Option<DVec3>) {
    let (size, k, mirror) = (app.mesh_opts.size, app.mesh_opts.strength, app.mesh_opts.mirror);
    let Some(st) = app.mesh_tool.sculpt.as_mut() else { return };
    let Some(m) = app.s.nodes.get_mut(&st.id).and_then(|n| n.mesh_mut()) else { return };
    let pm = &mut m.mesh;
    let c = inv.transform_point3(center);
    let scale = wm.transform_vector3(DVec3::X).length().max(1e-6);
    let rl = size / scale;
    let nrm = inv.transpose().inverse().transform_vector3(normal).normalize_or_zero();
    let nrm = if nrm == DVec3::ZERO { inv.transform_vector3(normal).normalize_or(DVec3::Y) } else { nrm };
    let fall = |d: f64| {
        let t = (1.0 - d / rl).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let brush = BRUSHES[st.brush];
    let ids: Vec<(u32, f64)> = pm.v.iter().enumerate().filter_map(|(i, p)| {
        let d = (*p - c).length();
        (d < rl).then_some((i as u32, d))
    }).collect();
    if brush == "grab" {
        let Some((_, set)) = st.grab.as_mut() else { return };
        if set.is_empty() {
            let c0 = inv.transform_point3(center);
            *set = st.orig.v.iter().enumerate().filter_map(|(i, p)| {
                let d = (*p - c0).length();
                (d < rl).then(|| (i as u32, fall(d)))
            }).collect();
        }
        let ld = inv.transform_vector3(grab_delta.unwrap_or_default());
        for &(i, w) in set.iter() {
            pm.v[i as usize] = st.orig.v[i as usize] + ld * w;
            st.touched.insert(i);
        }
    } else {
        let topo = pm.topo();
        let nbr = |i: u32| -> Vec<u32> {
            topo.vert_edges[i as usize].iter().map(|&e| {
                let (a, b) = topo.edges[e];
                if a == i { b } else { a }
            }).collect()
        };
        let vnormal = |pm: &PolyMesh, i: u32| -> DVec3 {
            topo.vert_faces[i as usize].iter().map(|&f| pm.face_normal(f)).sum::<DVec3>().normalize_or_zero()
        };
        let plane = if brush == "flatten" && !ids.is_empty() { ids.iter().map(|(i, _)| pm.v[*i as usize]).sum::<DVec3>() / ids.len() as f64 } else { DVec3::ZERO };
        let snapshot = pm.v.clone();
        for &(i, d) in &ids {
            let w = fall(d) * k;
            let v = snapshot[i as usize];
            let nv = match brush {
                "sculpt" => v + nrm * (st.sign * w * rl * 0.15),
                "inflate" => v + vnormal(pm, i) * (st.sign * w * rl * 0.12),
                "smooth" | "relax" => {
                    let ns = nbr(i);
                    if ns.is_empty() {
                        continue;
                    }
                    let mut avg = ns.iter().map(|&j| snapshot[j as usize]).sum::<DVec3>() / ns.len() as f64;
                    if brush == "relax" {
                        let n = vnormal(pm, i);
                        avg -= n * (avg - v).dot(n);
                    }
                    v.lerp(avg, (w * 2.0).min(1.0))
                }
                "pinch" => v.lerp(c, st.sign * w * 0.1),
                "flatten" => v - nrm * ((v - plane).dot(nrm) * w),
                _ => v,
            };
            pm.v[i as usize] = nv;
            st.touched.insert(i);
        }
    }
    if mirror {
        // mirror across local X: the vertex at (-x, y, z) in the original mesh follows
        let touched: Vec<u32> = st.touched.iter().copied().collect();
        for i in touched {
            let o = st.orig.v[i as usize];
            let target = DVec3::new(-o.x, o.y, o.z);
            if let Some((j, _)) = st.orig.v.iter().enumerate().map(|(j, q)| (j, (*q - target).length())).filter(|x| x.1 < 1e-3).min_by(|a, b| a.1.total_cmp(&b.1)) {
                if j as u32 != i {
                    let p = pm.v[i as usize];
                    pm.v[j] = DVec3::new(-p.x, p.y, p.z);
                    st.touched.insert(j as u32);
                }
            }
        }
    }
    if let Some(n) = app.s.nodes.get_mut(&st.id) {
        n.rev += 1;
    }
    app.s.rev += 1;
}

/// Commit the stroke as one polyTweak (one undo step), like a component move.
fn finish_sculpt(app: &mut IncaApp) {
    let Some(st) = app.mesh_tool.sculpt.take() else { return };
    let cur = app.s.node(&st.id).and_then(|n| n.mesh()).map(|m| m.mesh.clone()).unwrap_or_default();
    let mut verts: Vec<u32> = st.touched.iter().copied().collect();
    verts.sort();
    let deltas: Vec<[f64; 3]> = verts.iter().map(|&v| (cur.v[v as usize] - st.orig.v[v as usize]).to_array()).collect();
    if let Some(m) = app.s.nodes.get_mut(&st.id).and_then(|n| n.mesh_mut()) {
        m.mesh = st.orig.clone();
    }
    if verts.is_empty() {
        return;
    }
    app.s.checkpoint("sculpt");
    let mut p = Params::new();
    p.insert("verts".into(), json!(verts));
    p.insert("deltas".into(), json!(deltas));
    app.s.apply_op(&st.id, "polyTweak", p, true);
    app.echo(format!("// sculpt ({}) {} vertices", BRUSHES[st.brush], verts.len()));
}

// ---------------------------------------------------------------- quad draw

#[derive(Clone, Copy, PartialEq)]
enum QuadPick {
    Dot(usize),
    Vert(u32),
}

fn quad_surfaces(app: &IncaApp) -> Vec<String> {
    if let Some(l) = app.live_surface.as_ref().filter(|l| app.s.node(l).is_some()) {
        return vec![l.clone()];
    }
    let t = app.mesh_tool.quad_target.clone();
    sculpt_targets(app).into_iter().filter(|i| Some(i) != t.as_ref()).collect()
}

/// A point on the live surface, nudged off it so new faces don't z-fight.
fn surface_point(app: &IncaApp, cv: &CamView, rect: Rect, p: Pos2) -> Option<DVec3> {
    let (id, w, n) = hit_mesh(app, cv, rect, p, &quad_surfaces(app))?;
    let off = app.s.world_bbox(&id).size().length() * 0.003;
    Some(w + n * off)
}

fn quad_candidates(app: &IncaApp) -> Vec<(QuadPick, DVec3)> {
    let mut out: Vec<(QuadPick, DVec3)> = app.mesh_tool.dots.iter().enumerate().map(|(i, w)| (QuadPick::Dot(i), *w)).collect();
    if let Some(t) = &app.mesh_tool.quad_target {
        out.extend(world_verts(app, t).into_iter().enumerate().map(|(i, w)| (QuadPick::Vert(i as u32), w)));
    }
    out
}

fn quad_nearest(app: &IncaApp, cv: &CamView, rect: Rect, p: Pos2) -> Option<QuadPick> {
    quad_candidates(app).into_iter().filter_map(|(k, w)| cv.project(w, rect).map(|s| (k, s.distance(p)))).filter(|x| x.1 < 10.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|x| x.0)
}

fn cross2(o: Pos2, a: Pos2, b: Pos2) -> f32 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

/// The smallest convex quad (or triangle) of nearby dots / target vertices around the cursor.
fn quad_region(app: &IncaApp, cv: &CamView, rect: Rect, p: Pos2) -> Option<Vec<(QuadPick, DVec3)>> {
    let mut cs: Vec<(QuadPick, DVec3, Pos2)> = quad_candidates(app).into_iter().filter_map(|(k, w)| cv.project(w, rect).map(|s| (k, w, s))).collect();
    cs.sort_by(|a, b| a.2.distance(p).total_cmp(&b.2.distance(p)));
    cs.truncate(8);
    if cs.len() < 3 {
        return None;
    }
    let mut best: Option<(f32, Vec<usize>)> = None;
    let mut consider = |set: Vec<usize>| {
        let c = set.iter().fold(vec2(0.0, 0.0), |a, &i| a + cs[i].2.to_vec2()) / set.len() as f32;
        let mut ord = set.clone();
        ord.sort_by(|&a, &b| (cs[a].2.y - c.y).atan2(cs[a].2.x - c.x).total_cmp(&(cs[b].2.y - c.y).atan2(cs[b].2.x - c.x)));
        let poly: Vec<Pos2> = ord.iter().map(|&i| cs[i].2).collect();
        let n = poly.len();
        let mut sgn = 0.0f32;
        for i in 0..n {
            let cr = cross2(poly[i], poly[(i + 1) % n], poly[(i + 2) % n]);
            if cr.abs() < 1e-3 || (sgn != 0.0 && cr.signum() != sgn) {
                return;
            }
            sgn = cr.signum();
        }
        if !viewport::in_polygon(&poly, p) {
            return;
        }
        if (0..cs.len()).any(|i| !set.contains(&i) && viewport::in_polygon(&poly, cs[i].2)) {
            return;
        }
        let area: f32 = (0..n).map(|i| cross2(poly[0], poly[i], poly[(i + 1) % n])).sum::<f32>().abs();
        // prefer quads
        let score = area * if n == 4 { 0.5 } else { 1.0 };
        if best.as_ref().is_none_or(|b| score < b.0) {
            best = Some((score, ord));
        }
    };
    let m = cs.len();
    for a in 0..m {
        for b in a + 1..m {
            for c in b + 1..m {
                consider(vec![a, b, c]);
                for d in c + 1..m {
                    consider(vec![a, b, c, d]);
                }
            }
        }
    }
    best.map(|(_, ord)| ord.into_iter().map(|i| (cs[i].0, cs[i].1)).collect())
}

/// Add a face to the Quad Draw target mesh (created on first use), consuming dots.
fn quad_add_face(app: &mut IncaApp, region: Vec<(QuadPick, DVec3)>, cv: &CamView) {
    app.s.checkpoint("quad draw");
    let tid = match app.mesh_tool.quad_target.clone().filter(|t| app.s.node(t).is_some()) {
        Some(t) => t,
        None => {
            let id = app.s.create_mesh_from(PolyMesh::new(vec![], vec![]), "polySurface1", None, None);
            app.mesh_tool.quad_target = Some(id.clone());
            id
        }
    };
    let inv = app.s.world_matrix(&tid).inverse();
    let mut pm = app.s.node(&tid).unwrap().mesh().unwrap().mesh.clone();
    let mut used_dots = vec![];
    let mut face: Vec<u32> = vec![];
    for (k, w) in &region {
        match k {
            QuadPick::Vert(i) => face.push(*i),
            QuadPick::Dot(d) => {
                pm.v.push(inv.transform_point3(*w));
                face.push((pm.v.len() - 1) as u32);
                used_dots.push(*d);
            }
        }
    }
    // face the camera
    let pts: Vec<DVec3> = region.iter().map(|r| r.1).collect();
    let n = inca_core::mesh::newell(&pts);
    let toward = if cv.ortho { -cv.fwd } else { cv.eye - pts[0] };
    if n.dot(toward) < 0.0 {
        face.reverse();
    }
    pm.add_face(face, None, None);
    used_dots.sort();
    for d in used_dots.into_iter().rev() {
        app.mesh_tool.dots.remove(d);
    }
    app.s.bake_mesh(&tid, pm);
}

fn quad_draw(app: &mut IncaApp, resp: &egui::Response, cv: &CamView, rect: Rect, painter: &egui::Painter) {
    let mods = resp.ctx.input(|i| i.modifiers);
    // the live surface defaults to the selected mesh
    if app.live_surface.is_none() {
        if let Some(l) = app.s.lead().cloned().filter(|l| app.s.node(l).is_some_and(|n| n.mesh().is_some_and(|m| !m.mesh.f.is_empty())) && Some(l) != app.mesh_tool.quad_target.as_ref()) {
            app.live_surface = Some(l.clone());
            app.echo(format!("makeLive {};", app.s.name_of(&l)));
        }
    }
    // drawing: dots blue, target vertices yellow, target wire green
    if let Some(t) = &app.mesh_tool.quad_target {
        if let Some(pm) = app.s.node(t).and_then(|n| n.mesh()).map(|m| m.mesh.clone()) {
            let w = world_verts(app, t);
            for f in &pm.f {
                let pts: Vec<DVec3> = f.iter().map(|&i| w[i as usize]).collect();
                line(painter, cv, rect, &pts, true, GREEN);
            }
            for p in &w {
                dot(painter, cv, rect, *p, 6.0, YELLOW);
            }
        }
    }
    for d in &app.mesh_tool.dots {
        dot(painter, cv, rect, *d, 8.0, BLUE);
    }
    let hover = resp.hover_pos();
    if let Some(h) = hover.and_then(|p| quad_nearest(app, cv, rect, p)) {
        if let Some((_, w)) = quad_candidates(app).into_iter().find(|c| c.0 == h) {
            dot(painter, cv, rect, w, 11.0, RED);
        }
    }
    let region = (mods.shift && !mods.ctrl).then(|| hover.and_then(|p| quad_region(app, cv, rect, p))).flatten();
    if let Some(r) = &region {
        let pts: Vec<Pos2> = r.iter().filter_map(|x| cv.project(x.1, rect)).collect();
        if pts.len() >= 3 {
            painter.add(egui::Shape::convex_polygon(pts.clone(), Color32::from_rgba_unmultiplied(92, 255, 92, 70), Stroke::new(1.5, GREEN)));
        }
    }
    // drag a dot / vertex along the surface
    if let Some(k) = app.mesh_tool.quad_drag {
        if let Some(w) = resp.interact_pointer_pos().and_then(|q| surface_point(app, cv, rect, q)) {
            match k {
                QuadPick::Dot(i) => app.mesh_tool.dots[i] = w,
                QuadPick::Vert(i) => {
                    if let Some(t) = app.mesh_tool.quad_target.clone() {
                        let inv = app.s.world_matrix(&t).inverse();
                        if let Some(m) = app.s.nodes.get_mut(&t).and_then(|n| n.mesh_mut()) {
                            m.mesh.v[i as usize] = inv.transform_point3(w);
                        }
                        if let Some(n) = app.s.nodes.get_mut(&t) {
                            n.rev += 1;
                        }
                        app.s.rev += 1;
                    }
                }
            }
        }
        if resp.drag_stopped() || !resp.is_pointer_button_down_on() {
            if let (QuadPick::Vert(_), Some(t)) = (k, app.mesh_tool.quad_target.clone()) {
                let pm = app.s.node(&t).unwrap().mesh().unwrap().mesh.clone();
                app.s.bake_mesh(&t, pm);
            }
            app.mesh_tool.quad_drag = None;
        }
        return;
    }
    if resp.drag_started_by(egui::PointerButton::Primary) {
        if let Some(p) = resp.interact_pointer_pos() {
            if let Some(k) = quad_nearest(app, cv, rect, p - resp.drag_delta()) {
                if let QuadPick::Vert(_) = k {
                    app.s.checkpoint("quad draw: move vertex");
                }
                app.mesh_tool.quad_drag = Some(k);
            }
        }
        return;
    }
    if !resp.clicked_by(egui::PointerButton::Primary) {
        return;
    }
    let Some(p) = resp.interact_pointer_pos() else { return };
    if mods.shift && (mods.ctrl || mods.command) {
        // delete the dot under the cursor
        if let Some(QuadPick::Dot(i)) = quad_nearest(app, cv, rect, p) {
            app.mesh_tool.dots.remove(i);
        }
        return;
    }
    if mods.shift {
        if let Some(r) = region {
            quad_add_face(app, r, cv);
        }
        return;
    }
    if let Some(w) = surface_point(app, cv, rect, p) {
        app.mesh_tool.dots.push(w);
    }
}

/// Enter in Quad Draw: select the result.
pub fn quad_complete(app: &mut IncaApp) {
    app.mesh_tool.dots.clear();
    if let Some(t) = app.mesh_tool.quad_target.take().filter(|t| app.s.node(t).is_some()) {
        app.select_names(&[t], SelMode::Replace);
    }
}

/// Modify > Make Live: toggle the lead mesh as the live surface.
pub fn make_live(app: &mut IncaApp) {
    let lead = app.s.lead().cloned().filter(|l| app.s.node(l).is_some_and(|n| n.mesh().is_some()));
    app.live_surface = if app.live_surface.is_some() && app.live_surface == lead { None } else { lead };
    app.help = match &app.live_surface {
        Some(l) => format!("Live surface: {}", app.s.name_of(l)),
        None => "No live surface".into(),
    };
}

/// Point on the live surface under the cursor (creation tools draw on it).
pub fn live_point(app: &IncaApp, cv: &CamView, rect: Rect, p: Pos2) -> Option<DVec3> {
    let l = app.live_surface.as_ref().filter(|l| app.s.node(l).is_some())?;
    hit_mesh(app, cv, rect, p, std::slice::from_ref(l)).map(|h| h.1)
}

/// Tool Settings rows for the mesh tools.
pub fn settings(app: &mut IncaApp, ui: &mut egui::Ui) {
    let o = &mut app.mesh_opts;
    match app.tool {
        Tool::EdgeLoop => {
            ui.horizontal(|ui| {
                ui.label("Number of edge loops");
                ui.add(egui::Slider::new(&mut o.edge_loops, 1..=20));
            });
        }
        Tool::Sculpt => {
            ui.horizontal(|ui| {
                ui.label("Brush");
                egui::ComboBox::from_id_salt("brush").selected_text(cap(BRUSHES[o.brush])).show_ui(ui, |ui| {
                    for (i, b) in BRUSHES.iter().enumerate() {
                        ui.selectable_value(&mut o.brush, i, cap(b));
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label("Size (world)");
                ui.add(egui::Slider::new(&mut o.size, 0.05..=10.0).logarithmic(true));
            });
            ui.horizontal(|ui| {
                ui.label("Strength");
                ui.add(egui::Slider::new(&mut o.strength, 0.0..=1.0));
            });
            ui.checkbox(&mut o.mirror, "Mirror X");
            ui.label(egui::RichText::new("Ctrl inverts, Shift smooths").color(crate::theme::TEXT_DIM));
        }
        Tool::MultiCut => {
            ui.label("Click edges / vertices to cut, Enter or RMB completes, Backspace removes the last point. Shift snaps to 10% steps, Ctrl+click inserts an edge loop.");
        }
        Tool::QuadDraw => {
            ui.label("Click to place dots on the live surface, Shift+click fills the quad around the cursor, drag a dot or vertex to slide it, Ctrl+Shift+click deletes a dot. Enter selects the result.");
            if ui.button("Clear dots").clicked() {
                app.mesh_tool.dots.clear();
            }
        }
        _ => {}
    }
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}
