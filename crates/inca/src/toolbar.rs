//! Status line, shelves and the tool box (same groups, icons and shelves as the JS app).

use crate::actions::label;
use crate::app::{IncaApp, Layout, MenuSet, RightTab, Tool};
use crate::icons;
use crate::theme;
use egui::{vec2, Color32, RichText};
use inca_core::scene::{CompMode, SelMode};

fn ib(ui: &mut egui::Ui, icon: &str, tip: &str, on: bool) -> egui::Response {
    icons::button(ui, icon, tip, on, 24.0)
}

pub const INPUT_MODES: [&str; 4] = ["Absolute", "Relative", "Rename", "Select"];
pub const SYMMETRY: [&str; 5] = ["Symmetry: Off", "Object X", "World X", "World Y", "World Z"];

pub fn status_line(app: &mut IncaApp, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.x = 1.0;
    ui.horizontal_centered(|ui| {
        egui::ComboBox::from_id_salt("menuset")
            .width(96.0)
            .selected_text(app.menu_set.label())
            .show_ui(ui, |ui| {
                for m in MenuSet::ALL {
                    ui.selectable_value(&mut app.menu_set, m, m.label());
                }
            })
            .response
            .on_hover_text("Menu set: changes the menus to the right of Windows (F2 Modeling, F3 Rigging, F4 Animation, F5 FX, F6 Rendering)");
        icons::separator(ui);
        for (i, tip) in [("newScene", "Create a new scene (Ctrl+N)"), ("openScene", "Open a scene (Ctrl+O)"), ("saveScene", "Save the current scene (Ctrl+S)")] {
            if ib(ui, i, tip, false).clicked() {
                app.action(i);
            }
        }
        icons::separator(ui);
        if ib(ui, "undo", "Undo the last action (Z / Ctrl+Z)", false).clicked() {
            app.action("undo");
        }
        if ib(ui, "redo", "Redo the last undone action (Shift+Z / Ctrl+Y)", false).clicked() {
            app.action("redo");
        }
        icons::separator(ui);
        let comp = app.s.comp_mode.is_some();
        if ib(ui, "selHierarchy", "Select by hierarchy and combinations", app.select_hierarchy && !comp).clicked() {
            app.select_hierarchy = true;
            if comp {
                app.set_comp_mode(None);
            }
        }
        if ib(ui, "selObject", "Select by object type (F8)", !app.select_hierarchy && !comp).clicked() {
            app.select_hierarchy = false;
            if comp {
                app.set_comp_mode(None);
            }
        }
        if ib(ui, "selComponent", "Select by component type (F8)", comp).clicked() && !comp {
            app.set_comp_mode(Some(app.last_comp_mode));
        }
        icons::separator(ui);
        if comp {
            for (m, i, tip) in [(CompMode::Vertex, "compVertex", "Vertex (F9)"), (CompMode::Edge, "compEdge", "Edge (F10)"), (CompMode::Face, "compFace", "Face (F11)")] {
                if ib(ui, i, tip, app.s.comp_mode == Some(m)).clicked() {
                    app.set_comp_mode(Some(m));
                }
            }
            icons::button_ex(ui, "compUV", "UV (F12) — comes with the UV Editor", false, 24.0, 20.0, false);
        } else {
            for (k, i, tip) in [("curve", "olCurve", "Select curves"), ("mesh", "olMesh", "Select surfaces (polygons & NURBS)"), ("light", "olLight", "Select lights"), ("camera", "olCamera", "Select cameras"), ("locator", "olLocator", "Select locators")] {
                let on = !app.sel_mask_off.contains(k);
                if ib(ui, i, tip, on).clicked() {
                    if on {
                        app.sel_mask_off.insert(k.to_string());
                    } else {
                        app.sel_mask_off.remove(k);
                    }
                }
            }
        }
        icons::separator(ui);
        if ib(ui, "isolate", "Lock / unlock current selection", app.lock_selection).clicked() {
            app.lock_selection = !app.lock_selection;
        }
        icons::separator(ui);
        if ib(ui, "snapGrid", "Snap to grids (hold X)", app.snap_grid).clicked() {
            app.snap_grid = !app.snap_grid;
        }
        if ib(ui, "snapCurve", "Snap to curves (hold C)", app.snap_curve).clicked() {
            app.snap_curve = !app.snap_curve;
        }
        if ib(ui, "snapPoint", "Snap to points (hold V)", app.snap_point).clicked() {
            app.snap_point = !app.snap_point;
        }
        if ib(ui, "snapSurface", "Make the selected object live (draw curves on its surface)", app.live_surface.is_some()).clicked() {
            app.live_surface = if app.live_surface.is_some() { None } else { app.s.lead().filter(|l| app.s.node(l).is_some_and(|n| n.mesh().is_some())).cloned() };
            if app.live_surface.is_none() {
                app.help = "Select a polygon object to make live".into();
            }
        }
        icons::separator(ui);
        if ib(ui, "deleteHistory", "Construction history on/off", app.history_on).clicked() {
            app.history_on = !app.history_on;
            app.help = format!("Construction history {}", if app.history_on { "on" } else { "off" });
        }
        icons::separator(ui);
        for (i, a, tip) in [("renderView", "renderView", "Open Render View"), ("renderFrame", "renderCurrent", "Render the current frame"), ("ipr", "renderCurrent", "IPR render the current frame"), ("renderSettings", "renderSettings", "Display render settings"), ("hypershade", "hypershade", "Display Hypershade window")] {
            if ib(ui, i, tip, false).clicked() {
                app.action(a);
            }
        }
        icons::separator(ui);
        // numeric input line
        let mut mode = app.panels.input_mode;
        egui::ComboBox::from_id_salt("inputmode").width(74.0).show_index(ui, &mut mode, 4, |i| INPUT_MODES[i].to_string()).on_hover_text("Input line mode");
        app.panels.input_mode = mode;
        let r = ui.add(egui::TextEdit::singleline(&mut app.panels.input_line).desired_width(150.0).hint_text("x y z  or name"));
        let r = r.on_hover_text("Absolute/relative transform for the selection, rename the selection or select by name (wildcards *)");
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let t = std::mem::take(&mut app.panels.input_line);
            do_input(app, mode, t.trim());
        }
        icons::separator(ui);
        let mut sym = app.symmetry;
        egui::ComboBox::from_id_salt("symmetry").width(108.0).show_index(ui, &mut sym, 5, |i| SYMMETRY[i].to_string()).on_hover_text("Symmetry");
        if sym != app.symmetry {
            app.symmetry = sym;
            app.help = format!("Symmetry: {}", SYMMETRY[sym]);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let right = |app: &IncaApp, t: RightTab| app.show_right && app.right_tab == t;
            if ib(ui, "channelBox", "Show or hide the Channel Box / Layer Editor", right(app, RightTab::ChannelBox)).clicked() {
                toggle_right(app, RightTab::ChannelBox);
            }
            if ib(ui, "toolSettings", "Show or hide the Tool Settings", right(app, RightTab::ToolSettings)).clicked() {
                toggle_right(app, RightTab::ToolSettings);
            }
            if ib(ui, "attrEditor", "Show or hide the Attribute Editor (Ctrl+A)", right(app, RightTab::AttributeEditor)).clicked() {
                toggle_right(app, RightTab::AttributeEditor);
            }
            icons::button_ex(ui, "modelingToolkit", "Modeling Toolkit (planned)", false, 24.0, 20.0, false);
        });
    });
}

