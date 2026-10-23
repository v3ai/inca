//! Docked panels: Outliner, Channel Box / Layer Editor, Attribute Editor, Tool Settings.

use crate::app::{IncaApp, RightTab, Tool};
use crate::theme;
use egui::{Color32, RichText};
use inca_core::math::fmt_num;
use inca_core::ops;
use inca_core::primitives::ChanKind;
use inca_core::scene::{NodeRef, SelMode, Shape, CHANNELS};
use serde_json::{json, Value};

#[derive(Default)]
pub struct PanelState {
    pub outliner_filter: String,
    pub renaming: Option<(String, String)>,
    pub expanded: std::collections::HashSet<String>,
    pub input_line: String,
    pub input_mode: usize,
    pub quit: bool,
    /// a channel drag is in progress (one undo step per drag)
    pub editing: bool,
    pub ae_tab: usize,
    pub ol: OutlinerOpts,
    /// Attribute Editor locked to a node (List > Auto Load off, or the lock button)
    pub ae_locked: Option<String>,
    /// (shown name, edited text) of the AE node field
    pub ae_name: (String, String),
    pub ae_copies: Vec<(String, usize)>,
    /// outliner rows in display order (last frame), for Shift range selection and arrow keys
    pub ol_rows: Vec<String>,
    pub ol_anchor: Option<String>,
    /// the outliner was clicked last, so arrow keys walk it
    pub ol_active: bool,
    pub ol_scroll: bool,
}

/// Outliner icon for a node (the JS KIND_ICON, lights by type).
fn kind_icon(n: &inca_core::scene::Node) -> &'static str {
    match (&n.shape, n.kind.as_str()) {
        (Shape::Light(l), _) => match l.ty.as_str() {
            "ambientLight" => "ambientLight",
            "directionalLight" => "directionalLight",
            "spotLight" => "spotLight",
            "areaLight" => "areaLight",
            _ => "pointLight",
        },
        (_, "joint") => "joint",
        (_, "constraint") => "parent",
        (_, "lattice") => "nurbsCube",
        (Shape::Mesh(_), _) => "olMesh",
        (Shape::Group, _) => "olGroup",
        (Shape::Camera(_), _) => "olCamera",
        (Shape::Curve(_), _) => "olCurve",
        (Shape::Locator, _) => "olLocator",
    }
}

/// Outliner display options (JS prefs.opt.outliner).
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OutlinerOpts {
    pub shapes: bool,
    pub sets: bool,
    pub dag_only: bool,
    pub sort_alpha: bool,
    pub materials: bool,
}

impl Default for OutlinerOpts {
    fn default() -> Self {
        OutlinerOpts { shapes: false, sets: true, dag_only: false, sort_alpha: false, materials: false }
    }
}

// ------------------------------------------------------------------ outliner

pub fn outliner(app: &mut IncaApp, ui: &mut egui::Ui) {
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("Edit", |ui| {
            for (l, a) in [("Rename", "renameSel"), ("Select Hierarchy", "selectHierarchy")] {
                outliner_cmd(app, ui, l, a);
            }
            ui.separator();
            for (l, a) in [("Group", "group"), ("Ungroup", "ungroup"), ("Delete", "deleteSel")] {
                outliner_cmd(app, ui, l, a);
            }
            ui.separator();
            if ui.button("Expand All").clicked() {
                app.panels.expanded = app.s.all_dag(true).into_iter().collect();
                ui.close();
            }
            if ui.button("Collapse All").clicked() {
                app.panels.expanded.clear();
                ui.close();
            }
        });
        ui.menu_button("Display", |ui| {
            let o = &mut app.panels.ol;
            ui.checkbox(&mut o.dag_only, "DAG Objects Only");
            ui.checkbox(&mut o.shapes, "Shapes");
            ui.checkbox(&mut o.sets, "Sets");
            ui.checkbox(&mut o.materials, "Materials");
            ui.separator();
            ui.checkbox(&mut o.sort_alpha, "Sort Alphabetical");
        });
        ui.menu_button("Show", |ui| {
            if ui.button("Show Selected").clicked() {
                for id in app.s.sel.clone() {
                    let mut p = app.s.node(&id).and_then(|n| n.parent.clone());
                    while let Some(x) = p {
                        p = app.s.node(&x).and_then(|n| n.parent.clone());
                        app.panels.expanded.insert(x);
                    }
                }
                app.panels.ol_scroll = true;
                ui.close();
            }
            outliner_cmd(app, ui, "Frame Selection", "frameSelected");
        });
        ui.menu_button("Help", |ui| {
            if ui.button("Help on Outliner").clicked() {
                app.help = "Click to select, Ctrl to toggle, Shift for range. Drag onto another item to parent; drop on empty space to unparent. Double-click to rename.".into();
                ui.close();
            }
        });
    });
    ui.add(egui::TextEdit::singleline(&mut app.panels.outliner_filter).hint_text("Search...").desired_width(f32::INFINITY));
    let filter = app.panels.outliner_filter.trim().to_lowercase().replace('*', "");
    let hovered = ui.rect_contains_pointer(ui.max_rect());
    let mut rows = vec![];
    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let order = ["persp", "top", "front", "side"];
        let (mut cams, mut rest): (Vec<_>, Vec<_>) = app.s.roots.clone().into_iter().partition(|r| app.s.node(r).is_some_and(|n| n.startup()));
        cams.sort_by_key(|c| order.iter().position(|o| *o == app.s.name_of(c)).unwrap_or(9));
        if app.panels.ol.sort_alpha {
            rest.sort_by_key(|r| app.s.name_of(r).to_lowercase());
        }
        for r in cams.iter().chain(rest.iter()) {
            outliner_row(app, ui, r, 0, &filter, &mut rows);
        }
        let o = app.panels.ol;
        if !o.dag_only && o.sets {
            for n in ["defaultLightSet", "defaultObjectSet"] {
                plain_row(ui, 0, "olSet", n, Color32::from_gray(0xd0));
            }
        }
        if o.materials {
            let mats: Vec<String> = app.s.mats.values().map(|m| m.name.clone()).collect();
            for m in mats {
                if plain_row(ui, 0, "olMaterial", &m, theme::TEXT).clicked() {
                    let _ = app.run_mel("select material", &format!("select -r {m};"));
                }
            }
        }
        // empty space: click deselects, drop unparents
        let (_, zone) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ui.available_height().max(40.0)), egui::Sense::click());
        if zone.clicked() {
            app.select_names(&[], SelMode::Replace);
        }
        if let Some(ids) = zone.dnd_release_payload::<Vec<String>>() {
            let names: Vec<String> = ids.iter().map(|i| app.s.name_of(i)).collect();
            let _ = app.run_mel("unparent", &format!("parent -w {};", names.join(" ")));
        }
    });
    app.panels.ol_scroll = false;
    // arrow keys walk the list (Shift adds), left / right collapse / expand
    if (hovered || app.panels.ol_active) && !ui.ctx().egui_wants_keyboard_input() {
        let (up, down, left, right, shift) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight), i.modifiers.shift));
        if up || down {
            let cur = app.s.lead().and_then(|l| rows.iter().position(|r| r == l));
            let i = match cur {
                Some(i) if up => i.saturating_sub(1),
                Some(i) => (i + 1).min(rows.len().saturating_sub(1)),
                None => 0,
            };
            if let Some(r) = rows.get(i).cloned() {
                app.select_names(&[r], if shift { SelMode::Add } else { SelMode::Replace });
                app.panels.ol_scroll = true;
            }
        }
        if left || right {
            for id in app.s.sel.clone() {
                if right {
                    app.panels.expanded.insert(id);
                } else {
                    app.panels.expanded.remove(&id);
                }
            }
        }
    }
    app.panels.ol_rows = rows;
}

