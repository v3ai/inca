//! Preferences window (src/js/ui/prefsWindow.js): categories on the left, settings on the right.
//! Settings that live elsewhere in the app (background, HUD, manipulator size, time range, undo,
//! autosave, UI elements) are edited in place; the rest are kept here and saved with the app state.

use crate::app::{IncaApp, MenuSet};
use crate::theme;
use egui::RichText;

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub ui_scale: f32,
    pub menu_set: usize,
    pub tooltips: bool,
    pub confirm_quit: bool,
    pub ortho_tumble: bool,
    pub grid_size: f64,
    pub grid_spacing: f64,
    pub grid_major: i32,
    pub tangent_in: String,
    pub tangent_out: String,
    /// 0 = play every frame, 1 = real-time, 2 = half, 3 = twice
    pub playback_speed: usize,
    pub linear_unit: usize,
    pub angular_unit: usize,
    pub recent_count: usize,
    pub recent_projects_count: usize,
    pub autosave_limit: usize,
    pub incremental_save: bool,
    /// red pre-selection highlight of the component under the cursor
    pub preselect: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { ui_scale: 1.0, menu_set: 0, tooltips: true, confirm_quit: true, ortho_tumble: false, grid_size: 12.0, grid_spacing: 1.0, grid_major: 5, tangent_in: "auto".into(), tangent_out: "auto".into(), playback_speed: 1, linear_unit: 1, angular_unit: 0, recent_count: 10, recent_projects_count: 5, autosave_limit: 10, incremental_save: false, preselect: true }
    }
}

pub const CATEGORIES: &[&str] = &["Interface", "UI Elements", "Display", "Kinematics / Manipulators", "Animation", "Time Slider", "Modeling", "Settings", "Undo", "Files/Projects", "Save"];
const MENU_SETS: [(&str, MenuSet); 5] = [("Modeling", MenuSet::Modeling), ("Rigging", MenuSet::Rigging), ("Animation", MenuSet::Animation), ("FX", MenuSet::Fx), ("Rendering", MenuSet::Rendering)];
const UNITS: [&str; 6] = ["millimeter", "centimeter", "meter", "inch", "foot", "yard"];
const SPEEDS: [(&str, f64); 4] = [("Play every frame", 0.0), ("Real-time [fps]", 1.0), ("Half", 0.5), ("Twice", 2.0)];
const FPS: [(&str, f64); 6] = [("24 fps (Film)", 24.0), ("25 fps (PAL)", 25.0), ("30 fps (NTSC)", 30.0), ("48 fps (Show)", 48.0), ("50 fps (PAL Field)", 50.0), ("60 fps (NTSC Field)", 60.0)];

impl Prefs {
    /// Real-time playback multiplier (None = play every frame).
    pub fn speed(&self) -> Option<f64> {
        let s = SPEEDS[self.playback_speed.min(3)].1;
        (s > 0.0).then_some(s)
    }
}

/// Apply the settings that act every frame (UI scale, tooltips, grid, list sizes, time).
pub fn apply(app: &mut IncaApp, ctx: &egui::Context) {
    // like the JS, the menu set in use is the one the next session starts with
    app.prefs.menu_set = MENU_SETS.iter().position(|m| m.1 == app.menu_set).unwrap_or(0);
    let p = &app.prefs;
    if (ctx.zoom_factor() - p.ui_scale).abs() > 1e-4 {
        ctx.set_zoom_factor(p.ui_scale.clamp(0.5, 2.0));
    }
    let delay = if p.tooltips { 0.5 } else { f32::INFINITY };
    if ctx.global_style().interaction.tooltip_delay != delay {
        ctx.global_style_mut(|s| s.interaction.tooltip_delay = delay);
    }
    for v in app.vps.iter_mut() {
        v.grid_cfg = (p.grid_size.max(0.01), p.grid_spacing.max(0.001), p.grid_major.max(1));
    }
    app.recent.truncate(p.recent_count.max(1));
    app.recent_projects.truncate(p.recent_projects_count.max(1));
    app.s.time.play_every = p.playback_speed == 0;
    app.s.key_tangents = (p.tangent_in.clone(), p.tangent_out.clone());
}

fn row(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.label(label);
    add(ui);
    ui.end_row();
}

