//! Inca Path Tracer: a progressive CPU path tracer (BVH + rayon) for the Render View.

use crate::materials::Pbr;
use crate::math::{Aabb, DMat4, DVec2, DVec3, Ray};
use crate::scene::{Scene, Shape};
use rayon::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub samples: u32,
    pub bounces: u32,
    pub exposure: f32,
}

impl RenderSettings {
    pub fn from_scene(s: &Scene) -> Self {
        let g = |k: &str, d: f64| s.render.get(k).and_then(|v| v.as_f64()).unwrap_or(d);
        RenderSettings { width: g("width", 640.0) as u32, height: g("height", 480.0) as u32, samples: g("samples", 64.0) as u32, bounces: g("bounces", 4.0) as u32, exposure: g("exposure", 0.0) as f32 }
    }
}

#[derive(Clone, Copy)]
struct Tri {
    p: [DVec3; 3],
    n: [DVec3; 3],
    uv: [DVec2; 3],
    mat: u32,
}

#[derive(Clone)]
struct Mat {
    pbr: Pbr,
    tex: Option<TexSampler>,
}

#[derive(Clone)]
enum TexSampler {
    Proc(crate::materials::Texture),
    Image { w: u32, h: u32, px: std::sync::Arc<Vec<[f32; 3]>>, repeat: (f64, f64) },
}

