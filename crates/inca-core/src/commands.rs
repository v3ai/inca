//! Maya-style commands (polyCube, move, select, setAttr, setKeyframe, ...). MEL calls these by
//! name; the GUI runs its menu actions through them too so every action echoes a command.

use crate::curves::{self, Curve};
use crate::math::*;
use crate::mel::{glob_match, Val};
use crate::params::Params;
use crate::primitives;
use crate::scene::*;
use serde_json::json;
use std::collections::BTreeSet;

/// UI-side commands (tools, viewports, windows) are delegated to the host application.
pub trait Host {
    /// Return Some to claim the command.
    fn ui(&mut self, _s: &mut Scene, _cmd: &str, _args: &[Val]) -> Option<Result<Val, String>> {
        None
    }
}
pub struct NoHost;
impl Host for NoHost {}

/// Parsed command arguments: positional values and `-flag value...` pairs.
pub struct Args {
    pub pos: Vec<Val>,
    pub flags: Vec<(String, Vec<Val>)>,
}

impl Args {
    pub fn parse(raw: &[Val], arity: &[(&str, usize)]) -> Args {
        let mut pos = vec![];
        let mut flags = vec![];
        let mut i = 0;
        while i < raw.len() {
            let is_flag = matches!(&raw[i], Val::Str(s) if s.len() > 1 && s.starts_with('-') && s[1..].starts_with(|c: char| c.is_ascii_alphabetic()));
            if is_flag {
                let f = raw[i].str()[1..].to_string();
                let n = arity.iter().find(|(k, _)| k.split('|').any(|x| x == f)).map(|x| x.1);
                let n = n.unwrap_or_else(|| {
                    // unknown flag: take one value unless the next item is another flag
                    match raw.get(i + 1) {
                        Some(Val::Str(s)) if s.starts_with('-') && s.len() > 1 && s[1..].starts_with(|c: char| c.is_ascii_alphabetic()) => 0,
                        None => 0,
                        _ => 1,
                    }
                });
                let vals: Vec<Val> = raw[i + 1..(i + 1 + n).min(raw.len())].to_vec();
                i += 1 + n;
                flags.push((f, vals));
            } else {
                match &raw[i] {
                    Val::Array(a) => pos.extend(a.iter().cloned()),
                    v => pos.push(v.clone()),
                }
                i += 1;
            }
        }
        Args { pos, flags }
    }
    pub fn has(&self, names: &[&str]) -> bool {
        self.flags.iter().any(|(f, _)| names.contains(&f.as_str()))
    }
    /// Flag value(s); a boolean flag without a value counts as on.
    pub fn get(&self, names: &[&str]) -> Option<&Vec<Val>> {
        self.flags.iter().rev().find(|(f, _)| names.contains(&f.as_str())).map(|(_, v)| v)
    }
    pub fn f(&self, names: &[&str]) -> Option<f64> {
        self.get(names).and_then(|v| v.first()).map(|v| v.num())
    }
    pub fn s(&self, names: &[&str]) -> Option<String> {
        self.get(names).and_then(|v| v.first()).map(|v| v.str())
    }
    pub fn b(&self, names: &[&str], def: bool) -> bool {
        match self.get(names) {
            None => def,
            Some(v) if v.is_empty() => true,
            Some(v) => v[0].truthy() || matches!(&v[0], Val::Str(s) if s == "true" || s == "on"),
        }
    }
    pub fn v3(&self, names: &[&str]) -> Option<[f64; 3]> {
        let v = self.get(names)?;
        if let Some(Val::Vec3(a)) = v.first() {
            return Some(*a);
        }
        if v.len() >= 3 {
            return Some([v[0].num(), v[1].num(), v[2].num()]);
        }
        None
    }
    pub fn strings(&self) -> Vec<String> {
        self.pos.iter().filter(|v| matches!(v, Val::Str(_))).map(|v| v.str()).collect()
    }
    pub fn nums(&self) -> Vec<f64> {
        self.pos.iter().filter(|v| matches!(v, Val::Int(_) | Val::Float(_))).map(|v| v.num()).collect()
    }
}

fn strs(v: Vec<String>) -> Val {
    Val::Array(v.into_iter().map(Val::Str).collect())
}

pub const CORE_COMMANDS: &[&str] = &[
    "sphere", "cylinder", "cone", "torus", "circle", "nurbsSquare", "curve", "spaceLocator", "group", "camera", "ambientLight", "directionalLight", "pointLight", "spotLight",
    "areaLight", "skyDomeLight", "shadingNode", "createNode", "move", "rotate", "scale", "xform", "makeIdentity", "CenterPivot", "centerPivot", "setAttr", "getAttr", "select", "ls",
    "objExists", "nodeType", "listRelatives", "listHistory", "rename", "parent", "delete", "duplicate", "hide", "showHidden", "currentTime", "playbackOptions", "setKeyframe",
    "cutKey", "keyframe", "hyperShade", "sets", "polyEvaluate", "polyExtrudeFacet", "polyExtrudeFace", "polyExtrudeEdge", "polySmooth", "polySubdivideFacet", "polyTriangulate",
    "polyQuad", "polyNormal", "polyPoke", "polyCloseBorder", "polyMirrorFace", "polyMergeVertex", "polyDelFacet", "polyDelEdge", "polyDelVertex", "polyUnite", "polySeparate",
    "polyCBoolOp", "polyBoolOp", "polyBridgeEdge", "polySoftEdge", "polyProjection", "polySplitRing", "polyChipOff", "polyDuplicateAndConnect", "revolve", "loft", "extrude", "planarSrf",
    "rebuildCurve", "reverseCurve", "file", "undo", "redo", "connectAttr", "disconnectAttr", "listConnections", "isConnected", "warning", "error", "about", "selectMode", "selectType",
    "polyCreateFacet", "polyBevel", "nonLinear", "emitter", "fireEffect", "smokeEffect", "gravity", "drag", "turbulence", "radial", "vortex", "air", "rigidBody", "joint", "skinCluster", "parentConstraint", "pointConstraint", "orientConstraint", "scaleConstraint", "aimConstraint", "assignMaterial", "nurbsToPoly", "displaySmoothness", "toggle", "layerEditor", "createDisplayLayer", "editDisplayLayerMembers", "help", "whatIs",
    "matchTransform", "addAttr", "deleteAttr", "polyChamferVtx", "polyConnectComponents", "polyCollapseEdge", "polyCollapseFacet", "polyToCurve", "polyCleanup", "closeCurve", "openCurve",
    "particle", "connectDynamic", "deleteRigidBody", "ikHandle", "insertJoint", "mirrorJoint", "cluster", "softMod", "lattice", "blendShape", "poleVectorConstraint", "setDrivenKeyframe", "pathAnimation", "removeConstraints", "deleteDeformers",
];

pub fn exists(name: &str) -> bool {
    CORE_COMMANDS.contains(&name) || primitives::gen_info(name).is_some()
}

/// Objects named in the args, or the selection.
fn targets(s: &Scene, a: &Args) -> Vec<Id> {
    let named: Vec<Id> = a.strings().iter().filter_map(|n| s.find(n.split('.').next().unwrap_or(n))).collect();
    if named.is_empty() {
        s.sel.clone()
    } else {
        let mut out = vec![];
        for n in named {
            if !out.contains(&n) {
                out.push(n);
            }
        }
        out
    }
}

/// Parse a component string: `pCube1.f[0:3]`, `pCube1.vtx[2]`, `pCube1.e[4]`.
pub fn parse_component(s: &Scene, txt: &str) -> Option<(Id, CompMode, Vec<usize>)> {
    let (obj, rest) = txt.split_once('.')?;
    let (kind, range) = rest.split_once('[')?;
    let range = range.trim_end_matches(']');
    let mode = match kind {
        "vtx" | "v" => CompMode::Vertex,
        "e" => CompMode::Edge,
        "f" => CompMode::Face,
        _ => return None,
    };
    let id = s.find(obj)?;
    let n = s.node(&id)?.mesh()?;
    let count = match mode {
        CompMode::Vertex => n.mesh.v.len(),
        CompMode::Edge => n.mesh.topo().edges.len(),
        CompMode::Face => n.mesh.f.len(),
    };
    let idx: Vec<usize> = if range == "*" {
        (0..count).collect()
    } else if let Some((a, b)) = range.split_once(':') {
        (a.parse().ok()?..=b.parse::<usize>().ok()?.min(count.saturating_sub(1))).collect()
    } else {
        vec![range.parse().ok()?]
    };
    Some((id, mode, idx.into_iter().filter(|&i| i < count).collect()))
}

/// Component selection as strings (for `ls -sl`).
pub fn component_names(s: &Scene, id: &str) -> Vec<String> {
    let Some(n) = s.node(id) else { return vec![] };
    let Some(m) = n.mesh() else { return vec![] };
    let c = s.comp_sel(id);
    let mut out = vec![];
    let ranges = |v: Vec<usize>| -> Vec<String> {
        let mut r = vec![];
        let mut i = 0;
        while i < v.len() {
            let st = v[i];
            while i + 1 < v.len() && v[i + 1] == v[i] + 1 {
                i += 1;
            }
            r.push(if v[i] == st { st.to_string() } else { format!("{st}:{}", v[i]) });
            i += 1;
        }
        r
    };
    for r in ranges(c.v.iter().map(|&x| x as usize).collect()) {
        out.push(format!("{}.vtx[{r}]", n.name));
    }
    if !c.e.is_empty() {
        let topo = m.mesh.topo();
        let mut es: Vec<usize> = c.e.iter().filter_map(|&(a, b)| topo.edge(a, b)).collect();
        es.sort();
        for r in ranges(es) {
            out.push(format!("{}.e[{r}]", n.name));
        }
    }
    for r in ranges(c.f.iter().copied().collect()) {
        out.push(format!("{}.f[{r}]", n.name));
    }
    out
}

/// Faces to operate on for an object: component args, the face selection, or all faces.
fn op_faces(s: &Scene, a: &Args, id: &str, all_if_none: bool) -> Vec<usize> {
    let mut v: Vec<usize> = vec![];
    for t in a.strings() {
        if let Some((cid, CompMode::Face, idx)) = parse_component(s, &t) {
            if cid == id {
                v.extend(idx);
            }
        }
    }
    if v.is_empty() {
        v = s.comp_sel(id).f.iter().copied().collect();
    }
    if v.is_empty() && all_if_none {
        v = (0..s.node(id).and_then(|n| n.mesh()).map(|m| m.mesh.f.len()).unwrap_or(0)).collect();
    }
    v
}
fn op_edges(s: &Scene, a: &Args, id: &str) -> Vec<(u32, u32)> {
    let mut v = vec![];
    let topo = s.node(id).and_then(|n| n.mesh()).map(|m| m.mesh.topo());
    for t in a.strings() {
        if let (Some((cid, CompMode::Edge, idx)), Some(topo)) = (parse_component(s, &t), &topo) {
            if cid == id {
                v.extend(idx.iter().filter_map(|&i| topo.edges.get(i).copied()));
            }
        }
    }
    if v.is_empty() {
        v = s.comp_sel(id).e.iter().copied().collect();
    }
    v
}
fn op_verts(s: &Scene, a: &Args, id: &str) -> Vec<u32> {
    let mut v = vec![];
    for t in a.strings() {
        if let Some((cid, CompMode::Vertex, idx)) = parse_component(s, &t) {
            if cid == id {
                v.extend(idx.iter().map(|&i| i as u32));
            }
        }
    }
    if v.is_empty() {
        v = s.comp_verts(id);
    }
    v
}

/// Meshes an op applies to: objects owning named components, else selected meshes.
fn mesh_targets(s: &Scene, a: &Args) -> Vec<Id> {
    let mut out: Vec<Id> = vec![];
    for t in a.strings() {
        if let Some((id, _, _)) = parse_component(s, &t) {
            if !out.contains(&id) {
                out.push(id);
            }
        } else if let Some(id) = s.find(&t) {
            if s.node(&id).is_some_and(|n| n.mesh().is_some()) && !out.contains(&id) {
                out.push(id);
            }
        }
    }
    if out.is_empty() {
        out = s.sel.iter().filter(|i| s.node(i).is_some_and(|n| n.mesh().is_some())).cloned().collect();
    }
    out
}

fn history_on(a: &Args) -> bool {
    a.b(&["ch", "constructionHistory"], true)
}

fn result_names(s: &Scene, id: &Id) -> Val {
    let n = s.node(id).unwrap();
    let mut v = vec![Val::Str(n.name.clone())];
    if let Some(h) = n.mesh().and_then(|m| m.history.last()) {
        if !h.name.contains('#') {
            v.push(Val::Str(h.name.clone()));
        }
    }
    Val::Array(v)
}

fn place_new(s: &mut Scene, id: &Id, a: &Args) {
    if let Some(n) = a.s(&["n", "name"]) {
        rename(s, id, &n);
    }
    if let Some(ax) = a.v3(&["ax", "axis"]) {
        let d = DVec3::from_array(ax).normalize_or(DVec3::Y);
        if (d - DVec3::Y).length() > 1e-9 {
            let q = DQuat::from_rotation_arc(DVec3::Y, d);
            let e = euler_deg_from_quat(q, "xyz");
            if let Some(n) = s.nodes.get_mut(id) {
                n.xf.r = e.map(|x| round_to(x, 6));
            }
        }
    }
    s.select(std::slice::from_ref(id), SelMode::Replace);
}

pub fn rename(s: &mut Scene, id: &str, want: &str) -> String {
    let Some(old) = s.node(id).map(|n| n.name.clone()) else { return String::new() };
    if old == want {
        return old;
    }
    s.release_name(&old);
    let nn = s.unique_name(want);
    let shape_old = s.node(id).and_then(|n| n.shape_name.clone());
    if let Some(so) = shape_old {
        s.release_name(&so);
        let sn = s.unique_name(&shape_name_for(&nn));
        s.nodes.get_mut(id).unwrap().shape_name = Some(sn);
    }
    s.nodes.get_mut(id).unwrap().name = nn.clone();
    s.touch();
    nn
}

fn shape_name_for(n: &str) -> String {
    let digits = n.len() - n.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if digits > 0 {
        format!("{}Shape{}", &n[..n.len() - digits], &n[n.len() - digits..])
    } else {
        format!("{n}Shape")
    }
}

/// Create a primitive with MEL flags (`-w 2 -sx 4 -n name`).
fn create_primitive(s: &mut Scene, ty: &str, a: &Args) -> Val {
    let g = primitives::gen_info(ty).unwrap();
    let mut p = Params::new();
    for (f, v) in &a.flags {
        let key = g.short.iter().find(|(k, _)| k == f).map(|x| x.1).or_else(|| g.params.iter().find(|(k, _)| k == f).map(|x| x.0));
        if let (Some(k), Some(v)) = (key, v.first()) {
            p.insert(k.into(), json!(v.num()));
        }
    }
    let id = s.create_mesh(ty, &p, None);
    if !history_on(a) {
        s.delete_history(&id);
    }
    place_new(s, &id, a);
    result_names(s, &id)
}

