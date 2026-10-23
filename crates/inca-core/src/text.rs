//! 3D type: glyph outlines from a TrueType font, flattened, capped and extruded into a polygon mesh.

use crate::math::DVec3;
use crate::mesh::PolyMesh;

struct Outline {
    contours: Vec<Vec<[f64; 2]>>,
    cur: Vec<[f64; 2]>,
    steps: usize,
    scale: f64,
    dx: f64,
}

impl Outline {
    fn pt(&self, x: f32, y: f32) -> [f64; 2] {
        [x as f64 * self.scale + self.dx, y as f64 * self.scale]
    }
}

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        self.cur = vec![p];
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        self.cur.push(p);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let a = *self.cur.last().unwrap_or(&[0.0, 0.0]);
        let (b, c) = (self.pt(x1, y1), self.pt(x, y));
        for i in 1..=self.steps {
            let t = i as f64 / self.steps as f64;
            let u = 1.0 - t;
            self.cur.push([u * u * a[0] + 2.0 * u * t * b[0] + t * t * c[0], u * u * a[1] + 2.0 * u * t * b[1] + t * t * c[1]]);
        }
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let a = *self.cur.last().unwrap_or(&[0.0, 0.0]);
        let (b, c, d) = (self.pt(x1, y1), self.pt(x2, y2), self.pt(x, y));
        for i in 1..=self.steps {
            let t = i as f64 / self.steps as f64;
            let u = 1.0 - t;
            let k = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            self.cur.push([k[0] * a[0] + k[1] * b[0] + k[2] * c[0] + k[3] * d[0], k[0] * a[1] + k[1] * b[1] + k[2] * c[1] + k[3] * d[1]]);
        }
    }
    fn close(&mut self) {
        let mut c = std::mem::take(&mut self.cur);
        if c.len() > 1 && c.first() == c.last() {
            c.pop();
        }
        c.dedup();
        if c.len() >= 3 {
            self.contours.push(c);
        }
    }
}

fn area(c: &[[f64; 2]]) -> f64 {
    let n = c.len();
    (0..n).map(|i| c[i][0] * c[(i + 1) % n][1] - c[(i + 1) % n][0] * c[i][1]).sum::<f64>() * 0.5
}

