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
    #[allow(dead_code)]
    Todo(&'static str),
    /// submenu built by code ("Workspaces", "UI Elements")
    Dyn(&'static str),
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
        "File" => vec![A("newScene"), A("openScene"), A("saveScene"), A("saveSceneAs"), A("incrementAndSave"), A("revertScene"), Sep, A("importFile"), A("exportAll"), A("exportSelection"), Sep, A("projectWindow"), A("setProject"), Sep, Dyn("Recent Files"), Dyn("Recent Projects"), Sep, A("quit")],
        "Edit" => vec![
            A("undo"),
            A("redo"),
            A("repeatLast"),
            Sep,
            A("cut"),
            A("copy"),
            A("paste"),
            Sep,
            Sub("Keys", vec![A("copyKeys"), A("pasteKeys"), A("deleteKeysCurrent"), A("deleteChannels"), A("bakeKeys")]),
            Sep,
            A("deleteSel"),
            Sub("Delete by Type", vec![A("deleteHistory"), A("deleteChannels"), A("deleteAllLights"), A("deleteAllCameras"), A("deleteAllCurves")]),
            Sub("Delete All by Type", vec![A("deleteAllHistory"), A("deleteAllLights"), A("deleteAllCameras"), A("deleteAllCurves")]),
            Sep,
            A("duplicate"),
            A("duplicateSpecial"),
            A("duplicateSpecialOptions"),
            A("duplicateWithTransform"),
            A("duplicateHistory"),
            Sep,
            A("group"),
            A("ungroup"),
            Sep,
            A("parent"),
            A("unparent"),
            Sep,
            A("searchReplace"),
            A("prefixHierarchy"),
            A("renameSel"),
        ],
        "Create" => vec![
            Sub("Polygon Primitives", poly_prims()),
            Sub("NURBS Primitives", nurbs_prims()),
            Sep,
            Sub("Lights", LIGHT_TYPES.iter().map(|l| A(l.0)).collect()),
            Sub("Cameras", vec![A("createCamera")]),
            Sep,
            Sub("Curve Tools", vec![A("cvCurveTool"), A("epCurveTool"), A("pencilCurveTool"), A("arcTool")]),
            A("createPolygonTool"),
            Sep,
            A("createEmptyGroup"),
            A("createLocator"),
        ],
        "Select" => vec![
            A("selectAll"),
            Sub("All by Type", vec![A("selectAllGeometry"), A("selectAllLights"), A("selectAllCameras"), A("selectAllCurves")]),
            A("deselectAll"),
            A("selectHierarchy"),
            A("invertSelection"),
            A("selectSimilar"),
            Sep,
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
            A("selectByName"),
        ],
        "Modify" => vec![
            Sub("Transformation Tools", vec![A("moveTool"), A("rotateTool"), A("scaleTool"), A("pivotEdit")]),
            A("resetTransformations"),
            A("freezeTransformations"),
            Sub("Match Transformations", vec![A("matchTranslation"), A("matchRotation"), A("matchAll")]),
            Sep,
            Sub("Pivot", vec![A("centerPivot"), A("pivotEdit")]),
            A("centerPivot"),
            A("snapToGround"),
            Sep,
            A("prefixHierarchy"),
            A("searchReplace"),
            Sep,
            Sub("Convert", vec![A("convertNurbsToPoly")]),
            A("renameSel"),
            Sep,
            A("addAttribute"),
        ],
        "Display" => vec![
            A("toggleGrid"),
            Sub("Heads Up Display", vec![A("hudPolyCount"), A("hudFrameRate"), A("hudCurrentFrame"), A("hudCameraNames"), A("hudViewAxis")]),
            Dyn("UI Elements"),
            Sep,
            Sub("Hide", vec![A("hideSelection"), A("hideUnselected"), Sep, A("hideAllLights"), A("hideAllCameras"), A("hideAllGeometry")]),
            Sub("Show", vec![A("showSelection"), A("showLastHidden"), A("showAll"), Sep, A("showAllLights"), A("showAllCameras"), A("showAllGeometry")]),
            Sub("Object Display", vec![A("templateSel"), A("untemplateSel"), A("untemplateAll")]),
            A("toggleWireOnShaded"),
            A("toggleXray"),
            A("toggleBackface"),
            A("cycleBackground"),
            Sep,
            Sub("Polygons", vec![A("smoothOff"), A("smoothCage"), A("smoothOn"), Sep, A("toggleNormals"), A("toggleBorderEdges"), A("softenEdge"), A("hardenEdge")]),
            A("frameAll"),
            A("frameSelected"),
            A("frameAllAll"),
            A("frameSelectedAll"),
            A("isolateSelect"),
        ],
        "Windows" => vec![
            Dyn("Workspaces"),
            Sep,
            Sub("General Editors", vec![A("scriptEditor"), A("melReference")]),
            Sub("Modeling Editors", vec![A("uvEditor")]),
            Sub("Rendering Editors", vec![A("hypershade"), A("renderView"), A("renderSettings"), A("playblast")]),
            Sub("Animation Editors", vec![A("graphEditor"), A("dopeSheet")]),
            Sep,
            A("outlinerWindow"),
            A("attributeEditor"),
            A("channelBox"),
            A("toolSettings"),
            Sep,
            Sub("Settings/Preferences", vec![A("preferences"), A("hotkeyList")]),
        ],
        "Help" => vec![A("melReference"), A("hotkeyList"), Sep, A("about")],
        _ => vec![],
    }
}

