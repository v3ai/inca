//! Curves (CV B-splines / EP interpolating / three-point arcs) and surfaces built from curves.

use crate::math::{DMat4, DVec2, DVec3};
use crate::mesh::PolyMesh;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

fn default_degree() -> usize {
    3
}
fn default_form() -> String {
    "open".into()
}
fn default_kind() -> String {
    "cv".into()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Curve {
    pub cvs: Vec<DVec3>,
    #[serde(default = "default_degree")]
    pub degree: usize,
    /// "open" | "closed" | "periodic"
    #[serde(default = "default_form")]
    pub form: String,
    /// "cv" | "ep" | "arc"
    #[serde(default = "default_kind")]
    pub kind: String,
}

impl Default for Curve {
    fn default() -> Self {
        Curve { cvs: vec![], degree: 3, form: default_form(), kind: default_kind() }
    }
}

fn knots(n: usize, p: usize) -> Vec<f64> {
    (0..n + p + 1)
        .map(|i| {
            if i <= p {
                0.0
            } else if i >= n {
                (n - p) as f64
            } else {
                (i - p) as f64
            }
        })
        .collect()
}

fn de_boor(cvs: &[DVec3], p: usize, kv: &[f64], u: f64) -> DVec3 {
    let n = cvs.len();
    let mut k = p;
    while k < n - 1 && u >= kv[k + 1] {
        k += 1;
    }
    let mut d: Vec<DVec3> = (0..=p).map(|j| cvs[(j + k).saturating_sub(p).min(n - 1)]).collect();
    for r in 1..=p {
        for j in (r..=p).rev() {
            let i = j + k - p;
            let den = kv[i + p - r + 1] - kv[i];
            let a = if den == 0.0 { 0.0 } else { (u - kv[i]) / den };
            d[j] = d[j - 1].lerp(d[j], a);
        }
    }
    d[p]
}

fn catmull(pts: &[DVec3], seg: usize, closed: bool) -> Vec<DVec3> {
    let n = pts.len() as i64;
    let p = |i: i64| if closed { pts[i.rem_euclid(n) as usize] } else { pts[i.clamp(0, n - 1) as usize] };
    let mut out = vec![];
    let spans = if closed { n } else { n - 1 };
    for i in 0..spans {
        let (p0, p1, p2, p3) = (p(i - 1), p(i), p(i + 1), p(i + 2));
        for s in 0..seg {
            let t = s as f64 / seg as f64;
            let t2 = t * t;
            let t3 = t2 * t;
            out.push(0.5 * ((2.0 * p1) + (-p0 + p2) * t + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2 + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3));
        }
    }
    out.push(if closed { pts[0] } else { pts[(n - 1) as usize] });
    out
}

impl Curve {
    pub fn new(cvs: Vec<DVec3>, degree: usize, form: &str, kind: &str) -> Self {
        Curve { cvs, degree, form: form.into(), kind: kind.into() }
    }
    pub fn periodic(&self) -> bool {
        self.form == "periodic"
    }
    pub fn spans(&self) -> usize {
        let n = self.cvs.len();
        if self.periodic() {
            n.max(1)
        } else if self.kind == "ep" {
            n.saturating_sub(1).max(1)
        } else {
            n.saturating_sub(self.degree).max(1)
        }
    }

    /// Points along the curve.
    pub fn sample(&self, seg_per_span: usize) -> Vec<DVec3> {
        let c = &self.cvs;
        if c.len() < 2 {
            return c.clone();
        }
        if self.kind == "arc" && c.len() >= 3 {
            if let Some(a) = arc_through_points(c[0], c[2], c[1], std::f64::consts::PI / (seg_per_span * 2).max(6) as f64) {
                return a;
            }
        }
        if self.degree == 1 {
            let mut pts = c.clone();
            if self.periodic() {
                pts.push(c[0]);
            }
            return pts;
        }
        if self.kind == "ep" {
            return catmull(c, seg_per_span, self.periodic());
        }
        let p = self.degree.min(c.len() - 1);
        if self.periodic() {
            let mut cvs = c.clone();
            cvs.extend_from_slice(&c[..p]);
            let n = cvs.len();
            let kv: Vec<f64> = (0..n + p + 1).map(|i| i as f64).collect();
            let total = c.len() * seg_per_span;
            return (0..=total).map(|s| de_boor(&cvs, p, &kv, (p as f64 + (s as f64 / total as f64) * (n - p) as f64).min(n as f64 - 1e-9))).collect();
        }
        let n = c.len();
        let kv = knots(n, p);
        let umax = (n - p) as f64;
        let total = ((n - p) * seg_per_span).max(2);
        let mut out: Vec<DVec3> = (0..=total).map(|s| de_boor(c, p, &kv, (umax - 1e-9).min(s as f64 / total as f64 * umax))).collect();
        *out.last_mut().unwrap() = c[n - 1];
        out
    }

    pub fn length(&self) -> f64 {
        self.sample(8).windows(2).map(|w| w[0].distance(w[1])).sum()
    }

    pub fn transformed(&self, m: &DMat4) -> Curve {
        let mut c = self.clone();
        for p in &mut c.cvs {
            *p = m.transform_point3(*p);
        }
        c
    }

    pub fn reversed(&self) -> Curve {
        let mut c = self.clone();
        c.cvs.reverse();
        c
    }

    /// Resample to `spans` uniform spans (Rebuild Curve).
    pub fn rebuilt(&self, spans: usize, degree: usize) -> Curve {
        let pts = self.sample(16);
        let lens: Vec<f64> = std::iter::once(0.0).chain(pts.windows(2).scan(0.0, |acc, w| {
            *acc += w[0].distance(w[1]);
            Some(*acc)
        })).collect();
        let total = *lens.last().unwrap_or(&0.0);
        let count = if self.periodic() { spans } else { spans + degree };
        let cvs = (0..count)
            .map(|i| {
                let t = if count <= 1 { 0.0 } else { total * i as f64 / if self.periodic() { count } else { count - 1 } as f64 };
                let k = lens.partition_point(|&l| l < t).clamp(1, pts.len() - 1);
                let seg = (lens[k] - lens[k - 1]).max(1e-12);
                pts[k - 1].lerp(pts[k], ((t - lens[k - 1]) / seg).clamp(0.0, 1.0))
            })
            .collect();
        Curve { cvs, degree, form: self.form.clone(), kind: "cv".into() }
    }
}

pub fn nurbs_circle(radius: f64, sections: usize, normal: DVec3) -> Curve {
    let n = sections.max(3);
    let rot = glam::DQuat::from_rotation_arc(DVec3::Y, normal.normalize_or(DVec3::Y));
    // CVs placed so the periodic cubic passes at `radius`
    let k = 1.0 / (std::f64::consts::PI / n as f64).cos().powi(2) * 0.5 + 0.5;
    let cvs = (0..n)
        .map(|i| {
            let a = TAU * i as f64 / n as f64;
            rot * DVec3::new(a.cos() * radius * k, 0.0, -a.sin() * radius * k)
        })
        .collect();
    Curve::new(cvs, 3, "periodic", "cv")
}

pub fn nurbs_square(side: f64, normal: DVec3) -> Curve {
    let h = side / 2.0;
    let rot = glam::DQuat::from_rotation_arc(DVec3::Y, normal.normalize_or(DVec3::Y));
    let cvs = vec![DVec3::new(-h, 0.0, -h), DVec3::new(h, 0.0, -h), DVec3::new(h, 0.0, h), DVec3::new(-h, 0.0, h)].into_iter().map(|p| rot * p).collect();
    Curve::new(cvs, 1, "periodic", "cv")
}

/// Circular arc from `start` through `through` to `end`.
pub fn arc_through_points(start: DVec3, end: DVec3, through: DVec3, step: f64) -> Option<Vec<DVec3>> {
    let a = start - through;
    let b = end - through;
    let n = a.cross(b);
    let n2 = n.length_squared();
    if n2 < 1e-14 {
        return None;
    }
    let c = through + (b * a.length_squared() - a * b.length_squared()).cross(n) / (2.0 * n2);
    let r = start.distance(c);
    let nn = n.normalize();
    let u = (start - c).normalize();
    let v = nn.cross(u);
    let ang = |p: DVec3| {
        let d = p - c;
        let x = d.dot(v).atan2(d.dot(u));
        if x < 0.0 {
            x + TAU
        } else {
            x
        }
    };
    let mut e = ang(end);
    let t = ang(through);
    if t > e {
        e -= TAU;
    }
    let steps = ((e.abs() / step).ceil() as usize).max(2);
    Some((0..=steps).map(|i| {
        let a = e * i as f64 / steps as f64;
        c + (u * a.cos() + v * a.sin()) * r
    }).collect())
}

fn grid_mesh(rows: &[Vec<DVec3>], close_u: bool, close_v: bool) -> PolyMesh {
    let mut m = PolyMesh::new(vec![], vec![]);
    if rows.len() < 2 {
        return m;
    }
    let nv = rows[0].len();
    for r in rows {
        m.v.extend(r.iter().copied());
    }
    let nr = rows.len();
    let id = |i: usize, j: usize| ((i % nr) * nv + (j % nv)) as u32;
    let iu = if close_u { nr } else { nr - 1 };
    let jv = if close_v { nv } else { nv - 1 };
    for i in 0..iu {
        for j in 0..jv {
            let uv = vec![
                DVec2::new(i as f64 / iu as f64, j as f64 / jv as f64),
                DVec2::new((i + 1) as f64 / iu as f64, j as f64 / jv as f64),
                DVec2::new((i + 1) as f64 / iu as f64, (j + 1) as f64 / jv as f64),
                DVec2::new(i as f64 / iu as f64, (j + 1) as f64 / jv as f64),
            ];
            m.add_face(vec![id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)], Some(uv), None);
        }
    }
    let mut m = crate::mesh::op_merge_vertices(&m, None, 1e-7);
    m.soft_angle = 180.0;
    m
}

