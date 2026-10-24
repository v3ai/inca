//! Rigging: joint orient / insert / mirror, IK handles,
//! pole vector constraints, set driven keys and motion paths.
//!
//! Rig data uses the JS app's formats so scenes move between the two: structural data in the node's
//! `rig` record (kept in `passthrough`), keyable numbers in `extra` (saved as `extraAttrs` too).

use crate::anim::{AnimCurve, Key};
use crate::math::{euler_deg_from_quat, quat_from_euler_deg, DMat4, DQuat, DVec3};
use crate::scene::{Id, Scene};
use serde_json::{json, Map, Value};

/// Node kinds that are rig nodes (their keyable attributes live in `extra`).
pub const RIG_KINDS: &[&str] = &["joint", "ikHandle", "deformer", "deformHandle", "clusterHandle", "lattice", "constraint"];

fn v3(a: [f64; 3]) -> DVec3 {
    DVec3::from_array(a)
}

/// A rotation whose X axis points along `x` with Y towards `up` (the JS frameQuat).
pub fn frame_quat(x: DVec3, up: DVec3) -> DQuat {
    let xn = x.normalize_or(DVec3::X);
    let mut y = up - xn * up.dot(xn);
    if y.length_squared() < 1e-12 {
        y = DVec3::Z - xn * xn.z;
    }
    let y = y.normalize();
    let z = xn.cross(y);
    DQuat::from_mat3(&glam::DMat3::from_cols(xn, y, z))
}

impl Scene {
    pub fn rig(&self, id: &str) -> Value {
        self.nodes.get(id).and_then(|n| n.passthrough.get("rig")).cloned().unwrap_or(Value::Null)
    }
    pub fn set_rig(&mut self, id: &str, v: Value) {
        if let Some(n) = self.nodes.get_mut(id) {
            n.passthrough.insert("rig".into(), v);
        }
    }
    /// A keyable rig attribute (`extra`), or `def`.
    pub fn extra_f(&self, id: &str, k: &str, def: f64) -> f64 {
        self.nodes.get(id).and_then(|n| n.extra.get(k)).and_then(|v| v.as_f64().or_else(|| v.as_bool().map(|b| b as i32 as f64))).unwrap_or(def)
    }
    pub fn set_extra(&mut self, id: &str, k: &str, v: Value) {
        if let Some(n) = self.nodes.get_mut(id) {
            if !n.extra.is_object() {
                n.extra = json!({});
            }
            n.extra.as_object_mut().unwrap().insert(k.into(), v);
        }
    }
    pub fn world_pos(&self, id: &str) -> DVec3 {
        self.world_matrix(id).transform_point3(DVec3::ZERO)
    }

    /// Set channels for a local matrix without refreshing dependents (rig passes batch that).
    pub fn put_local(&mut self, id: &str, m: DMat4) {
        let Some(n) = self.nodes.get_mut(id) else { return };
        let (s, q, pos) = m.to_scale_rotation_translation();
        let e = euler_deg_from_quat(n.xf.jo_quat().inverse() * q, &n.xf.ro);
        let xf = &mut n.xf;
        xf.r = [0, 1, 2].map(|i| crate::math::near_angle(xf.r[i], e[i]));
        xf.s = s.to_array();
        let rs = DMat4::from_scale_rotation_translation(s, q, DVec3::ZERO);
        let rp = rs.transform_point3(v3(xf.p));
        xf.t = [0, 1, 2].map(|i| pos[i] - xf.pt[i] - xf.p[i] + rp[i]);
        self.rev += 1;
    }
    pub fn put_world(&mut self, id: &str, wm: DMat4) {
        let l = self.parent_world(id).inverse() * wm;
        self.put_local(id, l);
    }
    /// Rotate a node in world space about point `p`.
    fn rotate_world_about(&mut self, id: &str, p: DVec3, q: DQuat) {
        let w = self.world_matrix(id);
        let m = DMat4::from_translation(p) * DMat4::from_quat(q) * DMat4::from_translation(-p) * w;
        self.put_world(id, m);
    }

    // ------------------------------------------------------------ joints

    fn child_joints(&self, id: &str) -> Vec<Id> {
        self.children(Some(id)).iter().filter(|c| self.node(c).is_some_and(|n| n.kind == "joint")).cloned().collect()
    }

    /// Orient a joint: X towards its first child joint, Y towards world up; children stay put.
    pub fn orient_joint(&mut self, id: &str) {
        let kids: Vec<Id> = self.children(Some(id)).to_vec();
        let kids_w: Vec<DMat4> = kids.iter().map(|k| self.world_matrix(k)).collect();
        let pq = self.parent_world(id).to_scale_rotation_translation().1;
        let jk = self.child_joints(id);
        let q = match jk.first() {
            Some(k) => {
                let x = self.world_pos(k) - self.world_pos(id);
                if x.length_squared() < 1e-12 {
                    pq
                } else if x.normalize().dot(DVec3::Y).abs() < 0.99 {
                    frame_quat(x, DVec3::Y)
                } else {
                    // bone along up: keep Z on world Z (front-view chains bend about Z)
                    let xn = x.normalize();
                    let mut z = DVec3::Z - xn * xn.z;
                    if z.length_squared() < 1e-8 {
                        z = DVec3::X - xn * xn.x;
                    }
                    let z = z.normalize();
                    DQuat::from_mat3(&glam::DMat3::from_cols(xn, z.cross(xn), z))
                }
            }
            None => pq,
        };
        let ql = pq.inverse() * q;
        if let Some(n) = self.nodes.get_mut(id) {
            n.xf.r = [0.0; 3];
            n.xf.jo = euler_deg_from_quat(ql, "xyz").map(|a| (a * 1e6).round() / 1e6);
        }
        for (k, w) in kids.iter().zip(kids_w) {
            self.put_world(k, w);
        }
        self.touch();
    }

