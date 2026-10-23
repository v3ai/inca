//! Actions: every menu item, shelf button and hotkey is an action id. Most map to MEL so they
//! echo in the Script Editor and undo as one step; the rest are UI built-ins.

use crate::app::{IncaApp, Layout, RightTab, Tool};
use crate::viewport::{self, LightMode, ShadeMode};
use egui::{Key, Modifiers};
use inca_core::primitives::GEN_INFO;
use inca_core::scene::{CompMode, SelMode, LIGHT_TYPES};
use inca_core::selection;

/// (id, label, hotkey shown in menus)
pub static ACTIONS: &[(&str, &str, &str)] = &[
    ("newScene", "New Scene", "Ctrl+N"),
    ("openScene", "Open Scene...", "Ctrl+O"),
    ("saveScene", "Save Scene", "Ctrl+S"),
    ("saveSceneAs", "Save Scene As...", "Ctrl+Shift+S"),
    ("cut", "Cut", "Ctrl+X"),
    ("copyKeys", "Copy Keys", ""),
    ("pasteKeys", "Paste Keys", ""),
    ("bakeKeys", "Bake Simulation", ""),
    ("deleteAllLights", "Lights (All)", ""),
    ("deleteAllCameras", "Cameras (All)", ""),
    ("deleteAllCurves", "NURBS Curves (All)", ""),
    ("duplicateSpecial", "Duplicate Special", "Ctrl+Shift+D"),
    ("duplicateSpecialOptions", "Duplicate Special Options...", ""),
    ("duplicateWithTransform", "Duplicate with Transform", "Shift+D"),
    ("searchReplace", "Search and Replace Names...", ""),
    ("prefixHierarchy", "Prefix Hierarchy Names...", ""),
    ("selectByName", "Select by Name...", ""),
    ("selectAllGeometry", "Geometry", ""),
    ("selectAllLights", "Lights", ""),
    ("selectAllCameras", "Cameras", ""),
    ("selectAllCurves", "NURBS Curves", ""),
    ("selectSimilar", "Similar", ""),
    ("incrementAndSave", "Increment & Save", "Ctrl+Alt+S"),
    ("revertScene", "Revert to Saved", ""),
    ("projectWindow", "Project Window", ""),
    ("setProject", "Set Project...", ""),
    ("importFile", "Import...", ""),
    ("exportAll", "Export All...", ""),
    ("exportSelection", "Export Selection...", ""),
    ("quit", "Exit", "Ctrl+Q"),
    ("undo", "Undo", "Ctrl+Z"),
    ("redo", "Redo", "Ctrl+Y"),
    ("repeatLast", "Repeat Last", "G"),
    ("copy", "Copy", "Ctrl+C"),
    ("paste", "Paste", "Ctrl+V"),
    ("deleteSel", "Delete", "Del"),
    ("deleteHistory", "History", ""),
    ("deleteAllHistory", "All History", ""),
    ("deleteChannels", "Channels (keys)", ""),
    ("duplicate", "Duplicate", "Ctrl+D"),
    ("duplicateHistory", "Duplicate with Input Graph", ""),
    ("group", "Group", "Ctrl+G"),
    ("ungroup", "Ungroup", ""),
    ("parent", "Parent", "P"),
    ("unparent", "Unparent", "Shift+P"),
    ("renameSel", "Rename...", ""),
    ("nurbsCircle", "Circle", ""),
    ("nurbsSquare", "Square", ""),
    ("createCamera", "Camera", ""),
    ("createLocator", "Locator", ""),
    ("createEmptyGroup", "Empty Group", "Ctrl+Shift+G"),
    ("cvCurveTool", "CV Curve Tool", ""),
    ("epCurveTool", "EP Curve Tool", ""),
    ("arcTool", "Three Point Circular Arc", ""),
    ("createPolygonTool", "Create Polygon Tool", ""),
    ("jointTool", "Create Joints", ""),
    ("ikHandleTool", "Create IK Handle", ""),
    ("insertJoint", "Insert Joint", ""),
    ("mirrorJoint", "Mirror Joints", ""),
    ("orientJoint", "Orient Joint", ""),
    ("smoothBind", "Bind Skin", ""),
    ("unbindSkin", "Unbind Skin", ""),
    ("paintWeights", "Paint Skin Weights", ""),
    ("createCluster", "Cluster", ""),
    ("softModDeformer", "Soft Modification", ""),
    ("createLattice", "Lattice", ""),
    ("latticePointTool", "Lattice Point Tool", ""),
    ("blendShape", "Blend Shape", ""),
    ("deleteDeformers", "Delete Deformers", ""),
    ("poleVectorConstraint", "Pole Vector", ""),
    ("removeConstraints", "Remove Constraints", ""),
    ("attachToMotionPath", "Attach to Motion Path", ""),
    ("setDrivenKeyWindow", "Set Driven Key...", ""),
    ("bendDeformer", "Bend", ""),
    ("twistDeformer", "Twist", ""),
    ("flareDeformer", "Flare", ""),
    ("sineDeformer", "Sine", ""),
    ("squashDeformer", "Squash", ""),
    ("waveDeformer", "Wave", ""),
    ("parentConstraint", "Parent", ""),
    ("pointConstraint", "Point", ""),
    ("orientConstraint", "Orient", ""),
    ("scaleConstraint", "Scale", ""),
    ("aimConstraint", "Aim", ""),
    ("createEmitter", "Create Emitter (omni)", ""),
    ("createDirectionalEmitter", "Create Emitter (directional)", ""),
    ("emitFromObject", "Emit from Object (surface)", ""),
    ("gravityField", "Gravity", ""),
    ("dragField", "Drag", ""),
    ("turbulenceField", "Turbulence", ""),
    ("radialField", "Radial", ""),
    ("vortexField", "Vortex", ""),
    ("airField", "Air", ""),
    ("createActiveRigidBody", "Create Active Rigid Body", ""),
    ("createPassiveRigidBody", "Create Passive Rigid Body", ""),
    ("createFire", "Fire", ""),
    ("createSmoke", "Smoke", ""),
    ("finishTool", "Complete Tool", "Enter"),
    ("toolUndoPoint", "Remove Last Point", "Backspace"),
    ("cancelTool", "Cancel Tool", "Esc"),
    ("selectAll", "All", "Ctrl+Shift+A"),
    ("deselectAll", "Deselect All", ""),
    ("selectHierarchy", "Hierarchy", ""),
    ("invertSelection", "Inverse", "Ctrl+Shift+I"),
    ("objectMode", "Object/Component", "F8"),
    ("vertexMode", "Vertex", "F9"),
    ("edgeMode", "Edge", "F10"),
    ("faceMode", "Face", "F11"),
    ("growSelection", "Grow", ">"),
    ("shrinkSelection", "Shrink", "<"),
    ("selectEdgeLoop", "Select Edge Loop", ""),
    ("selectEdgeRing", "Select Edge Ring", ""),
    ("selectBorder", "Select Border Edges", ""),
    ("toVertices", "To Vertices", ""),
    ("toEdges", "To Edges", ""),
    ("toFaces", "To Faces", ""),
    ("selectTool", "Select Tool", "Q"),
    ("moveTool", "Move Tool", "W"),
    ("rotateTool", "Rotate Tool", "E"),
    ("scaleTool", "Scale Tool", "R"),
    ("lastToolUsed", "Last Tool", "Y"),
    ("resetTransformations", "Reset Transformations", ""),
    ("freezeTransformations", "Freeze Transformations", ""),
    ("centerPivot", "Center Pivot", ""),
    ("snapToGround", "Snap to Ground", ""),
    ("matchTranslation", "Match Translation", ""),
    ("matchRotation", "Match Rotation", ""),
    ("matchAll", "Match Transformations", ""),
    ("addAttribute", "Add Attribute...", ""),
    ("convertNurbsToPoly", "NURBS to Polygons", ""),
    ("toggleGrid", "Grid", ""),
    ("toggleHud", "Heads Up Display", ""),
    ("toggleWireOnShaded", "Wireframe on Shaded", ""),
    ("toggleXray", "X-Ray", "Alt+A"),
    ("toggleBackface", "Backface Culling", ""),
    ("shadeWire", "Wireframe", "4"),
    ("shadeSmooth", "Smooth Shade All", "5"),
    ("shadeTextured", "Textured", "6"),
    ("shadeAllLights", "Use All Lights", "7"),
    ("shadeDefaultLights", "Use Default Lighting", ""),
    ("shadeFlatLights", "Flat Lighting", ""),
    ("smoothOff", "Smooth Mesh Preview Off", "1"),
    ("smoothCage", "Smooth Mesh Preview (cage)", "2"),
    ("smoothOn", "Smooth Mesh Preview", "3"),
    ("hideSelection", "Hide Selection", "Ctrl+H"),
    ("showSelection", "Show Selection", "Shift+H"),
    ("showLastHidden", "Show Last Hidden", "Ctrl+Shift+H"),
    ("hideUnselected", "Hide Unselected Objects", "Alt+H"),
    ("hideAllLights", "Lights", ""),
    ("hideAllCameras", "Cameras", ""),
    ("hideAllGeometry", "All Geometry", ""),
    ("showAllLights", "Lights", ""),
    ("showAllCameras", "Cameras", ""),
    ("showAllGeometry", "All Geometry", ""),
    ("templateSel", "Template", ""),
    ("untemplateSel", "Untemplate", ""),
    ("untemplateAll", "Untemplate All", ""),
    ("toggleNormals", "Face Normals", ""),
    ("toggleBorderEdges", "Border Edges", ""),
    ("frameSelectedAll", "Frame Selection in All Views", "Shift+F"),
    ("frameAllAll", "Frame All in All Views", "Shift+A"),
    ("hudPolyCount", "Poly Count", ""),
    ("hudFrameRate", "Frame Rate", ""),
    ("hudCurrentFrame", "Current Frame", ""),
    ("hudCameraNames", "Camera Names", ""),
    ("hudViewAxis", "View Axis", ""),
    ("showAll", "All", ""),
    ("isolateSelect", "Isolate Select", "Ctrl+1"),
    ("frameSelected", "Frame Selection", "F"),
    ("frameAll", "Frame All", "A"),
    ("layoutSingle", "Single Pane", ""),
    ("layoutFour", "Four Panes", ""),
    ("layoutTwoSide", "Two Panes Side by Side", ""),
    ("layoutTwoStack", "Two Panes Stacked", ""),
    ("layoutThree", "Three Panes Split Left", ""),
    ("toggleLayout", "Toggle Single / Four View", "Space"),
    ("outlinerWindow", "Outliner", ""),
    ("attributeEditor", "Attribute Editor", "Ctrl+A"),
    ("channelBox", "Channel Box / Layer Editor", ""),
    ("toolSettings", "Tool Settings", ""),
    ("scriptEditor", "Script Editor", ""),
    ("hypershade", "Hypershade", ""),
    ("graphEditor", "Graph Editor", ""),
    ("dopeSheet", "Dope Sheet", ""),
    ("renderView", "Render View", ""),
    ("renderSettings", "Render Settings", ""),
    ("preferences", "Preferences", ""),
    ("hotkeyList", "Hotkeys", ""),
    ("melReference", "MEL Command Reference", ""),
    ("about", "About Inca", ""),
    ("combine", "Combine", ""),
    ("separate", "Separate", ""),
    ("booleanUnion", "Union", ""),
    ("booleanDifference", "Difference", ""),
    ("booleanIntersection", "Intersection", ""),
    ("smooth", "Smooth", ""),
    ("triangulate", "Triangulate", ""),
    ("quadrangulate", "Quadrangulate", ""),
    ("mirror", "Mirror", ""),
    ("fillHole", "Fill Hole", ""),
    ("addDivisions", "Add Divisions", ""),
    ("extrude", "Extrude", "Ctrl+E"),
    ("bevel", "Bevel", "Ctrl+B"),
    ("bridge", "Bridge", ""),
    ("pivotEdit", "Edit Pivot", "Insert / hold D"),
    ("insertEdgeLoop", "Insert Edge Loop (on selected edge)", ""),
    ("cleanup", "Cleanup", ""),
    ("insertEdgeLoopTool", "Insert Edge Loop Tool", "Ctrl+K"),
    ("multiCutTool", "Multi-Cut", "Ctrl+Shift+X"),
    ("targetWeldTool", "Target Weld", ""),
    ("quadDrawTool", "Quad Draw", "Ctrl+Shift+Q"),
    ("sculptTool", "Sculpt Tool", ""),
    ("smoothSculptTool", "Smooth Sculpt Tool", ""),
    ("relaxSculptTool", "Relax Sculpt Tool", ""),
    ("grabSculptTool", "Grab Sculpt Tool", ""),
    ("pinchSculptTool", "Pinch Sculpt Tool", ""),
    ("flattenSculptTool", "Flatten Sculpt Tool", ""),
    ("inflateSculptTool", "Inflate Sculpt Tool", ""),
    ("makeLive", "Make Live", ""),
    ("uvEditor", "UV Editor", ""),
    ("softSelect", "Soft Select", "B"),
    ("componentTypeCycle", "Cycle Component Type", "Shift+Q"),
    ("symmetryOff", "Symmetry: Off", ""),
    ("symmetryObjectX", "Symmetry: Object X", ""),
    ("symmetryWorldX", "Symmetry: World X", ""),
    ("symmetryWorldY", "Symmetry: World Y", ""),
    ("symmetryWorldZ", "Symmetry: World Z", ""),
    ("pencilCurveTool", "Pencil Curve Tool", ""),
    ("openCloseCurve", "Open/Close", ""),
    ("collapse", "Collapse", ""),
    ("connect", "Connect", ""),
    ("chamferVertex", "Chamfer Vertices", ""),
    ("curveFromEdges", "Polygon Edges to Curve", ""),
    ("softHardAngle", "Soften/Harden Edges (30°)", ""),
    ("merge", "Merge", ""),
    ("mergeToCenter", "Merge To Center", ""),
    ("poke", "Poke", ""),
    ("deleteEdgeVertex", "Delete Edge/Vertex", "Ctrl+Del"),
    ("extract", "Extract", ""),
    ("duplicateFaces", "Duplicate", ""),
    ("reverseNormals", "Reverse", ""),
    ("softenEdge", "Soften Edge", ""),
    ("hardenEdge", "Harden Edge", ""),
    ("planarMap", "Planar", ""),
    ("cylindricalMap", "Cylindrical", ""),
    ("sphericalMap", "Spherical", ""),
    ("automaticMap", "Automatic", ""),
    ("revolve", "Revolve", ""),
    ("loft", "Loft", ""),
    ("extrudeCurve", "Extrude", ""),
    ("planarSurface", "Planar", ""),
    ("rebuildCurve", "Rebuild", ""),
    ("reverseCurve", "Reverse Direction", ""),
    ("setKey", "Set Key", "S"),
    ("keyTranslate", "Key Translate", "Shift+W"),
    ("keyRotate", "Key Rotate", "Shift+E"),
    ("keyScale", "Key Scale", "Shift+R"),
    ("deleteKeysCurrent", "Delete Keys", ""),
    ("playToggle", "Play / Stop", "Alt+V"),
    ("playBackwards", "Play Backwards", ""),
    ("stopPlayback", "Stop", "Esc"),
    ("nextFrame", "Next Frame", "Alt+."),
    ("prevFrame", "Previous Frame", "Alt+,"),
    ("nextKey", "Next Key", "."),
    ("prevKey", "Previous Key", ","),
    ("goToStart", "Go to Start", "Shift+Alt+V"),
    ("goToEnd", "Go to End", ""),
    ("toggleAutoKey", "Auto Key", ""),
    ("assignNew_lambert", "Lambert", ""),
    ("assignNew_blinn", "Blinn", ""),
    ("assignNew_phong", "Phong", ""),
    ("assignNew_standardSurface", "Standard Surface", ""),
    ("assignNew_surfaceShader", "Surface Shader", ""),
    ("renderCurrent", "Render Current Frame", ""),
    ("snapGrid", "Snap to Grids (hold X)", ""),
    ("manipBigger", "Increase Manipulator Size", "="),
    ("manipSmaller", "Decrease Manipulator Size", "-"),
    ("toggleWorld", "World / Object space", ""),
    ("toggleHistory", "Construction History", ""),
    ("toggleShelf", "Shelf", ""),
    ("toggleTimeline", "Time Slider", ""),
    ("toggleCmdline", "Command Line", ""),
    ("toggleToolbox", "Tool Box", ""),
    ("toggleUIElements", "Hide/Show UI Elements", "Ctrl+Space"),
    ("cycleBackground", "Toggle Background Color", "Alt+B"),
];

