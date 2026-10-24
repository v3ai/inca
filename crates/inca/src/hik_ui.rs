//! Character Controls window (Skeleton > HumanIK): characters, their source, the definition's
//! slot -> joint mapping, skeleton creation and the control rig.

use crate::app::IncaApp;
use crate::theme;
use egui::{Color32, RichText};
use inca_core::hik::{REQUIRED, SLOTS, SOURCE_CONTROL_RIG};
use inca_core::mel::Val;
use inca_core::scene::SelMode;

pub struct HikState {
    pub current: String,
    pub height: f64,
    pub spines: usize,
    pub fingers: bool,
    pub tab: usize,
}

impl Default for HikState {
    fn default() -> Self {
        HikState { current: String::new(), height: 170.0, spines: 3, fingers: true, tab: 0 }
    }
}

const OK: Color32 = Color32::from_rgb(120, 200, 120);
const BAD: Color32 = Color32::from_rgb(230, 110, 110);

fn q(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\\\""))
}

/// Run a HumanIK command; a returned character name becomes the current one.
fn run(app: &mut IncaApp, label: &str, mel: &str, takes_name: bool) {
    if let Ok(Val::Str(n)) = app.run_mel(label, mel) {
        if takes_name && app.s.hik_character(&n).is_some() {
            app.hik_ui.current = n;
        }
    }
}

pub fn create_skeleton(app: &mut IncaApp) {
    let h = &app.hik_ui;
    let mel = format!("hikCreateSkeleton -n Character1 -h {} -sp {} -f {};", h.height, h.spines, h.fingers as i32);
    run(app, "HumanIK skeleton", &mel, true);
}

pub fn define_from_selection(app: &mut IncaApp) {
    run(app, "define character", "hikCreateCharacter;", true);
}

fn current(app: &IncaApp) -> Option<String> {
    app.s.hik_character(&app.hik_ui.current).map(|c| c.name.clone()).or_else(|| app.s.characters.last().map(|c| c.name.clone()))
}

pub fn create_control_rig(app: &mut IncaApp) {
    match current(app) {
        Some(c) => run(app, "control rig", &format!("hikCreateControlRig {};", q(&c)), false),
        None => app.last_result = "// Error: Create or define a character first (Skeleton > HumanIK) //".into(),
    }
}

pub fn bake(app: &mut IncaApp) {
    match current(app) {
        Some(c) => run(app, "bake character", &format!("hikBake {};", q(&c)), false),
        None => app.last_result = "// Error: No character to bake //".into(),
    }
}

pub fn window(app: &mut IncaApp, ctx: &egui::Context) {
    let mut open = app.win.hik;
    if !open {
        return;
    }
    if let Some(c) = current(app) {
        app.hik_ui.current = c;
    }
    egui::Window::new("Character Controls").open(&mut open).default_size([360.0, 600.0]).show(ctx, |ui| {
        let names: Vec<String> = app.s.characters.iter().map(|c| c.name.clone()).collect();
        let cur = app.hik_ui.current.clone();
        let ch = app.s.hik_character(&cur).cloned();
        egui::Grid::new("hikTop").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label("Character:");
            egui::ComboBox::from_id_salt("hikChar").selected_text(if cur.is_empty() { "None" } else { &cur }).width(200.0).show_ui(ui, |ui| {
                for n in &names {
                    ui.selectable_value(&mut app.hik_ui.current, n.clone(), n);
                }
            });
            ui.end_row();
            ui.label("Source:");
            ui.add_enabled_ui(ch.is_some(), |ui| {
                let src = ch.as_ref().map(|c| c.source.clone()).unwrap_or_default();
                let mut pick = src.clone();
                egui::ComboBox::from_id_salt("hikSrc").selected_text(if src.is_empty() { "None" } else { &src }).width(200.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut pick, String::new(), "None (stance)");
                    if ch.as_ref().is_some_and(|c| c.rig.is_some()) {
                        ui.selectable_value(&mut pick, SOURCE_CONTROL_RIG.to_string(), SOURCE_CONTROL_RIG);
                    }
                    for n in names.iter().filter(|n| **n != cur) {
                        ui.selectable_value(&mut pick, n.clone(), n);
                    }
                });
                if pick != src {
                    run(app, "character source", &format!("hikSetSource -s {} {};", q(&pick), q(&cur)), false);
                }
            });
            ui.end_row();
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button("Define from Selection").on_hover_text("Map the selected skeleton (select its root joint) to a new character").clicked() {
                define_from_selection(app);
            }
            if let Some(c) = &ch {
                if c.rig.is_none() {
                    if ui.add_enabled(c.missing().is_empty(), egui::Button::new("Create Control Rig")).clicked() {
                        create_control_rig(app);
                    }
                } else if ui.button("Delete Control Rig").clicked() {
                    run(app, "delete control rig", &format!("hikDeleteControlRig {};", q(&cur)), false);
                }
                if ui.add_enabled(!c.source.is_empty(), egui::Button::new("Bake to Skeleton")).on_hover_text("Key the joints over the playback range from the current source").clicked() {
                    bake(app);
                }
                if ui.button("Delete Character").clicked() {
                    run(app, "delete character", &format!("hikDeleteCharacter {};", q(&cur)), false);
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            for (i, t) in ["Definition", "Skeleton", "Controls"].iter().enumerate() {
                ui.selectable_value(&mut app.hik_ui.tab, i, *t);
            }
        });
        ui.separator();
        match app.hik_ui.tab {
            0 => definition(app, ui, ch.as_ref()),
            1 => skeleton(app, ui),
            _ => controls(app, ui, ch.as_ref()),
        }
    });
    app.win.hik = open;
}

