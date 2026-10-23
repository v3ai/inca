//! Main menu bar with Maya menu sets (Modeling, Rigging, Animation, FX, Rendering).

use crate::actions::{hotkey_label, label};
use crate::app::{IncaApp, MenuSet};
use inca_core::primitives::GEN_INFO;
use inca_core::scene::LIGHT_TYPES;

pub enum M {
    A(&'static str),
    Sub(&'static str, Vec<M>),
    Sep,
    /// planned item (shown disabled)
    Todo(&'static str),
}
use M::*;

fn poly_prims() -> Vec<M> {
    GEN_INFO.iter().filter(|g| !g.nurbs).map(|g| A(g.ty)).collect()
}
fn nurbs_prims() -> Vec<M> {
    let mut v: Vec<M> = GEN_INFO.iter().filter(|g| g.nurbs).map(|g| A(g.ty)).collect();
    v.push(A("nurbsCircle"));
    v.push(A("nurbsSquare"));
    v
}

pub fn common(name: &str) -> Vec<M> {
    match name {
        "File" => vec![A("newScene"), A("openScene"), A("saveScene"), A("saveSceneAs"), Sep, A("importFile"), A("exportAll"), A("exportSelection"), Sep, Todo("Project Window..."), Sep, A("quit")],
        "Edit" => vec![
            A("undo"),
            A("redo"),
            A("repeatLast"),
            Sep,
            A("copy"),
            A("paste"),
            Sep,
            Sub("Keys", vec![A("deleteKeys"), A("deleteChannels")]),
            Sep,
            A("deleteSel"),
            Sub("Delete by Type", vec![A("deleteHistory"), A("deleteChannels")]),
            Sub("Delete All by Type", vec![A("deleteAllHistory")]),
            Sep,
            A("duplicate"),
            A("duplicateHistory"),
            Sep,
            A("group"),
            A("ungroup"),
            Sep,
            A("parent"),
            A("unparent"),
            Sep,
            A("renameSel"),
        ],
        "Create" => vec![
            Sub("Polygon Primitives", poly_prims()),
            Sub("NURBS Primitives", nurbs_prims()),
            Sep,
            Sub("Lights", LIGHT_TYPES.iter().map(|l| A(l.0)).collect()),
            Sub("Cameras", vec![A("createCamera")]),
            Sep,
            Sub("Curve Tools", vec![A("cvCurveTool"), A("epCurveTool"), A("arcTool")]),
            A("createPolygonTool"),
            Sep,
            A("createEmptyGroup"),
            A("createLocator"),
        ],
        "Select" => vec![
            A("selectAll"),
            A("deselectAll"),
            A("selectHierarchy"),
            A("invertSelection"),
            A("growSelection"),
            A("shrinkSelection"),
            Sep,
            A("objectMode"),
            A("vertexMode"),
            A("edgeMode"),
            A("faceMode"),
            Sep,
            A("selectEdgeLoop"),
            A("selectEdgeRing"),
            A("selectBorder"),
            Sep,
            Sub("Convert Selection", vec![A("toVertices"), A("toEdges"), A("toFaces")]),
        ],
        "Modify" => vec![
            Sub("Transformation Tools", vec![A("moveTool"), A("rotateTool"), A("scaleTool")]),
            A("resetTransformations"),
            A("freezeTransformations"),
            Sep,
            A("centerPivot"),
            A("pivotMode"),
            A("snapToGround"),
            Sep,
            Sub("Convert", vec![A("convertNurbsToPoly")]),
        ],
        "Display" => vec![
            A("toggleGrid"),
            A("toggleHud"),
            Sep,
            A("hideSel"),
            A("showAll"),
            Sep,
            Sub("Polygons", vec![A("smooth1"), A("smooth2"), A("smooth3")]),
            Sep,
            Sub("UI Elements", vec![A("toggleShelf"), A("toggleToolbox"), A("toggleTimeline"), A("toggleCmdline"), A("channelBox"), A("outliner")]),
        ],
        "Windows" => vec![
            Sub("General Editors", vec![A("scriptEditor"), A("commandList")]),
            Sub("Rendering Editors", vec![A("hypershade"), A("renderView"), A("renderSettings")]),
            Sub("Animation Editors", vec![A("graphEditor")]),
            Sep,
            A("outliner"),
            A("attributeEditor"),
            A("channelBox"),
            A("toolSettings"),
            Sep,
            Sub("Settings/Preferences", vec![A("preferences"), A("hotkeyList")]),
        ],
        "Help" => vec![A("commandList"), A("hotkeyList"), Sep, A("about")],
        _ => vec![],
    }
}

pub fn set_menus(ms: MenuSet) -> Vec<(&'static str, Vec<M>)> {
    match ms {
        MenuSet::Modeling => vec![
            ("Mesh", vec![A("combine"), A("separate"), Sep, Sub("Booleans", vec![A("booleanUnion"), A("booleanDifference"), A("booleanIntersection")]), Sep, A("smooth"), A("triangulate"), A("quadrangulate"), A("mirror"), A("fillHole"), A("addDivisions")]),
            ("Edit Mesh", vec![A("extrude"), A("bevel"), A("bridge"), A("insertEdgeLoop"), Sep, A("merge"), A("mergeCenter"), A("poke"), A("deleteEdgeVertex"), Sep, A("extractFaces"), A("duplicateFaces")]),
            ("Mesh Display", vec![A("reverseNormals"), A("softenEdge"), A("hardenEdge")]),
            ("UV", vec![A("uvPlanar"), A("uvCylindrical"), A("uvSpherical"), A("uvAutomatic")]),
            ("Surfaces", vec![A("revolve"), A("loft"), A("extrudeCurve"), A("planar")]),
            ("Curves", vec![A("rebuildCurve"), A("reverseCurve")]),
        ],
        MenuSet::Rigging => vec![
            ("Skeleton", vec![A("jointTool"), Todo("Create IK Handle")]),
            ("Skin", vec![A("bindSkin"), Todo("Paint Skin Weights")]),
            ("Deform", vec![Sub("Nonlinear", vec![A("deformBend"), A("deformFlare"), A("deformSine"), A("deformSquash"), A("deformTwist"), A("deformWave")]), Todo("Lattice"), Todo("Blend Shape")]),
            ("Constrain", vec![A("cParent"), A("cPoint"), A("cOrient"), A("cScale"), A("cAim")]),
        ],
        MenuSet::Animation => vec![
            ("Key", vec![A("setKey"), A("keyTranslate"), A("keyRotate"), A("keyScale"), Sep, A("deleteKeys"), A("autoKey")]),
            ("Playback", vec![A("play"), A("nextFrame"), A("prevFrame"), A("nextKey"), A("prevKey"), A("goStart"), A("goEnd")]),
            ("Visualize", vec![A("graphEditor"), Todo("Create Editable Motion Trail"), Todo("Ghost Selected")]),
        ],
        MenuSet::Fx => vec![("nParticles", vec![Todo("Create Emitter")]), ("Fields/Solvers", vec![Todo("Gravity"), Todo("Turbulence"), Todo("Create Active Rigid Body")])],
        MenuSet::Rendering => vec![
            ("Lighting/Shading", vec![Sub("Assign New Material", vec![A("matLambert"), A("matBlinn"), A("matPhong"), A("matStandard"), A("matSurface")]), A("hypershade")]),
            ("Render", vec![A("renderFrame"), A("renderView"), A("renderSettings")]),
        ],
    }
}

pub fn items(app: &mut IncaApp, ui: &mut egui::Ui, list: &[M]) {
    for it in list {
        match it {
            A(id) => {
                let checked = toggle_state(app, id);
                let mut text = label(id);
                if let Some(c) = checked {
                    text = format!("{} {text}", if c { "✔" } else { "  " });
                }
                let b = egui::Button::new(text).shortcut_text(hotkey_label(id));
                if ui.add(b).clicked() {
                    app.action(id);
                    ui.close();
                }
            }
            Sub(l, v) => {
                ui.menu_button(*l, |ui| items(app, ui, v));
            }
            Sep => {
                ui.separator();
            }
            Todo(l) => {
                ui.add_enabled(false, egui::Button::new(format!("{l} (planned)")));
            }
        }
    }
}

/// Checkmark state for toggle actions.
pub fn toggle_state(app: &IncaApp, id: &str) -> Option<bool> {
    let v = &app.vps[app.active_vp];
    Some(match id {
        "toggleGrid" => v.grid,
        "toggleHud" => v.hud,
        "wireOnShaded" => v.wire_on_shaded,
        "xray" => v.xray,
        "backface" => v.backface,
        "toggleShelf" => app.show_shelf,
        "toggleTimeline" => app.show_timeline,
        "toggleCmdline" => app.show_cmdline,
        "toggleToolbox" => app.show_toolbox,
        "outliner" => app.show_outliner,
        "autoKey" => app.s.time.auto_key,
        _ => return None,
    })
}

pub fn menu_bar(app: &mut IncaApp, ui: &mut egui::Ui) {
    egui::MenuBar::new().ui(ui, |ui| {
        for name in ["File", "Edit", "Create", "Select", "Modify", "Display", "Windows"] {
            let list = common(name);
            ui.menu_button(name, |ui| {
                items(app, ui, &list);
                if name == "File" {
                    ui.separator();
                    ui.menu_button("Recent Files", |ui| {
                        if app.recent.is_empty() {
                            ui.add_enabled(false, egui::Button::new("(none)"));
                        }
                        for r in app.recent.clone() {
                            if ui.button(&r).clicked() {
                                app.open_path(std::path::Path::new(&r));
                                ui.close();
                            }
                        }
                    });
                }
            });
        }
        ui.separator();
        for (name, list) in set_menus(app.menu_set) {
            ui.menu_button(name, |ui| items(app, ui, &list));
        }
        let help = common("Help");
        ui.menu_button("Help", |ui| items(app, ui, &help));
    });
}
