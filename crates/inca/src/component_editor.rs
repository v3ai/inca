//! Component Editor: vertex positions and skin weights of the selected vertices, editable in a table.

use crate::app::IncaApp;
use crate::theme;
use egui::RichText;
use glam::DVec3;
use inca_core::params::Params;
use serde_json::json;

#[derive(Default)]
pub struct CeState {
    pub tab: usize,
}

pub fn window(app: &mut IncaApp, ctx: &egui::Context) {
    let mut open = app.win.component_editor;
    if !open {
        return;
    }
    egui::Window::new("Component Editor").open(&mut open).default_size([460.0, 380.0]).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.ce.tab, 0, "Polygons");
            ui.selectable_value(&mut app.ce.tab, 1, "Smooth Skins");
        });
        ui.separator();
        let objs = app.s.hilite();
        let rows: Vec<(String, u32)> = objs.iter().flat_map(|o| app.s.comp_verts(o).into_iter().map(move |v| (o.clone(), v))).take(500).collect();
        if rows.is_empty() {
            ui.label(RichText::new("Select vertices (or other components) to list them here.").color(theme::TEXT_DIM));
            return;
        }
        egui::ScrollArea::both().show(ui, |ui| match app.ce.tab {
            0 => positions(app, ui, &rows),
            _ => weights(app, ui, &rows),
        });
    });
    app.win.component_editor = open;
}

fn positions(app: &mut IncaApp, ui: &mut egui::Ui, rows: &[(String, u32)]) {
    let mut edit: Option<(String, u32, DVec3)> = None;
    egui::Grid::new("cepos").striped(true).num_columns(4).show(ui, |ui| {
        ui.label(RichText::new("vertex").strong());
        for a in ["X", "Y", "Z"] {
            ui.label(RichText::new(a).strong());
        }
        ui.end_row();
        for (o, v) in rows {
            let Some(p) = app.s.node(o).and_then(|n| n.mesh()).and_then(|m| m.mesh.v.get(*v as usize)).copied() else { continue };
            ui.label(format!("{}.vtx[{v}]", app.s.name_of(o)));
            let mut q = p;
            for k in 0..3 {
                let r = ui.add(egui::DragValue::new(&mut q[k]).speed(0.01).max_decimals(4));
                if r.drag_started() || (r.gained_focus() && !r.dragged()) {
                    app.s.checkpoint("component editor");
                }
            }
            if q != p {
                edit = Some((o.clone(), *v, q - p));
            }
            ui.end_row();
        }
    });
    if let Some((o, v, d)) = edit {
        // a tweak on top of the mesh (history keeps working)
        let mut p = Params::new();
        p.insert("verts".into(), json!([v]));
        p.insert("deltas".into(), json!([d.to_array()]));
        app.s.apply_op(&o, "polyTweak", p, app.history_on);
    }
}

fn weights(app: &mut IncaApp, ui: &mut egui::Ui, rows: &[(String, u32)]) {
    let Some(o) = rows.first().map(|r| r.0.clone()) else { return };
    let Some(h) = app.s.node(&o).and_then(|n| n.mesh()).and_then(|m| m.history.iter().position(|x| x.ty == "skinCluster")) else {
        ui.label(RichText::new("The selected vertices are not skinned.").color(theme::TEXT_DIM));
        return;
    };
    let params = app.s.node(&o).unwrap().mesh().unwrap().history[h].params.clone();
    let joints: Vec<String> = params.get("joints").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|j| j.as_str().map(String::from)).collect()).unwrap_or_default();
    let mut w = params.get("weights").cloned().unwrap_or(json!([]));
    let mut changed = false;
    egui::Grid::new("cew").striped(true).num_columns(joints.len() + 2).show(ui, |ui| {
        ui.label(RichText::new("vertex").strong());
        for j in &joints {
            ui.label(RichText::new(app.s.name_of(j)).strong());
        }
        ui.label(RichText::new("Total").strong());
        ui.end_row();
        for (ro, v) in rows.iter().filter(|r| r.0 == o) {
            ui.label(format!("{}.vtx[{v}]", app.s.name_of(ro)));
            let mut tot = 0.0;
            for ji in 0..joints.len() {
                let cur = crate::rig_tools::weight_of(&w[*v as usize], ji);
                let mut x = cur;
                let r = ui.add(egui::DragValue::new(&mut x).speed(0.005).range(0.0..=1.0).max_decimals(3));
                if r.drag_started() {
                    app.s.checkpoint("component editor weights");
                }
                if (x - cur).abs() > 1e-9 {
                    crate::rig_tools::set_weight(&mut w[*v as usize], ji, x);
                    changed = true;
                }
                tot += crate::rig_tools::weight_of(&w[*v as usize], ji);
            }
            ui.label(inca_core::math::fmt_num(tot));
            ui.end_row();
        }
    });
    if changed {
        app.s.set_history_param(&o, h, "weights", w);
    }
}
