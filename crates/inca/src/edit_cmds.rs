//! Edit / Select menu commands that need more than one MEL line or a dialog
//! (src/js/core/commands.js, src/js/ui/extraCommands.js).

use crate::app::IncaApp;
use inca_core::scene::{SelMode, Shape};

/// What a text prompt does with its answer.
#[derive(Clone)]
pub enum PromptKind {
    SearchFor,
    ReplaceWith(String),
    Prefix,
    SelectByName,
}

pub struct Prompt {
    pub kind: PromptKind,
    pub title: &'static str,
    pub label: &'static str,
    pub text: String,
}

/// Duplicate Special options (the JS option box).
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DupOpts {
    pub keep_history: bool,
    pub translate: [f64; 3],
    pub rotate: [f64; 3],
    pub scale: [f64; 3],
    pub copies: u32,
}

impl Default for DupOpts {
    fn default() -> Self {
        DupOpts { keep_history: false, translate: [0.0; 3], rotate: [0.0; 3], scale: [1.0; 3], copies: 1 }
    }
}

pub fn ask(app: &mut IncaApp, kind: PromptKind) {
    let (title, label, text) = match &kind {
        PromptKind::SearchFor => ("Search and Replace", "Search for:", String::new()),
        PromptKind::ReplaceWith(_) => ("Search and Replace", "Replace with:", String::new()),
        PromptKind::Prefix => ("Prefix Hierarchy", "Prefix:", "pre_".to_string()),
        PromptKind::SelectByName => ("Select by Name", "Name (wildcards *):", "*".to_string()),
    };
    app.prompt = Some(Prompt { kind, title, label, text });
}

/// Draw the open prompt; runs its command on OK.
pub fn prompt_window(app: &mut IncaApp, ctx: &egui::Context) {
    let Some(p) = app.prompt.as_mut() else { return };
    let mut done: Option<bool> = None;
    egui::Window::new(p.title).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, -80.0]).show(ctx, |ui| {
        ui.label(p.label);
        let r = ui.add(egui::TextEdit::singleline(&mut p.text).desired_width(280.0));
        r.request_focus();
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            done = Some(true);
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            done = Some(false);
        }
        ui.horizontal(|ui| {
            if ui.button("OK").clicked() {
                done = Some(true);
            }
            if ui.button("Cancel").clicked() {
                done = Some(false);
            }
        });
    });
    match done {
        Some(true) => {
            let p = app.prompt.take().unwrap();
            answer(app, p.kind, p.text);
        }
        Some(false) => app.prompt = None,
        None => {}
    }
}

fn answer(app: &mut IncaApp, kind: PromptKind, text: String) {
    match kind {
        PromptKind::SearchFor if !text.is_empty() => ask(app, PromptKind::ReplaceWith(text)),
        PromptKind::ReplaceWith(find) => search_replace(app, &find, &text),
        PromptKind::Prefix if !text.is_empty() => prefix_hierarchy(app, &text),
        PromptKind::SelectByName if !text.is_empty() => select_by_name(app, &text),
        _ => {}
    }
}

/// Non-startup DAG nodes, in outliner order.
fn scene_nodes(app: &IncaApp) -> Vec<String> {
    app.s.all_dag(false).into_iter().filter(|i| app.s.node(i).is_some_and(|n| !n.startup())).collect()
}

pub fn search_replace(app: &mut IncaApp, find: &str, with: &str) {
    let pool = if app.s.sel.is_empty() { scene_nodes(app) } else { app.s.sel.clone() };
    let mel: String = pool
        .iter()
        .map(|i| app.s.name_of(i))
        .filter(|n| n.contains(find))
        .map(|n| format!("rename {n} \"{}\";\n", n.replace(find, with)))
        .collect();
    if !mel.is_empty() {
        let _ = app.run_mel("search and replace", &mel);
    }
}