pub fn label(id: &str) -> String {
    if let Some(a) = ACTIONS.iter().find(|a| a.0 == id) {
        return a.1.to_string();
    }
    if let Some(g) = GEN_INFO.iter().find(|g| g.ty == id) {
        return g.label.to_string();
    }
    if let Some(l) = LIGHT_TYPES.iter().find(|l| l.0 == id) {
        return l.1.to_string();
    }
    id.to_string()
}

/// Does the Rust app implement this action (menus / shelves dim the rest)?
pub fn available(id: &str) -> bool {
    ACTIONS.iter().any(|a| a.0 == id) || GEN_INFO.iter().any(|g| g.ty == id) || LIGHT_TYPES.iter().any(|l| l.0 == id)
}

pub fn hotkey_label(id: &str) -> &'static str {
    ACTIONS.iter().find(|a| a.0 == id).map(|a| a.2).unwrap_or("")
}

/// Maya's echoed creation command for a primitive (all defaults spelled out).
fn primitive_mel(ty: &str, history: bool) -> String {
    let g = GEN_INFO.iter().find(|g| g.ty == ty).unwrap();
    let mut s = String::from(ty);
    for (short, long) in g.short {
        if let Some(v) = g.params.iter().find(|p| p.0 == *long) {
            if !s.contains(&format!(" -{short} ")) && !g.short.iter().take_while(|x| x.0 != *short).any(|x| x.1 == *long) {
                s += &format!(" -{short} {}", inca_core::math::fmt_num_long(v.1));
            }
        }
    }
    s += &format!(" -ch {};", history as i32);
    s
}