pub fn set_menus(ms: MenuSet) -> Vec<(&'static str, Vec<M>)> {
    match ms {
        MenuSet::Modeling => vec![
            ("Mesh", vec![Sub("Booleans", vec![A("booleanUnion"), A("booleanDifference"), A("booleanIntersection")]), A("combine"), A("separate"), Sep, A("fillHole"), A("smooth"), A("triangulate"), A("quadrangulate"), Sep, A("mirror"), Sep, A("cleanup")]),
            ("Edit Mesh", vec![A("addDivisions"), A("bevel"), A("bridge"), A("collapse"), A("connect"), A("chamferVertex"), A("extrude"), A("merge"), A("mergeToCenter"), A("poke"), Sep, A("deleteEdgeVertex"), Sep, A("extract"), A("duplicateFaces"), Sep, A("curveFromEdges")]),
            ("Mesh Tools", vec![A("createPolygonTool"), A("insertEdgeLoopTool"), A("multiCutTool"), A("quadDrawTool"), A("targetWeldTool"), A("sculptTool"), Sub("Sculpt Brushes", vec![A("smoothSculptTool"), A("relaxSculptTool"), A("grabSculptTool"), A("pinchSculptTool"), A("flattenSculptTool"), A("inflateSculptTool")]), A("makeLive"), Sep, A("insertEdgeLoop")]),
            ("Mesh Display", vec![A("softenEdge"), A("hardenEdge"), A("softHardAngle"), Sep, A("reverseNormals"), Sep, A("toggleNormals"), A("toggleBorderEdges"), Sep, A("smoothOff"), A("smoothOn")]),
            ("UV", vec![A("uvEditor"), Sep, A("planarMap"), A("cylindricalMap"), A("sphericalMap"), A("automaticMap")]),
            ("Surfaces", vec![A("revolve"), A("loft"), A("extrudeCurve"), A("planarSurface")]),
            ("Curves", vec![A("cvCurveTool"), A("epCurveTool"), A("pencilCurveTool"), Sep, A("reverseCurve"), A("rebuildCurve"), A("openCloseCurve"), Sep, A("curveFromEdges")]),
        ],
        MenuSet::Rigging => vec![
            ("Skeleton", vec![A("jointTool"), A("ikHandleTool"), Sep, A("insertJoint"), A("mirrorJoint"), A("orientJoint")]),
            ("Skin", vec![A("smoothBind"), A("unbindSkin"), Sep, A("paintWeights")]),
            ("Deform", vec![Sub("Nonlinear", vec![A("bendDeformer"), A("flareDeformer"), A("sineDeformer"), A("squashDeformer"), A("twistDeformer"), A("waveDeformer")]), Sep, A("createCluster"), A("softModDeformer"), A("createLattice"), A("latticePointTool"), A("blendShape"), Sep, A("deleteDeformers")]),
            ("Constrain", vec![A("parentConstraint"), A("pointConstraint"), A("orientConstraint"), A("scaleConstraint"), A("aimConstraint"), A("poleVectorConstraint"), Sep, A("removeConstraints"), Sep, A("attachToMotionPath")]),
            ("Control", vec![A("createLocator"), A("group"), Sep, A("setDrivenKeyWindow")]),
        ],
        MenuSet::Animation => vec![
            ("Key", vec![A("setKey"), A("keyTranslate"), A("keyRotate"), A("keyScale"), Sep, A("setDrivenKeyWindow"), Sep, A("copyKeys"), A("pasteKeys"), A("deleteKeysCurrent"), A("bakeKeys"), Sep, A("toggleAutoKey")]),
            ("Playback", vec![A("playToggle"), A("playBackwards"), A("stopPlayback"), Sep, A("nextFrame"), A("prevFrame"), A("nextKey"), A("prevKey"), A("goToStart"), A("goToEnd"), Sep, A("playblast")]),
            ("Visualize", vec![A("graphEditor"), A("dopeSheet"), Sep, A("createGhost"), A("unghostAll"), A("motionTrail")]),
        ],
        MenuSet::Fx => vec![
            ("Particles", vec![A("createEmitter"), A("createDirectionalEmitter"), A("createParticles"), A("emitFromObject")]),
            ("Fields/Solvers", vec![A("gravityField"), A("dragField"), A("turbulenceField"), A("radialField"), A("vortexField"), A("airField"), Sep, A("affectSelected"), Sep, A("createActiveRigidBody"), A("createPassiveRigidBody"), A("removeRigidBody"), Sep, A("dynamicsReset")]),
            ("Effects", vec![A("createFire"), A("createSmoke")]),
        ],
        MenuSet::Rendering => vec![
            ("Lighting/Shading", {
                let mut v: Vec<M> = LIGHT_TYPES.iter().map(|l| A(l.0)).collect();
                v.extend([Sep, A("hypershade"), A("assignNewMaterial"), Sub("Assign New Material", vec![A("assignNew_lambert"), A("assignNew_blinn"), A("assignNew_phong"), A("assignNew_standardSurface"), A("assignNew_surfaceShader")]), Dyn("Assign Existing Material"), A("materialAttributes")]);
                v
            }),
            ("Texturing", vec![A("planarMap"), A("automaticMap"), A("uvEditor")]),
            ("Render", vec![A("renderCurrent"), A("ipr"), A("renderSequence"), Sep, A("renderView"), A("renderSettings"), A("playblast")]),
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
                if crate::options::has_box(id) {
                    // the option box opens the command's options dialog
                    let mut opened = false;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        if ui.add(b).clicked() {
                            app.action(id);
                            ui.close();
                        }
                        if crate::icons::button_ex(ui, "optionBox", "Options", false, 16.0, 12.0, true).clicked() {
                            opened = true;
                        }
                    });
                    if opened {
                        crate::options::open(app, id);
                        ui.close();
                    }
                } else if ui.add(b).clicked() {
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
            Dyn(l) => {
                ui.menu_button(*l, |ui| match *l {
                    "Workspaces" => crate::workspace::menu(app, ui),
                    "Recent Files" | "Recent Projects" => {
                        let files = *l == "Recent Files";
                        let list = if files { app.recent.clone() } else { app.recent_projects.clone() };
                        if list.is_empty() {
                            ui.add_enabled(false, egui::Button::new("(none)"));
                        }
                        for r in list {
                            if ui.button(&r).clicked() {
                                if files {
                                    app.open_path(std::path::Path::new(&r));
                                } else {
                                    crate::project::set_project(app, &r, false);
                                }
                                ui.close();
                            }
                        }
                    }
                    "Assign Existing Material" => {
                        let mats: Vec<(String, String)> = app.s.mats.values().map(|m| (m.id.clone(), m.name.clone())).collect();
                        for (_, name) in mats {
                            if ui.button(&name).clicked() {
                                let _ = app.run_mel("assign material", &format!("hyperShade -assign {name};"));
                                ui.close();
                            }
                        }
                    }
                    _ => crate::workspace::elements_menu(app, ui),
                });
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
        "hudPolyCount" => app.hud.poly_count,
        "hudFrameRate" => app.hud.fps,
        "hudCurrentFrame" => app.hud.frame,
        "hudCameraNames" => app.hud.cam_names,
        "hudViewAxis" => app.hud.view_axis,
        "toggleWireOnShaded" => v.wire_on_shaded,
        "toggleXray" => v.xray,
        "toggleBackface" => v.backface,
        "toggleShelf" => app.shown("shelf"),
        "toggleTimeline" => app.shown("timeslider"),
        "toggleCmdline" => app.shown("cmdline"),
        "toggleToolbox" => app.shown("toolbox"),
        "outlinerWindow" => app.show_outliner,
        "toggleAutoKey" => app.s.time.auto_key,
        _ => return None,
    })
}