/// Move / rotate / scale objects or components.
fn transform_cmd(s: &mut Scene, name: &str, a: &Args) -> Result<Val, String> {
    let vals = a.nums();
    let relative = a.has(&["r", "relative", "by"]);
    let ws = !a.has(&["os", "objectSpace", "ls", "localSpace"]);
    let mut v = [0.0, 0.0, 0.0];
    let mut mask = [true; 3];
    if vals.len() >= 3 {
        v = [vals[0], vals[1], vals[2]];
    } else if vals.len() == 1 {
        let axis = if a.has(&["x"]) {
            0
        } else if a.has(&["y"]) {
            1
        } else if a.has(&["z"]) {
            2
        } else {
            v = [vals[0]; 3];
            3
        };
        if axis < 3 {
            mask = [false; 3];
            mask[axis] = true;
            v[axis] = vals[0];
        }
    } else {
        return Err(format!("{name}: expected 3 values"));
    }
    // components
    let comps: Vec<(Id, CompMode, Vec<usize>)> = a.strings().iter().filter_map(|t| parse_component(s, t)).collect();
    let comp_targets: Vec<(Id, Vec<u32>)> = if !comps.is_empty() {
        comps
            .iter()
            .map(|(id, m, idx)| {
                let pm = &s.node(id).unwrap().mesh().unwrap().mesh;
                let verts: BTreeSet<u32> = match m {
                    CompMode::Vertex => idx.iter().map(|&i| i as u32).collect(),
                    CompMode::Face => idx.iter().flat_map(|&f| pm.f[f].iter().copied()).collect(),
                    CompMode::Edge => {
                        let t = pm.topo();
                        idx.iter().flat_map(|&e| [t.edges[e].0, t.edges[e].1]).collect()
                    }
                };
                (id.clone(), verts.into_iter().collect())
            })
            .collect()
    } else if s.comp_mode.is_some() && a.strings().is_empty() {
        s.hilite().iter().map(|id| (id.clone(), s.comp_verts(id))).filter(|(_, v)| !v.is_empty()).collect()
    } else {
        vec![]
    };
    if !comp_targets.is_empty() {
        for (id, verts) in comp_targets {
            let wm = s.world_matrix(&id);
            let inv = wm.inverse();
            let pm = s.node(&id).unwrap().mesh().unwrap().mesh.clone();
            let pts: Vec<DVec3> = verts.iter().map(|&i| wm.transform_point3(pm.v[i as usize])).collect();
            let c = pts.iter().copied().sum::<DVec3>() / pts.len().max(1) as f64;
            let deltas: Vec<DVec3> = pts
                .iter()
                .map(|&p| {
                    let np = match name {
                        "move" => {
                            if relative {
                                p + DVec3::from_array(v)
                            } else {
                                DVec3::new(if mask[0] { v[0] } else { p.x }, if mask[1] { v[1] } else { p.y }, if mask[2] { v[2] } else { p.z })
                            }
                        }
                        "rotate" => c + quat_from_euler_deg(v, "xyz") * (p - c),
                        _ => c + (p - c) * DVec3::from_array(v),
                    };
                    inv.transform_point3(np) - inv.transform_point3(p)
                })
                .collect();
            let mut p = Params::new();
            p.insert("verts".into(), json!(verts));
            p.insert("deltas".into(), json!(deltas.iter().map(|d| [d.x, d.y, d.z]).collect::<Vec<_>>()));
            s.apply_op(&id, "polyTweak", p, true);
        }
        return Ok(Val::None);
    }
    for id in targets(s, a) {
        let Some(n) = s.node(&id) else { continue };
        let cur = match name {
            "move" => n.xf.t,
            "rotate" => n.xf.r,
            _ => n.xf.s,
        };
        let mut nv = cur;
        if name == "move" && ws && relative {
            // world-space relative move: convert the delta into parent space
            let pinv = s.parent_world(&id).inverse();
            let d = pinv.transform_vector3(DVec3::from_array(v));
            for i in 0..3 {
                if mask[i] {
                    nv[i] = cur[i] + d[i];
                }
            }
        } else {
            for i in 0..3 {
                if !mask[i] {
                    continue;
                }
                nv[i] = if relative {
                    if name == "scale" {
                        cur[i] * v[i]
                    } else {
                        cur[i] + v[i]
                    }
                } else {
                    v[i]
                };
            }
        }
        let attr = match name {
            "move" => "translate",
            "rotate" => "rotate",
            _ => "scale",
        };
        for (i, ax) in ["X", "Y", "Z"].iter().enumerate() {
            if mask[i] {
                s.set_attr(&NodeRef::Dag(id.clone()), &format!("{attr}{ax}"), json!(round_to(nv[i], 6)));
            }
        }
    }
    Ok(Val::None)
}

