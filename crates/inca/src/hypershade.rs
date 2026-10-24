//! Hypershade: material / texture palette with rendered swatches, a node
//! graph work area (drag a texture's output onto a material slot to connect, a material's output
//! onto an object to assign) and the property editor for the selected node.

use crate::app::IncaApp;
use crate::theme;
use egui::{pos2, vec2, Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use inca_core::materials::{MAT_TYPES, TEX_TYPES};
use inca_core::scene::{NodeRef, SelMode};
use serde_json::json;
use std::collections::HashMap;

const NODE_W: f32 = 150.0;
const HEAD_H: f32 = 20.0;
const SWATCH: f32 = 44.0;
const ROW_H: f32 = 16.0;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Texture,
    Material,
    Object,
}

#[derive(Clone)]
struct GNode {
    id: String,
    kind: Kind,
    name: String,
    /// input ports (attribute names), left side
    inputs: Vec<String>,
    /// has an output port, right side
    output: bool,
}

#[derive(Clone, PartialEq)]
enum Wire {
    /// texture -> material.attr
    Tex(String),
    /// material -> object
    Mat(String),
}

pub struct HsState {
    pos: HashMap<String, Pos2>,
    pan: Vec2,
    zoom: f32,
    drag_node: Option<String>,
    wire: Option<Wire>,
    pub selected_only: bool,
    swatches: HashMap<String, (u64, egui::TextureHandle)>,
    pub tab: usize,
}

impl Default for HsState {
    fn default() -> Self {
        HsState { pos: HashMap::new(), pan: vec2(20.0, 20.0), zoom: 1.0, drag_node: None, wire: None, selected_only: false, swatches: HashMap::new(), tab: 0 }
    }
}

fn hash_of<T: std::hash::Hash>(t: &T) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

/// Rendered swatch for a material or texture (cached until its attributes change).
fn swatch(app: &mut IncaApp, ctx: &egui::Context, id: &str) -> Option<egui::TextureId> {
    let (key, px) = if let Some(m) = app.s.mats.get(id) {
        let tex = m.maps.get(m.color_attr()).and_then(|t| t.as_str()).and_then(|t| app.s.texs.get(t));
        let key = hash_of(&(serde_json::to_string(&m.attrs).unwrap_or_default(), &m.ty, tex.map(|t| serde_json::to_string(&t.attrs).unwrap_or_default())));
        if let Some((k, t)) = app.windows_state.hs.swatches.get(id) {
            if *k == key {
                return Some(t.id());
            }
        }
        (key, inca_core::materials::swatch(m, tex, 64))
    } else if let Some(t) = app.s.texs.get(id) {
        let key = hash_of(&serde_json::to_string(&t.attrs).unwrap_or_default());
        if let Some((k, h)) = app.windows_state.hs.swatches.get(id) {
            if *k == key {
                return Some(h.id());
            }
        }
        let px = match inca_core::materials::texture_swatch(t, 64) {
            Some(p) => p,
            None => file_swatch(t.attrs.get("fileTextureName").and_then(|v| v.as_str()).unwrap_or(""), 64),
        };
        (key, px)
    } else {
        return None;
    };
    let img = egui::ColorImage::from_rgba_unmultiplied([64, 64], &px);
    let h = ctx.load_texture(format!("hs_{id}"), img, egui::TextureOptions::LINEAR);
    let tid = h.id();
    app.windows_state.hs.swatches.insert(id.to_string(), (key, h));
    Some(tid)
}

/// A file texture's image, shrunk (gray when it cannot be read).
fn file_swatch(path: &str, size: u32) -> Vec<u8> {
    match image::open(path) {
        Ok(img) => image::imageops::resize(&img.to_rgba8(), size, size, image::imageops::FilterType::Triangle).into_raw(),
        Err(_) => vec![90; (size * size * 4) as usize],
    }
}

/// Objects (meshes) that use a material.
fn users(app: &IncaApp, mid: &str) -> Vec<String> {
    app.s.nodes.values().filter(|n| n.mesh().is_some_and(|m| m.material == mid || m.mesh.fm.as_ref().is_some_and(|fm| fm.iter().flatten().any(|x| x == mid)))).map(|n| n.id.clone()).collect()
}