fn toggle_right(app: &mut IncaApp, t: RightTab) {
    if app.show_right && app.right_tab == t {
        app.show_right = false;
    } else {
        app.show_right = true;
        app.right_tab = t;
    }
}

/// The status line input field: move / rotate / scale (by the current tool), rename or select by name.
fn do_input(app: &mut IncaApp, mode: usize, txt: &str) {
    if txt.is_empty() {
        return;
    }
    match INPUT_MODES[mode] {
        "Rename" => {
            if app.s.lead().is_some() {
                let _ = app.run_mel("rename", &format!("rename \"{txt}\";"));
            }
        }
        "Select" => {
            let ids: Vec<String> = app.s.all_dag(false).into_iter().filter(|i| inca_core::mel::glob_match(txt, &app.s.name_of(i))).collect();
            app.select_names(&ids, SelMode::Replace);
        }
        m => {
            let v: Vec<f64> = txt.split([' ', ',']).filter(|s| !s.is_empty()).filter_map(|s| s.parse().ok()).collect();
            if v.is_empty() {
                app.last_result = "// Warning: Enter numbers: x y z".into();
                return;
            }
            let vec = if v.len() == 1 { [v[0]; 3] } else { [v[0], v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0)] };
            let cmd = match app.tool {
                Tool::Rotate => "rotate",
                Tool::Scale => "scale",
                _ => "move",
            };
            let f = inca_core::math::fmt_num;
            let _ = app.run_mel("input line", &format!("{cmd} {}{} {} {};", if m == "Relative" { "-r " } else { "" }, f(vec[0]), f(vec[1]), f(vec[2])));
        }
    }
}