pub fn menu_bar(app: &mut IncaApp, ui: &mut egui::Ui) {
    egui::MenuBar::new().ui(ui, |ui| {
        for name in ["File", "Edit", "Create", "Select", "Modify", "Display", "Windows"] {
            let list = common(name);
            ui.menu_button(name, |ui| {
                items(app, ui, &list);
            });
        }
        ui.separator();
        for (name, list) in set_menus(app.menu_set) {
            ui.menu_button(name, |ui| items(app, ui, &list));
        }
        let help = common("Help");
        ui.menu_button("Help", |ui| items(app, ui, &help));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| crate::workspace::selector(app, ui));
    });
}

/// Maya's hotbox: every menu around the cursor while Space is held (stays until a click outside or Esc).
pub fn hotbox(app: &mut IncaApp, ctx: &egui::Context) {
    let Some(pos) = app.hotbox else { return };
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.hotbox = None;
        return;
    }
    let r = egui::Area::new(egui::Id::new("hotbox")).fixed_pos(pos - egui::vec2(260.0, 70.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                for m in MenuSet::ALL {
                    if ui.selectable_label(app.menu_set == m, m.label()).clicked() {
                        app.menu_set = m;
                    }
                }
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(520.0);
                for name in ["File", "Edit", "Create", "Select", "Modify", "Display", "Windows"] {
                    let list = common(name);
                    ui.menu_button(name, |ui| items(app, ui, &list));
                }
                for (name, list) in set_menus(app.menu_set) {
                    ui.menu_button(name, |ui| items(app, ui, &list));
                }
                let help = common("Help");
                ui.menu_button("Help", |ui| items(app, ui, &help));
            });
        });
    });
    let clicked_outside = ctx.input(|i| i.pointer.any_click()) && !r.response.contains_pointer() && !ctx.any_popup_open();
    if clicked_outside && app.space_down.is_none() {
        app.hotbox = None;
    }
}
