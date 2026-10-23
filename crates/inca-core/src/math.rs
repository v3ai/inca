//! Math helpers on top of glam (f64), matching Maya/Inca transform conventions.

pub use glam::{DMat3, DMat4, DQuat, DVec2, DVec3, DVec4, EulerRot};

pub const D2R: f64 = std::f64::consts::PI / 180.0;
pub const R2D: f64 = 180.0 / std::f64::consts::PI;

/// Maya rotate orders ("xyz" = rotate about X first, then Y, then Z).
pub const ROTATE_ORDERS: [&str; 6] = ["xyz", "yzx", "zxy", "xzy", "yxz", "zyx"];

/// glam intrinsic Euler order for a Maya rotate order. Maya "abc" applies a first, so the matrix
/// is Rc * Rb * Ra, which is glam's intrinsic "CBA" with angles (c, b, a).
fn euler_rot(ro: &str) -> (EulerRot, [usize; 3]) {
    match ro {
        "yzx" => (EulerRot::XZY, [0, 2, 1]),
        "zxy" => (EulerRot::YXZ, [1, 0, 2]),
        "xzy" => (EulerRot::YZX, [1, 2, 0]),
        "yxz" => (EulerRot::ZXY, [2, 0, 1]),
        "zyx" => (EulerRot::XYZ, [0, 1, 2]),
        _ => (EulerRot::ZYX, [2, 1, 0]),
    }
}

/// Euler angles in degrees -> quaternion.
pub fn quat_from_euler_deg(r: [f64; 3], ro: &str) -> DQuat {
    let (er, ix) = euler_rot(ro);
    DQuat::from_euler(er, r[ix[0]] * D2R, r[ix[1]] * D2R, r[ix[2]] * D2R)
}

/// Quaternion -> Euler angles in degrees (Maya order).
pub fn euler_deg_from_quat(q: DQuat, ro: &str) -> [f64; 3] {
    let (er, ix) = euler_rot(ro);
    let (a, b, c) = q.to_euler(er);
    let mut out = [0.0; 3];
    out[ix[0]] = a * R2D;
    out[ix[1]] = b * R2D;
    out[ix[2]] = c * R2D;
    out
}

/// Pick the angle equivalent to `a` (mod 360) that is nearest `prev`.
pub fn near_angle(prev: f64, mut a: f64) -> f64 {
    while a - prev > 180.0 {
        a -= 360.0;
    }
    while a - prev < -180.0 {
        a += 360.0;
    }
    a
}

pub fn v3(a: [f64; 3]) -> DVec3 {
    DVec3::from_array(a)
}

/// Axis-aligned bounding box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: DVec3,
    pub max: DVec3,
}

impl Default for Aabb {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl Aabb {
    pub const EMPTY: Aabb = Aabb { min: DVec3::splat(f64::INFINITY), max: DVec3::splat(f64::NEG_INFINITY) };
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x
    }
    pub fn add(&mut self, p: DVec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }
    pub fn union(&mut self, o: &Aabb) {
        if !o.is_empty() {
            self.add(o.min);
            self.add(o.max);
        }
    }
    pub fn center(&self) -> DVec3 {
        (self.min + self.max) * 0.5
    }
    pub fn size(&self) -> DVec3 {
        self.max - self.min
    }
    pub fn transformed(&self, m: &DMat4) -> Aabb {
        let mut b = Aabb::EMPTY;
        if self.is_empty() {
            return b;
        }
        for i in 0..8 {
            let p = DVec3::new(
                if i & 1 == 0 { self.min.x } else { self.max.x },
                if i & 2 == 0 { self.min.y } else { self.max.y },
                if i & 4 == 0 { self.min.z } else { self.max.z },
            );
            b.add(m.transform_point3(p));
        }
        b
    }
}

/// A ray in world space.
#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub origin: DVec3,
    pub dir: DVec3,
}

impl Ray {
    pub fn at(&self, t: f64) -> DVec3 {
        self.origin + self.dir * t
    }
    pub fn transformed(&self, m: &DMat4) -> Ray {
        Ray { origin: m.transform_point3(self.origin), dir: m.transform_vector3(self.dir) }
    }
}

/// Möller–Trumbore ray/triangle intersection, returns t.
pub fn ray_triangle(r: &Ray, a: DVec3, b: DVec3, c: DVec3, cull_back: bool) -> Option<f64> {
    let e1 = b - a;
    let e2 = c - a;
    let p = r.dir.cross(e2);
    let det = e1.dot(p);
    if cull_back && det < 1e-12 {
        return None;
    }
    if det.abs() < 1e-14 {
        return None;
    }
    let inv = 1.0 / det;
    let s = r.origin - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = r.dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 1e-9).then_some(t)
}

pub fn ray_aabb(r: &Ray, b: &Aabb) -> Option<f64> {
    let inv = r.dir.recip();
    let t0 = (b.min - r.origin) * inv;
    let t1 = (b.max - r.origin) * inv;
    let tmin = t0.min(t1).max_element();
    let tmax = t0.max(t1).min_element();
    (tmax >= tmin.max(0.0)).then_some(tmin.max(0.0))
}

/// Round to `d` decimals (for tidy channel values).
pub fn round_to(x: f64, d: i32) -> f64 {
    let m = 10f64.powi(d);
    let r = (x * m).round() / m;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// Format a number the way the Channel Box shows it (up to 3 decimals, trailing zeros trimmed).
pub fn fmt_num(x: f64) -> String {
    let s = format!("{:.3}", round_to(x, 3));
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}