/// Run a core command (after giving the host a chance to claim it).
pub fn run(s: &mut Scene, h: &mut dyn Host, name: &str, raw: &[Val]) -> Result<Val, String> {
    if let Some(r) = h.ui(s, name, raw) {
        return r;
    }
    let arity: &[(&str, usize)] = match name {
        "move" | "rotate" | "scale" => &[("r|relative|by", 0), ("a|absolute", 0), ("ws|worldSpace", 0), ("os|objectSpace", 0), ("ls|localSpace", 0), ("x", 0), ("y", 0), ("z", 0), ("p|pivot", 3), ("cs", 0)],
        "xform" => &[("q|query", 0), ("ws|worldSpace", 0), ("os|objectSpace", 0), ("r|relative", 0), ("a|absolute", 0), ("t|translation", 3), ("ro|rotation", 3), ("s|scale", 3), ("piv|pivots", 3), ("rp|rotatePivot", 3), ("sp|scalePivot", 3), ("cp|centerPivots", 0), ("bb|boundingBox", 0), ("m|matrix", 16)],
        "select" => &[("r|replace", 0), ("add", 0), ("af|addFirst", 0), ("tgl|toggle", 0), ("d|deselect", 0), ("cl|clear", 0), ("all", 0), ("hi|hierarchy", 0), ("ne|noExpand", 0)],
        "ls" => &[("sl|selection", 0), ("dag", 0), ("tr|transforms", 0), ("s|shapes", 0), ("l|long", 0), ("fl|flatten", 0), ("mat|materials", 0), ("lt|lights", 0), ("ca|cameras", 0), ("v|visible", 0), ("type", 1), ("typ", 1), ("geometry", 0), ("hl|hilite", 0)],
        "listRelatives" => &[("c|children", 0), ("p|parent", 0), ("ad|allDescendents", 0), ("s|shapes", 0), ("f|fullPath", 0)],
        "parent" => &[("w|world", 0), ("r|relative", 0), ("a|absolute", 0), ("add", 0), ("s|shape", 0)],
        "delete" => &[("ch|constructionHistory", 0), ("all", 0), ("c|channels", 0)],
        "duplicate" => &[("rr|returnRootsOnly", 0), ("un|upstreamNodes", 0), ("ic|inputConnections", 0), ("n|name", 1), ("st|smartTransform", 0)],
        "currentTime" => &[("q|query", 0), ("e|edit", 0), ("u|update", 1)],
        "playbackOptions" => &[("q|query", 0), ("e|edit", 0), ("min|minTime", 1), ("max|maxTime", 1), ("ast|animationStartTime", 1), ("aet|animationEndTime", 1), ("l|loop", 1), ("fps", 1), ("by", 1)],
        "setKeyframe" => &[("at|attribute", 1), ("t|time", 1), ("v|value", 1), ("itt|inTangentType", 1), ("ott|outTangentType", 1), ("bd|breakdown", 0)],
        "keyframe" | "cutKey" => &[("q|query", 0), ("e|edit", 0), ("at|attribute", 1), ("t|time", 1), ("tc|timeChange", 1), ("vc|valueChange", 1), ("kc|keyframeCount", 0), ("r|relative", 0), ("cl|clear", 0), ("sl|selectedKey", 0), ("o|option", 1)],
        "polyEvaluate" => &[("v|vertex", 0), ("e|edge", 0), ("f|face", 0), ("t|triangle", 0), ("b|boundingBox", 0), ("a|area", 0)],
        "file" => &[("new|newFile", 0), ("f|force", 0), ("o|open", 0), ("s|save", 0), ("i|import", 0), ("ea|exportAll", 0), ("es|exportSelected", 0), ("rn|rename", 1), ("q|query", 0), ("sn|sceneName", 0), ("type", 1), ("typ", 1), ("pr|preserveReferences", 0), ("op|options", 1)],
        "hide" | "showHidden" => &[("all", 0), ("a|above", 0), ("b|below", 0)],
        "matchTransform" => &[("pos|position", 0), ("rot|rotation", 0), ("scl|scale", 0)],
        "polyChamferVtx" => &[("w|width", 1), ("ch|constructionHistory", 1)],
        "polyConnectComponents" | "polyCollapseEdge" | "polyCollapseFacet" | "polyCleanup" => &[("ch|constructionHistory", 1)],
        "polyToCurve" => &[("form", 1), ("degree", 1), ("n|name", 1)],
        "closeCurve" | "openCurve" => &[("ps|preserveShape", 1), ("ch|constructionHistory", 1), ("rpo|replaceOriginal", 1)],
        "addAttr" => &[("ln|longName", 1), ("sn|shortName", 1), ("at|attributeType", 1), ("dt|dataType", 1), ("dv|defaultValue", 1), ("min|minValue", 1), ("max|maxValue", 1), ("k|keyable", 1)],
        "deleteAttr" => &[("at|attribute", 1)],
        "group" => &[("em|empty", 0), ("w|world", 0), ("n|name", 1), ("p|parent", 1), ("a|absolute", 0), ("r|relative", 0)],
        "shadingNode" => &[("asShader", 0), ("asTexture", 0), ("asUtility", 0), ("n|name", 1)],
        "hyperShade" => &[("assign", 1), ("objects", 1)],
        "sets" => &[("e|edit", 0), ("fe|forceElement", 1), ("n|name", 1), ("r|renderable", 1), ("nss|noSurfaceShader", 1), ("em|empty", 0)],
        "makeIdentity" => &[("a|apply", 1), ("t|translate", 1), ("r|rotate", 1), ("s|scale", 1), ("n|normal", 1), ("pn|preserveNormals", 1)],
        "connectAttr" | "disconnectAttr" | "isConnected" => &[("f|force", 0), ("l|lock", 1), ("na|nextAvailable", 0)],
        "listConnections" => &[("s|source", 1), ("d|destination", 1), ("p|plugs", 1), ("c|connections", 1)],
        "spaceLocator" => &[("p|position", 3), ("n|name", 1), ("a|absolute", 0)],
        "camera" => &[("n|name", 1), ("fl|focalLength", 1), ("o|orthographic", 1), ("ow|orthographicWidth", 1)],
        "curve" => &[("d|degree", 1), ("p|point", 3), ("ep|editPoint", 3), ("per|periodic", 1), ("k|knot", 1), ("n|name", 1), ("bez|bezier", 0), ("arc", 0)],
        "polyCreateFacet" => &[("p|point", 3), ("n|name", 1), ("ch", 1)],
        "circle" => &[("r|radius", 1), ("s|sections", 1), ("nr|normal", 3), ("c|center", 3), ("n|name", 1), ("ch|constructionHistory", 1), ("d|degree", 1)],
        "selectMode" => &[("o|object", 0), ("co|component", 0), ("q|query", 0)],
        "selectType" => &[("v|vertex", 1), ("pv|polymeshVertex", 1), ("e|edge", 1), ("pe|polymeshEdge", 1), ("f|facet", 1), ("pf|polymeshFace", 1)],
        "rename" => &[("is|ignoreShape", 0)],
        "polyExtrudeFacet" | "polyExtrudeFace" => &[("ltz|localTranslateZ", 1), ("ltx", 1), ("lty", 1), ("t|translate", 3), ("tx", 1), ("ty", 1), ("tz", 1), ("off|offset", 1), ("kft|keepFacesTogether", 1), ("d|divisions", 1), ("ch|constructionHistory", 1), ("thk|thickness", 1)],
        "polyProjection" => &[("t|type", 1), ("md|mapDirection", 1), ("ch", 1)],
        "emitter" | "fireEffect" | "smokeEffect" => &[("pos|position", 3), ("type", 1), ("r|rate", 1), ("spd|speed", 1), ("sp|spread", 1), ("lsp|lifespan", 1), ("dx", 1), ("dy", 1), ("dz", 1), ("n|name", 1)],
        "gravity" | "drag" | "turbulence" | "radial" | "vortex" | "air" => &[("pos|position", 3), ("m|magnitude", 1), ("dx", 1), ("dy", 1), ("dz", 1), ("mxd|maxDistance", 1), ("att|attenuation", 1), ("f|frequency", 1)],
        "particle" => &[("ll|lowerLeft", 3), ("ur|upperRight", 3), ("grs|gridSpacing", 1), ("c|conserve", 1), ("p|position", 3), ("n|name", 1)],
        "connectDynamic" => &[("f|fields", 1), ("d|delete", 0)],
        "rigidBody" => &[("active", 0), ("passive", 0), ("pas", 0), ("m|mass", 1), ("b|bounciness", 1), ("sf|staticFriction", 1)],
        "joint" => &[("p|position", 3), ("n|name", 1), ("rad|radius", 1), ("a|absolute", 0), ("e|edit", 0), ("oj|orientJoint", 1), ("sao|secondaryAxisOrient", 1), ("ch|children", 0), ("zso|zeroScaleOrient", 0)],
        "ikHandle" => &[("sj|startJoint", 1), ("ee|endEffector", 1), ("sol|solver", 1), ("n|name", 1)],
        "mirrorJoint" => &[("myz|mirrorYZ", 0), ("mxz|mirrorXZ", 0), ("mxy|mirrorXY", 0), ("mb|mirrorBehavior", 0), ("sr|searchReplace", 2)],
        "cluster" | "softMod" => &[("n|name", 1), ("fr|falloffRadius", 1), ("rel|relative", 0)],
        "lattice" => &[("dv|divisions", 3), ("oc|objectCentered", 1), ("n|name", 1)],
        "blendShape" => &[("n|name", 1)],
        "poleVectorConstraint" => &[("w|weight", 1)],
        "setDrivenKeyframe" => &[("cd|currentDriver", 1), ("at|attribute", 1), ("dv|driverValue", 1), ("v|value", 1)],
        "pathAnimation" => &[("c|curve", 1), ("f|follow", 1), ("fa|followAxis", 1), ("ua|upAxis", 1), ("fm|fractionMode", 1), ("stu|startTimeU", 1), ("etu|endTimeU", 1), ("wut|worldUpType", 1)],
        "parentConstraint" | "pointConstraint" | "orientConstraint" | "scaleConstraint" | "aimConstraint" => &[("mo|maintainOffset", 0), ("w|weight", 1), ("aim|aimVector", 3), ("u|upVector", 3)],
        "skinCluster" => &[("tsb|toSelectedBones", 0), ("mi|maximumInfluences", 1), ("dr|dropoffRate", 1), ("e|edit", 0), ("ub|unbind", 0), ("bm|bindMethod", 1), ("sm|skinMethod", 1), ("nw|normalizeWeights", 1), ("wd|weightDistribution", 1)],
        "nurbsToPoly" => &[("f|format", 1), ("pt|polygonType", 1), ("ch", 1)],
        _ => &[("ch|constructionHistory", 1), ("n|name", 1)],
    };
    let a = Args::parse(raw, arity);
    if primitives::gen_info(name).is_some() {
        return Ok(create_primitive(s, name, &a));
    }
    let nurbs_alias = match name {
        "sphere" => Some("nurbsSphere"),
        "cylinder" => Some("nurbsCylinder"),
        "cone" => Some("nurbsCone"),
        "torus" => Some("nurbsTorus"),
        _ => None,
    };
    if let Some(ty) = nurbs_alias {
        return Ok(create_primitive(s, ty, &a));
    }
    match name {
        "circle" => {
            let nr = a.v3(&["nr", "normal"]).map(DVec3::from_array).unwrap_or(DVec3::Y);
            let c = curves::nurbs_circle(a.f(&["r", "radius"]).unwrap_or(1.0), a.f(&["s", "sections"]).unwrap_or(8.0) as usize, nr);
            let id = s.create_curve(c, "nurbsCircle1");
            if let Some(cn) = a.v3(&["c", "center"]) {
                s.nodes.get_mut(&id).unwrap().xf.t = cn;
            }
            place_new(s, &id, &a);
            Ok(result_names(s, &id))
        }
        "nurbsSquare" => {
            let c = curves::nurbs_square(a.f(&["sl1", "sideLength1"]).unwrap_or(1.0), DVec3::Y);
            let id = s.create_curve(c, "nurbsSquare1");
            place_new(s, &id, &a);
            Ok(result_names(s, &id))
        }
        "curve" => {
            let pts: Vec<DVec3> = a
                .flags
                .iter()
                .filter(|(f, _)| f == "p" || f == "point" || f == "ep" || f == "editPoint")
                .filter_map(|(_, v)| (v.len() >= 3).then(|| DVec3::new(v[0].num(), v[1].num(), v[2].num())))
                .collect();
            if pts.len() < 2 {
                return Err("curve: needs at least two -p points".into());
            }
            let arc = a.has(&["arc"]);
            if arc && pts.len() != 3 {
                return Err("curve -arc: needs start, through and end points".into());
            }
            let kind = if arc {
                "arc"
            } else if a.has(&["ep", "editPoint"]) {
                "ep"
            } else {
                "cv"
            };
            let per = a.b(&["per", "periodic"], false);
            let c = Curve::new(pts, if arc { 2 } else { a.f(&["d", "degree"]).unwrap_or(3.0) as usize }, if per { "periodic" } else { "open" }, kind);
            let id = s.create_curve(c, "curve1");
            place_new(s, &id, &a);
            Ok(Val::Str(s.name_of(&id)))
        }
        "polyCreateFacet" => {
            let pts: Vec<DVec3> = a.flags.iter().filter(|(f, _)| f == "p" || f == "point").filter_map(|(_, v)| (v.len() >= 3).then(|| DVec3::new(v[0].num(), v[1].num(), v[2].num()))).collect();
            if pts.len() < 3 {
                return Err("polyCreateFacet: needs at least three -p points".into());
            }
            let c = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
            let local: Vec<DVec3> = pts.iter().map(|p| *p - c).collect();
            let pm = crate::mesh::op_project_uv(&crate::mesh::polygon_from_points(&local), "automatic", 1, None);
            let id = s.create_mesh_from(pm, "polySurface1", None, None);
            s.nodes.get_mut(&id).unwrap().xf.t = c.to_array();
            place_new(s, &id, &a);
            Ok(Val::Array(vec![Val::Str(s.name_of(&id))]))
        }
        "spaceLocator" => {
            let id = s.create_locator("locator1");
            if let Some(p) = a.v3(&["p", "position"]) {
                s.nodes.get_mut(&id).unwrap().xf.t = p;
            }
            place_new(s, &id, &a);
            Ok(Val::Array(vec![Val::Str(s.name_of(&id))]))
        }
        "group" => {
            let objs = if a.has(&["em", "empty"]) { vec![] } else { targets(s, &a) };
            let nm = a.s(&["n", "name"]).unwrap_or("group1".into());
            let g = if objs.is_empty() { s.create_group(&nm, None) } else { s.group(&objs, &nm) };
            if !objs.is_empty() {
                s.center_pivot(&g);
                let _ = s.world_pivot(&g);
                // Maya groups pivot at the origin
                s.set_pivot_world(&g, DVec3::ZERO);
            }
            s.select(std::slice::from_ref(&g), SelMode::Replace);
            Ok(Val::Str(s.name_of(&g)))
        }
        "camera" => {
            let cam = CamShape { focal_length: a.f(&["fl", "focalLength"]).unwrap_or(35.0), ortho: a.b(&["o", "orthographic"], false), ortho_width: a.f(&["ow"]).unwrap_or(30.0), ..Default::default() };
            let id = s.create_camera("camera1", cam, false);
            place_new(s, &id, &a);
            let n = s.node(&id).unwrap();
            Ok(Val::Array(vec![Val::Str(n.name.clone()), Val::Str(n.shape_name.clone().unwrap_or_default())]))
        }
        "ambientLight" | "directionalLight" | "pointLight" | "spotLight" | "areaLight" | "skyDomeLight" => {
            let id = s.create_light(name);
            if let Some(i) = a.f(&["i", "intensity"]) {
                s.set_attr(&NodeRef::Dag(id.clone()), "intensity", json!(i));
            }
            if let Some(c) = a.v3(&["rgb"]) {
                s.set_attr(&NodeRef::Dag(id.clone()), "color", json!(c));
            }
            place_new(s, &id, &a);
            Ok(Val::Str(s.node(&id).unwrap().shape_name.clone().unwrap_or_default()))
        }
        "shadingNode" | "createNode" => {
            let ty = a.strings().first().cloned().ok_or("shadingNode: missing type")?;
            let nm = a.s(&["n", "name"]);
            if crate::materials::MAT_TYPES.iter().any(|m| m.ty == ty) {
                let id = s.create_material(&ty, nm.as_deref());
                return Ok(Val::Str(s.mats[&id].name.clone()));
            }
            if crate::materials::tex_type(&ty).is_some() {
                let id = s.create_texture(&ty, nm.as_deref());
                return Ok(Val::Str(s.texs[&id].name.clone()));
            }
            match ty.as_str() {
                "transform" => {
                    let id = s.create_group(nm.as_deref().unwrap_or("transform1"), None);
                    Ok(Val::Str(s.name_of(&id)))
                }
                "locator" => {
                    let id = s.create_locator(nm.as_deref().unwrap_or("locator1"));
                    Ok(Val::Str(s.name_of(&id)))
                }
                t => Err(format!("Unknown node type: {t}")),
            }
        }
        "hyperShade" | "assignMaterial" => {
            let m = a.s(&["assign"]).or_else(|| a.strings().first().cloned()).ok_or("hyperShade: -assign material")?;
            let m = m.trim_end_matches("SG").to_string();
            let mid = s.mats.values().find(|x| x.name == m).map(|x| x.id.clone()).ok_or(format!("No material named {m}"))?;
            assign_to_selection(s, &mid);
            Ok(Val::None)
        }
        "sets" => {
            if let Some(m) = a.s(&["fe", "forceElement"]) {
                let m = m.trim_end_matches("SG").to_string();
                let mid = s.mats.values().find(|x| x.name == m).map(|x| x.id.clone()).ok_or(format!("No material named {m}"))?;
                let objs = a.strings();
                if !objs.is_empty() {
                    let save = s.sel.clone();
                    let ids: Vec<Id> = objs.iter().filter_map(|o| s.find(o.split('.').next().unwrap())).collect();
                    s.select(&ids, SelMode::Replace);
                    assign_to_selection(s, &mid);
                    s.sel = save;
                } else {
                    assign_to_selection(s, &mid);
                }
                return Ok(Val::None);
            }
            Ok(Val::Str(a.s(&["n", "name"]).unwrap_or("set1".into())))
        }
        "move" | "rotate" | "scale" => transform_cmd(s, name, &a),
        "xform" => xform(s, &a),
        "makeIdentity" => {
            for id in targets(s, &a) {
                if a.b(&["a", "apply"], false) {
                    s.freeze(&id);
                } else {
                    s.reset_transforms(&id);
                }
            }
            Ok(Val::None)
        }
        "CenterPivot" | "centerPivot" => {
            for id in targets(s, &a) {
                s.center_pivot(&id);
            }
            Ok(Val::None)
        }
        "setAttr" => {
            let target = a.strings().first().cloned().ok_or("setAttr: no attribute")?;
            let (obj, attr) = target.split_once('.').ok_or("setAttr: expected node.attribute")?;
            let vals: Vec<&Val> = a.pos.iter().skip(1).collect();
            if matches!(obj, "defaultResolution" | "defaultRenderGlobals") {
                // render settings (Render Settings window)
                let v = match vals.first() {
                    Some(Val::Str(st)) if st.parse::<f64>().is_err() => json!(st),
                    Some(v) => v.to_json(),
                    None => return Err("setAttr: no value".into()),
                };
                s.render.insert(attr.to_string(), v);
                return Ok(Val::None);
            }
            let r = s.resolve(obj).ok_or(format!("No object matches name: {obj}"))?;
            let v = if a.has(&["type"]) && a.s(&["type"]).as_deref() == Some("string") {
                json!(vals.first().map(|v| v.str()).unwrap_or_default())
            } else if vals.len() >= 3 {
                json!([vals[0].num(), vals[1].num(), vals[2].num()])
            } else {
                match vals.first() {
                    Some(Val::Str(st)) if st.parse::<f64>().is_err() => json!(st),
                    Some(v) => v.to_json(),
                    None => return Err("setAttr: no value".into()),
                }
            };
            if !s.set_attr(&r, attr, v) {
                return Err(format!("setAttr: '{target}' is locked or does not exist"));
            }
            Ok(Val::None)
        }
        "getAttr" => {
            let target = a.strings().first().cloned().ok_or("getAttr: no attribute")?;
            let (obj, attr) = target.split_once('.').ok_or("getAttr: expected node.attribute")?;
            if matches!(obj, "defaultResolution" | "defaultRenderGlobals") {
                return s.render.get(attr).map(Val::from_json).ok_or(format!("No attribute '{attr}' on {obj}"));
            }
            let r = s.resolve(obj).ok_or(format!("No object matches name: {obj}"))?;
            let v = s.get_attr(&r, attr).ok_or(format!("No attribute '{attr}' on {obj}"))?;
            Ok(Val::from_json(&v))
        }
        "select" => select(s, &a),
        "ls" => ls(s, &a),
        "objExists" => Ok(Val::Int(a.strings().first().is_some_and(|n| s.resolve(n).is_some()) as i64)),
        "nodeType" => {
            let n = a.strings().first().cloned().unwrap_or_default();
            Ok(Val::Str(match s.resolve(&n) {
                Some(NodeRef::Dag(id)) => {
                    let node = s.node(&id).unwrap();
                    if node.shape_name.as_deref() == Some(n.as_str()) {
                        match &node.shape {
                            Shape::Light(l) => l.ty.clone(),
                            Shape::Curve(_) => "nurbsCurve".into(),
                            _ => node.kind.clone(),
                        }
                    } else if node.kind == "joint" {
                        "joint".into()
                    } else {
                        "transform".into()
                    }
                }
                Some(NodeRef::History(id, i)) => s.node(&id).unwrap().mesh().unwrap().history[i].ty.clone(),
                Some(NodeRef::Material(id)) => s.mats[&id].ty.clone(),
                Some(NodeRef::Texture(id)) => s.texs[&id].ty.clone(),
                None => return Err(format!("No object matches name: {n}")),
            }))
        }
        "listRelatives" => {
            let mut out = vec![];
            for id in targets(s, &a) {
                if a.has(&["p", "parent"]) {
                    if let Some(p) = s.node(&id).and_then(|n| n.parent.clone()) {
                        out.push(s.name_of(&p));
                    }
                } else if a.has(&["s", "shapes"]) {
                    if let Some(sn) = s.node(&id).and_then(|n| n.shape_name.clone()) {
                        out.push(sn);
                    }
                } else if a.has(&["ad", "allDescendents"]) {
                    out.extend(s.subtree(&id).iter().skip(1).map(|i| s.name_of(i)));
                } else {
                    out.extend(s.children(Some(&id)).iter().map(|i| s.name_of(i)));
                }
            }
            Ok(strs(out))
        }
        "listHistory" => {
            let mut out = vec![];
            for id in targets(s, &a) {
                if let Some(m) = s.node(&id).and_then(|n| n.mesh()) {
                    out.extend(m.history.iter().rev().filter(|h| !h.name.contains('#')).map(|h| h.name.clone()));
                }
            }
            Ok(strs(out))
        }
        "rename" => {
            let st = a.strings();
            let (obj, new) = match st.len() {
                0 => return Err("rename: new name required".into()),
                1 => (s.lead().cloned().ok_or("rename: nothing selected")?, st[0].clone()),
                _ => (s.find(&st[0]).ok_or(format!("No object matches name: {}", st[0]))?, st[1].clone()),
            };
            Ok(Val::Str(rename(s, &obj, &new)))
        }
        "parent" => {
            let mut objs = targets(s, &a);
            if a.has(&["w", "world"]) {
                for o in &objs {
                    s.reparent(o, None, None);
                }
                return Ok(strs(objs.iter().map(|o| s.name_of(o)).collect()));
            }
            if objs.len() < 2 {
                return Err("parent: select children then the parent".into());
            }
            let p = objs.pop().unwrap();
            for o in &objs {
                if !s.reparent(o, Some(&p), None) {
                    s.warn(format!("Cannot parent {} under {}", s.name_of(o), s.name_of(&p)));
                }
            }
            Ok(strs(objs.iter().map(|o| s.name_of(o)).collect()))
        }
        "delete" => {
            if a.has(&["ch", "constructionHistory"]) {
                let ts = if a.has(&["all"]) { s.all_dag(false) } else { targets(s, &a) };
                for id in ts {
                    s.delete_history(&id);
                }
                return Ok(Val::None);
            }
            if a.has(&["c", "channels"]) {
                for id in targets(s, &a) {
                    s.anim.remove_node(&id);
                }
                return Ok(Val::None);
            }
            // component deletion
            let comps: Vec<(Id, CompMode, Vec<usize>)> = a.strings().iter().filter_map(|t| parse_component(s, t)).collect();
            if !comps.is_empty() || (s.comp_mode.is_some() && a.strings().is_empty()) {
                delete_components(s, &a);
                return Ok(Val::None);
            }
            let ts = if a.has(&["all"]) { s.all_dag(false).into_iter().filter(|i| s.node(i).is_some_and(|n| n.parent.is_none())).collect() } else { targets(s, &a) };
            for id in ts {
                if s.node(&id).is_some_and(|n| !n.startup()) {
                    s.delete_node(&id);
                }
            }
            Ok(Val::None)
        }
        "duplicate" => {
            let ts = targets(s, &a);
            let ups = a.has(&["un", "upstreamNodes", "ic", "inputConnections"]);
            let d = s.duplicate(&ts, ups);
            if let (Some(n), Some(first)) = (a.s(&["n", "name"]), d.first()) {
                rename(s, first, &n);
            }
            s.select(&d, SelMode::Replace);
            Ok(strs(d.iter().map(|i| s.name_of(i)).collect()))
        }
        "hide" | "showHidden" => {
            let vis = name == "showHidden";
            let ts = if a.has(&["all"]) { s.all_dag(false) } else { targets(s, &a) };
            for id in ts {
                s.set_attr(&NodeRef::Dag(id), "visibility", json!(vis as i32));
            }
            Ok(Val::None)
        }
        "toggle" => {
            for id in targets(s, &a) {
                let v = s.node(&id).map(|n| n.visible).unwrap_or(true);
                s.set_attr(&NodeRef::Dag(id), "visibility", json!(!v as i32));
            }
            Ok(Val::None)
        }
        "currentTime" => {
            if a.has(&["q", "query"]) {
                return Ok(Val::Float(s.time.current));
            }
            let t = a.nums().first().copied().ok_or("currentTime: time required")?;
            s.set_time(t);
            Ok(Val::Float(t))
        }
        "playbackOptions" => {
            if a.has(&["q", "query"]) {
                let t = &s.time;
                let v = if a.has(&["min", "minTime"]) {
                    t.start
                } else if a.has(&["max", "maxTime"]) {
                    t.end
                } else if a.has(&["ast", "animationStartTime"]) {
                    t.anim_start
                } else if a.has(&["aet", "animationEndTime"]) {
                    t.anim_end
                } else {
                    t.fps
                };
                return Ok(Val::Float(v));
            }
            if let Some(v) = a.f(&["min", "minTime"]) {
                s.time.start = v;
            }
            if let Some(v) = a.f(&["max", "maxTime"]) {
                s.time.end = v;
            }
            if let Some(v) = a.f(&["ast", "animationStartTime"]) {
                s.time.anim_start = v;
            }
            if let Some(v) = a.f(&["aet", "animationEndTime"]) {
                s.time.anim_end = v;
            }
            if let Some(v) = a.f(&["fps"]) {
                s.time.fps = v;
            }
            s.rev += 1;
            Ok(Val::None)
        }
        "setKeyframe" => {
            let t = a.f(&["t", "time"]);
            let v = a.f(&["v", "value"]);
            let attrs: Vec<String> = a.flags.iter().filter(|(f, _)| f == "at" || f == "attribute").filter_map(|(_, v)| v.first().map(|x| x.str())).collect();
            let mut n = 0;
            for tgt in a.strings() {
                if let Some((obj, attr)) = tgt.split_once('.') {
                    if let Some(id) = s.find(obj) {
                        s.set_key(&id, attr, t, v);
                        n += 1;
                    }
                }
            }
            if n == 0 {
                let ts: Vec<Id> = if a.strings().is_empty() { s.sel.clone() } else { a.strings().iter().filter_map(|x| s.find(x)).collect() };
                for id in ts {
                    let list: Vec<String> = if attrs.is_empty() { CHANNELS.iter().map(|c| c.to_string()).collect() } else { attrs.clone() };
                    for at in list {
                        s.set_key(&id, &at, t, v);
                        n += 1;
                    }
                }
            }
            Ok(Val::Int(n))
        }
        "cutKey" => {
            let attrs: Vec<String> = a.flags.iter().filter(|(f, _)| f == "at" || f == "attribute").filter_map(|(_, v)| v.first().map(|x| alias(&x.str()).to_string())).collect();
            let t = a.s(&["t", "time"]).and_then(|x| x.split(':').next().and_then(|v| v.parse::<f64>().ok()));
            let mut n = 0;
            for id in targets(s, &a) {
                let keys: Vec<String> = s.anim.curves.iter().filter(|(_, c)| c.node == id && (attrs.is_empty() || attrs.contains(&c.attr))).map(|(k, _)| k.clone()).collect();
                for k in keys {
                    match t {
                        Some(t) => {
                            if s.anim.curves.get_mut(&k).unwrap().remove_at(t) {
                                n += 1;
                            }
                        }
                        None => {
                            n += s.anim.curves.shift_remove(&k).map(|c| c.keys.len()).unwrap_or(0);
                        }
                    }
                }
            }
            s.touch();
            Ok(Val::Int(n as i64))
        }
        "keyframe" => {
            let attrs: Vec<String> = a.flags.iter().filter(|(f, _)| f == "at" || f == "attribute").filter_map(|(_, v)| v.first().map(|x| alias(&x.str()).to_string())).collect();
            let ts = targets(s, &a);
            let curves: Vec<crate::anim::AnimCurve> = s.anim.curves.values().filter(|c| ts.contains(&c.node) && (attrs.is_empty() || attrs.contains(&c.attr))).cloned().collect();
            if a.has(&["kc", "keyframeCount"]) {
                return Ok(Val::Int(curves.iter().map(|c| c.keys.len()).sum::<usize>() as i64));
            }
            if a.has(&["q", "query"]) {
                let vc = a.has(&["vc", "valueChange"]);
                return Ok(Val::Array(curves.iter().flat_map(|c| c.keys.iter().map(move |k| Val::Float(if vc { k.v } else { k.t }))).collect()));
            }
            Ok(Val::Int(0))
        }
        "polyEvaluate" => {
            let id = mesh_targets(s, &a).first().cloned().ok_or("polyEvaluate: select a polygon object")?;
            let m = &s.node(&id).unwrap().mesh().unwrap().mesh;
            Ok(if a.has(&["v", "vertex"]) {
                Val::Int(m.v.len() as i64)
            } else if a.has(&["e", "edge"]) {
                Val::Int(m.edge_count() as i64)
            } else if a.has(&["t", "triangle"]) {
                Val::Int(m.tri_count() as i64)
            } else if a.has(&["b", "boundingBox"]) {
                let b = m.bbox();
                Val::Array(vec![Val::Float(b.min.x), Val::Float(b.max.x), Val::Float(b.min.y), Val::Float(b.max.y), Val::Float(b.min.z), Val::Float(b.max.z)])
            } else {
                Val::Int(m.f.len() as i64)
            })
        }
        "polyExtrudeFacet" | "polyExtrudeFace" => {
            let mut out = vec![];
            for id in mesh_targets(s, &a) {
                let faces = op_faces(s, &a, &id, false);
                if faces.is_empty() {
                    continue;
                }
                let mut p = Params::new();
                p.insert("faces".into(), json!(faces));
                p.insert("localTranslateZ".into(), json!(a.f(&["ltz", "localTranslateZ"]).or(a.f(&["thk", "thickness"])).unwrap_or(0.0)));
                let t = a.v3(&["t", "translate"]).unwrap_or([a.f(&["tx"]).unwrap_or(0.0), a.f(&["ty"]).unwrap_or(0.0), a.f(&["tz"]).unwrap_or(0.0)]);
                p.insert("translateX".into(), json!(t[0]));
                p.insert("translateY".into(), json!(t[1]));
                p.insert("translateZ".into(), json!(t[2]));
                p.insert("offset".into(), json!(a.f(&["off", "offset"]).unwrap_or(0.0)));
                p.insert("divisions".into(), json!(a.f(&["d", "divisions"]).unwrap_or(1.0)));
                p.insert("keepFacesTogether".into(), json!(a.b(&["kft", "keepFacesTogether"], true)));
                let r = s.apply_op(&id, "polyExtrudeFace", p, history_on(&a));
                apply_op_selection(s, &id, &r);
                out.push(last_history_name(s, &id));
            }
            Ok(strs(out))
        }
        "polyExtrudeEdge" => simple_edge_op(s, &a, "polyExtrudeEdge", |a, p| {
            p.insert("offset".into(), json!(a.f(&["off", "offset"]).unwrap_or(0.5)));
        }),
        "polySmooth" => simple_obj_op(s, &a, "polySmoothFace", |a, p| {
            p.insert("divisions".into(), json!(a.f(&["dv", "divisions"]).unwrap_or(1.0)));
        }),
        "polySubdivideFacet" => simple_obj_op(s, &a, "polySubdFace", |a, p| {
            p.insert("divisions".into(), json!(a.f(&["dv", "divisions"]).unwrap_or(1.0)));
        }),
        "polyTriangulate" => simple_face_op(s, &a, "polyTriangulate", true),
        "polyQuad" => simple_obj_op(s, &a, "polyQuad", |a, p| {
            p.insert("angle".into(), json!(a.f(&["a", "angle"]).unwrap_or(30.0)));
        }),
        "polyNormal" => simple_face_op(s, &a, "polyNormal", true),
        "polyPoke" => simple_face_op(s, &a, "polyPoke", true),
        "polyCloseBorder" => simple_obj_op(s, &a, "polyCloseBorder", |_, _| {}),
        "polyMirrorFace" => simple_obj_op(s, &a, "polyMirror", |a, p| {
            p.insert("axis".into(), json!(a.f(&["ma", "mirrorAxis", "axis"]).unwrap_or(0.0)));
            p.insert("position".into(), json!(a.f(&["p", "pivot"]).unwrap_or(0.0)));
            p.insert("merge".into(), json!(a.b(&["mm", "mergeMode"], true)));
            p.insert("threshold".into(), json!(a.f(&["mt", "mergeThreshold"]).unwrap_or(0.001)));
        }),
        "polyMergeVertex" => {
            let mut out = vec![];
            for id in mesh_targets(s, &a) {
                let v = op_verts(s, &a, &id);
                let mut p = Params::new();
                if !v.is_empty() {
                    p.insert("verts".into(), json!(v));
                }
                p.insert("threshold".into(), json!(a.f(&["d", "distance"]).unwrap_or(0.01)));
                s.apply_op(&id, "polyMergeVert", p, history_on(&a));
                s.comp.remove(&id);
                out.push(last_history_name(s, &id));
            }
            Ok(strs(out))
        }
        "polyDelFacet" | "polyDelEdge" | "polyDelVertex" => {
            delete_components(s, &a);
            Ok(Val::None)
        }
        "polyUnite" => {
            let ts = mesh_targets(s, &a);
            let o = s.combine(&ts).ok_or("polyUnite: select two or more polygon objects")?;
            if let Some(n) = a.s(&["n", "name"]) {
                rename(s, &o, &n);
            }
            s.select(std::slice::from_ref(&o), SelMode::Replace);
            Ok(Val::Array(vec![Val::Str(s.name_of(&o))]))
        }
        "polySeparate" => {
            let mut out = vec![];
            for id in mesh_targets(s, &a) {
                out.extend(s.separate(&id));
            }
            s.select(&out, SelMode::Replace);
            Ok(strs(out.iter().map(|i| s.name_of(i)).collect()))
        }
        "polyCBoolOp" | "polyBoolOp" => {
            let ts = mesh_targets(s, &a);
            if ts.len() < 2 {
                return Err("Select two polygon objects (first = A, second = B).".into());
            }
            let op = a.f(&["op", "operation"]).unwrap_or(1.0) as i64 - 1;
            let (am, bm) = (ts[0].clone(), ts[1].clone());
            let mat = s.node(&am).unwrap().mesh().unwrap().material.clone();
            let o = s.create_mesh_from(Default::default(), "polySurface1", Some(&mat), None);
            if let Some(m) = s.nodes.get_mut(&o).and_then(|n| n.mesh_mut()) {
                m.history.clear();
            }
            let mut p = Params::new();
            p.insert("a".into(), json!(am));
            p.insert("b".into(), json!(bm));
            p.insert("operation".into(), json!(op.clamp(0, 2)));
            s.add_history(&o, "polyCBoolOp", p);
            if let Some(e) = s.node(&o).and_then(|n| n.mesh()).and_then(|m| m.history[0].error.clone()) {
                s.delete_node(&o);
                return Err(e);
            }
            s.center_pivot(&o);
            for x in [&am, &bm] {
                s.set_attr(&NodeRef::Dag(x.clone()), "visibility", json!(0));
            }
            if let Some(n) = a.s(&["n", "name"]) {
                rename(s, &o, &n);
            }
            s.select(std::slice::from_ref(&o), SelMode::Replace);
            Ok(result_names(s, &o))
        }
        "polyBevel" | "polyBevel3" => {
            let mut out = vec![];
            for id in mesh_targets(s, &a) {
                let mut e = op_edges(s, &a, &id);
                if e.is_empty() {
                    // faces bevel their border edges; an object bevels every edge
                    let f = op_faces(s, &a, &id, false);
                    if !f.is_empty() {
                        let pm = &s.node(&id).unwrap().mesh().unwrap().mesh;
                        let set: std::collections::HashSet<usize> = f.iter().copied().collect();
                        let topo = pm.topo();
                        e = topo.edges.iter().enumerate().filter(|(i, _)| topo.edge_faces[*i].iter().filter(|x| set.contains(x)).count() == 1).map(|(_, e)| *e).collect();
                    }
                }
                let mut p = Params::new();
                if !e.is_empty() {
                    p.insert("edges".into(), crate::params::edges_value(&e));
                }
                p.insert("fraction".into(), json!(a.f(&["f", "fraction"]).unwrap_or(0.5)));
                let r = s.apply_op(&id, "polyBevel", p, history_on(&a));
                apply_op_selection(s, &id, &r);
                out.push(last_history_name(s, &id));
            }
            if out.is_empty() {
                return Err("Select edges, faces or a polygon object.".into());
            }
            Ok(strs(out))
        }
        "joint" if a.has(&["e", "edit"]) => {
            // joint -e -oj xyz: orient the joints (and their children)
            let js: Vec<Id> = targets(s, &a).into_iter().filter(|i| s.node(i).is_some_and(|n| n.kind == "joint")).collect();
            if js.is_empty() {
                return Err("joint -e: select joints".into());
            }
            s.orient_hierarchy(&js);
            Ok(Val::None)
        }
        "ikHandle" => {
            let ts = targets(s, &a);
            let sj = a.s(&["sj", "startJoint"]).and_then(|n| s.find(&n)).or_else(|| ts.first().cloned()).ok_or("ikHandle: no start joint")?;
            let ee = a.s(&["ee", "endEffector"]).and_then(|n| s.find(&n)).or_else(|| ts.last().cloned()).ok_or("ikHandle: no end joint")?;
            let sol = a.s(&["sol", "solver"]).unwrap_or("ikRPsolver".into());
            let h = s.create_ik_handle(&sj, &ee, &sol)?;
            if let Some(n) = a.s(&["n", "name"]) {
                rename(s, &h, &n);
            }
            s.select(std::slice::from_ref(&h), SelMode::Replace);
            Ok(Val::Array(vec![Val::Str(s.name_of(&h))]))
        }
        "insertJoint" => {
            let j = targets(s, &a).into_iter().find(|i| s.node(i).is_some_and(|n| n.kind == "joint")).ok_or("insertJoint: select a joint")?;
            let n = s.insert_joint(&j)?;
            s.select(std::slice::from_ref(&n), SelMode::Replace);
            Ok(Val::Str(s.name_of(&n)))
        }
        "mirrorJoint" => {
            let j = targets(s, &a).into_iter().find(|i| s.node(i).is_some_and(|n| n.kind == "joint")).ok_or("mirrorJoint: select the root joint")?;
            let axis = if a.has(&["mxz", "mirrorXZ"]) { 1 } else if a.has(&["mxy", "mirrorXY"]) { 2 } else { 0 };
            let sr: Vec<String> = a.flags.iter().find(|(k, _)| k == "sr" || k == "searchReplace").map(|(_, v)| v.iter().map(|x| x.str()).collect()).unwrap_or_default();
            let n = s.mirror_joints(&j, axis, sr.first().map(|x| x.as_str()).unwrap_or(""), sr.get(1).map(|x| x.as_str()).unwrap_or(""));
            s.select(std::slice::from_ref(&n), SelMode::Replace);
            Ok(Val::Str(s.name_of(&n)))
        }
        "cluster" | "softMod" => {
            // components (or whole meshes) get a cluster / soft mod handle at their centre
            let meshes = mesh_targets(s, &a);
            if meshes.is_empty() {
                return Err(format!("{name}: select vertices or meshes"));
            }
            let mut entries = vec![];
            let mut c = DVec3::ZERO;
            let mut n = 0.0f64;
            for m in &meshes {
                let comps = s.comp_mode.is_some() || a.strings().iter().any(|t| t.contains('['));
                let vs: Vec<u32> = if comps { op_verts(s, &a, m) } else { (0..s.node(m).unwrap().mesh().unwrap().mesh.v.len() as u32).collect() };
                let wm = s.world_matrix(m);
                let pm = &s.node(m).unwrap().mesh().unwrap().mesh;
                for v in &vs {
                    c += wm.transform_point3(pm.v[*v as usize]);
                    n += 1.0;
                }
                entries.push((m.clone(), vs));
            }
            let c = c / n.max(1.0);
            let kind = "clusterHandle";
            let base = a.s(&["n", "name"]).unwrap_or(format!("{name}1"));
            let h = s.create_handle(kind, &format!("{base}Handle"));
            s.nodes.get_mut(&h).unwrap().xf.t = c.to_array();
            let bind = s.world_matrix(&h).to_cols_array();
            s.set_rig(&h, json!({ "deformer": name, "name": base }));
            let mut out = vec![];
            for (m, vs) in entries {
                let mut p = Params::new();
                p.insert("handle".into(), json!(h));
                p.insert("owner".into(), json!(m));
                p.insert("bind".into(), json!(bind));
                p.insert("envelope".into(), json!(1.0));
                if name == "cluster" {
                    let w: serde_json::Map<String, serde_json::Value> = vs.iter().map(|v| (v.to_string(), json!(1.0))).collect();
                    p.insert("weights".into(), serde_json::Value::Object(w));
                } else {
                    p.insert("center".into(), json!(c.to_array()));
                    p.insert("falloffRadius".into(), json!(a.f(&["fr", "falloffRadius"]).unwrap_or(5.0)));
                }
                s.add_history(&m, name, p);
                s.comp.remove(&m);
                out.push(last_history_name(s, &m));
            }
            s.comp_mode = None;
            s.select(std::slice::from_ref(&h), SelMode::Replace);
            out.push(s.name_of(&h));
            Ok(strs(out))
        }
        "lattice" => {
            let meshes: Vec<Id> = targets(s, &a).iter().flat_map(|t| s.subtree(t)).filter(|i| s.node(i).is_some_and(|n| n.mesh().is_some())).collect();
            if meshes.is_empty() {
                return Err("lattice: select meshes".into());
            }
            let dv = a.v3(&["dv", "divisions"]).unwrap_or([2.0, 5.0, 2.0]).map(|x| x.round().clamp(2.0, 12.0) as usize);
            let mut bb = Aabb::EMPTY;
            for m in &meshes {
                bb.union(&s.world_bbox(m));
            }
            let sz = (bb.size() * 1.02).max(DVec3::splat(0.1));
            let base = a.s(&["n", "name"]).unwrap_or("ffd1".into());
            let h = s.create_handle("lattice", &format!("{base}Lattice"));
            {
                let n = s.nodes.get_mut(&h).unwrap();
                n.xf.t = bb.center().to_array();
                n.xf.s = sz.to_array();
                n.extra = json!({ "envelope": 1.0, "sDivisions": dv[0], "tDivisions": dv[1], "uDivisions": dv[2] });
            }
            let wm = s.world_matrix(&h).to_cols_array();
            s.set_rig(&h, json!({ "deformer": "ffd", "name": base, "base": wm, "pts": crate::deformers::lattice_grid(dv), "sel": [] }));
            let mut out = vec![];
            for m in &meshes {
                let mut p = Params::new();
                p.insert("handle".into(), json!(h));
                p.insert("owner".into(), json!(m));
                p.insert("envelope".into(), json!(1.0));
                s.add_history(m, "ffd", p);
                out.push(last_history_name(s, m));
            }
            s.select(std::slice::from_ref(&h), SelMode::Replace);
            out.push(s.name_of(&h));
            Ok(strs(out))
        }
        "blendShape" => {
            // blendShape target1 target2 ... base: weights become channels on the base mesh
            let ms: Vec<Id> = targets(s, &a).into_iter().filter(|i| s.node(i).is_some_and(|n| n.mesh().is_some())).collect();
            if ms.len() < 2 {
                return Err("blendShape: select the target shapes, then the base mesh last".into());
            }
            let base = ms.last().unwrap().clone();
            let bv = s.node(&base).unwrap().mesh().unwrap().mesh.v.clone();
            let mut tlist = vec![];
            for t in &ms[..ms.len() - 1] {
                let tv = &s.node(t).unwrap().mesh().unwrap().mesh.v;
                if tv.len() != bv.len() {
                    s.msg(MsgKind::Warning, format!("// Warning: {} has a different number of vertices than {}; skipped.", s.name_of(t), s.name_of(&base)));
                    continue;
                }
                let d: Vec<serde_json::Value> = bv.iter().zip(tv).enumerate().filter(|(_, (b, t))| (**t - **b).abs().max_element() > 1e-7).map(|(i, (b, t))| json!([i, t.x - b.x, t.y - b.y, t.z - b.z])).collect();
                let mut nm = s.name_of(t);
                while s.node(&base).and_then(|n| n.extra.get(&nm)).is_some() {
                    nm.push('_');
                }
                s.set_extra(&base, &nm, json!(0.0));
                tlist.push(json!({ "name": nm, "id": t, "d": d }));
            }
            // add to an existing blend shape node on the base, or make one
            let existing = s.node(&base).and_then(|n| n.mesh()).and_then(|m| m.history.iter().position(|h| h.ty == "blendShape"));
            match existing {
                Some(i) => {
                    let mut all = s.node(&base).unwrap().mesh().unwrap().history[i].params.get("targets").cloned().unwrap_or(json!([]));
                    all.as_array_mut().unwrap().extend(tlist);
                    s.set_history_param(&base, i, "targets", all);
                }
                None => {
                    let mut p = Params::new();
                    p.insert("owner".into(), json!(base));
                    p.insert("targets".into(), serde_json::Value::Array(tlist));
                    p.insert("envelope".into(), json!(1.0));
                    s.add_history(&base, "blendShape", p);
                }
            }
            s.select(std::slice::from_ref(&base), SelMode::Replace);
            Ok(Val::Str(last_history_name(s, &base)))
        }
        "poleVectorConstraint" => {
            let mut ts = targets(s, &a);
            let driven = ts.pop().ok_or("poleVectorConstraint: select the target, then the IK handle")?;
            if s.node(&driven).map(|n| n.kind.as_str()) != Some("ikHandle") || ts.is_empty() {
                return Err("Pole vector constraint needs a target and an IK handle (rotate plane solver) selected last".into());
            }
            let name = s.add_constraint("poleVector", ts, &driven, false);
            Ok(Val::Str(name))
        }
        "setDrivenKeyframe" => {
            // setDrivenKeyframe -cd driver.attr driven.attr ...
            let cd = a.s(&["cd", "currentDriver"]).ok_or("setDrivenKeyframe: -cd driver.attribute required")?;
            let (dn, da) = cd.split_once('.').ok_or("setDrivenKeyframe: -cd expects node.attribute")?;
            let drv = s.find(dn).ok_or(format!("No object matches name: {dn}"))?;
            let mut n = 0;
            for t in a.strings() {
                let (on, oa) = match t.split_once('.') {
                    Some((o, at)) => (o.to_string(), at.to_string()),
                    None => (t.clone(), a.s(&["at", "attribute"]).ok_or("setDrivenKeyframe: -at attribute required")?),
                };
                let id = s.find(&on).ok_or(format!("No object matches name: {on}"))?;
                s.set_driven_key(&id, &oa, &drv, da);
                n += 1;
            }
            if n == 0 {
                return Err("setDrivenKeyframe: name the driven node.attribute".into());
            }
            Ok(Val::Int(n))
        }
        "pathAnimation" => {
            let c = a.s(&["c", "curve"]).and_then(|n| s.find(&n)).ok_or("pathAnimation: -c curve required")?;
            let follow = a.b(&["f", "follow"], true);
            let fa = a.s(&["fa", "followAxis"]).unwrap_or("x".into());
            let ua = a.s(&["ua", "upAxis"]).unwrap_or("y".into());
            let objs: Vec<Id> = targets(s, &a).into_iter().filter(|i| *i != c).collect();
            if objs.is_empty() {
                return Err("pathAnimation: select the objects to animate".into());
            }
            for o in &objs {
                s.attach_motion_path(o, &c, follow, &fa, &ua);
            }
            Ok(strs(objs.iter().map(|o| s.name_of(o)).collect()))
        }
        "removeConstraints" => {
            let ts = targets(s, &a);
            Ok(Val::Int(s.remove_constraints(&ts) as i64))
        }
        "deleteDeformers" => {
            // deformer history on the selected meshes, and the selected handles
            let ts = targets(s, &a);
            let mut n = 0;
            for t in ts.iter().flat_map(|t| s.subtree(t)).collect::<Vec<_>>() {
                if s.node(&t).is_some_and(|n| n.mesh().is_some()) {
                    let removed = s.remove_history_types(&t, &["cluster", "softMod", "ffd", "blendShape", "skinCluster", "bend", "flare", "sine", "squash", "twist", "wave"]);
                    n += removed;
                    if let Some(x) = s.nodes.get_mut(&t) {
                        if let Some(e) = x.extra.as_object_mut() {
                            e.retain(|_, v| !v.is_number() || removed == 0);
                        }
                    }
                }
            }
            for h in ts.iter().filter(|t| s.node(t).is_some_and(|n| matches!(n.kind.as_str(), "deformer" | "clusterHandle" | "lattice"))).cloned().collect::<Vec<_>>() {
                for m in s.nodes.values().filter(|n| n.mesh().is_some_and(|m| m.history.iter().any(|x| x.params.get("handle").and_then(|v| v.as_str()) == Some(h.as_str())))).map(|n| n.id.clone()).collect::<Vec<_>>() {
                    if let Some(mm) = s.nodes.get_mut(&m).and_then(|n| n.mesh_mut()) {
                        mm.history.retain(|x| x.params.get("handle").and_then(|v| v.as_str()) != Some(h.as_str()));
                    }
                    s.evaluate(&m, 0);
                    n += 1;
                }
            }
            for h in s.orphan_deformer_handles() {
                s.delete_node(&h);
            }
            s.touch();
            Ok(Val::Int(n as i64))
        }
        "joint" => {
            // a selected joint becomes the parent, like Maya's Joint Tool
            let parent = s.lead().filter(|l| s.node(l).is_some_and(|n| n.kind == "joint")).cloned();
            let nm = a.s(&["n", "name"]).unwrap_or("joint1".into());
            let id = s.create_handle("joint", &nm);
            if let Some(r) = a.f(&["rad", "radius"]) {
                s.nodes.get_mut(&id).unwrap().passthrough.insert("rig".into(), json!({ "radius": r }));
            }
            if let Some(p) = &parent {
                s.reparent(&id, Some(p), None);
            }
            if let Some(p) = a.v3(&["p", "position"]) {
                let pw = s.parent_world(&id);
                let local = pw.inverse().transform_point3(DVec3::from_array(p));
                let n = s.nodes.get_mut(&id).unwrap();
                n.xf = Xform { t: local.to_array(), ..Default::default() };
            }
            s.select(std::slice::from_ref(&id), SelMode::Replace);
            Ok(Val::Str(s.name_of(&id)))
        }
        "skinCluster" if a.has(&["ub", "unbind"]) => {
            let mut n = 0;
            for t in targets(s, &a).iter().flat_map(|t| s.subtree(t)).collect::<Vec<_>>() {
                n += s.remove_history_types(&t, &["skinCluster"]);
            }
            Ok(Val::Int(n as i64))
        }
        "skinCluster" => {
            let ts = targets(s, &a);
            let mut joints: Vec<Id> = vec![];
            for t in &ts {
                for i in s.subtree(t) {
                    if s.node(&i).is_some_and(|n| n.kind == "joint") && !joints.contains(&i) {
                        joints.push(i);
                    }
                }
            }
            let meshes: Vec<Id> = ts.iter().flat_map(|t| s.subtree(t)).filter(|i| s.node(i).is_some_and(|n| n.mesh().is_some())).collect();
            if joints.is_empty() || meshes.is_empty() {
                return Err("skinCluster: select joints and a polygon object".into());
            }
            let mi = a.f(&["mi", "maximumInfluences"]).unwrap_or(3.0) as usize;
            let mut out = vec![];
            for m in &meshes {
                let p = crate::skin::bind(s, m, &joints, mi);
                s.add_history(m, "skinCluster", p);
                out.push(last_history_name(s, m));
            }
            Ok(strs(out))
        }
        "parentConstraint" | "pointConstraint" | "orientConstraint" | "scaleConstraint" | "aimConstraint" => {
            let mut ts = targets(s, &a);
            if ts.len() < 2 {
                return Err(format!("{name}: select target(s) then the object to constrain"));
            }
            let driven = ts.pop().unwrap();
            let kind = name.trim_end_matches("Constraint");
            let mo = a.b(&["mo", "maintainOffset"], false);
            let c = s.add_constraint(kind, ts, &driven, mo);
            if let (Some(aim), Some(c)) = (a.v3(&["aim", "aimVector"]), s.constraints.last_mut()) {
                c.aim = aim;
            }
            Ok(strs(vec![c]))
        }
        "emitter" | "fireEffect" | "smokeEffect" => {
            let ty = a.s(&["type"]).unwrap_or(if name == "emitter" { "omni".into() } else { "directional".into() });
            let sel_mesh = s.sel.iter().find(|i| s.node(i).is_some_and(|n| n.mesh().is_some())).cloned();
            let id = s.create_emitter(&ty, a.s(&["n", "name"]).as_deref().unwrap_or("emitter1"));
            let set = |s: &mut Scene, k: &str, v: serde_json::Value| {
                s.nodes.get_mut(&id).unwrap().extra[k] = v;
            };
            if let Some(src) = &sel_mesh {
                if ty == "surface" {
                    set(s, "source", json!(src));
                }
                let c = s.world_bbox(src).center();
                s.nodes.get_mut(&id).unwrap().xf.t = c.to_array();
            }
            if let Some(p) = a.v3(&["pos", "position"]) {
                s.nodes.get_mut(&id).unwrap().xf.t = p;
            }
            for (flag, key) in [("r", "rate"), ("spd", "speed"), ("sp", "spread"), ("lsp", "lifespan"), ("dx", "directionX"), ("dy", "directionY"), ("dz", "directionZ")] {
                if let Some(v) = a.f(&[flag, key]) {
                    set(s, key, json!(v));
                }
            }
            match name {
                "fireEffect" => {
                    for (k, v) in [("rate", json!(400.0)), ("speed", json!(2.0)), ("spread", json!(0.35)), ("lifespan", json!(0.9)), ("color", json!([1.0, 0.75, 0.2])), ("endColor", json!([0.8, 0.1, 0.0])), ("size", json!(5.0))] {
                        set(s, k, v);
                    }
                    let t = s.create_field("turbulence");
                    s.nodes.get_mut(&t).unwrap().extra["magnitude"] = json!(3.0);
                    s.connect_dynamic(&t, std::slice::from_ref(&id), true);
                }
                "smokeEffect" => {
                    for (k, v) in [("rate", json!(120.0)), ("speed", json!(1.0)), ("spread", json!(0.25)), ("lifespan", json!(4.0)), ("color", json!([0.55, 0.55, 0.55])), ("endColor", json!([0.25, 0.25, 0.25])), ("size", json!(7.0))] {
                        set(s, k, v);
                    }
                    let t = s.create_field("turbulence");
                    s.nodes.get_mut(&t).unwrap().extra["magnitude"] = json!(1.0);
                    s.connect_dynamic(&t, std::slice::from_ref(&id), true);
                }
                _ => {}
            }
            s.select(std::slice::from_ref(&id), SelMode::Replace);
            s.dynamics.frame = None;
            Ok(Val::Str(s.name_of(&id)))
        }
        "particle" => {
            let sp = a.f(&["grs", "gridSpacing"]).unwrap_or(0.5);
            let lo = a.v3(&["ll", "lowerLeft"]).map(DVec3::from_array).unwrap_or(DVec3::new(-2.25, 5.0, -2.25));
            let hi = a.v3(&["ur", "upperRight"]).map(DVec3::from_array).unwrap_or(DVec3::new(2.25, 5.0, 2.25));
            let id = s.create_particle_grid(lo.min(hi), lo.max(hi), sp);
            if let Some(n) = a.s(&["n", "name"]) {
                rename(s, &id, &n);
            }
            s.select(std::slice::from_ref(&id), SelMode::Replace);
            s.dynamics.frame = None;
            Ok(Val::Str(s.name_of(&id)))
        }
        "connectDynamic" => {
            let f = a.s(&["f", "fields"]).and_then(|n| s.find(&n)).ok_or("connectDynamic: -f field required")?;
            let objs: Vec<Id> = targets(s, &a).into_iter().filter(|i| *i != f).collect();
            if objs.is_empty() {
                return Err("connectDynamic: name the particle objects / rigid bodies".into());
            }
            s.connect_dynamic(&f, &objs, !a.has(&["d", "delete"]));
            Ok(strs(objs.iter().map(|o| s.name_of(o)).collect()))
        }
        "deleteRigidBody" => {
            let mut n = 0;
            for t in targets(s, &a) {
                n += s.remove_rigid(&t) as i64;
            }
            Ok(Val::Int(n))
        }
        "gravity" | "drag" | "turbulence" | "radial" | "vortex" | "air" => {
            // like Maya: selected particle objects / rigid bodies get connected to the new field
            let dynsel: Vec<Id> = s.sel.iter().filter(|i| s.node(i).is_some_and(|n| n.kind == "emitter" || n.extra.get("rigidBody").is_some())).cloned().collect();
            let id = s.create_field(name);
            if !dynsel.is_empty() {
                s.connect_dynamic(&id, &dynsel, true);
            }
            if let Some(m) = a.f(&["m", "magnitude"]) {
                s.nodes.get_mut(&id).unwrap().extra["magnitude"] = json!(m);
            }
            for (flag, key) in [("dx", "directionX"), ("dy", "directionY"), ("dz", "directionZ"), ("mxd", "maxDistance"), ("att", "attenuation"), ("f", "frequency")] {
                if let Some(v) = a.f(&[flag, key]) {
                    s.nodes.get_mut(&id).unwrap().extra[key] = json!(v);
                }
            }
            if let Some(p) = a.v3(&["pos", "position"]) {
                s.nodes.get_mut(&id).unwrap().xf.t = p;
            }
            s.select(std::slice::from_ref(&id), SelMode::Replace);
            s.dynamics.frame = None;
            Ok(Val::Str(s.name_of(&id)))
        }
        "rigidBody" => {
            let active = !a.has(&["passive", "pas"]);
            let ids: Vec<Id> = targets(s, &a).into_iter().filter(|i| s.node(i).is_some_and(|n| n.mesh().is_some())).collect();
            if ids.is_empty() {
                return Err("rigidBody: select polygon objects".into());
            }
            for id in &ids {
                s.make_rigid(id, active);
                for (flag, key) in [("m", "mass"), ("b", "bounciness"), ("sf", "friction")] {
                    if let Some(v) = a.f(&[flag, key]) {
                        s.nodes.get_mut(id).unwrap().extra[key] = json!(v);
                    }
                }
            }
            Ok(strs(ids.iter().map(|i| format!("rigidBody_{}", s.name_of(i))).collect()))
        }
        "nonLinear" => {
            let kind = a.s(&["type"]).unwrap_or("bend".into());
            if !crate::deformers::KINDS.contains(&kind.as_str()) {
                return Err(format!("nonLinear: unknown type {kind}"));
            }
            let meshes: Vec<Id> = targets(s, &a).iter().flat_map(|t| s.subtree(t)).filter(|i| s.node(i).is_some_and(|n| n.mesh().is_some())).collect();
            if meshes.is_empty() {
                return Err("nonLinear: select a polygon object".into());
            }
            // the handle covers the meshes: bounds -1..1 span their height
            let mut bb = Aabb::EMPTY;
            for m in &meshes {
                bb.union(&s.world_bbox(m));
            }
            let h = s.create_handle("deformer", &format!("{kind}1Handle"));
            let half = (bb.size().y * 0.5).max(0.01);
            {
                let n = s.nodes.get_mut(&h).unwrap();
                n.xf.t = bb.center().to_array();
                n.xf.s = [half; 3];
                n.extra = json!({ "deformer": kind });
            }
            let mut p = crate::deformers::defaults(&kind);
            for (f, v) in &a.flags {
                if p.contains_key(f) {
                    if let Some(x) = v.first() {
                        p.insert(f.clone(), json!(x.num()));
                    }
                }
            }
            p.insert("handle".into(), json!(h));
            let mut hist = String::new();
            for m in &meshes {
                let mut pm = p.clone();
                pm.insert("owner".into(), json!(m));
                s.add_history(m, &kind, pm);
                hist = last_history_name(s, m);
            }
            s.select(std::slice::from_ref(&h), SelMode::Replace);
            Ok(strs(vec![hist, s.name_of(&h)]))
        }
        "polyBridgeEdge" => simple_edge_op(s, &a, "polyBridgeEdge", |a, p| {
            p.insert("divisions".into(), json!(a.f(&["dv", "divisions"]).unwrap_or(0.0)));
        }),
        "polySoftEdge" => {
            let mut out = vec![];
            for id in mesh_targets(s, &a) {
                let e = op_edges(s, &a, &id);
                let mut p = Params::new();
                p.insert("angle".into(), json!(a.f(&["a", "angle"]).unwrap_or(180.0)));
                if !e.is_empty() {
                    p.insert("edges".into(), crate::params::edges_value(&e));
                }
                s.apply_op(&id, "polySoftEdge", p, history_on(&a));
                out.push(last_history_name(s, &id));
            }
            Ok(strs(out))
        }
        "polyProjection" => {
            let mode = a.s(&["t", "type"]).unwrap_or("planar".into()).to_lowercase();
            let axis = match a.s(&["md", "mapDirection"]).as_deref() {
                Some("x") | Some("X") => 0,
                Some("z") | Some("Z") => 2,
                _ => 1,
            };
            let mut out = vec![];
            for id in mesh_targets(s, &a) {
                let faces = op_faces(s, &a, &id, false);
                let mut p = Params::new();
                p.insert("mode".into(), json!(mode));
                p.insert("axis".into(), json!(axis));
                if !faces.is_empty() {
                    p.insert("faces".into(), json!(faces));
                }
                s.apply_op(&id, "polyProj", p, history_on(&a));
                out.push(last_history_name(s, &id));
            }
            Ok(strs(out))
        }
        "polySplitRing" => {
            let mut out = vec![];
            for id in mesh_targets(s, &a) {
                let e = op_edges(s, &a, &id);
                let Some(&(x, y)) = e.first() else { continue };
                let mut p = Params::new();
                p.insert("edge".into(), json!([x, y]));
                p.insert("t".into(), json!(a.f(&["wt", "weight"]).unwrap_or(0.5)));
                p.insert("multiple".into(), json!(a.f(&["div", "divisions"]).unwrap_or(1.0)));
                let r = s.apply_op(&id, "polySplitRing", p, history_on(&a));
                apply_op_selection(s, &id, &r);
                out.push(last_history_name(s, &id));
            }
            Ok(strs(out))
        }
        "polyChipOff" | "polyDuplicateAndConnect" => {
            let mut out = vec![];
            for id in mesh_targets(s, &a) {
                let f = op_faces(s, &a, &id, false);
                if f.is_empty() {
                    continue;
                }
                if let Some(n) = s.extract_faces(&id, &f, !a.b(&["dup", "duplicate"], true) || name == "polyDuplicateAndConnect", history_on(&a)) {
                    out.push(s.name_of(&n));
                }
                s.comp.remove(&id);
            }
            Ok(strs(out))
        }
        "revolve" => curve_surface(s, &a, "revolve"),
        "loft" => curve_surface(s, &a, "loft"),
        "extrude" => curve_surface(s, &a, "extrude"),
        "planarSrf" => curve_surface(s, &a, "planarTrim"),
        "closeCurve" | "openCurve" => {
            let form = if name == "closeCurve" { "periodic" } else { "open" };
            let mut out = vec![];
            for id in targets(s, &a) {
                if let Some(Shape::Curve(c)) = s.nodes.get_mut(&id).map(|n| &mut n.shape) {
                    c.form = form.into();
                    out.push(id.clone());
                }
                if let Some(n) = s.nodes.get_mut(&id) {
                    n.rev += 1;
                }
                s.refresh_dependents(&id);
            }
            if out.is_empty() {
                return Err(format!("{name}: select a curve"));
            }
            s.touch();
            Ok(strs(out.iter().map(|i| s.name_of(i)).collect()))
        }
        "rebuildCurve" | "reverseCurve" => {
            for id in targets(s, &a) {
                if let Some(Shape::Curve(c)) = s.node(&id).map(|n| n.shape.clone()) {
                    let nc = if name == "reverseCurve" { c.reversed() } else { c.rebuilt(a.f(&["s", "spans"]).unwrap_or(4.0) as usize, a.f(&["d", "degree"]).unwrap_or(3.0) as usize) };
                    if let Some(n) = s.nodes.get_mut(&id) {
                        n.shape = Shape::Curve(nc);
                        n.rev += 1;
                    }
                    s.refresh_dependents(&id);
                }
            }
            s.touch();
            Ok(Val::None)
        }
        "nurbsToPoly" => {
            let mut out = vec![];
            for id in targets(s, &a) {
                if let Some(m) = s.nodes.get_mut(&id).and_then(|n| n.mesh_mut()) {
                    m.nurbs = false;
                    m.mesh.display_edges = None;
                    m.mesh.soft_angle = 60.0;
                    let pm = m.mesh.clone();
                    s.bake_mesh(&id, pm);
                    out.push(s.name_of(&id));
                }
            }
            Ok(strs(out))
        }
        "displaySmoothness" => {
            let lvl = a.f(&["po", "polygonObject"]).map(|x| if x >= 3.0 { 2.0 } else { 0.0 }).unwrap_or(0.0);
            for id in targets(s, &a) {
                s.set_attr(&NodeRef::Dag(id), "smoothLevel", json!(lvl));
            }
            Ok(Val::None)
        }
        "file" => file_cmd(s, &a),
        "polyChamferVtx" => {
            let mut out = vec![];
            for id in mesh_targets(s, &a) {
                let v: Vec<u32> = if s.comp_mode == Some(CompMode::Vertex) || a.strings().iter().any(|t| t.contains(".vtx[")) { op_verts(s, &a, &id) } else { vec![] };
                if v.is_empty() {
                    continue;
                }
                // Maya's positional form: polyChamferVtx 1 <width> 0
                let w = a.f(&["w", "width"]).or_else(|| a.pos.get(1).map(|x| x.num()).filter(|x| *x > 0.0)).unwrap_or(0.25);
                let mut p = Params::new();
                p.insert("verts".into(), json!(v));
                p.insert("width".into(), json!(w));
                let r = s.apply_op(&id, "polyChamferVtx", p, history_on(&a));
                apply_op_selection(s, &id, &r);
                out.push(last_history_name(s, &id));
            }
            if out.is_empty() {
                return Err("Select vertices to chamfer".into());
            }
            Ok(strs(out))
        }
        "polyConnectComponents" => {
            let mut out = vec![];
            for id in mesh_targets(s, &a) {
                let pm = s.node(&id).and_then(|n| n.mesh()).map(|m| m.mesh.clone()).unwrap_or_default();
                let cs = s.comp_sel(&id);
                let pts: Vec<serde_json::Value> = if s.comp_mode == Some(CompMode::Vertex) && cs.v.len() >= 2 {
                    cs.v.iter().map(|v| json!({ "vertex": v })).collect()
                } else {
                    let es = op_edges(s, &a, &id);
                    if es.len() < 2 {
                        continue;
                    }
                    // order the edges so each shares a face with the next
                    let topo = pm.topo();
                    let faces_of = |e: (u32, u32)| topo.edge(e.0, e.1).map(|i| topo.edge_faces[i].clone()).unwrap_or_default();
                    let mut order = vec![es[0]];
                    let mut used = vec![false; es.len()];
                    used[0] = true;
                    while order.len() < es.len() {
                        let last = faces_of(*order.last().unwrap());
                        let Some(i) = (0..es.len()).find(|&i| !used[i] && faces_of(es[i]).iter().any(|f| last.contains(f))) else { break };
                        used[i] = true;
                        order.push(es[i]);
                    }
                    order.iter().map(|e| json!({ "edge": [e.0, e.1], "t": 0.5 })).collect()
                };
                if pts.len() < 2 {
                    continue;
                }
                let mut p = Params::new();
                p.insert("points".into(), serde_json::Value::Array(pts));
                let r = s.apply_op(&id, "polySplit", p, history_on(&a));
                apply_op_selection(s, &id, &r);
                out.push(last_history_name(s, &id));
            }
            if out.is_empty() {
                return Err("Select two or more vertices or edges to connect".into());
            }
            Ok(strs(out))
        }
        "polyCollapseEdge" | "polyCollapseFacet" => {
            // every selected edge / face collapses to its centre
            let mut out = vec![];
            for id in mesh_targets(s, &a) {
                let groups: Vec<Vec<u32>> = if name == "polyCollapseEdge" {
                    op_edges(s, &a, &id).into_iter().map(|(x, y)| vec![x, y]).collect()
                } else {
                    let pm = s.node(&id).and_then(|n| n.mesh()).map(|m| m.mesh.clone()).unwrap_or_default();
                    op_faces(s, &a, &id, false).into_iter().filter_map(|f| pm.f.get(f).cloned()).collect()
                };
                if groups.is_empty() {
                    continue;
                }
                let mut p = Params::new();
                p.insert("groups".into(), json!(groups));
                s.apply_op(&id, "polyMergeCenter", p, history_on(&a));
                s.comp.remove(&id);
                out.push(last_history_name(s, &id));
            }
            if out.is_empty() {
                return Err(format!("{name}: select {} to collapse", if name == "polyCollapseEdge" { "edges" } else { "faces" }));
            }
            Ok(strs(out))
        }
        "polyCleanup" => {
            // merge coincident vertices (JS Cleanup)
            let mut out = vec![];
            for id in mesh_targets(s, &a) {
                let mut p = Params::new();
                p.insert("threshold".into(), json!(1e-4));
                s.apply_op(&id, "polyMergeVert", p, history_on(&a));
                out.push(last_history_name(s, &id));
            }
            Ok(strs(out))
        }
        "polyToCurve" => {
            // a degree-1 curve along a chain of selected edges (world space)
            let id = mesh_targets(s, &a).into_iter().find(|i| !op_edges(s, &a, i).is_empty()).ok_or("polyToCurve: select a chain of edges")?;
            let es = op_edges(s, &a, &id);
            let mut adj: std::collections::BTreeMap<u32, Vec<u32>> = std::collections::BTreeMap::new();
            for &(x, y) in &es {
                adj.entry(x).or_default().push(y);
                adj.entry(y).or_default().push(x);
            }
            let start = adj.iter().find(|(_, n)| n.len() == 1).map(|(k, _)| *k).unwrap_or(es[0].0);
            let mut chain = vec![start];
            let mut used: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
            let mut cur = start;
            while let Some(&nx) = adj.get(&cur).and_then(|n| n.iter().find(|&&v| !used.contains(&crate::mesh::epair(cur, v)))) {
                used.insert(crate::mesh::epair(cur, nx));
                chain.push(nx);
                cur = nx;
                if nx == start {
                    break;
                }
            }
            let closed = chain.len() > 2 && chain.last() == chain.first();
            if closed {
                chain.pop();
            }
            let wm = s.world_matrix(&id);
            let pm = &s.node(&id).unwrap().mesh().unwrap().mesh;
            let pts: Vec<glam::DVec3> = chain.iter().map(|&i| wm.transform_point3(pm.v[i as usize])).collect();
            let c = crate::curves::Curve::new(pts, 1, if closed { "periodic" } else { "open" }, "cv");
            let nm = a.s(&["n", "name"]).unwrap_or_else(|| "polyToCurve1".into());
            let cid = s.create_curve(c, &nm);
            s.comp_mode = None;
            s.select(std::slice::from_ref(&cid), SelMode::Replace);
            Ok(Val::Array(vec![Val::Str(s.name_of(&cid))]))
        }
        "matchTransform" => {
            // the last object is the target (Modify > Match Transformations)
            let ts = targets(s, &a);
            let Some((tgt, rest)) = ts.split_last().filter(|(_, r)| !r.is_empty()) else {
                return Err("matchTransform: select the objects to move, then the target".into());
            };
            let (pos, rot, scl) = (a.has(&["pos", "position"]), a.has(&["rot", "rotation"]), a.has(&["scl", "scale"]));
            let all = !(pos || rot || scl);
            let tw = s.world_matrix(tgt);
            let (tr, ts_) = s.node(tgt).map(|n| (n.xf.r, n.xf.s)).unwrap_or_default();
            for id in rest {
                if all {
                    s.set_world_matrix(id, tw);
                    continue;
                }
                if pos {
                    let d = s.world_pivot(tgt) - s.world_pivot(id);
                    let pl = s.parent_world(id).inverse();
                    let d = pl.transform_vector3(d);
                    if let Some(n) = s.nodes.get_mut(id) {
                        n.xf.t = [n.xf.t[0] + d.x, n.xf.t[1] + d.y, n.xf.t[2] + d.z];
                    }
                }
                if let Some(n) = s.nodes.get_mut(id) {
                    if rot {
                        n.xf.r = tr;
                    }
                    if scl {
                        n.xf.s = ts_;
                    }
                }
                s.touch();
                s.refresh_dependents(id);
            }
            Ok(Val::None)
        }
        "addAttr" => {
            let ln = a.s(&["ln", "longName"]).ok_or("addAttr: -longName required")?;
            let at = a.s(&["at", "attributeType"]).or_else(|| a.s(&["dt", "dataType"])).unwrap_or_else(|| "double".into());
            let dv = a.f(&["dv", "defaultValue"]).unwrap_or(0.0);
            let v = match at.as_str() {
                "bool" => json!(dv != 0.0),
                "long" | "short" | "byte" | "int" => json!(dv.round() as i64),
                "string" => json!(""),
                "double3" | "float3" => json!([dv, dv, dv]),
                _ => json!(dv),
            };
            let mut out = vec![];
            for id in targets(s, &a) {
                let Some(n) = s.nodes.get_mut(&id) else { continue };
                if !n.extra.is_object() {
                    n.extra = json!({});
                }
                let e = n.extra.as_object_mut().unwrap();
                if e.contains_key(&ln) {
                    return Err(format!("addAttr: {} already has an attribute '{ln}'", n.name));
                }
                e.insert(ln.clone(), v.clone());
                out.push(format!("{}.{ln}", n.name));
            }
            s.touch();
            Ok(strs(out))
        }
        "deleteAttr" => {
            let target = a.strings().first().cloned().ok_or("deleteAttr: no attribute")?;
            let (obj, attr) = match target.split_once('.') {
                Some((o, at)) => (o.to_string(), at.to_string()),
                None => (target.clone(), a.s(&["at", "attribute"]).ok_or("deleteAttr: expected node.attribute")?),
            };
            let id = s.resolve(&obj).and_then(|r| match r {
                NodeRef::Dag(i) => Some(i),
                _ => None,
            });
            let removed = id.and_then(|i| s.nodes.get_mut(&i)).and_then(|n| n.extra.as_object_mut()).is_some_and(|e| e.remove(&attr).is_some());
            if !removed {
                return Err(format!("deleteAttr: no dynamic attribute '{attr}' on {obj}"));
            }
            s.touch();
            Ok(Val::None)
        }
        "undo" => {
            let l = s.undo().ok_or("There are no more commands to undo.")?;
            s.msg(MsgKind::Result, format!("// Undo: {l}"));
            Ok(Val::None)
        }
        "redo" => {
            let l = s.redo().ok_or("There are no more commands to redo.")?;
            s.msg(MsgKind::Result, format!("// Redo: {l}"));
            Ok(Val::None)
        }
        "connectAttr" | "disconnectAttr" | "isConnected" => {
            let st = a.strings();
            if st.len() < 2 {
                return Err(format!("{name}: source and destination required"));
            }
            let parse = |t: &str| -> Result<(Id, String), String> {
                let (o, at) = t.split_once('.').ok_or(format!("{t}: expected node.attribute"))?;
                let r = s.resolve(o).ok_or(format!("No object matches name: {o}"))?;
                Ok((s.ref_id(&r), alias(at).to_string()))
            };
            let (src, sa) = parse(&st[0])?;
            let (dst, da) = parse(&st[1])?;
            let c = Conn { src, src_attr: sa, dst, dst_attr: da };
            match name {
                "connectAttr" => {
                    if s.conns.iter().any(|x| x.dst == c.dst && x.dst_attr == c.dst_attr) {
                        if !a.has(&["f", "force"]) {
                            return Err(format!("'{}' is already connected.", st[1]));
                        }
                        s.conns.retain(|x| !(x.dst == c.dst && x.dst_attr == c.dst_attr));
                    }
                    s.conns.push(c);
                    s.apply_anim();
                }
                "disconnectAttr" => s.conns.retain(|x| x != &c),
                _ => return Ok(Val::Int(s.conns.contains(&c) as i64)),
            }
            s.touch();
            Ok(Val::None)
        }
        "listConnections" => {
            let mut out = vec![];
            for t in a.strings() {
                let (o, at) = t.split_once('.').map(|(a, b)| (a.to_string(), Some(alias(b).to_string()))).unwrap_or((t.clone(), None));
                let Some(r) = s.resolve(&o) else { continue };
                let id = s.ref_id(&r);
                for c in &s.conns {
                    if c.dst == id && at.as_ref().is_none_or(|x| *x == c.dst_attr) {
                        if let Some(r) = s.ref_by_id(&c.src) {
                            out.push(s.ref_name(&r));
                        }
                    }
                    if c.src == id && at.as_ref().is_none_or(|x| *x == c.src_attr) {
                        if let Some(r) = s.ref_by_id(&c.dst) {
                            out.push(s.ref_name(&r));
                        }
                    }
                }
            }
            Ok(strs(out))
        }
        "warning" => {
            let m: String = a.pos.iter().map(|v| v.str()).collect();
            s.warn(m);
            Ok(Val::None)
        }
        "error" => Err(a.pos.iter().map(|v| v.str()).collect()),
        "about" => Ok(Val::Str(if a.has(&["v", "version"]) { env!("CARGO_PKG_VERSION").to_string() } else { format!("Inca {}", env!("CARGO_PKG_VERSION")) })),
        "selectMode" => {
            if a.has(&["q", "query"]) {
                return Ok(Val::Int(s.comp_mode.is_some() as i64));
            }
            if a.has(&["o", "object"]) {
                s.set_comp_mode(None);
            } else if a.has(&["co", "component"]) && s.comp_mode.is_none() {
                s.set_comp_mode(Some(CompMode::Vertex));
            }
            Ok(Val::None)
        }
        "selectType" => {
            let on = |names: &[&str]| a.get(names).is_some_and(|v| v.first().is_none_or(|x| x.truthy()));
            if on(&["v", "vertex", "pv", "polymeshVertex"]) {
                s.set_comp_mode(Some(CompMode::Vertex));
            } else if on(&["e", "edge", "pe", "polymeshEdge"]) {
                s.set_comp_mode(Some(CompMode::Edge));
            } else if on(&["f", "facet", "pf", "polymeshFace"]) {
                s.set_comp_mode(Some(CompMode::Face));
            }
            Ok(Val::None)
        }
        "createDisplayLayer" => {
            let nm = a.s(&["n", "name"]).unwrap_or("layer1".into());
            let id = s.new_id("l");
            let nm = s.unique_name(&nm);
            s.layers.push(Layer { id: id.clone(), name: nm.clone(), visible: true, mode: "normal".into(), color: None, extra: Default::default() });
            if !a.has(&["e", "empty"]) {
                for o in targets(s, &a) {
                    if let Some(n) = s.nodes.get_mut(&o) {
                        n.layer = Some(id.clone());
                    }
                }
            }
            s.touch();
            Ok(Val::Str(nm))
        }
        "editDisplayLayerMembers" => {
            let st = a.strings();
            let l = st.first().ok_or("editDisplayLayerMembers: layer name")?;
            let lid = s.layers.iter().find(|x| &x.name == l).map(|x| x.id.clone()).ok_or(format!("No layer {l}"))?;
            for o in st.iter().skip(1).filter_map(|n| s.find(n)).collect::<Vec<_>>() {
                s.nodes.get_mut(&o).unwrap().layer = Some(lid.clone());
            }
            s.touch();
            Ok(Val::None)
        }
        "help" | "whatIs" => {
            let n = a.strings().first().cloned().unwrap_or_default();
            Ok(Val::Str(if exists(&n) { format!("{n} is a command") } else { "Unknown".into() }))
        }
        _ => Err(format!("Cannot find procedure \"{name}\".")),
    }
}

