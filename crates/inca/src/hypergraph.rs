//! Hypergraph (src/js/ui/hypergraph.js): the DAG as a top-down tree, or the selected objects'
//! input and output connections (transform → history → shape → material); and the Connection
//! Editor (src/js/ui/connectionEditor.js) for direct attribute connections.

use crate::app::IncaApp;
use crate::theme;
use egui::{pos2, vec2, Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use inca_core::scene::{NodeRef, SelMode, Shape};
use std::collections::HashMap;

const NW: f32 = 112.0;
const NH: f32 = 46.0;
const GX: f32 = 16.0;
const GY: f32 = 34.0;

pub struct HgState {
    pub connections: bool,
    /// objects graphed in the connections view
    pub io: Vec<String>,
    pan: Vec2,
    zoom: f32,
    show_cameras: bool,
    drag: Option<String>,
    // Connection Editor
    pub ce_left: Option<NodeRef>,
    pub ce_right: Option<NodeRef>,
    ce_attr: Option<String>,
}

impl Default for HgState {
    fn default() -> Self {
        HgState { connections: false, io: vec![], pan: vec2(30.0, 30.0), zoom: 1.0, show_cameras: false, drag: None, ce_left: None, ce_right: None, ce_attr: None }
    }
}

/// A box on the graph: what it stands for, its label lines and where it is (graph space).
struct Box_ {
    key: String,
    obj: Option<String>,
    label: String,
    sub: String,
    at: Pos2,
    material: bool,
}

fn kind_label(app: &IncaApp, id: &str) -> String {
    let Some(n) = app.s.node(id) else { return String::new() };
    match &n.shape {
        Shape::Mesh(_) => "mesh".into(),
        Shape::Light(l) => l.ty.clone(),
        Shape::Camera(_) => "camera".into(),
        Shape::Curve(_) => "nurbsCurve".into(),
        Shape::Group => "transform".into(),
        _ => n.kind.clone(),
    }
}

/// Top-down tidy tree: roots side by side, children centred under their parent.
fn layout_tree(app: &IncaApp, show_cameras: bool) -> Vec<Box_> {
    let kids = |id: Option<&str>| -> Vec<String> {
        app.s.children(id).iter().filter(|c| app.s.node(c).is_some_and(|n| show_cameras || !matches!(&n.shape, Shape::Camera(c) if c.startup))).cloned().collect()
    };
    fn width(id: &str, kids: &dyn Fn(Option<&str>) -> Vec<String>, memo: &mut HashMap<String, f32>, depth: usize) -> f32 {
        let ks = if depth > 256 { vec![] } else { kids(Some(id)) };
        let sum: f32 = ks.iter().enumerate().map(|(i, k)| width(k, kids, memo, depth + 1) + if i > 0 { GX } else { 0.0 }).sum();
        let w = NW.max(sum);
        memo.insert(id.to_string(), w);
        w
    }
    fn place(app: &IncaApp, id: &str, left: f32, depth: usize, kids: &dyn Fn(Option<&str>) -> Vec<String>, memo: &HashMap<String, f32>, out: &mut Vec<Box_>) {
        let w = memo[id];
        out.push(Box_ { key: id.to_string(), obj: Some(id.to_string()), label: app.s.name_of(id), sub: kind_label(app, id), at: pos2(left + (w - NW) / 2.0, depth as f32 * (NH + GY)), material: false });
        if depth > 256 {
            return;
        }
        let ks = kids(Some(id));
        let sum: f32 = ks.iter().enumerate().map(|(i, k)| memo[k] + if i > 0 { GX } else { 0.0 }).sum();
        let mut x = left + (w - sum) / 2.0;
        for k in ks {
            place(app, &k, x, depth + 1, kids, memo, out);
            x += memo[&k] + GX;
        }
    }
    let mut memo = HashMap::new();
    let mut out = vec![];
    let mut x = 0.0;
    for (i, r) in kids(None).iter().enumerate() {
        if i > 0 {
            x += GX * 2.0;
        }
        let w = width(r, &kids, &mut memo, 0);
        place(app, r, x, 0, &kids, &memo, &mut out);
        x += w;
    }
    out
}

/// One row per graphed object: transform, history (oldest first), shape; shared materials once.
fn layout_connections(app: &IncaApp, io: &[String]) -> (Vec<Box_>, Vec<(String, String)>) {
    let mut objs: Vec<String> = io.iter().filter(|i| app.s.node(i).is_some()).cloned().collect();
    // objects wired to the graphed ones by direct connections get rows too
    for c in &app.s.conns {
        let owner = |id: &str| app.s.ref_by_id(id).map(|r| match r {
            NodeRef::Dag(i) | NodeRef::History(i, _) => i,
            NodeRef::Material(i) | NodeRef::Texture(i) => i,
        });
        if let (Some(a), Some(b)) = (owner(&c.src), owner(&c.dst)) {
            if app.s.node(&a).is_some() && app.s.node(&b).is_some() {
                if objs.contains(&a) && !objs.contains(&b) {
                    objs.push(b);
                } else if objs.contains(&b) && !objs.contains(&a) {
                    objs.push(a);
                }
            }
        }
    }
    let (col, rowh) = (NW + 50.0, NH + 40.0);
    let max_hist = objs.iter().map(|o| app.s.node(o).and_then(|n| n.mesh()).map(|m| m.history.len()).unwrap_or(0)).max().unwrap_or(0);
    let shape_x = (max_hist + 1) as f32 * col;
    let mut boxes = vec![];
    let mut links = vec![];
    let mut mat_users: Vec<(String, Vec<f32>, Vec<String>)> = vec![];
    for (r, o) in objs.iter().enumerate() {
        let y = r as f32 * rowh;
        let n = app.s.node(o).unwrap();
        boxes.push(Box_ { key: format!("xf:{o}"), obj: Some(o.clone()), label: n.name.clone(), sub: "transform".into(), at: pos2(0.0, y), material: false });
        let hist = n.mesh().map(|m| m.history.clone()).unwrap_or_default();
        let mut prev: Option<String> = None;
        for (i, h) in hist.iter().enumerate() {
            let k = h.id.clone();
            boxes.push(Box_ { key: k.clone(), obj: Some(o.clone()), label: h.name.clone(), sub: h.ty.clone(), at: pos2((max_hist - hist.len() + i + 1) as f32 * col, y), material: false });
            if let Some(p) = prev {
                links.push((p, k.clone()));
            }
            prev = Some(k);
        }
        let sk = format!("sh:{o}");
        boxes.push(Box_ { key: sk.clone(), obj: Some(o.clone()), label: n.shape_name.clone().unwrap_or(format!("{}Shape", n.name)), sub: format!("{} shape", kind_label(app, o)), at: pos2(shape_x, y), material: false });
        if let Some(p) = prev {
            links.push((p, sk.clone()));
        }
        if let Some(m) = n.mesh() {
            match mat_users.iter_mut().find(|u| u.0 == m.material) {
                Some(u) => {
                    u.1.push(y);
                    u.2.push(sk);
                }
                None => mat_users.push((m.material.clone(), vec![y], vec![sk])),
            }
        }
    }
    let mut min_y = f32::NEG_INFINITY;
    for (mid, ys, shapes) in mat_users {
        let Some(m) = app.s.mats.get(&mid) else { continue };
        let y = (ys.iter().sum::<f32>() / ys.len() as f32).max(min_y);
        min_y = y + NH + 20.0;
        boxes.push(Box_ { key: mid.clone(), obj: None, label: m.name.clone(), sub: m.ty.clone(), at: pos2(shape_x + col, y), material: true });
        for s in shapes {
            links.push((s, mid.clone()));
        }
    }
    (boxes, links)
}

fn arrow(p: &egui::Painter, a: Pos2, b: Pos2, col: Color32) {
    let mx = (a.x + b.x) / 2.0;
    let pts: Vec<Pos2> = (0..=16).map(|i| {
        let t = i as f32 / 16.0;
        let u = 1.0 - t;
        let (c1, c2) = (pos2(mx, a.y), pos2(mx, b.y));
        pos2(u * u * u * a.x + 3.0 * u * u * t * c1.x + 3.0 * u * t * t * c2.x + t * t * t * b.x, u * u * u * a.y + 3.0 * u * u * t * c1.y + 3.0 * u * t * t * c2.y + t * t * t * b.y)
    }).collect();
    p.add(egui::Shape::line(pts, Stroke::new(1.6, col)));
    p.add(egui::Shape::convex_polygon(vec![b, b + vec2(-7.0, -4.0), b + vec2(-7.0, 4.0)], col, Stroke::NONE));
}

pub fn window(app: &mut IncaApp, ctx: &egui::Context) {
    let mut open = app.win.hypergraph;
    if !open {
        return;
    }
    let title = if app.hg.connections { "Hypergraph InputOutput" } else { "Hypergraph Hierarchy" };
    egui::Window::new(title).id(egui::Id::new("hypergraph")).open(&mut open).default_size([760.0, 460.0]).show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.selectable_label(!app.hg.connections, "Scene Hierarchy").clicked() {
                app.hg.connections = false;
            }
            if ui.selectable_label(app.hg.connections, "Input and Output Connections").on_hover_text("Graph the selected objects").clicked() {
                app.hg.connections = true;
                app.hg.io = app.s.sel.clone();
            }
            ui.separator();
            ui.checkbox(&mut app.hg.show_cameras, "Cameras");
            if ui.button("Frame All").clicked() {
                app.hg.pan = vec2(30.0, 30.0);
                app.hg.zoom = 1.0;
            }
        });
        graph(app, ui);
    });
    app.win.hypergraph = open;
}

