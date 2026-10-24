//! Nonlinear deformers (bend / flare / sine / squash / twist / wave). Each one is a history op on
//! the deformed mesh whose space is a handle node, so moving the handle or editing the deformer's
//! channels re-evaluates the mesh live.

use crate::math::{DMat4, DVec3};
use crate::params::{num_or, Params};
use crate::primitives::{ch, ChanKind, Channel};

const F: ChanKind = ChanKind::Float;

pub const KINDS: &[&str] = &["bend", "flare", "sine", "squash", "twist", "wave"];

static BEND: &[Channel] = &[ch("envelope", F, Some(0.0), Some(1.0)), ch("curvature", F, None, None), ch("lowBound", F, None, Some(0.0)), ch("highBound", F, Some(0.0), None)];
static FLARE: &[Channel] = &[ch("envelope", F, Some(0.0), Some(1.0)), ch("startFlareX", F, None, None), ch("startFlareZ", F, None, None), ch("endFlareX", F, None, None), ch("endFlareZ", F, None, None), ch("curve", F, None, None), ch("lowBound", F, None, Some(0.0)), ch("highBound", F, Some(0.0), None)];
static SINE: &[Channel] = &[ch("envelope", F, Some(0.0), Some(1.0)), ch("amplitude", F, None, None), ch("wavelength", F, Some(0.01), None), ch("offset", F, None, None), ch("dropoff", F, Some(-1.0), Some(1.0)), ch("lowBound", F, None, Some(0.0)), ch("highBound", F, Some(0.0), None)];
static SQUASH: &[Channel] = &[ch("envelope", F, Some(0.0), Some(1.0)), ch("factor", F, None, None), ch("expand", F, None, None), ch("maxExpandPos", F, Some(0.01), Some(0.99)), ch("lowBound", F, None, Some(0.0)), ch("highBound", F, Some(0.0), None)];
static TWIST: &[Channel] = &[ch("envelope", F, Some(0.0), Some(1.0)), ch("startAngle", F, None, None), ch("endAngle", F, None, None), ch("lowBound", F, None, Some(0.0)), ch("highBound", F, Some(0.0), None)];
static WAVE: &[Channel] = &[ch("envelope", F, Some(0.0), Some(1.0)), ch("amplitude", F, None, None), ch("wavelength", F, Some(0.01), None), ch("offset", F, None, None), ch("dropoff", F, Some(0.0), Some(1.0)), ch("minRadius", F, Some(0.0), None), ch("maxRadius", F, Some(0.0), None)];

pub fn channels(kind: &str) -> &'static [Channel] {
    match kind {
        "bend" => BEND,
        "flare" => FLARE,
        "sine" => SINE,
        "squash" => SQUASH,
        "twist" => TWIST,
        "wave" => WAVE,
        _ => &[],
    }
}

pub fn defaults(kind: &str) -> Params {
    let v = match kind {
        "bend" => serde_json::json!({ "envelope": 1, "curvature": 0, "lowBound": -1, "highBound": 1 }),
        "flare" => serde_json::json!({ "envelope": 1, "startFlareX": 1, "startFlareZ": 1, "endFlareX": 1, "endFlareZ": 1, "curve": 0, "lowBound": -1, "highBound": 1 }),
        "sine" => serde_json::json!({ "envelope": 1, "amplitude": 0, "wavelength": 2, "offset": 0, "dropoff": 0, "lowBound": -1, "highBound": 1 }),
        "squash" => serde_json::json!({ "envelope": 1, "factor": 0, "expand": 1, "maxExpandPos": 0.5, "lowBound": -1, "highBound": 1 }),
        "twist" => serde_json::json!({ "envelope": 1, "startAngle": 0, "endAngle": 0, "lowBound": -1, "highBound": 1 }),
        "wave" => serde_json::json!({ "envelope": 1, "amplitude": 0, "wavelength": 1, "offset": 0, "dropoff": 0, "minRadius": 0, "maxRadius": 1 }),
        _ => serde_json::json!({}),
    };
    v.as_object().cloned().unwrap_or_default()
}