impl TexSampler {
    fn sample(&self, uv: DVec2) -> Option<[f32; 3]> {
        match self {
            TexSampler::Proc(t) => t.sample(uv.x, uv.y),
            TexSampler::Image { w, h, px, repeat } => {
                let u = (uv.x * repeat.0).rem_euclid(1.0);
                let v = (1.0 - (uv.y * repeat.1).rem_euclid(1.0)).clamp(0.0, 0.9999);
                let x = ((u * *w as f64) as u32).min(w - 1);
                let y = ((v * *h as f64) as u32).min(h - 1);
                px.get((y * w + x) as usize).copied()
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum LightKind {
    Ambient,
    Directional(DVec3),
    Point,
    Spot { dir: DVec3, cos_outer: f64, cos_inner: f64 },
    Area { u: DVec3, v: DVec3, n: DVec3 },
    Sky,
}

#[derive(Clone, Copy, Debug)]
struct Light {
    kind: LightKind,
    pos: DVec3,
    color: DVec3,
    decay: f64,
    shadows: bool,
}

#[derive(Clone, Copy, Default)]
struct Node {
    b: Aabb,
    left: u32,
    count: u32, // >0 leaf
}

pub struct RenderScene {
    tris: Vec<Tri>,
    nodes: Vec<Node>,
    mats: Vec<Mat>,
    lights: Vec<Light>,
    cam: DMat4,
    fov_y: f64,
    ortho: Option<f64>,
    pub settings: RenderSettings,
    background: DVec3,
}

fn rgb(v: Option<&serde_json::Value>) -> DVec3 {
    v.and_then(|c| c.as_array()).map(|a| DVec3::new(a[0].as_f64().unwrap_or(1.0), a.get(1).and_then(|x| x.as_f64()).unwrap_or(1.0), a.get(2).and_then(|x| x.as_f64()).unwrap_or(1.0))).unwrap_or(DVec3::ONE)
}

impl RenderScene {
    pub fn build(s: &Scene, camera: &str, settings: RenderSettings) -> RenderScene {
        let mut mats: Vec<Mat> = vec![];
        let mut mat_index: HashMap<String, u32> = HashMap::new();
        let mut image_cache: HashMap<String, TexSampler> = HashMap::new();
        let mut mat_of = |id: &str, mats: &mut Vec<Mat>| -> u32 {
            if let Some(&i) = mat_index.get(id) {
                return i;
            }
            let m = s.mats.get(id);
            let pbr = m.map(|m| m.pbr()).unwrap_or_default();
            let tex = m.and_then(|m| m.maps.get(m.color_attr()).and_then(|t| t.as_str()).and_then(|t| s.texs.get(t))).and_then(|t| {
                if t.ty == "file" {
                    let path = t.attrs.get("fileTextureName").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if let Some(c) = image_cache.get(&path) {
                        return Some(c.clone());
                    }
                    let img = image::open(&path).ok()?.to_rgb32f();
                    let (w, h) = img.dimensions();
                    let px: Vec<[f32; 3]> = img.pixels().map(|p| [p[0].powf(2.2), p[1].powf(2.2), p[2].powf(2.2)]).collect();
                    let r = (t.attrs.get("repeatU").and_then(|v| v.as_f64()).unwrap_or(1.0), t.attrs.get("repeatV").and_then(|v| v.as_f64()).unwrap_or(1.0));
                    let ts = TexSampler::Image { w, h, px: std::sync::Arc::new(px), repeat: r };
                    image_cache.insert(path, ts.clone());
                    Some(ts)
                } else {
                    Some(TexSampler::Proc(t.clone()))
                }
            });
            mats.push(Mat { pbr, tex });
            let i = (mats.len() - 1) as u32;
            mat_index.insert(id.to_string(), i);
            i
        };
        let mut tris = vec![];
        let mut lights = vec![];
        for id in s.all_dag(false) {
            if !s.is_visible(&id) {
                continue;
            }
            let n = s.node(&id).unwrap();
            let wm = s.world_matrix(&id);
            match &n.shape {
                Shape::Mesh(m) => {
                    let pm = if m.smooth_level > 0 { crate::mesh::subdivide(&m.mesh, m.smooth_level.min(3), true) } else { m.mesh.clone() };
                    let nm = DMat4::from_mat3(glam::DMat3::from_mat4(wm).inverse().transpose());
                    let normals = pm.corner_normals();
                    let flip = wm.determinant() < 0.0;
                    for fi in 0..pm.f.len() {
                        let f = &pm.f[fi];
                        let mi = mat_of(pm.face_mat(fi).map(|x| x.as_str()).unwrap_or(&m.material), &mut mats);
                        let uv = pm.face_uv(fi);
                        for t in pm.triangulate_face(fi) {
                            let mut c = t;
                            if flip {
                                c.swap(1, 2);
                            }
                            tris.push(Tri {
                                p: c.map(|k| wm.transform_point3(pm.v[f[k] as usize])),
                                n: c.map(|k| nm.transform_vector3(normals[fi][k]).normalize_or_zero()),
                                uv: c.map(|k| uv.map(|u| u[k]).unwrap_or_default()),
                                mat: mi,
                            });
                        }
                    }
                }
                Shape::Light(l) => {
                    let pos = wm.transform_point3(DVec3::ZERO);
                    let dir = wm.transform_vector3(-DVec3::Z).normalize();
                    let i = l.attrs.get("intensity").and_then(|v| v.as_f64()).unwrap_or(1.0);
                    let color = rgb(l.attrs.get("color")) * i;
                    let decay = l.attrs.get("decayRate").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let shadows = true;
                    let kind = match l.ty.as_str() {
                        "ambientLight" => LightKind::Ambient,
                        "directionalLight" => LightKind::Directional(dir),
                        "spotLight" => {
                            let cone = l.attrs.get("coneAngle").and_then(|v| v.as_f64()).unwrap_or(40.0).to_radians() / 2.0;
                            let pen = l.attrs.get("penumbraAngle").and_then(|v| v.as_f64()).unwrap_or(0.0).to_radians();
                            LightKind::Spot { dir, cos_outer: (cone + pen.max(0.0)).cos(), cos_inner: (cone + pen.min(0.0)).cos() }
                        }
                        "areaLight" => LightKind::Area { u: wm.transform_vector3(DVec3::X), v: wm.transform_vector3(DVec3::Y), n: dir },
                        "skyDomeLight" => LightKind::Sky,
                        _ => LightKind::Point,
                    };
                    lights.push(Light { kind, pos, color, decay, shadows });
                }
                _ => {}
            }
        }
        // Maya renders with a default light when the scene has none
        let cid = s.find(camera).or_else(|| s.find("persp"));
        let cam = cid.as_ref().map(|c| s.world_matrix(c)).unwrap_or(DMat4::IDENTITY);
        let cshape = cid.as_ref().and_then(|c| match &s.node(c)?.shape {
            Shape::Camera(c) => Some(c.clone()),
            _ => None,
        });
        let cshape = cshape.unwrap_or_default();
        if lights.iter().all(|l| matches!(l.kind, LightKind::Ambient)) {
            let d = cam.transform_vector3(DVec3::new(-0.3, -0.5, -1.0)).normalize();
            lights.push(Light { kind: LightKind::Directional(d), pos: DVec3::ZERO, color: DVec3::splat(1.0), decay: 0.0, shadows: true });
            lights.push(Light { kind: LightKind::Ambient, pos: DVec3::ZERO, color: DVec3::splat(0.12), decay: 0.0, shadows: false });
        }
        let has_sky = lights.iter().any(|l| matches!(l.kind, LightKind::Sky));
        let background = if has_sky { lights.iter().find(|l| matches!(l.kind, LightKind::Sky)).unwrap().color } else { DVec3::ZERO };
        let aspect = settings.width as f64 / settings.height.max(1) as f64;
        let mut rs = RenderScene { tris, nodes: vec![], mats, lights, cam, fov_y: cshape.fov_y(aspect), ortho: cshape.ortho.then_some(cshape.ortho_width), settings, background };
        rs.build_bvh();
        rs
    }

    fn build_bvh(&mut self) {
        let n = self.tris.len();
        let mut idx: Vec<u32> = (0..n as u32).collect();
        let cent: Vec<DVec3> = self.tris.iter().map(|t| (t.p[0] + t.p[1] + t.p[2]) / 3.0).collect();
        self.nodes = vec![Node::default()];
        if n == 0 {
            return;
        }
        let mut stack = vec![(0usize, 0usize, n)];
        while let Some((ni, start, end)) = stack.pop() {
            let mut b = Aabb::EMPTY;
            let mut cb = Aabb::EMPTY;
            for &i in &idx[start..end] {
                for p in self.tris[i as usize].p {
                    b.add(p);
                }
                cb.add(cent[i as usize]);
            }
            self.nodes[ni].b = b;
            let count = end - start;
            if count <= 4 {
                self.nodes[ni].left = start as u32;
                self.nodes[ni].count = count as u32;
                continue;
            }
            let ext = cb.size();
            let axis = if ext.x >= ext.y && ext.x >= ext.z { 0 } else if ext.y >= ext.z { 1 } else { 2 };
            let mid = start + count / 2;
            idx[start..end].select_nth_unstable_by(count / 2, |a, b| cent[*a as usize][axis].partial_cmp(&cent[*b as usize][axis]).unwrap_or(std::cmp::Ordering::Equal));
            let l = self.nodes.len();
            self.nodes.push(Node::default());
            self.nodes.push(Node::default());
            self.nodes[ni].left = l as u32;
            self.nodes[ni].count = 0;
            stack.push((l, start, mid));
            stack.push((l + 1, mid, end));
        }
        let tris = std::mem::take(&mut self.tris);
        self.tris = idx.iter().map(|&i| tris[i as usize]).collect();
    }

    fn intersect(&self, r: &Ray, tmax: f64) -> Option<(f64, usize, f64, f64)> {
        if self.tris.is_empty() {
            return None;
        }
        let inv = r.dir.recip();
        let mut best: Option<(f64, usize, f64, f64)> = None;
        let mut limit = tmax;
        let mut stack = [0u32; 64];
        let mut sp = 1;
        while sp > 0 {
            sp -= 1;
            let node = &self.nodes[stack[sp] as usize];
            let t0 = (node.b.min - r.origin) * inv;
            let t1 = (node.b.max - r.origin) * inv;
            let tn = t0.min(t1).max_element();
            let tf = t0.max(t1).min_element();
            if tf < tn.max(0.0) || tn > limit {
                continue;
            }
            if node.count > 0 {
                for i in node.left..node.left + node.count {
                    let t = &self.tris[i as usize];
                    let e1 = t.p[1] - t.p[0];
                    let e2 = t.p[2] - t.p[0];
                    let p = r.dir.cross(e2);
                    let det = e1.dot(p);
                    if det.abs() < 1e-14 {
                        continue;
                    }
                    let id = 1.0 / det;
                    let sv = r.origin - t.p[0];
                    let u = sv.dot(p) * id;
                    if !(0.0..=1.0).contains(&u) {
                        continue;
                    }
                    let q = sv.cross(e1);
                    let v = r.dir.dot(q) * id;
                    if v < 0.0 || u + v > 1.0 {
                        continue;
                    }
                    let tt = e2.dot(q) * id;
                    if tt > 1e-6 && tt < limit {
                        limit = tt;
                        best = Some((tt, i as usize, u, v));
                    }
                }
            } else if sp + 2 <= stack.len() {
                stack[sp] = node.left;
                stack[sp + 1] = node.left + 1;
                sp += 2;
            }
        }
        best
    }

    fn occluded(&self, o: DVec3, d: DVec3, dist: f64) -> bool {
        self.intersect(&Ray { origin: o, dir: d }, dist).is_some_and(|h| self.mats[self.tris[h.1].mat as usize].pbr.transmission < 0.5)
    }

    fn camera_ray(&self, x: f64, y: f64) -> Ray {
        let (w, h) = (self.settings.width as f64, self.settings.height as f64);
        let nx = (x / w) * 2.0 - 1.0;
        let ny = 1.0 - (y / h) * 2.0;
        let aspect = w / h;
        match self.ortho {
            Some(ow) => {
                let o = self.cam.transform_point3(DVec3::new(nx * ow / 2.0, ny * ow / 2.0 / aspect, 0.0));
                Ray { origin: o, dir: self.cam.transform_vector3(-DVec3::Z).normalize() }
            }
            None => {
                let t = (self.fov_y / 2.0).tan();
                let d = DVec3::new(nx * t * aspect, ny * t, -1.0).normalize();
                Ray { origin: self.cam.transform_point3(DVec3::ZERO), dir: self.cam.transform_vector3(d).normalize() }
            }
        }
    }

    fn radiance(&self, mut ray: Ray, rng: &mut Rng) -> DVec3 {
        let mut col = DVec3::ZERO;
        let mut thr = DVec3::ONE;
        for depth in 0..=self.settings.bounces {
            let Some((t, ti, u, v)) = self.intersect(&ray, f64::INFINITY) else {
                col += thr * self.background;
                break;
            };
            let tri = &self.tris[ti];
            let w = 1.0 - u - v;
            let p = ray.at(t);
            let gn = (tri.p[1] - tri.p[0]).cross(tri.p[2] - tri.p[0]).normalize_or_zero();
            let mut n = (tri.n[0] * w + tri.n[1] * u + tri.n[2] * v).normalize_or(gn);
            let front = ray.dir.dot(gn) < 0.0;
            if !front {
                n = -n;
            }
            let ng = if front { gn } else { -gn };
            let m = &self.mats[tri.mat as usize];
            let mut base = DVec3::new(m.pbr.base[0] as f64, m.pbr.base[1] as f64, m.pbr.base[2] as f64);
            if let Some(tx) = &m.tex {
                if let Some(c) = tx.sample(tri.uv[0] * w + tri.uv[1] * u + tri.uv[2] * v) {
                    base = DVec3::new(c[0] as f64, c[1] as f64, c[2] as f64);
                }
            }
            let emis = DVec3::new(m.pbr.emission[0] as f64, m.pbr.emission[1] as f64, m.pbr.emission[2] as f64);
            if m.pbr.unlit {
                col += thr * base;
                break;
            }
            col += thr * emis;
            let p_off = p + ng * 1e-5 * (1.0 + p.length());
            // glass
            if m.pbr.transmission > 0.5 {
                let eta = if front { 1.0 / m.pbr.ior as f64 } else { m.pbr.ior as f64 };
                let cosi = -ray.dir.dot(n);
                let k = 1.0 - eta * eta * (1.0 - cosi * cosi);
                let r0 = ((1.0 - eta) / (1.0 + eta)).powi(2);
                let fres = r0 + (1.0 - r0) * (1.0 - cosi).powi(5);
                if k < 0.0 || rng.f() < fres {
                    ray = Ray { origin: p_off, dir: (ray.dir + n * 2.0 * cosi).normalize() };
                } else {
                    ray = Ray { origin: p - ng * 1e-5 * (1.0 + p.length()), dir: (ray.dir * eta + n * (eta * cosi - k.sqrt())).normalize() };
                    thr *= base.max(DVec3::splat(0.6));
                }
                continue;
            }
            let metal = m.pbr.metalness as f64;
            let rough = (m.pbr.roughness as f64).clamp(0.03, 1.0);
            let spec_k = (m.pbr.specular as f64 * 0.04 * (1.0 - metal) + metal).clamp(0.0, 1.0);
            let diffuse = base * (1.0 - metal);
            let spec_col = DVec3::splat(spec_k).lerp(base, metal);
            let v_dir = -ray.dir;
            // direct light
            for l in &self.lights {
                let (ldir, dist, li) = match l.kind {
                    LightKind::Ambient | LightKind::Sky => {
                        col += thr * diffuse * l.color * if matches!(l.kind, LightKind::Sky) { 0.0 } else { 1.0 };
                        continue;
                    }
                    LightKind::Directional(d) => (-d, f64::INFINITY, l.color),
                    LightKind::Point | LightKind::Spot { .. } | LightKind::Area { .. } => {
                        let mut lp = l.pos;
                        if let LightKind::Area { u, v, .. } = l.kind {
                            lp += u * (rng.f() * 2.0 - 1.0) + v * (rng.f() * 2.0 - 1.0);
                        }
                        let d = lp - p;
                        let dist = d.length();
                        let ld = d / dist.max(1e-9);
                        let mut li = l.color;
                        if l.decay > 0.5 {
                            li /= dist.max(0.01).powf(l.decay);
                        }
                        match l.kind {
                            LightKind::Spot { dir, cos_outer, cos_inner } => {
                                let c = (-ld).dot(dir);
                                let t = ((c - cos_outer) / (cos_inner - cos_outer).max(1e-4)).clamp(0.0, 1.0);
                                li *= t * t * (3.0 - 2.0 * t);
                            }
                            LightKind::Area { n: an, .. } => li *= (-ld).dot(an).max(0.0),
                            _ => {}
                        }
                        (ld, dist, li)
                    }
                };
                let ndl = n.dot(ldir);
                if ndl <= 0.0 || li.max_element() <= 0.0 {
                    continue;
                }
                if l.shadows && self.occluded(p_off, ldir, dist - 1e-4) {
                    continue;
                }
                let h = (ldir + v_dir).normalize();
                let shin = 2.0 / (rough * rough) - 2.0;
                let spec = spec_col * (n.dot(h).max(0.0)).powf(shin.max(1.0)) * (shin + 8.0) / (8.0 * std::f64::consts::PI);
                col += thr * li * ndl * (diffuse / std::f64::consts::PI * std::f64::consts::PI + spec);
            }
            if depth == self.settings.bounces {
                break;
            }
            // indirect bounce: choose specular or diffuse
            let ps = (spec_col.max_element() * (1.0 - rough * 0.5)).clamp(0.05, 0.95);
            if rng.f() < ps {
                let refl = (ray.dir - n * 2.0 * ray.dir.dot(n)).normalize();
                let d = (refl + random_unit(rng) * rough * rough).normalize_or(refl);
                if d.dot(n) <= 0.0 {
                    break;
                }
                thr *= spec_col / ps;
                ray = Ray { origin: p_off, dir: d };
            } else {
                let d = (n + random_unit(rng)).normalize_or(n);
                thr *= diffuse / (1.0 - ps);
                ray = Ray { origin: p_off, dir: d };
            }
            if depth > 2 {
                let q = thr.max_element().min(0.95);
                if rng.f() > q {
                    break;
                }
                thr /= q;
            }
        }
        col
    }

    /// Add one sample per pixel to `accum` (RGB triples, row-major).
    pub fn render_pass(&self, accum: &mut [f32], pass: u32) {
        let w = self.settings.width as usize;
        accum.par_chunks_mut(w * 3).enumerate().for_each(|(y, row)| {
            let mut rng = Rng::new((y as u64 + 1).wrapping_mul(0x9E3779B97F4A7C15) ^ (pass as u64 + 1).wrapping_mul(0xD1B54A32D192ED03));
            for x in 0..w {
                let r = self.camera_ray(x as f64 + rng.f(), y as f64 + rng.f());
                let c = self.radiance(r, &mut rng);
                let c = if c.is_finite() { c.min(DVec3::splat(20.0)) } else { DVec3::ZERO };
                row[x * 3] += c.x as f32;
                row[x * 3 + 1] += c.y as f32;
                row[x * 3 + 2] += c.z as f32;
            }
        });
    }

    /// Alpha: 255 where a camera ray (4 per pixel) hits geometry, 0 where it sees the background.
    pub fn coverage(&self) -> Vec<u8> {
        let (w, h) = (self.settings.width as usize, self.settings.height as usize);
        let mut out = vec![0u8; w * h];
        out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            for (x, a) in row.iter_mut().enumerate() {
                let hits = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)].iter().filter(|(dx, dy)| self.intersect(&self.camera_ray(x as f64 + dx, y as f64 + dy), f64::INFINITY).is_some()).count();
                *a = (hits * 255 / 4) as u8;
            }
        });
        out
    }

