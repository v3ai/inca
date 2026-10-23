//! Construction-history node types. Each history entry: `{ id, name, type, params }`;
//! evaluating a mesh's history runs these in order.

use crate::curves::{self, Curve};
use crate::math::DVec3;
use crate::mesh::{self, OpOut, PolyMesh};
use crate::params::*;
use crate::primitives::{self, ch, ChanKind, Channel};

/// Things an op may need from the scene (input curves / meshes in world space).
pub trait OpCtx {
    fn world_curve(&self, id: &str) -> Option<Curve>;
    fn world_mesh(&self, id: &str) -> Option<PolyMesh>;
    fn world_matrix(&self, _id: &str) -> Option<crate::math::DMat4> {
        None
    }
}

pub struct NoCtx;
impl OpCtx for NoCtx {
    fn world_curve(&self, _: &str) -> Option<Curve> {
        None
    }
    fn world_mesh(&self, _: &str) -> Option<PolyMesh> {
        None
    }
}

pub struct OpInfo {
    pub ty: &'static str,
    /// base for the history node name (polyExtrudeFace1, ...)
    pub base: &'static str,
    pub hidden: bool,
    pub channels: &'static [Channel],
}

const F: ChanKind = ChanKind::Float;
const I: ChanKind = ChanKind::Int;
const B: ChanKind = ChanKind::Bool;
const AXES: &[&str] = &["X", "Y", "Z"];
pub const BOOL_OPS: &[&str] = &["Union", "Difference", "Intersection"];

pub static OPS: &[OpInfo] = &[
    OpInfo { ty: "meshData", base: "meshData", hidden: true, channels: &[] },
    OpInfo { ty: "polyTweak", base: "polyTweak", hidden: true, channels: &[] },
    OpInfo {
        ty: "polyExtrudeFace",
        base: "polyExtrudeFace",
        hidden: false,
        channels: &[ch("translateX", F, None, None), ch("translateY", F, None, None), ch("translateZ", F, None, None), ch("localTranslateZ", F, None, None), ch("offset", F, Some(-1.0), Some(1.0)), ch("divisions", I, Some(1.0), Some(50.0)), ch("keepFacesTogether", B, None, None)],
    },
    OpInfo { ty: "polyExtrudeEdge", base: "polyExtrudeEdge", hidden: false, channels: &[ch("offset", F, None, None), ch("translateY", F, None, None)] },
    OpInfo { ty: "polyBevel", base: "polyBevel", hidden: false, channels: &[ch("fraction", F, Some(0.001), Some(1.0))] },
    OpInfo { ty: "skinCluster", base: "skinCluster", hidden: false, channels: &[] },
    OpInfo { ty: "polySplitRing", base: "polySplitRing", hidden: false, channels: &[Channel { key: "t", kind: F, min: Some(0.001), max: Some(0.999), label: Some("Weight") }, Channel { key: "multiple", kind: I, min: Some(1.0), max: Some(50.0), label: Some("Number Of Edge Loops") }] },
    OpInfo { ty: "deleteComponent", base: "deleteComponent", hidden: false, channels: &[] },
    OpInfo { ty: "polyMergeVert", base: "polyMergeVert", hidden: false, channels: &[ch("threshold", F, Some(0.0), None)] },
    OpInfo { ty: "polyMergeCenter", base: "polyMergeVert", hidden: false, channels: &[] },
    OpInfo { ty: "polySmoothFace", base: "polySmoothFace", hidden: false, channels: &[ch("divisions", I, Some(0.0), Some(4.0))] },
    OpInfo { ty: "polySubdFace", base: "polySubdFace", hidden: false, channels: &[ch("divisions", I, Some(1.0), Some(4.0))] },
    OpInfo { ty: "polyTriangulate", base: "polyTriangulate", hidden: false, channels: &[] },
    OpInfo { ty: "polyQuad", base: "polyQuad", hidden: false, channels: &[ch("angle", F, Some(0.0), Some(180.0))] },
    OpInfo { ty: "polyNormal", base: "polyNormal", hidden: false, channels: &[] },
    OpInfo { ty: "polyPoke", base: "polyPoke", hidden: false, channels: &[] },
    OpInfo { ty: "polyCloseBorder", base: "polyCloseBorder", hidden: false, channels: &[] },
    OpInfo { ty: "polyMirror", base: "polyMirror", hidden: false, channels: &[ch("axis", ChanKind::Enum(AXES), None, None), ch("position", F, None, None), ch("merge", B, None, None), ch("threshold", F, Some(0.0), None)] },
    OpInfo { ty: "polyBridgeEdge", base: "polyBridgeEdge", hidden: false, channels: &[ch("divisions", I, Some(0.0), Some(100.0))] },
    OpInfo { ty: "polySoftEdge", base: "polySoftEdge", hidden: false, channels: &[ch("angle", F, Some(0.0), Some(180.0))] },
    OpInfo { ty: "polyProj", base: "polyProj", hidden: false, channels: &[ch("mode", ChanKind::Enum(&["planar", "cylindrical", "spherical", "automatic"]), None, None), ch("axis", ChanKind::Enum(AXES), None, None)] },
    OpInfo { ty: "polyCBoolOp", base: "polyCBoolOp", hidden: false, channels: &[ch("operation", ChanKind::Enum(BOOL_OPS), None, None)] },
    OpInfo { ty: "revolve", base: "revolve", hidden: false, channels: &[ch("axis", ChanKind::Enum(AXES), None, None), ch("startSweep", F, None, None), ch("endSweep", F, None, None), ch("sections", I, Some(1.0), Some(50.0)), Channel { key: "segments", kind: I, min: Some(1.0), max: Some(50.0), label: Some("Curve Segments") }] },
    OpInfo { ty: "loft", base: "loft", hidden: false, channels: &[ch("segments", I, Some(1.0), Some(50.0)), ch("close", B, None, None)] },
    OpInfo { ty: "extrude", base: "extrude", hidden: false, channels: &[ch("segments", I, Some(1.0), Some(50.0)), ch("scale", F, Some(0.0), None), ch("twist", F, None, None)] },
    OpInfo { ty: "planarTrim", base: "planarTrimSurface", hidden: false, channels: &[ch("segments", I, Some(1.0), Some(50.0))] },
];

