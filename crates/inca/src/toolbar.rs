//! Status line, shelves and the tool box.

use crate::actions::label;
use crate::app::{IncaApp, Layout, MenuSet, RightTab, Tool};
use crate::theme;
use egui::{vec2, Color32, RichText};
use inca_core::scene::CompMode;

use crate::icons::{self, Icon};

fn icon_btn(ui: &mut egui::Ui, icon: Icon, tip: &str, on: bool) -> egui::Response {
    icons::button(ui, icon, tip, on, 24.0)
}

pub fn status_line(app: &mut IncaApp, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        egui::ComboBox::from_id_salt("menuset").width(96.0).selected_text(app.menu_set.label()).show_ui(ui, |ui| {
            for m in MenuSet::ALL {
                ui.selectable_value(&mut app.menu_set, m, m.label());
            }
        });
        ui.separator();
        for (i, a, tip) in [(Icon::New, "newScene", "New Scene (Ctrl+N)"), (Icon::Open, "openScene", "Open Scene (Ctrl+O)"), (Icon::Save, "saveScene", "Save Scene (Ctrl+S)")] {
            if icon_btn(ui, i, tip, false).clicked() {
                app.action(a);
            }
        }
        ui.separator();
        if icon_btn(ui, Icon::Undo, "Undo (Ctrl+Z)", false).clicked() {
            app.action("undo");
        }
        if icon_btn(ui, Icon::Redo, "Redo (Ctrl+Y)", false).clicked() {
            app.action("redo");
        }
        ui.separator();
        let obj = app.s.comp_mode.is_none();
        if icon_btn(ui, Icon::Object, "Select by object type (F8)", obj).clicked() {
            app.set_comp_mode(None);
        }
        if icon_btn(ui, Icon::Component, "Select by component type (F8)", !obj).clicked() && obj {
            app.set_comp_mode(Some(CompMode::Vertex));
        }
        ui.add_space(4.0);
        for (m, i, tip) in [(CompMode::Vertex, Icon::Vertex, "Vertex (F9)"), (CompMode::Edge, Icon::Edge, "Edge (F10)"), (CompMode::Face, Icon::Face, "Face (F11)")] {
            if icon_btn(ui, i, tip, app.s.comp_mode == Some(m)).clicked() {
                app.set_comp_mode(Some(m));
            }
        }
        ui.separator();
        if icon_btn(ui, Icon::Snap, "Snap to grids (X)", app.snap_grid).clicked() {
            app.snap_grid = !app.snap_grid;
        }
        if icon_btn(ui, Icon::World, "World space manipulators (off = object space)", app.world_space).clicked() {
            app.world_space = !app.world_space;
        }
        if icon_btn(ui, Icon::History, "Construction history on/off", app.history_on).clicked() {
            app.history_on = !app.history_on;
        }
        ui.separator();
        if icon_btn(ui, Icon::Render, "Render the current frame", false).clicked() {
            app.action("renderFrame");
        }
        if icon_btn(ui, Icon::Settings, "Render Settings", false).clicked() {
            app.action("renderSettings");
        }
        ui.separator();
        // numeric input line: "rename" / "select by name"
        egui::ComboBox::from_id_salt("inputmode").width(60.0).selected_text(if app.panels.input_select { "Sel" } else { "Rename" }).show_ui(ui, |ui| {
            ui.selectable_value(&mut app.panels.input_select, false, "Rename");
            ui.selectable_value(&mut app.panels.input_select, true, "Select by name");
        });
        let r = ui.add(egui::TextEdit::singleline(&mut app.panels.input_line).desired_width(120.0).hint_text(if app.panels.input_select { "pCube*" } else { "new name" }));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let t = app.panels.input_line.trim().to_string();
            if !t.is_empty() {
                let mel = if app.panels.input_select { format!("select -r {t};") } else { format!("rename \"{t}\";") };
                let _ = app.run_mel("input line", &mel);
            }
            app.panels.input_line.clear();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_btn(ui, Icon::ChannelBox, "Channel Box / Layer Editor", app.show_right && app.right_tab == RightTab::ChannelBox).clicked() {
                if app.show_right && app.right_tab == RightTab::ChannelBox {
                    app.show_right = false;
                } else {
                    app.action("channelBox");
                }
            }
            if icon_btn(ui, Icon::AttrEditor, "Attribute Editor", app.show_right && app.right_tab == RightTab::AttributeEditor).clicked() {
                if app.show_right && app.right_tab == RightTab::AttributeEditor {
                    app.show_right = false;
                } else {
                    app.action("attributeEditor");
                }
            }
            if icon_btn(ui, Icon::ToolSettings, "Tool Settings", app.show_right && app.right_tab == RightTab::ToolSettings).clicked() {
                app.action("toolSettings");
            }
            if icon_btn(ui, Icon::Outliner, "Outliner", app.show_outliner).clicked() {
                app.show_outliner = !app.show_outliner;
            }
        });
    });
}

pub struct ShelfItem {
    pub label: &'static str,
    pub action: &'static str,
}