    /// Tone-mapped sRGB pixels from an accumulation buffer.
    pub fn resolve(&self, accum: &[f32], passes: u32) -> Vec<u8> {
        let k = 2f32.powf(self.settings.exposure) / passes.max(1) as f32;
        let mut out = Vec::with_capacity(accum.len() / 3 * 4);
        for c in accum.chunks(3) {
            for &x in c {
                let v = x * k;
                let v = v / (1.0 + v * 0.15); // gentle shoulder
                let s = if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
                out.push((s.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            }
            out.push(255);
        }
        out
    }

    pub fn triangle_count(&self) -> usize {
        self.tris.len()
    }
}

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    fn f(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn random_unit(r: &mut Rng) -> DVec3 {
    let z = r.f() * 2.0 - 1.0;
    let a = r.f() * std::f64::consts::TAU;
    let s = (1.0 - z * z).max(0.0).sqrt();
    DVec3::new(s * a.cos(), s * a.sin(), z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;

    #[test]
    fn renders_a_lit_cube() {
        let mut s = Scene::new();
        s.create_mesh("polyCube", &Params::new(), None);
        let cam = s.find("persp").unwrap();
        s.nodes.get_mut(&cam).unwrap().xf.t = [3.0, 2.25, 3.0];
        let rs = RenderScene::build(&s, "persp", RenderSettings { width: 32, height: 24, samples: 1, bounces: 1, exposure: 0.0 });
        assert_eq!(rs.triangle_count(), 12);
        let mut acc = vec![0.0; 32 * 24 * 3];
        rs.render_pass(&mut acc, 0);
        // the cube is in the middle of the frame and lit
        let c = (12 * 32 + 16) * 3;
        assert!(acc[c] > 0.05, "{}", acc[c]);
        // background corner is black
        assert_eq!(acc[0], 0.0);
        let px = rs.resolve(&acc, 1);
        assert_eq!(px.len(), 32 * 24 * 4);
    }
}