fn last_history_name(s: &Scene, id: &str) -> String {
    s.node(id).and_then(|n| n.mesh()).and_then(|m| m.history.last()).filter(|h| !h.name.contains('#')).map(|h| h.name.clone()).unwrap_or_else(|| s.name_of(id))
}

fn apply_op_selection(s: &mut Scene, id: &str, r: &crate::mesh::OpOut) {
    let c = s.comp.entry(id.to_string()).or_default();
    if let Some(f) = &r.select_faces {
        *c = CompSel { f: f.iter().copied().collect(), ..Default::default() };
    } else if let Some(e) = &r.select_edges {
        *c = CompSel { e: e.iter().map(|&(a, b)| crate::mesh::epair(a, b)).collect(), ..Default::default() };
    } else if let Some(v) = &r.select_verts {
        *c = CompSel { v: v.iter().copied().collect(), ..Default::default() };
    }
}

fn simple_obj_op(s: &mut Scene, a: &Args, ty: &str, f: impl Fn(&Args, &mut Params)) -> Result<Val, String> {
    let mut out = vec![];
    for id in mesh_targets(s, a) {
        let mut p = Params::new();
        f(a, &mut p);
        s.apply_op(&id, ty, p, history_on(a));
        s.comp.remove(&id);
        out.push(last_history_name(s, &id));
    }
    if out.is_empty() {
        return Err("Select a polygon object.".into());
    }
    Ok(strs(out))
}