/// Deform one point in handle space (the handle's Y axis is the deformer axis).
pub fn deform(kind: &str, q: DVec3, a: &Params) -> DVec3 {
    let g = |k: &str, d: f64| num_or(a, k, d);
    let (lo, hi) = (g("lowBound", -1.0), g("highBound", 1.0));
    let span = (hi - lo).max(1e-6);
    let mut q = q;
    match kind {
        "bend" => {
            let k = g("curvature", 0.0);
            if k.abs() < 1e-6 {
                return q;
            }
            let yc = q.y.clamp(lo, hi);
            let th = k * yc;
            let r = 1.0 / k;
            let (c, s) = (th.cos(), th.sin());
            let ex = q.y - yc;
            // bend in the XY plane around a centre at x = r
            let x = r - (r - q.x) * c;
            let y = (r - q.x) * s;
            q = DVec3::new(x - ex * s, y + ex * c, q.z);
        }
        "flare" => {
            let t = ((q.y - lo) / span).clamp(0.0, 1.0);
            let b = g("curve", 0.0) * (std::f64::consts::PI * t).sin();
            q.x *= g("startFlareX", 1.0) + (g("endFlareX", 1.0) - g("startFlareX", 1.0)) * t + b;
            q.z *= g("startFlareZ", 1.0) + (g("endFlareZ", 1.0) - g("startFlareZ", 1.0)) * t + b;
        }
        "sine" => {
            let yc = q.y.clamp(lo, hi);
            let m = lo.abs().max(hi.abs()).max(1e-6);
            let dr = g("dropoff", 0.0);
            let d = if dr >= 0.0 { 1.0 - dr * yc.abs() / m } else { 1.0 + dr * (1.0 - yc.abs() / m) };
            q.x += g("amplitude", 0.0) * d * (std::f64::consts::TAU * yc / g("wavelength", 2.0).max(0.01) + g("offset", 0.0)).sin();
        }
        "squash" => {
            let sy = (1.0 + g("factor", 0.0)).max(0.01);
            let yc = q.y.clamp(lo, hi);
            let t = (yc - lo) / span;
            let e = 0.5f64.ln() / g("maxExpandPos", 0.5).clamp(0.02, 0.98).ln();
            let bulge = (std::f64::consts::PI * t.powf(e)).sin();
            let sx = 1.0 + g("expand", 1.0) * (1.0 / sy.sqrt() - 1.0) * bulge;
            q.x *= sx;
            q.z *= sx;
            q.y = yc * sy + (q.y - yc);
        }
        "twist" => {
            let t = ((q.y - lo) / span).clamp(0.0, 1.0);
            let ang = (g("startAngle", 0.0) + (g("endAngle", 0.0) - g("startAngle", 0.0)) * t).to_radians();
            let (c, s) = (ang.cos(), ang.sin());
            q = DVec3::new(q.x * c - q.z * s, q.y, q.x * s + q.z * c);
        }
        "wave" => {
            let r = q.x.hypot(q.z);
            let (r0, r1) = (g("minRadius", 0.0), g("maxRadius", 1.0));
            if r < r0 || r > r1 {
                return q;
            }
            let d = (1.0 - g("dropoff", 0.0) * (r / r1.max(1e-6))).clamp(0.0, 1.0);
            q.y += g("amplitude", 0.0) * d * (std::f64::consts::TAU * r / g("wavelength", 1.0).max(0.01) + g("offset", 0.0)).sin();
        }
        _ => {}
    }
    q
}