/// Revolve a profile curve around a world axis.
pub fn revolve(c: &Curve, axis: usize, start: f64, end: f64, sections: usize, segments: usize) -> PolyMesh {
    let prof = c.sample(segments.max(1));
    let full = (end - start).abs() >= 360.0 - 1e-6;
    let n = sections.max(1) * 2;
    let ax = [DVec3::X, DVec3::Y, DVec3::Z][axis.min(2)];
    let count = if full { n } else { n + 1 };
    let rows: Vec<Vec<DVec3>> = (0..count)
        .map(|i| {
            let a = (start + (end - start) * i as f64 / n as f64).to_radians();
            let q = glam::DQuat::from_axis_angle(ax, a);
            prof.iter().map(|p| q * *p).collect()
        })
        .collect();
    let mut m = grid_mesh(&rows, full, c.periodic());
    if m.signed_volume() < 0.0 {
        m = crate::mesh::op_reverse(&m, None);
    }
    m
}

/// Loft through two or more curves.
pub fn loft(cs: &[Curve], segments: usize, close: bool) -> PolyMesh {
    let n = cs.iter().map(|c| c.sample(segments.max(1)).len()).max().unwrap_or(0);
    let rows: Vec<Vec<DVec3>> = cs.iter().map(|c| resample(&c.sample(segments.max(1)), n)).collect();
    grid_mesh(&rows, close, cs.iter().all(|c| c.periodic()))
}

