//! Marking menus (src/js/ui/markingMenus.js): RMB object / component menus, Shift+RMB tool menus,
//! Ctrl+RMB selection conversion, W/E/R + LMB transform tool menus.

use crate::app::{IncaApp, Tool};
use crate::viewport::{self, CamView};
use egui::{pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Stroke};
use inca_core::scene::{CompMode, SelMode, Shape};

/// What picking an item does.
#[derive(Clone, PartialEq)]
pub enum Act {
    Action(String),
    Mode(Option<CompMode>),
    Select(String),
    AttrEditor(String),
    LookThrough(String),
    Frame(String),
    WorldSpace(bool),
    Discrete,
    Step(f64),
    Symmetry(usize),
    ToolSettings,
    ResetTool,
    AssignExisting(String),
    History(String, usize),
    SetTool(Tool),
    DefaultCamera,
}

#[derive(Clone)]
pub struct Item {
    pub label: String,
    pub act: Option<Act>,
    pub sub: Vec<Item>,
    pub check: Option<bool>,
    pub enabled: bool,
}

fn it(label: &str, act: Act) -> Item {
    Item { label: label.into(), act: Some(act), sub: vec![], check: None, enabled: true }
}
fn cmd(label: &str, action: &str) -> Item {
    it(label, Act::Action(action.into()))
}
fn acmd(action: &str) -> Item {
    cmd(&crate::actions::label(action), action)
}
fn sub(label: &str, items: Vec<Item>) -> Item {
    Item { label: label.into(), act: None, sub: items, check: None, enabled: true }
}
fn checked(mut i: Item, on: bool) -> Item {
    i.check = Some(on);
    i
}
fn disabled(mut i: Item) -> Item {
    i.enabled = false;
    i
}