    pub fn orient_hierarchy(&mut self, roots: &[Id]) {
        let mut seen = vec![];
        for r in roots {
            for j in self.subtree(r) {
                if self.node(&j).is_some_and(|n| n.kind == "joint") && !seen.contains(&j) {
                    seen.push(j.clone());
                    self.orient_joint(&j);
                }
            }
        }
    }

    /// A new joint at `p` (world), parented under `parent`.
    pub fn create_joint_at(&mut self, p: DVec3, parent: Option<&str>, name: &str) -> Id {
        let id = self.create_handle("joint", name);
        if let Some(pa) = parent {
            self.reparent(&id, Some(pa), None);
        }
        let local = self.parent_world(&id).inverse().transform_point3(p);
        if let Some(n) = self.nodes.get_mut(&id) {
            n.xf.t = local.to_array();
        }
        id
    }

    /// Skeleton > Insert Joint: a joint halfway to the first child joint.
    pub fn insert_joint(&mut self, j: &str) -> Result<Id, String> {
        let kid = self.child_joints(j).into_iter().next().ok_or("The joint has no child joint")?;
        let mid = self.world_pos(j).lerp(self.world_pos(&kid), 0.5);
        let n = self.create_joint_at(mid, Some(j), "joint1");
        let kw = self.world_matrix(&kid);
        self.reparent(&kid, Some(&n), None);
        self.put_world(&kid, kw);
        self.orient_hierarchy(&[j.to_string()]);
        Ok(n)
    }

    /// Skeleton > Mirror Joints across the YZ / XZ / XY plane (axis 0 / 1 / 2), renaming search -> replace.
    pub fn mirror_joints(&mut self, root: &str, axis: usize, search: &str, replace: &str) -> Id {
        let mut map: Vec<(Id, Id)> = vec![];
        let root_parent = self.node(root).and_then(|n| n.parent.clone());
        for c in self.subtree(root) {
            if !self.node(&c).is_some_and(|n| n.kind == "joint") {
                continue;
            }
            let mut p = self.world_pos(&c);
            p[axis] = -p[axis];
            let parent = if c == root { root_parent.clone() } else { self.node(&c).and_then(|n| n.parent.clone()).and_then(|pp| map.iter().find(|m| m.0 == pp).map(|m| m.1.clone())) };
            let name = self.name_of(&c);
            let nm = if !search.is_empty() && name.contains(search) { name.replace(search, replace) } else { format!("{name}_mirror") };
            let n = self.create_joint_at(p, parent.as_deref(), &nm);
            map.push((c, n));
        }
        let new_root = map[0].1.clone();
        self.orient_hierarchy(std::slice::from_ref(&new_root));
        new_root
    }

    /// [start .. end] along the hierarchy, or None.
    pub fn joint_chain(&self, start: &str, end: &str) -> Option<Vec<Id>> {
        let mut path = vec![];
        let mut c = Some(end.to_string());
        while let Some(i) = c {
            path.insert(0, i.clone());
            if i == start {
                return Some(path);
            }
            c = self.node(&i).and_then(|n| n.parent.clone());
        }
        None
    }

    // ------------------------------------------------------------ IK

    /// Skeleton > Create IK Handle: rotate-plane solver for 3-joint chains, single-chain (CCD) otherwise.
    pub fn create_ik_handle(&mut self, start: &str, end: &str, solver: &str) -> Result<Id, String> {
        let chain = self.joint_chain(start, end).filter(|c| c.len() >= 2).ok_or("End joint must be a descendant of the start joint")?;
        let solver = if chain.len() == 3 { solver } else { "ikSCsolver" };
        let h = self.create_handle("ikHandle", "ikHandle1");
        let p = self.world_pos(end);
        if let Some(n) = self.nodes.get_mut(&h) {
            n.xf.t = p.to_array();
        }
        let rest: Map<String, Value> = chain.iter().map(|j| (j.clone(), json!(self.node(j).map(|n| n.xf.r).unwrap_or_default()))).collect();
        self.set_rig(&h, json!({ "start": start, "end": end, "chain": chain, "solver": solver, "rest": rest }));
        let mut ea = json!({ "ikBlend": 1.0, "twist": 0.0 });
        if solver == "ikRPsolver" {
            let (a, b, c) = (self.world_pos(&chain[0]), self.world_pos(&chain[1]), self.world_pos(&chain[2]));
            let ac = (c - a).normalize_or(DVec3::X);
            let mut pv = b - a;
            pv -= ac * pv.dot(ac);
            if pv.length_squared() < 1e-8 {
                let z = self.world_matrix(&chain[0]).to_scale_rotation_translation().1 * DVec3::Z;
                pv = z.cross(ac);
            }
            let pv = pv.normalize_or(DVec3::Y);
            let o = ea.as_object_mut().unwrap();
            o.insert("poleVectorX".into(), json!(pv.x));
            o.insert("poleVectorY".into(), json!(pv.y));
            o.insert("poleVectorZ".into(), json!(pv.z));
        }
        self.nodes.get_mut(&h).unwrap().extra = ea;
        self.touch();
        Ok(h)
    }