fn outliner_cmd(app: &mut IncaApp, ui: &mut egui::Ui, label: &str, action: &str) {
    if ui.button(label).clicked() {
        app.action(action);
        ui.close();
    }
}

fn matches_filter(app: &IncaApp, id: &str, f: &str) -> bool {
    f.is_empty() || app.s.subtree(id).iter().any(|i| app.s.name_of(i).to_lowercase().contains(f))
}

const ROW_H: f32 = 19.0;

/// A non-node row (shape, set, material).
fn plain_row(ui: &mut egui::Ui, depth: usize, icon: &str, text: &str, col: Color32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 0.0, Color32::from_gray(0x4d));
    }
    let x = r.min.x + 4.0 + depth as f32 * 16.0 + 12.0;
    crate::icons::paint(ui, egui::Rect::from_min_size(egui::pos2(x, r.center().y - 7.5), egui::vec2(15.0, 15.0)), icon, Color32::WHITE);
    ui.painter().text(egui::pos2(x + 20.0, r.center().y), egui::Align2::LEFT_CENTER, text, egui::FontId::proportional(12.0), col);
    resp
}

fn outliner_row(app: &mut IncaApp, ui: &mut egui::Ui, id: &str, depth: usize, filter: &str, rows: &mut Vec<String>) {
    if !matches_filter(app, id, filter) {
        return;
    }
    let Some(n) = app.s.node(id) else { return };
    let name = n.name.clone();
    let icon = kind_icon(n);
    let startup = n.startup();
    let shape_name = n.shape_name.clone().filter(|_| app.panels.ol.shapes);
    let mut kids = n.children.clone();
    if app.panels.ol.sort_alpha {
        kids.sort_by_key(|k| app.s.name_of(k).to_lowercase());
    }
    let has_kids = !kids.is_empty() || shape_name.is_some();
    let open = has_kids && (app.panels.expanded.contains(id) || !filter.is_empty());
    let selected = app.s.sel.iter().any(|s| s == id) || app.s.hilite().iter().any(|s| s == id);
    let child_sel = !selected && app.s.sel.iter().any(|s| app.s.is_ancestor(id, s));
    // hidden here or above
    let mut hidden = false;
    let mut p = Some(id.to_string());
    while let Some(x) = p {
        let Some(nn) = app.s.node(&x) else { break };
        hidden |= !nn.visible;
        p = nn.parent.clone();
    }
    rows.push(id.to_string());
    let (r, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), egui::Sense::click_and_drag());
    if app.panels.ol_scroll && app.s.lead().is_some_and(|l| l == id) {
        ui.scroll_to_rect(r, None);
    }
    let x0 = r.min.x + 4.0 + depth as f32 * 16.0;
    let exp_rect = egui::Rect::from_min_size(egui::pos2(x0, r.min.y), egui::vec2(12.0, ROW_H));
    // drop target: dragging selected rows onto this one parents them here
    let drop_here = resp.dnd_hover_payload::<Vec<String>>().is_some_and(|p| !p.iter().any(|x| x == id));
    let bg = if selected {
        Some(theme::HI)
    } else if resp.hovered() {
        Some(Color32::from_gray(0x4d))
    } else {
        None
    };
    if let Some(c) = bg {
        ui.painter().rect_filled(r, 0.0, c);
    }
    if drop_here {
        ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.0, Color32::from_rgb(0x8f, 0xd0, 0xff)), egui::StrokeKind::Inside);
    }
    let dim = if hidden { 110 } else { 255 };
    if has_kids {
        let c = exp_rect.center();
        let pts = if open {
            vec![c + egui::vec2(-3.5, -2.0), c + egui::vec2(3.5, -2.0), c + egui::vec2(0.0, 2.5)]
        } else {
            vec![c + egui::vec2(-2.0, -3.5), c + egui::vec2(2.5, 0.0), c + egui::vec2(-2.0, 3.5)]
        };
        ui.painter().add(egui::Shape::convex_polygon(pts, theme::TEXT, egui::Stroke::NONE));
    }
    crate::icons::paint(ui, egui::Rect::from_min_size(egui::pos2(x0 + 12.0, r.center().y - 7.5), egui::vec2(15.0, 15.0)), icon, Color32::from_white_alpha(dim));
    let col = if selected {
        Color32::WHITE
    } else if child_sel {
        Color32::from_rgb(0x9f, 0xd7, 0xff)
    } else if startup {
        Color32::from_gray(0x8a)
    } else {
        theme::TEXT
    };
    let col = if hidden { col.gamma_multiply(0.55) } else { col };
    let name_pos = egui::pos2(x0 + 32.0, r.center().y);
    let renaming = app.panels.renaming.as_ref().is_some_and(|(rid, _)| rid == id);
    if renaming {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(egui::pos2(name_pos.x, r.min.y), r.max)));
        let (_, text) = app.panels.renaming.as_mut().unwrap();
        let te = child.text_edit_singleline(text);
        if te.lost_focus() {
            let t = text.trim().to_string();
            let cancel = child.input(|i| i.key_pressed(egui::Key::Escape));
            app.panels.renaming = None;
            if !cancel && !t.is_empty() && t != name {
                let _ = app.run_mel("rename", &format!("rename {name} \"{t}\";"));
            }
        } else {
            te.request_focus();
        }
    } else {
        ui.painter().text(name_pos, egui::Align2::LEFT_CENTER, &name, egui::FontId::proportional(12.0), col);
    }
    let resp = if n_template(app, id) { resp.on_hover_text(format!("{name} (template)")) } else { resp };
    let m = ui.input(|i| i.modifiers);
    let on_exp = resp.interact_pointer_pos().is_some_and(|p| exp_rect.contains(p));
    if resp.clicked() && on_exp && has_kids {
        // Shift expands / collapses the whole branch
        let set = !open;
        let ids = if m.shift { app.s.subtree(id) } else { vec![id.to_string()] };
        for i in ids {
            if set {
                app.panels.expanded.insert(i);
            } else {
                app.panels.expanded.remove(&i);
            }
        }
    } else if resp.clicked() {
        app.panels.ol_active = true;
        let anchor = app.panels.ol_anchor.clone().filter(|a| app.panels.ol_rows.contains(a));
        if m.shift && !startup && anchor.is_some() {
            let a = app.panels.ol_rows.iter().position(|x| Some(x) == anchor.as_ref()).unwrap();
            let b = app.panels.ol_rows.iter().position(|x| x == id).unwrap_or(a);
            let range: Vec<String> = app.panels.ol_rows[a.min(b)..=a.max(b)].to_vec();
            app.select_names(&range, if m.ctrl || m.command { SelMode::Add } else { SelMode::Replace });
        } else if m.ctrl || m.command {
            app.select_names(&[id.to_string()], SelMode::Toggle);
        } else {
            app.select_names(&[id.to_string()], SelMode::Replace);
            app.panels.ol_anchor = Some(id.to_string());
        }
    }
    if resp.double_clicked() && !startup && !on_exp {
        app.panels.renaming = Some((id.to_string(), name.clone()));
    }
    if resp.drag_started() && !startup {
        let moving: Vec<String> = if app.s.sel.iter().any(|s| s == id) { app.s.sel.clone() } else { vec![id.to_string()] };
        resp.dnd_set_drag_payload(moving);
    }
    if resp.dragged() {
        if let (Some(p), Some(ids)) = (ui.ctx().pointer_latest_pos(), egui::DragAndDrop::payload::<Vec<String>>(ui.ctx())) {
            let names: Vec<String> = ids.iter().map(|i| app.s.name_of(i)).collect();
            egui::Area::new(egui::Id::new("ol-drag")).fixed_pos(p + egui::vec2(12.0, 6.0)).order(egui::Order::Tooltip).show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| ui.label(names.join(", ")));
            });
        }
    }
    if let Some(ids) = resp.dnd_release_payload::<Vec<String>>() {
        let moving: Vec<String> = ids.iter().filter(|i| i.as_str() != id && app.s.node(i).is_some_and(|n| !n.startup())).map(|i| app.s.name_of(i)).collect();
        if !moving.is_empty() && !startup {
            let _ = app.run_mel("parent", &format!("parent {} {name};", moving.join(" ")));
            app.panels.expanded.insert(id.to_string());
        }
    }
    resp.context_menu(|ui| {
        if !app.s.sel.iter().any(|s| s == id) {
            app.select_names(&[id.to_string()], SelMode::Replace);
        }
        if ui.button("Rename").clicked() {
            if !startup {
                app.panels.renaming = Some((id.to_string(), name.clone()));
            }
            ui.close();
        }
        outliner_cmd(app, ui, "Select Hierarchy", "selectHierarchy");
        ui.separator();
        for (l, a) in [("Group", "group"), ("Ungroup", "ungroup"), ("Duplicate", "duplicate"), ("Unparent", "unparent"), ("Delete", "deleteSel")] {
            outliner_cmd(app, ui, l, a);
        }
        ui.separator();
        for (l, a) in [("Hide", "hideSelection"), ("Show", "showSelection"), ("Isolate Select", "isolateSelect")] {
            outliner_cmd(app, ui, l, a);
        }
        ui.separator();
        for (l, a) in [("Frame Selection", "frameSelected"), ("Attribute Editor", "attributeEditor")] {
            outliner_cmd(app, ui, l, a);
        }
        ui.separator();
        if ui.button("Expand All").clicked() {
            for s in app.s.sel.clone() {
                app.panels.expanded.extend(app.s.subtree(&s));
            }
            ui.close();
        }
        if ui.button("Collapse All").clicked() {
            app.panels.expanded.clear();
            ui.close();
        }
    });
    if open {
        if let Some(sn) = shape_name {
            plain_row(ui, depth + 1, icon, &sn, Color32::from_gray(0xaa));
        }
        for k in kids {
            outliner_row(app, ui, &k, depth + 1, filter, rows);
        }
    }
}

