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
    pub input_select: bool,
    pub quit: bool,
    /// a channel drag is in progress (one undo step per drag)
    pub editing: bool,
    pub ae_tab: usize,
}

fn kind_icon(n: &inca_core::scene::Node) -> &'static str {
    match &n.shape {
        Shape::Mesh(m) if m.nurbs => "◍",
        Shape::Mesh(_) => "▲",
        Shape::Group => "⧉",
        Shape::Light(_) => "☀",
        Shape::Camera(_) => "🎥",
        Shape::Curve(_) => "∿",
        Shape::Locator => "✛",
    }
}

// ------------------------------------------------------------------ outliner

pub fn outliner(app: &mut IncaApp, ui: &mut egui::Ui) {
    theme::section_header(ui, "Outliner");
    ui.horizontal(|ui| {
        ui.label("🔍");
        ui.add(egui::TextEdit::singleline(&mut app.panels.outliner_filter).hint_text("search").desired_width(ui.available_width()));
    });
    ui.separator();
    let filter = app.panels.outliner_filter.to_lowercase();
    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        let roots = app.s.roots.clone();
        // startup cameras first, like Maya
        let (cams, rest): (Vec<_>, Vec<_>) = roots.into_iter().partition(|r| app.s.node(r).is_some_and(|n| n.startup()));
        for r in cams.iter().chain(rest.iter()) {
            outliner_row(app, ui, r, 0, &filter);
        }
        // drop on empty space = unparent
        let (_, zone) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::hover());
        if let Some(id) = zone.dnd_release_payload::<String>().map(|p| (*p).clone()) {
            let n = app.s.name_of(&id);
            let _ = app.run_mel("unparent", &format!("parent -w {n};"));
        }
        // materials
        ui.add_space(6.0);
        ui.label(RichText::new("Materials").color(theme::TEXT_DIM));
        for m in app.s.mats.values() {
            ui.label(RichText::new(format!("  ● {}", m.name)).color(theme::TEXT_DIM));
        }
    });
}

fn matches_filter(app: &IncaApp, id: &str, f: &str) -> bool {
    f.is_empty() || app.s.subtree(id).iter().any(|i| app.s.name_of(i).to_lowercase().contains(f))
}

fn outliner_row(app: &mut IncaApp, ui: &mut egui::Ui, id: &str, depth: usize, filter: &str) {
    if !matches_filter(app, id, filter) {
        return;
    }
    let Some(n) = app.s.node(id) else { return };
    let name = n.name.clone();
    let icon = kind_icon(n);
    let has_kids = !n.children.is_empty();
    let visible = n.visible;
    let selected = app.s.sel.iter().any(|s| s == id);
    let expanded = app.panels.expanded.contains(id) || !filter.is_empty();
    ui.horizontal(|ui| {
        ui.add_space(depth as f32 * 14.0);
        if has_kids {
            if ui.small_button(if expanded { "-" } else { "+" }).clicked() {
                if expanded {
                    app.panels.expanded.remove(id);
                } else {
                    app.panels.expanded.insert(id.to_string());
                }
            }
        } else {
            ui.add_space(16.0);
        }
        if let Some((rid, text)) = &mut app.panels.renaming {
            if rid == id {
                let r = ui.text_edit_singleline(text);
                if r.lost_focus() {
                    let t = text.trim().to_string();
                    app.panels.renaming = None;
                    if !t.is_empty() && t != name {
                        let _ = app.run_mel("rename", &format!("rename {name} \"{t}\";"));
                    }
                } else {
                    r.request_focus();
                }
                return;
            }
        }
        let col = if !visible { theme::TEXT_DIM } else if selected { Color32::WHITE } else { theme::TEXT };
        let text = RichText::new(format!("{icon} {name}")).color(col);
        let resp = ui.dnd_drag_source(egui::Id::new(("outl", id)), id.to_string(), |ui| {
            ui.add(egui::Button::selectable(selected, text).frame_when_inactive(false));
        });
        let r = resp.response.interact(egui::Sense::click());
        if r.clicked() {
            let m = ui.input(|i| i.modifiers);
            let mode = if m.ctrl || m.command {
                SelMode::Toggle
            } else if m.shift {
                SelMode::Add
            } else {
                SelMode::Replace
            };
            app.select_names(&[id.to_string()], mode);
        }
        if r.double_clicked() {
            app.panels.renaming = Some((id.to_string(), name.clone()));
        }
        if let Some(src) = r.dnd_release_payload::<String>() {
            if src.as_str() != id {
                let a = app.s.name_of(&src);
                let _ = app.run_mel("parent", &format!("parent {a} {name};"));
            }
        }
        r.context_menu(|ui| {
            if ui.button("Rename").clicked() {
                app.panels.renaming = Some((id.to_string(), name.clone()));
                ui.close();
            }
            if ui.button(if visible { "Hide" } else { "Show" }).clicked() {
                let _ = app.run_mel("visibility", &format!("{} {name};", if visible { "hide" } else { "showHidden" }));
                ui.close();
            }
            if ui.button("Delete").clicked() {
                let _ = app.run_mel("delete", &format!("delete {name};"));
                ui.close();
            }
        });
    });
    if has_kids && expanded {
        let kids = app.s.node(id).map(|n| n.children.clone()).unwrap_or_default();
        for k in kids {
            outliner_row(app, ui, &k, depth + 1, filter);
        }
    }
}