/// Op description (history types and primitive generators).
pub fn info(ty: &str) -> Option<(&'static str, bool, &'static [Channel])> {
    if let Some(k) = crate::deformers::KINDS.iter().find(|k| **k == ty) {
        return Some((k, false, crate::deformers::channels(k)));
    }
    if let Some(o) = OPS.iter().find(|o| o.ty == ty) {
        return Some((o.base, o.hidden, o.channels));
    }
    primitives::gen_info(ty).map(|g| (if g.nurbs { nurbs_base(g.ty) } else { g.ty }, false, g.channels))
}

fn nurbs_base(ty: &str) -> &'static str {
    // history node names as in Maya: makeNurbSphere1 (not nurbsSphere2, which reads as a second object)
    match ty {
        "nurbsSphere" => "makeNurbSphere",
        "nurbsCube" => "makeNurbCube",
        "nurbsCylinder" => "makeNurbCylinder",
        "nurbsCone" => "makeNurbCone",
        "nurbsPlane" => "makeNurbPlane",
        "nurbsTorus" => "makeNurbTorus",
        _ => "makeNurb",
    }
}

pub fn is_generator(ty: &str) -> bool {
    primitives::gen_info(ty).is_some() || matches!(ty, "revolve" | "loft" | "extrude" | "planarTrim" | "polyCBoolOp")
}

pub fn nice_name(k: &str) -> String {
    if k == "t" {
        return "Weight".into();
    }
    let mut s = String::new();
    for (i, c) in k.chars().enumerate() {
        if i == 0 {
            s.extend(c.to_uppercase());
        } else if c.is_uppercase() {
            s.push(' ');
            s.push(c);
        } else {
            s.push(c);
        }
    }
    s
}