fn n_template(app: &IncaApp, id: &str) -> bool {
    app.s.is_template(id)
}

// ------------------------------------------------------------------ right panel

/// Right dock panels as the JS side tabs list them (Modeling Toolkit is still planned).
const SIDE_TABS: &[(Option<RightTab>, &str)] = &[
    (Some(RightTab::ChannelBox), "Channel Box / Layer Editor"),
    (Some(RightTab::AttributeEditor), "Attribute Editor"),
    (Some(RightTab::ToolSettings), "Tool Settings"),
    (None, "Modeling Toolkit"),
];

/// The vertical tab strip at the right edge: click shows a panel, clicking the shown one hides the dock.
pub fn side_tabs(app: &mut IncaApp, ui: &mut egui::Ui) {
    let mut y = ui.max_rect().min.y + 6.0;
    let x = ui.max_rect().center().x;
    let font = egui::FontId::proportional(10.5);
    for (tab, label) in SIDE_TABS {
        let galley = ui.fonts_mut(|f| f.layout_no_wrap(label.to_string(), font.clone(), Color32::WHITE));
        let len = galley.size().x + 16.0;
        let r = egui::Rect::from_min_size(egui::pos2(x - 9.0, y), egui::vec2(18.0, len));
        let resp = ui.interact(r, egui::Id::new(("sidetab", *label)), egui::Sense::click()).on_hover_text(*label);
        let active = app.show_right && tab.is_some_and(|t| t == app.right_tab);
        if active {
            ui.painter().rect_filled(r, 3.0, Color32::from_gray(0x55));
        }
        let col = if active || resp.hovered() { Color32::WHITE } else { Color32::from_gray(0x9d) };
        // vertical-rl: text reads top to bottom
        let pos = egui::pos2(r.max.x - (18.0 - galley.size().y) / 2.0, r.min.y + 8.0);
        ui.painter().add(egui::epaint::TextShape::new(pos, galley, col).with_override_text_color(col).with_angle(std::f32::consts::FRAC_PI_2));
        if resp.clicked() {
            match tab {
                Some(_) if active => app.show_right = false,
                Some(t) => {
                    app.right_tab = *t;
                    app.show_right = true;
                }
                None => app.help = "The Modeling Toolkit panel is planned".into(),
            }
        }
        y += len + 2.0;
    }
}