/// A shelf button: action id, optional icon override, optional short label drawn on the icon.
pub struct ShelfItem {
    pub action: &'static str,
    pub icon: Option<&'static str>,
    pub label: Option<&'static str>,
}

pub enum Shelf {
    Item(ShelfItem),
    Sep,
}

/// The JS app's default shelves (src/js/ui/shelf.js).
pub fn shelves() -> Vec<(&'static str, Vec<Shelf>)> {
    let it = |action| Shelf::Item(ShelfItem { action, icon: None, label: None });
    let ic = |action, icon| Shelf::Item(ShelfItem { action, icon: Some(icon), label: None });
    let il = |action, icon, label| Shelf::Item(ShelfItem { action, icon: Some(icon), label: Some(label) });
    let sep = || Shelf::Sep;
    vec![
        (
            "Curves / Surfaces",
            vec![
                it("cvCurveTool"), it("epCurveTool"), it("pencilCurveTool"), it("arcTool"), it("nurbsCircle"), it("nurbsSquare"), sep(),
                it("nurbsSphere"), it("nurbsCube"), it("nurbsCylinder"), it("nurbsCone"), it("nurbsPlane"), it("nurbsTorus"), sep(),
                it("revolve"), it("loft"), it("planarSurface"), it("extrudeCurve"), sep(),
                il("rebuildCurve", "cvCurve", "Rbld"), il("reverseCurve", "epCurve", "Rev"), il("convertNurbsToPoly", "polyCube", "N→P"),
            ],
        ),
        (
            "Poly Modeling",
            vec![
                it("polySphere"), it("polyCube"), it("polyCylinder"), it("polyCone"), it("polyTorus"), it("polyPlane"), it("polyDisc"), it("polyPlatonic"), it("polyPyramid"), it("polyPipe"), it("polyHelix"), sep(),
                it("combine"), it("separate"), it("mirror"), it("smooth"), ic("booleanUnion", "boolean"), sep(),
                it("extrude"), it("bridge"), it("bevel"), it("addDivisions"), sep(),
                it("multiCutTool"), it("targetWeldTool"), it("insertEdgeLoopTool"), il("quadDrawTool", "polyPlane", "Quad"), sep(),
                it("planarMap"), it("automaticMap"), it("uvEditor"),
            ],
        ),
        (
            "Sculpting",
            vec![
                il("sculptTool", "softSelect", "Sculpt"), il("smoothSculptTool", "smooth", "Smooth"), il("relaxSculptTool", "smooth", "Relax"), il("grabSculptTool", "move", "Grab"),
                il("pinchSculptTool", "mergeCenter", "Pinch"), il("flattenSculptTool", "polyPlane", "Flat"), il("inflateSculptTool", "polySphere", "Infl"), sep(), it("softSelect"),
            ],
        ),
        (
            "Rigging",
            vec![
                ic("jointTool", "joint"), il("ikHandleTool", "joint", "IK"), il("smoothBind", "joint", "Bind"), sep(),
                it("createLocator"), il("createCluster", "locator", "Clus"), il("createLattice", "nurbsCube", "Lat"), sep(),
                il("parentConstraint", "parent", "Par"), il("pointConstraint", "move", "Pt"), il("orientConstraint", "rotate", "Ori"), il("scaleConstraint", "scale", "Scl"), il("aimConstraint", "camera", "Aim"), sep(),
                il("bendDeformer", "polyCylinder", "Bend"), il("twistDeformer", "polyHelix", "Twist"), il("flareDeformer", "polyCone", "Flare"), il("sineDeformer", "polyHelix", "Sine"), il("squashDeformer", "polySphere", "Sqsh"), il("waveDeformer", "polyPlane", "Wave"),
            ],
        ),
        (
            "Animation",
            vec![
                it("setKey"), it("keyTranslate"), it("keyRotate"), it("keyScale"), sep(), it("graphEditor"), it("dopeSheet"), it("playblast"), sep(),
                it("toggleAutoKey"), il("setDrivenKeyWindow", "setKey", "SDK"), il("attachToMotionPath", "epCurve", "Path"), il("bakeKeys", "setKey", "Bake"),
            ],
        ),
        (
            "Rendering",
            vec![
                it("createCamera"), sep(), it("ambientLight"), it("directionalLight"), it("pointLight"), it("spotLight"), it("areaLight"), it("skyDomeLight"), sep(),
                it("assignNew_standardSurface"), it("assignNew_lambert"), it("assignNew_blinn"), it("assignNew_phong"), sep(),
                it("hypershade"), it("renderView"), it("renderCurrent"), it("ipr"), it("renderSettings"),
            ],
        ),
        (
            "FX",
            vec![
                il("createEmitter", "pointLight", "Emit"), il("createParticles", "polySoccer", "nPart"), il("gravityField", "move", "Grav"), il("turbulenceField", "polyHelix", "Turb"),
                il("radialField", "pointLight", "Rad"), il("dragField", "stop", "Drag"), sep(), il("createActiveRigidBody", "polyCube", "Act"), il("createPassiveRigidBody", "polyPlane", "Pas"),
                sep(), il("createFire", "pointLight", "Fire"), il("createSmoke", "pointLight", "Smoke"),
            ],
        ),
        ("Custom", vec![]),
    ]
}