fn inside(p: [f64; 2], c: &[[f64; 2]]) -> bool {
    let mut odd = false;
    let n = c.len();
    for i in 0..n {
        let (a, b) = (c[i], c[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
            odd = !odd;
        }
    }
    odd
}

/// Polygon mesh of `text` in the XY plane (baseline at y = 0, facing +Z), `size` units tall per em
/// and `depth` deep (0 = flat caps only). `steps` segments per curve.
pub fn text_mesh(text: &str, size: f64, depth: f64, steps: usize) -> PolyMesh {
    let Ok(face) = ttf_parser::Face::parse(epaint_default_fonts::UBUNTU_LIGHT, 0) else { return PolyMesh::default() };
    let scale = size / face.units_per_em() as f64;
    let mut ol = Outline { contours: vec![], cur: vec![], steps: steps.max(1), scale, dx: 0.0 };
    let line_h = (face.ascender() - face.descender() + face.line_gap()) as f64 * scale;
    let mut groups: Vec<Vec<Vec<[f64; 2]>>> = vec![];
    for (li, line) in text.lines().enumerate() {
        ol.dx = 0.0;
        for ch in line.chars() {
            let Some(g) = face.glyph_index(ch) else {
                ol.dx += size * 0.5;
                continue;
            };
            ol.contours.clear();
            face.outline_glyph(g, &mut ol);
            let mut cs: Vec<Vec<[f64; 2]>> = std::mem::take(&mut ol.contours);
            for c in cs.iter_mut() {
                for p in c.iter_mut() {
                    p[1] -= li as f64 * line_h;
                }
            }
            if !cs.is_empty() {
                groups.push(cs);
            }
            ol.dx += face.glyph_hor_advance(g).unwrap_or(0) as f64 * scale;
        }
    }
    let (zf, zb) = (depth * 0.5, -depth * 0.5);
    let mut v: Vec<DVec3> = vec![];
    let mut f: Vec<Vec<u32>> = vec![];
    for cs in groups {
        // the largest contour's winding marks the outer contours; the rest are holes
        let big = cs.iter().map(|c| area(c)).fold(0.0f64, |m, a| if a.abs() > m.abs() { a } else { m });
        let (outers, holes): (Vec<_>, Vec<_>) = cs.into_iter().partition(|c| area(c).signum() == big.signum());
        let mut shapes: Vec<(Vec<[f64; 2]>, Vec<Vec<[f64; 2]>>)> = outers.into_iter().map(|mut o| {
            if area(&o) < 0.0 {
                o.reverse(); // counter-clockwise seen from +Z
            }
            (o, vec![])
        }).collect();
        for mut h in holes {
            if area(&h) > 0.0 {
                h.reverse();
            }
            let at = shapes.iter().position(|(o, _)| inside(h[0], o)).unwrap_or(0);
            if let Some(s) = shapes.get_mut(at) {
                s.1.push(h);
            }
        }
        for (outer, hs) in shapes {
            let mut flat: Vec<f64> = vec![];
            let mut hole_idx = vec![];
            let mut ring: Vec<[f64; 2]> = outer.clone();
            for h in &hs {
                hole_idx.push(ring.len());
                ring.extend_from_slice(h);
            }
            for p in &ring {
                flat.extend_from_slice(p);
            }
            let base = v.len() as u32;
            let n = ring.len() as u32;
            for p in &ring {
                v.push(DVec3::new(p[0], p[1], zf));
            }
            if depth > 0.0 {
                for p in &ring {
                    v.push(DVec3::new(p[0], p[1], zb));
                }
            }
            if let Ok(tris) = earcutr::earcut(&flat, &hole_idx, 2) {
                for t in tris.chunks(3) {
                    let (a, b, c) = (t[0] as u32, t[1] as u32, t[2] as u32);
                    // earcut keeps the input winding: make the front cap face +Z
                    let ccw = {
                        let (p, q, r) = (ring[t[0]], ring[t[1]], ring[t[2]]);
                        (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]) > 0.0
                    };
                    let (b, c) = if ccw { (b, c) } else { (c, b) };
                    f.push(vec![base + a, base + b, base + c]);
                    if depth > 0.0 {
                        f.push(vec![base + n + a, base + n + c, base + n + b]);
                    }
                }
            }
            if depth > 0.0 {
                // side walls, outward for counter-clockwise outers and clockwise holes
                let mut start = 0usize;
                let mut loops: Vec<(usize, usize)> = vec![];
                for &hi in &hole_idx {
                    loops.push((start, hi));
                    start = hi;
                }
                loops.push((start, ring.len()));
                for (s0, s1) in loops {
                    for i in s0..s1 {
                        let j = if i + 1 == s1 { s0 } else { i + 1 };
                        let (a, b) = (base + i as u32, base + j as u32);
                        f.push(vec![a + n, b + n, b, a]);
                    }
                }
            }
        }
    }
    let mut m = PolyMesh::new(v, f);
    m.soft_angle = 30.0;
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_has_caps_and_walls() {
        let m = text_mesh("Ao", 1.0, 0.2, 4);
        assert!(m.f.len() > 50);
        let b = m.v.iter().fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        assert!((b.1.z - 0.1).abs() < 1e-9 && (b.0.z + 0.1).abs() < 1e-9);
        assert!(b.1.y > 0.5 && b.1.y < 1.0, "cap height about 0.7 em: {}", b.1.y);
        // front faces point +Z
        let front = m.f.iter().filter(|f| f.iter().all(|&i| m.v[i as usize].z > 0.0)).count();
        assert!(front > 10);
        for fc in m.f.iter().filter(|f| f.iter().all(|&i| m.v[i as usize].z > 0.0)) {
            let p: Vec<DVec3> = fc.iter().map(|&i| m.v[i as usize]).collect();
            assert!(crate::mesh::newell(&p).z >= -1e-12);
        }
    }
}