fn combo(ui: &mut egui::Ui, id: &str, cur: &mut usize, opts: &[&str]) {
    egui::ComboBox::from_id_salt(id).selected_text(opts[(*cur).min(opts.len() - 1)]).show_ui(ui, |ui| {
        for (i, o) in opts.iter().enumerate() {
            ui.selectable_value(cur, i, *o);
        }
    });
}

fn tangent(ui: &mut egui::Ui, id: &str, cur: &mut String) {
    egui::ComboBox::from_id_salt(id).selected_text(cur.as_str()).show_ui(ui, |ui| {
        for t in inca_core::anim::TANGENTS {
            ui.selectable_value(cur, t.to_string(), *t);
        }
    });
}

pub fn window(app: &mut IncaApp, ctx: &egui::Context) {
    let mut open = app.win.prefs;
    if !open {
        return;
    }
    egui::Window::new("Preferences").open(&mut open).default_size([640.0, 440.0]).show(ctx, |ui| {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(170.0);
                ui.label(RichText::new("Categories").strong());
                for (i, c) in CATEGORIES.iter().enumerate() {
                    ui.selectable_value(&mut app.prefs_cat, i, *c);
                }
            });
            ui.separator();
            ui.vertical(|ui| {
                egui::ScrollArea::vertical().show(ui, |ui| category(app, ui));
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Restore Default Settings").clicked() {
                        app.prefs = Prefs::default();
                    }
                });
            });
        });
    });
    app.win.prefs = open;
}