fn graph_nodes(app: &IncaApp) -> Vec<GNode> {
    let sel = app.windows_state.hs_selected.clone().filter(|m| app.s.mats.contains_key(m));
    let only = app.windows_state.hs.selected_only;
    let mats: Vec<&inca_core::materials::Material> = app.s.mats.values().filter(|m| !only || Some(&m.id) == sel.as_ref()).collect();
    let mut out = vec![];
    let used: Vec<String> = mats.iter().flat_map(|m| m.maps.values().filter_map(|v| v.as_str().map(String::from))).collect();
    for t in app.s.texs.values().filter(|t| !only || used.contains(&t.id)) {
        out.push(GNode { id: t.id.clone(), kind: Kind::Texture, name: t.name.clone(), inputs: vec![], output: true });
    }
    for m in &mats {
        let slots = inca_core::materials::mat_type(&m.ty).mappable.iter().map(|s| s.to_string()).collect();
        out.push(GNode { id: m.id.clone(), kind: Kind::Material, name: m.name.clone(), inputs: slots, output: true });
    }
    if let Some(mid) = &sel {
        for o in users(app, mid) {
            out.push(GNode { id: o.clone(), kind: Kind::Object, name: app.s.name_of(&o), inputs: vec!["material".into()], output: false });
        }
    }
    out
}

fn node_h(n: &GNode) -> f32 {
    HEAD_H + if n.kind == Kind::Object { 0.0 } else { SWATCH + 6.0 } + ROW_H * n.inputs.len().max(1) as f32 + 4.0
}

/// Default column layout: textures, materials, objects.
fn default_pos(nodes: &[GNode], i: usize) -> Pos2 {
    let col = match nodes[i].kind {
        Kind::Texture => 0.0,
        Kind::Material => 220.0,
        Kind::Object => 440.0,
    };
    let y: f32 = nodes[..i].iter().filter(|n| n.kind == nodes[i].kind).map(|n| node_h(n) + 16.0).sum();
    pos2(col, y)
}

pub fn window(app: &mut IncaApp, ui: &mut egui::Ui) {
    // ---- create bar
    ui.horizontal_wrapped(|ui| {
        ui.label("Create:");
        for t in MAT_TYPES {
            if ui.small_button(t.label).on_hover_text(format!("Create a {} material", t.label)).clicked() {
                let _ = app.run_mel("create material", &format!("shadingNode -asShader {};", t.ty));
                app.windows_state.hs_selected = app.s.mats.keys().last().cloned();
            }
        }
        ui.separator();
        for t in TEX_TYPES {
            if ui.small_button(t.label).on_hover_text(format!("Create a {} texture", t.label)).clicked() {
                let _ = app.run_mel("create texture", &format!("shadingNode -asTexture {};", t.ty));
                app.windows_state.hs_selected = app.s.texs.keys().last().cloned();
            }
        }
    });
    ui.separator();
    let avail = ui.available_size();
    let side = 230.0f32.min(avail.x * 0.3);
    ui.horizontal(|ui| {
        ui.set_min_height(avail.y);
        // ---- palette
        ui.vertical(|ui| {
            ui.set_width(side * 0.75);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut app.windows_state.hs.tab, 0, "Materials");
                ui.selectable_value(&mut app.windows_state.hs.tab, 1, "Textures");
            });
            egui::ScrollArea::vertical().id_salt("hspal").show(ui, |ui| palette(app, ui));
        });
        ui.separator();
        // ---- work area
        ui.vertical(|ui| {
            ui.set_width(avail.x - side * 1.75 - 24.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Work Area").strong());
                ui.checkbox(&mut app.windows_state.hs.selected_only, "Graph selected network only");
                if ui.small_button("Rearrange").clicked() {
                    app.windows_state.hs.pos.clear();
                    app.windows_state.hs.pan = vec2(20.0, 20.0);
                    app.windows_state.hs.zoom = 1.0;
                }
            });
            work_area(app, ui);
        });
        ui.separator();
        // ---- property editor
        ui.vertical(|ui| {
            ui.set_width(side);
            egui::ScrollArea::vertical().id_salt("hsattr").show(ui, |ui| properties(app, ui));
        });
    });
}