pub fn right_panel(app: &mut IncaApp, ui: &mut egui::Ui) {
    if app.shown("sidetabs") {
        // the side tabs switch panels; show the panel's title like the JS dock
        let title = SIDE_TABS.iter().find(|t| t.0 == Some(app.right_tab)).map(|t| t.1).unwrap_or("");
        ui.label(RichText::new(title).strong());
    } else {
        ui.horizontal(|ui| {
            for (t, l) in [(RightTab::ChannelBox, "Channel Box"), (RightTab::AttributeEditor, "Attribute Editor"), (RightTab::ToolSettings, "Tool Settings")] {
                if ui.selectable_label(app.right_tab == t, l).clicked() {
                    app.right_tab = t;
                }
            }
        });
    }
    ui.separator();
    match app.right_tab {
        RightTab::ChannelBox => {
            let h = ui.available_height();
            egui::ScrollArea::vertical().id_salt("cbscroll").max_height(h * 0.68).auto_shrink([false, false]).show(ui, |ui| channel_box(app, ui));
            ui.separator();
            layer_editor(app, ui);
        }
        RightTab::AttributeEditor => attribute_editor(app, ui),
        RightTab::ToolSettings => tool_settings(app, ui),
    }
}

/// Apply a channel edit to every selected object (Maya edits all selected channels).
fn edit_channel(app: &mut IncaApp, targets: &[NodeRef], attr: &str, v: Value, resp: &egui::Response) {
    if (resp.drag_started() || (resp.changed() && !resp.dragged())) && !app.panels.editing {
        app.s.checkpoint(&format!("setAttr {attr}"));
        app.panels.editing = true;
    }
    for r in targets {
        app.s.set_attr(r, attr, v.clone());
        if app.s.time.auto_key {
            if let NodeRef::Dag(id) = r {
                if app.s.anim.is_keyed(id, attr) {
                    app.s.set_key(id, attr, None, None);
                }
            }
        }
    }
    let _ = resp;
}

fn finish_edit(app: &mut IncaApp, targets: &[NodeRef], attr: &str, resp: &egui::Response) {
    if (resp.drag_stopped() || resp.lost_focus() || (resp.changed() && !resp.dragged())) && app.panels.editing {
        app.panels.editing = false;
        for r in targets {
            let name = app.s.ref_name(r);
            if let Some(v) = app.s.get_attr(r, attr) {
                let vs = match &v {
                    Value::Number(n) => fmt_num(n.as_f64().unwrap_or(0.0)),
                    Value::Bool(b) => (*b as i32).to_string(),
                    Value::Array(a) => a.iter().map(|x| fmt_num(x.as_f64().unwrap_or(0.0))).collect::<Vec<_>>().join(" "),
                    Value::String(s) => format!("-type \"string\" \"{s}\""),
                    _ => String::new(),
                };
                app.echo(format!("setAttr \"{name}.{attr}\" {vs};"));
            }
        }
    }
}

fn channel_row(app: &mut IncaApp, ui: &mut egui::Ui, targets: &[NodeRef], attr: &str, label: &str, kind: ChanKind, min: Option<f64>, max: Option<f64>) {
    let Some(first) = targets.first() else { return };
    let Some(cur) = app.s.get_attr(first, attr) else { return };
    let id = app.s.ref_id(first);
    let keyed = app.s.anim.is_keyed(&id, attr);
    let on_key = app.s.anim.on_key(&id, attr, app.s.time.current);
    let driven = app.s.conns.iter().any(|c| c.dst == id && c.dst_attr == attr);
    let locked = match first {
        NodeRef::Dag(i) => app.s.node(i).is_some_and(|n| n.is_locked(attr)),
        _ => false,
    };
    ui.horizontal(|ui| {
        let lw = 120.0;
        let (rect, lresp) = ui.allocate_exact_size(egui::vec2(lw, 18.0), egui::Sense::click());
        ui.painter().text(rect.right_center() - egui::vec2(4.0, 0.0), egui::Align2::RIGHT_CENTER, label, egui::FontId::proportional(12.0), theme::TEXT);
        let fill = if locked {
            theme::LOCKED
        } else if driven {
            theme::DRIVEN.gamma_multiply(0.6)
        } else if on_key {
            theme::KEY
        } else if keyed {
            theme::KEYED.gamma_multiply(0.7)
        } else {
            theme::FIELD
        };
        let w = ui.available_width();
        let resp = egui::Frame::NONE.fill(fill).show(ui, |ui| {
            ui.set_width(w);
            ui.add_enabled_ui(!locked, |ui| match kind {
                ChanKind::Bool => {
                    let mut b = cur.as_bool().unwrap_or(cur.as_f64().unwrap_or(0.0) != 0.0);
                    let lbl = if b { "on" } else { "off" };
                    let r = ui.checkbox(&mut b, lbl);
                    if r.changed() {
                        let v = if attr == "visibility" { json!(b as i32) } else { json!(b) };
                        edit_channel(app, targets, attr, v, &r);
                        finish_edit(app, targets, attr, &r);
                    }
                    r
                }
                ChanKind::Enum(opts) => {
                    let mut i = cur.as_f64().map(|x| x as usize).or_else(|| cur.as_str().and_then(|s| opts.iter().position(|o| *o == s))).unwrap_or(0);
                    let before = i;
                    let r = egui::ComboBox::from_id_salt(("enum", attr, id.clone())).width(w - 8.0).selected_text(opts.get(i).copied().unwrap_or("?")).show_index(ui, &mut i, opts.len(), |k| opts[k].to_string());
                    if i != before {
                        let v = if cur.is_string() { json!(opts[i]) } else { json!(i) };
                        app.s.checkpoint(&format!("setAttr {attr}"));
                        for t in targets {
                            app.s.set_attr(t, attr, v.clone());
                        }
                        app.echo(format!("setAttr \"{}.{attr}\" {i};", app.s.ref_name(first)));
                    }
                    r
                }
                _ => {
                    let mut x = cur.as_f64().unwrap_or(0.0);
                    let mut dv = egui::DragValue::new(&mut x).speed(if kind == ChanKind::Int { 0.1 } else { 0.02 }).max_decimals(3);
                    if kind == ChanKind::Int {
                        dv = dv.fixed_decimals(0);
                    }
                    if let (Some(a), Some(b)) = (min, max) {
                        dv = dv.range(a..=b);
                    } else if let Some(a) = min {
                        dv = dv.range(a..=f64::MAX);
                    }
                    let r = ui.add_sized([w - 4.0, 18.0], dv);
                    if r.changed() {
                        let v = if kind == ChanKind::Int { json!(x.round() as i64) } else { json!(x) };
                        edit_channel(app, targets, attr, v, &r);
                    }
                    finish_edit(app, targets, attr, &r);
                    r
                }
            })
            .inner
        });
        let _ = resp;
        lresp.context_menu(|ui| {
            let names: Vec<String> = targets.iter().map(|t| app.s.ref_name(t)).collect();
            if ui.button("Key Selected").clicked() {
                let m: String = names.iter().map(|n| format!("setKeyframe \"{n}.{attr}\";\n")).collect();
                let _ = app.run_mel("key", &m);
                ui.close();
            }
            if ui.button("Delete Keys").clicked() {
                let m: String = names.iter().map(|n| format!("cutKey -at {attr} {n};\n")).collect();
                let _ = app.run_mel("cut keys", &m);
                ui.close();
            }
            if ui.button(if locked { "Unlock" } else { "Lock" }).clicked() {
                app.s.checkpoint("lock");
                for t in targets {
                    if let NodeRef::Dag(i) = t {
                        if let Some(n) = app.s.nodes.get_mut(i) {
                            n.locks.insert(attr.to_string(), json!(!locked));
                        }
                    }
                }
                app.s.touch();
                ui.close();
            }
            if driven && ui.button("Break Connections").clicked() {
                app.s.checkpoint("break connection");
                app.s.conns.retain(|c| !(c.dst == id && c.dst_attr == attr));
                app.s.touch();
                ui.close();
            }
        });
    });
}

