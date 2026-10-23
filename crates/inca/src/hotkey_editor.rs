//! Hotkey Editor (src/js/ui/hotkeyEditor.js): user key bindings layered over the Maya defaults.

use crate::app::IncaApp;
use crate::theme;
use egui::{Key, Modifiers, RichText};
use std::collections::BTreeMap;

/// chord ("Ctrl+Shift+K") -> action id; "unbind:<action>" entries switch an action's default hotkey off.
pub type Keymap = BTreeMap<String, String>;

#[derive(Default)]
pub struct HkState {
    pub filter: String,
    pub selected: Option<String>,
    /// waiting for the next key press to bind to `selected`
    pub capturing: bool,
}

pub fn chord(key: Key, m: Modifiers) -> String {
    let mut s = String::new();
    if m.ctrl || m.command {
        s += "Ctrl+";
    }
    if m.shift {
        s += "Shift+";
    }
    if m.alt {
        s += "Alt+";
    }
    s + key.name()
}

/// The action for a key press: a user binding first, then the Maya default unless it was removed.
pub fn resolve(app: &IncaApp, key: Key, m: Modifiers) -> Option<String> {
    if let Some(a) = app.keymap.get(&chord(key, m)) {
        return Some(a.clone());
    }
    let a = crate::actions::hotkey(key, m, app)?;
    (!app.keymap.contains_key(&format!("unbind:{a}"))).then(|| a.to_string())
}

/// The chords that run an action: user bindings plus its default (unless that chord was rebound).
pub fn chords_for(app: &IncaApp, id: &str) -> Vec<String> {
    let mut out: Vec<String> = app.keymap.iter().filter(|(c, a)| *a == id && !c.starts_with("unbind:")).map(|(c, _)| c.clone()).collect();
    let def = crate::actions::hotkey_label(id);
    if !def.is_empty() && !app.keymap.contains_key(&format!("unbind:{id}")) {
        out.push(def.to_string());
    }
    out
}

/// Which action a chord runs (user binding or default label), for conflict warnings.
fn owner(app: &IncaApp, c: &str) -> Option<String> {
    if let Some(a) = app.keymap.get(c) {
        return Some(a.clone());
    }
    crate::actions::ACTIONS.iter().find(|(_, _, hk)| *hk == c).map(|(id, _, _)| id.to_string())
}

pub fn window(app: &mut IncaApp, ctx: &egui::Context) {
    let mut open = app.win.hotkeys;
    if !open {
        return;
    }
    // capture the next key press for the selected action
    if app.hk.capturing {
        let pressed = ctx.input(|i| i.events.iter().find_map(|e| match e {
            egui::Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
            _ => None,
        }));
        if let Some((key, m)) = pressed {
            app.hk.capturing = false;
            if key != Key::Escape {
                if let Some(id) = app.hk.selected.clone() {
                    let c = chord(key, m);
                    app.keymap.insert(c.clone(), id.clone());
                    app.help = format!("Hotkey {c} now runs {}", crate::actions::label(&id));
                }
            }
        }
    }
    egui::Window::new("Hotkey Editor").open(&mut open).default_size([520.0, 520.0]).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Search");
            ui.text_edit_singleline(&mut app.hk.filter);
            if ui.button("Restore Defaults").clicked() {
                app.keymap.clear();
            }
        });
        ui.separator();
        let q = app.hk.filter.to_lowercase();
        let rows: Vec<(&str, &str)> = crate::actions::ACTIONS.iter().filter(|(id, l, _)| q.is_empty() || l.to_lowercase().contains(&q) || id.to_lowercase().contains(&q)).map(|(id, l, _)| (*id, *l)).collect();
        egui::ScrollArea::vertical().max_height(ui.available_height() - 90.0).show(ui, |ui| {
            egui::Grid::new("hked").striped(true).num_columns(2).show(ui, |ui| {
                for (id, l) in rows {
                    let sel = app.hk.selected.as_deref() == Some(id);
                    if ui.selectable_label(sel, l).on_hover_text(id).clicked() {
                        app.hk.selected = Some(id.to_string());
                        app.hk.capturing = false;
                    }
                    ui.label(RichText::new(chords_for(app, id).join(", ")).monospace().color(theme::TEXT_DIM));
                    ui.end_row();
                }
            });
        });
        ui.separator();
        match app.hk.selected.clone() {
            Some(id) => {
                ui.label(RichText::new(crate::actions::label(&id)).strong());
                ui.horizontal(|ui| {
                    let txt = if app.hk.capturing { "Press a key… (Esc cancels)" } else { "Assign New Hotkey" };
                    if ui.button(txt).clicked() {
                        app.hk.capturing = !app.hk.capturing;
                    }
                    if ui.button("Remove Hotkeys").clicked() {
                        app.keymap.retain(|_, a| *a != id);
                        if !crate::actions::hotkey_label(&id).is_empty() {
                            app.keymap.insert(format!("unbind:{id}"), id.clone());
                        }
                    }
                });
                for c in chords_for(app, &id) {
                    if let Some(o) = owner(app, &c).filter(|o| *o != id) {
                        ui.label(RichText::new(format!("{c} is also the default for {}", crate::actions::label(&o))).color(theme::TEXT_DIM));
                    }
                }
            }
            None => {
                ui.label(RichText::new("Pick a command to change its hotkey").color(theme::TEXT_DIM));
            }
        }
    });
    if !open {
        app.hk.capturing = false;
    }
    app.win.hotkeys = open;
}