fn palette(app: &mut IncaApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let ids: Vec<(String, String)> = if app.windows_state.hs.tab == 0 { app.s.mats.values().map(|m| (m.id.clone(), m.name.clone())).collect() } else { app.s.texs.values().map(|t| (t.id.clone(), t.name.clone())).collect() };
    ui.horizontal_wrapped(|ui| {
        for (id, name) in ids {
            let sel = app.windows_state.hs_selected.as_deref() == Some(id.as_str());
            let tex = swatch(app, &ctx, &id);
            ui.vertical(|ui| {
                ui.set_width(56.0);
                let (r, resp) = ui.allocate_exact_size(vec2(52.0, 52.0), Sense::click());
                ui.painter().rect_filled(r, 2.0, if sel { theme::HI } else { theme::BG_DARK });
                if let Some(t) = tex {
                    ui.painter().image(t, r.shrink(3.0), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                ui.label(RichText::new(&name).size(10.0));
                if resp.clicked() {
                    app.windows_state.hs_selected = Some(id.clone());
                }
                if resp.double_clicked() && app.s.mats.contains_key(&id) {
                    let _ = app.run_mel("assign material", &format!("hyperShade -assign {name};"));
                }
                resp.on_hover_text("Click to edit, double-click to assign to the selection");
            });
        }
    });
}

fn properties(app: &mut IncaApp, ui: &mut egui::Ui) {
    let Some(id) = app.windows_state.hs_selected.clone() else {
        ui.label(RichText::new("Select a node to edit it").color(theme::TEXT_DIM));
        return;
    };
    if app.s.mats.contains_key(&id) {
        ui.horizontal(|ui| {
            if ui.button("Assign to Selection").clicked() {
                let n = app.s.mats[&id].name.clone();
                let _ = app.run_mel("assign material", &format!("hyperShade -assign {n};"));
            }
            if id != app.s.default_mat && ui.button("Delete").clicked() {
                app.s.checkpoint("delete material");
                app.s.delete_material(&id);
                app.windows_state.hs_selected = None;
            }
        });
        if app.s.mats.contains_key(&id) {
            crate::windows::material_editor(app, ui, &id);
        }
    } else if let Some(t) = app.s.texs.get(&id).cloned() {
        theme::section_header(ui, &format!("{} ({})", t.name, t.ty));
        let keys: Vec<String> = t.attrs.keys().filter(|k| *k != "colors").cloned().collect();
        crate::panels::attr_map_editor(app, ui, &NodeRef::Texture(id.clone()), &keys);
        if t.ty == "file" && ui.button("Browse image...").clicked() {
            if let Some(p) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "hdr"]).pick_file() {
                app.s.set_attr(&NodeRef::Texture(id.clone()), "fileTextureName", json!(p.to_string_lossy()));
            }
        }
        if ui.button("Delete").clicked() {
            app.s.checkpoint("delete texture");
            for m in app.s.mats.values_mut() {
                m.maps.retain(|_, v| v.as_str() != Some(id.as_str()));
            }
            app.s.texs.shift_remove(&id);
            app.s.touch();
            app.windows_state.hs_selected = None;
        }
    } else {
        ui.label(RichText::new("Select a node to edit it").color(theme::TEXT_DIM));
    }
}

fn bezier(p: &egui::Painter, a: Pos2, b: Pos2, col: Color32, w: f32) {
    let d = ((b.x - a.x).abs() * 0.5).max(30.0);
    let pts: Vec<Pos2> = (0..=24).map(|i| {
        let t = i as f32 / 24.0;
        let u = 1.0 - t;
        let (c1, c2) = (a + vec2(d, 0.0), b - vec2(d, 0.0));
        pos2(u * u * u * a.x + 3.0 * u * u * t * c1.x + 3.0 * u * t * t * c2.x + t * t * t * b.x, u * u * u * a.y + 3.0 * u * u * t * c1.y + 3.0 * u * t * t * c2.y + t * t * t * b.y)
    }).collect();
    p.add(egui::Shape::line(pts, Stroke::new(w, col)));
}

