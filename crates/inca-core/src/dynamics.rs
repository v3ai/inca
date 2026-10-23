//! Dynamics: particle emitters + fields, and rigid bodies simulated with rapier3d.
//!
//! The simulation runs forward from the playback start frame; scrubbing backwards re-simulates
//! from the start, so results are deterministic. Emitter / field / rigid body settings live in
//! each node's `extra` map (editable in the Attribute Editor and with `setAttr`).

use crate::math::{DMat4, DQuat, DVec3};
use crate::scene::{Id, Scene, Shape, Xform};
use rapier3d::prelude as rp;
use serde_json::{json, Value};
use std::collections::HashMap;

pub const EMITTER_TYPES: &[&str] = &["omni", "directional", "volume", "surface"];
pub const FIELD_TYPES: &[&str] = &["gravity", "drag", "turbulence", "radial", "vortex", "air"];

pub fn emitter_defaults(ty: &str) -> Value {
    json!({ "emitterType": ty, "rate": 100.0, "speed": 1.0, "spread": 0.2, "lifespan": 3.0, "directionX": 0.0, "directionY": 1.0, "directionZ": 0.0, "color": [1.0, 1.0, 1.0], "endColor": [1.0, 1.0, 1.0], "size": 3.0, "seed": 1.0, "source": "" })
}

pub fn field_defaults(ty: &str) -> Value {
    let (m, d) = match ty {
        "gravity" => (9.8, [0.0, -1.0, 0.0]),
        "drag" => (0.5, [0.0, 0.0, 0.0]),
        "turbulence" => (2.0, [0.0, 0.0, 0.0]),
        "radial" => (5.0, [0.0, 0.0, 0.0]),
        "vortex" => (5.0, [0.0, 1.0, 0.0]),
        _ => (4.0, [1.0, 0.0, 0.0]),
    };
    json!({ "fieldType": ty, "magnitude": m, "directionX": d[0], "directionY": d[1], "directionZ": d[2], "frequency": 1.0, "maxDistance": -1.0, "attenuation": 0.0 })
}

pub fn rigid_defaults(active: bool) -> Value {
    json!({ "rigidBody": if active { "active" } else { "passive" }, "mass": 1.0, "bounciness": 0.6, "friction": 0.3 })
}

#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub p: DVec3,
    pub v: DVec3,
    pub age: f64,
    pub life: f64,
}

struct Rigid {
    world: rp::PhysicsWorld,
    bodies: Vec<(Id, rp::RigidBodyHandle, DVec3)>, // node, body, scale
}

#[derive(Default)]
pub struct Dynamics {
    /// particles per emitter at `frame`
    pub particles: HashMap<Id, Vec<Particle>>,
    pub frame: Option<f64>,
    /// transforms of active rigid bodies at the start frame
    initial: HashMap<Id, Xform>,
    rigid: Option<Rigid>,
    emit_carry: HashMap<Id, f64>,
    rng: u64,
}

fn ef(v: &Value, k: &str, d: f64) -> f64 {
    v.get(k).and_then(|x| x.as_f64()).unwrap_or(d)
}
fn rgb(v: &Value, k: &str) -> [f64; 3] {
    v.get(k).and_then(|c| c.as_array()).map(|a| [a[0].as_f64().unwrap_or(1.0), a.get(1).and_then(|x| x.as_f64()).unwrap_or(1.0), a.get(2).and_then(|x| x.as_f64()).unwrap_or(1.0)]).unwrap_or([1.0; 3])
}

impl Dynamics {
    fn rand(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }
    fn rand_dir(&mut self) -> DVec3 {
        let z = self.rand() * 2.0 - 1.0;
        let a = self.rand() * std::f64::consts::TAU;
        let s = (1.0 - z * z).max(0.0).sqrt();
        DVec3::new(s * a.cos(), s * a.sin(), z)
    }

    pub fn particle_count(&self) -> usize {
        self.particles.values().map(|p| p.len()).sum()
    }