fn simple_face_op(s: &mut Scene, a: &Args, ty: &str, all: bool) -> Result<Val, String> {
    let mut out = vec![];
    for id in mesh_targets(s, a) {
        let faces = op_faces(s, a, &id, false);
        let mut p = Params::new();
        if !faces.is_empty() {
            p.insert("faces".into(), json!(faces));
        } else if !all {
            continue;
        }
        let r = s.apply_op(&id, ty, p, history_on(a));
        apply_op_selection(s, &id, &r);
        out.push(last_history_name(s, &id));
    }
    if out.is_empty() {
        return Err("Select a polygon object or faces.".into());
    }
    Ok(strs(out))
}

fn simple_edge_op(s: &mut Scene, a: &Args, ty: &str, f: impl Fn(&Args, &mut Params)) -> Result<Val, String> {
    let mut out = vec![];
    for id in mesh_targets(s, a) {
        let e = op_edges(s, a, &id);
        let faces = op_faces(s, a, &id, false);
        if e.is_empty() && faces.len() != 2 {
            continue;
        }
        let mut p = Params::new();
        if !e.is_empty() {
            p.insert("edges".into(), crate::params::edges_value(&e));
        } else {
            p.insert("faces".into(), json!(faces));
        }
        f(a, &mut p);
        let r = s.apply_op(&id, ty, p, history_on(a));
        apply_op_selection(s, &id, &r);
        out.push(last_history_name(s, &id));
    }
    if out.is_empty() {
        return Err("Select edges.".into());
    }
    Ok(strs(out))
}