pub fn channel_box(app: &mut IncaApp, ui: &mut egui::Ui) {
    let Some(lead) = app.s.lead().cloned() else {
        ui.label(RichText::new("Nothing selected").color(theme::TEXT_DIM));
        return;
    };
    let Some(n) = app.s.node(&lead) else { return };
    let name = n.name.clone();
    let shape = n.shape_name.clone();
    // object name (editable)
    let mut nm = name.clone();
    let r = ui.add(egui::TextEdit::singleline(&mut nm).font(egui::TextStyle::Heading).desired_width(f32::INFINITY));
    if r.lost_focus() && nm.trim() != name && !nm.trim().is_empty() {
        let _ = app.run_mel("rename", &format!("rename {name} \"{}\";", nm.trim()));
        return;
    }
    // the lead object comes first: its values are the ones shown
    let mut targets: Vec<NodeRef> = vec![NodeRef::Dag(lead.clone())];
    targets.extend(app.s.sel.iter().filter(|i| **i != lead).map(|i| NodeRef::Dag(i.clone())));
    for c in CHANNELS {
        let kind = if c == "visibility" { ChanKind::Bool } else { ChanKind::Float };
        channel_row(app, ui, &targets, c, &ops::nice_name(c), kind, None, None);
    }
    // added (dynamic) attributes, unless the SHAPES section lists them
    let sh_channels = app.s.shape_channels(&lead);
    let extra: Vec<(String, ChanKind)> = app.s.node(&lead).and_then(|n| n.extra.as_object()).map(|e| {
        e.iter()
            .filter(|(k, _)| !sh_channels.contains(k))
            .filter_map(|(k, v)| match v {
                Value::Bool(_) => Some((k.clone(), ChanKind::Bool)),
                Value::Number(n) if n.is_f64() => Some((k.clone(), ChanKind::Float)),
                Value::Number(_) => Some((k.clone(), ChanKind::Int)),
                _ => None,
            })
            .collect()
    }).unwrap_or_default();
    for (k, kind) in extra {
        channel_row(app, ui, &targets, &k, &ops::nice_name(&k), kind, None, None);
    }
    // SHAPES
    let shape = shape.or_else(|| (!sh_channels.is_empty()).then(|| app.s.node(&lead).map(|n| n.kind.clone()).unwrap_or_default()));
    if let Some(sn) = shape {
        ui.add_space(4.0);
        theme::section_header(ui, "SHAPES");
        ui.label(RichText::new(sn).color(theme::TEXT));
        for c in sh_channels {
            let v = app.s.get_attr(&NodeRef::Dag(lead.clone()), &c);
            let kind = match v {
                Some(Value::Bool(_)) => ChanKind::Bool,
                _ if c == "smoothLevel" => ChanKind::Int,
                _ => ChanKind::Float,
            };
            let (mn, mx) = if c == "smoothLevel" { (Some(0.0), Some(3.0)) } else { (None, None) };
            channel_row(app, ui, &[NodeRef::Dag(lead.clone())], &c, &ops::nice_name(&c), kind, mn, mx);
        }
    }
    // INPUTS (construction history, newest first); a deformer handle shows its deformer
    let mut hist: Vec<(String, usize, String, String)> = app.s.node(&lead).and_then(|n| n.mesh()).map(|m| m.history.iter().enumerate().filter(|(_, h)| !h.name.contains('#')).map(|(i, h)| (lead.clone(), i, h.name.clone(), h.ty.clone())).collect()).unwrap_or_default();
    hist.reverse();
    if app.s.node(&lead).is_some_and(|n| n.kind == "deformer") {
        for n in app.s.nodes.values() {
            if let Some(m) = n.mesh() {
                for (i, h) in m.history.iter().enumerate() {
                    if h.params.get("handle").and_then(|v| v.as_str()) == Some(lead.as_str()) {
                        hist.push((n.id.clone(), i, h.name.clone(), h.ty.clone()));
                    }
                }
            }
        }
    }
    if !hist.is_empty() {
        ui.add_space(4.0);
        theme::section_header(ui, "INPUTS");
        for (owner, i, hname, ty) in hist {
            let lead = owner;
            let open_id = format!("{lead}:{hname}");
            let open = app.panels.expanded.contains(&open_id);
            let err = app.s.node(&lead).and_then(|n| n.mesh()).and_then(|m| m.history[i].error.clone());
            let t = RichText::new(format!("{} {hname}", if open { "-" } else { "+" })).color(if err.is_some() { Color32::from_rgb(240, 120, 120) } else { theme::TEXT });
            let r = ui.add(egui::Button::new(t).frame(false));
            if let Some(e) = err {
                r.clone().on_hover_text(e);
            }
            if r.clicked() {
                if open {
                    app.panels.expanded.remove(&open_id);
                } else {
                    app.panels.expanded.insert(open_id);
                }
            }
            if open {
                if let Some((_, _, chans)) = ops::info(&ty) {
                    let r = NodeRef::History(lead.clone(), i);
                    for c in chans {
                        let label = c.label.map(String::from).unwrap_or_else(|| ops::nice_name(c.key));
                        channel_row(app, ui, std::slice::from_ref(&r), c.key, &label, c.kind, c.min, c.max);
                    }
                }
            }
        }
    }
}