pub fn prefix_hierarchy(app: &mut IncaApp, prefix: &str) {
    let mut ids = vec![];
    for s in app.s.sel.clone() {
        for i in app.s.subtree(&s) {
            if !ids.contains(&i) && app.s.node(&i).is_some_and(|n| !n.startup()) {
                ids.push(i);
            }
        }
    }
    // rename by id order so parents and children don't fight over names
    let mel: String = ids.iter().map(|i| { let n = app.s.name_of(i); format!("rename {n} \"{prefix}{n}\";\n") }).collect();
    if !mel.is_empty() {
        let _ = app.run_mel("prefix hierarchy", &mel);
    }
}

/// `*` and `?` wildcards, whole-name match.
pub fn glob(pat: &str, s: &str) -> bool {
    fn m(p: &[char], s: &[char]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some('*'), _) => m(&p[1..], s) || (!s.is_empty() && m(p, &s[1..])),
            (Some('?'), Some(_)) => m(&p[1..], &s[1..]),
            (Some(a), Some(b)) if a == b => m(&p[1..], &s[1..]),
            _ => false,
        }
    }
    m(&pat.chars().collect::<Vec<_>>(), &s.chars().collect::<Vec<_>>())
}

pub fn select_by_name(app: &mut IncaApp, pat: &str) {
    let ids: Vec<String> = app.s.all_dag(false).into_iter().filter(|i| glob(pat, &app.s.name_of(i))).collect();
    app.select_names(&ids, SelMode::Replace);
}

/// Select > All by Type.
pub fn select_by_type(app: &mut IncaApp, ty: &str) {
    let ids: Vec<String> = scene_nodes(app)
        .into_iter()
        .filter(|i| {
            let n = app.s.node(i).unwrap();
            match ty {
                "geometry" => matches!(n.shape, Shape::Mesh(_)),
                "light" => matches!(n.shape, Shape::Light(_)),
                "camera" => matches!(n.shape, Shape::Camera(_)),
                _ => matches!(n.shape, Shape::Curve(_)),
            }
        })
        .collect();
    app.select_names(&ids, SelMode::Replace);
}

/// Edit > Delete All by Type (lights / cameras / curves).
pub fn delete_by_type(app: &mut IncaApp, ty: &str) {
    let names: Vec<String> = scene_nodes(app)
        .into_iter()
        .filter(|i| {
            let n = app.s.node(i).unwrap();
            match ty {
                "light" => matches!(n.shape, Shape::Light(_)),
                "camera" => matches!(n.shape, Shape::Camera(_)),
                _ => matches!(n.shape, Shape::Curve(_)),
            }
        })
        .map(|i| app.s.name_of(&i))
        .collect();
    if !names.is_empty() {
        let _ = app.run_mel(&format!("delete {ty}s"), &format!("delete {};", names.join(" ")));
    }
}

/// Select > Similar: visible objects of the lead's kind (and light type).
pub fn select_similar(app: &mut IncaApp) {
    let Some(lead) = app.s.lead().cloned() else { return };
    let key = |app: &IncaApp, i: &str| {
        let n = app.s.node(i).unwrap();
        let sub = match &n.shape {
            Shape::Light(l) => l.ty.clone(),
            Shape::Mesh(_) => "mesh".into(),
            Shape::Curve(_) => "curve".into(),
            Shape::Camera(_) => "camera".into(),
            Shape::Locator => "locator".into(),
            Shape::Group => "group".into(),
        };
        (n.kind.clone(), sub)
    };
    let k = key(app, &lead);
    let ids: Vec<String> = scene_nodes(app).into_iter().filter(|i| app.s.is_visible(i) && key(app, i) == k).collect();
    app.select_names(&ids, SelMode::Replace);
}

fn xf_of(app: &IncaApp, id: &str) -> Option<([f64; 3], [f64; 3], [f64; 3])> {
    app.s.node(id).map(|n| (n.xf.t, n.xf.r, n.xf.s))
}

fn set_xf_mel(name: &str, t: [f64; 3], r: [f64; 3], s: [f64; 3]) -> String {
    let f = inca_core::math::fmt_num_long;
    format!(
        "setAttr {name}.translate {} {} {}; setAttr {name}.rotate {} {} {}; setAttr {name}.scale {} {} {};\n",
        f(t[0]), f(t[1]), f(t[2]), f(r[0]), f(r[1]), f(r[2]), f(s[0]), f(s[1]), f(s[2])
    )
}

