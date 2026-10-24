//! Workspaces and UI element visibility.

use crate::app::{IncaApp, Layout, RightTab};

/// Hideable UI elements: (key, label) as in the JS `ELEMENTS`.
pub const ELEMENTS: &[(&str, &str)] = &[
    ("statusline", "Status Line"),
    ("shelf", "Shelf"),
    ("timeslider", "Time Slider"),
    ("rangeslider", "Range Slider"),
    ("cmdline", "Command Line"),
    ("helpline", "Help Line"),
    ("toolbox", "Tool Box"),
    ("sidetabs", "Side Tabs"),
];

pub struct Workspace {
    pub name: &'static str,
    pub right: RightTab,
    pub outliner: bool,
    pub hide: &'static [&'static str],
    /// action run after applying (opens the workspace's editor)
    pub after: Option<&'static str>,
}

pub const WORKSPACES: &[Workspace] = &[
    Workspace { name: "Maya Classic", right: RightTab::ChannelBox, outliner: false, hide: &[], after: None },
    Workspace { name: "Modeling - Standard", right: RightTab::ModelingToolkit, outliner: true, hide: &[], after: None },
    Workspace { name: "Modeling - Expert", right: RightTab::ChannelBox, outliner: false, hide: &["shelf", "statusline", "toolbox", "helpline", "rangeslider"], after: None },
    Workspace { name: "Sculpting", right: RightTab::ToolSettings, outliner: false, hide: &["rangeslider"], after: None },
    Workspace { name: "Animation", right: RightTab::ChannelBox, outliner: true, hide: &[], after: Some("graphEditor") },
    Workspace { name: "Rigging", right: RightTab::ChannelBox, outliner: true, hide: &[], after: None },
    Workspace { name: "Rendering", right: RightTab::AttributeEditor, outliner: true, hide: &[], after: Some("hypershade") },
    Workspace { name: "UV Editing", right: RightTab::ChannelBox, outliner: false, hide: &[], after: None },
];

pub fn apply(app: &mut IncaApp, name: &str) {
    let w = WORKSPACES.iter().find(|w| w.name == name).unwrap_or(&WORKSPACES[0]);
    app.workspace = w.name.to_string();
    app.hidden = w.hide.iter().map(|s| s.to_string()).collect();
    app.show_outliner = w.outliner;
    app.right_tab = w.right;
    app.show_right = true;
    app.layout = Layout::Single;
    app.active_vp = 0;
    app.ui_stash = None;
    if let Some(a) = w.after {
        app.action(a);
    }
}

/// Ctrl+Space: hide every element except the menu bar, or bring back what was showing.
pub fn toggle_all(app: &mut IncaApp) {
    if let Some((hidden, right)) = app.ui_stash.take() {
        app.hidden = hidden;
        app.show_right = right;
        return;
    }
    app.ui_stash = Some((app.hidden.clone(), app.show_right));
    app.hidden = ELEMENTS.iter().map(|(k, _)| k.to_string()).collect();
    app.show_right = false;
}

pub fn restore_all(app: &mut IncaApp) {
    app.hidden.clear();
    app.ui_stash = None;
}

/// Windows > Workspaces and the menu bar selector share this list.
pub fn menu(app: &mut IncaApp, ui: &mut egui::Ui) {
    for w in WORKSPACES {
        if ui.selectable_label(app.workspace == w.name, w.name).clicked() {
            apply(app, w.name);
            ui.close();
        }
    }
    ui.separator();
    if ui.button("Reset Current Workspace").clicked() {
        let n = app.workspace.clone();
        apply(app, &n);
        ui.close();
    }
}

/// Display > UI Elements.
pub fn elements_menu(app: &mut IncaApp, ui: &mut egui::Ui) {
    for (k, l) in ELEMENTS {
        let mut on = app.shown(k);
        if ui.checkbox(&mut on, *l).clicked() {
            app.set_shown(k, on);
        }
    }
    let mut on = app.show_right;
    if ui.checkbox(&mut on, "Channel Box / Attribute Editor").clicked() {
        app.show_right = on;
    }
    let mut on = app.show_outliner;
    if ui.checkbox(&mut on, "Outliner").clicked() {
        app.show_outliner = on;
    }
    ui.separator();
    if ui.add(egui::Button::new("Hide UI Elements").shortcut_text("Ctrl+Space")).clicked() {
        toggle_all(app);
        ui.close();
    }
    if ui.button("Restore UI Elements").clicked() {
        restore_all(app);
        ui.close();
    }
}

/// Workspace selector at the right end of the menu bar (laid out right to left).
pub fn selector(app: &mut IncaApp, ui: &mut egui::Ui) {
    egui::ComboBox::from_id_salt("workspace").width(150.0).selected_text(app.workspace.clone()).show_ui(ui, |ui| menu(app, ui));
    ui.label(egui::RichText::new("Workspace:").color(crate::theme::TEXT_DIM));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspaces_and_elements_are_unique() {
        let mut names: Vec<&str> = WORKSPACES.iter().map(|w| w.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), WORKSPACES.len());
        let mut keys: Vec<&str> = ELEMENTS.iter().map(|e| e.0).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), ELEMENTS.len());
        // every element a workspace hides is a real UI element
        for w in WORKSPACES {
            for h in w.hide {
                assert!(ELEMENTS.iter().any(|e| e.0 == *h), "{} hides unknown {h}", w.name);
            }
        }
    }
}