fn category(app: &mut IncaApp, ui: &mut egui::Ui) {
    let cat = CATEGORIES[app.prefs_cat.min(CATEGORIES.len() - 1)];
    egui::Grid::new(("prefs", cat)).num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| match cat {
        "Interface" => {
            theme::section_header(ui, "Interface");
            ui.end_row();
            row(ui, "Interface scaling", |ui| {
                ui.add(egui::Slider::new(&mut app.prefs.ui_scale, 0.8..=1.5).step_by(0.05));
            });
            row(ui, "Default menu set", |ui| {
                let names: Vec<&str> = MENU_SETS.iter().map(|m| m.0).collect();
                combo(ui, "pms", &mut app.prefs.menu_set, &names);
                app.menu_set = MENU_SETS[app.prefs.menu_set.min(4)].1;
            });
            row(ui, "Show tooltips", |ui| {
                ui.checkbox(&mut app.prefs.tooltips, "");
            });
            row(ui, "Ask before quitting with unsaved changes", |ui| {
                ui.checkbox(&mut app.prefs.confirm_quit, "");
            });
        }
        "UI Elements" => {
            theme::section_header(ui, "Visible UI Elements");
            ui.end_row();
            for (k, l) in crate::workspace::ELEMENTS {
                let mut on = app.shown(k);
                ui.label(*l);
                if ui.checkbox(&mut on, "").changed() {
                    app.set_shown(k, on);
                }
                ui.end_row();
            }
        }
        "Display" => {
            theme::section_header(ui, "Viewport");
            ui.end_row();
            row(ui, "Background color", |ui| {
                let names: Vec<&str> = crate::viewport::BG_MODES.to_vec();
                combo(ui, "pbg", &mut app.bg_mode, &names);
            });
            row(ui, "Allow tumbling orthographic views", |ui| {
                ui.checkbox(&mut app.prefs.ortho_tumble, "");
            });
            theme::section_header(ui, "Grid");
            ui.end_row();
            row(ui, "Length and width", |ui| {
                ui.add(egui::DragValue::new(&mut app.prefs.grid_size).speed(0.5).range(0.01..=10000.0).suffix(" units"));
            });
            row(ui, "Grid lines every", |ui| {
                ui.add(egui::DragValue::new(&mut app.prefs.grid_spacing).speed(0.1).range(0.001..=1000.0).suffix(" units"));
            });
            row(ui, "Major line every", |ui| {
                ui.add(egui::DragValue::new(&mut app.prefs.grid_major).range(1..=100).suffix(" lines"));
            });
            theme::section_header(ui, "Heads Up Display");
            ui.end_row();
            let h = &mut app.hud;
            for (l, v) in [("Poly count", &mut h.poly_count), ("Frame rate", &mut h.fps), ("Current frame", &mut h.frame), ("Camera names", &mut h.cam_names), ("View axis", &mut h.view_axis)] {
                ui.label(l);
                ui.checkbox(v, "");
                ui.end_row();
            }
        }
        "Kinematics / Manipulators" => {
            theme::section_header(ui, "Manipulators");
            ui.end_row();
            row(ui, "Global scale", |ui| {
                ui.add(egui::Slider::new(&mut app.manip_size, 0.2..=4.0));
            });
        }
        "Animation" => {
            theme::section_header(ui, "Tangents");
            ui.end_row();
            row(ui, "Default in tangent", |ui| tangent(ui, "pti", &mut app.prefs.tangent_in));
            row(ui, "Default out tangent", |ui| tangent(ui, "pto", &mut app.prefs.tangent_out));
        }
        "Time Slider" => {
            theme::section_header(ui, "Playback");
            ui.end_row();
            row(ui, "Playback speed", |ui| {
                let names: Vec<&str> = SPEEDS.iter().map(|s| s.0).collect();
                combo(ui, "pps", &mut app.prefs.playback_speed, &names);
            });
            row(ui, "Looping", |ui| {
                let opts = ["once", "oscillate", "continuous"];
                let mut i = opts.iter().position(|o| *o == app.s.time.r#loop).unwrap_or(2);
                combo(ui, "plp", &mut i, &["Once", "Oscillate", "Continuous"]);
                app.s.time.r#loop = opts[i].into();
            });
            theme::section_header(ui, "Time Range");
            ui.end_row();
            let t = &mut app.s.time;
            for (l, v) in [("Playback start", &mut t.start), ("Playback end", &mut t.end), ("Animation start", &mut t.anim_start), ("Animation end", &mut t.anim_end)] {
                ui.label(l);
                ui.add(egui::DragValue::new(v));
                ui.end_row();
            }
        }
        "Modeling" => {
            theme::section_header(ui, "Construction History");
            ui.end_row();
            row(ui, "Construction history on by default", |ui| {
                ui.checkbox(&mut app.history_on, "");
            });
        }
        "Settings" => {
            theme::section_header(ui, "Working Units");
            ui.end_row();
            row(ui, "Linear", |ui| combo(ui, "plu", &mut app.prefs.linear_unit, &UNITS));
            row(ui, "Angular", |ui| combo(ui, "pau", &mut app.prefs.angular_unit, &["degrees", "radians"]));
            row(ui, "Time", |ui| {
                let names: Vec<&str> = FPS.iter().map(|f| f.0).collect();
                let mut i = FPS.iter().position(|f| f.1 == app.s.time.fps).unwrap_or(0);
                combo(ui, "pfps", &mut i, &names);
                app.s.time.fps = FPS[i].1;
            });
        }
        "Undo" => {
            theme::section_header(ui, "Undo");
            ui.end_row();
            row(ui, "Queue size (levels)", |ui| {
                ui.add(egui::DragValue::new(&mut app.s.undo_limit).range(1..=1000));
            });
        }
        "Files/Projects" => {
            theme::section_header(ui, "Recent Files");
            ui.end_row();
            row(ui, "Recent files list size", |ui| {
                ui.add(egui::DragValue::new(&mut app.prefs.recent_count).range(1..=30));
            });
            row(ui, "Recent projects list size", |ui| {
                ui.add(egui::DragValue::new(&mut app.prefs.recent_projects_count).range(1..=30));
            });
        }
        "Save" => {
            theme::section_header(ui, "Auto Save");
            ui.end_row();
            row(ui, "Enable auto save", |ui| {
                ui.checkbox(&mut app.autosave.0, "");
            });
            row(ui, "Interval", |ui| {
                ui.add(egui::DragValue::new(&mut app.autosave.1).speed(0.2).range(1.0..=600.0).suffix(" minutes"));
            });
            row(ui, "Limit auto saves", |ui| {
                ui.add(egui::DragValue::new(&mut app.prefs.autosave_limit).range(1..=100));
            });
            theme::section_header(ui, "Save");
            ui.end_row();
            row(ui, "Incremental save", |ui| {
                ui.checkbox(&mut app.prefs.incremental_save, "");
            });
        }
        _ => {}
    });
}

/// The menu set chosen as the default in Preferences.
pub fn default_menu_set(p: &Prefs) -> MenuSet {
    MENU_SETS[p.menu_set.min(4)].1
}