fn delete_components(s: &mut Scene, a: &Args) {
    for id in mesh_targets(s, a) {
        let f = op_faces(s, a, &id, false);
        let e = op_edges(s, a, &id);
        let v = if f.is_empty() && e.is_empty() { s.comp_sel(&id).v.iter().copied().collect::<Vec<_>>() } else { vec![] };
        let mut p = Params::new();
        if !f.is_empty() {
            p.insert("faces".into(), json!(f));
        } else if !e.is_empty() {
            p.insert("edges".into(), crate::params::edges_value(&e));
        } else if !v.is_empty() {
            p.insert("verts".into(), json!(v));
        } else {
            continue;
        }
        s.apply_op(&id, "deleteComponent", p, true);
        s.comp.remove(&id);
    }
}

fn assign_to_selection(s: &mut Scene, mid: &str) {
    let ids = s.sel.clone();
    for id in ids {
        if s.comp_mode == Some(CompMode::Face) && !s.comp_sel(&id).f.is_empty() {
            let f: Vec<usize> = s.comp_sel(&id).f.iter().copied().collect();
            s.assign_material(&id, mid, Some(&f));
        } else {
            for i in s.subtree(&id) {
                if s.node(&i).is_some_and(|n| n.mesh().is_some()) {
                    s.assign_material(&i, mid, None);
                }
            }
        }
    }
}