fn shelf_button(ui: &mut egui::Ui, icon: &str, label: Option<&str>, tip: &str, enabled: bool) -> egui::Response {
    let r = icons::button_ex(ui, icon, tip, false, 34.0, 30.0, true);
    if let Some(l) = label {
        let p = ui.painter();
        let pos = egui::pos2(r.rect.center().x, r.rect.max.y - 1.0);
        for o in [vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.0, -1.0), vec2(0.0, 1.0)] {
            p.text(pos + o, egui::Align2::CENTER_BOTTOM, l, egui::FontId::proportional(8.5), Color32::BLACK);
        }
        p.text(pos, egui::Align2::CENTER_BOTTOM, l, egui::FontId::proportional(8.5), Color32::WHITE);
    }
    if !enabled {
        ui.painter().rect_filled(r.rect, 3.0, Color32::from_black_alpha(140));
    }
    r
}

pub fn shelf(app: &mut IncaApp, ui: &mut egui::Ui) {
    let all = shelves();
    let tab = app.shelf_tab.min(all.len() - 1);
    // tabs strip
    egui::Frame::NONE.fill(theme::BG2).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.add_space(26.0);
            ui.spacing_mut().item_spacing.x = 0.0;
            for (i, (name, _)) in all.iter().enumerate() {
                let on = tab == i;
                let t = RichText::new(*name).size(11.0).color(if on { Color32::WHITE } else { Color32::from_gray(0xbd) });
                let b = egui::Button::new(t).fill(if on { theme::BG } else { Color32::TRANSPARENT }).stroke(egui::Stroke::NONE).corner_radius(egui::CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 });
                if ui.add(b).clicked() {
                    app.shelf_tab = i;
                }
            }
        });
    });
    let is_custom = all[tab].0 == "Custom";
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let gear = icons::texture(ui.ctx(), "gear", 18.0).unwrap_or_default();
        ui.menu_image_button(egui::Image::new(egui::load::SizedTexture::new(gear, vec2(18.0, 18.0))), |ui| {
            if ui.button("Open Script Editor (Save to Shelf)").clicked() {
                app.win.script_editor = true;
                ui.close();
            }
            if ui.button("Clear Custom shelf").clicked() {
                app.custom_shelf.clear();
                ui.close();
            }
        });
        egui::ScrollArea::horizontal().id_salt("shelfscroll").show(ui, |ui| {
            ui.horizontal(|ui| {
                for s in &all[tab].1 {
                    match s {
                        Shelf::Sep => {
                            let (r, _) = ui.allocate_exact_size(vec2(6.0, 30.0), egui::Sense::hover());
                            ui.painter().line_segment([egui::pos2(r.center().x, r.min.y + 4.0), egui::pos2(r.center().x, r.max.y - 4.0)], egui::Stroke::new(1.0, Color32::from_gray(0x5a)));
                        }
                        Shelf::Item(it) => {
                            let avail = crate::actions::available(it.action);
                            let icon = it.icon.unwrap_or_else(|| icons::for_action(it.action));
                            let tip = if avail { label(it.action) } else { format!("{} (not in the Rust version yet)", label(it.action)) };
                            let r = shelf_button(ui, icon, it.label, &tip, avail);
                            if r.hovered() {
                                app.help = label(it.action);
                            }
                            if r.clicked() {
                                if avail {
                                    app.action(it.action);
                                } else {
                                    app.last_result = format!("// Warning: {} is not available in this version of Inca", it.action);
                                }
                            }
                        }
                    }
                }
                if is_custom {
                    let mut remove = None;
                    for (i, (l, mel)) in app.custom_shelf.clone().iter().enumerate() {
                        let short: String = l.chars().take(6).collect();
                        let r = shelf_button(ui, "scriptEditor", Some(&short), mel, true);
                        if r.clicked() {
                            let _ = app.run_mel(l, mel);
                        }
                        r.context_menu(|ui| {
                            if ui.button("Delete").clicked() {
                                remove = Some(i);
                                ui.close();
                            }
                        });
                    }
                    if let Some(i) = remove {
                        app.custom_shelf.remove(i);
                    }
                    if app.custom_shelf.is_empty() {
                        ui.label(RichText::new("Script Editor ▸ Save to Shelf adds buttons here").color(theme::TEXT_DIM));
                    }
                }
            });
        });
    });
}