pub fn shelves() -> Vec<(&'static str, Vec<ShelfItem>)> {
    let s = |label, action| ShelfItem { label, action };
    vec![
        ("Curves / Surfaces", vec![s("CV", "cvCurveTool"), s("EP", "epCurveTool"), s("Arc", "arcTool"), s("Circle", "nurbsCircle"), s("Square", "nurbsSquare"), s("NSphere", "nurbsSphere"), s("NCube", "nurbsCube"), s("NCyl", "nurbsCylinder"), s("NCone", "nurbsCone"), s("NPlane", "nurbsPlane"), s("NTorus", "nurbsTorus"), s("Revolve", "revolve"), s("Loft", "loft"), s("Extrude", "extrudeCurve"), s("Planar", "planar"), s("Rebuild", "rebuildCurve"), s("Reverse", "reverseCurve")]),
        (
            "Poly Modeling",
            vec![
                s("Sphere", "polySphere"),
                s("Cube", "polyCube"),
                s("Cyl", "polyCylinder"),
                s("Cone", "polyCone"),
                s("Torus", "polyTorus"),
                s("Plane", "polyPlane"),
                s("Disc", "polyDisc"),
                s("Pipe", "polyPipe"),
                s("PolyTool", "createPolygonTool"),
                s("Combine", "combine"),
                s("Separate", "separate"),
                s("Extrude", "extrude"),
                s("Bevel", "bevel"),
                s("Bridge", "bridge"),
                s("Loop", "insertEdgeLoop"),
                s("Merge", "merge"),
                s("Smooth", "smooth"),
                s("Mirror", "mirror"),
                s("Union", "booleanUnion"),
                s("Diff", "booleanDifference"),
                s("Isect", "booleanIntersection"),
                s("Planar", "uvPlanar"),
                s("AutoUV", "uvAutomatic"),
            ],
        ),
        ("Rigging", vec![s("Locator", "createLocator"), s("Group", "group"), s("Parent", "parent"), s("Unparent", "unparent")]),
        ("Animation", vec![s("Key", "setKey"), s("KeyT", "keyTranslate"), s("KeyR", "keyRotate"), s("KeyS", "keyScale"), s("Graph", "graphEditor"), s("Play", "play")]),
        (
            "Rendering",
            vec![
                s("Camera", "createCamera"),
                s("Amb", "ambientLight"),
                s("Dir", "directionalLight"),
                s("Point", "pointLight"),
                s("Spot", "spotLight"),
                s("Area", "areaLight"),
                s("Sky", "skyDomeLight"),
                s("Lambert", "matLambert"),
                s("Blinn", "matBlinn"),
                s("Standard", "matStandard"),
                s("Hyper", "hypershade"),
                s("Render", "renderFrame"),
                s("Settings", "renderSettings"),
            ],
        ),
        ("FX", vec![]),
        ("Custom", vec![s("Script", "scriptEditor"), s("Freeze", "freezeTransformations"), s("Center", "centerPivot"), s("DelHist", "deleteHistory")]),
    ]
}

pub fn shelf(app: &mut IncaApp, ui: &mut egui::Ui) {
    let all = shelves();
    ui.horizontal(|ui| {
        for (i, (name, _)) in all.iter().enumerate() {
            let on = app.shelf_tab == i;
            let t = RichText::new(*name).size(11.0).color(if on { Color32::WHITE } else { theme::TEXT_DIM });
            if ui.add(egui::Button::new(t).selected(on).frame(on)).clicked() {
                app.shelf_tab = i;
            }
        }
    });
    let items = &all[app.shelf_tab.min(all.len() - 1)].1;
    egui::ScrollArea::horizontal().id_salt("shelfscroll").show(ui, |ui| {
        ui.horizontal(|ui| {
            for it in items {
                let b = egui::Button::new(RichText::new(it.label).size(10.5)).min_size(vec2(40.0, 34.0)).fill(theme::BG2);
                if ui.add(b).on_hover_text(label(it.action)).clicked() {
                    app.action(it.action);
                }
            }
            if items.is_empty() {
                ui.label(RichText::new("(empty shelf)").color(theme::TEXT_DIM));
            }
        });
    });
}

pub fn tool_box(app: &mut IncaApp, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        for (t, i, tip) in [(Tool::Select, Icon::Select, "Select Tool (Q)"), (Tool::Move, Icon::Move, "Move Tool (W)"), (Tool::Rotate, Icon::Rotate, "Rotate Tool (E)"), (Tool::Scale, Icon::Scale, "Scale Tool (R)")] {
            if icons::button(ui, i, tip, app.tool == t, 30.0).clicked() {
                app.set_tool(t);
            }
        }
        ui.separator();
        for (l, i, tip) in [(Layout::Single, Icon::Single, "Single Perspective View"), (Layout::Four, Icon::Four, "Four View"), (Layout::TwoSide, Icon::Two, "Two panes side by side")] {
            if icons::button(ui, i, tip, app.layout == l, 30.0).clicked() {
                app.layout = l;
            }
        }
        ui.separator();
        if icons::button(ui, Icon::Script, "Script Editor", app.win.script_editor, 30.0).clicked() {
            app.win.script_editor = !app.win.script_editor;
        }
    });
}