/// Duplicate Special: `copies` duplicates, each offset from the previous by the option transform.
pub fn duplicate_special(app: &mut IncaApp, o: &DupOpts) {
    if app.s.sel.is_empty() {
        app.last_result = "// Warning: Nothing selected".into();
        return;
    }
    app.s.checkpoint("duplicate special");
    let mut cur = app.s.sel.clone();
    for _ in 0..o.copies.max(1) {
        let names: Vec<String> = cur.iter().map(|i| app.s.name_of(i)).collect();
        let before: std::collections::HashSet<String> = app.s.nodes.keys().cloned().collect();
        let _ = app.exec_mel(&format!("select -r {}; duplicate -rr{};", names.join(" "), if o.keep_history { " -un" } else { "" }), false);
        let out = app.s.sel.clone();
        let mut mel = String::new();
        for id in out.iter().filter(|i| !before.contains(*i)) {
            if let Some((t, r, s)) = xf_of(app, id) {
                let t = [t[0] + o.translate[0], t[1] + o.translate[1], t[2] + o.translate[2]];
                let r = [r[0] + o.rotate[0], r[1] + o.rotate[1], r[2] + o.rotate[2]];
                let s = [s[0] * o.scale[0], s[1] * o.scale[1], s[2] * o.scale[2]];
                mel += &set_xf_mel(&app.s.name_of(id), t, r, s);
            }
        }
        let _ = app.exec_mel(&mel, false);
        cur = out;
    }
    app.echo(format!("duplicate -rr; // x{}", o.copies.max(1)));
}

/// Shift+D: duplicate, repeating the transform offset between the last duplicate and its source.
pub fn duplicate_with_transform(app: &mut IncaApp) {
    let src = app.s.sel.clone();
    if src.is_empty() {
        app.last_result = "// Warning: Nothing selected".into();
        return;
    }
    // the delta the user applied to the previous duplicate, if they are duplicating it again
    let delta: Option<Vec<_>> = app.last_dup.as_ref().filter(|(_, out)| *out == src).map(|(prev, out)| {
        out.iter()
            .enumerate()
            .map(|(i, o)| {
                let s = prev.get(i).or(prev.first()).and_then(|p| xf_of(app, p));
                match (s, xf_of(app, o)) {
                    (Some((st, sr, ss)), Some((ot, or, os))) => ([ot[0] - st[0], ot[1] - st[1], ot[2] - st[2]], [or[0] - sr[0], or[1] - sr[1], or[2] - sr[2]], [os[0] / nz(ss[0]), os[1] / nz(ss[1]), os[2] / nz(ss[2])]),
                    _ => ([0.0; 3], [0.0; 3], [1.0; 3]),
                }
            })
            .collect()
    });
    app.s.checkpoint("duplicate with transform");
    let names: Vec<String> = src.iter().map(|i| app.s.name_of(i)).collect();
    let _ = app.exec_mel(&format!("select -r {}; duplicate -rr;", names.join(" ")), false);
    let out = app.s.sel.clone();
    if let Some(d) = delta {
        let mut mel = String::new();
        for (i, id) in out.iter().enumerate() {
            let (dt, dr, ds) = d.get(i).or(d.first()).copied().unwrap_or(([0.0; 3], [0.0; 3], [1.0; 3]));
            if let Some((t, r, s)) = xf_of(app, id) {
                mel += &set_xf_mel(&app.s.name_of(id), [t[0] + dt[0], t[1] + dt[1], t[2] + dt[2]], [r[0] + dr[0], r[1] + dr[1], r[2] + dr[2]], [s[0] * ds[0], s[1] * ds[1], s[2] * ds[2]]);
            }
        }
        let _ = app.exec_mel(&mel, false);
    }
    app.echo("duplicate -rr -st;");
    app.last_dup = Some((src, out));
}

fn nz(v: f64) -> f64 {
    if v.abs() < 1e-12 {
        1.0
    } else {
        v
    }
}