fn curve_surface(s: &mut Scene, a: &Args, ty: &str) -> Result<Val, String> {
    let curves: Vec<Id> = targets(s, a).into_iter().filter(|i| matches!(s.node(i).map(|n| &n.shape), Some(Shape::Curve(_)))).collect();
    let need = match ty {
        "loft" | "extrude" => 2,
        _ => 1,
    };
    if curves.len() < need {
        return Err(format!("{ty}: select {need} curve(s)"));
    }
    let mut p = Params::new();
    let base = match ty {
        "revolve" => {
            p.insert("curve".into(), json!(curves[0]));
            p.insert("axis".into(), json!(a.f(&["ax", "axis"]).unwrap_or(1.0)));
            p.insert("startSweep".into(), json!(a.f(&["ssw", "startSweep"]).unwrap_or(0.0)));
            p.insert("endSweep".into(), json!(a.f(&["esw", "endSweep"]).unwrap_or(360.0)));
            p.insert("sections".into(), json!(a.f(&["s", "sections"]).unwrap_or(8.0)));
            p.insert("segments".into(), json!(8));
            "revolvedSurface1"
        }
        "loft" => {
            p.insert("curves".into(), json!(curves));
            p.insert("segments".into(), json!(8));
            p.insert("close".into(), json!(a.b(&["c", "close"], false)));
            "loftedSurface1"
        }
        "extrude" => {
            p.insert("profile".into(), json!(curves[0]));
            p.insert("path".into(), json!(curves[1]));
            p.insert("segments".into(), json!(8));
            p.insert("scale".into(), json!(a.f(&["sc", "scale"]).unwrap_or(1.0)));
            p.insert("twist".into(), json!(a.f(&["rn", "rotation"]).unwrap_or(0.0)));
            "extrudedSurface1"
        }
        _ => {
            p.insert("curves".into(), json!(curves));
            p.insert("segments".into(), json!(8));
            "planarTrimmedSurface1"
        }
    };
    let id = s.create_mesh_from(Default::default(), base, None, None);
    if let Some(m) = s.nodes.get_mut(&id).and_then(|n| n.mesh_mut()) {
        m.history.clear();
        m.nurbs = true;
    }
    s.add_history(&id, ty, p);
    if !history_on(a) {
        s.delete_history(&id);
    }
    s.select(std::slice::from_ref(&id), SelMode::Replace);
    Ok(result_names(s, &id))
}

fn select(s: &mut Scene, a: &Args) -> Result<Val, String> {
    if a.has(&["cl", "clear"]) {
        s.select(&[], SelMode::Replace);
        s.comp.clear();
        return Ok(Val::None);
    }
    let mode = if a.has(&["add", "af", "addFirst"]) {
        SelMode::Add
    } else if a.has(&["tgl", "toggle"]) {
        SelMode::Toggle
    } else if a.has(&["d", "deselect"]) {
        SelMode::Remove
    } else {
        SelMode::Replace
    };
    if a.has(&["all"]) {
        let all: Vec<Id> = s.roots.iter().filter(|i| !s.node(i).unwrap().startup()).cloned().collect();
        s.select(&all, SelMode::Replace);
        return Ok(Val::None);
    }
    let mut objs = vec![];
    let mut comps: Vec<(Id, CompMode, Vec<usize>)> = vec![];
    for t in a.strings() {
        if let Some(c) = parse_component(s, &t) {
            comps.push(c);
            continue;
        }
        if t.contains('*') || t.contains('?') {
            let matches: Vec<Id> = s.all_dag(false).into_iter().filter(|i| glob_match(&t, &s.name_of(i))).collect();
            objs.extend(matches);
            continue;
        }
        match s.find(&t) {
            Some(id) => objs.push(id),
            None => return Err(format!("No object matches name: {t}")),
        }
    }
    if !comps.is_empty() {
        if mode == SelMode::Replace {
            s.comp.clear();
        }
        let mut owners = vec![];
        for (id, m, idx) in comps {
            let topo = s.node(&id).unwrap().mesh().unwrap().mesh.topo();
            let c = s.comp.entry(id.clone()).or_default();
            for i in idx {
                match (m, mode) {
                    (CompMode::Vertex, SelMode::Remove) => {
                        c.v.remove(&(i as u32));
                    }
                    (CompMode::Vertex, _) => {
                        c.v.insert(i as u32);
                    }
                    (CompMode::Face, SelMode::Remove) => {
                        c.f.remove(&i);
                    }
                    (CompMode::Face, _) => {
                        c.f.insert(i);
                    }
                    (CompMode::Edge, SelMode::Remove) => {
                        c.e.remove(&topo.edges[i]);
                    }
                    (CompMode::Edge, _) => {
                        c.e.insert(topo.edges[i]);
                    }
                }
            }
            s.comp_mode = Some(m);
            owners.push(id);
        }
        if !owners.iter().all(|o| s.sel.contains(o)) {
            s.select(&owners, SelMode::Add);
        }
        s.rev += 1;
        return Ok(Val::None);
    }
    let objs = if a.has(&["hi", "hierarchy"]) { objs.iter().flat_map(|o| s.subtree(o)).collect() } else { objs };
    s.select(&objs, mode);
    Ok(Val::None)
}

fn ls(s: &Scene, a: &Args) -> Result<Val, String> {
    let long = a.has(&["l", "long"]);
    let nm = |id: &Id| if long { s.full_path(id) } else { s.name_of(id) };
    if a.has(&["mat", "materials"]) {
        return Ok(strs(s.mats.values().map(|m| m.name.clone()).collect()));
    }
    if a.has(&["sl", "selection"]) {
        let mut out = vec![];
        for id in &s.sel {
            if s.comp_mode.is_some() && !s.comp_sel(id).is_empty() {
                out.extend(component_names(s, id));
            } else {
                out.push(nm(id));
            }
        }
        return Ok(strs(out));
    }
    let ty = a.s(&["type", "typ"]);
    let mut out = vec![];
    let filter_names: Vec<String> = a.strings();
    for id in s.all_dag(true) {
        let n = s.node(&id).unwrap();
        let kind_ok = match ty.as_deref() {
            None => true,
            Some("transform") => true,
            Some("mesh") => n.mesh().is_some(),
            Some("light") => matches!(n.shape, Shape::Light(_)),
            Some("camera") => matches!(n.shape, Shape::Camera(_)),
            Some("nurbsCurve") => matches!(n.shape, Shape::Curve(_)),
            Some("joint") => n.kind == "joint",
            Some(t) => matches!(&n.shape, Shape::Light(l) if l.ty == t),
        };
        let kind_ok = kind_ok
            && (!a.has(&["lt", "lights"]) || matches!(n.shape, Shape::Light(_)))
            && (!a.has(&["ca", "cameras"]) || matches!(n.shape, Shape::Camera(_)))
            && (!a.has(&["geometry"]) || n.mesh().is_some() || matches!(n.shape, Shape::Curve(_)))
            && (!a.has(&["v", "visible"]) || s.is_visible(&id));
        let name_ok = filter_names.is_empty() || filter_names.iter().any(|f| glob_match(f, &n.name));
        if kind_ok && name_ok {
            if a.has(&["s", "shapes"]) || ty.as_deref().is_some_and(|t| t != "transform") {
                if let Some(sn) = &n.shape_name {
                    out.push(sn.clone());
                }
            } else {
                out.push(nm(&id));
            }
        }
    }
    if ty.is_none() && !a.has(&["dag", "tr", "transforms", "s", "shapes", "lt", "ca", "geometry", "v"]) && filter_names.is_empty() {
        out.extend(s.mats.values().map(|m| m.name.clone()));
    }
    Ok(strs(out))
}

fn xform(s: &mut Scene, a: &Args) -> Result<Val, String> {
    let ts = targets(s, a);
    let ws = a.has(&["ws", "worldSpace"]);
    if a.has(&["q", "query"]) {
        let id = ts.first().ok_or("xform: nothing selected")?;
        let n = s.node(id).unwrap();
        let v3 = |v: [f64; 3]| Val::Array(v.iter().map(|x| Val::Float(*x)).collect());
        return Ok(if a.has(&["t", "translation"]) {
            if ws {
                v3(s.world_matrix(id).transform_point3(DVec3::ZERO).to_array())
            } else {
                v3(n.xf.t)
            }
        } else if a.has(&["ro", "rotation"]) {
            v3(n.xf.r)
        } else if a.has(&["s", "scale"]) {
            v3(n.xf.s)
        } else if a.has(&["piv", "pivots", "rp", "rotatePivot", "sp", "scalePivot"]) {
            if ws {
                v3(s.world_pivot(id).to_array())
            } else {
                v3(n.xf.p)
            }
        } else if a.has(&["bb", "boundingBox"]) {
            let b = s.world_bbox(id);
            Val::Array([b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z].iter().map(|x| Val::Float(*x)).collect())
        } else if a.has(&["m", "matrix"]) {
            let m = if ws { s.world_matrix(id) } else { s.local_matrix(id) };
            Val::Array(m.to_cols_array().iter().map(|x| Val::Float(*x)).collect())
        } else {
            Val::None
        });
    }
    let rel = a.has(&["r", "relative"]);
    for id in ts {
        if a.has(&["cp", "centerPivots"]) {
            s.center_pivot(&id);
        }
        if let Some(t) = a.v3(&["t", "translation"]) {
            if ws && !rel {
                let wm = s.world_matrix(&id);
                let cur = wm.transform_point3(DVec3::ZERO);
                let d = s.parent_world(&id).inverse().transform_vector3(DVec3::from_array(t) - cur);
                let n = s.nodes.get_mut(&id).unwrap();
                n.xf.t = [n.xf.t[0] + d.x, n.xf.t[1] + d.y, n.xf.t[2] + d.z];
            } else {
                let n = s.nodes.get_mut(&id).unwrap();
                n.xf.t = if rel { [n.xf.t[0] + t[0], n.xf.t[1] + t[1], n.xf.t[2] + t[2]] } else { t };
            }
        }
        if let Some(r) = a.v3(&["ro", "rotation"]) {
            let n = s.nodes.get_mut(&id).unwrap();
            n.xf.r = if rel { [n.xf.r[0] + r[0], n.xf.r[1] + r[1], n.xf.r[2] + r[2]] } else { r };
        }
        if let Some(sc) = a.v3(&["s", "scale"]) {
            let n = s.nodes.get_mut(&id).unwrap();
            n.xf.s = if rel { [n.xf.s[0] * sc[0], n.xf.s[1] * sc[1], n.xf.s[2] * sc[2]] } else { sc };
        }
        if let Some(p) = a.v3(&["piv", "pivots", "rp", "rotatePivot", "sp", "scalePivot"]) {
            if ws {
                s.set_pivot_world(&id, DVec3::from_array(p));
            } else {
                s.set_pivot_local(&id, p);
            }
        }
        if let Some(m) = a.get(&["m", "matrix"]) {
            if m.len() == 16 {
                let arr: Vec<f64> = m.iter().map(|x| x.num()).collect();
                let mm = DMat4::from_cols_slice(&arr);
                if ws {
                    s.set_world_matrix(&id, mm);
                } else {
                    s.set_local_matrix(&id, mm);
                }
            }
        }
        s.touch();
    }
    Ok(Val::None)
}