    /// Particle colour (rgb + alpha) by age.
    pub fn particle_color(s: &Scene, emitter: &str, p: &Particle) -> [f32; 4] {
        let e = s.node(emitter).map(|n| n.extra.clone()).unwrap_or(Value::Null);
        let t = (p.age / p.life.max(1e-6)).clamp(0.0, 1.0);
        let (a, b) = (rgb(&e, "color"), rgb(&e, "endColor"));
        let c = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
        [c[0] as f32, c[1] as f32, c[2] as f32, (1.0 - t * t) as f32]
    }
}

fn kind_of(s: &Scene, id: &str, key: &str) -> Option<String> {
    s.node(id)?.extra.get(key)?.as_str().map(String::from)
}

impl Scene {
    pub fn emitters(&self) -> Vec<Id> {
        self.nodes.values().filter(|n| n.kind == "emitter").map(|n| n.id.clone()).collect()
    }
    pub fn fields(&self) -> Vec<Id> {
        self.nodes.values().filter(|n| n.kind == "field").map(|n| n.id.clone()).collect()
    }
    pub fn rigid_bodies(&self) -> Vec<Id> {
        self.nodes.values().filter(|n| n.mesh().is_some() && n.extra.get("rigidBody").is_some()).map(|n| n.id.clone()).collect()
    }
    pub fn has_dynamics(&self) -> bool {
        self.nodes.values().any(|n| n.kind == "emitter" || (n.mesh().is_some() && n.extra.get("rigidBody").is_some()))
    }

    /// Forget simulation state (after edits); rigid bodies return to their start positions.
    pub fn reset_dynamics(&mut self) {
        let init = std::mem::take(&mut self.dynamics.initial);
        for (id, xf) in init {
            if let Some(n) = self.nodes.get_mut(&id) {
                n.xf = xf;
            }
        }
        self.dynamics = Dynamics::default();
        self.rev += 1;
    }

    /// Bring the simulation to time `t` (called from `set_time`).
    pub fn step_dynamics(&mut self, t: f64) {
        if !self.has_dynamics() {
            return;
        }
        let start = self.time.start;
        let need_reset = match self.dynamics.frame {
            None => true,
            Some(cur) => t < cur - 1e-9 || t <= start,
        };
        if need_reset {
            self.reset_dynamics();
            self.dynamics.rng = 0x9E3779B97F4A7C15;
            self.dynamics.frame = Some(start);
            for id in self.rigid_bodies() {
                let xf = self.nodes[&id].xf.clone();
                self.dynamics.initial.insert(id, xf);
            }
            self.build_rigid();
        }
        let mut cur = self.dynamics.frame.unwrap_or(start);
        let fps = self.time.fps.max(1.0);
        let mut steps = 0;
        while cur + 0.5 < t && steps < 2000 {
            self.sim_step(1.0 / fps);
            cur += 1.0;
            steps += 1;
        }
        self.dynamics.frame = Some(cur.max(t.min(cur)));
        self.rev += 1;
    }