/// Apply one history op to a mesh.
pub fn apply(ty: &str, pm: &PolyMesh, p: &Params, ctx: &dyn OpCtx) -> Result<OpOut, String> {
    if primitives::gen_info(ty).is_some() {
        return primitives::generate(ty, p).map(Into::into).ok_or_else(|| format!("unknown generator {ty}"));
    }
    let faces = || usize_list(p, "faces");
    let edges = || edge_list(p, "edges");
    let verts = || u32_list(p, "verts");
    Ok(match ty {
        "meshData" => {
            let d = p.get("data").ok_or("meshData without data")?;
            serde_json::from_value::<PolyMesh>(d.clone()).map_err(|e| e.to_string())?.into()
        }
        "polyTweak" => mesh::op_tweak(pm, &verts().unwrap_or_default(), &vec3_list(p, "deltas").unwrap_or_default()).into(),
        "polyExtrudeFace" => mesh::op_extrude_faces(
            pm,
            &faces().unwrap_or_default(),
            &mesh::ExtrudeParams {
                local_translate_z: num_or(p, "localTranslateZ", 0.0) + num_or(p, "thickness", 0.0),
                translate: DVec3::new(num_or(p, "translateX", 0.0), num_or(p, "translateY", 0.0), num_or(p, "translateZ", 0.0)),
                offset: num_or(p, "offset", 0.0),
                divisions: num_or(p, "divisions", 1.0).round().max(1.0) as u32,
                keep_faces_together: bool_or(p, "keepFacesTogether", true),
            },
        ),
        "polyExtrudeEdge" => mesh::op_extrude_edges(pm, &edges().unwrap_or_default(), num_or(p, "offset", 0.0), DVec3::new(0.0, num_or(p, "translateY", 0.0), 0.0)),
        "skinCluster" => crate::skin::apply(pm, p, ctx)?.into(),
        "polyBevel" => {
            let es = match edges() {
                Some(e) => e,
                None => pm.topo().edges,
            };
            mesh::op_bevel(pm, &es, num_or(p, "fraction", 0.5))
        }
        "polySplitRing" => {
            let e = p.get("edge").and_then(|e| e.as_array()).ok_or("polySplitRing needs an edge")?;
            let a = e.first().and_then(|x| x.as_u64()).ok_or("bad edge")? as u32;
            let b = e.get(1).and_then(|x| x.as_u64()).ok_or("bad edge")? as u32;
            mesh::op_insert_edge_loop(pm, a, b, num_or(p, "t", 0.5), num_or(p, "multiple", 1.0) as u32)
        }
        "deleteComponent" => {
            if let Some(f) = faces() {
                mesh::op_delete_faces(pm, &f).into()
            } else if let Some(e) = edges() {
                mesh::op_delete_edges(pm, &e).into()
            } else {
                mesh::op_delete_vertices(pm, &verts().unwrap_or_default()).into()
            }
        }
        "polyMergeVert" => mesh::op_merge_vertices(pm, verts().as_deref(), num_or(p, "threshold", 0.001)).into(),
        "polyMergeCenter" => {
            let groups: Vec<Vec<u32>> = p
                .get("groups")
                .and_then(|g| g.as_array())
                .map(|g| g.iter().filter_map(|x| x.as_array().map(|a| a.iter().filter_map(|v| v.as_u64().map(|v| v as u32)).collect())).collect())
                .unwrap_or_default();
            mesh::op_merge_to_center(pm, &groups).into()
        }
        "polySmoothFace" => mesh::subdivide(pm, num_or(p, "divisions", 1.0).round().clamp(0.0, 4.0) as u32, true).into(),
        "polySubdFace" => mesh::subdivide(pm, num_or(p, "divisions", 1.0).round().clamp(1.0, 4.0) as u32, false).into(),
        "polyTriangulate" => mesh::op_triangulate(pm, faces().as_deref()).into(),
        "polyQuad" => mesh::op_quadrangulate(pm, num_or(p, "angle", 30.0)).into(),
        "polyNormal" => mesh::op_reverse(pm, faces().as_deref()).into(),
        "polyPoke" => mesh::op_poke(pm, &faces().unwrap_or_else(|| (0..pm.f.len()).collect())),
        "polyCloseBorder" => mesh::op_fill_hole(pm, edges().as_deref()).into(),
        "polyMirror" => mesh::op_mirror(pm, num_or(p, "axis", 0.0) as usize, num_or(p, "position", 0.0), bool_or(p, "merge", true), num_or(p, "threshold", 0.001)).into(),
        "polyBridgeEdge" => mesh::op_bridge(pm, faces().as_deref(), edges().as_deref(), num_or(p, "divisions", 0.0) as u32).into(),
        "polySoftEdge" => mesh::op_soft_edge(pm, num_or(p, "angle", 180.0), edges().as_deref()).into(),
        "polyProj" => {
            let mode = match p.get("mode") {
                Some(serde_json::Value::Number(n)) => ["planar", "cylindrical", "spherical", "automatic"].get(n.as_u64().unwrap_or(0) as usize).copied().unwrap_or("planar"),
                _ => str_or(p, "mode", "planar"),
            };
            mesh::op_project_uv(pm, mode, num_or(p, "axis", 1.0) as usize, faces().as_deref()).into()
        }
        "polyCBoolOp" => {
            let a = ctx.world_mesh(str_or(p, "a", ""));
            let b = ctx.world_mesh(str_or(p, "b", ""));
            match (a, b) {
                (Some(a), Some(b)) => crate::booleans::boolean(&a, &b, num_or(p, "operation", 0.0) as usize)?.into(),
                _ => return Err("boolean input missing".into()),
            }
        }
        "revolve" => {
            let c = curve_param(p, "curve", ctx).ok_or("revolve: missing curve")?;
            curves::revolve(&c, num_or(p, "axis", 1.0) as usize, num_or(p, "startSweep", 0.0), num_or(p, "endSweep", 360.0), num_or(p, "sections", 8.0) as usize, num_or(p, "segments", 8.0) as usize).into()
        }
        "loft" => {
            let cs: Vec<Curve> = ids(p, "curves").iter().filter_map(|id| ctx.world_curve(id)).collect();
            if cs.len() < 2 {
                return Err("loft needs two curves".into());
            }
            curves::loft(&cs, num_or(p, "segments", 8.0) as usize, bool_or(p, "close", false)).into()
        }
        "extrude" => {
            let a = ctx.world_curve(str_or(p, "profile", "")).ok_or("extrude: missing profile")?;
            let b = ctx.world_curve(str_or(p, "path", "")).ok_or("extrude: missing path")?;
            curves::extrude_along(&a, &b, num_or(p, "segments", 8.0) as usize, num_or(p, "scale", 1.0), num_or(p, "twist", 0.0)).into()
        }
        "planarTrim" => {
            let cs: Vec<Curve> = ids(p, "curves").iter().filter_map(|id| ctx.world_curve(id)).collect();
            curves::planar(&cs, num_or(p, "segments", 8.0) as usize).into()
        }
        k if crate::deformers::KINDS.contains(&k) => {
            let mut m = pm.clone();
            let mw = ctx.world_matrix(str_or(p, "owner", "")).unwrap_or(crate::math::DMat4::IDENTITY);
            let hw = ctx.world_matrix(str_or(p, "handle", "")).ok_or("deformer handle missing")?;
            crate::deformers::apply(k, &mut m.v, mw, hw, p);
            m.into()
        }
        _ => return Err(format!("unknown op {ty}")),
    })
}

fn ids(p: &Params, k: &str) -> Vec<String> {
    p.get(k).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}

fn curve_param(p: &Params, k: &str, ctx: &dyn OpCtx) -> Option<Curve> {
    ctx.world_curve(str_or(p, k, "")).or_else(|| p.get("curveData").and_then(|d| serde_json::from_value(d.clone()).ok()))
}