pub fn tool_box(app: &mut IncaApp, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.y = 2.0;
    ui.vertical_centered(|ui| {
        ui.add_space(2.0);
        for (t, i, tip) in [(Tool::Select, "select", "Select Tool (Q)"), (Tool::Lasso, "lasso", "Lasso Select Tool"), (Tool::Paint, "paintSelect", "Paint Selection Tool"), (Tool::Move, "move", "Move Tool (W)"), (Tool::Rotate, "rotate", "Rotate Tool (E)"), (Tool::Scale, "scale", "Scale Tool (R)")] {
            let r = icons::button_ex(ui, i, tip, app.tool == t, 28.0, 22.0, true);
            if r.clicked() {
                app.set_tool(t);
            }
            if r.double_clicked() {
                app.show_right = true;
                app.right_tab = RightTab::ToolSettings;
            }
        }
        let creation = app.tool.is_creation();
        let last_icon = if creation { app.tool.icon() } else { "lastTool" };
        if icons::button_ex(ui, last_icon, "Last tool used (Y)", creation, 28.0, 22.0, true).clicked() {
            app.action("lastToolUsed");
        }
        let h = ui.available_height() - 5.0 * 24.0 - 12.0;
        ui.add_space(h.max(10.0));
        for (i, tip, which) in [("layoutSingle", "Single Perspective View", 0), ("layoutFour", "Four View", 1), ("layoutOutliner", "Perspective / Outliner", 2), ("layoutTwo", "Perspective / Graph Editor", 3), ("outliner", "Outliner", 4)] {
            if icons::button_ex(ui, i, tip, false, 24.0, 18.0, true).clicked() {
                match which {
                    0 => {
                        app.show_outliner = false;
                        app.layout = Layout::Single;
                        app.active_vp = 0;
                    }
                    1 => app.layout = Layout::Four,
                    2 => {
                        app.layout = Layout::Single;
                        app.active_vp = 0;
                        app.show_outliner = true;
                    }
                    3 => {
                        app.layout = Layout::Single;
                        app.active_vp = 0;
                        app.win.graph_editor = true;
                    }
                    _ => app.show_outliner = !app.show_outliner,
                }
            }
        }
    });
}

/// Status line selection masks: may this object be picked?
pub fn mask_allows(app: &IncaApp, id: &str) -> bool {
    use inca_core::scene::Shape;
    let Some(n) = app.s.node(id) else { return false };
    let k = match &n.shape {
        Shape::Mesh(_) => "mesh",
        Shape::Curve(_) => "curve",
        Shape::Light(_) => "light",
        Shape::Camera(_) => "camera",
        Shape::Locator => "locator",
        Shape::Group => "group",
    };
    !app.sel_mask_off.contains(k)
}