fn graph(app: &mut IncaApp, ui: &mut egui::Ui) {
    let (rect, resp) = ui.allocate_exact_size(ui.available_size() - vec2(0.0, 18.0), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(0x6b, 0x6b, 0x6b));
    let (boxes, links) = if app.hg.connections {
        if app.hg.io.is_empty() && !app.s.sel.is_empty() {
            app.hg.io = app.s.sel.clone();
        }
        layout_connections(app, &app.hg.io.clone())
    } else {
        (layout_tree(app, app.hg.show_cameras), vec![])
    };
    let (pan, zoom) = (app.hg.pan, app.hg.zoom);
    let to_scr = |p: Pos2| rect.min + pan + p.to_vec2() * zoom;
    let rect_of = |b: &Box_| Rect::from_min_size(to_scr(b.at), vec2(NW, NH) * zoom);
    let idx: HashMap<&str, usize> = boxes.iter().enumerate().map(|(i, b)| (b.key.as_str(), i)).collect();
    // edges: parent → child, or the connection rows
    if app.hg.connections {
        for (a, b) in &links {
            if let (Some(&i), Some(&j)) = (idx.get(a.as_str()), idx.get(b.as_str())) {
                let (ra, rb) = (rect_of(&boxes[i]), rect_of(&boxes[j]));
                arrow(&painter, pos2(ra.max.x, ra.center().y), pos2(rb.min.x - 1.0, rb.center().y), Color32::from_rgb(0xd0, 0x40, 0xd0));
            }
        }
        // direct connections (Connection Editor)
        for c in &app.s.conns {
            let find = |id: &str| boxes.iter().position(|b| b.key == id || b.key == format!("xf:{id}"));
            if let (Some(i), Some(j)) = (find(&c.src), find(&c.dst)) {
                let (ra, rb) = (rect_of(&boxes[i]), rect_of(&boxes[j]));
                let col = if c.src_attr.ends_with('X') || c.src_attr.ends_with('Y') || c.src_attr.ends_with('Z') { Color32::from_rgb(0x28, 0x48, 0xd8) } else { Color32::from_rgb(0x3c, 0xcf, 0x3c) };
                if (ra.min.x - rb.min.x).abs() < NW * zoom {
                    // same column: loop out to the left of both nodes
                    let (a, b) = (pos2(ra.min.x, ra.center().y + 6.0), pos2(rb.min.x - 1.0, rb.center().y + 6.0));
                    let lx = ra.min.x.min(rb.min.x) - 45.0 * zoom;
                    let pts: Vec<Pos2> = (0..=16).map(|k| {
                        let t = k as f32 / 16.0;
                        let u = 1.0 - t;
                        pos2(u * u * u * a.x + 3.0 * u * t * lx + t * t * t * b.x, u * u * u * a.y + 3.0 * u * u * t * a.y + 3.0 * u * t * t * b.y + t * t * t * b.y)
                    }).collect();
                    painter.add(egui::Shape::line(pts, Stroke::new(1.6, col)));
                    painter.add(egui::Shape::convex_polygon(vec![b, b + vec2(-7.0, -4.0), b + vec2(-7.0, 4.0)], col, Stroke::NONE));
                } else {
                    arrow(&painter, pos2(ra.max.x, ra.center().y + 6.0), pos2(rb.min.x - 1.0, rb.center().y + 6.0), col);
                }
            }
        }
    } else {
        for b in &boxes {
            let Some(p) = b.obj.as_ref().and_then(|o| app.s.node(o)).and_then(|n| n.parent.clone()) else { continue };
            if let Some(&pi) = idx.get(p.as_str()) {
                let (ra, rb) = (rect_of(&boxes[pi]), rect_of(b));
                painter.line_segment([pos2(ra.center().x, ra.max.y), pos2(rb.center().x, rb.min.y)], Stroke::new(1.2, Color32::from_gray(40)));
            }
        }
    }
    // nodes
    let font = egui::FontId::proportional(11.0 * zoom.clamp(0.6, 1.4));
    let small = egui::FontId::proportional(9.5 * zoom.clamp(0.6, 1.4));
    let lead = app.s.lead().cloned();
    let pointer = ui.ctx().input(|i| i.pointer.hover_pos());
    let mut hovered: Option<usize> = None;
    for (i, b) in boxes.iter().enumerate() {
        let r = rect_of(b);
        if !rect.intersects(r) {
            continue;
        }
        let sel = !app.hg.connections && b.obj.as_ref().is_some_and(|o| app.s.sel.contains(o));
        let is_lead = sel && b.obj.as_ref() == lead.as_ref();
        let hidden = b.obj.as_ref().is_some_and(|o| !app.s.is_visible(o));
        let fill = if is_lead { Color32::from_rgb(0xc8, 0xc8, 0x00) } else if sel { Color32::from_rgb(0xb3, 0xb3, 0x00) } else if b.material { Color32::from_rgb(0x4e, 0x5e, 0x70) } else { Color32::from_gray(0x5a) };
        painter.rect_filled(r, 3.0, if hidden { fill.gamma_multiply(0.55) } else { fill });
        painter.rect_stroke(r, 3.0, Stroke::new(1.0, Color32::from_gray(0x2a)), egui::StrokeKind::Inside);
        if app.hg.drag.is_some() && pointer.is_some_and(|q| r.contains(q)) && b.obj != app.hg.drag {
            painter.rect_stroke(r, 3.0, Stroke::new(2.0, Color32::from_rgb(0xe6, 0xc5, 0x4a)), egui::StrokeKind::Outside);
        }
        let tc = if sel { Color32::from_gray(0x11) } else { Color32::from_gray(0xe8) };
        painter.text(r.center() - vec2(0.0, 6.0 * zoom), egui::Align2::CENTER_CENTER, &b.label, font.clone(), tc);
        painter.text(r.center() + vec2(0.0, 9.0 * zoom), egui::Align2::CENTER_CENTER, &b.sub, small.clone(), if sel { Color32::from_gray(0x33) } else { Color32::from_gray(0xbb) });
        if pointer.is_some_and(|q| r.contains(q)) {
            hovered = Some(i);
        }
    }
    // interaction: click selects, MMB-drag a node onto another parents it (empty space: unparent),
    // drag empty space (or Alt+MMB) pans, wheel zooms
    let press = ui.ctx().input(|i| i.pointer.press_origin());
    if resp.drag_started_by(egui::PointerButton::Middle) && !app.hg.connections {
        app.hg.drag = press.and_then(|p| boxes.iter().rposition(|b| rect_of(b).contains(p))).and_then(|i| boxes[i].obj.clone());
    }
    if resp.dragged() && app.hg.drag.is_none() {
        app.hg.pan += resp.drag_delta();
    }
    if let (Some(d), Some(q)) = (app.hg.drag.clone(), pointer) {
        painter.text(q + vec2(10.0, 0.0), egui::Align2::LEFT_CENTER, app.s.name_of(&d), font.clone(), Color32::WHITE);
    }
    if resp.drag_stopped() {
        if let Some(d) = app.hg.drag.take() {
            let target = hovered.and_then(|i| boxes[i].obj.clone()).filter(|t| *t != d);
            let mel = match target {
                Some(t) => format!("parent {} {};", app.s.name_of(&d), app.s.name_of(&t)),
                None if app.s.node(&d).and_then(|n| n.parent.clone()).is_some() => format!("parent -w {};", app.s.name_of(&d)),
                None => String::new(),
            };
            if !mel.is_empty() {
                let _ = app.run_mel("parent", &mel);
            }
        }
    }
    if resp.clicked() {
        let shift = ui.input(|i| i.modifiers.shift);
        match hovered.and_then(|i| boxes[i].obj.clone().map(|o| (i, o))) {
            Some((i, o)) => {
                app.select_names(&[app.s.name_of(&o)], if shift { SelMode::Toggle } else { SelMode::Replace });
                if app.hg.connections && !boxes[i].key.starts_with("xf:") {
                    // history / shape nodes: show them in the Attribute Editor
                    app.show_right = true;
                    app.right_tab = crate::app::RightTab::AttributeEditor;
                }
            }
            None => {
                if let Some(i) = hovered.filter(|i| boxes[*i].material) {
                    app.windows_state.hs_selected = Some(boxes[i].key.clone());
                    app.win.hypershade = true;
                } else if !shift {
                    app.select_names(&[], SelMode::Replace);
                }
            }
        }
    }
    if let Some(q) = pointer.filter(|q| rect.contains(*q)) {
        let scroll = ui.ctx().input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let nz = (zoom * (1.0 + scroll * 0.002)).clamp(0.2, 3.0);
            let g = (q - rect.min - pan) / zoom;
            app.hg.zoom = nz;
            app.hg.pan = q - rect.min - g * nz;
        }
    }
    let hint = if app.hg.connections { "click a node to select it · drag to pan · wheel zoom" } else { "MMB-drag a node onto another to parent (empty space: unparent) · drag to pan · wheel zoom" };
    ui.label(RichText::new(format!("{} selected   |   {hint}", app.s.sel.len())).color(theme::TEXT_DIM).size(10.5));
}