/// Edit > Keys > Bake Simulation: key every frame of the playback range on the selection.
pub fn bake(app: &mut IncaApp) {
    if app.s.sel.is_empty() {
        app.last_result = "// Warning: Nothing selected".into();
        return;
    }
    app.s.checkpoint("bake");
    let (t0, t1, cur) = (app.s.time.start, app.s.time.end, app.s.time.current);
    let mut t = t0;
    while t <= t1 {
        app.s.set_time(t);
        let _ = app.exec_mel("setKeyframe;", false);
        t += 1.0;
    }
    app.s.set_time(cur);
    app.echo(format!("bakeResults -t \"{}:{}\";", t0, t1));
}

/// Edit > Duplicate Special option box.
pub fn dup_special_window(app: &mut IncaApp, ctx: &egui::Context) {
    if !app.dup_special_open {
        return;
    }
    let mut open = true;
    let mut run = false;
    let o = &mut app.dup_opts;
    egui::Window::new("Duplicate Special Options").open(&mut open).resizable(false).show(ctx, |ui| {
        egui::Grid::new("dupopts").num_columns(2).show(ui, |ui| {
            ui.label("Duplicate input graph");
            ui.checkbox(&mut o.keep_history, "");
            ui.end_row();
            for (l, v, sp) in [("Translate", &mut o.translate, 0.1), ("Rotate", &mut o.rotate, 1.0), ("Scale", &mut o.scale, 0.05)] {
                ui.label(l);
                ui.horizontal(|ui| {
                    for x in v.iter_mut() {
                        ui.add(egui::DragValue::new(x).speed(sp).max_decimals(3));
                    }
                });
                ui.end_row();
            }
            ui.label("Number of copies");
            ui.add(egui::DragValue::new(&mut o.copies).range(1..=1000));
            ui.end_row();
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Duplicate Special").clicked() {
                run = true;
            }
            if ui.button("Reset").clicked() {
                *o = DupOpts::default();
            }
        });
    });
    if run {
        let o = app.dup_opts.clone();
        duplicate_special(app, &o);
    }
    if !open {
        app.dup_special_open = false;
    }
}

/// Heads Up Display options (JS prefs.hud).
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Hud {
    pub poly_count: bool,
    pub fps: bool,
    pub frame: bool,
    pub cam_names: bool,
    pub view_axis: bool,
}

impl Default for Hud {
    fn default() -> Self {
        Hud { poly_count: false, fps: false, frame: false, cam_names: true, view_axis: true }
    }
}

fn names(app: &IncaApp, ids: &[String]) -> String {
    ids.iter().map(|i| app.s.name_of(i)).collect::<Vec<_>>().join(" ")
}

/// Display > Hide > Hide Unselected Objects: hide every top-level object without a selected node inside.
pub fn hide_unselected(app: &mut IncaApp) {
    let hide: Vec<String> = app
        .s
        .roots
        .clone()
        .into_iter()
        .filter(|r| app.s.node(r).is_some_and(|n| !n.startup() && n.visible))
        .filter(|r| !app.s.subtree(r).iter().any(|i| app.s.sel.contains(i)))
        .collect();
    if !hide.is_empty() {
        let _ = app.run_mel("hide unselected", &format!("hide {};", names(app, &hide)));
    }
    app.last_hidden = hide;
}

/// Display > Hide / Show > Lights / Cameras / All Geometry.
pub fn show_type(app: &mut IncaApp, ty: &str, vis: bool) {
    let ids: Vec<String> = scene_nodes(app)
        .into_iter()
        .filter(|i| {
            let n = app.s.node(i).unwrap();
            match ty {
                "light" => matches!(n.shape, Shape::Light(_)),
                "camera" => matches!(n.shape, Shape::Camera(_)),
                _ => matches!(n.shape, Shape::Mesh(_) | Shape::Curve(_)),
            }
        })
        .collect();
    if ids.is_empty() {
        return;
    }
    if !vis {
        app.last_hidden = ids.clone();
    }
    let _ = app.run_mel(if vis { "show" } else { "hide" }, &format!("{} {};", if vis { "showHidden" } else { "hide" }, names(app, &ids)));
}

