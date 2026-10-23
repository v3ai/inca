//! Smooth skinning (linear blend) as a `skinCluster` history op.

use crate::math::{DMat4, DVec3};
use crate::mesh::PolyMesh;
use crate::ops::OpCtx;
use crate::params::{str_or, Params};
use crate::scene::{Id, Scene};
use serde_json::{json, Value};

/// Closest distance from p to segment ab.
fn seg_dist(p: DVec3, a: DVec3, b: DVec3) -> f64 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
    (a + ab * t).distance(p)
}

/// Bind parameters for a mesh: joint ids, inverse bind matrices and per-vertex weights
/// (up to `max_influences` per vertex, falling off with distance to each joint's bones).
pub fn bind(s: &Scene, mesh: &str, joints: &[Id], max_influences: usize) -> Params {
    let pm = &s.node(mesh).unwrap().mesh().unwrap().mesh;
    let mw = s.world_matrix(mesh);
    let jpos: Vec<DVec3> = joints.iter().map(|j| s.world_matrix(j).transform_point3(DVec3::ZERO)).collect();
    // bones: segments from each joint to its child joints (a leaf is a point)
    let bones: Vec<Vec<(DVec3, DVec3)>> = joints
        .iter()
        .enumerate()
        .map(|(i, j)| {
            let kids: Vec<DVec3> = s.children(Some(j)).iter().filter_map(|c| joints.iter().position(|x| x == c)).map(|k| jpos[k]).collect();
            if kids.is_empty() {
                vec![(jpos[i], jpos[i])]
            } else {
                kids.into_iter().map(|k| (jpos[i], k)).collect()
            }
        })
        .collect();
    let weights: Vec<Value> = pm
        .v
        .iter()
        .map(|v| {
            let p = mw.transform_point3(*v);
            let mut w: Vec<(usize, f64)> = bones.iter().enumerate().map(|(i, b)| (i, 1.0 / b.iter().map(|(a, c)| seg_dist(p, *a, *c)).fold(f64::INFINITY, f64::min).max(1e-4).powi(4))).collect();
            w.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            w.truncate(max_influences.max(1));
            let sum: f64 = w.iter().map(|x| x.1).sum();
            json!(w.iter().map(|(i, x)| json!([i, (x / sum * 1e4).round() / 1e4])).collect::<Vec<_>>())
        })
        .collect();
    let bind_inv: Vec<Value> = joints.iter().map(|j| json!(s.world_matrix(j).inverse().to_cols_array())).collect();
    let mut p = Params::new();
    p.insert("joints".into(), json!(joints));
    p.insert("bindInv".into(), Value::Array(bind_inv));
    p.insert("meshBind".into(), json!(mw.to_cols_array()));
    p.insert("weights".into(), Value::Array(weights));
    p.insert("owner".into(), json!(mesh));
    p
}

fn mat(v: &Value) -> Option<DMat4> {
    let a: Vec<f64> = v.as_array()?.iter().filter_map(|x| x.as_f64()).collect();
    (a.len() == 16).then(|| DMat4::from_cols_slice(&a))
}

/// Deform the mesh by the joints' current transforms.
pub fn apply(pm: &PolyMesh, p: &Params, ctx: &dyn OpCtx) -> Result<PolyMesh, String> {
    let joints: Vec<String> = p.get("joints").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
    let bind_inv: Vec<DMat4> = p.get("bindInv").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(mat).collect()).unwrap_or_default();
    let mesh_bind = p.get("meshBind").and_then(mat).unwrap_or(DMat4::IDENTITY);
    let mw = ctx.world_matrix(str_or(p, "owner", "")).unwrap_or(mesh_bind);
    if bind_inv.len() != joints.len() {
        return Err("skinCluster: bind data mismatch".into());
    }
    // skinning matrices (world): J_now * J_bind^-1
    let skin: Vec<DMat4> = joints.iter().zip(&bind_inv).map(|(j, b)| ctx.world_matrix(j).map(|m| m * *b).unwrap_or(DMat4::IDENTITY)).collect();
    let inv_mw = mw.inverse();
    let weights = p.get("weights").and_then(|v| v.as_array()).ok_or("skinCluster: no weights")?;
    let mut out = pm.clone();
    for (vi, v) in out.v.iter_mut().enumerate() {
        let Some(w) = weights.get(vi).and_then(|w| w.as_array()) else { continue };
        let wp = mesh_bind.transform_point3(*v);
        let mut acc = DVec3::ZERO;
        let mut tot = 0.0;
        for e in w {
            let (Some(j), Some(x)) = (e.get(0).and_then(|j| j.as_u64()), e.get(1).and_then(|x| x.as_f64())) else { continue };
            if let Some(m) = skin.get(j as usize) {
                acc += m.transform_point3(wp) * x;
                tot += x;
            }
        }
        if tot > 0.0 {
            *v = inv_mw.transform_point3(acc / tot);
        }
    }
    Ok(out)
}