    fn build_rigid(&mut self) {
        let ids = self.rigid_bodies();
        if ids.is_empty() {
            return;
        }
        let mut w = rp::PhysicsWorld::default();
        let g = self.fields().into_iter().filter(|f| kind_of(self, f, "fieldType").as_deref() == Some("gravity")).map(|f| {
            let e = &self.nodes[&f].extra;
            DVec3::new(ef(e, "directionX", 0.0), ef(e, "directionY", -1.0), ef(e, "directionZ", 0.0)).normalize_or(-DVec3::Y) * ef(e, "magnitude", 9.8)
        }).fold(None, |acc: Option<DVec3>, x| Some(acc.unwrap_or(DVec3::ZERO) + x)).unwrap_or(DVec3::new(0.0, -9.81, 0.0));
        w.gravity = rp::Vector::new(g.x as f32, g.y as f32, g.z as f32);
        let mut bodies = vec![];
        for id in ids {
            let e = self.nodes[&id].extra.clone();
            let active = e["rigidBody"].as_str() == Some("active");
            let (scale, rot, pos) = self.world_matrix(&id).to_scale_rotation_translation();
            let pm = &self.nodes[&id].mesh().unwrap().mesh;
            let pts: Vec<rp::Vector> = pm.v.iter().map(|v| {
                let p = *v * scale;
                rp::Vector::new(p.x as f32, p.y as f32, p.z as f32)
            }).collect();
            let pose = rp::Pose::from_parts(rp::Vector::new(pos.x as f32, pos.y as f32, pos.z as f32), rp::Rotation::from_xyzw(rot.x as f32, rot.y as f32, rot.z as f32, rot.w as f32));
            let collider = if active {
                rp::ColliderBuilder::convex_hull(&pts).unwrap_or_else(|| rp::ColliderBuilder::ball(0.5))
            } else {
                let mut idx = vec![];
                for fi in 0..pm.f.len() {
                    let fc = &pm.f[fi];
                    for t in pm.triangulate_face(fi) {
                        idx.push([fc[t[0]], fc[t[1]], fc[t[2]]]);
                    }
                }
                rp::ColliderBuilder::trimesh(pts.clone(), idx).unwrap_or_else(|_| rp::ColliderBuilder::cuboid(0.5, 0.5, 0.5))
            };
            let collider = collider.restitution(ef(&e, "bounciness", 0.6) as f32).friction(ef(&e, "friction", 0.3) as f32).mass(ef(&e, "mass", 1.0).max(0.001) as f32);
            let body = if active { rp::RigidBodyBuilder::dynamic() } else { rp::RigidBodyBuilder::fixed() }.pose(pose);
            let (h, _) = w.insert(body, collider);
            if active {
                bodies.push((id, h, scale));
            }
        }
        self.dynamics.rigid = Some(Rigid { world: w, bodies });
    }