// ------------------------------------------------------------------ right panel

pub fn right_panel(app: &mut IncaApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for (t, l) in [(RightTab::ChannelBox, "Channel Box"), (RightTab::AttributeEditor, "Attribute Editor"), (RightTab::ToolSettings, "Tool Settings")] {
            if ui.selectable_label(app.right_tab == t, l).clicked() {
                app.right_tab = t;
            }
        }
    });
    ui.separator();
    match app.right_tab {
        RightTab::ChannelBox => {
            let h = ui.available_height();
            egui::ScrollArea::vertical().id_salt("cbscroll").max_height(h * 0.68).auto_shrink([false, false]).show(ui, |ui| channel_box(app, ui));
            ui.separator();
            layer_editor(app, ui);
        }
        RightTab::AttributeEditor => {
            egui::ScrollArea::vertical().id_salt("aescroll").auto_shrink([false, false]).show(ui, |ui| attribute_editor(app, ui));
        }
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
    // SHAPES
    let sh_channels = app.s.shape_channels(&lead);
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

pub fn attribute_editor(app: &mut IncaApp, ui: &mut egui::Ui) {
    let Some(lead) = app.s.lead().cloned() else {
        ui.label(RichText::new("Select an object to see its attributes").color(theme::TEXT_DIM));
        return;
    };
    let Some(n) = app.s.node(&lead) else { return };
    // tabs: transform, shape, history nodes, material
    let mut tabs: Vec<(String, NodeRef)> = vec![(n.name.clone(), NodeRef::Dag(lead.clone()))];
    if let Some(sn) = &n.shape_name {
        tabs.push((sn.clone(), NodeRef::Dag(lead.clone())));
    }
    if let Some(m) = n.mesh() {
        for (i, h) in m.history.iter().enumerate().rev() {
            if !h.name.contains('#') {
                tabs.push((h.name.clone(), NodeRef::History(lead.clone(), i)));
            }
        }
        if app.s.mats.contains_key(&m.material) {
            tabs.push((app.s.mats[&m.material].name.clone(), NodeRef::Material(m.material.clone())));
        }
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
    ui.separator();
    let (title, r) = tabs[tab].clone();
    ui.label(RichText::new(&title).heading());
    match &r {
        NodeRef::Dag(id) if tab == 0 => {
            theme::section_header(ui, "Transform Attributes");
            let targets = vec![r.clone()];
            for c in ["translateX", "translateY", "translateZ", "rotateX", "rotateY", "rotateZ", "scaleX", "scaleY", "scaleZ"] {
                channel_row(app, ui, &targets, c, &ops::nice_name(c), ChanKind::Float, None, None);
            }
            channel_row(app, ui, &targets, "rotateOrder", "Rotate Order", ChanKind::Enum(&inca_core::math::ROTATE_ORDERS), None, None);
            theme::section_header(ui, "Pivots");
            let p = app.s.world_pivot(id);
            ui.label(format!("World pivot  {:.3} {:.3} {:.3}", p.x, p.y, p.z));
            if ui.button("Center Pivot").clicked() {
                let _ = app.run_mel("center pivot", &format!("xform -cp {};", app.s.name_of(id)));
            }
            theme::section_header(ui, "Display");
            channel_row(app, ui, &targets, "visibility", "Visibility", ChanKind::Bool, None, None);
            channel_row(app, ui, &targets, "template", "Template", ChanKind::Bool, None, None);
        }
        NodeRef::Dag(id) => shape_tab(app, ui, id),
        NodeRef::History(id, i) => {
            let ty = app.s.node(id).and_then(|n| n.mesh()).map(|m| m.history[*i].ty.clone()).unwrap_or_default();
            ui.label(RichText::new(format!("type: {ty}")).color(theme::TEXT_DIM));
            if let Some((_, _, chans)) = ops::info(&ty) {
                for c in chans {
                    let label = c.label.map(String::from).unwrap_or_else(|| ops::nice_name(c.key));
                    channel_row(app, ui, std::slice::from_ref(&r), c.key, &label, c.kind, c.min, c.max);
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
    match &n.shape {
        Shape::Mesh(m) => {
            let (v, f, t) = (m.mesh.v.len(), m.mesh.f.len(), m.mesh.tri_count());
            let mat = m.material.clone();
            theme::section_header(ui, "Mesh");
            ui.label(format!("{v} vertices, {f} faces, {t} triangles"));
            channel_row(app, ui, std::slice::from_ref(&r), "smoothLevel", "Smooth Mesh Preview", ChanKind::Int, Some(0.0), Some(3.0));
            theme::section_header(ui, "Shading");
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
            theme::section_header(ui, &format!("{} Attributes", crate::actions::label(&l.ty)));
            let keys: Vec<String> = l.attrs.keys().cloned().collect();
            attr_map_editor(app, ui, &r, &keys);
        }
        Shape::Camera(_) => {
            theme::section_header(ui, "Camera Attributes");
            let keys: Vec<String> = ["focalLength", "near", "far", "ortho", "orthoWidth", "coi", "horizontalFilmAperture", "verticalFilmAperture"].iter().map(|s| s.to_string()).collect();
            attr_map_editor(app, ui, &r, &keys);
        }
        Shape::Curve(c) => {
            theme::section_header(ui, "NURBS Curve");
            ui.label(format!("degree {}, {} CVs, {} spans, {}", c.degree, c.cvs.len(), c.spans(), c.form));
        }
        _ => {}
    }
}

fn tool_settings(app: &mut IncaApp, ui: &mut egui::Ui) {
    theme::section_header(ui, match app.tool {
        Tool::Select => "Select Tool",
        Tool::Move => "Move Tool",
        Tool::Rotate => "Rotate Tool",
        Tool::Scale => "Scale Tool",
        Tool::CvCurve => "CV Curve Tool",
        Tool::EpCurve => "EP Curve Tool",
        Tool::Arc => "Three Point Circular Arc",
        Tool::Polygon => "Create Polygon Tool",
        Tool::Joint => "Joint Tool",
    });
    if app.tool.is_creation() {
        ui.label("Click in a viewport to place points (on the ground grid in perspective, on the view plane in orthographic views). Enter completes, Backspace removes the last point, Esc cancels.");
        ui.checkbox(&mut app.snap_grid, "Snap to grid");
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
    ui.separator();
    ui.checkbox(&mut app.history_on, "Construction history");
}