// ---------------------------------------------------------------- Connection Editor

/// Attributes listed for a node.
fn attrs_of(app: &IncaApp, r: &NodeRef) -> Vec<String> {
    match r {
        NodeRef::Dag(id) => {
            let mut v: Vec<String> = inca_core::scene::CHANNELS.iter().map(|c| c.to_string()).collect();
            if let Some(e) = app.s.node(id).and_then(|n| n.extra.as_object()) {
                v.extend(e.iter().filter(|(_, x)| x.is_number() || x.is_boolean()).map(|(k, _)| k.clone()));
            }
            v.extend(app.s.shape_channels(id).into_iter().filter(|k| !v.contains(k)).collect::<Vec<_>>());
            v
        }
        NodeRef::History(id, i) => app.s.node(id).and_then(|n| n.mesh()).and_then(|m| m.history.get(*i)).map(|h| h.params.iter().filter(|(_, x)| x.is_number()).map(|(k, _)| k.clone()).collect()).unwrap_or_default(),
        NodeRef::Material(id) => app.s.mats.get(id).map(|m| m.attrs.keys().cloned().collect()).unwrap_or_default(),
        NodeRef::Texture(id) => {
            let mut v = vec!["outColor".to_string()];
            v.extend(app.s.texs.get(id).map(|t| t.attrs.iter().filter(|(_, x)| x.is_number()).map(|(k, _)| k.clone()).collect::<Vec<_>>()).unwrap_or_default());
            v
        }
    }
}

