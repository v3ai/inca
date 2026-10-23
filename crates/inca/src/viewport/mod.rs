//! Viewports: cameras & navigation, scene -> GPU draw lists, picking and component display.

pub mod gpu;

use egui::{Color32, Pos2, Rect};
use glam::{DMat4, DQuat, DVec3, Mat4};
use gpu::*;
use inca_core::math::{euler_deg_from_quat, quat_from_euler_deg, Aabb, Ray};
use inca_core::mesh::PolyMesh;
use inca_core::scene::{CompMode, Id, Scene, Shape};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub const C_WIRE: [f32; 4] = [0.063, 0.063, 0.43, 1.0];
pub const C_LEAD: [f32; 4] = [0.36, 0.96, 0.64, 1.0];
pub const C_SEL: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
pub const C_HILITE: [f32; 4] = [0.53, 0.78, 1.0, 1.0];
pub const C_TEMPLATE: [f32; 4] = [0.55, 0.55, 0.55, 1.0];
pub const C_ICON: [f32; 4] = [0.063, 0.063, 0.063, 1.0];
pub const C_CURVE: [f32; 4] = [0.05, 0.05, 0.4, 1.0];
pub const C_FACE_SEL: [f32; 3] = [1.0, 0.54, 0.12];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShadeMode {
    Wire,
    Shaded,
    Textured,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LightMode {
    Default,
    All,
    Flat,
}

#[derive(Clone, Debug)]
pub struct Viewport {
    pub camera: String,
    pub shade: ShadeMode,
    pub lights: LightMode,
    pub wire_on_shaded: bool,
    pub xray: bool,
    pub backface: bool,
    pub grid: bool,
    pub hud: bool,
    pub show_lights: bool,
    pub show_cameras: bool,
    pub show_curves: bool,
    pub isolate: Option<Vec<Id>>,
    pub rect: Rect,
}

impl Viewport {
    pub fn new(camera: &str) -> Self {
        Viewport {
            camera: camera.into(),
            shade: ShadeMode::Shaded,
            lights: LightMode::Default,
            wire_on_shaded: false,
            xray: false,
            backface: false,
            grid: true,
            hud: true,
            show_lights: true,
            show_cameras: true,
            show_curves: true,
            isolate: None,
            rect: Rect::NOTHING,
        }
    }
}

/// Camera state derived from a camera node.
#[derive(Clone, Copy, Debug)]
pub struct CamView {
    pub view: DMat4,
    pub proj: DMat4,
    pub eye: DVec3,
    pub fwd: DVec3,
    pub up: DVec3,
    pub right: DVec3,
    pub ortho: bool,
    pub ortho_width: f64,
    pub fov_y: f64,
    pub coi: f64,
    pub size: [f32; 2],
}

impl CamView {
    pub fn view_proj(&self) -> DMat4 {
        self.proj * self.view
    }
    /// World -> screen (relative to the viewport rect). Returns None behind the camera.
    pub fn project(&self, p: DVec3, rect: Rect) -> Option<Pos2> {
        let c = self.view_proj() * p.extend(1.0);
        if c.w <= 1e-9 && !self.ortho {
            return None;
        }
        let n = c.truncate() / c.w;
        Some(Pos2::new(rect.min.x + ((n.x + 1.0) * 0.5) as f32 * rect.width(), rect.min.y + ((1.0 - n.y) * 0.5) as f32 * rect.height()))
    }
    pub fn depth(&self, p: DVec3) -> f64 {
        (p - self.eye).dot(self.fwd)
    }
    pub fn ray(&self, pos: Pos2, rect: Rect) -> Ray {
        let nx = ((pos.x - rect.min.x) / rect.width()) as f64 * 2.0 - 1.0;
        let ny = 1.0 - ((pos.y - rect.min.y) / rect.height()) as f64 * 2.0;
        let inv = self.view_proj().inverse();
        let a = inv.project_point3(DVec3::new(nx, ny, 0.0));
        let b = inv.project_point3(DVec3::new(nx, ny, 0.5));
        Ray { origin: a, dir: (b - a).normalize() }
    }
    /// World units per pixel at the centre of interest.
    pub fn units_per_px(&self) -> f64 {
        if self.ortho {
            self.ortho_width / self.size[0].max(1.0) as f64
        } else {
            2.0 * self.coi * (self.fov_y / 2.0).tan() / self.size[1].max(1.0) as f64
        }
    }
}

pub fn camera_id(s: &Scene, name: &str) -> Option<Id> {
    s.find(name).filter(|i| matches!(s.node(i).map(|n| &n.shape), Some(Shape::Camera(_)))).or_else(|| s.find("persp"))
}

pub fn cam_view(s: &Scene, cam: &str, size: [f32; 2]) -> CamView {
    let id = camera_id(s, cam);
    let (m, c) = match id.as_ref().and_then(|i| s.node(i).map(|n| (i, n))) {
        Some((i, n)) => (s.world_matrix(i), if let Shape::Camera(c) = &n.shape { c.clone() } else { Default::default() }),
        None => (DMat4::look_at_rh(DVec3::new(28.0, 21.0, 28.0), DVec3::ZERO, DVec3::Y).inverse(), Default::default()),
    };
    let aspect = (size[0] / size[1].max(1.0)) as f64;
    let eye = m.transform_point3(DVec3::ZERO);
    let fwd = m.transform_vector3(-DVec3::Z).normalize();
    let up = m.transform_vector3(DVec3::Y).normalize();
    let right = fwd.cross(up).normalize();
    let view = DMat4::look_to_rh(eye, fwd, up);
    let fov_y = c.fov_y(aspect);
    let proj = if c.ortho {
        let w = c.ortho_width / 2.0;
        let h = w / aspect;
        DMat4::orthographic_rh(-w, w, -h, h, c.near.max(0.01), c.far.max(10.0) + 2000.0)
    } else {
        DMat4::perspective_rh(fov_y, aspect, c.near.max(0.001), c.far.max(1.0))
    };
    CamView { view, proj, eye, fwd, up, right, ortho: c.ortho, ortho_width: c.ortho_width, fov_y, coi: c.coi.max(0.01), size }
}

// ------------------------------------------------------------------ navigation

/// Alt+LMB tumble around the centre of interest.
pub fn tumble(s: &mut Scene, cam: &str, dx: f32, dy: f32) {
    let Some(id) = camera_id(s, cam) else { return };
    let Some(n) = s.nodes.get(&id) else { return };
    let Shape::Camera(c) = &n.shape else { return };
    if c.ortho {
        return;
    }
    let coi = c.coi;
    let q = quat_from_euler_deg(n.xf.r, &n.xf.ro);
    let eye = glam::DVec3::from_array(n.xf.t);
    let fwd = q * -DVec3::Z;
    let center = eye + fwd * coi;
    let r = euler_deg_from_quat(q, "xyz");
    // derive yaw / pitch, then apply
    let yaw = fwd.x.atan2(fwd.z).to_degrees() + 180.0;
    let pitch = fwd.y.clamp(-1.0, 1.0).asin().to_degrees();
    let _ = r;
    let yaw = yaw - dx as f64 * 0.3;
    let pitch = (pitch - dy as f64 * 0.3).clamp(-89.9, 89.9);
    let nq = DQuat::from_euler(glam::EulerRot::YXZ, yaw.to_radians(), pitch.to_radians(), 0.0);
    let nf = nq * -DVec3::Z;
    let ne = center - nf * coi;
    let e = euler_deg_from_quat(nq, "xyz");
    let n = s.nodes.get_mut(&id).unwrap();
    n.xf.t = ne.to_array();
    n.xf.r = e;
    n.xf.ro = "xyz".into();
    s.rev += 1;
}

/// Alt+MMB track (pan).
pub fn track(s: &mut Scene, cv: &CamView, cam: &str, dx: f32, dy: f32) {
    let Some(id) = camera_id(s, cam) else { return };
    let k = cv.units_per_px();
    let d = cv.right * (-dx as f64 * k) + cv.up * (dy as f64 * k);
    if let Some(n) = s.nodes.get_mut(&id) {
        n.xf.t = (DVec3::from_array(n.xf.t) + d).to_array();
    }
    s.rev += 1;
}

/// Alt+RMB / wheel dolly: `amount` > 0 moves in.
pub fn dolly(s: &mut Scene, cam: &str, amount: f64) {
    let Some(id) = camera_id(s, cam) else { return };
    let Some(n) = s.nodes.get_mut(&id) else { return };
    let q = quat_from_euler_deg(n.xf.r, &n.xf.ro);
    let fwd = q * -DVec3::Z;
    if let Shape::Camera(c) = &mut n.shape {
        let f = (-amount).exp();
        if c.ortho {
            c.ortho_width = (c.ortho_width * f).clamp(0.01, 100000.0);
        } else {
            let center = DVec3::from_array(n.xf.t) + fwd * c.coi;
            c.coi = (c.coi * f).clamp(0.01, 1e6);
            n.xf.t = (center - fwd * c.coi).to_array();
        }
    }
    s.rev += 1;
}

/// Frame a world box (F / A).
pub fn frame_box(s: &mut Scene, cam: &str, b: &Aabb, aspect: f64) {
    if b.is_empty() {
        return;
    }
    let Some(id) = camera_id(s, cam) else { return };
    let Some(n) = s.nodes.get_mut(&id) else { return };
    let q = quat_from_euler_deg(n.xf.r, &n.xf.ro);
    let fwd = q * -DVec3::Z;
    let center = b.center();
    let radius = (b.size().length() * 0.5).max(0.05);
    if let Shape::Camera(c) = &mut n.shape {
        if c.ortho {
            c.ortho_width = radius * 2.0 * 1.2 * aspect.max(1.0);
            let d = c.coi;
            n.xf.t = (center - fwd * d).to_array();
        } else {
            let fov = c.fov_y(aspect).min(2.0 * ((c.fov_y(aspect) / 2.0).tan() * aspect).atan());
            let d = radius / (fov / 2.0).sin() * 1.05;
            c.coi = d;
            n.xf.t = (center - fwd * d).to_array();
        }
    }
    s.rev += 1;
}

// ------------------------------------------------------------------ display meshes

fn to_f32(p: DVec3) -> [f32; 3] {
    [p.x as f32, p.y as f32, p.z as f32]
}

fn hash_f(h: &mut DefaultHasher, x: f32) {
    x.to_bits().hash(h);
}

/// The mesh shown in the viewport (smooth mesh preview applied).
pub fn display_mesh(s: &Scene, id: &str, cache: &mut std::collections::HashMap<String, (u64, PolyMesh)>) -> Option<PolyMesh> {
    let n = s.node(id)?;
    let m = n.mesh()?;
    if m.smooth_level == 0 {
        return Some(m.mesh.clone());
    }
    let key = n.rev * 4 + m.smooth_level as u64;
    if let Some((k, pm)) = cache.get(id) {
        if *k == key {
            return Some(pm.clone());
        }
    }
    let pm = inca_core::mesh::subdivide(&m.mesh, m.smooth_level.min(3), true);
    cache.insert(id.to_string(), (key, pm.clone()));
    Some(pm)
}

pub fn build_mesh_buffers(pm: &PolyMesh, color_of: &dyn Fn(Option<&String>) -> [f32; 3]) -> (Vec<MeshVertex>, Vec<LineVertex>) {
    let normals = pm.corner_normals();
    let mut tris = Vec::with_capacity(pm.tri_count() * 3);
    for fi in 0..pm.f.len() {
        let f = &pm.f[fi];
        let col = color_of(pm.face_mat(fi));
        for t in pm.triangulate_face(fi) {
            for &c in &t {
                tris.push(MeshVertex { pos: to_f32(pm.v[f[c] as usize]), normal: to_f32(normals[fi][c]), color: col });
            }
        }
    }
    let mut wire = vec![];
    let col = [1.0, 1.0, 1.0, 1.0];
    match &pm.display_edges {
        Some(de) => {
            for e in de {
                wire.push(LineVertex { pos: to_f32(pm.v[e[0] as usize]), color: col });
                wire.push(LineVertex { pos: to_f32(pm.v[e[1] as usize]), color: col });
            }
        }
        None => {
            for (a, b) in pm.topo().edges {
                wire.push(LineVertex { pos: to_f32(pm.v[a as usize]), color: col });
                wire.push(LineVertex { pos: to_f32(pm.v[b as usize]), color: col });
            }
        }
    }
    (tris, wire)
}

/// Grid on the ground plane (or facing an ortho camera).
pub fn grid_lines(cv: &CamView) -> Vec<LineVertex> {
    let mut out = vec![];
    let sub = [0.36, 0.36, 0.36, 1.0];
    let maj = [0.30, 0.30, 0.30, 1.0];
    let axis = [0.12, 0.12, 0.12, 1.0];
    let (size, step) = (12.0f64, 1.0f64);
    // ortho cameras get a grid in their view plane
    let (u, v) = if cv.ortho {
        let f = cv.fwd.abs();
        if f.y > 0.9 {
            (DVec3::X, DVec3::Z)
        } else if f.z > 0.9 {
            (DVec3::X, DVec3::Y)
        } else {
            (DVec3::Z, DVec3::Y)
        }
    } else {
        (DVec3::X, DVec3::Z)
    };
    let n = (size / step) as i32;
    for i in -n..=n {
        let c = if i == 0 { axis } else if i % 5 == 0 { maj } else { sub };
        let k = i as f64 * step;
        for (a, b) in [(u * k - v * size, u * k + v * size), (v * k - u * size, v * k + u * size)] {
            out.push(LineVertex { pos: to_f32(a), color: c });
            out.push(LineVertex { pos: to_f32(b), color: c });
        }
    }
    out
}

fn push_line(out: &mut Vec<LineVertex>, a: DVec3, b: DVec3, c: [f32; 4]) {
    out.push(LineVertex { pos: to_f32(a), color: c });
    out.push(LineVertex { pos: to_f32(b), color: c });
}

fn circle(out: &mut Vec<LineVertex>, c: DVec3, u: DVec3, v: DVec3, r: f64, col: [f32; 4]) {
    let n = 24;
    for i in 0..n {
        let a0 = std::f64::consts::TAU * i as f64 / n as f64;
        let a1 = std::f64::consts::TAU * (i + 1) as f64 / n as f64;
        push_line(out, c + (u * a0.cos() + v * a0.sin()) * r, c + (u * a1.cos() + v * a1.sin()) * r, col);
    }
}

/// Object-space icon lines for lights, cameras, locators and curves.
pub fn icon_lines(s: &Scene, id: &str, col: [f32; 4]) -> Vec<LineVertex> {
    let mut o = vec![];
    let Some(n) = s.node(id) else { return o };
    match &n.shape {
        Shape::Locator if n.kind == "joint" => {
            let r = n.passthrough.get("rig").and_then(|r| r["radius"].as_f64()).unwrap_or(0.5) * 0.5;
            circle(&mut o, DVec3::ZERO, DVec3::X, DVec3::Y, r, col);
            circle(&mut o, DVec3::ZERO, DVec3::Y, DVec3::Z, r, col);
            circle(&mut o, DVec3::ZERO, DVec3::Z, DVec3::X, r, col);
        }
        Shape::Locator if n.kind == "emitter" => {
            for ax in [DVec3::X, DVec3::Y, DVec3::Z, DVec3::new(1.0, 1.0, 0.0).normalize(), DVec3::new(0.0, 1.0, 1.0).normalize(), DVec3::new(1.0, 0.0, 1.0).normalize()] {
                push_line(&mut o, -ax * 0.4, ax * 0.4, col);
            }
            circle(&mut o, DVec3::ZERO, DVec3::X, DVec3::Z, 0.25, col);
        }
        Shape::Locator if n.kind == "field" => {
            circle(&mut o, DVec3::ZERO, DVec3::X, DVec3::Z, 0.5, col);
            let d = DVec3::new(n.extra["directionX"].as_f64().unwrap_or(0.0), n.extra["directionY"].as_f64().unwrap_or(-1.0), n.extra["directionZ"].as_f64().unwrap_or(0.0)).normalize_or(-DVec3::Y);
            push_line(&mut o, DVec3::ZERO, d, col);
            let s = d.any_orthonormal_vector() * 0.15;
            push_line(&mut o, d, d * 0.75 + s, col);
            push_line(&mut o, d, d * 0.75 - s, col);
        }
        Shape::Locator if n.kind == "deformer" => {
            push_line(&mut o, -DVec3::Y, DVec3::Y, col);
            for y in [-1.0, 0.0, 1.0] {
                circle(&mut o, DVec3::Y * y, DVec3::X, DVec3::Z, 0.5, col);
            }
        }
        Shape::Locator => {
            for ax in [DVec3::X, DVec3::Y, DVec3::Z] {
                push_line(&mut o, -ax, ax, col);
            }
        }
        Shape::Curve(c) => {
            let pts = c.sample(12);
            for w in pts.windows(2) {
                push_line(&mut o, w[0], w[1], col);
            }
        }
        Shape::Light(l) => {
            let z = DVec3::ZERO;
            match l.ty.as_str() {
                "directionalLight" => {
                    for dx in [-0.3, 0.0, 0.3] {
                        let a = DVec3::new(dx, 0.0, 0.0);
                        push_line(&mut o, a, a - DVec3::Z * 1.5, col);
                        push_line(&mut o, a - DVec3::Z * 1.5, a - DVec3::Z * 1.2 + DVec3::Y * 0.15, col);
                    }
                }
                "spotLight" => {
                    let ang = l.attrs.get("coneAngle").and_then(|v| v.as_f64()).unwrap_or(40.0).to_radians() / 2.0;
                    let r = ang.tan() * 1.5;
                    circle(&mut o, -DVec3::Z * 1.5, DVec3::X, DVec3::Y, r, col);
                    for k in 0..4 {
                        let a = k as f64 * std::f64::consts::FRAC_PI_2;
                        push_line(&mut o, z, -DVec3::Z * 1.5 + (DVec3::X * a.cos() + DVec3::Y * a.sin()) * r, col);
                    }
                }
                "areaLight" => {
                    let c = [DVec3::new(-1.0, -1.0, 0.0), DVec3::new(1.0, -1.0, 0.0), DVec3::new(1.0, 1.0, 0.0), DVec3::new(-1.0, 1.0, 0.0)];
                    for i in 0..4 {
                        push_line(&mut o, c[i], c[(i + 1) % 4], col);
                    }
                    push_line(&mut o, z, -DVec3::Z, col);
                }
                "ambientLight" | "skyDomeLight" => {
                    circle(&mut o, z, DVec3::X, DVec3::Y, 0.5, col);
                    circle(&mut o, z, DVec3::X, DVec3::Z, 0.5, col);
                }
                _ => {
                    for ax in [DVec3::X, DVec3::Y, DVec3::Z] {
                        push_line(&mut o, -ax * 0.4, ax * 0.4, col);
                    }
                    circle(&mut o, z, DVec3::X, DVec3::Y, 0.3, col);
                    circle(&mut o, z, DVec3::Y, DVec3::Z, 0.3, col);
                }
            }
        }
        Shape::Camera(c) if !c.startup => {
            let b = [DVec3::new(-0.4, -0.3, 0.6), DVec3::new(0.4, -0.3, 0.6), DVec3::new(0.4, 0.3, 0.6), DVec3::new(-0.4, 0.3, 0.6), DVec3::new(-0.4, -0.3, -0.4), DVec3::new(0.4, -0.3, -0.4), DVec3::new(0.4, 0.3, -0.4), DVec3::new(-0.4, 0.3, -0.4)];
            for i in 0..4 {
                push_line(&mut o, b[i], b[(i + 1) % 4], col);
                push_line(&mut o, b[i + 4], b[(i + 1) % 4 + 4], col);
                push_line(&mut o, b[i], b[i + 4], col);
            }
            let lens = [DVec3::new(-0.2, -0.2, -0.4), DVec3::new(0.2, -0.2, -0.4), DVec3::new(0.2, 0.2, -0.4), DVec3::new(-0.2, 0.2, -0.4)];
            for i in 0..4 {
                push_line(&mut o, lens[i], lens[i] * DVec3::new(1.6, 1.6, 2.0), col);
            }
        }
        _ => {}
    }
    o
}

pub fn mat_color(s: &Scene, mid: &str) -> [f32; 3] {
    match s.mats.get(mid) {
        Some(m) => {
            let p = m.pbr();
            // texture-mapped colour: show the texture's average tone
            if let Some(tid) = m.maps.get(m.color_attr()).and_then(|v| v.as_str()) {
                if let Some(t) = s.texs.get(tid) {
                    if let Some(c) = t.sample(0.25, 0.25) {
                        return c;
                    }
                }
            }
            if p.unlit {
                p.base
            } else {
                [p.base[0].max(p.emission[0]), p.base[1].max(p.emission[1]), p.base[2].max(p.emission[2])]
            }
        }
        None => [0.4, 0.4, 0.4],
    }
}

/// Key for the GPU cache of a mesh node: geometry revision, smoothing and material colours.
pub fn mesh_key(s: &Scene, id: &str) -> u64 {
    let mut h = DefaultHasher::new();
    let Some(n) = s.node(id) else { return 0 };
    n.rev.hash(&mut h);
    if let Some(m) = n.mesh() {
        m.smooth_level.hash(&mut h);
        m.material.hash(&mut h);
        let mut mats: Vec<&String> = vec![&m.material];
        if let Some(fm) = &m.mesh.fm {
            mats.extend(fm.iter().flatten());
        }
        mats.sort();
        mats.dedup();
        for mid in mats {
            for c in mat_color(s, mid) {
                hash_f(&mut h, c);
            }
        }
    }
    h.finish()
}

pub fn frame_uniform(s: &Scene, vp: &Viewport, cv: &CamView) -> FrameUniform {
    let m4 = |m: DMat4| m.as_mat4().to_cols_array_2d();
    let mut lights = [GpuLight { pos: [0.0; 4], dir: [0.0; 4], color: [0.0; 4], params: [0.0; 4] }; 8];
    let mut count = 0;
    let light_mode = match vp.lights {
        LightMode::Default => 0.0,
        LightMode::All => 1.0,
        LightMode::Flat => 2.0,
    };
    if vp.lights == LightMode::All {
        for n in s.nodes.values() {
            if count >= 8 {
                break;
            }
            let Shape::Light(l) = &n.shape else { continue };
            if !s.is_visible(&n.id) {
                continue;
            }
            let wm = s.world_matrix(&n.id);
            let pos = wm.transform_point3(DVec3::ZERO);
            let dir = wm.transform_vector3(-DVec3::Z).normalize();
            let col = l.attrs.get("color").and_then(|c| c.as_array()).map(|a| [a[0].as_f64().unwrap_or(1.0), a[1].as_f64().unwrap_or(1.0), a[2].as_f64().unwrap_or(1.0)]).unwrap_or([1.0; 3]);
            let mut i = l.attrs.get("intensity").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let ty = match l.ty.as_str() {
                "ambientLight" => 0.0,
                "directionalLight" => 1.0,
                "pointLight" => 2.0,
                "spotLight" => 3.0,
                "areaLight" => 4.0,
                _ => {
                    i *= 0.5;
                    0.0
                }
            };
            let cone = l.attrs.get("coneAngle").and_then(|v| v.as_f64()).unwrap_or(40.0).to_radians() / 2.0;
            let pen = l.attrs.get("penumbraAngle").and_then(|v| v.as_f64()).unwrap_or(0.0).to_radians();
            let outer = (cone + pen.max(0.0)).cos();
            let inner = (cone + pen.min(0.0)).cos().max(outer + 1e-4);
            let decay = l.attrs.get("decayRate").and_then(|v| v.as_f64()).unwrap_or(0.0);
            lights[count] = GpuLight {
                pos: [pos.x as f32, pos.y as f32, pos.z as f32, ty],
                dir: [dir.x as f32, dir.y as f32, dir.z as f32, 0.0],
                color: [(col[0] * i) as f32, (col[1] * i) as f32, (col[2] * i) as f32, 1.0],
                params: [outer as f32, inner as f32, decay as f32, 0.0],
            };
            count += 1;
        }
    }
    FrameUniform {
        view_proj: m4(cv.view_proj()),
        view: m4(cv.view),
        cam_pos: [cv.eye.x as f32, cv.eye.y as f32, cv.eye.z as f32, if cv.ortho { 1.0 } else { 0.0 }],
        cam_dir: [cv.fwd.x as f32, cv.fwd.y as f32, cv.fwd.z as f32, 0.0],
        bg_top: [0.27, 0.29, 0.32, 1.0],
        bg_bottom: [0.045, 0.048, 0.052, 1.0],
        mode: [light_mode, count as f32, if vp.xray { 1.0 } else { 0.0 }, if vp.backface { 1.0 } else { 0.0 }],
        lights,
    }
}

/// Draw list for one viewport; uploads changed meshes.
pub fn build_draws(s: &Scene, vp: &Viewport, cv: &CamView, g: &mut Gpu, smooth_cache: &mut std::collections::HashMap<String, (u64, PolyMesh)>, live: &std::collections::HashSet<String>) -> Vec<DrawCmd> {
    let mut draws = vec![];
    if vp.grid {
        // drawn first without depth so any geometry (e.g. a ground plane) covers it
        draws.push(DrawCmd { pass: Pass::LinesUnder, geo: Geo::TransientLines(grid_lines(cv)), u: DrawUniform::new(Mat4::IDENTITY) });
    }
    let hilite = s.hilite();
    let lead = s.lead().cloned();
    for id in s.all_dag(false) {
        if !s.is_visible(&id) {
            continue;
        }
        if let Some(iso) = &vp.isolate {
            if !iso.iter().any(|r| r == &id || s.is_ancestor(r, &id)) {
                continue;
            }
        }
        let n = s.node(&id).unwrap();
        let wm = s.world_matrix(&id).as_mat4();
        let selected = s.sel.iter().any(|x| x == &id || s.is_ancestor(x, &id));
        let is_lead = lead.as_ref().is_some_and(|l| l == &id || s.is_ancestor(l, &id));
        let template = s.is_template(&id);
        let sel_col = if template { C_TEMPLATE } else if is_lead { C_LEAD } else if selected { C_SEL } else { C_WIRE };
        match &n.shape {
            Shape::Mesh(m) => {
                let key = mesh_key(s, &id);
                if g.mesh_key(&id) != Some(key) || live.contains(&id) {
                    if let Some(pm) = display_mesh(s, &id, smooth_cache) {
                        let (t, w) = build_mesh_buffers(&pm, &|fm| mat_color(s, fm.map(|x| x.as_str()).unwrap_or(&m.material)));
                        g.upload_mesh(&id, if live.contains(&id) { 0 } else { key }, &t, &w);
                    }
                }
                let pbr = s.mats.get(&m.material).map(|x| x.pbr()).unwrap_or_default();
                let in_comp = hilite.contains(&id);
                if vp.shade != ShadeMode::Wire {
                    let mut u = DrawUniform::new(wm).spec(pbr.specular, pbr.roughness);
                    if pbr.unlit {
                        u = u.unlit();
                    }
                    if template {
                        u = u.tint([0.5, 0.5, 0.5], 0.6);
                    }
                    if vp.xray || pbr.opacity < 0.99 {
                        let a = if vp.xray { 0.45 } else { pbr.opacity.max(0.1) };
                        draws.push(DrawCmd { pass: Pass::MeshBlend, geo: Geo::MeshTris(id.clone()), u: u.alpha(a) });
                    } else {
                        draws.push(DrawCmd { pass: Pass::Mesh, geo: Geo::MeshTris(id.clone()), u });
                    }
                }
                let show_wire = vp.shade == ShadeMode::Wire || vp.wire_on_shaded || selected || in_comp;
                if show_wire {
                    let c = if in_comp {
                        C_HILITE
                    } else if vp.shade == ShadeMode::Wire || selected || template {
                        sel_col
                    } else {
                        [0.0, 0.0, 0.0, 1.0]
                    };
                    draws.push(DrawCmd { pass: Pass::Lines, geo: Geo::MeshWire(id.clone()), u: DrawUniform::new(wm).tint([c[0], c[1], c[2]], 1.0).bias(0.00012) });
                }
                if in_comp && s.comp_mode == Some(CompMode::Face) {
                    let cs = s.comp_sel(&id);
                    if !cs.f.is_empty() {
                        let mut tris = vec![];
                        let pm = &m.mesh;
                        for &fi in &cs.f {
                            let Some(f) = pm.f.get(fi) else { continue };
                            for t in pm.triangulate_face(fi) {
                                for &c in &t {
                                    tris.push(MeshVertex { pos: to_f32(pm.v[f[c] as usize]), normal: [0.0, 1.0, 0.0], color: C_FACE_SEL });
                                }
                            }
                        }
                        draws.push(DrawCmd { pass: Pass::MeshBlend, geo: Geo::TransientTris(tris), u: DrawUniform::new(wm).unlit().alpha(0.5).bias(0.0002) });
                    }
                }
                if in_comp && s.comp_mode == Some(CompMode::Edge) {
                    let cs = s.comp_sel(&id);
                    if !cs.e.is_empty() {
                        let pm = &m.mesh;
                        let mut l = vec![];
                        for &(a, b) in &cs.e {
                            if (a as usize) < pm.v.len() && (b as usize) < pm.v.len() {
                                push_line(&mut l, pm.v[a as usize], pm.v[b as usize], [1.0, 0.54, 0.12, 1.0]);
                            }
                        }
                        draws.push(DrawCmd { pass: Pass::Lines, geo: Geo::TransientLines(l), u: DrawUniform::new(wm).bias(0.0004) });
                    }
                }
            }
            Shape::Light(_) if vp.show_lights => {
                let c = if selected { sel_col } else { [0.9, 0.85, 0.3, 1.0] };
                draws.push(DrawCmd { pass: Pass::Lines, geo: Geo::TransientLines(icon_lines(s, &id, c)), u: DrawUniform::new(wm) });
            }
            Shape::Camera(_) if vp.show_cameras => {
                let c = if selected { sel_col } else { C_ICON };
                draws.push(DrawCmd { pass: Pass::Lines, geo: Geo::TransientLines(icon_lines(s, &id, c)), u: DrawUniform::new(wm) });
            }
            Shape::Curve(_) if vp.show_curves => {
                let c = if selected { sel_col } else if template { C_TEMPLATE } else { C_CURVE };
                draws.push(DrawCmd { pass: Pass::Lines, geo: Geo::TransientLines(icon_lines(s, &id, c)), u: DrawUniform::new(wm) });
            }
            Shape::Locator => {
                let c = if selected {
                    sel_col
                } else if n.kind == "joint" {
                    [0.1, 0.35, 0.9, 1.0]
                } else if n.kind == "deformer" {
                    [0.75, 0.3, 0.85, 1.0]
                } else if n.kind == "emitter" || n.kind == "field" {
                    [0.95, 0.6, 0.2, 1.0]
                } else {
                    C_WIRE
                };
                draws.push(DrawCmd { pass: Pass::LinesTop, geo: Geo::TransientLines(icon_lines(s, &id, c)), u: DrawUniform::new(wm) });
                if n.kind == "joint" {
                    // bones: a thin pyramid from the joint to each child joint (world space)
                    let a = wm.as_dmat4().transform_point3(DVec3::ZERO);
                    let mut l = vec![];
                    for k in n.children.iter().filter(|k| s.node(k).is_some_and(|c| c.kind == "joint")) {
                        let b = s.world_matrix(k).transform_point3(DVec3::ZERO);
                        let d = b - a;
                        let len = d.length();
                        if len < 1e-6 {
                            continue;
                        }
                        let u = d.any_orthonormal_vector() * len * 0.08;
                        let v = d.normalize().cross(u);
                        let base = a + d * 0.15;
                        for corner in [u, v, -u, -v] {
                            push_line(&mut l, a, base + corner, c);
                            push_line(&mut l, base + corner, b, c);
                        }
                    }
                    draws.push(DrawCmd { pass: Pass::LinesTop, geo: Geo::TransientLines(l), u: DrawUniform::new(Mat4::IDENTITY) });
                }
            }
            _ => {}
        }
    }
    draws
}

// ------------------------------------------------------------------ picking

/// Nearest object under the cursor.
pub fn pick_object(s: &Scene, vp: &Viewport, cv: &CamView, pos: Pos2) -> Option<Id> {
    let ray = cv.ray(pos, vp.rect);
    let mut best: Option<(f64, Id)> = None;
    for id in s.all_dag(false) {
        if !s.is_visible(&id) || s.is_template(&id) {
            continue;
        }
        if let Some(iso) = &vp.isolate {
            if !iso.iter().any(|r| r == &id || s.is_ancestor(r, &id)) {
                continue;
            }
        }
        let n = s.node(&id).unwrap();
        let wm = s.world_matrix(&id);
        match &n.shape {
            Shape::Mesh(m) => {
                let lr = ray.transformed(&wm.inverse());
                let bb = m.mesh.bbox();
                if inca_core::math::ray_aabb(&lr, &bb).is_none() {
                    continue;
                }
                if let Some((t, _)) = m.mesh.raycast(&lr, vp.backface) {
                    let wp = wm.transform_point3(lr.at(t));
                    let d = cv.depth(wp);
                    if best.as_ref().is_none_or(|b| d < b.0) {
                        best = Some((d, id.clone()));
                    }
                }
            }
            Shape::Group => {}
            Shape::Camera(c) if c.startup => {}
            _ => {
                // icon objects: screen distance to their lines
                let lines = icon_lines(s, &id, [1.0; 4]);
                for seg in lines.chunks(2) {
                    let a = wm.transform_point3(DVec3::new(seg[0].pos[0] as f64, seg[0].pos[1] as f64, seg[0].pos[2] as f64));
                    let b = wm.transform_point3(DVec3::new(seg[1].pos[0] as f64, seg[1].pos[1] as f64, seg[1].pos[2] as f64));
                    if let (Some(pa), Some(pb)) = (cv.project(a, vp.rect), cv.project(b, vp.rect)) {
                        if seg_dist(pos, pa, pb) < 6.0 {
                            let d = cv.depth(a) - 1e6; // icons win over meshes behind them
                            if best.as_ref().is_none_or(|x| d < x.0) {
                                best = Some((d, id.clone()));
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
    best.map(|b| b.1)
}

pub fn seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t).distance(p)
}

/// Objects inside a marquee (any projected vertex or the centre inside).
pub fn marquee_objects(s: &Scene, vp: &Viewport, cv: &CamView, r: Rect) -> Vec<Id> {
    let mut out = vec![];
    for id in s.all_dag(false) {
        if !s.is_visible(&id) || s.is_template(&id) {
            continue;
        }
        let n = s.node(&id).unwrap();
        let wm = s.world_matrix(&id);
        let hit = match &n.shape {
            Shape::Mesh(m) => {
                let step = (m.mesh.v.len() / 4000).max(1);
                m.mesh.v.iter().step_by(step).any(|p| cv.project(wm.transform_point3(*p), vp.rect).is_some_and(|q| r.contains(q)))
            }
            Shape::Group => false,
            Shape::Camera(c) if c.startup => false,
            _ => icon_lines(s, &id, [1.0; 4]).iter().any(|v| cv.project(wm.transform_point3(DVec3::new(v.pos[0] as f64, v.pos[1] as f64, v.pos[2] as f64)), vp.rect).is_some_and(|q| r.contains(q))),
        };
        if hit {
            out.push(id);
        }
    }
    out
}

/// A picked component.
#[derive(Clone, Debug, PartialEq)]
pub enum Comp {
    V(u32),
    E(u32, u32),
    F(usize),
}

/// Is a world point visible (not hidden behind its own mesh)?
fn visible_point(pm: &PolyMesh, wm: &DMat4, cv: &CamView, p: DVec3) -> bool {
    let wp = wm.transform_point3(p);
    let to = if cv.ortho { -cv.fwd } else { (cv.eye - wp).normalize() };
    let r = Ray { origin: wp + to * 1e-4 * (1.0 + wp.length()), dir: to };
    let lr = r.transformed(&wm.inverse());
    match pm.raycast(&lr, false) {
        Some((t, _)) => {
            let hit = wm.transform_point3(lr.at(t));
            let dist = if cv.ortho { f64::INFINITY } else { (cv.eye - wp).length() };
            (hit - wp).length() > dist - 1e-6
        }
        None => true,
    }
}

/// Component visibility: exact raycasts on small meshes, front-facing tests on big ones.
pub struct Vis<'a> {
    pm: &'a PolyMesh,
    wm: DMat4,
    cv: &'a CamView,
    exact: bool,
    facing: Vec<bool>,
    topo: inca_core::mesh::Topo,
}

impl<'a> Vis<'a> {
    pub fn new(pm: &'a PolyMesh, wm: DMat4, cv: &'a CamView, xray: bool) -> Self {
        let exact = !xray && pm.f.len() <= 2500;
        let nm = wm.inverse().transpose();
        let facing = if xray {
            vec![true; pm.f.len()]
        } else {
            (0..pm.f.len())
                .map(|f| {
                    let c = wm.transform_point3(pm.face_center(f));
                    let n = nm.transform_vector3(pm.face_normal(f));
                    let to = if cv.ortho { -cv.fwd } else { cv.eye - c };
                    n.dot(to) > 0.0
                })
                .collect()
        };
        Vis { pm, wm, cv, exact, facing, topo: pm.topo() }
    }
    pub fn vertex(&self, v: u32) -> bool {
        if !self.topo.vert_faces[v as usize].iter().any(|&f| self.facing[f]) && !self.topo.vert_faces[v as usize].is_empty() {
            return false;
        }
        !self.exact || visible_point(self.pm, &self.wm, self.cv, self.pm.v[v as usize])
    }
    pub fn edge(&self, a: u32, b: u32) -> bool {
        let Some(e) = self.topo.edge(a, b) else { return false };
        if !self.topo.edge_faces[e].iter().any(|&f| self.facing[f]) {
            return false;
        }
        !self.exact || visible_point(self.pm, &self.wm, self.cv, (self.pm.v[a as usize] + self.pm.v[b as usize]) * 0.5)
    }
    pub fn face(&self, f: usize) -> bool {
        self.facing[f] && (!self.exact || visible_point(self.pm, &self.wm, self.cv, self.pm.face_center(f)))
    }
}

pub fn pick_component(s: &Scene, vp: &Viewport, cv: &CamView, id: &str, mode: CompMode, pos: Pos2) -> Option<Comp> {
    let n = s.node(id)?;
    let pm = &n.mesh()?.mesh;
    let wm = s.world_matrix(id);
    match mode {
        CompMode::Vertex => {
            let vis = Vis::new(pm, wm, cv, vp.xray);
            let mut best: Option<(f32, u32)> = None;
            for (i, p) in pm.v.iter().enumerate() {
                if let Some(q) = cv.project(wm.transform_point3(*p), vp.rect) {
                    let d = q.distance(pos);
                    if d < 10.0 && best.is_none_or(|b| d < b.0) && vis.vertex(i as u32) {
                        best = Some((d, i as u32));
                    }
                }
            }
            best.map(|b| Comp::V(b.1))
        }
        CompMode::Edge => {
            let vis = Vis::new(pm, wm, cv, vp.xray);
            let topo = pm.topo();
            let mut best: Option<(f32, (u32, u32))> = None;
            for &(a, b) in &topo.edges {
                let (pa, pb) = (wm.transform_point3(pm.v[a as usize]), wm.transform_point3(pm.v[b as usize]));
                if let (Some(qa), Some(qb)) = (cv.project(pa, vp.rect), cv.project(pb, vp.rect)) {
                    let d = seg_dist(pos, qa, qb);
                    if d < 8.0 && best.is_none_or(|x| d < x.0) && vis.edge(a, b) {
                        best = Some((d, (a, b)));
                    }
                }
            }
            best.map(|b| Comp::E(b.1 .0, b.1 .1))
        }
        CompMode::Face => {
            let ray = cv.ray(pos, vp.rect);
            let lr = ray.transformed(&wm.inverse());
            pm.raycast(&lr, vp.backface).map(|(_, f)| Comp::F(f))
        }
    }
}

pub fn marquee_components(s: &Scene, vp: &Viewport, cv: &CamView, id: &str, mode: CompMode, r: Rect) -> Vec<Comp> {
    let Some(pm) = s.node(id).and_then(|n| n.mesh()).map(|m| &m.mesh) else { return vec![] };
    let wm = s.world_matrix(id);
    let vis = Vis::new(pm, wm, cv, vp.xray);
    let inside = |p: DVec3| cv.project(wm.transform_point3(p), vp.rect).is_some_and(|q| r.contains(q));
    match mode {
        CompMode::Vertex => (0..pm.v.len()).filter(|&i| inside(pm.v[i]) && vis.vertex(i as u32)).map(|i| Comp::V(i as u32)).collect(),
        CompMode::Edge => pm.topo().edges.iter().filter(|&&(a, b)| inside((pm.v[a as usize] + pm.v[b as usize]) * 0.5) && vis.edge(a, b)).map(|&(a, b)| Comp::E(a, b)).collect(),
        CompMode::Face => (0..pm.f.len()).filter(|&f| inside(pm.face_center(f)) && vis.face(f)).map(Comp::F).collect(),
    }
}

/// Particles as soft dots (drawn with egui on top of the image).
pub fn draw_particles(s: &Scene, vp: &Viewport, cv: &CamView, painter: &egui::Painter) {
    for (em, list) in &s.dynamics.particles {
        let size = s.node(em).and_then(|n| n.extra["size"].as_f64()).unwrap_or(3.0) as f32;
        for p in list {
            if let Some(q) = cv.project(p.p, vp.rect) {
                if !vp.rect.contains(q) {
                    continue;
                }
                let c = inca_core::dynamics::Dynamics::particle_color(s, em, p);
                let col = Color32::from_rgba_unmultiplied((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, (c[3] * 200.0) as u8);
                painter.circle_filled(q, size * 0.5, col);
            }
        }
    }
}

/// Vertex dots for component mode (drawn with egui on top of the image).
pub fn draw_component_overlay(s: &Scene, vp: &Viewport, cv: &CamView, painter: &egui::Painter) {
    let Some(mode) = s.comp_mode else { return };
    for id in s.hilite() {
        let Some(pm) = s.node(&id).and_then(|n| n.mesh()).map(|m| &m.mesh) else { continue };
        let wm = s.world_matrix(&id);
        let cs = s.comp_sel(&id);
        match mode {
            CompMode::Vertex => {
                // exact occlusion only for small meshes (it runs every frame)
                let mut vis = Vis::new(pm, wm, cv, vp.xray);
                vis.exact &= pm.f.len() <= 600;
                for (i, p) in pm.v.iter().enumerate() {
                    let sel = cs.v.contains(&(i as u32));
                    let shown = vis.vertex(i as u32);
                    if !sel && !shown {
                        continue;
                    }
                    if let Some(q) = cv.project(wm.transform_point3(*p), vp.rect) {
                        let c = if sel { Color32::from_rgb(255, 230, 0) } else { Color32::from_rgb(194, 58, 214) };
                        painter.rect_filled(Rect::from_center_size(q, egui::vec2(4.0, 4.0)), 0.0, c);
                    }
                }
            }
            CompMode::Face => {
                for f in 0..pm.f.len() {
                    if let Some(q) = cv.project(wm.transform_point3(pm.face_center(f)), vp.rect) {
                        if !cs.f.contains(&f) && pm.f.len() < 3000 {
                            painter.rect_filled(Rect::from_center_size(q, egui::vec2(3.0, 3.0)), 0.0, Color32::from_rgb(80, 150, 220));
                        }
                    }
                }
            }
            CompMode::Edge => {}
        }
    }
}

/// World bbox of the current selection (components when in component mode).
pub fn selection_bbox(s: &Scene) -> Aabb {
    let mut b = Aabb::EMPTY;
    if s.comp_mode.is_some() {
        for id in s.hilite() {
            let wm = s.world_matrix(&id);
            let pm = &s.node(&id).unwrap().mesh().unwrap().mesh;
            for v in s.comp_verts(&id) {
                b.add(wm.transform_point3(pm.v[v as usize]));
            }
        }
        if !b.is_empty() {
            return b;
        }
    }
    for id in &s.sel {
        b.union(&s.world_bbox(id));
        if s.world_bbox(id).is_empty() {
            b.add(s.world_matrix(id).transform_point3(DVec3::ZERO));
        }
    }
    b
}

pub fn scene_bbox(s: &Scene) -> Aabb {
    let mut b = Aabb::EMPTY;
    for id in s.all_dag(false) {
        if s.is_visible(&id) && s.node(&id).is_some_and(|n| !matches!(n.shape, Shape::Group)) {
            b.union(&s.local_bbox(&id).transformed(&s.world_matrix(&id)));
        }
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use inca_core::params::Params;

    fn setup() -> (Scene, Viewport, CamView, Id) {
        let mut s = Scene::new();
        let id = s.create_mesh("polyCube", &Params::new(), None);
        let mut vp = Viewport::new("persp");
        vp.rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let b = s.world_bbox(&id);
        frame_box(&mut s, "persp", &b, 800.0 / 600.0);
        let cv = cam_view(&s, "persp", [800.0, 600.0]);
        (s, vp, cv, id)
    }

    #[test]
    fn projection_round_trip() {
        let (_, vp, cv, _) = setup();
        let p = DVec3::new(0.2, 0.3, -0.1);
        let q = cv.project(p, vp.rect).unwrap();
        let r = cv.ray(q, vp.rect);
        // the ray through the projected point passes the point
        let t = (p - r.origin).dot(r.dir);
        assert!((r.at(t) - p).length() < 1e-6);
    }

    #[test]
    fn pick_object_and_components() {
        let (s, vp, cv, id) = setup();
        let c = cv.project(DVec3::ZERO, vp.rect).unwrap();
        assert_eq!(pick_object(&s, &vp, &cv, c), Some(id.clone()));
        assert_eq!(pick_object(&s, &vp, &cv, Pos2::new(2.0, 2.0)), None);
        // the corner facing the camera (+x +y +z) is visible and pickable
        let pm = &s.node(&id).unwrap().mesh().unwrap().mesh;
        let vi = pm.v.iter().position(|v| v.x > 0.0 && v.y > 0.0 && v.z > 0.0).unwrap();
        let q = cv.project(pm.v[vi], vp.rect).unwrap();
        assert_eq!(pick_component(&s, &vp, &cv, &id, CompMode::Vertex, q + egui::vec2(3.0, 2.0)), Some(Comp::V(vi as u32)));
        // the hidden corner is not picked through the cube
        let hidden = pm.v.iter().position(|v| v.x < 0.0 && v.y < 0.0 && v.z < 0.0).unwrap();
        let hq = cv.project(pm.v[hidden], vp.rect).unwrap();
        assert_ne!(pick_component(&s, &vp, &cv, &id, CompMode::Vertex, hq), Some(Comp::V(hidden as u32)));
        // top face under the cursor
        let top = (0..pm.f.len()).find(|&f| pm.face_normal(f).y > 0.9).unwrap();
        let tq = cv.project(pm.face_center(top), vp.rect).unwrap();
        assert_eq!(pick_component(&s, &vp, &cv, &id, CompMode::Face, tq), Some(Comp::F(top)));
        // marquee over everything selects the cube and only visible vertices
        assert_eq!(marquee_objects(&s, &vp, &cv, vp.rect), vec![id.clone()]);
        let vs = marquee_components(&s, &vp, &cv, &id, CompMode::Vertex, vp.rect);
        assert_eq!(vs.len(), 7);
    }

    #[test]
    fn navigation_keeps_center_of_interest() {
        let (mut s, _, cv, _) = setup();
        let center = cv.eye + cv.fwd * cv.coi;
        tumble(&mut s, "persp", 40.0, 10.0);
        let cv2 = cam_view(&s, "persp", [800.0, 600.0]);
        let center2 = cv2.eye + cv2.fwd * cv2.coi;
        assert!((center - center2).length() < 1e-6);
        dolly(&mut s, "persp", 0.5);
        let cv3 = cam_view(&s, "persp", [800.0, 600.0]);
        assert!(cv3.coi < cv2.coi);
        assert!(((cv3.eye + cv3.fwd * cv3.coi) - center).length() < 1e-6);
    }
}