/// Apply a deformer to mesh points (object space) given the mesh and handle world matrices.
pub fn apply(kind: &str, pts: &mut [DVec3], mesh_world: DMat4, handle_world: DMat4, a: &Params) {
    let env = num_or(a, "envelope", 1.0);
    if env == 0.0 {
        return;
    }
    let to_h = handle_world.inverse() * mesh_world;
    let from_h = mesh_world.inverse() * handle_world;
    for p in pts.iter_mut() {
        let q = to_h.transform_point3(*p);
        let d = deform(kind, q, a);
        let d = q.lerp(d, env);
        *p = from_h.transform_point3(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twist_and_bend() {
        let mut a = defaults("twist");
        a.insert("endAngle".into(), serde_json::json!(90));
        let q = deform("twist", DVec3::new(1.0, 1.0, 0.0), &a);
        assert!((q - DVec3::new(0.0, 1.0, 1.0)).length() < 1e-9);
        let mut b = defaults("bend");
        b.insert("curvature".into(), serde_json::json!(0.5));
        // the axis bends: points off the centre move in x
        let q = deform("bend", DVec3::new(0.0, 1.0, 0.0), &b);
        assert!(q.x > 0.1 && q.y < 1.0);
        // zero envelope leaves points alone
        let mut pts = vec![DVec3::new(1.0, 1.0, 0.0)];
        a.insert("envelope".into(), serde_json::json!(0));
        apply("twist", &mut pts, DMat4::IDENTITY, DMat4::IDENTITY, &a);
        assert_eq!(pts[0], DVec3::new(1.0, 1.0, 0.0));
    }
}

// ---------------------------------------------------------------- cluster, soft mod, lattice, blend shape
//. History ops on the deformed mesh; the handle / lattice is a node.

fn mat16(v: Option<&serde_json::Value>) -> Option<DMat4> {
    let a: Vec<f64> = v?.as_array()?.iter().filter_map(|x| x.as_f64()).collect();
    (a.len() == 16).then(|| DMat4::from_cols_slice(&a))
}

/// Cluster: the weighted vertices follow the handle's motion since bind.
pub fn cluster(pts: &mut [DVec3], mesh_world: DMat4, handle_world: DMat4, p: &Params) {
    let env = num_or(p, "envelope", 1.0);
    let Some(bind) = mat16(p.get("bind")) else { return };
    let x = mesh_world.inverse() * handle_world * bind.inverse() * mesh_world;
    let Some(w) = p.get("weights").and_then(|w| w.as_object()) else { return };
    for (k, v) in w {
        let (Ok(i), Some(wt)) = (k.parse::<usize>(), v.as_f64()) else { continue };
        if let Some(q) = pts.get_mut(i) {
            *q = q.lerp(x.transform_point3(*q), wt * env);
        }
    }
}

/// Soft Modification: like a cluster with a smooth volume falloff around the bind centre.
pub fn soft_mod(pts: &mut [DVec3], mesh_world: DMat4, handle_world: DMat4, p: &Params) {
    let env = num_or(p, "envelope", 1.0);
    let Some(bind) = mat16(p.get("bind")) else { return };
    let c: [f64; 3] = p.get("center").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    let c = DVec3::from_array(c);
    let r = num_or(p, "falloffRadius", 5.0).max(1e-4);
    let x = mesh_world.inverse() * handle_world * bind.inverse() * mesh_world;
    for q in pts.iter_mut() {
        let d = (mesh_world.transform_point3(*q) - c).length() / r;
        if d >= 1.0 {
            continue;
        }
        let w = (1.0 - d * d * (3.0 - 2.0 * d)) * env;
        *q = q.lerp(x.transform_point3(*q), w);
    }
}

fn bern(n: usize, i: usize, t: f64) -> f64 {
    let mut c = 1.0;
    for k in 1..=i {
        c = c * (n - k + 1) as f64 / k as f64;
    }
    c * t.powi(i as i32) * (1.0 - t).powi((n - i) as i32)
}

/// Lattice points in lattice space (-0.5..0.5), index (i * t + j) * u + k.
pub fn lattice_grid(div: [usize; 3]) -> Vec<[f64; 3]> {
    let mut pts = vec![];
    for i in 0..div[0] {
        for j in 0..div[1] {
            for k in 0..div[2] {
                pts.push([i as f64 / (div[0] - 1) as f64 - 0.5, j as f64 / (div[1] - 1) as f64 - 0.5, k as f64 / (div[2] - 1) as f64 - 0.5]);
            }
        }
    }
    pts
}

pub fn lattice_index(div: [usize; 3], i: usize, j: usize, k: usize) -> usize {
    (i * div[1] + j) * div[2] + k
}

/// Lattice (free-form deformation): Bernstein blend of the lattice points.
pub fn ffd(pts: &mut [DVec3], mesh_world: DMat4, lattice_world: DMat4, base: DMat4, div: [usize; 3], lpts: &[[f64; 3]], env: f64) {
    if lpts.len() != div[0] * div[1] * div[2] || div.iter().any(|d| *d < 2) {
        return;
    }
    let a = base.inverse() * mesh_world;
    let b = mesh_world.inverse() * lattice_world;
    let e = 1e-4;
    for q in pts.iter_mut() {
        let l = a.transform_point3(*q);
        let (s, t, u) = (l.x + 0.5, l.y + 0.5, l.z + 0.5);
        if [s, t, u].iter().any(|x| *x < -e || *x > 1.0 + e) {
            continue;
        }
        let bs: Vec<f64> = (0..div[0]).map(|i| bern(div[0] - 1, i, s.clamp(0.0, 1.0))).collect();
        let bt: Vec<f64> = (0..div[1]).map(|j| bern(div[1] - 1, j, t.clamp(0.0, 1.0))).collect();
        let bu: Vec<f64> = (0..div[2]).map(|k| bern(div[2] - 1, k, u.clamp(0.0, 1.0))).collect();
        let mut acc = DVec3::ZERO;
        for i in 0..div[0] {
            for j in 0..div[1] {
                let w0 = bs[i] * bt[j];
                if w0 == 0.0 {
                    continue;
                }
                for k in 0..div[2] {
                    acc += DVec3::from_array(lpts[lattice_index(div, i, j, k)]) * (w0 * bu[k]);
                }
            }
        }
        let out = b.transform_point3(acc);
        *q = q.lerp(out, env);
    }
}

/// Blend shape: add each target's deltas scaled by its weight.
pub fn blend_shape(pts: &mut [DVec3], p: &Params, weight: &dyn Fn(&str) -> f64, target_mesh: &dyn Fn(&str) -> Option<Vec<DVec3>>) {
    let env = num_or(p, "envelope", 1.0);
    let base: Vec<DVec3> = pts.to_vec();
    for t in p.get("targets").and_then(|t| t.as_array()).into_iter().flatten() {
        let name = t["name"].as_str().unwrap_or("");
        let w = weight(name) * env;
        if w.abs() < 1e-7 {
            continue;
        }
        // a live target mesh with the same vertex count drives it; otherwise the stored deltas
        match t["id"].as_str().and_then(target_mesh).filter(|v| v.len() == base.len()) {
            Some(tv) => {
                for (i, q) in pts.iter_mut().enumerate() {
                    *q += (tv[i] - base[i]) * w;
                }
            }
            None => {
                for d in t["d"].as_array().into_iter().flatten() {
                    let (Some(i), Some(dx), Some(dy), Some(dz)) = (d[0].as_u64(), d[1].as_f64(), d[2].as_f64(), d[3].as_f64()) else { continue };
                    if let Some(q) = pts.get_mut(i as usize) {
                        *q += DVec3::new(dx, dy, dz) * w;
                    }
                }
            }
        }
    }
}