fn file_cmd(s: &mut Scene, a: &Args) -> Result<Val, String> {
    if a.has(&["q", "query"]) && a.has(&["sn", "sceneName"]) || a.has(&["q", "query"]) {
        return Ok(Val::Str(s.file.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()));
    }
    if a.has(&["new", "newFile"]) {
        if s.modified && !a.has(&["f", "force"]) {
            return Err("Unsaved changes: use file -f -new".into());
        }
        s.reset();
        return Ok(Val::None);
    }
    if let Some(n) = a.s(&["rn", "rename"]) {
        s.file = Some(n.into());
    }
    let path = a.strings().first().cloned().map(std::path::PathBuf::from);
    if a.has(&["o", "open"]) {
        let p = path.ok_or("file -o: path required")?;
        crate::io::open_scene(s, &p)?;
        return Ok(Val::Str(p.to_string_lossy().into()));
    }
    if a.has(&["i", "import"]) {
        let p = path.ok_or("file -i: path required")?;
        let made = crate::io::import(s, &p)?;
        s.select(&made, SelMode::Replace);
        return Ok(strs(made.iter().map(|i| s.name_of(i)).collect()));
    }
    if a.has(&["ea", "exportAll", "es", "exportSelected"]) {
        let p = path.ok_or("file -ea/-es: path required")?;
        crate::io::export(s, &p, a.has(&["es", "exportSelected"]))?;
        return Ok(Val::Str(p.to_string_lossy().into()));
    }
    if a.has(&["s", "save"]) {
        let p = s.file.clone().ok_or("file -s: scene has no name; use file -rename first")?;
        crate::io::save_scene(s, &p)?;
        return Ok(Val::Str(p.to_string_lossy().into()));
    }
    // file -rename on its own just names the scene
    if let Some(p) = &s.file {
        if a.has(&["rn", "rename"]) {
            return Ok(Val::Str(p.to_string_lossy().into()));
        }
    }
    Err("file: nothing to do".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mel::Interp;

    fn mel(s: &mut Scene, src: &str) -> Result<Val, String> {
        Interp::new().run(s, &mut NoHost, src)
    }

    #[test]
    fn components_extrude_and_ls() {
        let mut s = Scene::new();
        mel(&mut s, "polyCube; select -r pCube1.f[1]; polyExtrudeFacet -ltz 2;").unwrap();
        let id = s.find("pCube1").unwrap();
        let m = &s.node(&id).unwrap().mesh().unwrap();
        assert_eq!(m.mesh.f.len(), 10);
        assert_eq!(m.history.len(), 2);
        assert_eq!(m.history[1].name, "polyExtrudeFace1");
        let r = mel(&mut s, "ls -sl;").unwrap();
        assert_eq!(r.list().len(), 1);
        assert!(r.str().starts_with("pCube1.f["));
        // the extrude is live: changing ltz moves the cap
        mel(&mut s, "setAttr polyExtrudeFace1.localTranslateZ 4;").unwrap();
        let b = s.world_bbox(&id);
        assert!((b.max.y - 4.5).abs() < 1e-9, "{:?}", b);
    }

    #[test]
    fn chamfer_connect_collapse_to_curve() {
        let mut s = Scene::new();
        mel(&mut s, "polyCube -n c; selectMode -component; selectType -v 1; select -r c.vtx[0]; polyChamferVtx -w 0.2;").unwrap();
        let id = s.find("c").unwrap();
        assert_eq!(s.node(&id).unwrap().mesh().unwrap().mesh.f.len(), 7);
        mel(&mut s, "polyCube -n d; select -r d.e[0] d.e[2]; polyConnectComponents;").unwrap();
        let d = s.find("d").unwrap();
        let fd = s.node(&d).unwrap().mesh().unwrap().mesh.f.len();
        assert!(fd > 6, "connect split a face ({fd})");
        mel(&mut s, "polyCube -n e; select -r e.e[0]; polyCollapseEdge;").unwrap();
        let e = s.find("e").unwrap();
        assert_eq!(s.node(&e).unwrap().mesh().unwrap().mesh.v.len(), 7);
        let r = mel(&mut s, "polyCube -n g; select -r g.e[0] g.e[1]; polyToCurve -form 2 -degree 1;").unwrap();
        let cv = s.find(&r.str()).unwrap();
        assert!(matches!(&s.node(&cv).unwrap().shape, Shape::Curve(_)));
    }

    #[test]
    fn rig_commands() {
        let mut s = Scene::new();
        mel(&mut s, "select -cl; joint -p 0 0 0 -n a; joint -p 2 0 -0.2 -n b; joint -p 4 0 0 -n c; select -r a; joint -e -oj xyz;").unwrap();
        let r = mel(&mut s, "ikHandle -sj a -ee c;").unwrap();
        let h = s.find(&r.str()).unwrap();
        let hn = s.name_of(&h);
        mel(&mut s, &format!("move 2.5 1.5 0 {hn};")).unwrap();
        s.eval_constraints();
        assert!((s.world_pos(&s.find("c").unwrap()) - DVec3::new(2.5, 1.5, 0.0)).length() < 1e-4);
        // cluster: moving the handle moves the clustered vertex
        mel(&mut s, "polyCube -n box; select -r box.vtx[0]; cluster;").unwrap();
        let bx = s.find("box").unwrap();
        let v0 = s.node(&bx).unwrap().mesh().unwrap().mesh.v[0];
        mel(&mut s, "move -r 0 1 0 cluster1Handle;").unwrap();
        let v1 = s.node(&bx).unwrap().mesh().unwrap().mesh.v[0];
        assert!((v1 - v0 - DVec3::Y).length() < 1e-9, "{v0} {v1}");
        assert_eq!(s.node(&bx).unwrap().mesh().unwrap().mesh.v[1], s.node(&bx).unwrap().mesh().unwrap().history[0].out.as_ref().unwrap().v[1]);
        // blend shape weight on the base
        mel(&mut s, "polyCube -n tgt; move -r 0 0 0 tgt; polyCube -n base;").unwrap();
        let t = s.find("tgt").unwrap();
        let mut pm = s.node(&t).unwrap().mesh().unwrap().mesh.clone();
        pm.v[0].y += 2.0;
        s.bake_mesh(&t, pm);
        mel(&mut s, "select -r tgt base; blendShape; setAttr base.tgt 0.5;").unwrap();
        let b = s.find("base").unwrap();
        let base0 = s.node(&b).unwrap().mesh().unwrap().history[0].out.as_ref().unwrap().v[0];
        assert!((s.node(&b).unwrap().mesh().unwrap().mesh.v[0].y - base0.y - 1.0).abs() < 1e-9);
        // lattice: identity until points move
        mel(&mut s, "polySphere -n ball; select -r ball; lattice -dv 2 3 2;").unwrap();
        let ball = s.find("ball").unwrap();
        let before = s.node(&ball).unwrap().mesh().unwrap().history[0].out.as_ref().unwrap().v.clone();
        let after = &s.node(&ball).unwrap().mesh().unwrap().mesh.v;
        assert!(before.iter().zip(after).all(|(a, b)| (*a - *b).length() < 1e-9));
        // delete deformers removes the history and the handle
        mel(&mut s, "select -r box; deleteDeformers;").unwrap();
        assert!(s.find("cluster1Handle").is_none());
    }

    #[test]
    fn open_close_curve() {
        let mut s = Scene::new();
        mel(&mut s, "curve -n c -d 3 -p 0 0 0 -p 1 0 0 -p 1 0 1 -p 0 0 1; closeCurve c;").unwrap();
        let id = s.find("c").unwrap();
        assert!(matches!(&s.node(&id).unwrap().shape, Shape::Curve(c) if c.form == "periodic"));
        mel(&mut s, "openCurve c;").unwrap();
        assert!(matches!(&s.node(&id).unwrap().shape, Shape::Curve(c) if c.form == "open"));
    }

    #[test]
    fn match_transform_and_add_attr() {
        let mut s = Scene::new();
        mel(&mut s, "polyCube -n a; polyCube -n b; move 3 4 5 b; rotate 0 30 0 b; select -r a b; matchTransform -pos;").unwrap();
        let a = s.find("a").unwrap();
        assert_eq!(s.node(&a).unwrap().xf.t, [3.0, 4.0, 5.0]);
        assert_eq!(s.node(&a).unwrap().xf.r, [0.0, 0.0, 0.0]);
        mel(&mut s, "select -r a b; matchTransform -rot;").unwrap();
        assert_eq!(s.node(&a).unwrap().xf.r, [0.0, 30.0, 0.0]);
        // parented: the world position still matches
        mel(&mut s, "group -n g a; move 1 0 0 g; select -r a b; matchTransform;").unwrap();
        assert!((s.world_pivot(&a) - s.world_pivot(&s.find("b").unwrap())).length() < 1e-9);
        mel(&mut s, "addAttr -ln speed -at double -dv 2.5 b;").unwrap();
        assert_eq!(mel(&mut s, "getAttr b.speed;").unwrap().num(), 2.5);
        mel(&mut s, "setAttr b.speed 4;").unwrap();
        assert_eq!(mel(&mut s, "getAttr b.speed;").unwrap().num(), 4.0);
        mel(&mut s, "deleteAttr b.speed;").unwrap();
        assert!(mel(&mut s, "getAttr b.speed;").is_err());
    }

    #[test]
    fn transform_commands() {
        let mut s = Scene::new();
        mel(&mut s, "polySphere -r 2; move 1 2 3; rotate -r 0 45 0; scale 2 2 2;").unwrap();
        let id = s.find("pSphere1").unwrap();
        assert_eq!(s.node(&id).unwrap().xf.t, [1.0, 2.0, 3.0]);
        assert_eq!(s.node(&id).unwrap().xf.r, [0.0, 45.0, 0.0]);
        let r = mel(&mut s, "xform -q -ws -t pSphere1;").unwrap();
        assert_eq!(r.list().len(), 3);
        mel(&mut s, "group -n grp pSphere1; move -r 10 0 0 grp; parent -w pSphere1;").unwrap();
        let w = s.world_matrix(&id).transform_point3(DVec3::ZERO);
        assert!((w.x - 11.0).abs() < 1e-6);
    }

    #[test]
    fn booleans_and_materials() {
        let mut s = Scene::new();
        mel(&mut s, "polyCube -w 2 -h 2 -d 2; polyCube -w 2 -h 2 -d 2; move 1 1 1; select pCube1 pCube2; polyCBoolOp -op 2;").unwrap();
        let o = s.lead().unwrap().clone();
        let v = s.node(&o).unwrap().mesh().unwrap().mesh.signed_volume();
        assert!((v - 7.0).abs() < 1e-6, "{v}");
        // moving an input re-evaluates the boolean
        mel(&mut s, "move 3 0 0 pCube2;").unwrap();
        let v = s.node(&o).unwrap().mesh().unwrap().mesh.signed_volume();
        assert!((v - 8.0).abs() < 1e-6, "{v}");
        mel(&mut s, "string $m = `shadingNode -asShader blinn`; select pCube1; hyperShade -assign $m;").unwrap();
        let c = s.find("pCube1").unwrap();
        assert_eq!(s.mats[&s.node(&c).unwrap().mesh().unwrap().material].name, "blinn1");
    }

    #[test]
    fn bevel_command_and_history() {
        let mut s = Scene::new();
        mel(&mut s, "polyCube; polyBevel -f 0.3;").unwrap();
        let id = s.find("pCube1").unwrap();
        assert_eq!(s.node(&id).unwrap().mesh().unwrap().mesh.f.len(), 26);
        mel(&mut s, "setAttr polyBevel1.fraction 0.8;").unwrap();
        assert_eq!(s.node(&id).unwrap().mesh().unwrap().mesh.f.len(), 26);
        mel(&mut s, "select -r pCube1.e[0]; polyBevel;").unwrap();
    }

    #[test]
    fn nonlinear_deformer_follows_handle() {
        let mut s = Scene::new();
        mel(&mut s, "polyCylinder -h 4 -sh 8; nonLinear -type twist -endAngle 90;").unwrap();
        let c = s.find("pCylinder1").unwrap();
        assert_eq!(s.node(&c).unwrap().mesh().unwrap().history.last().unwrap().name, "twist1");
        let ti = s.node(&c).unwrap().mesh().unwrap().mesh.v.iter().position(|v| v.y > 1.99).unwrap();
        let top = |s: &Scene| s.node(&c).unwrap().mesh().unwrap().mesh.v[ti];
        let before = top(&s);
        mel(&mut s, "setAttr twist1.endAngle 0;").unwrap();
        let after = top(&s);
        assert!((before - after).length() > 0.1);
        // moving the handle re-evaluates the deformation
        mel(&mut s, "setAttr twist1.endAngle 90; move -r 0 10 0 twist1Handle;").unwrap();
        let moved = top(&s);
        assert!((moved - before).length() > 0.01);
    }

    #[test]
    fn joints_skin_and_constraints() {
        let mut s = Scene::new();
        mel(&mut s, "polyCylinder -h 4 -sh 8; move -r 0 2 0; select -cl; joint -p 0 0 0; joint -p 0 2 0; joint -p 0 4 0;").unwrap();
        let j2 = s.find("joint2").unwrap();
        assert_eq!(s.node(&j2).unwrap().parent.as_deref(), s.find("joint1").as_deref());
        mel(&mut s, "select -r joint1 pCylinder1; skinCluster -mi 2;").unwrap();
        let c = s.find("pCylinder1").unwrap();
        let tip = s.node(&c).unwrap().mesh().unwrap().mesh.v.iter().position(|v| v.y > 1.99).unwrap();
        let before = s.mesh_world(&c).unwrap().v[tip];
        mel(&mut s, "setAttr joint2.rotateZ 90;").unwrap();
        let after = s.mesh_world(&c).unwrap().v[tip];
        assert!(after.x < before.x - 1.0, "{before} -> {after}");
        // a point constraint follows its target and survives save/load
        mel(&mut s, "spaceLocator; move 5 1 0; polySphere -n ball; select -r locator1 ball; pointConstraint;").unwrap();
        let b = s.find("ball").unwrap();
        assert!((s.world_pivot(&b) - DVec3::new(5.0, 1.0, 0.0)).length() < 1e-9);
        mel(&mut s, "move 2 2 2 locator1; currentTime 2;").unwrap();
        assert!((s.world_pivot(&b) - DVec3::new(2.0, 2.0, 2.0)).length() < 1e-9);
        let data = s.serialize();
        let mut s2 = Scene::new();
        s2.load(&data).unwrap();
        assert_eq!(s2.constraints.len(), 1);
    }

    #[test]
    fn keys_and_time() {
        let mut s = Scene::new();
        mel(&mut s, "polyCube; setKeyframe -at tx -t 1 -v 0; setKeyframe -at tx -t 10 -v 9; currentTime 10;").unwrap();
        let id = s.find("pCube1").unwrap();
        assert!((s.get_attr_f(&id, "tx") - 9.0).abs() < 1e-9);
        let r = mel(&mut s, "keyframe -q -tc -at tx pCube1;").unwrap();
        assert_eq!(r.list().len(), 2);
    }
}

#[cfg(test)]
mod more_tests {
    use super::*;
    use crate::mel::Interp;

    #[test]
    fn curve_points_are_not_a_position() {
        let mut s = Scene::new();
        Interp::new().run(&mut s, &mut NoHost, "curve -d 1 -p 0 0 0 -p 5 0 0; curve -arc -p -2 0 3 -p 0 1.5 3 -p 2 0 3; polyCreateFacet -p 0 0 0 -p 1 0 0 -p 1 0 1;").unwrap();
        let c = s.find("curve1").unwrap();
        assert_eq!(s.node(&c).unwrap().xf.t, [0.0; 3]);
        let a = s.find("curve2").unwrap();
        let b = s.world_bbox(&a);
        assert!(b.max.y > 1.4 && b.max.y <= 1.5 + 1e-9 && (b.min.x + 2.0).abs() < 1e-6, "{b:?}");
        let f = s.find("polySurface1").unwrap();
        let fb = s.world_bbox(&f);
        assert!((fb.min.x).abs() < 1e-9 && (fb.max.x - 1.0).abs() < 1e-9);
        assert!(s.node(&f).unwrap().mesh().unwrap().mesh.face_normal(0).y > 0.0);
    }

    #[test]
    fn create_then_move_selection() {
        let mut s = Scene::new();
        Interp::new().run(&mut s, &mut NoHost, "camera; move -r 6 3 6; spotLight; move -r 1 2 3;").unwrap();
        let c = s.find("camera1").unwrap();
        assert_eq!(s.node(&c).unwrap().xf.t, [6.0, 3.0, 6.0]);
        let l = s.find("spotLight1").unwrap();
        assert_eq!(s.node(&l).unwrap().xf.t, [1.0, 2.0, 3.0]);
    }
}
