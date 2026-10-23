//! Scene graph: DAG nodes (transform + shape), construction history, attributes, selection,
//! undo snapshots and the `.inca` JSON format (the same layout the JS app writes).

use crate::anim::{Anim, AnimCurve};
use crate::curves::Curve;
use crate::materials::{Material, ShadingData, Texture};
use crate::math::*;
use crate::mesh::{OpOut, PolyMesh};
use crate::ops::{self, OpCtx};
use crate::params::Params;
use crate::primitives;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashMap, HashSet};

pub type Id = String;

pub const CHANNELS: [&str; 10] = ["translateX", "translateY", "translateZ", "rotateX", "rotateY", "rotateZ", "scaleX", "scaleY", "scaleZ", "visibility"];

pub fn alias(attr: &str) -> &str {
    match attr {
        "tx" => "translateX",
        "ty" => "translateY",
        "tz" => "translateZ",
        "rx" => "rotateX",
        "ry" => "rotateY",
        "rz" => "rotateZ",
        "sx" => "scaleX",
        "sy" => "scaleY",
        "sz" => "scaleZ",
        "v" => "visibility",
        "t" => "translate",
        "r" => "rotate",
        "s" => "scale",
        "ro" => "rotateOrder",
        a => a,
    }
}

// ------------------------------------------------------------------ data

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Xform {
    pub t: [f64; 3],
    pub r: [f64; 3],
    pub s: [f64; 3],
    pub ro: String,
    /// pivot (rotate/scale pivot, local)
    pub p: [f64; 3],
    /// pivot translate (keeps the object in place when the pivot moves)
    pub pt: [f64; 3],
    /// joint orient (joints only): local = T * JO * R * S
    #[serde(default)]
    pub jo: [f64; 3],
}

impl Default for Xform {
    fn default() -> Self {
        Xform { t: [0.0; 3], r: [0.0; 3], s: [1.0; 3], ro: "xyz".into(), p: [0.0; 3], pt: [0.0; 3], jo: [0.0; 3] }
    }
}

impl Xform {
    /// local = T(t + pt + p) * R * S * T(-p)
    pub fn jo_quat(&self) -> DQuat {
        quat_from_euler_deg(self.jo, "xyz")
    }