    fn sim_step(&mut self, dt: f64) {
        // ---- rigid bodies
        if let Some(r) = self.dynamics.rigid.as_mut() {
            r.world.integration_parameters.dt = dt as f32;
            r.world.step();
            let updates: Vec<(Id, DMat4)> = r
                .bodies
                .iter()
                .filter_map(|(id, h, scale)| {
                    let b = r.world.bodies.get(*h)?;
                    let t = b.translation();
                    let q = b.rotation();
                    Some((id.clone(), DMat4::from_scale_rotation_translation(*scale, DQuat::from_xyzw(q.x as f64, q.y as f64, q.z as f64, q.w as f64), DVec3::new(t.x as f64, t.y as f64, t.z as f64))))
                })
                .collect();
            for (id, m) in updates {
                self.set_world_matrix_quiet(&id, m);
            }
        }
        // ---- particles
        let fields: Vec<(String, DVec3, DVec3, Value)> = self.fields().into_iter().map(|f| {
            let wm = self.world_matrix(&f);
            let e = self.nodes[&f].extra.clone();
            let dir = wm.transform_vector3(DVec3::new(f_(&e, "directionX"), f_(&e, "directionY"), f_(&e, "directionZ"))).normalize_or_zero();
            (e["fieldType"].as_str().unwrap_or("gravity").to_string(), wm.transform_point3(DVec3::ZERO), dir, e)
        }).collect();
        fn f_(e: &Value, k: &str) -> f64 {
            e.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0)
        }
        let emitters = self.emitters();
        let frame = self.dynamics.frame.unwrap_or(0.0);
        for em in emitters {
            let e = self.nodes[&em].extra.clone();
            let wm = self.world_matrix(&em);
            let origin = wm.transform_point3(DVec3::ZERO);
            let dir = wm.transform_vector3(DVec3::new(ef(&e, "directionX", 0.0), ef(&e, "directionY", 1.0), ef(&e, "directionZ", 0.0))).normalize_or(DVec3::Y);
            let ty = e["emitterType"].as_str().unwrap_or("omni").to_string();
            let source = e["source"].as_str().and_then(|s| self.mesh_world(s));
            let carry = self.dynamics.emit_carry.get(&em).copied().unwrap_or(0.0) + ef(&e, "rate", 100.0).max(0.0) * dt;
            let n = carry.floor() as usize;
            self.dynamics.emit_carry.insert(em.clone(), carry - n as f64);
            let (speed, spread, life) = (ef(&e, "speed", 1.0), ef(&e, "spread", 0.2).clamp(0.0, 1.0), ef(&e, "lifespan", 3.0).max(0.01));
            let mut born = vec![];
            for _ in 0..n.min(10000) {
                let rd = self.dynamics.rand_dir();
                let (p, v) = match ty.as_str() {
                    "directional" => (origin, (dir + rd * spread).normalize_or(dir) * speed),
                    "volume" => {
                        let q = DVec3::new(self.dynamics.rand() - 0.5, self.dynamics.rand() - 0.5, self.dynamics.rand() - 0.5);
                        (wm.transform_point3(q), rd * speed * 0.2)
                    }
                    "surface" => match &source {
                        Some(pm) if !pm.f.is_empty() => {
                            let fi = ((self.dynamics.rand() * pm.f.len() as f64) as usize).min(pm.f.len() - 1);
                            let fc = &pm.f[fi];
                            let a = pm.v[fc[0] as usize];
                            let b = pm.v[fc[1 % fc.len()] as usize];
                            let c = pm.v[fc[2 % fc.len()] as usize];
                            let (mut u, mut w) = (self.dynamics.rand(), self.dynamics.rand());
                            if u + w > 1.0 {
                                u = 1.0 - u;
                                w = 1.0 - w;
                            }
                            let nrm = pm.face_normal(fi).normalize_or(DVec3::Y);
                            (a + (b - a) * u + (c - a) * w, (nrm + rd * spread).normalize_or(nrm) * speed)
                        }
                        _ => (origin, rd * speed),
                    },
                    _ => (origin, rd * speed),
                };
                let jitter = self.dynamics.rand();
                born.push(Particle { p, v, age: 0.0, life: life * (0.75 + 0.5 * jitter) });
            }
            let list = self.dynamics.particles.entry(em.clone()).or_default();
            list.extend(born);
            for pt in list.iter_mut() {
                let mut acc = DVec3::ZERO;
                for (ft, fp, fd, fe) in &fields {
                    let m = f_(fe, "magnitude");
                    let maxd = fe.get("maxDistance").and_then(|x| x.as_f64()).unwrap_or(-1.0);
                    let rel = pt.p - *fp;
                    let dist = rel.length();
                    if maxd > 0.0 && dist > maxd && ft != "gravity" {
                        continue;
                    }
                    let att = f_(fe, "attenuation");
                    let k = if att > 0.0 && ft != "gravity" { 1.0 / (1.0 + dist).powf(att) } else { 1.0 };
                    acc += match ft.as_str() {
                        "gravity" => fd.normalize_or(-DVec3::Y) * m,
                        "drag" => -pt.v * m,
                        "turbulence" => {
                            let fq = fe.get("frequency").and_then(|x| x.as_f64()).unwrap_or(1.0);
                            let q = pt.p * fq + DVec3::splat(frame * 0.05);
                            DVec3::new((q.y * 1.7 + q.z * 2.3).sin(), (q.z * 1.3 + q.x * 2.9).sin(), (q.x * 2.1 + q.y * 1.1).sin()) * m * k
                        }
                        "radial" => rel.normalize_or_zero() * m * k,
                        "vortex" => fd.normalize_or(DVec3::Y).cross(rel).normalize_or_zero() * m * k,
                        _ => (*fd * m - pt.v) * k, // air: pushes particles towards the air speed
                    };
                }
                pt.v += acc * dt;
                pt.p += pt.v * dt;
                pt.age += dt;
            }
            list.retain(|p| p.age < p.life);
        }
    }

    /// Set a world matrix without marking the scene modified (simulation playback).
    pub fn set_world_matrix_quiet(&mut self, id: &str, wm: DMat4) {
        let local = self.parent_world(id).inverse() * wm;
        let Some(n) = self.nodes.get_mut(id) else { return };
        let (s, q, pos) = local.to_scale_rotation_translation();
        let e = crate::math::euler_deg_from_quat(n.xf.jo_quat().inverse() * q, &n.xf.ro);
        let xf = &mut n.xf;
        xf.r = [0, 1, 2].map(|i| crate::math::near_angle(xf.r[i], e[i]));
        xf.s = s.to_array();
        let rs = DMat4::from_scale_rotation_translation(s, q, DVec3::ZERO);
        let rp = rs.transform_point3(DVec3::from_array(xf.p));
        xf.t = [0, 1, 2].map(|i| pos[i] - xf.pt[i] - xf.p[i] + rp[i]);
    }

    pub fn create_emitter(&mut self, ty: &str, name: &str) -> Id {
        let id = self.create_handle("emitter", name);
        self.nodes.get_mut(&id).unwrap().extra = emitter_defaults(ty);
        self.reset_dynamics();
        id
    }

    pub fn create_field(&mut self, ty: &str) -> Id {
        let id = self.create_handle("field", &format!("{ty}Field1"));
        self.nodes.get_mut(&id).unwrap().extra = field_defaults(ty);
        self.reset_dynamics();
        id
    }

    pub fn make_rigid(&mut self, id: &str, active: bool) {
        if let Some(n) = self.nodes.get_mut(id) {
            let mut e = match &n.extra {
                Value::Object(m) => m.clone(),
                _ => Default::default(),
            };
            if let Value::Object(d) = rigid_defaults(active) {
                for (k, v) in d {
                    e.insert(k, v);
                }
            }
            n.extra = Value::Object(e);
        }
        self.reset_dynamics();
        self.touch();
    }
}