/// Load the selection: first object on the left (outputs), last on the right (inputs).
pub fn load_connection_editor(app: &mut IncaApp) {
    if let Some(f) = app.s.sel.first() {
        app.hg.ce_left = Some(NodeRef::Dag(f.clone()));
    }
    if let Some(l) = app.s.sel.last() {
        app.hg.ce_right = Some(NodeRef::Dag(l.clone()));
    }
}

pub fn connection_editor(app: &mut IncaApp, ctx: &egui::Context) {
    let mut open = app.win.connection_editor;
    if !open {
        return;
    }
    egui::Window::new("Connection Editor").open(&mut open).default_size([520.0, 440.0]).show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.button("Reload Left").clicked() {
                app.hg.ce_left = app.s.lead().map(|l| NodeRef::Dag(l.clone()));
                app.hg.ce_attr = None;
            }
            if ui.button("Reload Right").clicked() {
                app.hg.ce_right = app.s.lead().map(|l| NodeRef::Dag(l.clone()));
            }
            if ui.button("Swap").clicked() {
                std::mem::swap(&mut app.hg.ce_left, &mut app.hg.ce_right);
                app.hg.ce_attr = None;
            }
        });
        ui.label(RichText::new("Click an output on the left, then an input on the right to connect (click a connected input to break it).").color(theme::TEXT_DIM).size(10.5));
        ui.separator();
        let (l, r) = (app.hg.ce_left.clone(), app.hg.ce_right.clone());
        let (lid, rid) = (l.as_ref().map(|x| app.s.ref_id(x)), r.as_ref().map(|x| app.s.ref_id(x)));
        let mut connect: Option<(String, String)> = None;
        let mut disconnect: Option<(String, String, String)> = None;
        ui.columns(2, |cols| {
            cols[0].label(RichText::new(format!("Outputs: {}", l.as_ref().map(|x| app.s.ref_name(x)).unwrap_or("—".into()))).strong());
            if let Some(l) = &l {
                egui::ScrollArea::vertical().id_salt("ceL").show(&mut cols[0], |ui| {
                    for a in attrs_of(app, l) {
                        let wired = app.s.conns.iter().any(|c| Some(&c.src) == lid.as_ref() && c.src_attr == a);
                        let txt = if wired { RichText::new(&a).italics().color(Color32::from_rgb(140, 200, 255)) } else { RichText::new(&a) };
                        if ui.selectable_label(app.hg.ce_attr.as_deref() == Some(a.as_str()), txt).clicked() {
                            app.hg.ce_attr = Some(a.clone());
                        }
                    }
                });
            }
            cols[1].label(RichText::new(format!("Inputs: {}", r.as_ref().map(|x| app.s.ref_name(x)).unwrap_or("—".into()))).strong());
            if let Some(r) = &r {
                egui::ScrollArea::vertical().id_salt("ceR").show(&mut cols[1], |ui| {
                    for a in attrs_of(app, r) {
                        let from = app.s.conns.iter().find(|c| Some(&c.dst) == rid.as_ref() && c.dst_attr == a).map(|c| (c.src.clone(), c.src_attr.clone()));
                        let txt = match &from {
                            Some(_) => RichText::new(&a).italics().color(Color32::from_rgb(140, 200, 255)),
                            None => RichText::new(&a),
                        };
                        let resp = ui.selectable_label(false, txt);
                        let resp = match &from {
                            Some((s, sa)) => resp.on_hover_text(format!("← {}.{sa}", app.s.ref_by_id(s).map(|x| app.s.ref_name(&x)).unwrap_or_default())),
                            None => resp,
                        };
                        if resp.clicked() {
                            match (&from, &app.hg.ce_attr) {
                                (Some((s, sa)), _) => disconnect = Some((s.clone(), sa.clone(), a.clone())),
                                (None, Some(la)) => connect = Some((la.clone(), a.clone())),
                                _ => {}
                            }
                        }
                    }
                });
            }
        });
        if let (Some((la, ra)), Some(l), Some(r)) = (connect, &l, &r) {
            let _ = app.run_mel("connect", &format!("connectAttr {}.{la} {}.{ra};", app.s.ref_name(l), app.s.ref_name(r)));
        }
        if let (Some((s, sa, ra)), Some(r)) = (disconnect, &r) {
            let sn = app.s.ref_by_id(&s).map(|x| app.s.ref_name(&x)).unwrap_or_default();
            let _ = app.run_mel("disconnect", &format!("disconnectAttr {sn}.{sa} {}.{ra};", app.s.ref_name(r)));
        }
    });
    app.win.connection_editor = open;
}