/// MEL for an action, if it is a scripted one.
/// Attach to Motion Path: the selected objects follow the curve selected last.
fn motion_path_mel(app: &IncaApp) -> Option<String> {
    let curve = app.s.sel.iter().rev().find(|i| app.s.node(i).is_some_and(|n| matches!(n.shape, inca_core::scene::Shape::Curve(_))))?;
    let objs: Vec<String> = app.s.sel.iter().filter(|i| *i != curve).map(|i| app.s.name_of(i)).collect();
    if objs.is_empty() {
        return None;
    }
    let f = crate::options::get(app, "attachToMotionPath", "follow") as i64;
    Some(format!("pathAnimation -c {} -f {f} -fa x -ua y {};", app.s.name_of(curve), objs.join(" ")))
}

pub fn mel_for(app: &IncaApp, id: &str) -> Option<String> {
    if GEN_INFO.iter().any(|g| g.ty == id) {
        return Some(primitive_mel(id, app.history_on));
    }
    if LIGHT_TYPES.iter().any(|l| l.0 == id) {
        return Some(format!("{id};"));
    }
    let ch = app.history_on as i32;
    let opt = |a: &str, k: &str| crate::options::get(app, a, k);
    let f = inca_core::math::fmt_num_long;
    Some(
        match id {
            "nurbsCircle" => format!("circle -c 0 0 0 -nr 0 1 0 -sw 360 -r 1 -d 3 -s 8 -ch {ch};"),
            "nurbsSquare" => "nurbsSquare -nr 0 1 0 -c 0 0 0 -sl1 1 -sl2 1;".into(),
            "createCamera" => "camera;".into(),
            "createLocator" => "spaceLocator -p 0 0 0;".into(),
            "createEmptyGroup" => "group -em;".into(),
            "deleteSel" => "delete;".into(),
            "deleteHistory" => "delete -ch;".into(),
            "deleteAllHistory" => "delete -all -ch;".into(),
            "deleteChannels" => "delete -c;".into(),
            "duplicate" => "duplicate -rr;".into(),
            "duplicateHistory" => "duplicate -rr -un;".into(),
            "group" => if opt("group", "pivot") == 0.0 { "group; xform -cp;".into() } else { "group;".into() },
            "ungroup" => return ungroup_mel(app),
            "parent" => "parent;".into(),
            "unparent" => "parent -w;".into(),
            "resetTransformations" => "makeIdentity;".into(),
            "freezeTransformations" => "makeIdentity -apply true -t 1 -r 1 -s 1 -n 0 -pn 1;".into(),
            "centerPivot" => "xform -cp;".into(),
            "convertNurbsToPoly" => "nurbsToPoly -f 3 -pt 1;".into(),
            "hideSelection" => "hide;".into(),
            "showSelection" => "showHidden;".into(),
            "showAll" => "showHidden -all;".into(),
            "combine" => format!("polyUnite -ch {ch};"),
            "separate" => "polySeparate;".into(),
            "booleanUnion" => "polyCBoolOp -op 1;".into(),
            "booleanDifference" => "polyCBoolOp -op 2;".into(),
            "booleanIntersection" => "polyCBoolOp -op 3;".into(),
            "smooth" => format!("polySmooth -dv {} -ch {ch};", opt("smooth", "divisions") as i64),
            "triangulate" => "polyTriangulate;".into(),
            "quadrangulate" => "polyQuad -a 30;".into(),
            "mirror" => format!("polyMirrorFace -ma {} -p 0 -mm {} -mt {};", opt("mirror", "axis") as i64, opt("mirror", "merge") as i64, f(opt("mirror", "threshold"))),
            "fillHole" => "polyCloseBorder;".into(),
            "addDivisions" => format!("polySubdivideFacet -dv {};", opt("addDivisions", "divisions") as i64),
            "extrude" => match app.s.comp_mode {
                Some(CompMode::Edge) => "polyExtrudeEdge -off 0.5;".into(),
                _ => format!("polyExtrudeFacet -ltz {} -off {} -d {} -kft {} -ch {ch};", f(opt("extrude", "thickness")), f(opt("extrude", "offset")), opt("extrude", "divisions").max(1.0) as i64, opt("extrude", "keepFacesTogether") as i64),
            },
            "bridge" => format!("polyBridgeEdge -dv {};", opt("bridge", "divisions") as i64),
            "bevel" => format!("polyBevel -f {} -ch {ch};", f(opt("bevel", "fraction"))),
            "smoothBind" => format!("skinCluster -tsb -mi {};", opt("smoothBind", "maxInfluences") as i64),
            "insertJoint" => "insertJoint;".into(),
            "mirrorJoint" => format!("mirrorJoint -{} -mb -sr \"left\" \"right\";", ["myz", "mxz", "mxy"][(opt("mirrorJoint", "axis") as usize).min(2)]),
            "orientJoint" => "joint -e -oj xyz -sao yup -ch;".into(),
            "unbindSkin" => "skinCluster -e -ub;".into(),
            "createCluster" => "cluster;".into(),
            "softModDeformer" => format!("softMod -fr {};", f(opt("softModDeformer", "falloffRadius"))),
            "createLattice" => format!("lattice -dv {} {} {} -oc true;", opt("createLattice", "s") as i64, opt("createLattice", "t") as i64, opt("createLattice", "u") as i64),
            "blendShape" => "blendShape;".into(),
            "deleteDeformers" => "deleteDeformers;".into(),
            "poleVectorConstraint" => "poleVectorConstraint;".into(),
            "removeConstraints" => "removeConstraints;".into(),
            "attachToMotionPath" => return motion_path_mel(app),
            "createEmitter" => "emitter -type omni -r 100 -spd 1;".into(),
            "createDirectionalEmitter" => "emitter -type directional -r 100 -spd 3 -dx 0 -dy 1 -dz 0;".into(),
            "emitFromObject" => "emitter -type surface -r 200 -spd 0.5;".into(),
            "gravityField" => "gravity -m 9.8;".into(),
            "dragField" => "drag -m 0.5;".into(),
            "turbulenceField" => "turbulence -m 2;".into(),
            "radialField" => "radial -m 5 -mxd 5;".into(),
            "vortexField" => "vortex -m 5;".into(),
            "airField" => "air -m 4 -dx 1;".into(),
            "createActiveRigidBody" => "rigidBody -active -m 1 -b 0.6;".into(),
            "createPassiveRigidBody" => "rigidBody -passive;".into(),
            "createFire" => "fireEffect;".into(),
            "createSmoke" => "smokeEffect;".into(),
            "bendDeformer" => "nonLinear -type bend -curvature 0.5;".into(),
            "twistDeformer" => "nonLinear -type twist -endAngle 90;".into(),
            "flareDeformer" => "nonLinear -type flare -endFlareX 1.5 -endFlareZ 1.5;".into(),
            "sineDeformer" => "nonLinear -type sine -amplitude 0.2;".into(),
            "squashDeformer" => "nonLinear -type squash -factor -0.3;".into(),
            "waveDeformer" => "nonLinear -type wave -amplitude 0.1;".into(),
            "parentConstraint" => "parentConstraint -mo;".into(),
            "pointConstraint" => "pointConstraint -mo;".into(),
            "orientConstraint" => "orientConstraint -mo;".into(),
            "scaleConstraint" => "scaleConstraint -mo;".into(),
            "aimConstraint" => "aimConstraint;".into(),
            "insertEdgeLoop" => "polySplitRing -wt 0.5;".into(),
            "cleanup" => "polyCleanup;".into(),
            "collapse" => if app.s.comp_mode == Some(inca_core::scene::CompMode::Face) { "polyCollapseFacet;" } else { "polyCollapseEdge;" }.into(),
            "connect" => "polyConnectComponents;".into(),
            "chamferVertex" => format!("polyChamferVtx -w {};", f(opt("chamferVertex", "width"))),
            "curveFromEdges" => "polyToCurve -form 2 -degree 1;".into(),
            "softHardAngle" => "polySoftEdge -a 30;".into(),
            "merge" => format!("polyMergeVertex -d {};", f(opt("merge", "threshold"))),
            "poke" => "polyPoke;".into(),
            "deleteEdgeVertex" => "delete;".into(),
            "extract" => "polyChipOff -dup 0;".into(),
            "duplicateFaces" => "polyChipOff -dup 1;".into(),
            "reverseNormals" => "polyNormal;".into(),
            "softenEdge" => "polySoftEdge -a 180;".into(),
            "hardenEdge" => "polySoftEdge -a 0;".into(),
            "planarMap" => format!("polyProjection -t planar -md {};", ["x", "y", "z"][(opt("planarMap", "axis") as usize).min(2)]),
            "cylindricalMap" => "polyProjection -t cylindrical;".into(),
            "sphericalMap" => "polyProjection -t spherical;".into(),
            "automaticMap" => "polyProjection -t automatic;".into(),
            "revolve" => format!("revolve -ax {} -ssw {} -esw {} -s {} -ch {ch};", opt("revolve", "axis") as i64, f(opt("revolve", "startSweep")), f(opt("revolve", "endSweep")), opt("revolve", "sections") as i64),
            "loft" => format!("loft -ch {ch};"),
            "extrudeCurve" => format!("extrude -ch {ch};"),
            "planarSurface" => format!("planarSrf -ch {ch};"),
            "rebuildCurve" => "rebuildCurve -s 4 -d 3;".into(),
            "reverseCurve" => "reverseCurve;".into(),
            "setKey" => "setKeyframe;".into(),
            "keyTranslate" => "setKeyframe -at translate;".into(),
            "keyRotate" => "setKeyframe -at rotate;".into(),
            "keyScale" => "setKeyframe -at scale;".into(),
            "matchTranslation" => "matchTransform -pos;".into(),
            "matchRotation" => "matchTransform -rot;".into(),
            "matchAll" => "matchTransform;".into(),
            "deleteKeysCurrent" => format!("cutKey -t {};", inca_core::math::fmt_num_long(app.s.time.current)),
            "assignNew_lambert" => "string $m = `shadingNode -asShader lambert`; hyperShade -assign $m;".into(),
            "assignNew_blinn" => "string $m = `shadingNode -asShader blinn`; hyperShade -assign $m;".into(),
            "assignNew_phong" => "string $m = `shadingNode -asShader phong`; hyperShade -assign $m;".into(),
            "assignNew_standardSurface" => "string $m = `shadingNode -asShader standardSurface`; hyperShade -assign $m;".into(),
            "assignNew_surfaceShader" => "string $m = `shadingNode -asShader surfaceShader`; hyperShade -assign $m;".into(),
            _ => return None,
        },
    )
    .map(|m| {
        // key translate/rotate/scale expand to the three channels
        m.replace("setKeyframe -at translate;", "setKeyframe -at translateX -at translateY -at translateZ;")
            .replace("setKeyframe -at rotate;", "setKeyframe -at rotateX -at rotateY -at rotateZ;")
            .replace("setKeyframe -at scale;", "setKeyframe -at scaleX -at scaleY -at scaleZ;")
    })
}