/// Is this node a dynamics node (for icon drawing)?
pub fn is_dynamics_node(s: &Scene, id: &str) -> bool {
    s.node(id).is_some_and(|n| matches!(n.shape, Shape::Locator) && (n.kind == "emitter" || n.kind == "field"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;

    #[test]
    fn particles_fall_and_die() {
        let mut s = Scene::new();
        let e = s.create_emitter("omni", "emitter1");
        s.create_field("gravity");
        s.set_time(1.0);
        s.set_time(25.0);
        let n = s.dynamics.particle_count();
        assert!(n > 50, "{n}");
        let avg_y: f64 = s.dynamics.particles[&e].iter().map(|p| p.p.y).sum::<f64>() / n as f64;
        assert!(avg_y < -0.5, "gravity pulls particles down: {avg_y}");
        // scrubbing back re-simulates deterministically
        s.set_time(10.0);
        let a = s.dynamics.particle_count();
        s.set_time(25.0);
        s.set_time(10.0);
        assert_eq!(a, s.dynamics.particle_count());
    }

    #[test]
    fn rigid_body_lands_on_ground() {
        let mut s = Scene::new();
        let g = s.create_mesh("polyPlane", &Params::new(), None);
        s.set_attr_f(&g, "sx", 10.0);
        s.set_attr_f(&g, "sz", 10.0);
        s.make_rigid(&g, false);
        let c = s.create_mesh("polyCube", &Params::new(), None);
        s.set_attr_f(&c, "ty", 5.0);
        s.make_rigid(&c, true);
        s.set_time(1.0);
        s.set_time(100.0);
        let y = s.world_matrix(&c).transform_point3(DVec3::ZERO).y;
        assert!(y > 0.3 && y < 0.8, "cube rests on the ground: {y}");
        // back to the start restores the initial position
        s.set_time(1.0);
        assert_eq!(s.get_attr_f(&c, "ty"), 5.0);
    }
}