fn layer_editor(app: &mut IncaApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Display Layers").strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("＋").on_hover_text("Create a new layer and assign the selected objects").clicked() {
                let _ = app.run_mel("create layer", "createDisplayLayer;");
            }
        });
    });
    let layers = app.s.layers.clone();
    egui::ScrollArea::vertical().id_salt("layers").show(ui, |ui| {
        for (i, l) in layers.iter().enumerate() {
            ui.horizontal(|ui| {
                let mut vis = l.visible;
                if ui.add(egui::Button::new(if vis { "V" } else { " " }).selected(vis).min_size(egui::vec2(18.0, 16.0))).on_hover_text("Visibility").clicked() {
                    vis = !vis;
                    app.s.checkpoint("layer visibility");
                    app.s.layers[i].visible = vis;
                    app.s.touch();
                }
                let mode = l.mode.clone();
                let next = match mode.as_str() {
                    "normal" => "template",
                    "template" => "reference",
                    _ => "normal",
                };
                let tag = match mode.as_str() {
                    "template" => "T",
                    "reference" => "R",
                    _ => " ",
                };
                if ui.add(egui::Button::new(tag).min_size(egui::vec2(18.0, 16.0))).on_hover_text("Normal / Template / Reference").clicked() {
                    app.s.checkpoint("layer mode");
                    app.s.layers[i].mode = next.into();
                    app.s.touch();
                }
                let r = ui.selectable_label(false, &l.name);
                if r.clicked() {
                    let ids: Vec<String> = app.s.nodes.values().filter(|n| n.layer.as_deref() == Some(l.id.as_str())).map(|n| n.id.clone()).collect();
                    app.select_names(&ids, SelMode::Replace);
                }
                r.context_menu(|ui| {
                    if ui.button("Add Selected Objects").clicked() {
                        let names: Vec<String> = app.s.sel.iter().map(|i| app.s.name_of(i)).collect();
                        let _ = app.run_mel("layer members", &format!("editDisplayLayerMembers {} {};", l.name, names.join(" ")));
                        ui.close();
                    }
                    if ui.button("Delete Layer").clicked() {
                        app.s.checkpoint("delete layer");
                        let lid = l.id.clone();
                        app.s.layers.retain(|x| x.id != lid);
                        for n in app.s.nodes.values_mut() {
                            if n.layer.as_deref() == Some(lid.as_str()) {
                                n.layer = None;
                            }
                        }
                        app.s.touch();
                        ui.close();
                    }
                });
            });
        }
        if layers.is_empty() {
            ui.label(RichText::new("No layers").color(theme::TEXT_DIM));
        }
    });
}

// ------------------------------------------------------------------ attribute editor

/// Generic editor for a JSON attribute map (materials, lights, textures, history params).
pub fn attr_map_editor(app: &mut IncaApp, ui: &mut egui::Ui, r: &NodeRef, keys: &[String]) {
    egui::Grid::new(("ae", app.s.ref_id(r))).num_columns(2).spacing([8.0, 4.0]).striped(false).show(ui, |ui| {
        for k in keys {
            let Some(v) = app.s.get_attr(r, k) else { continue };
            ui.label(ops::nice_name(k));
            match v {
                Value::Array(a) if a.len() == 3 && a.iter().all(|x| x.is_number()) => {
                    let mut c = [a[0].as_f64().unwrap_or(0.0) as f32, a[1].as_f64().unwrap_or(0.0) as f32, a[2].as_f64().unwrap_or(0.0) as f32];
                    let is_color = k.to_lowercase().contains("color") || k.contains("transparency") || k.contains("incandescence") || k == "opacity";
                    let changed = if is_color {
                        ui.color_edit_button_rgb(&mut c).changed()
                    } else {
                        let mut ch = false;
                        ui.horizontal(|ui| {
                            for x in c.iter_mut() {
                                ch |= ui.add(egui::DragValue::new(x).speed(0.02)).changed();
                            }
                        });
                        ch
                    };
                    if changed {
                        if !app.panels.editing {
                            app.s.checkpoint(&format!("setAttr {k}"));
                        }
                        app.s.set_attr(r, k, json!([c[0], c[1], c[2]]));
                    }
                }
                Value::Number(n) => {
                    let mut x = n.as_f64().unwrap_or(0.0);
                    let resp = ui.add(egui::DragValue::new(&mut x).speed(0.01).max_decimals(3));
                    if resp.changed() {
                        edit_channel(app, std::slice::from_ref(r), k, json!(x), &resp);
                    }
                    finish_edit(app, std::slice::from_ref(r), k, &resp);
                }
                Value::Bool(mut b) => {
                    if ui.checkbox(&mut b, "").changed() {
                        app.s.checkpoint(&format!("setAttr {k}"));
                        app.s.set_attr(r, k, json!(b));
                    }
                }
                Value::String(mut s) => {
                    let resp = ui.text_edit_singleline(&mut s);
                    if resp.changed() {
                        app.s.set_attr(r, k, json!(s));
                    }
                }
                other => {
                    ui.label(RichText::new(other.to_string()).color(theme::TEXT_DIM));
                }
            }
            ui.end_row();
        }
    });
}

/// Attribute Editor tabs for a node: transform, shape, history nodes, material.
fn ae_tabs(app: &IncaApp, id: &str) -> Vec<(String, NodeRef)> {
    let Some(n) = app.s.node(id) else { return vec![] };
    let mut tabs: Vec<(String, NodeRef)> = vec![(n.name.clone(), NodeRef::Dag(id.to_string()))];
    if let Some(sn) = &n.shape_name {
        tabs.push((sn.clone(), NodeRef::Dag(id.to_string())));
    } else if n.extra.as_object().is_some_and(|e| !e.is_empty()) {
        tabs.push((format!("{} attributes", n.kind), NodeRef::Dag(id.to_string())));
    }
    if let Some(m) = n.mesh() {
        for (i, h) in m.history.iter().enumerate().rev() {
            if !h.name.contains('#') {
                tabs.push((h.name.clone(), NodeRef::History(id.to_string(), i)));
            }
        }
        if app.s.mats.contains_key(&m.material) {
            tabs.push((app.s.mats[&m.material].name.clone(), NodeRef::Material(m.material.clone())));
        }
    }
    tabs
}

/// The node the Attribute Editor shows: the locked one, or the lead (hilited object in component mode).
fn ae_target(app: &mut IncaApp) -> Option<String> {
    if let Some(l) = &app.panels.ae_locked {
        if app.s.node(l).is_some() {
            return Some(l.clone());
        }
        app.panels.ae_locked = None;
    }
    if app.s.comp_mode.is_some() {
        if let Some(h) = app.s.hilite().last() {
            return Some(h.clone());
        }
    }
    app.s.lead().cloned()
}