/// Sweep a profile along a path.
pub fn extrude_along(profile: &Curve, path: &Curve, segments: usize, scale: f64, twist: f64) -> PolyMesh {
    let pp = path.sample(segments.max(1));
    let prof = profile.sample(8);
    if pp.len() < 2 {
        return PolyMesh::default();
    }
    let pc = prof.iter().copied().sum::<DVec3>() / prof.len() as f64;
    let t0 = (pp[1] - pp[0]).normalize_or(DVec3::Y);
    let pn = crate::mesh::newell(&prof).normalize_or(t0);
    let base = glam::DQuat::from_rotation_arc(pn, t0);
    let mut frame = glam::DQuat::IDENTITY;
    let mut prev_t = t0;
    let rows: Vec<Vec<DVec3>> = pp
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let t = if i + 1 < pp.len() { (pp[i + 1] - pp[i]).normalize_or(prev_t) } else { prev_t };
            frame = glam::DQuat::from_rotation_arc(prev_t, t) * frame;
            prev_t = t;
            let k = i as f64 / (pp.len() - 1) as f64;
            let s = 1.0 + (scale - 1.0) * k;
            let tw = glam::DQuat::from_axis_angle(t, (twist * k).to_radians());
            prof.iter().map(|q| *p + tw * frame * base * ((*q - pc) * s)).collect()
        })
        .collect();
    grid_mesh(&rows, path.periodic(), profile.periodic())
}

/// Planar surface (n-gon) bounded by closed curves.
pub fn planar(cs: &[Curve], segments: usize) -> PolyMesh {
    let Some(c) = cs.first() else { return PolyMesh::default() };
    let mut pts = c.sample(segments.max(1));
    if pts.len() > 2 && pts[0].distance(*pts.last().unwrap()) < 1e-9 {
        pts.pop();
    }
    if pts.len() < 3 {
        return PolyMesh::default();
    }
    let mut m = crate::mesh::polygon_from_points(&pts);
    m.soft_angle = 180.0;
    m
}

fn resample(pts: &[DVec3], n: usize) -> Vec<DVec3> {
    if pts.len() == n || pts.len() < 2 {
        return pts.to_vec();
    }
    (0..n)
        .map(|i| {
            let t = i as f64 / (n - 1) as f64 * (pts.len() - 1) as f64;
            let k = (t.floor() as usize).min(pts.len() - 2);
            pts[k].lerp(pts[k + 1], t - k as f64)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_cubic_passes_endpoints() {
        let c = Curve::new(vec![DVec3::ZERO, DVec3::X, DVec3::new(2.0, 1.0, 0.0), DVec3::new(3.0, 0.0, 0.0)], 3, "open", "cv");
        let s = c.sample(8);
        assert!(s[0].distance(DVec3::ZERO) < 1e-9);
        assert!(s.last().unwrap().distance(DVec3::new(3.0, 0.0, 0.0)) < 1e-9);
        assert_eq!(c.spans(), 1);
    }

    #[test]
    fn circle_radius_close_to_one() {
        let c = nurbs_circle(1.0, 8, DVec3::Y);
        for p in c.sample(8) {
            assert!((p.length() - 1.0).abs() < 0.03, "{}", p.length());
        }
    }

    #[test]
    fn revolve_line_makes_cylinder() {
        let c = Curve::new(vec![DVec3::new(1.0, 0.0, 0.0), DVec3::new(1.0, 1.0, 0.0)], 1, "open", "cv");
        let m = revolve(&c, 1, 0.0, 360.0, 8, 1);
        assert_eq!(m.f.len(), 16);
    }

    #[test]
    fn arc_three_points() {
        let a = arc_through_points(DVec3::X, -DVec3::X, DVec3::Y, 0.1).unwrap();
        for p in &a {
            assert!((p.length() - 1.0).abs() < 1e-9);
        }
        assert!(a.iter().any(|p| p.y > 0.99));
    }
}