pub struct Menu {
    pub pos: Pos2,
    title: Option<String>,
    radial: Vec<(&'static str, Item)>,
    list: Vec<Option<Item>>,
    vp: usize,
    opened: f64,
    button: egui::PointerButton,
    sticky: bool,
    hot_radial: Option<usize>,
}

const DIRS: [(&str, f32); 8] = [("N", -90.0), ("NE", -45.0), ("E", 0.0), ("SE", 45.0), ("S", 90.0), ("SW", 135.0), ("W", 180.0), ("NW", -135.0)];

fn menu(pos: Pos2, vp: usize, button: egui::PointerButton, title: Option<String>, radial: Vec<(&'static str, Item)>, list: Vec<Option<Item>>, now: f64) -> Menu {
    Menu { pos, title, radial, list, vp, opened: now, button, sticky: false, hot_radial: None }
}

/// RMB in a viewport: build the menu for the modifiers and what's under the cursor.
pub fn open_rmb(app: &mut IncaApp, idx: usize, cv: &CamView, pos: Pos2, mods: egui::Modifiers, now: f64) {
    let r = egui::PointerButton::Secondary;
    let vp = app.vps[idx].clone();
    // Shift+RMB: tool menus per component mode
    if mods.shift && !mods.ctrl {
        let (radial, list): (Vec<(&'static str, Item)>, Vec<Option<Item>>) = match app.s.comp_mode {
            Some(CompMode::Face) => (
                vec![("N", cmd("Extrude Face", "extrude")), ("NE", cmd("Duplicate Face", "duplicateFaces")), ("E", cmd("Extract Face", "extract")), ("S", cmd("Poke Face", "poke")), ("W", cmd("Bridge", "bridge")), ("NW", cmd("Bevel Face", "bevel")), ("SW", cmd("Add Divisions", "addDivisions")), ("SE", cmd("Triangulate", "triangulate"))],
                vec![Some(acmd("multiCutTool")), Some(acmd("insertEdgeLoopTool")), None, Some(acmd("toVertices")), Some(acmd("toEdges")), None, Some(sub("Assign New Material", new_mats()))],
            ),
            Some(CompMode::Edge) => (
                vec![("N", cmd("Extrude Edge", "extrude")), ("NE", cmd("Collapse Edge", "collapse")), ("E", cmd("Bridge Edge", "bridge")), ("S", cmd("Delete Edge", "deleteEdgeVertex")), ("W", cmd("Bevel Edge", "bevel")), ("NW", cmd("Merge Edges", "mergeToCenter")), ("SW", sub("Soften/Harden", vec![acmd("softenEdge"), acmd("hardenEdge")])), ("SE", cmd("Fill Hole", "fillHole"))],
                vec![Some(acmd("insertEdgeLoopTool")), Some(acmd("multiCutTool")), Some(acmd("connect")), None, Some(acmd("selectEdgeLoop")), Some(acmd("selectEdgeRing")), Some(acmd("selectBorder"))],
            ),
            Some(CompMode::Vertex) => (
                vec![("N", cmd("Extrude Vertex", "chamferVertex")), ("NE", cmd("Merge Vertices", "merge")), ("E", cmd("Connect", "connect")), ("S", cmd("Delete Vertex", "deleteEdgeVertex")), ("W", cmd("Chamfer Vertex", "chamferVertex")), ("NW", cmd("Merge to Center", "mergeToCenter")), ("SW", cmd("Target Weld Tool", "targetWeldTool"))],
                vec![Some(acmd("multiCutTool")), Some(acmd("insertEdgeLoopTool")), None, Some(acmd("toEdges")), Some(acmd("toFaces"))],
            ),
            None => (
                vec![("N", cmd("Extrude", "extrude")), ("NE", cmd("Combine", "combine")), ("E", cmd("Separate", "separate")), ("S", cmd("Smooth", "smooth")), ("W", cmd("Mirror", "mirror")), ("NW", cmd("Bevel", "bevel")), ("SE", cmd("Delete History", "deleteHistory")), ("SW", cmd("Bridge", "bridge"))],
                vec![Some(acmd("multiCutTool")), Some(acmd("insertEdgeLoopTool")), Some(acmd("targetWeldTool")), None, Some(acmd("centerPivot")), Some(acmd("freezeTransformations")), Some(acmd("resetTransformations"))],
            ),
        };
        app.marking = Some(menu(pos, idx, r, None, radial, list, now));
        return;
    }
    // Ctrl+RMB: selection conversion
    if mods.ctrl || mods.command {
        let radial = vec![("N", cmd("To Edges", "toEdges")), ("W", cmd("To Vertices", "toVertices")), ("E", cmd("To Faces", "toFaces")), ("S", cmd("Edge Loop", "selectEdgeLoop")), ("SE", cmd("Edge Ring", "selectEdgeRing")), ("SW", cmd("Border", "selectBorder")), ("NE", cmd("Grow", "growSelection")), ("NW", cmd("Shrink", "shrinkSelection"))];
        app.marking = Some(menu(pos, idx, r, None, radial, vec![], now));
        return;
    }
    let under = viewport::pick_object(&app.s, &vp, cv, pos);
    let target = under.clone().filter(|o| app.s.node(o).is_some_and(|n| matches!(n.shape, Shape::Mesh(_) | Shape::Curve(_)))).or_else(|| app.s.comp_mode.and_then(|_| app.s.hilite().first().cloned()));
    if let Some(t) = target {
        let is_curve = matches!(app.s.node(&t).map(|n| &n.shape), Some(Shape::Curve(_)));
        let name = app.s.name_of(&t);
        let radial = if is_curve {
            vec![("N", it("Control Vertex", Act::Mode(Some(CompMode::Vertex)))), ("E", it("Object Mode", Act::Mode(None))), ("W", it("Edit Point", Act::Mode(Some(CompMode::Vertex)))), ("S", disabled(it("Curve Point", Act::Mode(Some(CompMode::Vertex))))), ("NE", it("Select", Act::Select(t.clone())))]
        } else {
            vec![("N", it("Edge", Act::Mode(Some(CompMode::Edge)))), ("W", it("Vertex", Act::Mode(Some(CompMode::Vertex)))), ("S", it("Face", Act::Mode(Some(CompMode::Face)))), ("E", it("Object Mode", Act::Mode(None))), ("SE", disabled(it("UV", Act::Mode(Some(CompMode::Vertex))))), ("NE", it("Select", Act::Select(t.clone()))), ("NW", it("Multi", Act::Mode(Some(CompMode::Vertex)))), ("SW", it("Vertex Face", Act::Mode(Some(CompMode::Vertex))))]
        };
        let inputs: Vec<Item> = app.s.node(&t).and_then(|n| n.mesh()).map(|m| m.history.iter().enumerate().rev().filter(|(_, h)| !h.name.contains('#') && inca_core::ops::info(&h.ty).is_some_and(|i| !i.1)).map(|(i, h)| it(&h.name, Act::History(t.clone(), i))).collect()).unwrap_or_default();
        let shape = app.s.node(&t).and_then(|n| n.shape_name.clone()).unwrap_or("(none)".into());
        let list = vec![
            Some(cmd("Select All", "selectAll")),
            Some(cmd("Deselect", "deselectAll")),
            Some(cmd("Select Hierarchy", "selectHierarchy")),
            Some(cmd("Inverse Selection", "invertSelection")),
            None,
            Some(sub("Assign New Material...", new_mats())),
            Some(sub("Assign Existing Material", app.s.mats.values().map(|m| it(&m.name, Act::AssignExisting(m.name.clone()))).collect())),
            Some(it("Material Attributes...", Act::Action("hypershade".into()))),
            None,
            Some(sub("Actions", ["hideSelection", "showLastHidden", "templateSel", "centerPivot", "freezeTransformations", "deleteHistory"].iter().map(|a| acmd(a)).collect())),
            Some(sub("Inputs", inputs)),
            Some(sub("Outputs", vec![disabled(cmd(&shape, ""))])),
            None,
            Some(sub("Paint", vec![it("Paint Select Tool", Act::SetTool(Tool::Paint)), cmd("Sculpt Tool", "sculptTool")])),
            Some(disabled(cmd("Metadata", ""))),
        ];
        // picking from this menu works on the object under the cursor
        app.marking_target = Some(t);
        app.marking = Some(menu(pos, idx, r, Some(format!("{name}...")), radial, list, now));
        return;
    }
    if let Some(o) = under {
        let name = app.s.name_of(&o);
        let cam = matches!(app.s.node(&o).map(|n| &n.shape), Some(Shape::Camera(_)));
        let radial = vec![("N", it("Select", Act::Select(o.clone()))), ("E", it("Attribute Editor", Act::AttrEditor(o.clone()))), ("W", if cam { it("Look Through", Act::LookThrough(name.clone())) } else { it("Frame", Act::Frame(o.clone())) })];
        let list = vec![Some(cmd("Select All", "selectAll")), Some(cmd("Select Hierarchy", "selectHierarchy")), None, Some(acmd("hideSelection")), Some(cmd("Delete", "deleteSel"))];
        app.marking = Some(menu(pos, idx, r, Some(format!("{name}...")), radial, list, now));
        return;
    }
    let radial = vec![("N", cmd("Select All", "selectAll")), ("S", cmd("Complete Tool", "finishTool")), ("E", it("Object Mode", Act::Mode(None))), ("W", cmd("Deselect All", "deselectAll")), ("NE", cmd("Frame All", "frameAll")), ("NW", cmd("Repeat Last", "repeatLast"))];
    let create: Vec<Item> = crate::menus::common("Create").into_iter().filter_map(|m| match m {
        crate::menus::M::A(a) => Some(acmd(a)),
        crate::menus::M::Sub(l, v) => Some(sub(l, v.into_iter().filter_map(|x| if let crate::menus::M::A(a) = x { Some(acmd(a)) } else { None }).collect())),
        _ => None,
    }).collect();
    let list = vec![Some(sub("Create", create)), None, Some(cmd("Select Similar", "selectSimilar")), Some(cmd("Paste", "paste")), None, Some(it("Look Through Default", Act::DefaultCamera))];
    app.marking = Some(menu(pos, idx, r, None, radial, list, now));
}

fn new_mats() -> Vec<Item> {
    [("Lambert", "assignNew_lambert"), ("Blinn", "assignNew_blinn"), ("Phong", "assignNew_phong"), ("Standard Surface", "assignNew_standardSurface"), ("Surface Shader", "assignNew_surfaceShader")].iter().map(|(l, a)| cmd(l, a)).collect()
}

/// W / E / R + LMB: transform tool marking menu.
pub fn open_tool(app: &mut IncaApp, idx: usize, tool: Tool, pos: Pos2, now: f64) {
    app.set_tool(tool);
    let (discrete, steps, unit): (&str, &[f64], &str) = match tool {
        Tool::Rotate => ("Discrete Rotate", &[1.0, 5.0, 10.0, 15.0, 30.0, 45.0, 90.0], "°"),
        Tool::Scale => ("Discrete Scale", &[0.1, 0.25, 0.5, 1.0], ""),
        _ => ("Discrete Move", &[0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0], ""),
    };
    let step = if tool == Tool::Rotate { app.vp_state.snap_angle } else { app.vp_state.snap_step };
    let sym = ["Off", "Object X", "World X", "World Y", "World Z"];
    let radial = vec![
        ("N", checked(it("World", Act::WorldSpace(true)), app.world_space)),
        ("NE", checked(it("Object", Act::WorldSpace(false)), !app.world_space)),
        ("E", checked(cmd("Edit Pivot", "pivotEdit"), app.pivot_mode)),
        ("SE", checked(cmd("Soft Select", "softSelect"), app.soft.on)),
        ("S", it("Tool Settings...", Act::ToolSettings)),
        ("SW", checked(it(discrete, Act::Discrete), app.snap_grid)),
        ("W", sub("Step Size", steps.iter().map(|&v| checked(it(&format!("{}{unit}", inca_core::math::fmt_num(v)), Act::Step(v)), (step - v).abs() < 1e-9)).collect())),
        ("NW", sub("Symmetry", sym.iter().enumerate().map(|(i, l)| checked(it(l, Act::Symmetry(i)), app.symmetry == i)).collect())),
    ];
    let list = vec![Some(disabled(cmd("Reset Active Handle", ""))), Some(it("Reset Tool", Act::ResetTool))];
    let title = crate::actions::label(match tool {
        Tool::Rotate => "rotateTool",
        Tool::Scale => "scaleTool",
        _ => "moveTool",
    });
    app.marking = Some(menu(pos, idx, egui::PointerButton::Primary, Some(title), radial, list, now));
}

fn run(app: &mut IncaApp, act: Act) {
    let vp = app.marking.as_ref().map(|m| m.vp).unwrap_or(app.active_vp);
    // menus opened over an object act on it
    if let Some(t) = app.marking_target.take() {
        if matches!(act, Act::Mode(_) | Act::Action(_) | Act::AssignExisting(_)) && !app.s.sel.contains(&t) && !app.s.hilite().contains(&t) {
            if app.s.comp_mode.is_some() {
                app.set_comp_mode(None);
            }
            app.select_names(std::slice::from_ref(&t), SelMode::Replace);
        }
    }
    match act {
        Act::Action(a) if !a.is_empty() => {
            app.active_vp = vp;
            app.action(&a);
        }
        Act::Action(_) => {}
        Act::Mode(m) => app.set_comp_mode(m),
        Act::Select(o) => {
            if app.s.comp_mode.is_some() {
                app.set_comp_mode(None);
            }
            app.select_names(&[o], SelMode::Replace);
        }
        Act::AttrEditor(o) => {
            app.select_names(&[o], SelMode::Replace);
            app.action("attributeEditor");
        }
        Act::LookThrough(c) => app.vps[vp].camera = c,
        Act::Frame(o) => {
            app.select_names(&[o], SelMode::Replace);
            app.active_vp = vp;
            app.action("frameSelected");
        }
        Act::WorldSpace(w) => app.world_space = w,
        Act::Discrete => app.snap_grid = !app.snap_grid,
        Act::Step(v) => {
            if app.tool == Tool::Rotate {
                app.vp_state.snap_angle = v;
            } else {
                app.vp_state.snap_step = v;
            }
            app.snap_grid = true;
        }
        Act::Symmetry(i) => app.symmetry = i,
        Act::ToolSettings => {
            app.right_tab = crate::app::RightTab::ToolSettings;
            app.show_right = true;
        }
        Act::ResetTool => {
            app.world_space = app.tool != Tool::Scale;
            app.snap_grid = false;
            app.vp_state.snap_step = 1.0;
            app.vp_state.snap_angle = 15.0;
            app.pivot_mode = false;
        }
        Act::AssignExisting(m) => {
            let _ = app.run_mel("assign material", &format!("hyperShade -assign {m};"));
        }
        Act::History(id, i) => {
            app.select_names(std::slice::from_ref(&id), SelMode::Replace);
            app.inview = Some((id, i));
        }
        Act::SetTool(t) => app.set_tool(t),
        Act::DefaultCamera => app.vps[vp].camera = ["persp", "top", "front", "side"][vp.min(3)].into(),
    }
}

const ROW: f32 = 31.0;

fn radial_rect(painter_fonts: &egui::Context, m: &Menu, dir: &str, label: &str) -> Rect {
    let row = match dir {
        "N" => -2.0,
        "S" => 2.0,
        d if d.starts_with('N') => -1.0,
        d if d.starts_with('S') => 1.0,
        _ => 0.0,
    };
    let side = if dir == "N" || dir == "S" { 0.0 } else if dir.ends_with('W') { -1.0 } else { 1.0 };
    let w = painter_fonts.fonts_mut(|f| f.layout_no_wrap(label.to_string(), FontId::proportional(12.0), Color32::WHITE).size().x) + 22.0;
    let gap = if row != 0.0 { 30.0 } else { 46.0 };
    let cx = m.pos.x + side * gap;
    let cy = m.pos.y + row * ROW;
    let x0 = if side < 0.0 { cx - w } else if side > 0.0 { cx } else { cx - w / 2.0 };
    Rect::from_min_size(pos2(x0, cy - 11.0), vec2(w, 22.0))
}

/// Draw and drive the open marking menu.
pub fn show(app: &mut IncaApp, ctx: &egui::Context) {
    let Some(m) = app.marking.as_ref() else { return };
    let (ptr, now, esc) = ctx.input(|i| (i.pointer.clone(), i.time, i.key_pressed(egui::Key::Escape)));
    if esc {
        app.marking = None;
        app.marking_target = None;
        return;
    }
    let cur = ptr.hover_pos().unwrap_or(m.pos);
    let rects: Vec<Rect> = m.radial.iter().map(|(d, i)| radial_rect(ctx, m, d, &i.label)).collect();
    let list_top = m.pos.y + 2.0 * ROW + 22.0;
    // hot radial item: under the cursor, or by direction once past the dead zone
    let d = cur - m.pos;
    let mut hot = rects.iter().position(|r| r.contains(cur));
    let in_list_zone = cur.y > list_top - 6.0 && d.x.abs() < 120.0 && !m.list.is_empty();
    if hot.is_none() && d.length() > 28.0 && !in_list_zone {
        let ang = d.y.atan2(d.x).to_degrees();
        let best = m.radial.iter().enumerate().map(|(i, (dir, _))| {
            let a = DIRS.iter().find(|x| x.0 == *dir).unwrap().1;
            (i, ((ang - a + 540.0) % 360.0 - 180.0).abs())
        }).min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((i, diff)) = best {
            if diff < 28.0 {
                hot = Some(i);
            }
        }
    }
    // a radial submenu stays hot while the pointer is in it
    let sub_rect = |i: usize| -> Rect {
        let r = rects[i];
        let n = m.radial[i].1.sub.len() as f32;
        let left = m.radial[i].0.ends_with('W');
        let w = 130.0;
        let x0 = if left { r.min.x - w - 2.0 } else { r.max.x + 2.0 };
        Rect::from_min_size(pos2(x0, r.min.y), vec2(w, n * 20.0 + 6.0))
    };
    if let Some(h) = m.hot_radial {
        if !m.radial[h].1.sub.is_empty() && sub_rect(h).contains(cur) {
            hot = Some(h);
        }
    }
    let mut picked: Option<Act> = None;
    let mut close = false;
    let list = m.list.clone();
    let title = m.title.clone();
    let radial = m.radial.clone();
    let pos = m.pos;
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("marking"));
    let p = ctx.layer_painter(layer);
    p.line_segment([pos, cur], Stroke::new(1.5, Color32::from_gray(0xdd)));
    p.circle_filled(pos, 4.0, Color32::from_gray(0xdd));
    let mut sub_hit: Option<Act> = None;
    for (i, ((_, item), r)) in radial.iter().zip(&rects).enumerate() {
        let is_hot = hot == Some(i);
        let bg = if !item.enabled { Color32::from_gray(0x45) } else if is_hot { Color32::from_rgb(0x52, 0x85, 0xa6) } else { Color32::from_gray(0x50) };
        p.rect_filled(*r, 3.0, bg);
        p.rect_stroke(*r, 3.0, Stroke::new(1.0, Color32::from_gray(0x2a)), egui::StrokeKind::Inside);
        let mut label = item.label.clone();
        if let Some(c) = item.check {
            label = format!("{} {label}", if c { "✔" } else { " " });
        }
        if !item.sub.is_empty() {
            label = if radial[i].0.ends_with('W') { format!("◂ {label}") } else { format!("{label} ▸") };
        }
        p.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.0), if item.enabled { Color32::WHITE } else { Color32::from_gray(0x80) });
        if is_hot && !item.sub.is_empty() {
            let sr = sub_rect(i);
            p.rect_filled(sr, 3.0, Color32::from_gray(0x44));
            p.rect_stroke(sr, 3.0, Stroke::new(1.0, Color32::from_gray(0x2a)), egui::StrokeKind::Inside);
            for (k, s) in item.sub.iter().enumerate() {
                let rr = Rect::from_min_size(pos2(sr.min.x + 3.0, sr.min.y + 3.0 + k as f32 * 20.0), vec2(sr.width() - 6.0, 20.0));
                let over = rr.contains(cur);
                if over {
                    p.rect_filled(rr, 2.0, Color32::from_rgb(0x52, 0x85, 0xa6));
                    sub_hit = s.act.clone();
                }
                let l = match s.check {
                    Some(true) => format!("✔ {}", s.label),
                    Some(false) => format!("   {}", s.label),
                    None => s.label.clone(),
                };
                p.text(pos2(rr.min.x + 6.0, rr.center().y), Align2::LEFT_CENTER, l, FontId::proportional(12.0), Color32::WHITE);
            }
        }
    }
    // the list below the radial items (egui menu widgets, so submenus work)
    let mut list_rect = Rect::NOTHING;
    if !list.is_empty() || title.is_some() {
        let area = egui::Area::new(egui::Id::new("marking-list")).order(egui::Order::Foreground).fixed_pos(pos2(pos.x - 85.0, list_top)).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(170.0);
                if let Some(t) = &title {
                    ui.label(egui::RichText::new(t).italics().color(Color32::from_gray(0xaa)));
                }
                for i in &list {
                    match i {
                        None => {
                            ui.separator();
                        }
                        Some(i) => list_item(ui, i, &mut picked),
                    }
                }
            });
        });
        list_rect = area.response.rect;
    }
    let menu_open = ctx.any_popup_open();
    let quick = now - app.marking.as_ref().unwrap().opened < 0.26 && d.length() < 6.0;
    let button = app.marking.as_ref().unwrap().button;
    let released = ptr.button_released(button);
    let sticky = app.marking.as_ref().unwrap().sticky;
    if picked.is_none() {
        if released && !sticky {
            if quick {
                app.marking.as_mut().unwrap().sticky = true;
            } else if let Some(a) = sub_hit.clone() {
                picked = Some(a);
            } else if let Some(h) = hot {
                if radial[h].1.enabled && radial[h].1.sub.is_empty() {
                    picked = radial[h].1.act.clone();
                } else if !radial[h].1.sub.is_empty() {
                    app.marking.as_mut().unwrap().sticky = true;
                } else {
                    close = true;
                }
            } else if !list_rect.contains(cur) {
                close = true;
            } else {
                app.marking.as_mut().unwrap().sticky = true;
            }
        } else if sticky && ptr.any_click() {
            if let Some(a) = sub_hit {
                picked = Some(a);
            } else if let Some(h) = hot.filter(|h| rects[*h].contains(cur)) {
                if radial[h].1.enabled && radial[h].1.sub.is_empty() {
                    picked = radial[h].1.act.clone();
                }
            } else if !list_rect.contains(cur) && !menu_open {
                close = true;
            }
        }
    }
    if let Some(m) = app.marking.as_mut() {
        m.hot_radial = hot;
    }
    if let Some(a) = picked {
        run(app, a);
        close = true;
    }
    if close {
        app.marking = None;
        app.marking_target = None;
    }
    ctx.request_repaint();
}

fn list_item(ui: &mut egui::Ui, i: &Item, picked: &mut Option<Act>) {
    if !i.sub.is_empty() {
        ui.menu_button(&i.label, |ui| {
            for s in &i.sub {
                list_item(ui, s, picked);
            }
        });
        return;
    }
    let label = match i.check {
        Some(true) => format!("✔ {}", i.label),
        Some(false) => format!("   {}", i.label),
        None => i.label.clone(),
    };
    if ui.add_enabled(i.enabled, egui::Button::new(label).frame_when_inactive(false)).clicked() {
        *picked = i.act.clone();
        ui.close();
    }
}