    /// Solve one IK handle from the chain's rest pose (stateless: undo and scrubbing safe).
    fn solve_ik(&mut self, h: &str) -> Vec<Id> {
        let rig = self.rig(h);
        let chain: Vec<Id> = rig["chain"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        if chain.len() < 2 || chain.iter().any(|j| !self.nodes.contains_key(j)) {
            return vec![];
        }
        let before: Vec<[f64; 3]> = chain.iter().map(|j| self.nodes[j].xf.r).collect();
        for j in &chain {
            if let Some(r) = rig["rest"].get(j).and_then(|v| serde_json::from_value::<[f64; 3]>(v.clone()).ok()) {
                self.nodes.get_mut(j).unwrap().xf.r = r;
            }
        }
        let fk = before.clone();
        let blend = self.extra_f(h, "ikBlend", 1.0).clamp(0.0, 1.0);
        if blend > 0.0 {
            let t = self.world_pos(h);
            if rig["solver"] == "ikRPsolver" && chain.len() == 3 {
                let pole = DVec3::new(self.extra_f(h, "poleVectorX", 0.0), self.extra_f(h, "poleVectorY", 0.0), self.extra_f(h, "poleVectorZ", 0.0));
                let twist = self.extra_f(h, "twist", 0.0).to_radians();
                self.solve_two_bone(&chain, t, pole, twist);
            } else {
                self.solve_ccd(&chain, t);
            }
            if blend < 1.0 {
                for (i, j) in chain.iter().enumerate() {
                    let ro = self.nodes[j].xf.ro.clone();
                    let qi = quat_from_euler_deg(self.nodes[j].xf.r, &ro);
                    let qf = quat_from_euler_deg(fk[i], &ro);
                    self.nodes.get_mut(j).unwrap().xf.r = euler_deg_from_quat(qf.slerp(qi, blend), &ro);
                }
            }
        } else {
            for (i, j) in chain.iter().enumerate() {
                self.nodes.get_mut(j).unwrap().xf.r = fk[i];
            }
        }
        chain.iter().enumerate().filter(|(i, j)| self.nodes[*j].xf.r.iter().zip(before[*i]).any(|(a, b)| (a - b).abs() > 1e-6)).map(|(_, j)| j.clone()).collect()
    }

    fn solve_two_bone(&mut self, chain: &[Id], t: DVec3, pole: DVec3, twist: f64) {
        let (a, b, c) = (self.world_pos(&chain[0]), self.world_pos(&chain[1]), self.world_pos(&chain[2]));
        let (l1, l2) = ((b - a).length(), (c - b).length());
        if l1 < 1e-6 || l2 < 1e-6 {
            return;
        }
        let to_t = t - a;
        let d0 = to_t.length();
        if d0 < 1e-6 {
            return;
        }
        let u = to_t / d0;
        let d = d0.clamp((l1 - l2).abs() + 1e-5, l1 + l2 - 1e-5);
        let mut pv = if twist != 0.0 { DQuat::from_axis_angle(u, twist) * pole } else { pole };
        pv -= u * pv.dot(u);
        if pv.length_squared() < 1e-10 {
            pv = b - a;
            pv -= u * pv.dot(u);
            if pv.length_squared() < 1e-10 {
                pv = DVec3::Y - u * u.y;
            }
        }
        let n = pv.normalize();
        let cos_a = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
        let sin_a = (1.0 - cos_a * cos_a).sqrt();
        let bn = a + u * (l1 * cos_a) + n * (l1 * sin_a);
        // map the rest (bone, bend normal) frame onto the solved one
        let br = (b - a).normalize();
        let mut nr = (c - a).cross(b - a);
        if nr.length_squared() < 1e-10 {
            nr = self.world_matrix(&chain[0]).to_scale_rotation_translation().1 * DVec3::Z;
        }
        let nr = nr.normalize();
        let bnn = (bn - a).normalize();
        let nn = u.cross(n).normalize();
        let fr = |b: DVec3, m: DVec3| {
            let mm = (m - b * m.dot(b)).normalize();
            glam::DMat3::from_cols(b, mm, b.cross(mm))
        };
        let r1 = fr(bnn, nn) * fr(br, nr).transpose();
        self.rotate_world_about(&chain[0], a, DQuat::from_mat3(&r1));
        let (b1, c1) = (self.world_pos(&chain[1]), self.world_pos(&chain[2]));
        let tc = a + u * d;
        let q2 = DQuat::from_rotation_arc((c1 - b1).normalize(), (tc - b1).normalize());
        self.rotate_world_about(&chain[1], b1, q2);
    }

    fn solve_ccd(&mut self, chain: &[Id], t: DVec3) {
        let end = chain.last().unwrap().clone();
        for _ in 0..64 {
            for i in (0..chain.len() - 1).rev() {
                let p = self.world_pos(&chain[i]);
                let (ea, tb) = (self.world_pos(&end) - p, t - p);
                if ea.length_squared() < 1e-12 || tb.length_squared() < 1e-12 {
                    continue;
                }
                let q = DQuat::from_rotation_arc(ea.normalize(), tb.normalize());
                if q.w.abs() > 1.0 - 1e-14 {
                    continue;
                }
                self.rotate_world_about(&chain[i], p, q);
            }
            if (self.world_pos(&end) - t).length() < 1e-4 {
                break;
            }
        }
    }

    // ------------------------------------------------------------ set driven keys

    /// Key `driven.attr` against the current value of `driver.driver_attr` (stored in the driven node's rig.sdk).
    pub fn set_driven_key(&mut self, driven: &str, attr: &str, driver: &str, driver_attr: &str) {
        let x = self.get_attr_f(driver, driver_attr);
        let v = self.get_attr_f(driven, attr);
        let mut rig = self.rig(driven);
        if !rig.is_object() {
            rig = json!({});
        }
        let list = rig.as_object_mut().unwrap().entry("sdk").or_insert(json!([]));
        let arr = list.as_array_mut().unwrap();
        let idx = match arr.iter().position(|e| e["attr"] == attr) {
            Some(i) => i,
            None => {
                arr.push(json!({ "attr": attr, "driver": driver, "driverAttr": driver_attr, "keys": [], "pre": "constant", "post": "constant" }));
                arr.len() - 1
            }
        };
        let e = &mut arr[idx];
        e["driver"] = json!(driver);
        e["driverAttr"] = json!(driver_attr);
        let mut c = AnimCurve::new(driven, attr);
        c.keys = serde_json::from_value::<Vec<Key>>(e["keys"].clone()).unwrap_or_default();
        c.set_key(x, v, "auto");
        e["keys"] = serde_json::to_value(&c.keys).unwrap_or_default();
        self.set_rig(driven, rig);
        self.touch();
    }

    fn eval_sdk(&mut self) -> Vec<Id> {
        let mut moved = vec![];
        let with: Vec<(Id, Value)> = self.nodes.values().filter_map(|n| n.passthrough.get("rig").and_then(|r| r.get("sdk")).map(|s| (n.id.clone(), s.clone()))).collect();
        for (id, list) in with {
            for e in list.as_array().into_iter().flatten() {
                let (Some(drv), Some(da), Some(attr)) = (e["driver"].as_str(), e["driverAttr"].as_str(), e["attr"].as_str()) else { continue };
                if !self.nodes.contains_key(drv) {
                    continue;
                }
                let mut c = AnimCurve::new(&id, attr);
                c.keys = serde_json::from_value::<Vec<Key>>(e["keys"].clone()).unwrap_or_default();
                if c.keys.is_empty() {
                    continue;
                }
                let x = self.get_attr_f(drv, da);
                let v = c.eval(x);
                let v = if attr == "visibility" { (v >= 0.5) as i32 as f64 } else { v };
                if (self.get_attr_f(&id, attr) - v).abs() > 1e-7 {
                    self.set_attr_ex(&crate::scene::NodeRef::Dag(id.clone()), attr, json!(v), false);
                    moved.push(id.clone());
                }
            }
        }
        moved
    }

    // ------------------------------------------------------------ motion paths

    /// Animate → Motion Paths → Attach to Motion Path: uValue keyed 0..1 over the playback range.
    pub fn attach_motion_path(&mut self, id: &str, curve: &str, follow: bool, front: &str, up: &str) {
        let mut rig = self.rig(id);
        if !rig.is_object() {
            rig = json!({});
        }
        let name = self.unique_name("motionPath1");
        rig["motionPath"] = json!({ "name": name, "curve": curve, "follow": follow, "frontAxis": front, "upAxis": up, "worldUpVector": [0, 1, 0], "fractionMode": true });
        self.set_rig(id, rig);
        self.set_extra(id, "uValue", json!(0.0));
        let (t0, t1) = (self.time.start, self.time.end);
        if let Some(c) = self.anim.curve_mut(id, "uValue", true) {
            c.set_key(t0, 0.0, "auto");
            c.set_key(t1, 1.0, "auto");
        }
        self.apply_anim();
        self.touch();
    }

    fn eval_motion_paths(&mut self) -> Vec<Id> {
        let mut moved = vec![];
        let with: Vec<(Id, Value)> = self.nodes.values().filter_map(|n| n.passthrough.get("rig").and_then(|r| r.get("motionPath")).map(|m| (n.id.clone(), m.clone()))).collect();
        for (id, mp) in with {
            let Some(cid) = mp["curve"].as_str() else { continue };
            let Some(crate::scene::Shape::Curve(c)) = self.node(cid).map(|n| n.shape.clone()) else { continue };
            let wc = c.transformed(&self.world_matrix(cid));
            let pts = wc.sample(16);
            if pts.len() < 2 {
                continue;
            }
            // arc-length parameter (fraction mode)
            let mut acc = vec![0.0];
            for w in pts.windows(2) {
                acc.push(acc.last().unwrap() + (w[1] - w[0]).length());
            }
            let total = acc.last().copied().unwrap_or(0.0).max(1e-9);
            let u = self.extra_f(&id, "uValue", 0.0).clamp(0.0, 1.0) * total;
            let i = acc.iter().position(|&a| a >= u).unwrap_or(acc.len() - 1).max(1);
            let f = ((u - acc[i - 1]) / (acc[i] - acc[i - 1]).max(1e-12)).clamp(0.0, 1.0);
            let p = pts[i - 1].lerp(pts[i], f);
            let tangent = (pts[i] - pts[i - 1]).normalize_or(DVec3::X);
            let cur = self.world_matrix(&id);
            let (s, mut q, _) = cur.to_scale_rotation_translation();
            if mp["follow"].as_bool().unwrap_or(true) {
                let axis = |a: &str| match a {
                    "y" => DVec3::Y,
                    "z" => DVec3::Z,
                    _ => DVec3::X,
                };
                let (fa, ua) = (axis(mp["frontAxis"].as_str().unwrap_or("x")), axis(mp["upAxis"].as_str().unwrap_or("y")));
                // object front axis along the tangent, up axis towards world up
                let frame = frame_quat(tangent, DVec3::Y);
                let local = frame_quat(fa, ua);
                q = frame * local.inverse();
            }
            let m = DMat4::from_scale_rotation_translation(s, q, p);
            if (m - cur).to_cols_array().iter().any(|x| x.abs() > 1e-9) {
                self.put_world(&id, m);
                moved.push(id);
            }
        }
        moved
    }

    // ------------------------------------------------------------ pole vectors, removal, the pass

    fn eval_pole_vectors(&mut self) -> Vec<Id> {
        let mut moved = vec![];
        for c in self.constraints.clone().iter().filter(|c| c.kind == "poleVector") {
            if !self.nodes.contains_key(&c.driven) || c.targets.iter().any(|t| !self.nodes.contains_key(t)) || c.targets.is_empty() {
                continue;
            }
            let Some(st) = self.rig(&c.driven)["start"].as_str().map(String::from).filter(|s| self.nodes.contains_key(s)) else { continue };
            let p = c.targets.iter().map(|t| self.world_pivot(t)).sum::<DVec3>() / c.targets.len() as f64;
            let v = p - self.world_pos(&st);
            let old = DVec3::new(self.extra_f(&c.driven, "poleVectorX", 0.0), self.extra_f(&c.driven, "poleVectorY", 0.0), self.extra_f(&c.driven, "poleVectorZ", 0.0));
            if (old - v).length() > 1e-6 {
                self.set_extra(&c.driven, "poleVectorX", json!(v.x));
                self.set_extra(&c.driven, "poleVectorY", json!(v.y));
                self.set_extra(&c.driven, "poleVectorZ", json!(v.z));
                moved.push(c.driven.clone());
            }
        }
        moved
    }

    /// Constrain > Remove Constraints: constraints driving (or targeting nothing but) the given nodes.
    pub fn remove_constraints(&mut self, ids: &[Id]) -> usize {
        let n = self.constraints.len();
        self.constraints.retain(|c| !ids.contains(&c.driven) && !ids.contains(&c.id));
        self.touch();
        n - self.constraints.len()
    }

    /// Before the constraints: driven keys and motion paths.
    /// Distance Tool / distanceDimension: two locators and a node that shows the distance between them.
    pub fn create_distance(&mut self, p0: DVec3, p1: DVec3) -> (Id, Id, Id) {
        let l0 = self.create_locator("locator1");
        self.nodes.get_mut(&l0).unwrap().xf.t = p0.to_array();
        let l1 = self.create_locator("locator1");
        self.nodes.get_mut(&l1).unwrap().xf.t = p1.to_array();
        let d = self.create_handle("distanceDimension", "distanceDimension1");
        self.set_rig(&d, json!({ "start": l0, "end": l1 }));
        self.set_extra(&d, "distance", json!((p1 - p0).length()));
        (d, l0, l1)
    }

    /// World end points and length of a distance dimension.
    pub fn distance_of(&self, id: &str) -> Option<(DVec3, DVec3, f64)> {
        let r = self.rig(id);
        let (a, b) = (r["start"].as_str()?, r["end"].as_str()?);
        if self.node(a).is_none() || self.node(b).is_none() {
            return None;
        }
        let (p, q) = (self.world_pos(a), self.world_pos(b));
        Some((p, q, (q - p).length()))
    }

    fn eval_distances(&mut self) {
        let ids: Vec<Id> = self.nodes.values().filter(|n| n.kind == "distanceDimension").map(|n| n.id.clone()).collect();
        for id in ids {
            if let Some((_, _, d)) = self.distance_of(&id) {
                if (self.extra_f(&id, "distance", -1.0) - d).abs() > 1e-12 {
                    if let Some(n) = self.nodes.get_mut(&id) {
                        n.extra["distance"] = json!(d);
                    }
                }
            }
        }
    }

    /// instance: a new transform whose mesh follows the source's shape.
    pub fn make_instance(&mut self, src: &str) -> Option<Id> {
        let n = self.node(src)?;
        n.mesh()?;
        let d = self.duplicate(&[src.to_string()], false).into_iter().next()?;
        if let Some(m) = self.nodes.get_mut(&d) {
            m.passthrough.insert("instanceOf".into(), json!(src));
            m.passthrough.insert("instanceRev".into(), json!(-1));
            if let Some(ms) = m.mesh_mut() {
                ms.history.clear();
            }
        }
        self.sync_instances();
        Some(d)
    }

    /// Copy the source mesh into instances whose source changed since the last sync.
    pub fn sync_instances(&mut self) -> Vec<Id> {
        let pairs: Vec<(Id, Id)> = self.nodes.values().filter_map(|n| Some((n.id.clone(), n.passthrough.get("instanceOf")?.as_str()?.to_string()))).collect();
        let mut changed = vec![];
        for (inst, src) in pairs {
            let Some(sn) = self.node(&src) else { continue };
            let rev = sn.rev as i64;
            if self.node(&inst).and_then(|n| n.passthrough.get("instanceRev")).and_then(|v| v.as_i64()) == Some(rev) {
                continue;
            }
            let Some(sm) = sn.mesh().map(|m| (m.mesh.clone(), m.material.clone(), m.smooth_level)) else { continue };
            if let Some(n) = self.nodes.get_mut(&inst) {
                if let Some(ms) = n.mesh_mut() {
                    ms.mesh = sm.0;
                    ms.material = sm.1;
                    ms.smooth_level = sm.2;
                }
                n.passthrough.insert("instanceRev".into(), json!(rev));
                n.rev += 1;
                changed.push(inst);
            }
        }
        changed
    }

    pub fn eval_rig_pre(&mut self) -> Vec<Id> {
        self.eval_distances();
        let mut moved = self.sync_instances();
        moved.extend(self.eval_sdk());
        moved.extend(self.eval_motion_paths());
        moved
    }

    /// After the constraints: pole vectors, then IK.
    pub fn eval_rig_post(&mut self) -> Vec<Id> {
        let mut moved = self.eval_pole_vectors();
        let handles: Vec<Id> = self.nodes.values().filter(|n| n.kind == "ikHandle").map(|n| n.id.clone()).collect();
        for h in handles {
            moved.extend(self.solve_ik(&h));
        }
        if !self.characters.is_empty() {
            moved.extend(self.eval_hik());
        }
        moved
    }

    /// Remove history nodes of the given types from a mesh (Unbind Skin, Delete Deformers).
    pub fn remove_history_types(&mut self, id: &str, types: &[&str]) -> usize {
        let Some(m) = self.nodes.get(id).and_then(|n| n.mesh()) else { return 0 };
        let first = m.history.iter().position(|h| types.contains(&h.ty.as_str()));
        let Some(first) = first.filter(|f| *f > 0) else { return 0 };
        let names: Vec<String> = m.history.iter().filter(|h| types.contains(&h.ty.as_str())).map(|h| h.name.clone()).collect();
        for n in &names {
            self.release_name(n);
        }
        let m = self.nodes.get_mut(id).unwrap().mesh_mut().unwrap();
        m.history.retain(|h| !types.contains(&h.ty.as_str()));
        self.evaluate(id, first);
        names.len()
    }

    /// Deformer handles no mesh history uses any more.
    pub fn orphan_deformer_handles(&self) -> Vec<Id> {
        let used: std::collections::HashSet<String> = self.nodes.values().filter_map(|n| n.mesh()).flat_map(|m| m.history.iter().filter_map(|h| h.params.get("handle").and_then(|v| v.as_str()).map(String::from))).collect();
        self.nodes.values().filter(|n| matches!(n.kind.as_str(), "deformer" | "clusterHandle" | "lattice") && !used.contains(&n.id)).map(|n| n.id.clone()).collect()
    }

    /// After loading: turn the JS app's rig records into this app's rig (keyable attributes into
    /// `extra`, deformers / skins / blend shapes into history ops, constraint nodes into constraints).
    pub fn import_js_rig(&mut self) {
        // keyable attributes
        for n in self.nodes.values_mut() {
            if let Some(Value::Object(ea)) = n.passthrough.remove("extraAttrs") {
                if !n.extra.is_object() {
                    n.extra = json!({});
                }
                let e = n.extra.as_object_mut().unwrap();
                for (k, v) in ea {
                    e.entry(k).or_insert(v);
                }
            }
        }
        let ids: Vec<Id> = self.nodes.keys().cloned().collect();
        for id in ids {
            let rig = self.rig(&id);
            if !rig.is_object() || rig.get("imported").is_some() {
                continue;
            }
            let kind = self.nodes[&id].kind.clone();
            // a deformer this app made (or imported before) is already in mesh history
            let in_history = self.nodes.values().filter_map(|n| n.mesh()).any(|m| m.history.iter().any(|h| h.params.get("handle").and_then(|v| v.as_str()) == Some(id.as_str())));
            if in_history && rig.get("skin").is_none() && rig.get("blendShape").is_none() {
                continue;
            }
            let targets: Vec<Id> = rig["targets"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).filter(|t| self.nodes.contains_key(t)).collect()).unwrap_or_default();
            let mut done = false;
            match (kind.as_str(), rig["deformer"].as_str()) {
                ("deformHandle", Some(t)) if crate::deformers::KINDS.contains(&t) => {
                    let mut p = crate::deformers::defaults(t);
                    if let Some(e) = self.nodes[&id].extra.as_object() {
                        for (k, v) in e {
                            if p.contains_key(k) {
                                p.insert(k.clone(), v.clone());
                            }
                        }
                    }
                    p.insert("handle".into(), json!(id));
                    for m in &targets {
                        let mut pm = p.clone();
                        pm.insert("owner".into(), json!(m));
                        self.add_history(m, t, pm);
                    }
                    let n = self.nodes.get_mut(&id).unwrap();
                    n.kind = "deformer".into();
                    n.extra = json!({ "deformer": t });
                    done = true;
                }
                ("clusterHandle", Some(t @ ("cluster" | "softMod"))) => {
                    for m in &targets {
                        let mut p = crate::params::Params::new();
                        p.insert("handle".into(), json!(id));
                        p.insert("owner".into(), json!(m));
                        p.insert("bind".into(), rig["bind"].clone());
                        p.insert("envelope".into(), json!(self.extra_f(&id, "envelope", 1.0)));
                        if t == "cluster" {
                            p.insert("weights".into(), rig["weights"].get(m).cloned().unwrap_or(json!({})));
                        } else {
                            p.insert("center".into(), rig["center"].clone());
                            p.insert("falloffRadius".into(), json!(self.extra_f(&id, "falloffRadius", 5.0)));
                        }
                        self.add_history(m, t, p);
                    }
                    done = true;
                }
                ("lattice", Some("ffd")) => {
                    for m in &targets {
                        let mut p = crate::params::Params::new();
                        p.insert("handle".into(), json!(id));
                        p.insert("owner".into(), json!(m));
                        p.insert("envelope".into(), json!(1.0));
                        self.add_history(m, "ffd", p);
                    }
                    done = true;
                }
                ("constraint", _) => {
                    // the constrained object is the constraint node's parent
                    if let (Some(driven), Some(t)) = (self.nodes[&id].parent.clone(), rig["type"].as_str()) {
                        let tg: Vec<Id> = rig["targets"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).filter(|t| self.nodes.contains_key(t)).collect()).unwrap_or_default();
                        if !tg.is_empty() {
                            self.add_constraint(t, tg, &driven, rig["maintainOffset"].as_bool().unwrap_or(false));
                        }
                    }
                    self.delete_node(&id);
                    continue;
                }
                _ => {}
            }
            // skins and blend shapes live on the mesh
            if let Some(sk) = rig.get("skin").filter(|_| self.nodes[&id].mesh().is_some()) {
                let w: Vec<Value> = sk["w"].as_array().into_iter().flatten().map(|v| {
                    let a: Vec<f64> = v.as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).collect()).unwrap_or_default();
                    json!(a.chunks(2).filter(|c| c.len() == 2).map(|c| json!([c[0] as u64, c[1]])).collect::<Vec<_>>())
                }).collect();
                let mut p = crate::params::Params::new();
                p.insert("joints".into(), sk["joints"].clone());
                p.insert("bindInv".into(), sk["bindInv"].clone());
                p.insert("meshBind".into(), sk["bindShape"].clone());
                p.insert("weights".into(), Value::Array(w));
                p.insert("owner".into(), json!(id));
                self.add_history(&id, "skinCluster", p);
                done = true;
            }
            if let Some(bs) = rig.get("blendShape").filter(|_| self.nodes[&id].mesh().is_some()) {
                let mut p = crate::params::Params::new();
                p.insert("owner".into(), json!(id));
                p.insert("targets".into(), bs["targets"].clone());
                p.insert("envelope".into(), json!(bs["envelope"].as_f64().unwrap_or(1.0)));
                self.add_history(&id, "blendShape", p);
                done = true;
            }
            if done {
                let mut r = rig.clone();
                let o = r.as_object_mut().unwrap();
                o.remove("skin");
                o.remove("blendShape");
                o.insert("imported".into(), json!(true));
                self.set_rig(&id, r);
            }
        }
        let moved = self.eval_constraints();
        self.refresh_many(&moved);
    }

    /// The whole rig pass (eval_constraints runs it around the constraints).
    pub fn eval_rig(&mut self) -> Vec<Id> {
        self.eval_constraints()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arm(s: &mut Scene) -> (Id, Id, Id) {
        let a = s.create_joint_at(DVec3::ZERO, None, "a");
        let b = s.create_joint_at(DVec3::new(2.0, 0.0, -0.2), Some(&a), "b");
        let c = s.create_joint_at(DVec3::new(4.0, 0.0, 0.0), Some(&b), "c");
        s.orient_hierarchy(std::slice::from_ref(&a));
        (a, b, c)
    }

    #[test]
    fn orient_keeps_positions_and_points_x_at_child() {
        let mut s = Scene::new();
        let (a, b, c) = arm(&mut s);
        assert!((s.world_pos(&b) - DVec3::new(2.0, 0.0, -0.2)).length() < 1e-9);
        assert!((s.world_pos(&c) - DVec3::new(4.0, 0.0, 0.0)).length() < 1e-9);
        let x = s.world_matrix(&a).to_scale_rotation_translation().1 * DVec3::X;
        assert!((x - (s.world_pos(&b) - s.world_pos(&a)).normalize()).length() < 1e-6);
    }

    #[test]
    fn two_bone_ik_reaches_the_target() {
        let mut s = Scene::new();
        let (a, _b, c) = arm(&mut s);
        let h = s.create_ik_handle(&a, &c, "ikRPsolver").unwrap();
        s.nodes.get_mut(&h).unwrap().xf.t = [2.5, 1.5, 0.0];
        s.eval_rig();
        assert!((s.world_pos(&c) - DVec3::new(2.5, 1.5, 0.0)).length() < 1e-4, "{:?}", s.world_pos(&c));
        // out of reach: the chain straightens towards it
        s.nodes.get_mut(&h).unwrap().xf.t = [0.0, 10.0, 0.0];
        s.eval_rig();
        assert!((s.world_pos(&c) - s.world_pos(&a)).normalize().dot(DVec3::Y) > 0.999);
    }

    #[test]
    fn ccd_reaches_with_four_joints() {
        let mut s = Scene::new();
        let a = s.create_joint_at(DVec3::ZERO, None, "a");
        let b = s.create_joint_at(DVec3::new(1.0, 0.0, 0.0), Some(&a), "b");
        let c = s.create_joint_at(DVec3::new(2.0, 0.0, 0.0), Some(&b), "c");
        let d = s.create_joint_at(DVec3::new(3.0, 0.0, 0.0), Some(&c), "d");
        let h = s.create_ik_handle(&a, &d, "ikRPsolver").unwrap();
        assert_eq!(s.rig(&h)["solver"], "ikSCsolver");
        s.nodes.get_mut(&h).unwrap().xf.t = [1.5, 1.5, 0.0];
        s.eval_rig();
        assert!((s.world_pos(&d) - DVec3::new(1.5, 1.5, 0.0)).length() < 1e-3, "{:?}", s.world_pos(&d));
    }

    #[test]
    fn driven_keys_follow_the_driver() {
        let mut s = Scene::new();
        let drv = s.create_handle("locator", "drv");
        let dn = s.create_handle("locator", "dn");
        s.set_driven_key(&dn, "translateY", &drv, "translateX");
        s.nodes.get_mut(&drv).unwrap().xf.t[0] = 10.0;
        s.nodes.get_mut(&dn).unwrap().xf.t[1] = 5.0;
        s.set_driven_key(&dn, "translateY", &drv, "translateX");
        s.nodes.get_mut(&drv).unwrap().xf.t[0] = 5.0;
        s.eval_rig();
        let y = s.nodes[&dn].xf.t[1];
        assert!(y > 0.5 && y < 4.5, "{y}");
    }

    #[test]
    fn imports_js_rig_records() {
        let mut s = Scene::new();
        let m = s.create_mesh("polyCube", &crate::params::Params::new(), Some("box"));
        // a JS cluster handle on vertex 0
        let h = s.create_handle("clusterHandle", "cluster1Handle");
        let bind = s.world_matrix(&h).to_cols_array();
        s.set_rig(&h, json!({ "deformer": "cluster", "name": "cluster1", "targets": [m], "weights": { m.clone(): { "0": 1 } }, "bind": bind }));
        s.nodes.get_mut(&h).unwrap().passthrough.insert("extraAttrs".into(), json!({ "envelope": 1 }));
        // a JS point constraint node under the driven object
        let t = s.create_handle("locator", "target");
        s.nodes.get_mut(&t).unwrap().xf.t = [3.0, 0.0, 0.0];
        let d = s.create_handle("locator", "driven");
        let c = s.create_handle("constraint", "driven_pointConstraint1");
        s.reparent(&c, Some(&d), None);
        s.set_rig(&c, json!({ "type": "point", "targets": [t], "maintainOffset": false }));
        s.import_js_rig();
        assert_eq!(s.extra_f(&h, "envelope", 0.0), 1.0);
        assert!(s.node(&c).is_none() && s.constraints.len() == 1);
        assert!((s.world_pos(&d) - DVec3::new(3.0, 0.0, 0.0)).length() < 1e-9);
        let v0 = s.node(&m).unwrap().mesh().unwrap().mesh.v[0];
        s.nodes.get_mut(&h).unwrap().xf.t[1] = 2.0;
        s.refresh_dependents(&h);
        assert!((s.node(&m).unwrap().mesh().unwrap().mesh.v[0] - v0 - DVec3::new(0.0, 2.0, 0.0)).length() < 1e-9);
        // importing again changes nothing
        let n = s.node(&m).unwrap().mesh().unwrap().history.len();
        s.import_js_rig();
        assert_eq!(s.node(&m).unwrap().mesh().unwrap().history.len(), n);
    }

    #[test]
    fn mirror_and_insert() {
        let mut s = Scene::new();
        let (a, b, _c) = arm(&mut s);
        let m = s.mirror_joints(&a, 0, "a", "m");
        let kids = s.subtree(&m);
        assert_eq!(kids.len(), 3);
        assert!((s.world_pos(&kids[2]).x + 4.0).abs() < 1e-9);
        let n = s.insert_joint(&a).unwrap();
        assert!((s.world_pos(&n) - DVec3::new(1.0, 0.0, -0.1)).length() < 1e-9);
        assert!((s.world_pos(&b) - DVec3::new(2.0, 0.0, -0.2)).length() < 1e-9);
    }
}