    pub fn matrix(&self) -> DMat4 {
        let q = self.jo_quat() * quat_from_euler_deg(self.r, &self.ro);
        let rs = DMat4::from_scale_rotation_translation(v3(self.s), q, DVec3::ZERO);
        let p = v3(self.p);
        let rp = rs.transform_point3(p);
        let pos = v3(self.t) + v3(self.pt) + p - rp;
        DMat4::from_scale_rotation_translation(v3(self.s), q, pos)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoryNode {
    pub id: Id,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub params: Params,
    #[serde(skip)]
    pub out: Option<PolyMesh>,
    #[serde(skip)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeshShape {
    pub material: Id,
    pub smooth_level: u32,
    pub nurbs: bool,
    pub history: Vec<HistoryNode>,
    pub mesh: PolyMesh,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LightShape {
    pub ty: String,
    pub attrs: Params,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CamShape {
    pub focal_length: f64,
    pub near: f64,
    pub far: f64,
    pub ortho: bool,
    pub ortho_width: f64,
    pub coi: f64,
    pub horizontal_film_aperture: f64,
    pub vertical_film_aperture: f64,
    #[serde(default)]
    pub startup: bool,
}

impl Default for CamShape {
    fn default() -> Self {
        CamShape { focal_length: 35.0, near: 0.1, far: 10000.0, ortho: false, ortho_width: 30.0, coi: 10.0, horizontal_film_aperture: 1.417, vertical_film_aperture: 0.945, startup: false }
    }
}

impl CamShape {
    /// Vertical field of view (radians) for an aspect ratio, Maya's horizontal film fit.
    pub fn fov_y(&self, aspect: f64) -> f64 {
        let hfov = 2.0 * ((self.horizontal_film_aperture * 25.4 / 2.0) / self.focal_length).atan();
        2.0 * ((hfov / 2.0).tan() / aspect.max(1e-6)).atan()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Group,
    Locator,
    Mesh(Box<MeshShape>),
    Light(LightShape),
    Camera(CamShape),
    Curve(Curve),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: Id,
    pub name: String,
    /// "mesh" | "group" | "locator" | "light" | "camera" | "curve" | (rig kinds from the JS app)
    pub kind: String,
    pub parent: Option<Id>,
    pub children: Vec<Id>,
    pub xf: Xform,
    pub visible: bool,
    pub layer: Option<Id>,
    pub template: bool,
    pub locks: Map<String, Value>,
    pub shape_name: Option<String>,
    pub extra: Value,
    pub shape: Shape,
    /// data we don't interpret yet (rigging) but keep so files round-trip
    pub passthrough: Map<String, Value>,
    /// bumped whenever the shape's geometry changes (GPU caches key on it)
    pub rev: u64,
}

impl Node {
    pub fn startup(&self) -> bool {
        matches!(&self.shape, Shape::Camera(c) if c.startup)
    }
    pub fn mesh(&self) -> Option<&MeshShape> {
        match &self.shape {
            Shape::Mesh(m) => Some(m),
            _ => None,
        }
    }
    pub fn mesh_mut(&mut self) -> Option<&mut MeshShape> {
        match &mut self.shape {
            Shape::Mesh(m) => Some(m),
            _ => None,
        }
    }
    pub fn is_locked(&self, attr: &str) -> bool {
        self.locks.get(attr).and_then(|v| v.as_bool()).unwrap_or(false)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: Id,
    pub name: String,
    #[serde(default = "yes")]
    pub visible: bool,
    /// "normal" | "template" | "reference"
    #[serde(default = "normal")]
    pub mode: String,
    #[serde(default)]
    pub color: Option<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
fn yes() -> bool {
    true
}
fn normal() -> String {
    "normal".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeState {
    pub current: f64,
    pub start: f64,
    pub end: f64,
    pub anim_start: f64,
    pub anim_end: f64,
    pub fps: f64,
    #[serde(default)]
    pub playing: bool,
    #[serde(default = "continuous")]
    pub r#loop: String,
    #[serde(default)]
    pub auto_key: bool,
    #[serde(default)]
    pub play_every: bool,
}
fn continuous() -> String {
    "continuous".into()
}
impl Default for TimeState {
    fn default() -> Self {
        TimeState { current: 1.0, start: 1.0, end: 120.0, anim_start: 1.0, anim_end: 200.0, fps: 24.0, playing: false, r#loop: continuous(), auto_key: false, play_every: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompMode {
    Vertex,
    Edge,
    Face,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompSel {
    pub v: BTreeSet<u32>,
    pub e: BTreeSet<(u32, u32)>,
    pub f: BTreeSet<usize>,
}

impl CompSel {
    pub fn is_empty(&self) -> bool {
        self.v.is_empty() && self.e.is_empty() && self.f.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum MsgKind {
    Echo,
    Result,
    Warning,
    Error,
    Info,
}

/// A direct attribute connection (connectAttr).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Conn {
    pub src: Id,
    #[serde(rename = "srcAttr")]
    pub src_attr: String,
    pub dst: Id,
    #[serde(rename = "dstAttr")]
    pub dst_attr: String,
}

/// A transform constraint (point / orient / parent / scale / aim).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    pub id: Id,
    pub name: String,
    pub kind: String,
    pub driven: Id,
    pub targets: Vec<Id>,
    /// maintained offset (column-major matrix)
    #[serde(default)]
    pub offset: Option<[f64; 16]>,
    #[serde(default = "aim_x")]
    pub aim: [f64; 3],
    #[serde(default = "up_y")]
    pub up: [f64; 3],
}
fn aim_x() -> [f64; 3] {
    [1.0, 0.0, 0.0]
}
fn up_y() -> [f64; 3] {
    [0.0, 1.0, 0.0]
}

/// What a name refers to.
#[derive(Clone, Debug, PartialEq)]
pub enum NodeRef {
    Dag(Id),
    History(Id, usize),
    Material(Id),
    Texture(Id),
}

#[derive(Clone)]
pub struct Snapshot {
    pub label: String,
    pub data: Value,
}

pub struct Scene {
    pub nodes: IndexMap<Id, Node>,
    pub roots: Vec<Id>,
    pub names: HashSet<String>,
    pub mats: IndexMap<Id, Material>,
    pub texs: IndexMap<Id, Texture>,
    pub default_mat: Id,
    pub layers: Vec<Layer>,
    pub anim: Anim,
    pub conns: Vec<Conn>,
    pub constraints: Vec<Constraint>,
    pub time: TimeState,
    pub render: Params,
    pub sel: Vec<Id>,
    pub comp_mode: Option<CompMode>,
    pub comp: HashMap<Id, CompSel>,
    pub id_counter: u64,
    pub project: Option<String>,
    pub file: Option<std::path::PathBuf>,
    pub modified: bool,
    pub msgs: Vec<(MsgKind, String)>,
    pub undo_stack: Vec<Snapshot>,
    pub redo_stack: Vec<Snapshot>,
    pub undo_limit: usize,
    /// bumped on every change (UI caches)
    pub rev: u64,
    pub version: String,
    /// simulation state (not saved)
    pub dynamics: crate::dynamics::Dynamics,
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

fn shape_name_for(n: &str) -> String {
    let digits = n.len() - n.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if digits > 0 {
        format!("{}Shape{}", &n[..n.len() - digits], &n[n.len() - digits..])
    } else {
        format!("{n}Shape")
    }
}

impl Scene {
    pub fn new() -> Self {
        let mut s = Scene {
            nodes: IndexMap::new(),
            roots: vec![],
            names: HashSet::new(),
            mats: IndexMap::new(),
            texs: IndexMap::new(),
            default_mat: String::new(),
            layers: vec![],
            anim: Anim::default(),
            conns: vec![],
            constraints: vec![],
            time: TimeState::default(),
            render: Params::new(),
            sel: vec![],
            comp_mode: None,
            comp: HashMap::new(),
            id_counter: 1,
            project: None,
            file: None,
            modified: false,
            msgs: vec![],
            undo_stack: vec![],
            redo_stack: vec![],
            undo_limit: 100,
            rev: 0,
            version: env!("CARGO_PKG_VERSION").into(),
            dynamics: Default::default(),
        };
        s.init_default_materials();
        s.create_startup_cameras();
        s
    }

    // ------------------------------------------------------------ ids & names
    pub fn new_id(&mut self, prefix: &str) -> Id {
        let id = format!("{prefix}{}", self.id_counter);
        self.id_counter += 1;
        id
    }

    pub fn unique_name(&mut self, want: &str) -> String {
        let mut n: String = want.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == ':' { c } else { '_' }).collect();
        if n.is_empty() {
            n = "node".into();
        }
        if n.starts_with(|c: char| c.is_ascii_digit()) {
            n.insert(0, '_');
        }
        if !self.names.contains(&n) {
            self.names.insert(n.clone());
            return n;
        }
        let digits = n.len() - n.trim_end_matches(|c: char| c.is_ascii_digit()).len();
        let (stem, mut i) = if digits > 0 { (n[..n.len() - digits].to_string(), n[n.len() - digits..].parse::<u64>().unwrap_or(1)) } else { (n.clone(), 1) };
        while self.names.contains(&format!("{stem}{i}")) {
            i += 1;
        }
        let r = format!("{stem}{i}");
        self.names.insert(r.clone());
        r
    }
    pub fn release_name(&mut self, n: &str) {
        self.names.remove(n);
    }

    pub fn msg(&mut self, k: MsgKind, s: impl Into<String>) {
        self.msgs.push((k, s.into()));
    }
    pub fn echo(&mut self, s: impl Into<String>) {
        self.msg(MsgKind::Echo, s)
    }
    pub fn warn(&mut self, s: impl Into<String>) {
        self.msg(MsgKind::Warning, format!("// Warning: {}", s.into()))
    }

    pub fn touch(&mut self) {
        self.rev += 1;
        self.modified = true;
    }

    // ------------------------------------------------------------ materials
    pub fn init_default_materials(&mut self) {
        let lam = self.create_material("lambert", Some("lambert1"));
        self.default_mat = lam;
        self.create_material("standardSurface", Some("standardSurface1"));
    }
    pub fn create_material(&mut self, ty: &str, name: Option<&str>) -> Id {
        let id = self.new_id("m");
        let name = self.unique_name(name.unwrap_or(&format!("{ty}1")));
        self.mats.insert(id.clone(), Material::new(id.clone(), name, ty));
        self.touch();
        id
    }
    pub fn create_texture(&mut self, ty: &str, name: Option<&str>) -> Id {
        let id = self.new_id("x");
        let name = self.unique_name(name.unwrap_or(&format!("{ty}1")));
        self.texs.insert(id.clone(), Texture { id: id.clone(), name, ty: ty.into(), attrs: crate::materials::tex_defaults_for(ty) });
        self.touch();
        id
    }
    pub fn assign_material(&mut self, node: &str, mat: &str, faces: Option<&[usize]>) {
        let def = self.default_mat.clone();
        if let Some(n) = self.nodes.get_mut(node) {
            if let Some(m) = n.mesh_mut() {
                match faces {
                    None => {
                        m.material = mat.into();
                        m.mesh.fm = None;
                        bake_fm(m);
                    }
                    Some(fs) => {
                        let nf = m.mesh.f.len();
                        let fm = m.mesh.fm.get_or_insert_with(|| vec![None; nf]);
                        fm.resize(nf, None);
                        for &f in fs {
                            if f < nf {
                                fm[f] = if mat == m.material { None } else { Some(mat.to_string()) };
                            }
                        }
                        bake_fm(m);
                    }
                }
                n.rev += 1;
            }
        }
        let _ = def;
        self.touch();
    }
    pub fn delete_material(&mut self, id: &str) {
        if id == self.default_mat {
            return;
        }
        if let Some(m) = self.mats.shift_remove(id) {
            self.release_name(&m.name);
        }
        let def = self.default_mat.clone();
        for n in self.nodes.values_mut() {
            if let Some(m) = n.mesh_mut() {
                if m.material == id {
                    m.material = def.clone();
                }
                if let Some(fm) = &mut m.mesh.fm {
                    for x in fm.iter_mut() {
                        if x.as_deref() == Some(id) {
                            *x = None;
                        }
                    }
                }
                n.rev += 1;
            }
        }
        self.touch();
    }

    // ------------------------------------------------------------ DAG
    fn make_dag(&mut self, kind: &str, name: &str, parent: Option<&str>, shape: Shape, exact: bool, id: Option<Id>) -> Id {
        let id = id.unwrap_or_else(|| self.new_id("d"));
        let name = if exact {
            self.names.insert(name.to_string());
            name.to_string()
        } else {
            self.unique_name(name)
        };
        let shape_name = match &shape {
            Shape::Group => None,
            _ => Some(self.unique_name(&shape_name_for(&name))),
        };
        let node = Node {
            id: id.clone(),
            name,
            kind: kind.into(),
            parent: parent.map(String::from),
            children: vec![],
            xf: Xform::default(),
            visible: true,
            layer: None,
            template: false,
            locks: Map::new(),
            shape_name,
            extra: Value::Null,
            shape,
            passthrough: Map::new(),
            rev: 1,
        };
        match parent.and_then(|p| self.nodes.get_mut(p)) {
            Some(p) => p.children.push(id.clone()),
            None => self.roots.push(id.clone()),
        }
        self.nodes.insert(id.clone(), node);
        self.touch();
        id
    }

    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }
    pub fn node_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.touch();
        self.nodes.get_mut(id)
    }

    pub fn create_group(&mut self, name: &str, parent: Option<&str>) -> Id {
        self.make_dag("group", name, parent, Shape::Group, false, None)
    }
    pub fn create_locator(&mut self, name: &str) -> Id {
        self.make_dag("locator", name, None, Shape::Locator, false, None)
    }

    pub fn create_mesh(&mut self, gen: &str, params: &Params, name: Option<&str>) -> Id {
        let info = primitives::gen_info(gen);
        let base = name.map(String::from).unwrap_or_else(|| format!("{}1", info.map(|g| g.base).unwrap_or("polySurface")));
        let shape = Shape::Mesh(Box::new(MeshShape { material: self.default_mat.clone(), smooth_level: 0, nurbs: info.is_some_and(|g| g.nurbs), history: vec![], mesh: PolyMesh::default() }));
        let id = self.make_dag("mesh", &base, None, shape, false, None);
        let mut p = primitives::default_params(gen);
        for (k, v) in params {
            p.insert(k.clone(), v.clone());
        }
        self.add_history(&id, gen, p);
        id
    }

    /// A mesh without construction history (its data lives in a hidden meshData node).
    pub fn create_mesh_from(&mut self, pm: PolyMesh, name: &str, material: Option<&str>, parent: Option<&str>) -> Id {
        let mat = material.map(String::from).unwrap_or(self.default_mat.clone());
        let shape = Shape::Mesh(Box::new(MeshShape { material: mat, smooth_level: 0, nurbs: false, history: vec![], mesh: PolyMesh::default() }));
        let id = self.make_dag("mesh", name, parent, shape, false, None);
        self.bake_mesh(&id, pm);
        id
    }

    pub fn create_light(&mut self, ty: &str) -> Id {
        let attrs = match light_defaults(ty) {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        let id = self.make_dag("light", &format!("{ty}1"), None, Shape::Light(LightShape { ty: ty.into(), attrs }), false, None);
        if ty == "directionalLight" || ty == "spotLight" || ty == "areaLight" {
            let n = self.nodes.get_mut(&id).unwrap();
            if ty == "directionalLight" {
                n.xf.r = [-45.0, 30.0, 0.0];
            }
        }
        id
    }

    /// A deformer handle (drawn as a locator-like icon).
    pub fn create_handle(&mut self, kind: &str, name: &str) -> Id {
        let id = self.make_dag(kind, name, None, Shape::Locator, false, None);
        if let Some(n) = self.nodes.get_mut(&id) {
            n.shape_name = None;
        }
        id
    }

    pub fn create_camera(&mut self, name: &str, cam: CamShape, exact: bool) -> Id {
        self.make_dag("camera", name, None, Shape::Camera(cam), exact, None)
    }

    pub fn create_curve(&mut self, c: Curve, name: &str) -> Id {
        self.make_dag("curve", name, None, Shape::Curve(c), false, None)
    }

    pub fn create_startup_cameras(&mut self) {
        let mk = |s: &mut Scene, name: &str, t: [f64; 3], r: [f64; 3], cam: CamShape| {
            let id = s.create_camera(name, CamShape { startup: true, ..cam }, true);
            let n = s.nodes.get_mut(&id).unwrap();
            n.xf.t = t;
            n.xf.r = r;
        };
        mk(self, "persp", [28.0, 21.0, 28.0], [-27.938, 45.0, 0.0], CamShape { coi: 44.82, ..Default::default() });
        mk(self, "top", [0.0, 1000.1, 0.0], [-90.0, 0.0, 0.0], CamShape { ortho: true, coi: 1000.1, ..Default::default() });
        mk(self, "front", [0.0, 0.0, 1000.1], [0.0, 0.0, 0.0], CamShape { ortho: true, coi: 1000.1, ..Default::default() });
        mk(self, "side", [1000.1, 0.0, 0.0], [0.0, 90.0, 0.0], CamShape { ortho: true, coi: 1000.1, ..Default::default() });
    }

    pub fn find(&self, name: &str) -> Option<Id> {
        let name = name.trim_start_matches('|');
        let name = name.rsplit('|').next().unwrap_or(name);
        self.nodes.values().find(|n| n.name == name).or_else(|| self.nodes.values().find(|n| n.shape_name.as_deref() == Some(name))).map(|n| n.id.clone())
    }

    pub fn resolve(&self, name: &str) -> Option<NodeRef> {
        if let Some(id) = self.find(name) {
            return Some(NodeRef::Dag(id));
        }
        for n in self.nodes.values() {
            if let Some(m) = n.mesh() {
                if let Some(i) = m.history.iter().position(|h| h.name == name) {
                    return Some(NodeRef::History(n.id.clone(), i));
                }
            }
        }
        if let Some(m) = self.mats.values().find(|m| m.name == name) {
            return Some(NodeRef::Material(m.id.clone()));
        }
        self.texs.values().find(|t| t.name == name).map(|t| NodeRef::Texture(t.id.clone()))
    }

    pub fn ref_name(&self, r: &NodeRef) -> String {
        match r {
            NodeRef::Dag(id) => self.nodes.get(id).map(|n| n.name.clone()).unwrap_or_default(),
            NodeRef::History(id, i) => self.nodes.get(id).and_then(|n| n.mesh()).and_then(|m| m.history.get(*i)).map(|h| h.name.clone()).unwrap_or_default(),
            NodeRef::Material(id) => self.mats.get(id).map(|m| m.name.clone()).unwrap_or_default(),
            NodeRef::Texture(id) => self.texs.get(id).map(|m| m.name.clone()).unwrap_or_default(),
        }
    }

    pub fn name_of(&self, id: &str) -> String {
        self.nodes.get(id).map(|n| n.name.clone()).unwrap_or_default()
    }

    pub fn children(&self, id: Option<&str>) -> &[Id] {
        match id {
            None => &self.roots,
            Some(i) => self.nodes.get(i).map(|n| n.children.as_slice()).unwrap_or(&[]),
        }
    }

    /// Node and all of its descendants (depth first).
    pub fn subtree(&self, id: &str) -> Vec<Id> {
        let mut out = vec![];
        let mut st = vec![id.to_string()];
        while let Some(i) = st.pop() {
            if let Some(n) = self.nodes.get(&i) {
                out.push(i.clone());
                for c in n.children.iter().rev() {
                    st.push(c.clone());
                }
            }
        }
        out
    }

    /// All DAG nodes in hierarchy order (startup cameras optional).
    pub fn all_dag(&self, include_startup: bool) -> Vec<Id> {
        let mut out = vec![];
        for r in &self.roots {
            for id in self.subtree(r) {
                if include_startup || !self.nodes[&id].startup() {
                    out.push(id);
                }
            }
        }
        out
    }

    pub fn is_ancestor(&self, a: &str, b: &str) -> bool {
        let mut c = self.nodes.get(b).and_then(|n| n.parent.clone());
        while let Some(p) = c {
            if p == a {
                return true;
            }
            c = self.nodes.get(&p).and_then(|n| n.parent.clone());
        }
        false
    }

    pub fn full_path(&self, id: &str) -> String {
        let mut parts = vec![];
        let mut c = Some(id.to_string());
        while let Some(i) = c {
            let Some(n) = self.nodes.get(&i) else { break };
            parts.push(n.name.clone());
            c = n.parent.clone();
        }
        parts.reverse();
        format!("|{}", parts.join("|"))
    }

    // ------------------------------------------------------------ transforms
    pub fn local_matrix(&self, id: &str) -> DMat4 {
        self.nodes.get(id).map(|n| n.xf.matrix()).unwrap_or(DMat4::IDENTITY)
    }
    pub fn world_matrix(&self, id: &str) -> DMat4 {
        let mut m = DMat4::IDENTITY;
        let mut c = Some(id.to_string());
        while let Some(i) = c {
            let Some(n) = self.nodes.get(&i) else { break };
            m = n.xf.matrix() * m;
            c = n.parent.clone();
        }
        m
    }
    pub fn parent_world(&self, id: &str) -> DMat4 {
        match self.nodes.get(id).and_then(|n| n.parent.clone()) {
            Some(p) => self.world_matrix(&p),
            None => DMat4::IDENTITY,
        }
    }

    /// Set channels so the local matrix equals m (keeps pivots).
    pub fn set_local_matrix(&mut self, id: &str, m: DMat4) {
        let Some(n) = self.nodes.get_mut(id) else { return };
        let (s, q, pos) = m.to_scale_rotation_translation();
        // joints keep their orient: only the remaining rotation goes into rotate
        let e = euler_deg_from_quat(n.xf.jo_quat().inverse() * q, &n.xf.ro);
        let xf = &mut n.xf;
        xf.r = [0, 1, 2].map(|i| round_to(near_angle(xf.r[i], e[i]), 6));
        xf.s = [s.x, s.y, s.z].map(|x| round_to(x, 6));
        let rs = DMat4::from_scale_rotation_translation(s, q, DVec3::ZERO);
        let rp = rs.transform_point3(v3(xf.p));
        xf.t = [0, 1, 2].map(|i| round_to(pos[i] - xf.pt[i] - xf.p[i] + rp[i], 6));
        self.touch();
        let id = id.to_string();
        self.refresh_dependents(&id);
    }
    pub fn set_world_matrix(&mut self, id: &str, wm: DMat4) {
        let pinv = self.parent_world(id).inverse();
        self.set_local_matrix(id, pinv * wm);
    }
    pub fn world_pivot(&self, id: &str) -> DVec3 {
        let Some(n) = self.nodes.get(id) else { return DVec3::ZERO };
        let p = v3(n.xf.t) + v3(n.xf.pt) + v3(n.xf.p);
        self.parent_world(id).transform_point3(p)
    }
    pub fn set_pivot_local(&mut self, id: &str, np: [f64; 3]) {
        let Some(n) = self.nodes.get_mut(id) else { return };
        let m = n.xf.matrix();
        let rs = DMat4::from_cols(m.x_axis, m.y_axis, m.z_axis, DVec4::W);
        let d = v3(n.xf.p) - v3(np);
        let rsd = rs.transform_point3(d);
        n.xf.pt = [0, 1, 2].map(|i| n.xf.pt[i] + d[i] - rsd[i]);
        n.xf.p = np;
        self.touch();
    }
    pub fn set_pivot_world(&mut self, id: &str, wp: DVec3) {
        let lp = self.world_matrix(id).inverse().transform_point3(wp);
        self.set_pivot_local(id, lp.to_array());
    }

    /// Object-space bounding box of the shape.
    pub fn local_bbox(&self, id: &str) -> Aabb {
        let Some(n) = self.nodes.get(id) else { return Aabb::EMPTY };
        match &n.shape {
            Shape::Mesh(m) => m.mesh.bbox(),
            Shape::Curve(c) => {
                let mut b = Aabb::EMPTY;
                for p in c.sample(4) {
                    b.add(p);
                }
                b
            }
            Shape::Group => Aabb::EMPTY,
            _ => Aabb { min: DVec3::splat(-0.5), max: DVec3::splat(0.5) },
        }
    }
    /// World bounding box of the node and its descendants.
    pub fn world_bbox(&self, id: &str) -> Aabb {
        let mut b = Aabb::EMPTY;
        for i in self.subtree(id) {
            b.union(&self.local_bbox(&i).transformed(&self.world_matrix(&i)));
        }
        b
    }

    pub fn center_pivot(&mut self, id: &str) {
        let b = self.world_bbox(id);
        if !b.is_empty() {
            self.set_pivot_world(id, b.center());
        }
    }

    /// Bake transforms into mesh vertices / curve CVs and reset channels.
    pub fn freeze(&mut self, id: &str) {
        let m = self.local_matrix(id);
        let Some(n) = self.nodes.get_mut(id) else { return };
        match &mut n.shape {
            Shape::Mesh(ms) => {
                let pm = ms.mesh.transformed(&m);
                self.bake_mesh(id, pm);
            }
            Shape::Curve(c) => *c = c.transformed(&m),
            _ => {}
        }
        // children keep their world position
        let kids = self.nodes[id].children.clone();
        for k in &kids {
            let lm = self.local_matrix(k);
            self.set_local_matrix(k, m * lm);
        }
        let n = self.nodes.get_mut(id).unwrap();
        n.xf = Xform { ro: n.xf.ro.clone(), ..Default::default() };
        n.rev += 1;
        self.touch();
    }

    pub fn reset_transforms(&mut self, id: &str) {
        if let Some(n) = self.nodes.get_mut(id) {
            n.xf.t = [0.0; 3];
            n.xf.r = [0.0; 3];
            n.xf.s = [1.0; 3];
        }
        self.touch();
    }

    // ------------------------------------------------------------ hierarchy edits
    fn detach(&mut self, id: &str) {
        let parent = self.nodes.get(id).and_then(|n| n.parent.clone());
        match parent {
            Some(p) => {
                if let Some(pn) = self.nodes.get_mut(&p) {
                    pn.children.retain(|c| c != id);
                }
            }
            None => self.roots.retain(|c| c != id),
        }
    }

    /// Re-parent (None = world), keeping the world transform. `index` inserts at a position.
    pub fn reparent(&mut self, id: &str, parent: Option<&str>, index: Option<usize>) -> bool {
        if let Some(p) = parent {
            if p == id || self.is_ancestor(id, p) || !self.nodes.contains_key(p) {
                return false;
            }
        }
        let wm = self.world_matrix(id);
        self.detach(id);
        let list = match parent {
            Some(p) => &mut self.nodes.get_mut(p).unwrap().children,
            None => &mut self.roots,
        };
        let at = index.unwrap_or(list.len()).min(list.len());
        list.insert(at, id.to_string());
        self.nodes.get_mut(id).unwrap().parent = parent.map(String::from);
        self.set_world_matrix(id, wm);
        self.touch();
        true
    }

    pub fn delete_node(&mut self, id: &str) {
        if !self.nodes.contains_key(id) {
            return;
        }
        for i in self.subtree(id) {
            if i != id && !self.nodes.contains_key(&i) {
                continue;
            }
        }
        let ids = self.subtree(id);
        self.detach(id);
        for i in &ids {
            if let Some(n) = self.nodes.shift_remove(i) {
                self.release_name(&n.name);
                if let Some(s) = &n.shape_name {
                    self.release_name(s);
                }
                if let Shape::Mesh(m) = &n.shape {
                    for h in &m.history {
                        self.release_name(&h.name);
                    }
                }
            }
            self.anim.remove_node(i);
            self.comp.remove(i);
            self.conns.retain(|c| c.src != *i && c.dst != *i);
            self.constraints.retain(|c| c.driven != *i && !c.targets.contains(i));
        }
        self.sel.retain(|s| !ids.contains(s));
        // booleans etc. that used a deleted mesh keep their current result
        let users: Vec<Id> = self
            .nodes
            .values()
            .filter(|n| n.mesh().is_some_and(|m| m.history.iter().any(|h| h.params.values().any(|v| v.as_str().is_some_and(|s| ids.iter().any(|d| d == s))))))
            .map(|n| n.id.clone())
            .collect();
        for u in users {
            let pm = self.nodes[&u].mesh().unwrap().mesh.clone();
            self.bake_mesh(&u, pm);
        }
        self.touch();
    }

    pub fn group(&mut self, ids: &[Id], name: &str) -> Id {
        let parent = ids.first().and_then(|i| self.nodes.get(i)).and_then(|n| n.parent.clone());
        let g = self.create_group(name, parent.as_deref());
        for i in ids {
            self.reparent(i, Some(&g), None);
        }
        g
    }

    pub fn ungroup(&mut self, g: &str) {
        let parent = self.nodes.get(g).and_then(|n| n.parent.clone());
        let kids = self.nodes.get(g).map(|n| n.children.clone()).unwrap_or_default();
        for k in &kids {
            self.reparent(k, parent.as_deref(), None);
        }
        self.delete_node(g);
    }

    // ------------------------------------------------------------ history
    pub fn add_history(&mut self, id: &str, ty: &str, params: Params) -> OpOut {
        let (base, hidden, _) = ops::info(ty).unwrap_or((ty, false, &[]));
        let hid = self.new_id("h");
        let name = if hidden { format!("{base}#{hid}") } else { self.unique_name(&format!("{base}1")) };
        if let Some(m) = self.nodes.get_mut(id).and_then(|n| n.mesh_mut()) {
            // a generator replaces everything below it
            m.history.push(HistoryNode { id: hid, name, ty: ty.into(), params, out: None, error: None });
            let last = m.history.len() - 1;
            return self.evaluate(id, last);
        }
        OpOut::default()
    }

    /// Re-run history from entry `from` (falls back to the start when caches are missing).
    pub fn evaluate(&mut self, id: &str, from: usize) -> OpOut {
        let Some(node) = self.nodes.get(id) else { return OpOut::default() };
        let Some(ms) = node.mesh() else { return OpOut::default() };
        let mut history = ms.history.clone();
        let mut start = from.min(history.len());
        let mut pm = if start > 0 {
            match &history[start - 1].out {
                Some(m) => m.clone(),
                None => {
                    start = 0;
                    PolyMesh::default()
                }
            }
        } else {
            PolyMesh::default()
        };
        let mut last = OpOut::default();
        for h in history.iter_mut().skip(start) {
            match ops::apply(&h.ty, &pm, &h.params, self) {
                Ok(out) => {
                    pm = out.mesh.clone();
                    h.error = None;
                    last = out;
                }
                Err(e) => {
                    h.error = Some(e);
                    last = OpOut { mesh: pm.clone(), ..Default::default() };
                }
            }
            h.out = Some(pm.clone());
        }
        let n = self.nodes.get_mut(id).unwrap();
        let ms = n.mesh_mut().unwrap();
        if !history.is_empty() {
            ms.mesh = pm;
        }
        bake_fm(ms);
        ms.history = history;
        n.rev += 1;
        self.touch();
        last
    }

    pub fn set_history_param(&mut self, id: &str, idx: usize, key: &str, v: Value) {
        if let Some(h) = self.nodes.get_mut(id).and_then(|n| n.mesh_mut()).and_then(|m| m.history.get_mut(idx)) {
            h.params.insert(key.into(), v);
        }
        self.evaluate(id, idx);
        // meshes whose history reads this node (booleans, curve surfaces) follow
        self.refresh_dependents(id);
    }

    /// Re-evaluate meshes whose history references `src` (curve surfaces, booleans).
    pub fn refresh_dependents(&mut self, src: &str) {
        self.refresh_many(&[src.to_string()]);
    }

    /// Re-evaluate every mesh whose history reads one of `srcs` or their descendants
    /// (curve surfaces, booleans, deformer handles, skin joints).
    pub fn refresh_many(&mut self, srcs: &[Id]) {
        let ids: HashSet<String> = srcs.iter().flat_map(|s| self.subtree(s)).collect();
        if ids.is_empty() || !self.nodes.values().any(|n| n.mesh().is_some_and(|m| m.history.len() > 1 || m.history.first().is_some_and(|h| h.ty != "meshData"))) {
            return;
        }
        let refs = |v: &Value| v.as_str().is_some_and(|s| ids.contains(s)) || v.as_array().is_some_and(|a| a.iter().any(|x| x.as_str().is_some_and(|s| ids.contains(s))));
        let users: Vec<(Id, usize)> = self
            .nodes
            .values()
            .filter_map(|n| {
                let m = n.mesh()?;
                let i = m.history.iter().position(|h| h.params.values().any(&refs))?;
                Some((n.id.clone(), i))
            })
            .collect();
        for (u, i) in users {
            self.evaluate(&u, i);
        }
    }

    pub fn bake_mesh(&mut self, id: &str, pm: PolyMesh) {
        let names: Vec<String> = self.nodes.get(id).and_then(|n| n.mesh()).map(|m| m.history.iter().map(|h| h.name.clone()).collect()).unwrap_or_default();
        for n in names {
            self.release_name(&n);
        }
        let hid = self.new_id("h");
        if let Some(n) = self.nodes.get_mut(id) {
            if let Some(m) = n.mesh_mut() {
                let data = serde_json::to_value(&pm).unwrap_or(Value::Null);
                let mut params = Params::new();
                params.insert("data".into(), data);
                m.history = vec![HistoryNode { id: hid.clone(), name: format!("meshData#{hid}"), ty: "meshData".into(), params, out: Some(pm.clone()), error: None }];
                m.mesh = pm;
                bake_fm(m);
            }
            n.rev += 1;
        }
        self.touch();
    }

    pub fn delete_history(&mut self, id: &str) {
        if let Some(pm) = self.nodes.get(id).and_then(|n| n.mesh()).map(|m| m.mesh.clone()) {
            self.bake_mesh(id, pm);
        }
    }

    /// Apply a modeling op to the current mesh (adds history; bakes when history is off).
    pub fn apply_op(&mut self, id: &str, ty: &str, params: Params, keep_history: bool) -> OpOut {
        if keep_history {
            return self.add_history(id, ty, params);
        }
        let Some(pm) = self.nodes.get(id).and_then(|n| n.mesh()).map(|m| m.mesh.clone()) else { return OpOut::default() };
        match ops::apply(ty, &pm, &params, self) {
            Ok(out) => {
                self.bake_mesh(id, out.mesh.clone());
                out
            }
            Err(e) => {
                self.warn(e);
                OpOut { mesh: pm, ..Default::default() }
            }
        }
    }

    /// Mesh in world space.
    pub fn mesh_world(&self, id: &str) -> Option<PolyMesh> {
        let m = self.nodes.get(id)?.mesh()?;
        Some(m.mesh.transformed(&self.world_matrix(id)))
    }

    // ------------------------------------------------------------ duplicate / combine / separate
    pub fn duplicate(&mut self, ids: &[Id], keep_history: bool) -> Vec<Id> {
        let mut out = vec![];
        for id in ids {
            let datas: Vec<Value> = self.subtree(id).iter().map(|i| self.serialize_node(i)).collect();
            let mut map: HashMap<String, Id> = HashMap::new();
            let parent = self.nodes.get(id).and_then(|n| n.parent.clone());
            let mut root = None;
            for mut d in datas {
                let old = d["id"].as_str().unwrap_or("").to_string();
                let par = if old == *id { parent.clone() } else { d["parent"].as_str().and_then(|p| map.get(p).cloned()) };
                if !keep_history && d["kind"] == "mesh" {
                    let mesh = d["mesh"].clone();
                    d["history"] = json!([{ "id": "x", "name": "meshData#", "type": "meshData", "params": { "data": mesh } }]);
                }
                let nid = self.deserialize_node(&d, par.as_deref(), true);
                map.insert(old, nid.clone());
                root.get_or_insert(nid);
            }
            if let Some(r) = root {
                out.push(r);
            }
        }
        out
    }

    pub fn combine(&mut self, ids: &[Id]) -> Option<Id> {
        let meshes: Vec<Id> = ids.iter().flat_map(|i| self.subtree(i)).filter(|i| self.nodes[i].mesh().is_some()).collect();
        if meshes.len() < 2 {
            return None;
        }
        let list: Vec<(PolyMesh, Option<String>)> = meshes.iter().map(|m| (self.mesh_world(m).unwrap(), Some(self.nodes[m].mesh().unwrap().material.clone()))).collect();
        let mat = list.last().unwrap().1.clone().unwrap();
        let mut pm = crate::mesh::combine_meshes(&list);
        if let Some(fm) = &mut pm.fm {
            for x in fm.iter_mut() {
                if x.as_deref() == Some(mat.as_str()) {
                    *x = None;
                }
            }
        }
        let o = self.create_mesh_from(pm, "polySurface1", Some(&mat), None);
        self.center_pivot(&o);
        for m in &meshes {
            self.delete_node(m);
        }
        for g in ids {
            if self.nodes.get(g).is_some_and(|n| n.kind == "group" && n.children.is_empty()) {
                self.delete_node(g);
            }
        }
        Some(o)
    }

    pub fn separate(&mut self, id: &str) -> Vec<Id> {
        let Some(ms) = self.nodes.get(id).and_then(|n| n.mesh()) else { return vec![] };
        let pm = ms.mesh.clone();
        let mat = ms.material.clone();
        let groups = pm.components_connected();
        if groups.len() < 2 {
            return vec![id.to_string()];
        }
        let name = self.nodes[id].name.clone();
        let parent = self.nodes[id].parent.clone();
        let wm = self.world_matrix(id);
        self.delete_node(id);
        let g = self.create_group(&name, parent.as_deref());
        let mut out = vec![];
        for fs in groups {
            let (_, piece) = crate::mesh::op_extract_faces(&pm, &fs);
            let n = self.create_mesh_from(piece, "polySurface1", Some(&mat), Some(&g));
            self.set_world_matrix(&n, wm);
            self.center_pivot(&n);
            out.push(n);
        }
        out
    }

    pub fn extract_faces(&mut self, id: &str, faces: &[usize], duplicate: bool, keep_history: bool) -> Option<Id> {
        let ms = self.nodes.get(id)?.mesh()?;
        let (_, piece) = crate::mesh::op_extract_faces(&ms.mesh, faces);
        let mat = ms.material.clone();
        let parent = self.nodes[id].parent.clone();
        let lm = self.local_matrix(id);
        let n = self.create_mesh_from(piece, "polySurface1", Some(&mat), parent.as_deref());
        self.set_local_matrix(&n, lm);
        self.center_pivot(&n);
        if !duplicate {
            let mut p = Params::new();
            p.insert("faces".into(), json!(faces));
            self.apply_op(id, "deleteComponent", p, keep_history);
        }
        Some(n)
    }

    // ------------------------------------------------------------ attributes
    /// Get an attribute of a named node (`pCube1.translateX`, `polyCube1.width`, `lambert1.color`).
    pub fn get_attr(&self, r: &NodeRef, attr: &str) -> Option<Value> {
        let attr = if matches!(r, NodeRef::Dag(_)) { alias(attr) } else { attr };
        match r {
            NodeRef::Dag(id) => {
                let n = self.nodes.get(id)?;
                if let Some((k, i)) = xf_attr(attr) {
                    return Some(json!(xf_vec(&n.xf, k)[i]));
                }
                match attr {
                    "translate" => return Some(json!(n.xf.t)),
                    "rotate" => return Some(json!(n.xf.r)),
                    "scale" => return Some(json!(n.xf.s)),
                    "rotatePivot" | "scalePivot" => return Some(json!(n.xf.p)),
                    "visibility" => return Some(json!(if n.visible { 1 } else { 0 })),
                    "template" => return Some(json!(if n.template { 1 } else { 0 })),
                    "rotateOrder" => return Some(json!(n.xf.ro)),
                    _ => {}
                }
                if let Some(v) = n.extra.as_object().and_then(|e| get_compound(e, attr)) {
                    return Some(v);
                }
                match &n.shape {
                    Shape::Mesh(m) => match attr {
                        "smoothLevel" => Some(json!(m.smooth_level)),
                        "material" => Some(json!(self.mats.get(&m.material).map(|x| x.name.clone()))),
                        _ => None,
                    },
                    Shape::Light(l) => get_compound(&l.attrs, attr),
                    Shape::Camera(c) => get_compound(serde_json::to_value(c).ok()?.as_object()?, attr),
                    Shape::Curve(c) => match attr {
                        "degree" => Some(json!(c.degree)),
                        "spans" => Some(json!(c.spans())),
                        _ => None,
                    },
                    _ => None,
                }
            }
            NodeRef::History(id, i) => {
                let h = self.nodes.get(id)?.mesh()?.history.get(*i)?;
                h.params.get(attr).cloned().or_else(|| primitives::gen_info(&h.ty).and_then(|g| g.params.iter().find(|p| p.0 == attr).map(|p| json!(p.1))))
            }
            NodeRef::Material(id) => get_compound(&self.mats.get(id)?.attrs, attr),
            NodeRef::Texture(id) => get_compound(&self.texs.get(id)?.attrs, attr),
        }
    }

    /// Set an attribute; returns false when it is locked or unknown.
    pub fn set_attr(&mut self, r: &NodeRef, attr: &str, v: Value) -> bool {
        self.set_attr_ex(r, attr, v, true)
    }

    /// `refresh` = re-evaluate meshes that depend on the node (off while batching).
    pub fn set_attr_ex(&mut self, r: &NodeRef, attr: &str, v: Value, refresh: bool) -> bool {
        // short names (t, r, s...) are transform aliases; history params keep their own keys
        let attr = if matches!(r, NodeRef::Dag(_)) { alias(attr) } else { attr }.to_string();
        let attr = attr.as_str();
        match r {
            NodeRef::Dag(id) => {
                let Some(n) = self.nodes.get_mut(id) else { return false };
                if n.is_locked(attr) {
                    return false;
                }
                let f = v.as_f64().or_else(|| v.as_bool().map(|b| b as i32 as f64));
                if let Some((k, i)) = xf_attr(attr) {
                    let Some(x) = f else { return false };
                    xf_vec_mut(&mut n.xf, k)[i] = x;
                } else {
                    match attr {
                        "translate" | "rotate" | "scale" => {
                            let Some(vv) = crate::params::value_vec3(&v) else { return false };
                            *xf_vec_mut(&mut n.xf, &attr[..1]) = vv.to_array();
                        }
                        "visibility" => n.visible = f.unwrap_or(1.0) != 0.0,
                        "template" => n.template = f.unwrap_or(0.0) != 0.0,
                        "rotateOrder" => {
                            n.xf.ro = match &v {
                                Value::Number(k) => crate::math::ROTATE_ORDERS.get(k.as_u64().unwrap_or(0) as usize).copied().unwrap_or("xyz").to_string(),
                                _ => v.as_str().unwrap_or("xyz").to_string(),
                            }
                        }
                        _ if n.extra.as_object().is_some_and(|e| e.contains_key(attr) || get_compound(e, attr).is_some()) => {
                            set_compound(n.extra.as_object_mut().unwrap(), attr, v);
                            self.dynamics.frame = None;
                        }
                        _ => match &mut n.shape {
                            Shape::Mesh(m) if attr == "smoothLevel" => {
                                m.smooth_level = f.unwrap_or(0.0).clamp(0.0, 3.0) as u32;
                                n.rev += 1;
                            }
                            Shape::Light(l) => {
                                if !set_compound(&mut l.attrs, attr, v) {
                                    return false;
                                }
                            }
                            Shape::Camera(c) => {
                                let Ok(mut o) = serde_json::to_value(&*c) else { return false };
                                if !set_compound(o.as_object_mut().unwrap(), attr, v) {
                                    return false;
                                }
                                if let Ok(nc) = serde_json::from_value(o) {
                                    *c = nc;
                                }
                            }
                            _ => return false,
                        },
                    }
                }
                self.touch();
                if refresh {
                    let id = id.clone();
                    self.refresh_dependents(&id);
                }
                true
            }
            NodeRef::History(id, i) => {
                let (id, i) = (id.clone(), *i);
                self.set_history_param(&id, i, attr, v);
                true
            }
            NodeRef::Material(id) => {
                let ok = self.mats.get_mut(id).is_some_and(|m| set_compound(&mut m.attrs, attr, v));
                self.touch();
                ok
            }
            NodeRef::Texture(id) => {
                let ok = self.texs.get_mut(id).is_some_and(|m| set_compound(&mut m.attrs, attr, v));
                self.touch();
                ok
            }
        }
    }

    pub fn get_attr_f(&self, id: &str, attr: &str) -> f64 {
        self.get_attr(&NodeRef::Dag(id.into()), attr).and_then(|v| v.as_f64()).unwrap_or(0.0)
    }
    pub fn set_attr_f(&mut self, id: &str, attr: &str, v: f64) -> bool {
        self.set_attr(&NodeRef::Dag(id.into()), attr, json!(v))
    }

    /// Numeric shape attributes shown in the Channel Box SHAPES section.
    pub fn shape_channels(&self, id: &str) -> Vec<String> {
        let Some(n) = self.nodes.get(id) else { return vec![] };
        match &n.shape {
            Shape::Light(l) => l.attrs.iter().filter(|(_, v)| v.is_number() || v.is_boolean()).map(|(k, _)| k.clone()).collect(),
            Shape::Camera(_) => ["focalLength", "near", "far", "orthoWidth", "coi"].iter().map(|s| s.to_string()).collect(),
            Shape::Mesh(_) => vec!["smoothLevel".into()],
            Shape::Locator if n.kind == "emitter" || n.kind == "field" => n.extra.as_object().map(|e| e.iter().filter(|(_, v)| v.is_number()).map(|(k, _)| k.clone()).collect()).unwrap_or_default(),
            _ => vec![],
        }
    }

    // ------------------------------------------------------------ animation
    pub fn set_key(&mut self, id: &str, attr: &str, t: Option<f64>, v: Option<f64>) {
        let t = t.unwrap_or(self.time.current);
        let v = v.unwrap_or_else(|| self.get_attr_f(id, attr));
        let tangent = "auto";
        if let Some(c) = self.anim.curve_mut(id, alias(attr), true) {
            c.set_key(t, v, tangent);
        }
        self.touch();
    }

    /// Evaluate anim curves at the current time and push values into channels.
    pub fn apply_anim(&mut self) {
        let t = self.time.current;
        let vals: Vec<(String, String, f64)> = self.anim.curves.values().filter(|c| !c.keys.is_empty()).map(|c| (c.node.clone(), c.attr.clone(), c.eval(t))).collect();
        let mut moved: Vec<Id> = vec![];
        for (n, a, v) in vals {
            if self.nodes.contains_key(&n) {
                let r = NodeRef::Dag(n.clone());
                self.set_attr_ex(&r, &a, json!(v), false);
                if !moved.contains(&n) {
                    moved.push(n);
                }
            } else if let Some(r) = self.history_ref_by_id(&n) {
                self.set_attr(&r, &a, json!(v));
            }
        }
        // direct connections: dst = src
        for c in self.conns.clone() {
            if let (Some(sr), Some(dr)) = (self.ref_by_id(&c.src), self.ref_by_id(&c.dst)) {
                if let Some(v) = self.get_attr(&sr, &c.src_attr) {
                    self.set_attr_ex(&dr, &c.dst_attr, v, false);
                    if matches!(dr, NodeRef::Dag(_)) {
                        moved.push(c.dst.clone());
                    }
                }
            }
        }
        moved.extend(self.eval_constraints());
        self.refresh_many(&moved);
    }

    // ------------------------------------------------------------ constraints
    /// Apply all constraints; returns the nodes that moved.
    pub fn eval_constraints(&mut self) -> Vec<Id> {
        let mut moved = self.eval_rig_pre();
        for c in self.constraints.clone() {
            if c.kind == "poleVector" {
                continue;
            }
            if !self.nodes.contains_key(&c.driven) || c.targets.iter().any(|t| !self.nodes.contains_key(t)) || c.targets.is_empty() {
                continue;
            }
            let cur = self.world_matrix(&c.driven);
            let (cs, cq, ct) = cur.to_scale_rotation_translation();
            let n = c.targets.len() as f64;
            let tw: Vec<DMat4> = c.targets.iter().map(|t| self.world_matrix(t)).collect();
            let avg_pos = c.targets.iter().map(|t| self.world_pivot(t)).sum::<DVec3>() / n;
            let off = c.offset.map(|o| DMat4::from_cols_array(&o)).unwrap_or(DMat4::IDENTITY);
            let new = match c.kind.as_str() {
                "point" => {
                    let pivot_off = self.world_pivot(&c.driven) - ct;
                    DMat4::from_scale_rotation_translation(cs, cq, avg_pos + off.w_axis.truncate() - pivot_off)
                }
                "orient" => {
                    let mut q = tw[0].to_scale_rotation_translation().1;
                    for (i, m) in tw.iter().enumerate().skip(1) {
                        q = q.slerp(m.to_scale_rotation_translation().1, 1.0 / (i as f64 + 1.0));
                    }
                    let oq = DQuat::from_mat4(&off);
                    DMat4::from_scale_rotation_translation(cs, q * oq, ct)
                }
                "scale" => {
                    let s = tw.iter().map(|m| m.to_scale_rotation_translation().0).sum::<DVec3>() / n;
                    DMat4::from_scale_rotation_translation(s * off.to_scale_rotation_translation().0, cq, ct)
                }
                "aim" => {
                    let aim = DVec3::from_array(c.aim).normalize_or(DVec3::X);
                    let up = DVec3::from_array(c.up).normalize_or(DVec3::Y);
                    let dir = (avg_pos - self.world_pivot(&c.driven)).normalize_or(aim);
                    // rotate the aim axis onto dir, then roll the up axis towards world up
                    let q1 = DQuat::from_rotation_arc(aim, dir);
                    let up1 = q1 * up;
                    let want = (DVec3::Y - dir * DVec3::Y.dot(dir)).normalize_or(up1);
                    let ang = up1.cross(want).dot(dir).atan2(up1.dot(want));
                    let q = DQuat::from_axis_angle(dir, ang) * q1;
                    DMat4::from_scale_rotation_translation(cs, q, ct)
                }
                _ => tw[0] * off, // parent
            };
            if (new - cur).to_cols_array().iter().any(|x| x.abs() > 1e-9) {
                let pinv = self.parent_world(&c.driven).inverse();
                let local = pinv * new;
                // quiet update: no refresh here, the caller batches it
                let Some(nd) = self.nodes.get_mut(&c.driven) else { continue };
                let (s, q, pos) = local.to_scale_rotation_translation();
                let e = euler_deg_from_quat(nd.xf.jo_quat().inverse() * q, &nd.xf.ro);
                let xf = &mut nd.xf;
                xf.r = [0, 1, 2].map(|i| near_angle(xf.r[i], e[i]));
                xf.s = s.to_array();
                let rs = DMat4::from_scale_rotation_translation(s, q, DVec3::ZERO);
                let rp = rs.transform_point3(v3(xf.p));
                xf.t = [0, 1, 2].map(|i| pos[i] - xf.pt[i] - xf.p[i] + rp[i]);
                self.rev += 1;
                moved.push(c.driven.clone());
            }
        }
        moved.extend(self.eval_rig_post());
        moved
    }

    /// Create a constraint (`maintain_offset` keeps the driven object where it is).
    pub fn add_constraint(&mut self, kind: &str, targets: Vec<Id>, driven: &str, maintain_offset: bool) -> String {
        let name = self.unique_name(&format!("{}_{kind}Constraint1", self.name_of(driven)));
        let id = self.new_id("c");
        let dw = self.world_matrix(driven);
        let offset = if maintain_offset {
            match kind {
                "point" => {
                    let avg = targets.iter().map(|t| self.world_pivot(t)).sum::<DVec3>() / targets.len().max(1) as f64;
                    Some(DMat4::from_translation(self.world_pivot(driven) - avg).to_cols_array())
                }
                "orient" => {
                    let tq = self.world_matrix(&targets[0]).to_scale_rotation_translation().1;
                    let dq = dw.to_scale_rotation_translation().1;
                    Some(DMat4::from_quat(tq.inverse() * dq).to_cols_array())
                }
                "scale" => {
                    let ts = self.world_matrix(&targets[0]).to_scale_rotation_translation().0;
                    let ds = dw.to_scale_rotation_translation().0;
                    Some(DMat4::from_scale(ds / ts.max(DVec3::splat(1e-9))).to_cols_array())
                }
                "parent" => Some((self.world_matrix(&targets[0]).inverse() * dw).to_cols_array()),
                _ => None,
            }
        } else if kind == "parent" {
            // snap: keep the driven object's own scale
            let (s, _, _) = dw.to_scale_rotation_translation();
            Some(DMat4::from_scale(s).to_cols_array())
        } else {
            None
        };
        self.constraints.push(Constraint { id, name: name.clone(), kind: kind.into(), driven: driven.into(), targets, offset, aim: [1.0, 0.0, 0.0], up: [0.0, 1.0, 0.0] });
        let moved = self.eval_constraints();
        self.refresh_many(&moved);
        self.touch();
        name
    }

    pub fn set_time(&mut self, t: f64) {
        self.time.current = t;
        self.apply_anim();
        self.step_dynamics(t);
        self.rev += 1;
    }

    pub fn ref_by_id(&self, id: &str) -> Option<NodeRef> {
        if self.nodes.contains_key(id) {
            return Some(NodeRef::Dag(id.into()));
        }
        if self.mats.contains_key(id) {
            return Some(NodeRef::Material(id.into()));
        }
        if self.texs.contains_key(id) {
            return Some(NodeRef::Texture(id.into()));
        }
        self.history_ref_by_id(id)
    }
    fn history_ref_by_id(&self, hid: &str) -> Option<NodeRef> {
        for n in self.nodes.values() {
            if let Some(i) = n.mesh().and_then(|m| m.history.iter().position(|h| h.id == hid)) {
                return Some(NodeRef::History(n.id.clone(), i));
            }
        }
        None
    }
    pub fn ref_id(&self, r: &NodeRef) -> Id {
        match r {
            NodeRef::Dag(i) | NodeRef::Material(i) | NodeRef::Texture(i) => i.clone(),
            NodeRef::History(n, i) => self.nodes.get(n).and_then(|n| n.mesh()).and_then(|m| m.history.get(*i)).map(|h| h.id.clone()).unwrap_or_default(),
        }
    }

    // ------------------------------------------------------------ selection
    pub fn select(&mut self, ids: &[Id], mode: SelMode) {
        match mode {
            SelMode::Replace => self.sel = ids.to_vec(),
            SelMode::Add => {
                for i in ids {
                    self.sel.retain(|s| s != i);
                    self.sel.push(i.clone());
                }
            }
            SelMode::Toggle => {
                for i in ids {
                    if self.sel.contains(i) {
                        self.sel.retain(|s| s != i);
                    } else {
                        self.sel.push(i.clone());
                    }
                }
            }
            SelMode::Remove => self.sel.retain(|s| !ids.contains(s)),
        }
        self.sel.retain(|s| self.nodes.contains_key(s));
        self.rev += 1;
    }
    pub fn lead(&self) -> Option<&Id> {
        self.sel.last()
    }

    /// Objects whose components are being edited (the selected meshes in component mode).
    pub fn hilite(&self) -> Vec<Id> {
        if self.comp_mode.is_none() {
            return vec![];
        }
        self.sel.iter().filter(|i| self.nodes.get(*i).is_some_and(|n| n.mesh().is_some())).cloned().collect()
    }

    pub fn set_comp_mode(&mut self, m: Option<CompMode>) {
        self.comp_mode = m;
        if m.is_none() {
            self.comp.clear();
        }
        self.rev += 1;
    }

    pub fn comp_sel(&self, id: &str) -> CompSel {
        self.comp.get(id).cloned().unwrap_or_default()
    }

    /// Component selection converted to vertex indices.
    pub fn comp_verts(&self, id: &str) -> Vec<u32> {
        let Some(pm) = self.nodes.get(id).and_then(|n| n.mesh()).map(|m| &m.mesh) else { return vec![] };
        let c = self.comp_sel(id);
        let mut s: BTreeSet<u32> = c.v.clone();
        for (a, b) in &c.e {
            s.insert(*a);
            s.insert(*b);
        }
        for &f in &c.f {
            if let Some(face) = pm.f.get(f) {
                s.extend(face.iter().copied());
            }
        }
        s.into_iter().filter(|&v| (v as usize) < pm.v.len()).collect()
    }

    // ------------------------------------------------------------ undo
    pub fn checkpoint(&mut self, label: &str) {
        let data = self.serialize();
        self.undo_stack.push(Snapshot { label: label.into(), data });
        if self.undo_stack.len() > self.undo_limit {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }
    pub fn undo(&mut self) -> Option<String> {
        let s = self.undo_stack.pop()?;
        let cur = self.serialize();
        self.redo_stack.push(Snapshot { label: s.label.clone(), data: cur });
        self.restore(&s.data);
        Some(s.label)
    }
    pub fn redo(&mut self) -> Option<String> {
        let s = self.redo_stack.pop()?;
        let cur = self.serialize();
        self.undo_stack.push(Snapshot { label: s.label.clone(), data: cur });
        self.restore(&s.data);
        Some(s.label)
    }
    fn restore(&mut self, d: &Value) {
        let (file, project) = (self.file.clone(), self.project.clone());
        let undo = std::mem::take(&mut self.undo_stack);
        let redo = std::mem::take(&mut self.redo_stack);
        let comp_mode = self.comp_mode;
        let _ = self.load(d);
        self.comp_mode = comp_mode;
        self.undo_stack = undo;
        self.redo_stack = redo;
        self.file = file;
        self.project = project;
        self.modified = true;
    }

    // ------------------------------------------------------------ serialization
    pub fn serialize_node(&self, id: &str) -> Value {
        let n = &self.nodes[id];
        let mut d = json!({
            "id": n.id, "name": n.name, "kind": n.kind, "parent": n.parent,
            "t": n.xf.t, "r": n.xf.r, "s": n.xf.s, "ro": n.xf.ro, "p": n.xf.p, "pt": n.xf.pt,
            "visible": n.visible, "layer": n.layer, "template": n.template, "locks": n.locks,
            "shapeName": n.shape_name, "extra": n.extra,
        });
        let o = d.as_object_mut().unwrap();
        match &n.shape {
            Shape::Mesh(m) => {
                o.insert("material".into(), json!(m.material));
                o.insert("smoothLevel".into(), json!(m.smooth_level));
                o.insert("nurbs".into(), json!(m.nurbs));
                o.insert("history".into(), serde_json::to_value(&m.history).unwrap());
                o.insert("mesh".into(), serde_json::to_value(&m.mesh).unwrap());
            }
            Shape::Light(l) => {
                let mut lm = Map::new();
                lm.insert("type".into(), json!(l.ty));
                lm.extend(l.attrs.clone());
                o.insert("light".into(), Value::Object(lm));
            }
            Shape::Camera(c) => {
                o.insert("cam".into(), serde_json::to_value(c).unwrap());
            }
            Shape::Curve(c) => {
                o.insert("curve".into(), serde_json::to_value(c).unwrap());
            }
            _ => {}
        }
        for (k, v) in &n.passthrough {
            o.insert(k.clone(), v.clone());
        }
        // the JS app reads rig nodes' keyable attributes from extraAttrs
        if (crate::rig::RIG_KINDS.contains(&n.kind.as_str()) || n.passthrough.contains_key("rig")) && n.extra.as_object().is_some_and(|e| !e.is_empty()) {
            o.insert("extraAttrs".into(), n.extra.clone());
        }
        if n.xf.jo != [0.0; 3] || n.kind == "joint" {
            o.insert("jo".into(), json!(n.xf.jo));
        }
        d
    }

    /// Create a node from its JSON. `fresh` = new ids/names (duplicate / import).
    pub fn deserialize_node(&mut self, d: &Value, parent: Option<&str>, fresh: bool) -> Id {
        let kind = d["kind"].as_str().unwrap_or("group").to_string();
        let name = d["name"].as_str().unwrap_or("node").to_string();
        let id = if fresh { None } else { d["id"].as_str().map(String::from) };
        let startup = d["cam"]["startup"].as_bool().unwrap_or(false);
        let shape = match kind.as_str() {
            "mesh" => {
                let mesh: PolyMesh = serde_json::from_value(d["mesh"].clone()).unwrap_or_default();
                let mut history: Vec<HistoryNode> = serde_json::from_value(d["history"].clone()).unwrap_or_default();
                for h in &mut history {
                    if fresh {
                        h.id = self.new_id("h");
                    }
                    if !h.name.contains('#') {
                        h.name = if fresh { self.unique_name(&h.name) } else { self.claim(&h.name) };
                    }
                }
                if let Some(l) = history.last_mut() {
                    l.out = Some(mesh.clone());
                }
                let material = d["material"].as_str().filter(|m| self.mats.contains_key(*m)).map(String::from).unwrap_or(self.default_mat.clone());
                Shape::Mesh(Box::new(MeshShape { material, smooth_level: d["smoothLevel"].as_u64().unwrap_or(0) as u32, nurbs: d["nurbs"].as_bool().unwrap_or(false), history, mesh }))
            }
            "light" => {
                let mut attrs = d["light"].as_object().cloned().unwrap_or_default();
                let ty = attrs.remove("type").and_then(|t| t.as_str().map(String::from)).unwrap_or("pointLight".into());
                let mut full = match light_defaults(&ty) {
                    Value::Object(m) => m,
                    _ => Map::new(),
                };
                full.extend(attrs);
                Shape::Light(LightShape { ty, attrs: full })
            }
            "camera" => Shape::Camera(serde_json::from_value(d["cam"].clone()).unwrap_or_default()),
            "curve" => Shape::Curve(serde_json::from_value(d["curve"].clone()).unwrap_or_default()),
            "locator" => Shape::Locator,
            "group" => Shape::Group,
            _ => Shape::Locator, // rig nodes from the JS app (joints, handles, ...) show as locators
        };
        let exact = !fresh && startup;
        let nid = if exact || !fresh {
            // loading a file: keep names exactly when free
            let nm = if exact { name.clone() } else { self.claim(&name) };
            self.make_dag(&kind, &nm, parent, shape, true, id)
        } else {
            self.make_dag(&kind, &name, parent, shape, false, id)
        };
        if !fresh {
            if let Some(sn) = d["shapeName"].as_str() {
                let n = self.nodes.get_mut(&nid).unwrap();
                if let Some(old) = n.shape_name.take() {
                    self.names.remove(&old);
                }
                let sn = self.claim(sn);
                self.nodes.get_mut(&nid).unwrap().shape_name = Some(sn);
            }
        }
        let n = self.nodes.get_mut(&nid).unwrap();
        let a3 = |v: &Value, def: [f64; 3]| -> [f64; 3] { serde_json::from_value(v.clone()).unwrap_or(def) };
        n.xf.t = a3(&d["t"], [0.0; 3]);
        n.xf.r = a3(&d["r"], [0.0; 3]);
        n.xf.s = a3(&d["s"], [1.0; 3]);
        n.xf.ro = d["ro"].as_str().unwrap_or("xyz").into();
        n.xf.p = a3(&d["p"], [0.0; 3]);
        n.xf.pt = a3(&d["pt"], [0.0; 3]);
        n.visible = d["visible"].as_bool().unwrap_or(true);
        n.layer = d["layer"].as_str().map(String::from);
        n.template = d["template"].as_bool().unwrap_or(false);
        n.locks = d["locks"].as_object().cloned().unwrap_or_default();
        n.extra = d["extra"].clone();
        for k in ["rig", "extraAttrs"] {
            if let Some(v) = d.get(k) {
                n.passthrough.insert(k.into(), v.clone());
            }
        }
        n.xf.jo = a3(&d["jo"], [0.0; 3]);
        if let Some(m) = n.mesh_mut() {
            bake_fm(m);
        }
        nid
    }

    /// Exact name if free, else the next unique one.
    fn claim(&mut self, n: &str) -> String {
        self.unique_name(n)
    }

    pub fn serialize(&self) -> Value {
        let nodes: Vec<Value> = self.all_dag(true).iter().map(|i| self.serialize_node(i)).collect();
        json!({
            "format": "inca-scene",
            "version": self.version,
            "idCounter": self.id_counter,
            "nodes": nodes,
            "shading": ShadingData { mats: self.mats.values().cloned().collect(), texs: self.texs.values().cloned().collect(), default_mat_id: Some(self.default_mat.clone()) },
            "layers": self.layers,
            "anim": self.anim.serialize(),
            "conns": self.conns,
            "constraints": self.constraints,
            "time": TimeState { playing: false, ..self.time.clone() },
            "render": if self.render.is_empty() { Value::Null } else { Value::Object(self.render.clone()) },
            "sel": self.sel,
            "project": self.project,
        })
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.roots.clear();
        self.names.clear();
        self.mats.clear();
        self.texs.clear();
        self.layers.clear();
        self.anim = Anim::default();
        self.conns.clear();
        self.constraints.clear();
        self.dynamics = Default::default();
        self.sel.clear();
        self.comp.clear();
        self.comp_mode = None;
        self.touch();
    }

    pub fn load(&mut self, data: &Value) -> Result<(), String> {
        if data.get("nodes").and_then(|n| n.as_array()).is_none() {
            return Err("not an Inca scene".into());
        }
        self.clear();
        self.id_counter = self.id_counter.max(data["idCounter"].as_u64().unwrap_or(1));
        let shading: ShadingData = serde_json::from_value(data["shading"].clone()).unwrap_or_default();
        for t in shading.texs {
            self.names.insert(t.name.clone());
            self.texs.insert(t.id.clone(), t);
        }
        for m in shading.mats {
            self.names.insert(m.name.clone());
            self.mats.insert(m.id.clone(), m);
        }
        self.default_mat = shading.default_mat_id.filter(|d| self.mats.contains_key(d)).or_else(|| self.mats.keys().next().cloned()).unwrap_or_default();
        if self.default_mat.is_empty() {
            self.init_default_materials();
        }
        self.layers = serde_json::from_value(data["layers"].clone()).unwrap_or_default();
        let mut by_id: HashMap<String, Id> = HashMap::new();
        for d in data["nodes"].as_array().unwrap() {
            let parent = d["parent"].as_str().and_then(|p| by_id.get(p).cloned());
            let id = self.deserialize_node(d, parent.as_deref(), false);
            by_id.insert(d["id"].as_str().unwrap_or("").to_string(), id);
        }
        if self.find("persp").is_none() {
            self.create_startup_cameras();
        }
        let anim: Vec<AnimCurve> = serde_json::from_value(data["anim"].clone()).unwrap_or_default();
        self.anim.load(anim);
        self.conns = serde_json::from_value::<Vec<Conn>>(data["conns"].clone()).unwrap_or_default();
        self.constraints = serde_json::from_value::<Vec<Constraint>>(data["constraints"].clone()).unwrap_or_default();
        if let Ok(t) = serde_json::from_value::<TimeState>(data["time"].clone()) {
            self.time = TimeState { playing: false, ..t };
        }
        self.render = data["render"].as_object().cloned().unwrap_or_default();
        self.sel = data["sel"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).filter(|i| self.nodes.contains_key(i)).collect()).unwrap_or_default();
        self.project = data["project"].as_str().map(String::from);
        self.import_js_rig();
        self.touch();
        Ok(())
    }

    /// Fresh empty scene (File > New).
    pub fn reset(&mut self) {
        *self = Scene { project: self.project.take(), ..Scene::new() };
    }

    /// Visible = node + all ancestors visible and the display layer visible.
    pub fn is_visible(&self, id: &str) -> bool {
        let mut c = Some(id.to_string());
        while let Some(i) = c {
            let Some(n) = self.nodes.get(&i) else { return false };
            if !n.visible {
                return false;
            }
            if let Some(l) = &n.layer {
                if self.layers.iter().any(|x| &x.id == l && !x.visible) {
                    return false;
                }
            }
            c = n.parent.clone();
        }
        true
    }

    /// Template (unselectable, grey) through the node, its ancestors or its layer.
    pub fn is_template(&self, id: &str) -> bool {
        let mut c = Some(id.to_string());
        while let Some(i) = c {
            let Some(n) = self.nodes.get(&i) else { return false };
            if n.template {
                return true;
            }
            if let Some(l) = &n.layer {
                if self.layers.iter().any(|x| &x.id == l && x.mode != "normal") {
                    return true;
                }
            }
            c = n.parent.clone();
        }
        false
    }

    pub fn poly_count(&self) -> (usize, usize, usize, usize) {
        let (mut v, mut e, mut f, mut t) = (0, 0, 0, 0);
        for n in self.nodes.values() {
            if let Some(m) = n.mesh() {
                v += m.mesh.v.len();
                e += m.mesh.edge_count();
                f += m.mesh.f.len();
                t += m.mesh.tri_count();
            }
        }
        (v, e, f, t)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelMode {
    Replace,
    Add,
    Toggle,
    Remove,
}

impl OpCtx for Scene {
    fn world_curve(&self, id: &str) -> Option<Curve> {
        match &self.nodes.get(id)?.shape {
            Shape::Curve(c) => Some(c.transformed(&self.world_matrix(id))),
            _ => None,
        }
    }
    fn world_mesh(&self, id: &str) -> Option<PolyMesh> {
        self.mesh_world(id)
    }
    fn world_matrix(&self, id: &str) -> Option<DMat4> {
        self.nodes.contains_key(id).then(|| Scene::world_matrix(self, id))
    }
    fn node_rig(&self, id: &str) -> Option<Value> {
        self.nodes.get(id)?.passthrough.get("rig").cloned()
    }
    fn node_extra(&self, id: &str, key: &str) -> Option<f64> {
        let v = self.nodes.get(id)?.extra.get(key)?;
        v.as_f64().or_else(|| v.as_bool().map(|b| b as i32 as f64))
    }
    fn local_verts(&self, id: &str) -> Option<Vec<DVec3>> {
        self.nodes.get(id)?.mesh().map(|m| m.mesh.v.clone())
    }
}

/// Per-face materials equal to the object material are stored as None.
fn bake_fm(m: &mut MeshShape) {
    let mat = m.material.clone();
    if let Some(fm) = &mut m.mesh.fm {
        for x in fm.iter_mut() {
            if x.as_deref() == Some(mat.as_str()) {
                *x = None;
            }
        }
        if fm.iter().all(|x| x.is_none()) {
            m.mesh.fm = None;
        }
    }
}

fn xf_attr(a: &str) -> Option<(&'static str, usize)> {
    let k = match a.get(..a.len().saturating_sub(1))? {
        "translate" => "t",
        "rotate" => "r",
        "scale" => "s",
        "rotatePivot" | "scalePivot" => "p",
        _ => return None,
    };
    let i = match a.chars().last()? {
        'X' => 0,
        'Y' => 1,
        'Z' => 2,
        _ => return None,
    };
    Some((k, i))
}
fn xf_vec<'a>(x: &'a Xform, k: &str) -> &'a [f64; 3] {
    match k {
        "t" => &x.t,
        "r" => &x.r,
        "s" => &x.s,
        _ => &x.p,
    }
}
fn xf_vec_mut<'a>(x: &'a mut Xform, k: &str) -> &'a mut [f64; 3] {
    match k {
        "t" => &mut x.t,
        "r" => &mut x.r,
        "s" => &mut x.s,
        _ => &mut x.p,
    }
}

/// `color` returns the vector, `colorR`/`colorG`/`colorB` a component.
fn get_compound(m: &Map<String, Value>, attr: &str) -> Option<Value> {
    if let Some(v) = m.get(attr) {
        return Some(v.clone());
    }
    let (base, idx) = split_component(attr)?;
    m.get(base)?.as_array()?.get(idx).cloned()
}
fn set_compound(m: &mut Map<String, Value>, attr: &str, v: Value) -> bool {
    if m.contains_key(attr) {
        let old = &m[attr];
        let v = match (old, &v) {
            (Value::Bool(_), Value::Number(n)) => Value::Bool(n.as_f64().unwrap_or(0.0) != 0.0),
            _ => v,
        };
        m.insert(attr.into(), v);
        return true;
    }
    if let Some((base, idx)) = split_component(attr) {
        if let Some(Value::Array(a)) = m.get_mut(base) {
            if idx < a.len() {
                a[idx] = v;
                return true;
            }
        }
    }
    false
}
fn split_component(attr: &str) -> Option<(&str, usize)> {
    let last = attr.chars().last()?;
    let idx = match last {
        'R' | 'X' => 0,
        'G' | 'Y' => 1,
        'B' | 'Z' => 2,
        _ => return None,
    };
    Some((&attr[..attr.len() - 1], idx))
}

pub fn light_defaults(ty: &str) -> Value {
    match ty {
        "ambientLight" => json!({ "color": [1, 1, 1], "intensity": 1, "ambientShade": 0.45 }),
        "directionalLight" => json!({ "color": [1, 1, 1], "intensity": 1, "shadows": false, "shadowRadius": 2, "shadowColor": [0, 0, 0] }),
        "pointLight" => json!({ "color": [1, 1, 1], "intensity": 1, "decayRate": 0, "shadows": false, "shadowRadius": 2 }),
        "spotLight" => json!({ "color": [1, 1, 1], "intensity": 1, "decayRate": 0, "coneAngle": 40, "penumbraAngle": 0, "dropoff": 0, "shadows": false, "shadowRadius": 2 }),
        "areaLight" => json!({ "color": [1, 1, 1], "intensity": 1, "decayRate": 2, "shadows": false }),
        "skyDomeLight" => json!({ "color": [1, 1, 1], "intensity": 1, "textureFile": "", "showBackground": true }),
        _ => json!({}),
    }
}

pub const LIGHT_TYPES: &[(&str, &str)] = &[
    ("ambientLight", "Ambient Light"),
    ("directionalLight", "Directional Light"),
    ("pointLight", "Point Light"),
    ("spotLight", "Spot Light"),
    ("areaLight", "Area Light"),
    ("skyDomeLight", "Sky Dome Light"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_params_are_not_aliased() {
        let mut s = Scene::new();
        let id = s.create_mesh("polyCube", &Params::new(), None);
        let mut p = Params::new();
        p.insert("edge".into(), json!([0, 1]));
        p.insert("t".into(), json!(0.5));
        s.apply_op(&id, "polySplitRing", p, true);
        let r = NodeRef::History(id.clone(), 1);
        assert!(s.set_attr(&r, "t", json!(0.25)));
        assert_eq!(s.get_attr(&r, "t"), Some(json!(0.25)));
    }

    #[test]
    fn names_increment_like_maya() {
        let mut s = Scene::new();
        let a = s.create_mesh("polyCube", &Params::new(), None);
        let b = s.create_mesh("polyCube", &Params::new(), None);
        assert_eq!(s.name_of(&a), "pCube1");
        assert_eq!(s.name_of(&b), "pCube2");
        assert_eq!(s.nodes[&a].shape_name.as_deref(), Some("pCubeShape1"));
        assert_eq!(s.nodes[&a].mesh().unwrap().history[0].name, "polyCube1");
    }

    #[test]
    fn history_param_reevaluates() {
        let mut s = Scene::new();
        let a = s.create_mesh("polyCube", &Params::new(), None);
        let r = s.resolve("polyCube1").unwrap();
        assert!(s.set_attr(&r, "subdivisionsWidth", json!(3)));
        assert_eq!(s.nodes[&a].mesh().unwrap().mesh.f.len(), 14);
    }

    #[test]
    fn pivot_and_matrix() {
        let mut s = Scene::new();
        let a = s.create_mesh("polyCube", &Params::new(), None);
        s.set_attr_f(&a, "tx", 2.0);
        s.set_pivot_world(&a, DVec3::new(3.0, 0.0, 0.0));
        let before = s.world_matrix(&a).transform_point3(DVec3::ZERO);
        assert!((before - DVec3::new(2.0, 0.0, 0.0)).length() < 1e-9);
        s.set_attr_f(&a, "ry", 180.0);
        let after = s.world_matrix(&a).transform_point3(DVec3::ZERO);
        assert!((after - DVec3::new(4.0, 0.0, 0.0)).length() < 1e-6, "{after}");
    }

    #[test]
    fn round_trip_and_undo() {
        let mut s = Scene::new();
        let a = s.create_mesh("polySphere", &Params::new(), None);
        s.set_attr_f(&a, "ty", 3.0);
        s.create_light("pointLight");
        let data = s.serialize();
        let mut s2 = Scene::new();
        s2.load(&data).unwrap();
        assert_eq!(s2.serialize(), data);
        s.checkpoint("move");
        s.set_attr_f(&a, "ty", 5.0);
        s.undo();
        assert_eq!(s.get_attr_f(&a, "ty"), 3.0);
        s.redo();
        assert_eq!(s.get_attr_f(&a, "ty"), 5.0);
    }

    #[test]
    fn parenting_keeps_world() {
        let mut s = Scene::new();
        let a = s.create_mesh("polyCube", &Params::new(), None);
        let g = s.create_group("group1", None);
        s.set_attr_f(&g, "tx", 5.0);
        s.set_attr_f(&g, "ry", 90.0);
        s.set_attr_f(&a, "tz", 1.0);
        let w = s.world_matrix(&a).transform_point3(DVec3::ZERO);
        s.reparent(&a, Some(&g), None);
        let w2 = s.world_matrix(&a).transform_point3(DVec3::ZERO);
        assert!((w - w2).length() < 1e-6);
        let d = s.duplicate(std::slice::from_ref(&g), false);
        assert_eq!(s.name_of(&d[0]), "group2");
        assert_eq!(s.nodes[&d[0]].children.len(), 1);
    }

    #[test]
    fn animation_drives_channels() {
        let mut s = Scene::new();
        let a = s.create_mesh("polyCube", &Params::new(), None);
        s.set_key(&a, "tx", Some(1.0), Some(0.0));
        s.set_key(&a, "tx", Some(11.0), Some(10.0));
        s.set_time(11.0);
        assert!((s.get_attr_f(&a, "tx") - 10.0).abs() < 1e-9);
    }
}