fn work_area(app: &mut IncaApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(38, 38, 38));
    let nodes = graph_nodes(app);
    // positions (graph space) -> screen
    for (i, n) in nodes.iter().enumerate() {
        if !app.windows_state.hs.pos.contains_key(&n.id) {
            let p = default_pos(&nodes, i);
            app.windows_state.hs.pos.insert(n.id.clone(), p);
        }
    }
    let (pan, zoom) = (app.windows_state.hs.pan, app.windows_state.hs.zoom);
    let to_scr = |p: Pos2| rect.min + pan + p.to_vec2() * zoom;
    let node_rect = |n: &GNode, pos: &HashMap<String, Pos2>| Rect::from_min_size(to_scr(pos[&n.id]), vec2(NODE_W, node_h(n)) * zoom);
    let in_port = |n: &GNode, r: Rect, i: usize| pos2(r.min.x, r.min.y + (HEAD_H + if n.kind == Kind::Object { 0.0 } else { SWATCH + 6.0 } + ROW_H * (i as f32 + 0.5)) * zoom);
    let out_port = |r: Rect| pos2(r.max.x, r.min.y + HEAD_H * 0.5 * zoom);
    let pos = app.windows_state.hs.pos.clone();
    let rects: Vec<Rect> = nodes.iter().map(|n| node_rect(n, &pos)).collect();
    let find = |id: &str| nodes.iter().position(|n| n.id == id);
    // ---- wires
    for (mi, m) in nodes.iter().enumerate().filter(|(_, n)| n.kind == Kind::Material) {
        let Some(mat) = app.s.mats.get(&m.id) else { continue };
        for (si, slot) in m.inputs.iter().enumerate() {
            if let Some(ti) = mat.maps.get(slot).and_then(|v| v.as_str()).and_then(find) {
                bezier(&painter, out_port(rects[ti]), in_port(m, rects[mi], si), Color32::from_rgb(110, 190, 255), 2.0);
            }
        }
        for (oi, o) in nodes.iter().enumerate().filter(|(_, n)| n.kind == Kind::Object) {
            if users(app, &m.id).contains(&o.id) {
                bezier(&painter, out_port(rects[mi]), in_port(o, rects[oi], 0), Color32::from_rgb(120, 220, 120), 2.0);
            }
        }
    }
    // ---- nodes
    let font = egui::FontId::proportional(11.0 * zoom.clamp(0.6, 1.4));
    let pointer = ctx.input(|i| i.pointer.hover_pos());
    let mut hovered_port: Option<(usize, Option<usize>)> = None; // (node, input index or None = body)
    for (i, n) in nodes.iter().enumerate() {
        let r = rects[i];
        if !rect.intersects(r) {
            continue;
        }
        let sel = app.windows_state.hs_selected.as_deref() == Some(n.id.as_str()) || (n.kind == Kind::Object && app.s.sel.contains(&n.id));
        let head = match n.kind {
            Kind::Texture => Color32::from_rgb(120, 90, 50),
            Kind::Material => Color32::from_rgb(60, 95, 130),
            Kind::Object => Color32::from_rgb(70, 110, 70),
        };
        painter.rect_filled(r, 4.0, Color32::from_rgb(58, 58, 58));
        painter.rect_filled(Rect::from_min_size(r.min, vec2(r.width(), HEAD_H * zoom)), 4.0, head);
        if sel {
            painter.rect_stroke(r, 4.0, Stroke::new(2.0, theme::HI), egui::StrokeKind::Outside);
        }
        painter.text(r.min + vec2(6.0, HEAD_H * 0.5 * zoom), egui::Align2::LEFT_CENTER, &n.name, font.clone(), Color32::WHITE);
        if n.kind != Kind::Object {
            if let Some(t) = swatch(app, &ctx, &n.id) {
                let sr = Rect::from_min_size(r.min + vec2(6.0, (HEAD_H + 3.0) * zoom), vec2(SWATCH, SWATCH) * zoom);
                painter.image(t, sr, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
        }
        for (si, slot) in n.inputs.iter().enumerate() {
            let p = in_port(n, r, si);
            let connected = app.s.mats.get(&n.id).is_some_and(|m| m.maps.contains_key(slot));
            painter.circle_filled(p, 4.0 * zoom, if connected { Color32::from_rgb(110, 190, 255) } else { Color32::from_gray(150) });
            painter.text(p + vec2(8.0 * zoom, 0.0), egui::Align2::LEFT_CENTER, inca_core::ops::nice_name(slot), font.clone(), theme::TEXT_DIM);
            if pointer.is_some_and(|q| q.distance(p) < 8.0 * zoom.max(0.8)) {
                hovered_port = Some((i, Some(si)));
            }
        }
        if n.output {
            let p = out_port(r);
            painter.circle_filled(p, 4.5 * zoom, Color32::from_rgb(230, 200, 90));
        }
        if hovered_port.is_none() && pointer.is_some_and(|q| r.contains(q)) {
            hovered_port = Some((i, None));
        }
    }
    // ---- interaction
    let press = ctx.input(|i| i.pointer.press_origin());
    if resp.drag_started_by(egui::PointerButton::Primary) {
        if let Some(p) = press {
            // an output port starts a wire, a node body drags the node, empty space pans
            let out_hit = nodes.iter().enumerate().find(|(i, n)| n.output && out_port(rects[*i]).distance(p) < 9.0);
            if let Some((_, n)) = out_hit {
                app.windows_state.hs.wire = Some(if n.kind == Kind::Texture { Wire::Tex(n.id.clone()) } else { Wire::Mat(n.id.clone()) });
            } else if let Some(i) = rects.iter().rposition(|r| r.contains(p)) {
                app.windows_state.hs.drag_node = Some(nodes[i].id.clone());
            }
        }
    }
    if resp.dragged_by(egui::PointerButton::Middle) || (resp.dragged_by(egui::PointerButton::Primary) && app.windows_state.hs.drag_node.is_none() && app.windows_state.hs.wire.is_none()) {
        app.windows_state.hs.pan += resp.drag_delta();
    }
    if let Some(id) = app.windows_state.hs.drag_node.clone() {
        if let Some(p) = app.windows_state.hs.pos.get_mut(&id) {
            *p += resp.drag_delta() / zoom;
        }
    }
    if let (Some(w), Some(q)) = (app.windows_state.hs.wire.clone(), pointer) {
        let src = match &w {
            Wire::Tex(t) | Wire::Mat(t) => find(t).map(|i| out_port(rects[i])),
        };
        if let Some(a) = src {
            bezier(&painter, a, q, Color32::from_rgb(255, 230, 120), 1.5);
        }
    }
    if resp.drag_stopped() {
        if let Some(w) = app.windows_state.hs.wire.take() {
            match (w, hovered_port) {
                (Wire::Tex(t), Some((ni, slot))) if nodes[ni].kind == Kind::Material => {
                    let mid = nodes[ni].id.clone();
                    let attr = match slot {
                        Some(si) => nodes[ni].inputs[si].clone(),
                        None => app.s.mats[&mid].color_attr().to_string(),
                    };
                    app.s.checkpoint("connect texture");
                    app.s.mats.get_mut(&mid).unwrap().maps.insert(attr.clone(), json!(t));
                    app.s.touch();
                    app.echo(format!("connectAttr -force {}.outColor {}.{attr};", app.s.texs.get(&t).map(|x| x.name.clone()).unwrap_or_default(), app.s.mats[&mid].name));
                }
                (Wire::Mat(m), Some((ni, _))) if nodes[ni].kind == Kind::Object => {
                    let name = app.s.mats[&m].name.clone();
                    let _ = app.run_mel("assign material", &format!("select -r {}; hyperShade -assign {name};", nodes[ni].name));
                }
                _ => {}
            }
        }
        app.windows_state.hs.drag_node = None;
    }
    if resp.clicked() {
        match hovered_port {
            Some((i, _)) => {
                let n = &nodes[i];
                if n.kind == Kind::Object {
                    app.select_names(std::slice::from_ref(&n.name), SelMode::Replace);
                } else {
                    app.windows_state.hs_selected = Some(n.id.clone());
                }
            }
            None => {}
        }
    }
    if resp.double_clicked() {
        if let Some((i, None)) = hovered_port {
            if nodes[i].kind == Kind::Material {
                let _ = app.run_mel("assign material", &format!("hyperShade -assign {};", nodes[i].name));
            }
        }
    }
    // right-click a connected input to break the connection
    if resp.secondary_clicked() {
        if let Some((i, Some(si))) = hovered_port {
            let mid = nodes[i].id.clone();
            let attr = nodes[i].inputs[si].clone();
            if app.s.mats.get(&mid).is_some_and(|m| m.maps.contains_key(&attr)) {
                app.s.checkpoint("disconnect texture");
                app.s.mats.get_mut(&mid).unwrap().maps.remove(&attr);
                app.s.touch();
                app.echo(format!("disconnectAttr {}.{attr};", app.s.mats[&mid].name));
            }
        }
    }
    // wheel zooms around the cursor
    if let Some(q) = pointer.filter(|q| rect.contains(*q)) {
        let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let nz = (zoom * (1.0 + scroll * 0.002)).clamp(0.3, 2.5);
            let g = (q - rect.min - pan) / zoom;
            app.windows_state.hs.zoom = nz;
            app.windows_state.hs.pan = q - rect.min - g * nz;
        }
    }
    painter.text(rect.left_bottom() + vec2(6.0, -6.0), egui::Align2::LEFT_BOTTOM, "Drag an output (yellow) onto a material slot or object · right-click a slot to disconnect · wheel zooms · drag empty space to pan", egui::FontId::proportional(10.0), theme::TEXT_DIM);
}