pub fn attribute_editor(app: &mut IncaApp, ui: &mut egui::Ui) {
    let target = ae_target(app);
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("List", |ui| {
            let mut auto = app.panels.ae_locked.is_none();
            if ui.checkbox(&mut auto, "Auto Load Selected Attributes").clicked() {
                app.panels.ae_locked = if auto { None } else { target.clone() };
            }
            if ui.button("Load Selected Attributes").clicked() {
                app.panels.ae_locked = None;
                ui.close();
            }
        });
        ui.menu_button("Selected", |ui| {
            if ui.button("Select Node").clicked() {
                if let Some(t) = &target {
                    app.select_names(std::slice::from_ref(t), SelMode::Replace);
                }
                ui.close();
            }
        });
        ui.menu_button("Focus", |ui| {
            for id in app.s.sel.clone() {
                if ui.button(app.s.name_of(&id)).clicked() {
                    app.panels.ae_locked = Some(id);
                    ui.close();
                }
            }
        });
        ui.menu_button("Attributes", |ui| {
            ui.add_enabled(false, egui::Button::new("Add Attribute..."));
        });
        ui.menu_button("Show", |ui| {
            if ui.button("Show Notes").clicked() {
                app.help = "Notes are under Extra Attributes on the transform tab".into();
                ui.close();
            }
        });
        ui.menu_button("Help", |ui| {
            if ui.button("Help on Attribute Editor").clicked() {
                app.help = "The Attribute Editor shows every attribute of the selected node; tabs list connected nodes (shape, history, material).".into();
                ui.close();
            }
        });
    });
    let Some(id) = target else {
        ui.label(RichText::new("Select an object to see its attributes.").color(theme::TEXT_DIM));
        return;
    };
    let tabs = ae_tabs(app, &id);
    if tabs.is_empty() {
        return;
    }
    let mut tab = app.panels.ae_tab.min(tabs.len() - 1);
    ui.horizontal_wrapped(|ui| {
        for (i, (t, _)) in tabs.iter().enumerate() {
            if ui.selectable_label(tab == i, t).clicked() {
                tab = i;
            }
        }
    });
    app.panels.ae_tab = tab;
    // node: name field, Focus, lock
    ui.horizontal(|ui| {
        ui.label(RichText::new("node:").color(theme::TEXT_DIM));
        let title = tabs[tab].0.clone();
        if app.panels.ae_name.0 != title {
            app.panels.ae_name = (title.clone(), title.clone());
        }
        let w = ui.available_width() - 80.0;
        let te = ui.add(egui::TextEdit::singleline(&mut app.panels.ae_name.1).desired_width(w));
        if te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let new = app.panels.ae_name.1.trim().to_string();
            if !new.is_empty() && new != title {
                let _ = app.run_mel("rename", &format!("rename {title} \"{new}\";"));
            }
        }
        if ui.button("Focus").on_hover_text("Show the selected node").clicked() {
            app.panels.ae_locked = None;
        }
        let locked = app.panels.ae_locked.is_some();
        if lock_button(ui, locked).on_hover_text("Lock the Attribute Editor to the current node").clicked() {
            app.panels.ae_locked = if locked { None } else { Some(id.clone()) };
        }
    });
    let h = ui.available_height() - 26.0;
    egui::ScrollArea::vertical().id_salt("aescroll").max_height(h.max(40.0)).auto_shrink([false, false]).show(ui, |ui| ae_body(app, ui, &tabs, tab));
    ui.separator();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        ui.spacing_mut().button_padding.x = 2.0;
        // Load Attributes gets the extra room
        let w = (ui.available_width() - 9.0) / 4.4;
        let b = |ui: &mut egui::Ui, t: &str, w: f32| ui.add_sized([w, 20.0], egui::Button::new(t)).clicked();
        if b(ui, "Select", w) {
            app.select_names(std::slice::from_ref(&id), SelMode::Replace);
        }
        if b(ui, "Load Attributes", w * 1.4) {
            app.panels.ae_locked = None;
        }
        if b(ui, "Copy Tab", w) {
            app.panels.ae_copies.push((id.clone(), tab));
        }
        if b(ui, "Close", w) {
            app.show_right = false;
        }
    });
}

/// A painted padlock toggle (the icon set has none).
fn lock_button(ui: &mut egui::Ui, on: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
    let bg = if on { Color32::from_rgb(0x5d, 0x7a, 0x8f) } else if resp.hovered() { Color32::from_gray(0x6f) } else { theme::BTN };
    ui.painter().rect_filled(r, 3.0, bg);
    let c = r.center();
    let body = egui::Rect::from_center_size(c + egui::vec2(0.0, 2.5), egui::vec2(10.0, 7.0));
    let col = Color32::from_gray(0xe8);
    ui.painter().rect_filled(body, 1.0, col);
    // the shackle: open when unlocked
    let lift = if on { 0.0 } else { 2.0 };
    let pts: Vec<egui::Pos2> = (0..=12).map(|i| {
        let a = std::f32::consts::PI * i as f32 / 12.0;
        egui::pos2(c.x - 3.0 * a.cos(), body.min.y - lift - 4.0 * a.sin())
    }).collect();
    ui.painter().add(egui::Shape::line(pts, egui::Stroke::new(1.5, col)));
    resp
}

/// Copy Tab: floating Attribute Editor windows pinned to a node and tab.
pub fn ae_copies(app: &mut IncaApp, ctx: &egui::Context) {
    let copies = std::mem::take(&mut app.panels.ae_copies);
    let mut keep = vec![];
    for (i, (id, tab)) in copies.into_iter().enumerate() {
        let tabs = ae_tabs(app, &id);
        let Some((title, _)) = tabs.get(tab).cloned() else { continue };
        let mut open = true;
        egui::Window::new(format!("Attribute Editor: {title}")).id(egui::Id::new(("aecopy", i, &id, tab))).open(&mut open).default_size([380.0, 520.0]).show(ctx, |ui| {
            ui.push_id(("aecopy", i), |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| ae_body(app, ui, &tabs, tab));
            });
        });
        if open {
            keep.push((id, tab));
        }
    }
    keep.extend(std::mem::take(&mut app.panels.ae_copies));
    app.panels.ae_copies = keep;
}

fn ae_body(app: &mut IncaApp, ui: &mut egui::Ui, tabs: &[(String, NodeRef)], tab: usize) {
    let (_, r) = tabs[tab].clone();
    match &r {
        NodeRef::Dag(id) if tab == 0 => {
            let targets = vec![r.clone()];
            if theme::fold(ui, "Transform Attributes") {
                for c in ["translateX", "translateY", "translateZ", "rotateX", "rotateY", "rotateZ", "scaleX", "scaleY", "scaleZ"] {
                    channel_row(app, ui, &targets, c, &ops::nice_name(c), ChanKind::Float, None, None);
                }
                channel_row(app, ui, &targets, "rotateOrder", "Rotate Order", ChanKind::Enum(&inca_core::math::ROTATE_ORDERS), None, None);
            }
            if theme::fold(ui, "Pivots") {
                let p = app.s.world_pivot(id);
                ui.label(format!("World pivot  {:.3} {:.3} {:.3}", p.x, p.y, p.z));
                if ui.button("Center Pivot").clicked() {
                    let _ = app.run_mel("center pivot", &format!("xform -cp {};", app.s.name_of(id)));
                }
            }
            if theme::fold(ui, "Display") {
                channel_row(app, ui, &targets, "visibility", "Visibility", ChanKind::Bool, None, None);
                channel_row(app, ui, &targets, "template", "Template", ChanKind::Bool, None, None);
            }
        }
        NodeRef::Dag(id) => shape_tab(app, ui, id),
        NodeRef::History(id, i) => {
            let ty = app.s.node(id).and_then(|n| n.mesh()).and_then(|m| m.history.get(*i)).map(|h| h.ty.clone()).unwrap_or_default();
            if theme::fold(ui, &format!("{ty} History")) {
                if let Some((_, _, chans)) = ops::info(&ty) {
                    for c in chans {
                        let label = c.label.map(String::from).unwrap_or_else(|| ops::nice_name(c.key));
                        channel_row(app, ui, std::slice::from_ref(&r), c.key, &label, c.kind, c.min, c.max);
                    }
                }
            }
        }
        NodeRef::Material(mid) => crate::windows::material_editor(app, ui, mid),
        NodeRef::Texture(_) => {}
    }
}