fn definition(app: &mut IncaApp, ui: &mut egui::Ui, ch: Option<&inca_core::hik::Character>) {
    let Some(c) = ch else {
        ui.label(RichText::new("No character yet. Create a skeleton on the Skeleton tab, or select an existing skeleton's root joint and click Define from Selection.").color(theme::TEXT_DIM));
        return;
    };
    let missing = c.missing();
    if missing.is_empty() {
        ui.label(RichText::new("✔ Definition complete").color(OK));
    } else {
        ui.label(RichText::new(format!("Missing: {}", missing.join(", "))).color(BAD));
    }
    ui.label(RichText::new("Select a joint and click ◀ to assign it to a slot. Define the character in a T-pose facing +Z.").color(theme::TEXT_DIM).size(10.5));
    let sel_joint = app.s.sel.last().filter(|i| app.s.node(i).is_some_and(|n| n.kind == "joint")).cloned();
    let mut assign: Option<(String, Option<String>)> = None;
    let mut select: Option<String> = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let mut groups: Vec<&str> = vec![];
        for (_, g) in SLOTS {
            if !groups.contains(g) {
                groups.push(g);
            }
        }
        for g in groups {
            egui::CollapsingHeader::new(g).default_open(!g.ends_with("Hand")).show(ui, |ui| {
                egui::Grid::new(format!("hikGrid{g}")).num_columns(4).striped(true).spacing([6.0, 2.0]).show(ui, |ui| {
                    for (slot, _) in SLOTS.iter().filter(|(_, gg)| *gg == g) {
                        let req = REQUIRED.contains(slot);
                        let j = c.joints.get(*slot).filter(|j| app.s.nodes.contains_key(*j));
                        let col = match (j, req) {
                            (Some(_), _) => theme::TEXT,
                            (None, true) => BAD,
                            (None, false) => theme::TEXT_DIM,
                        };
                        ui.label(RichText::new(format!("{slot}{}", if req { " *" } else { "" })).color(col));
                        match j {
                            Some(j) => {
                                if ui.link(app.s.name_of(j)).clicked() {
                                    select = Some(j.clone());
                                }
                            }
                            None => {
                                ui.label(RichText::new("—").color(theme::TEXT_DIM));
                            }
                        }
                        if ui.add_enabled(sel_joint.is_some(), egui::Button::new("◀").small()).on_hover_text("Assign the selected joint").clicked() {
                            assign = Some((slot.to_string(), sel_joint.clone()));
                        }
                        if ui.add_enabled(j.is_some(), egui::Button::new("x").small()).on_hover_text("Clear").clicked() {
                            assign = Some((slot.to_string(), None));
                        }
                        ui.end_row();
                    }
                });
            });
        }
    });
    if let Some(j) = select {
        app.select_names(&[j], SelMode::Replace);
    }
    if let Some((slot, j)) = assign {
        let joint = j.map(|j| format!(" {}", q(&app.s.name_of(&j)))).unwrap_or_default();
        run(app, "character slot", &format!("hikSetSlot {} {slot}{joint};", q(&c.name)), false);
    }
}

fn skeleton(app: &mut IncaApp, ui: &mut egui::Ui) {
    ui.label("A standard HumanIK skeleton in a T-pose, already defined as a character.");
    egui::Grid::new("hikSkel").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        ui.label("Height");
        ui.add(egui::DragValue::new(&mut app.hik_ui.height).range(1.0..=10000.0).speed(1.0));
        ui.end_row();
        ui.label("Spine joints");
        ui.add(egui::DragValue::new(&mut app.hik_ui.spines).range(1..=10));
        ui.end_row();
        ui.label("Fingers");
        ui.checkbox(&mut app.hik_ui.fingers, "");
        ui.end_row();
    });
    if ui.button("Create Skeleton").clicked() {
        create_skeleton(app);
    }
}

fn controls(app: &mut IncaApp, ui: &mut egui::Ui, ch: Option<&inca_core::hik::Character>) {
    let Some(rig) = ch.and_then(|c| c.rig.as_ref()) else {
        ui.label(RichText::new("No control rig. Complete the definition, then click Create Control Rig.").color(theme::TEXT_DIM));
        return;
    };
    ui.label(RichText::new("Click a control to select it. Effectors drive the arms and legs with IK; body controls rotate the spine, neck and head (FK).").color(theme::TEXT_DIM).size(10.5));
    let mut select: Option<String> = None;
    for (title, pick) in [("Effectors", 0), ("Pole vectors", 1), ("Body", 2)] {
        ui.label(RichText::new(title).strong());
        ui.horizontal_wrapped(|ui| {
            for (k, id) in &rig.controls {
                let kind = if k.ends_with("Pole") { 1 } else if rig.offsets.contains_key(k) { 0 } else { 2 };
                if kind == pick && app.s.nodes.contains_key(id) && ui.button(k.as_str()).clicked() {
                    select = Some(id.clone());
                }
            }
        });
    }
    if let Some(id) = select {
        app.select_names(&[id], SelMode::Replace);
    }
}