fn ungroup_mel(app: &IncaApp) -> Option<String> {
    let groups: Vec<String> = app.s.sel.iter().filter(|i| app.s.node(i).is_some_and(|n| n.kind == "group")).map(|i| app.s.name_of(i)).collect();
    if groups.is_empty() {
        return None;
    }
    // ungroup = parent the children one level up, then delete the group
    let mut m = String::new();
    for g in groups {
        m += &format!("string $c[] = `listRelatives -c {g}`; string $p[] = `listRelatives -p {g}`; if (size($p)) {{ for ($x in $c) parent $x $p[0]; }} else {{ for ($x in $c) parent -w $x; }} delete {g};\n");
    }
    Some(m)
}

/// Actions that aren't plain MEL.
pub fn builtin(app: &mut IncaApp, id: &str) {
    let vp = app.active_vp;
    match id {
        "undo" => match app.s.undo() {
            Some(l) => {
                app.echo("undo;");
                app.last_result = format!("// Undo: {l}");
                app.smooth_cache.clear();
            }
            None => app.last_result = "// Warning: There are no more commands to undo.".into(),
        },
        "redo" => match app.s.redo() {
            Some(l) => {
                app.echo("redo;");
                app.last_result = format!("// Redo: {l}");
            }
            None => app.last_result = "// Warning: There are no more commands to redo.".into(),
        },
        "repeatLast" => {
            if let Some(a) = app.last_action.clone() {
                app.action(&a);
            }
        }
        "newScene" => {
            app.s.checkpoint("new scene");
            app.s.reset();
            app.echo("file -f -new;");
            app.smooth_cache.clear();
        }
        "openScene" => {
            if let Some(p) = rfd::FileDialog::new().add_filter("Inca scene", &["inca"]).set_directory(crate::project::default_dir(app, "scenes")).pick_file() {
                app.open_path(&p);
            }
        }
        "saveScene" | "saveSceneAs" => {
            let path = if id == "saveScene" { app.s.file.clone() } else { None };
            let path = path.or_else(|| rfd::FileDialog::new().add_filter("Inca scene", &["inca"]).set_directory(crate::project::default_dir(app, "scenes")).set_file_name("untitled.inca").save_file());
            if let Some(p) = path {
                crate::project::save_to(app, &p);
            }
        }
        "incrementAndSave" => crate::project::increment_and_save(app),
        "revertScene" => crate::project::revert(app),
        "projectWindow" => crate::project::open_project_window(app),
        "setProject" => {
            let start = app.s.project.clone().map(std::path::PathBuf::from).unwrap_or_else(|| crate::project::default_dir(app, ""));
            if let Some(d) = rfd::FileDialog::new().set_title("Set Project").set_directory(start).pick_folder() {
                crate::project::set_project(app, &d.display().to_string(), false);
            }
        }
        "importFile" => {
            if let Some(p) = rfd::FileDialog::new().add_filter("Scenes and models", inca_core::io::IMPORT_EXTS).pick_file() {
                let _ = app.run_mel("import", &format!("file -i \"{}\";", p.display().to_string().replace('\\', "/")));
            }
        }
        "exportAll" | "exportSelection" => {
            if let Some(p) = rfd::FileDialog::new().add_filter("Export", inca_core::io::EXPORT_EXTS).set_file_name("export.obj").save_file() {
                let flag = if id == "exportAll" { "-ea" } else { "-es" };
                let _ = app.exec_mel(&format!("file {flag} \"{}\";", p.display().to_string().replace('\\', "/")), true);
            }
        }
        "quit" => app.panels.quit = true,
        "cut" => {
            app.action("copy");
            app.action("deleteSel");
        }
        "copyKeys" => {
            let ks = app.s.anim.key_times(&app.s.sel);
            if let (Some(a), Some(b)) = (ks.first().copied(), ks.last().copied()) {
                crate::timeline::copy_keys(app, a, b);
            }
        }
        "pasteKeys" => crate::timeline::paste_keys(app),
        "bakeKeys" => crate::edit_cmds::bake(app),
        "deleteAllLights" => crate::edit_cmds::delete_by_type(app, "light"),
        "deleteAllCameras" => crate::edit_cmds::delete_by_type(app, "camera"),
        "deleteAllCurves" => crate::edit_cmds::delete_by_type(app, "curve"),
        "duplicateSpecial" => {
            let o = app.dup_opts.clone();
            crate::edit_cmds::duplicate_special(app, &o);
        }
        "duplicateSpecialOptions" => app.dup_special_open = true,
        "duplicateWithTransform" => crate::edit_cmds::duplicate_with_transform(app),
        "searchReplace" => crate::edit_cmds::ask(app, crate::edit_cmds::PromptKind::SearchFor),
        "prefixHierarchy" => {
            if app.s.sel.is_empty() {
                app.last_result = "// Warning: Nothing selected".into();
            } else {
                crate::edit_cmds::ask(app, crate::edit_cmds::PromptKind::Prefix);
            }
        }
        "selectByName" => crate::edit_cmds::ask(app, crate::edit_cmds::PromptKind::SelectByName),
        "selectAllGeometry" => crate::edit_cmds::select_by_type(app, "geometry"),
        "selectAllLights" => crate::edit_cmds::select_by_type(app, "light"),
        "selectAllCameras" => crate::edit_cmds::select_by_type(app, "camera"),
        "selectAllCurves" => crate::edit_cmds::select_by_type(app, "curve"),
        "selectSimilar" => crate::edit_cmds::select_similar(app),
        "insertEdgeLoopTool" => app.set_tool(Tool::EdgeLoop),
        "multiCutTool" => app.set_tool(Tool::MultiCut),
        "targetWeldTool" => app.set_tool(Tool::TargetWeld),
        "quadDrawTool" => app.set_tool(Tool::QuadDraw),
        "sculptTool" | "smoothSculptTool" | "relaxSculptTool" | "grabSculptTool" | "pinchSculptTool" | "flattenSculptTool" | "inflateSculptTool" => {
            let b = id.trim_end_matches("SculptTool").trim_end_matches("Tool");
            app.mesh_opts.brush = crate::mesh_tools::BRUSHES.iter().position(|x| *x == b).unwrap_or(0);
            app.set_tool(Tool::Sculpt);
            app.right_tab = crate::app::RightTab::ToolSettings;
            app.show_right = true;
        }
        "makeLive" => crate::mesh_tools::make_live(app),
        "uvEditor" => crate::uv_editor::open(app),
        "softSelect" => {
            app.soft.on = !app.soft.on;
            app.help = format!("Soft Select: {}", if app.soft.on { "ON" } else { "OFF" });
        }
        "symmetryOff" => app.symmetry = 0,
        "symmetryObjectX" => app.symmetry = 1,
        "symmetryWorldX" => app.symmetry = 2,
        "symmetryWorldY" => app.symmetry = 3,
        "symmetryWorldZ" => app.symmetry = 4,
        "componentTypeCycle" => {
            use inca_core::scene::CompMode;
            let next = match app.s.comp_mode {
                Some(CompMode::Vertex) => CompMode::Edge,
                Some(CompMode::Edge) => CompMode::Face,
                _ => CompMode::Vertex,
            };
            app.set_comp_mode(Some(next));
        }
        "pencilCurveTool" => app.set_tool(Tool::Pencil),
        "openCloseCurve" => {
            let mel: String = app
                .s
                .sel
                .iter()
                .filter_map(|i| match &app.s.node(i)?.shape {
                    inca_core::scene::Shape::Curve(c) => Some(format!("{} {};\n", if c.form == "periodic" { "openCurve" } else { "closeCurve" }, app.s.name_of(i))),
                    _ => None,
                })
                .collect();
            if mel.is_empty() {
                app.last_result = "// Warning: Select a curve".into();
            } else {
                let _ = app.run_mel("open/close", &mel);
            }
        }
        "showLastHidden" => crate::edit_cmds::show_last_hidden(app),
        "hideUnselected" => crate::edit_cmds::hide_unselected(app),
        "hideAllLights" => crate::edit_cmds::show_type(app, "light", false),
        "hideAllCameras" => crate::edit_cmds::show_type(app, "camera", false),
        "hideAllGeometry" => crate::edit_cmds::show_type(app, "geometry", false),
        "showAllLights" => crate::edit_cmds::show_type(app, "light", true),
        "showAllCameras" => crate::edit_cmds::show_type(app, "camera", true),
        "showAllGeometry" => crate::edit_cmds::show_type(app, "geometry", true),
        "templateSel" => crate::edit_cmds::template(app, true, false),
        "untemplateSel" => crate::edit_cmds::template(app, false, false),
        "untemplateAll" => crate::edit_cmds::template(app, false, true),
        "toggleNormals" => crate::edit_cmds::toggle_overlay(app, true),
        "toggleBorderEdges" => crate::edit_cmds::toggle_overlay(app, false),
        "hudPolyCount" => app.hud.poly_count = !app.hud.poly_count,
        "hudFrameRate" => app.hud.fps = !app.hud.fps,
        "hudCurrentFrame" => app.hud.frame = !app.hud.frame,
        "hudCameraNames" => app.hud.cam_names = !app.hud.cam_names,
        "hudViewAxis" => app.hud.view_axis = !app.hud.view_axis,
        "frameSelectedAll" | "frameAllAll" => {
            let b = if id == "frameAllAll" || app.s.sel.is_empty() { viewport::scene_bbox(&app.s) } else { viewport::selection_bbox(&app.s) };
            for v in 0..app.vps.len() {
                let r = app.vps[v].rect;
                let aspect = if r.height() > 1.0 { (r.width() / r.height()) as f64 } else { 1.5 };
                let cam = app.vps[v].camera.clone();
                viewport::frame_box(&mut app.s, &cam, &b, aspect);
            }
        }
        "addAttribute" => app.add_attr = Some(crate::edit_cmds::AddAttr { name: String::new(), ty: 0, default: 0.0 }),
        "copy" => {
            let data: Vec<serde_json::Value> = app.s.sel.iter().flat_map(|i| app.s.subtree(i)).map(|i| app.s.serialize_node(&i)).collect();
            app.clipboard = Some(serde_json::Value::Array(data));
        }
        "paste" => {
            if let Some(serde_json::Value::Array(list)) = app.clipboard.clone() {
                app.s.checkpoint("paste");
                let mut map = std::collections::HashMap::new();
                let mut made = vec![];
                for d in list {
                    let parent = d["parent"].as_str().and_then(|p| map.get(p).cloned());
                    let nid = app.s.deserialize_node(&d, parent.as_deref(), true);
                    map.insert(d["id"].as_str().unwrap_or("").to_string(), nid.clone());
                    if parent.is_none() {
                        made.push(nid);
                    }
                }
                app.s.select(&made, SelMode::Replace);
            }
        }
        "renameSel" => {
            if let Some(l) = app.s.lead() {
                app.win.rename = Some(app.s.name_of(l));
            }
        }
        "selectAll" => {
            if app.s.comp_mode.is_some() {
                selection::all_components(&mut app.s, false);
            } else {
                let _ = app.exec_mel("select -all;", true);
            }
        }
        "deselectAll" => app.select_names(&[], SelMode::Replace),
        "selectHierarchy" => {
            let all: Vec<String> = app.s.sel.iter().flat_map(|i| app.s.subtree(i)).collect();
            app.select_names(&all, SelMode::Replace);
        }
        "invertSelection" => {
            if app.s.comp_mode.is_some() {
                selection::all_components(&mut app.s, true);
            } else {
                let inv: Vec<String> = app.s.roots.iter().filter(|r| !app.s.sel.contains(r) && !app.s.node(r).unwrap().startup()).cloned().collect();
                app.select_names(&inv, SelMode::Replace);
            }
        }
        "objectMode" => {
            let m = if app.s.comp_mode.is_some() { None } else { Some(CompMode::Vertex) };
            app.set_comp_mode(m);
        }
        "vertexMode" => app.set_comp_mode(Some(CompMode::Vertex)),
        "edgeMode" => app.set_comp_mode(Some(CompMode::Edge)),
        "faceMode" => app.set_comp_mode(Some(CompMode::Face)),
        "growSelection" => {
            selection::grow(&mut app.s, false);
            app.echo("GrowPolygonSelectionRegion;");
        }
        "shrinkSelection" => {
            selection::grow(&mut app.s, true);
            app.echo("ShrinkPolygonSelectionRegion;");
        }
        "selectEdgeLoop" => {
            selection::loops(&mut app.s, false);
            app.echo("SelectEdgeLoopSp;");
        }
        "selectEdgeRing" => {
            selection::loops(&mut app.s, true);
            app.echo("SelectEdgeRingSp;");
        }
        "selectBorder" => {
            selection::border(&mut app.s);
            app.echo("SelectPolygonSelectionBoundary;");
        }
        "toVertices" => {
            selection::convert(&mut app.s, CompMode::Vertex);
            app.echo("ConvertSelectionToVertices;");
        }
        "toEdges" => {
            selection::convert(&mut app.s, CompMode::Edge);
            app.echo("ConvertSelectionToEdges;");
        }
        "toFaces" => {
            selection::convert(&mut app.s, CompMode::Face);
            app.echo("ConvertSelectionToFaces;");
        }
        "selectTool" => app.set_tool(Tool::Select),
        "moveTool" => app.set_tool(Tool::Move),
        "rotateTool" => app.set_tool(Tool::Rotate),
        "scaleTool" => app.set_tool(Tool::Scale),
        "lastToolUsed" => app.set_tool(app.last_tool),
        "cvCurveTool" => app.set_tool(Tool::CvCurve),
        "epCurveTool" => app.set_tool(Tool::EpCurve),
        "arcTool" => app.set_tool(Tool::Arc),
        "createPolygonTool" => app.set_tool(Tool::Polygon),
        "ikHandleTool" => app.set_tool(Tool::IkHandle),
        "latticePointTool" => app.set_tool(Tool::LatticePoint),
        "paintWeights" => app.set_tool(Tool::PaintWeights),
        "setDrivenKeyWindow" => app.win.sdk = true,
        "jointTool" => {
            app.select_names(&[], SelMode::Replace);
            app.set_tool(Tool::Joint);
        }
        "finishTool" => match app.tool {
            Tool::MultiCut => crate::mesh_tools::multi_cut_complete(app),
            Tool::QuadDraw => crate::mesh_tools::quad_complete(app),
            _ => crate::vp_ui::finish_creation(app),
        },
        "toolUndoPoint" => {
            app.vp_state.tool_pts.pop();
            if app.mesh_tool.cut.pop().is_some() && app.mesh_tool.cut.is_empty() {
                app.mesh_tool.cut_obj = None;
            }
            if app.tool == Tool::QuadDraw {
                app.mesh_tool.dots.pop();
            }
        }
        "cancelTool" => {
            app.vp_state.tool_pts.clear();
            app.mesh_tool.cut.clear();
            app.mesh_tool.cut_obj = None;
            app.set_tool(Tool::Select);
        }
        "snapToGround" => {
            let mut m = String::new();
            for id in app.s.sel.clone() {
                let b = app.s.world_bbox(&id);
                if !b.is_empty() {
                    m += &format!("move -r 0 {} 0 {};\n", inca_core::math::fmt_num_long(-b.min.y), app.s.name_of(&id));
                }
            }
            if !m.is_empty() {
                let _ = app.run_mel("snap to ground", &m);
            }
        }
        "toggleGrid" => app.vps[vp].grid = !app.vps[vp].grid,
        "toggleHud" => app.vps[vp].hud = !app.vps[vp].hud,
        "toggleWireOnShaded" => app.vps[vp].wire_on_shaded = !app.vps[vp].wire_on_shaded,
        "toggleXray" => app.vps[vp].xray = !app.vps[vp].xray,
        "toggleBackface" => app.vps[vp].backface = !app.vps[vp].backface,
        "shadeWire" => app.vps[vp].shade = ShadeMode::Wire,
        "shadeSmooth" => app.vps[vp].shade = ShadeMode::Shaded,
        "shadeTextured" => app.vps[vp].shade = ShadeMode::Textured,
        "shadeAllLights" => app.vps[vp].lights = LightMode::All,
        "shadeDefaultLights" => app.vps[vp].lights = LightMode::Default,
        "shadeFlatLights" => app.vps[vp].lights = LightMode::Flat,
        "smoothOff" | "smoothCage" | "smoothOn" => {
            let lvl = match id {
                "smoothOff" => 0,
                "smoothCage" => 1,
                _ => 2,
            };
            let names: Vec<String> = app.s.sel.iter().flat_map(|i| app.s.subtree(i)).filter(|i| app.s.node(i).is_some_and(|n| n.mesh().is_some())).map(|i| app.s.name_of(&i)).collect();
            if !names.is_empty() {
                let m: String = names.iter().map(|n| format!("setAttr {n}.smoothLevel {lvl};\n")).collect();
                let _ = app.run_mel("smooth preview", &m);
            }
        }
        "isolateSelect" => {
            let v = &mut app.vps[vp];
            v.isolate = if v.isolate.is_some() || app.s.sel.is_empty() { None } else { Some(app.s.sel.clone()) };
        }
        "frameSelected" | "frameAll" => {
            let b = if id == "frameAll" || app.s.sel.is_empty() { viewport::scene_bbox(&app.s) } else { viewport::selection_bbox(&app.s) };
            let r = app.vps[vp].rect;
            let aspect = if r.height() > 1.0 { (r.width() / r.height()) as f64 } else { 1.5 };
            let cam = app.vps[vp].camera.clone();
            viewport::frame_box(&mut app.s, &cam, &b, aspect);
        }
        "layoutSingle" => app.layout = Layout::Single,
        "layoutFour" => app.layout = Layout::Four,
        "layoutTwoSide" => app.layout = Layout::TwoSide,
        "layoutTwoStack" => app.layout = Layout::TwoStack,
        "layoutThree" => app.layout = Layout::Three,
        "toggleLayout" => {
            app.layout = if app.layout == Layout::Single { Layout::Four } else { Layout::Single };
        }
        "outlinerWindow" => app.show_outliner = !app.show_outliner,
        "attributeEditor" => {
            app.show_right = true;
            app.right_tab = RightTab::AttributeEditor;
        }
        "channelBox" => {
            app.show_right = true;
            app.right_tab = RightTab::ChannelBox;
        }
        "toolSettings" => {
            app.show_right = true;
            app.right_tab = RightTab::ToolSettings;
        }
        "scriptEditor" => app.win.script_editor = !app.win.script_editor,
        "hypershade" => app.win.hypershade = !app.win.hypershade,
        "graphEditor" => app.win.graph_editor = !app.win.graph_editor,
        "dopeSheet" => app.win.dope_sheet = !app.win.dope_sheet,
        "renderView" => app.win.render_view = !app.win.render_view,
        "renderSettings" => app.win.render_settings = !app.win.render_settings,
        "preferences" => app.win.prefs = !app.win.prefs,
        "hotkeyList" => app.win.hotkeys = !app.win.hotkeys,
        "melReference" => app.win.command_list = !app.win.command_list,
        "about" => app.win.about = true,
        "mergeToCenter" => {
            let mut m = String::new();
            for id in app.s.hilite() {
                let v = app.s.comp_verts(&id);
                if v.len() > 1 {
                    m += &format!("polyMergeVertex -d 1000000 {};\n", v.iter().map(|x| format!("{}.vtx[{x}]", app.s.name_of(&id))).collect::<Vec<_>>().join(" "));
                }
            }
            if !m.is_empty() {
                let _ = app.run_mel("merge to center", &m);
            }
        }
        "playToggle" => app.toggle_play(1.0),
        "playBackwards" => app.toggle_play(-1.0),
        "stopPlayback" => {
            if app.playing {
                app.toggle_play(app.play_dir);
            }
        }
        "nextFrame" | "prevFrame" | "goToStart" | "goToEnd" => {
            let t = &app.s.time;
            let nt = match id {
                "nextFrame" => {
                    if t.current + 1.0 > t.end {
                        t.start
                    } else {
                        t.current + 1.0
                    }
                }
                "prevFrame" => {
                    if t.current - 1.0 < t.start {
                        t.end
                    } else {
                        t.current - 1.0
                    }
                }
                "goToStart" => t.start,
                _ => t.end,
            };
            app.s.set_time(nt);
        }
        "nextKey" | "prevKey" => {
            let ts = app.s.anim.key_times(&app.s.sel);
            let cur = app.s.time.current;
            let t = if id == "nextKey" { ts.iter().copied().find(|&t| t > cur + 1e-6).or(ts.first().copied()) } else { ts.iter().rev().copied().find(|&t| t < cur - 1e-6).or(ts.last().copied()) };
            if let Some(t) = t {
                app.s.set_time(t);
            }
        }
        "toggleAutoKey" => app.s.time.auto_key = !app.s.time.auto_key,
        "renderCurrent" => {
            app.win.render_view = true;
            let cam = app.vps[vp].camera.clone();
            app.windows_state.start_render(&app.s, &cam, 0);
        }
        "snapGrid" => app.snap_grid = !app.snap_grid,
        "manipBigger" | "manipSmaller" => {
            let f = if id == "manipBigger" { 1.15 } else { 1.0 / 1.15 };
            app.manip_size = (app.manip_size * f).clamp(0.2, 4.0);
            app.manip_size_shown = 2.5;
        }
        "pivotEdit" => {
            app.pivot_mode = !app.pivot_mode;
            if app.pivot_mode && !app.tool.is_manip() {
                app.set_tool(Tool::Move);
            }
        }
        "toggleWorld" => app.world_space = !app.world_space,
        "toggleHistory" => app.history_on = !app.history_on,
        "toggleShelf" => app.set_shown("shelf", !app.shown("shelf")),
        "toggleTimeline" => app.set_shown("timeslider", !app.shown("timeslider")),
        "toggleCmdline" => app.set_shown("cmdline", !app.shown("cmdline")),
        "toggleToolbox" => app.set_shown("toolbox", !app.shown("toolbox")),
        "toggleUIElements" => crate::workspace::toggle_all(app),
        "cycleBackground" => app.bg_mode = (app.bg_mode + 1) % crate::viewport::BG_MODES.len(),
        _ => app.last_result = format!("// Warning: {} is not available yet.", label(id)),
    }
}

