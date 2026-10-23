//! Option boxes (the ▢ next to menu items) and their dialogs, from the JS command `options` specs.

use crate::app::IncaApp;
use std::collections::BTreeMap;

pub enum Kind {
    Float,
    Int,
    Bool,
    Radio(&'static [&'static str]),
}

pub struct Opt {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    pub default: f64,
}

const fn o(key: &'static str, label: &'static str, kind: Kind, default: f64) -> Opt {
    Opt { key, label, kind, default }
}

const XYZ: &[&str] = &["X", "Y", "Z"];

/// action -> options (Duplicate Special keeps its own window).
pub const SPECS: &[(&str, &[Opt])] = &[
    ("smooth", &[o("divisions", "Division levels", Kind::Int, 1.0)]),
    ("mirror", &[o("axis", "Mirror axis", Kind::Radio(XYZ), 0.0), o("merge", "Merge border vertices", Kind::Bool, 1.0), o("threshold", "Merge threshold", Kind::Float, 0.001)]),
    ("addDivisions", &[o("divisions", "Subdivision levels", Kind::Int, 1.0)]),
    ("extrude", &[o("thickness", "Thickness", Kind::Float, 0.5), o("offset", "Offset", Kind::Float, 0.0), o("divisions", "Divisions", Kind::Int, 1.0), o("keepFacesTogether", "Keep faces together", Kind::Bool, 1.0)]),
    ("bridge", &[o("divisions", "Divisions", Kind::Int, 0.0)]),
    ("bevel", &[o("fraction", "Fraction", Kind::Float, 0.5)]),
    ("merge", &[o("threshold", "Threshold", Kind::Float, 0.001)]),
    ("planarMap", &[o("axis", "Project from", Kind::Radio(&["X axis", "Y axis", "Z axis"]), 1.0)]),
    ("revolve", &[o("axis", "Axis", Kind::Radio(XYZ), 1.0), o("startSweep", "Start sweep angle", Kind::Float, 0.0), o("endSweep", "End sweep angle", Kind::Float, 360.0), o("sections", "Segments", Kind::Int, 8.0)]),
    ("group", &[o("pivot", "Group pivot", Kind::Radio(&["Center", "Origin"]), 0.0)]),
    ("chamferVertex", &[o("width", "Width", Kind::Float, 0.25)]),
    ("smoothBind", &[o("maxInfluences", "Max influences", Kind::Int, 3.0)]),
    ("mirrorJoint", &[o("axis", "Mirror across", Kind::Radio(&["YZ", "XZ", "XY"]), 0.0)]),
    ("softModDeformer", &[o("falloffRadius", "Falloff radius", Kind::Float, 5.0)]),
    ("createLattice", &[o("s", "S divisions", Kind::Int, 2.0), o("t", "T divisions", Kind::Int, 5.0), o("u", "U divisions", Kind::Int, 2.0)]),
    ("createGhost", &[o("steps", "Steps before/after", Kind::Int, 3.0), o("every", "Frame step (0 = keys)", Kind::Int, 0.0)]),
    ("attachToMotionPath", &[o("follow", "Follow", Kind::Bool, 1.0)]),
];

pub fn spec(action: &str) -> Option<&'static [Opt]> {
    SPECS.iter().find(|s| s.0 == action).map(|s| s.1)
}

pub fn has_box(action: &str) -> bool {
    spec(action).is_some() || action == "duplicateSpecial"
}

/// The value of an option (saved, or the spec default).
pub fn get(app: &IncaApp, action: &str, key: &str) -> f64 {
    app.options.get(action).and_then(|m| m.get(key)).copied().or_else(|| spec(action).and_then(|s| s.iter().find(|o| o.key == key)).map(|o| o.default)).unwrap_or(0.0)
}

pub type Values = BTreeMap<String, BTreeMap<String, f64>>;

pub fn open(app: &mut IncaApp, action: &str) {
    if action == "duplicateSpecial" {
        app.dup_special_open = true;
    } else if spec(action).is_some() {
        app.option_dialog = Some(action.to_string());
    }
}

/// "<Action> Options" window: Apply, Apply and Close, Reset, Close.
pub fn dialog(app: &mut IncaApp, ctx: &egui::Context) {
    let Some(action) = app.option_dialog.clone() else { return };
    let Some(sp) = spec(&action) else {
        app.option_dialog = None;
        return;
    };
    let mut open = true;
    let (mut apply, mut close, mut reset) = (false, false, false);
    let mut vals: BTreeMap<String, f64> = sp.iter().map(|o| (o.key.to_string(), get(app, &action, o.key))).collect();
    egui::Window::new(format!("{} Options", crate::actions::label(&action))).id(egui::Id::new(("optbox", &action))).open(&mut open).resizable(false).show(ctx, |ui| {
        egui::Grid::new("opts").num_columns(2).show(ui, |ui| {
            for o in sp {
                ui.label(o.label);
                let v = vals.get_mut(o.key).unwrap();
                match o.kind {
                    Kind::Float => {
                        ui.add(egui::DragValue::new(v).speed(0.01).max_decimals(4));
                    }
                    Kind::Int => {
                        let mut i = *v as i64;
                        if ui.add(egui::DragValue::new(&mut i).range(0..=1000)).changed() {
                            *v = i as f64;
                        }
                    }
                    Kind::Bool => {
                        let mut b = *v != 0.0;
                        if ui.checkbox(&mut b, "").changed() {
                            *v = b as i32 as f64;
                        }
                    }
                    Kind::Radio(labels) => {
                        ui.horizontal(|ui| {
                            for (i, l) in labels.iter().enumerate() {
                                if ui.radio(*v as usize == i, *l).clicked() {
                                    *v = i as f64;
                                }
                            }
                        });
                    }
                }
                ui.end_row();
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(format!("{} and Close", crate::actions::label(&action))).clicked() {
                apply = true;
                close = true;
            }
            apply |= ui.button("Apply").clicked();
            reset = ui.button("Reset").clicked();
            close |= ui.button("Close").clicked();
        });
    });
    if reset {
        vals = sp.iter().map(|o| (o.key.to_string(), o.default)).collect();
    }
    app.options.insert(action.clone(), vals);
    if apply {
        app.action(&action);
    }
    if close || !open {
        app.option_dialog = None;
    }
}