impl Scene {
    /// Objects driven by `objs` through connections, constraints and rig links (the viewport's
    /// purple "affected" highlight), not counting `objs` themselves.
    pub fn affected_by(&self, objs: &[Id]) -> std::collections::HashSet<Id> {
        let owner = |id: &str| -> Option<Id> {
            match self.ref_by_id(id)? {
                crate::scene::NodeRef::Dag(i) | crate::scene::NodeRef::History(i, _) => Some(i),
                _ => None,
            }
        };
        let mut edges: Vec<(Id, Id)> = vec![];
        for c in &self.conns {
            if let (Some(a), Some(b)) = (owner(&c.src), owner(&c.dst)) {
                if a != b {
                    edges.push((a, b));
                }
            }
        }
        for c in &self.constraints {
            for t in &c.targets {
                if *t != c.driven {
                    edges.push((t.clone(), c.driven.clone()));
                }
            }
        }
        // deformer handles and IK handles drive what they deform / move
        for n in self.nodes.values() {
            if let Some(m) = n.mesh() {
                for h in &m.history {
                    for k in ["handle", "joints"] {
                        match h.params.get(k) {
                            Some(Value::String(s)) => edges.push((s.clone(), n.id.clone())),
                            Some(Value::Array(a)) => edges.extend(a.iter().filter_map(|x| x.as_str()).map(|s| (s.to_string(), n.id.clone()))),
                            _ => {}
                        }
                    }
                }
            }
            if n.kind == "ikHandle" {
                if let Some(ch) = self.rig(&n.id)["chain"].as_array() {
                    edges.extend(ch.iter().filter_map(|j| j.as_str()).map(|j| (n.id.clone(), j.to_string())));
                }
            }
        }
        let mut out = std::collections::HashSet::new();
        if edges.is_empty() {
            return out;
        }
        let mut todo: Vec<Id> = objs.to_vec();
        let mut seen: std::collections::HashSet<Id> = objs.iter().cloned().collect();
        while let Some(a) = todo.pop() {
            for (x, y) in &edges {
                if *x == a && seen.insert(y.clone()) {
                    out.insert(y.clone());
                    todo.push(y.clone());
                }
            }
        }
        out
    }
}