/// Maya default hotkeys.
pub fn hotkey(key: Key, m: Modifiers, app: &IncaApp) -> Option<&'static str> {
    let ctrl = m.command || m.ctrl;
    if crate::mesh_tools::is_mesh_tool(app.tool) {
        match key {
            Key::Enter => return Some("finishTool"),
            Key::Backspace if !app.mesh_tool.cut.is_empty() || !app.mesh_tool.dots.is_empty() => return Some("toolUndoPoint"),
            Key::Escape => return Some("cancelTool"),
            _ => {}
        }
    }
    if app.tool.is_creation() {
        match key {
            Key::Enter => return Some("finishTool"),
            Key::Backspace if !app.vp_state.tool_pts.is_empty() => return Some("toolUndoPoint"),
            Key::Escape => return Some("cancelTool"),
            _ => {}
        }
    }
    Some(match (key, ctrl, m.shift, m.alt) {
        (Key::Space, true, false, false) => "toggleUIElements",
        (Key::N, true, false, _) => "newScene",
        (Key::O, true, false, _) => "openScene",
        (Key::S, true, false, true) => "incrementAndSave",
        (Key::S, true, false, _) => "saveScene",
        (Key::S, true, true, _) => "saveSceneAs",
        (Key::Q, true, false, _) => "quit",
        (Key::Z, true, false, _) => "undo",
        (Key::Z, true, true, _) | (Key::Y, true, _, _) => "redo",
        (Key::C, true, _, _) => "copy",
        (Key::V, true, _, _) => "paste",
        (Key::D, true, true, _) => "duplicateSpecial",
        (Key::K, true, false, _) => "insertEdgeLoopTool",
        (Key::X, true, true, _) => "multiCutTool",
        (Key::Q, true, true, _) => "quadDrawTool",
        (Key::D, true, false, _) => "duplicate",
        (Key::D, false, true, false) => "duplicateWithTransform",
        (Key::X, true, false, false) => "cut",
        (Key::G, true, true, _) => "createEmptyGroup",
        (Key::G, true, false, _) => "group",
        (Key::H, true, false, _) => "hideSelection",
        (Key::H, false, true, false) => "showSelection",
        (Key::H, true, true, _) => "showLastHidden",
        (Key::H, false, false, true) => "hideUnselected",
        (Key::A, true, true, _) => "selectAll",
        (Key::A, true, false, _) => "attributeEditor",
        (Key::I, true, true, _) => "invertSelection",
        (Key::E, true, _, _) => "extrude",
        (Key::B, true, _, _) => "bevel",
        (Key::B, false, false, false) => "softSelect",
        (Key::Equals | Key::Plus, false, _, false) => "manipBigger",
        (Key::Minus, false, false, false) => "manipSmaller",
        (Key::Q, false, true, false) => "componentTypeCycle",
        (Key::B, false, false, true) => "cycleBackground",
        (Key::Insert, ..) => "pivotEdit",
        (Key::Num1, true, _, _) => "isolateSelect",
        (Key::Delete, true, _, _) => "deleteEdgeVertex",
        (Key::Delete | Key::Backspace, false, _, _) => "deleteSel",
        (Key::G, false, false, false) => "repeatLast",
        (Key::P, false, true, _) => "unparent",
        (Key::P, false, false, _) => "parent",
        (Key::Q, false, false, false) => "selectTool",
        (Key::W, false, false, false) => "moveTool",
        (Key::E, false, false, false) => "rotateTool",
        (Key::R, false, false, false) => "scaleTool",
        (Key::Y, false, false, false) => "lastToolUsed",
        (Key::W, false, true, _) => "keyTranslate",
        (Key::E, false, true, _) => "keyRotate",
        (Key::R, false, true, _) => "keyScale",
        (Key::S, false, false, false) => "setKey",
        (Key::F, false, false, false) => "frameSelected",
        (Key::A, false, false, false) => "frameAll",
        (Key::F, false, true, false) => "frameSelectedAll",
        (Key::A, false, true, false) => "frameAllAll",
        (Key::A, false, false, true) => "toggleXray",
        (Key::Num1, false, false, false) => "smoothOff",
        (Key::Num2, false, false, false) => "smoothCage",
        (Key::Num3, false, false, false) => "smoothOn",
        (Key::Num4, false, false, false) => "shadeWire",
        (Key::Num5, false, false, false) => "shadeSmooth",
        (Key::Num6, false, false, false) => "shadeTextured",
        (Key::Num7, false, false, false) => "shadeAllLights",
        (Key::F8, ..) => "objectMode",
        (Key::F9, ..) => "vertexMode",
        (Key::F10, ..) => "edgeMode",
        (Key::F11, ..) => "faceMode",
        (Key::V, false, false, true) => "playToggle",
        (Key::V, false, true, true) => "goToStart",
        (Key::Period, false, false, true) => "nextFrame",
        (Key::Comma, false, false, true) => "prevFrame",
        (Key::Period, false, true, false) => "growSelection",
        (Key::Comma, false, true, false) => "shrinkSelection",
        (Key::Period, false, false, false) => "nextKey",
        (Key::Comma, false, false, false) => "prevKey",
        (Key::Escape, ..) if app.playing => "stopPlayback",
        _ => return None,
    })
}