pub fn show_last_hidden(app: &mut IncaApp) {
    let ids: Vec<String> = app.last_hidden.iter().filter(|i| app.s.node(i).is_some()).cloned().collect();
    if !ids.is_empty() {
        let _ = app.run_mel("show last hidden", &format!("showHidden {};", names(app, &ids)));
    }
}

/// Display > Object Display > Template / Untemplate / Untemplate All.
pub fn template(app: &mut IncaApp, on: bool, all: bool) {
    let ids: Vec<String> = if all { scene_nodes(app).into_iter().filter(|i| app.s.is_template(i)).collect() } else { app.s.sel.clone() };
    if ids.is_empty() {
        return;
    }
    let mel: String = ids.iter().map(|i| format!("setAttr {}.template {};\n", app.s.name_of(i), on as i32)).collect();
    let _ = app.run_mel(if on { "template" } else { "untemplate" }, &mel);
    if on {
        app.select_names(&[], SelMode::Replace);
    }
}

/// Face Normals / Border Edges: toggle the overlay on the selected meshes.
pub fn toggle_overlay(app: &mut IncaApp, normals: bool) {
    let meshes: Vec<String> = app.s.sel.iter().filter(|i| app.s.node(i).is_some_and(|n| n.mesh().is_some())).cloned().collect();
    let set = if normals { &mut app.overlay_normals } else { &mut app.overlay_border };
    if meshes.is_empty() {
        set.clear();
        return;
    }
    let on = !meshes.iter().all(|m| set.contains(m));
    for m in meshes {
        if on {
            set.insert(m);
        } else {
            set.remove(&m);
        }
    }
}

/// Modify > Add Attribute dialog state.
pub struct AddAttr {
    pub name: String,
    pub ty: usize,
    pub default: f64,
}

pub const ATTR_TYPES: &[(&str, &str)] = &[("Float", "double"), ("Integer", "long"), ("Boolean", "bool"), ("Vector", "double3"), ("String", "string")];

pub fn add_attr_window(app: &mut IncaApp, ctx: &egui::Context) {
    let Some(w) = app.add_attr.as_mut() else { return };
    let mut open = true;
    let (mut add, mut close) = (false, false);
    egui::Window::new("Add Attribute").open(&mut open).resizable(false).show(ctx, |ui| {
        egui::Grid::new("addattr").num_columns(2).show(ui, |ui| {
            ui.label("Long name:");
            ui.text_edit_singleline(&mut w.name);
            ui.end_row();
            ui.label("Data type:");
            ui.horizontal(|ui| {
                for (i, (l, _)) in ATTR_TYPES.iter().enumerate() {
                    ui.radio_value(&mut w.ty, i, *l);
                }
            });
            ui.end_row();
            ui.label("Default:");
            ui.add_enabled(ATTR_TYPES[w.ty].1 != "string", egui::DragValue::new(&mut w.default).speed(0.1));
            ui.end_row();
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("OK").clicked() {
                add = true;
                close = true;
            }
            add |= ui.button("Add").clicked();
            close |= ui.button("Cancel").clicked();
        });
    });
    if add {
        let name: String = w.name.trim().chars().filter(|c| c.is_alphanumeric() || *c == '_').collect();
        let (ty, dv) = (ATTR_TYPES[w.ty].1, w.default);
        let targets: Vec<String> = app.s.sel.iter().map(|i| app.s.name_of(i)).collect();
        if name.is_empty() || targets.is_empty() {
            app.last_result = "// Warning: Give the attribute a name and select an object".into();
        } else {
            let flag = if ty == "string" { "-dt string".to_string() } else { format!("-at {ty} -dv {}", inca_core::math::fmt_num_long(dv)) };
            let _ = app.run_mel("add attribute", &format!("addAttr -ln {name} {flag} {};", targets.join(" ")));
        }
    }
    if close || !open {
        app.add_attr = None;
    }
}

#[cfg(test)]
mod tests {
    use super::glob;

    #[test]
    fn wildcards() {
        assert!(glob("*", "pCube1"));
        assert!(glob("pCube*", "pCube12"));
        assert!(glob("p?ube1", "pCube1"));
        assert!(!glob("pCube", "pCube1"));
        assert!(glob("*Light*", "pointLight1"));
    }
}