fn shape_tab(app: &mut IncaApp, ui: &mut egui::Ui, id: &str) {
    let Some(n) = app.s.node(id) else { return };
    let r = NodeRef::Dag(id.to_string());
    let (kind, extra) = (n.kind.clone(), n.extra.clone());
    if let Some(e) = extra.as_object().filter(|e| !e.is_empty()) {
        let title = match kind.as_str() {
            "emitter" => "Emitter Attributes",
            "field" => "Field Attributes",
            "deformer" => "Deformer Handle",
            _ if e.contains_key("rigidBody") => "Rigid Body Attributes",
            _ => "Extra Attributes",
        };
        if theme::fold(ui, title) {
            let keys: Vec<String> = e.keys().filter(|k| *k != "source").cloned().collect();
            attr_map_editor(app, ui, &r, &keys);
        }
    }
    let Some(n) = app.s.node(id) else { return };
    match &n.shape {
        Shape::Mesh(m) => {
            let (v, f, t) = (m.mesh.v.len(), m.mesh.f.len(), m.mesh.tri_count());
            let mat = m.material.clone();
            if theme::fold(ui, "Mesh") {
                ui.label(format!("{v} vertices, {f} faces, {t} triangles"));
                channel_row(app, ui, std::slice::from_ref(&r), "smoothLevel", "Smooth Mesh Preview", ChanKind::Int, Some(0.0), Some(3.0));
            }
            if !theme::fold(ui, "Shading") {
                return;
            }
            let mut sel = mat.clone();
            egui::ComboBox::from_id_salt("aemat").selected_text(app.s.mats.get(&mat).map(|m| m.name.clone()).unwrap_or_default()).show_ui(ui, |ui| {
                for m in app.s.mats.values() {
                    ui.selectable_value(&mut sel, m.id.clone(), &m.name);
                }
            });
            if sel != mat {
                let mn = app.s.mats[&sel].name.clone();
                let _ = app.run_mel("assign material", &format!("select -r {}; hyperShade -assign {mn};", app.s.name_of(id)));
            }
        }
        Shape::Light(l) => {
            if theme::fold(ui, &format!("{} Attributes", crate::actions::label(&l.ty))) {
                let keys: Vec<String> = l.attrs.keys().cloned().collect();
                attr_map_editor(app, ui, &r, &keys);
            }
        }
        Shape::Camera(_) => {
            if theme::fold(ui, "Camera Attributes") {
                let keys: Vec<String> = ["focalLength", "near", "far", "ortho", "orthoWidth", "coi", "horizontalFilmAperture", "verticalFilmAperture"].iter().map(|s| s.to_string()).collect();
                attr_map_editor(app, ui, &r, &keys);
            }
        }
        Shape::Curve(c) => {
            if theme::fold(ui, "NURBS Curve") {
                ui.label(format!("degree {}, {} CVs, {} spans, {}", c.degree, c.cvs.len(), c.spans(), c.form));
            }
        }
        _ => {}
    }
}

fn tool_settings(app: &mut IncaApp, ui: &mut egui::Ui) {
    theme::section_header(ui, match app.tool {
        Tool::Select => "Select Tool",
        Tool::Lasso => "Lasso Tool",
        Tool::Paint => "Paint Selection Tool",
        Tool::Move => "Move Tool",
        Tool::Rotate => "Rotate Tool",
        Tool::Scale => "Scale Tool",
        Tool::CvCurve => "CV Curve Tool",
        Tool::EpCurve => "EP Curve Tool",
        Tool::Arc => "Three Point Circular Arc",
        Tool::Polygon => "Create Polygon Tool",
        Tool::Joint => "Joint Tool",
        Tool::EdgeLoop => "Insert Edge Loop Tool",
        Tool::MultiCut => "Multi-Cut Tool",
        Tool::TargetWeld => "Target Weld Tool",
        Tool::Sculpt => "Sculpt Tool",
        Tool::QuadDraw => "Quad Draw Tool",
        Tool::Pencil => "Pencil Curve Tool",
    });
    crate::mesh_tools::settings(app, ui);
    if app.tool.is_creation() {
        ui.label("Click in a viewport to place points (on the ground grid in perspective, on the view plane in orthographic views). Enter completes, Backspace removes the last point, Esc cancels.");
        ui.checkbox(&mut app.snap_grid, "Snap to grid");
    }
    if app.tool == Tool::Paint {
        ui.horizontal(|ui| {
            ui.label("Brush radius");
            ui.add(egui::Slider::new(&mut app.paint_radius, 2.0..=200.0));
        });
    }
    if app.tool.is_manip() {
        ui.horizontal(|ui| {
            ui.label("Axis Orientation");
            ui.radio_value(&mut app.world_space, true, "World");
            ui.radio_value(&mut app.world_space, false, "Object");
        });
        ui.checkbox(&mut app.snap_grid, "Discrete / grid snapping");
        ui.horizontal(|ui| {
            ui.label("Step");
            ui.add(egui::DragValue::new(&mut app.vp_state.snap_step).speed(0.05).range(0.01..=100.0));
            ui.label("Angle");
            ui.add(egui::DragValue::new(&mut app.vp_state.snap_angle).speed(0.5).range(1.0..=180.0));
        });
        ui.horizontal(|ui| {
            ui.label("Manipulator size");
            ui.add(egui::Slider::new(&mut app.manip_size, 0.2..=5.0));
        });
    }
    if app.tool.is_manip() && theme::fold(ui, "Soft Selection") {
        ui.checkbox(&mut app.soft.on, "Soft Select (B)");
        ui.horizontal(|ui| {
            ui.label("Falloff radius");
            ui.add(egui::DragValue::new(&mut app.soft.radius).speed(0.02).range(0.001..=1000.0));
        });
        ui.horizontal(|ui| {
            ui.label("Falloff curve");
            ui.radio_value(&mut app.soft.linear, false, "Smooth");
            ui.radio_value(&mut app.soft.linear, true, "Linear");
        });
        ui.label(RichText::new("B + drag in a viewport changes the radius").color(theme::TEXT_DIM));
    }
    ui.separator();
    ui.checkbox(&mut app.history_on, "Construction history");
}
