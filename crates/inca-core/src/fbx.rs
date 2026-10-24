//! Autodesk FBX import and export.
//!
//! Reads binary and ASCII FBX 7.x (2011 and later) and writes binary FBX 7.4, the version Maya,
//! MotionBuilder, Blender, Unity and Unreal all read. Covered: the transform hierarchy (with
//! pre-rotations as joint orients), meshes with UVs, normals and per-face materials, Lambert /
//! Phong materials with file textures, skeletons (LimbNodes), skin clusters with their weights and
//! bind pose, cameras, lights, and translate / rotate / scale / visibility animation.

use crate::math::{euler_deg_from_quat, quat_from_euler_deg};
use crate::mesh::PolyMesh;
use crate::params::Params;
use crate::scene::{CamShape, Id, NodeRef, Scene, Shape};
use glam::{DMat4, DVec2, DVec3};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Write};
use std::path::Path;

/// FBX time unit: ticks per second.
const KTIME: f64 = 46_186_158_000.0;
const MAGIC: &[u8] = b"Kaydara FBX Binary  \0\x1a\0";
const ROTATE_ORDERS: [&str; 6] = ["xyz", "xzy", "yzx", "yxz", "zxy", "zyx"];

// ---------------------------------------------------------------- node tree

#[derive(Clone, Debug, PartialEq)]
pub enum Prop {
    Bool(bool),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Str(String),
    Raw(Vec<u8>),
    ABool(Vec<bool>),
    AI32(Vec<i32>),
    AI64(Vec<i64>),
    AF32(Vec<f32>),
    AF64(Vec<f64>),
}

impl Prop {
    fn as_f64(&self) -> Option<f64> {
        Some(match self {
            Prop::Bool(b) => *b as i32 as f64,
            Prop::I16(x) => *x as f64,
            Prop::I32(x) => *x as f64,
            Prop::I64(x) => *x as f64,
            Prop::F32(x) => *x as f64,
            Prop::F64(x) => *x,
            _ => return None,
        })
    }
    fn as_i64(&self) -> Option<i64> {
        match self {
            Prop::I64(x) => Some(*x),
            p => p.as_f64().map(|x| x as i64),
        }
    }
    fn as_str(&self) -> Option<&str> {
        match self {
            Prop::Str(s) => Some(s),
            _ => None,
        }
    }
    fn f64s(&self) -> Vec<f64> {
        match self {
            Prop::ABool(a) => a.iter().map(|&b| b as i32 as f64).collect(),
            Prop::AI32(a) => a.iter().map(|&x| x as f64).collect(),
            Prop::AI64(a) => a.iter().map(|&x| x as f64).collect(),
            Prop::AF32(a) => a.iter().map(|&x| x as f64).collect(),
            Prop::AF64(a) => a.clone(),
            p => p.as_f64().into_iter().collect(),
        }
    }
    fn i64s(&self) -> Vec<i64> {
        match self {
            Prop::AI32(a) => a.iter().map(|&x| x as i64).collect(),
            Prop::AI64(a) => a.clone(),
            p => p.f64s().into_iter().map(|x| x as i64).collect(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FNode {
    pub name: String,
    pub props: Vec<Prop>,
    pub kids: Vec<FNode>,
}

impl FNode {
    fn new(name: &str) -> Self {
        FNode { name: name.into(), ..Default::default() }
    }
    fn p(mut self, p: Prop) -> Self {
        self.props.push(p);
        self
    }
    fn kid(mut self, k: FNode) -> Self {
        self.kids.push(k);
        self
    }
    fn child(&self, name: &str) -> Option<&FNode> {
        self.kids.iter().find(|k| k.name == name)
    }
    fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a FNode> + 'a {
        self.kids.iter().filter(move |k| k.name == name)
    }
    fn arr_f64(&self, name: &str) -> Vec<f64> {
        self.child(name).and_then(|c| c.props.first()).map(Prop::f64s).unwrap_or_default()
    }
    fn arr_i64(&self, name: &str) -> Vec<i64> {
        self.child(name).and_then(|c| c.props.first()).map(Prop::i64s).unwrap_or_default()
    }
    fn str_of(&self, name: &str) -> Option<&str> {
        self.child(name).and_then(|c| c.props.first()).and_then(Prop::as_str)
    }
    fn id(&self) -> Option<i64> {
        self.props.first().and_then(Prop::as_i64)
    }
    /// The `Properties70` block: name -> values.
    fn props70(&self) -> HashMap<String, Vec<Prop>> {
        self.child("Properties70")
            .map(|p| p.children("P").filter_map(|e| Some((e.props.first()?.as_str()?.to_string(), e.props.get(4..).map(|v| v.to_vec()).unwrap_or_default()))).collect())
            .unwrap_or_default()
    }
}

/// Object name without its class ("Cube\0\x01Model" in binary, "Model::Cube" in ASCII).
fn obj_name(o: &FNode) -> String {
    let s = o.props.get(1).and_then(Prop::as_str).unwrap_or("");
    if let Some((n, _)) = s.split_once("\0\x01") {
        n.to_string()
    } else if let Some((_, n)) = s.split_once("::") {
        n.to_string()
    } else {
        s.to_string()
    }
}
fn subtype(o: &FNode) -> &str {
    o.props.get(2).and_then(Prop::as_str).unwrap_or("")
}

type P70 = HashMap<String, Vec<Prop>>;
fn pf(p: &P70, k: &str) -> Option<f64> {
    p.get(k).and_then(|v| v.first()).and_then(Prop::as_f64)
}
fn pv3(p: &P70, k: &str) -> Option<DVec3> {
    let v = p.get(k)?;
    Some(DVec3::new(v.first()?.as_f64()?, v.get(1)?.as_f64()?, v.get(2)?.as_f64()?))
}

// ---------------------------------------------------------------- binary reader

struct Rd<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let e = self.p.checked_add(n).filter(|&e| e <= self.b.len()).ok_or("FBX: unexpected end of file")?;
        let s = &self.b[self.p..e];
        self.p = e;
        Ok(s)
    }
    fn arr<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.take(N)?.try_into().unwrap())
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.arr()?))
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.arr()?))
    }
}

fn read_binary(b: &[u8]) -> Result<(u32, Vec<FNode>), String> {
    let ver = u32::from_le_bytes(b.get(23..27).ok_or("FBX: truncated header")?.try_into().unwrap());
    let wide = ver >= 7500;
    let mut r = Rd { b, p: 27 };
    let mut out = vec![];
    while let Some(n) = read_node(&mut r, wide)? {
        out.push(n);
    }
    Ok((ver, out))
}

fn read_node(r: &mut Rd, wide: bool) -> Result<Option<FNode>, String> {
    if r.p + if wide { 25 } else { 13 } > r.b.len() {
        return Ok(None);
    }
    let (end, np) = if wide {
        let (e, n, _) = (r.u64()?, r.u64()?, r.u64()?);
        (e, n)
    } else {
        let (e, n, _) = (r.u32()?, r.u32()?, r.u32()?);
        (e as u64, n as u64)
    };
    let nl = r.take(1)?[0] as usize;
    if end == 0 {
        return Ok(None);
    }
    if end as usize > r.b.len() {
        return Err("FBX: node runs past the end of the file".into());
    }
    let name = String::from_utf8_lossy(r.take(nl)?).into_owned();
    let mut props = Vec::with_capacity(np as usize);
    for _ in 0..np {
        props.push(read_prop(r)?);
    }
    let mut kids = vec![];
    while (r.p as u64) < end {
        match read_node(r, wide)? {
            Some(k) => kids.push(k),
            None => break,
        }
    }
    r.p = end as usize;
    Ok(Some(FNode { name, props, kids }))
}

fn read_prop(r: &mut Rd) -> Result<Prop, String> {
    let t = r.take(1)?[0];
    Ok(match t {
        b'C' => Prop::Bool(r.take(1)?[0] != 0),
        b'Y' => Prop::I16(i16::from_le_bytes(r.arr()?)),
        b'I' => Prop::I32(i32::from_le_bytes(r.arr()?)),
        b'L' => Prop::I64(i64::from_le_bytes(r.arr()?)),
        b'F' => Prop::F32(f32::from_le_bytes(r.arr()?)),
        b'D' => Prop::F64(f64::from_le_bytes(r.arr()?)),
        b'S' | b'R' => {
            let n = r.u32()? as usize;
            let d = r.take(n)?;
            if t == b'S' {
                Prop::Str(String::from_utf8_lossy(d).into_owned())
            } else {
                Prop::Raw(d.to_vec())
            }
        }
        b'f' | b'd' | b'l' | b'i' | b'b' => {
            let (n, enc, len) = (r.u32()? as usize, r.u32()?, r.u32()? as usize);
            let raw = r.take(len)?;
            let data = if enc == 1 {
                let mut v = Vec::with_capacity(n * 8);
                flate2::read::ZlibDecoder::new(raw).read_to_end(&mut v).map_err(|e| format!("FBX: bad compressed array: {e}"))?;
                v
            } else {
                raw.to_vec()
            };
            let w = match t {
                b'f' | b'i' => 4,
                b'b' => 1,
                _ => 8,
            };
            if data.len() < n * w {
                return Err("FBX: short array".into());
            }
            let c = data.chunks_exact(w).take(n);
            match t {
                b'f' => Prop::AF32(c.map(|x| f32::from_le_bytes(x.try_into().unwrap())).collect()),
                b'd' => Prop::AF64(c.map(|x| f64::from_le_bytes(x.try_into().unwrap())).collect()),
                b'l' => Prop::AI64(c.map(|x| i64::from_le_bytes(x.try_into().unwrap())).collect()),
                b'i' => Prop::AI32(c.map(|x| i32::from_le_bytes(x.try_into().unwrap())).collect()),
                _ => Prop::ABool(c.map(|x| x[0] != 0).collect()),
            }
        }
        t => return Err(format!("FBX: unknown property type '{}'", t as char)),
    })
}

// ---------------------------------------------------------------- ASCII reader

#[derive(Debug, PartialEq)]
enum Tok {
    Key(String),
    Str(String),
    Num(String),
    Word(String),
    Open,
    Close,
    Comma,
    Star,
}

fn lex(src: &str) -> Vec<Tok> {
    let c: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = vec![];
    while i < c.len() {
        let ch = c[i];
        match ch {
            ';' => {
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
            }
            '{' => {
                out.push(Tok::Open);
                i += 1;
            }
            '}' => {
                out.push(Tok::Close);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            '*' => {
                out.push(Tok::Star);
                i += 1;
            }
            '"' => {
                let mut s = String::new();
                i += 1;
                while i < c.len() && c[i] != '"' {
                    s.push(c[i]);
                    i += 1;
                }
                i += 1;
                // ASCII files spell the binary "\0\x01" name separator as "::"
                out.push(Tok::Str(s));
            }
            c0 if c0.is_whitespace() => i += 1,
            _ => {
                let st = i;
                while i < c.len() && !c[i].is_whitespace() && !matches!(c[i], ',' | '{' | '}' | ':' | '"' | ';') {
                    i += 1;
                }
                let w: String = c[st..i].iter().collect();
                if i < c.len() && c[i] == ':' {
                    out.push(Tok::Key(w));
                    i += 1;
                } else if w.starts_with(|x: char| x.is_ascii_digit() || x == '-' || x == '+' || x == '.') {
                    out.push(Tok::Num(w));
                } else if w.is_empty() {
                    i += 1;
                } else {
                    out.push(Tok::Word(w));
                }
            }
        }
    }
    out
}

fn num(s: &str) -> Prop {
    if s.contains(['.', 'e', 'E']) {
        Prop::F64(s.parse().unwrap_or(0.0))
    } else {
        s.parse::<i64>().map(Prop::I64).unwrap_or_else(|_| Prop::F64(s.parse().unwrap_or(0.0)))
    }
}

fn ascii_nodes(t: &[Tok], i: &mut usize) -> Vec<FNode> {
    let mut out = vec![];
    while *i < t.len() {
        match &t[*i] {
            Tok::Close => {
                *i += 1;
                break;
            }
            Tok::Key(k) => {
                *i += 1;
                let mut n = FNode::new(k);
                loop {
                    match t.get(*i) {
                        Some(Tok::Str(s)) => {
                            n.props.push(Prop::Str(s.clone()));
                            *i += 1;
                        }
                        Some(Tok::Num(x)) => {
                            n.props.push(num(x));
                            *i += 1;
                        }
                        Some(Tok::Word(w)) => {
                            n.props.push(Prop::Str(w.clone()));
                            *i += 1;
                        }
                        Some(Tok::Comma) => *i += 1,
                        Some(Tok::Star) => {
                            // *N { a: v, v, ... }
                            *i += 1;
                            if matches!(t.get(*i), Some(Tok::Num(_))) {
                                *i += 1;
                            }
                            if t.get(*i) == Some(&Tok::Open) {
                                *i += 1;
                            }
                            if matches!(t.get(*i), Some(Tok::Key(_))) {
                                *i += 1;
                            }
                            let mut vals: Vec<&str> = vec![];
                            while let Some(tk) = t.get(*i) {
                                *i += 1;
                                match tk {
                                    Tok::Num(x) => vals.push(x),
                                    Tok::Close => break,
                                    _ => {}
                                }
                            }
                            if vals.iter().any(|v| v.contains(['.', 'e', 'E'])) {
                                n.props.push(Prop::AF64(vals.iter().map(|v| v.parse().unwrap_or(0.0)).collect()));
                            } else {
                                n.props.push(Prop::AI64(vals.iter().map(|v| v.parse().unwrap_or(0)).collect()));
                            }
                        }
                        Some(Tok::Open) => {
                            *i += 1;
                            n.kids = ascii_nodes(t, i);
                            break;
                        }
                        _ => break,
                    }
                }
                out.push(n);
            }
            _ => *i += 1,
        }
    }
    out
}

/// Parse a binary or ASCII FBX file into its node tree; returns (version, top-level nodes).
pub fn parse(bytes: &[u8]) -> Result<(u32, Vec<FNode>), String> {
    if bytes.starts_with(&MAGIC[..20]) {
        return read_binary(bytes);
    }
    let src = std::str::from_utf8(bytes).map_err(|_| "not an FBX file")?;
    if !src.contains("FBXHeaderExtension") {
        return Err("not an FBX file".into());
    }
    let mut i = 0;
    let nodes = ascii_nodes(&lex(src), &mut i);
    let ver = nodes.iter().find(|n| n.name == "FBXHeaderExtension").and_then(|h| h.child("FBXVersion")).and_then(|v| v.props.first()).and_then(Prop::as_i64).unwrap_or(0) as u32;
    Ok((ver, nodes))
}

// ---------------------------------------------------------------- import

fn mat16(v: &[f64]) -> Option<DMat4> {
    (v.len() == 16).then(|| DMat4::from_cols_slice(v))
}

/// FBX local = T * Roff * Rp * Rpre * R * Rpost^-1 * Rp^-1 * Soff * Sp * S * Sp^-1
struct FbxXf {
    t: DVec3,
    r: DVec3,
    s: DVec3,
    pre: DVec3,
    post: DVec3,
    ro: usize,
    rp: DVec3,
    sp: DVec3,
    roff: DVec3,
    soff: DVec3,
}

impl FbxXf {
    fn read(p: &P70, unit: f64) -> Self {
        let z = DVec3::ZERO;
        FbxXf {
            t: pv3(p, "Lcl Translation").unwrap_or(z) * unit,
            r: pv3(p, "Lcl Rotation").unwrap_or(z),
            s: pv3(p, "Lcl Scaling").unwrap_or(DVec3::ONE),
            pre: pv3(p, "PreRotation").unwrap_or(z),
            post: pv3(p, "PostRotation").unwrap_or(z),
            ro: (pf(p, "RotationOrder").unwrap_or(0.0) as usize).min(5),
            rp: pv3(p, "RotationPivot").unwrap_or(z) * unit,
            sp: pv3(p, "ScalingPivot").unwrap_or(z) * unit,
            roff: pv3(p, "RotationOffset").unwrap_or(z) * unit,
            soff: pv3(p, "ScalingOffset").unwrap_or(z) * unit,
        }
    }
    fn simple(&self) -> bool {
        self.post == DVec3::ZERO && self.roff == DVec3::ZERO && self.soff == DVec3::ZERO && self.rp.distance(self.sp) < 1e-9
    }
    fn matrix(&self) -> DMat4 {
        let q = |e: DVec3, ro: &str| DMat4::from_quat(quat_from_euler_deg(e.to_array(), ro));
        DMat4::from_translation(self.t + self.roff + self.rp)
            * q(self.pre, "xyz")
            * q(self.r, ROTATE_ORDERS[self.ro])
            * q(self.post, "xyz").inverse()
            * DMat4::from_translation(self.soff + self.sp - self.rp)
            * DMat4::from_scale(self.s)
            * DMat4::from_translation(-self.sp)
    }
}

struct GeoOut {
    pm: PolyMesh,
    face_mats: Option<Vec<i64>>,
}

/// Mesh data of a Geometry object; `gm` is the model's geometric transform.
fn geometry_mesh(g: &FNode, unit: f64, gm: DMat4) -> GeoOut {
    let v: Vec<DVec3> = g.arr_f64("Vertices").chunks_exact(3).map(|c| gm.transform_point3(DVec3::new(c[0], c[1], c[2]) * unit)).collect();
    let pvi = g.arr_i64("PolygonVertexIndex");
    // faces as (first corner index, vertex indices)
    let mut faces: Vec<(usize, Vec<u32>)> = vec![];
    let mut cur: Vec<u32> = vec![];
    let mut start = 0;
    for (k, &x) in pvi.iter().enumerate() {
        let (vi, last) = if x < 0 { (!x, true) } else { (x, false) };
        cur.push(vi as u32);
        if last {
            faces.push((start, std::mem::take(&mut cur)));
            start = k + 1;
        }
    }
    let layer = |name: &str| g.child(name);
    // UVs
    let uv_of: Option<Box<dyn Fn(usize, u32) -> DVec2>> = layer("LayerElementUV").map(|l| {
        let uvs: Vec<DVec2> = l.arr_f64("UV").chunks_exact(2).map(|c| DVec2::new(c[0], c[1])).collect();
        let idx = l.arr_i64("UVIndex");
        let by_corner = l.str_of("MappingInformationType").unwrap_or("ByPolygonVertex") == "ByPolygonVertex";
        let indexed = l.str_of("ReferenceInformationType").unwrap_or("Direct") != "Direct";
        Box::new(move |k: usize, vi: u32| {
            let i = if by_corner { k } else { vi as usize };
            let i = if indexed { idx.get(i).copied().unwrap_or(0).max(0) as usize } else { i };
            uvs.get(i).copied().unwrap_or(DVec2::ZERO)
        }) as Box<dyn Fn(usize, u32) -> DVec2>
    });
    // per-polygon material slots
    let mats: Option<Vec<i64>> = layer("LayerElementMaterial").and_then(|l| {
        let m = l.arr_i64("Materials");
        (l.str_of("MappingInformationType") == Some("ByPolygon") && m.len() >= faces.len()).then_some(m)
    });
    let mut pm = PolyMesh { v, ..Default::default() };
    let mut uvs = vec![];
    let mut fm = vec![];
    for (fi, (k0, f)) in faces.iter().enumerate() {
        if f.len() < 3 || f.iter().any(|&i| i as usize >= pm.v.len()) {
            continue;
        }
        if let Some(u) = &uv_of {
            uvs.push(Some(f.iter().enumerate().map(|(j, &vi)| u(k0 + j, vi)).collect()));
        }
        if let Some(m) = &mats {
            fm.push(m[fi]);
        }
        pm.f.push(f.clone());
    }
    if uv_of.is_some() {
        pm.uv = Some(uvs);
    }
    // smooth normals in the file (shared per vertex) -> smooth shading
    if let Some(l) = layer("LayerElementNormal") {
        let n: Vec<DVec3> = l.arr_f64("Normals").chunks_exact(3).map(|c| DVec3::new(c[0], c[1], c[2])).collect();
        let by_corner = l.str_of("MappingInformationType").unwrap_or("ByPolygonVertex") == "ByPolygonVertex";
        let idx = l.arr_i64("NormalsIndex");
        let indexed = l.str_of("ReferenceInformationType").unwrap_or("Direct") != "Direct";
        if !by_corner {
            pm.soft_angle = 180.0;
        } else {
            let mut per_v: HashMap<u32, DVec3> = HashMap::new();
            let (mut same, mut total) = (0usize, 0usize);
            for (k0, f) in &faces {
                for (j, &vi) in f.iter().enumerate() {
                    let k = if indexed { idx.get(k0 + j).copied().unwrap_or(0).max(0) as usize } else { k0 + j };
                    let Some(nn) = n.get(k) else { continue };
                    if let Some(prev) = per_v.get(&vi) {
                        total += 1;
                        same += (prev.dot(*nn) > 0.999) as usize;
                    } else {
                        per_v.insert(vi, *nn);
                    }
                }
            }
            if total > 0 && same as f64 / total as f64 > 0.95 {
                pm.soft_angle = 180.0;
            }
        }
    }
    GeoOut { pm, face_mats: mats.map(|_| fm) }
}

fn fps_of_mode(mode: i64, custom: f64) -> Option<f64> {
    Some(match mode {
        1 => 120.0,
        2 => 100.0,
        3 => 60.0,
        4 => 50.0,
        5 => 48.0,
        6 | 7 => 30.0,
        8 | 9 => 29.97,
        10 => 25.0,
        11 => 24.0,
        12 => 1000.0,
        13 => 23.976,
        14 if custom > 0.0 => custom,
        15 => 96.0,
        16 => 72.0,
        17 => 59.94,
        18 => 119.88,
        _ => return None,
    })
}
fn mode_of_fps(fps: f64) -> i64 {
    [(120.0, 1), (100.0, 2), (60.0, 3), (50.0, 4), (48.0, 5), (30.0, 6), (25.0, 10), (24.0, 11), (96.0, 15), (72.0, 16)].iter().find(|(f, _)| (f - fps).abs() < 1e-6).map(|x| x.1).unwrap_or(14)
}

/// FBX cameras look down +X and lights down -Y; Inca's (like Maya's) look down -Z.
fn shape_fix(kind: &str) -> DMat4 {
    match kind {
        "camera" => DMat4::from_rotation_y(-std::f64::consts::FRAC_PI_2),
        "light" => DMat4::from_rotation_x(-std::f64::consts::FRAC_PI_2),
        _ => DMat4::IDENTITY,
    }
}

/// Import an FBX file into the scene; `dir` resolves relative texture paths. Returns the new
/// top-level nodes.
pub fn import_fbx(s: &mut Scene, bytes: &[u8], dir: Option<&Path>, file_stem: &str) -> Result<Vec<Id>, String> {
    let (ver, root) = parse(bytes)?;
    if ver < 7000 {
        return Err(format!("FBX {}.{} files predate FBX 2011 and aren't supported; re-export as FBX 2011 or later", ver / 1000, ver % 1000 / 100));
    }
    let top = |n: &str| root.iter().find(|x| x.name == n);
    let objects = top("Objects").ok_or("FBX: no Objects section")?;
    let obj: HashMap<i64, &FNode> = objects.kids.iter().filter_map(|o| o.id().map(|i| (i, o))).collect();
    // (kind, child, parent, property)
    let conns: Vec<(String, i64, i64, String)> = top("Connections")
        .map(|c| c.children("C").chain(c.children("Connect")).filter_map(|k| Some((k.props.first()?.as_str()?.to_string(), k.props.get(1)?.as_i64()?, k.props.get(2)?.as_i64()?, k.props.get(3).and_then(Prop::as_str).unwrap_or("").to_string()))).collect())
        .unwrap_or_default();
    let class_of = |id: i64| obj.get(&id).map(|o| o.name.as_str()).unwrap_or("");
    let kids_of = |p: i64, class: &'static str| -> Vec<i64> { conns.iter().filter(|c| c.2 == p && class_of(c.1) == class).map(|c| c.1).collect() };
    let parents_of = |c: i64, class: &'static str| -> Vec<i64> { conns.iter().filter(|x| x.1 == c && class_of(x.2) == class).map(|x| x.2).collect() };

    // ---- global settings
    let gs = top("GlobalSettings").map(|g| g.props70()).unwrap_or_default();
    let unit = pf(&gs, "UnitScaleFactor").filter(|u| *u > 0.0).unwrap_or(1.0);
    let axis = |k: &str, ks: &str, d: usize| {
        let a = (pf(&gs, k).unwrap_or(d as f64) as usize).min(2);
        let sgn = pf(&gs, ks).unwrap_or(1.0).signum();
        DVec3::from_array([0, 1, 2].map(|i| if i == a { sgn } else { 0.0 }))
    };
    let (up, front, coord) = (axis("UpAxis", "UpAxisSign", 1), axis("FrontAxis", "FrontAxisSign", 2), axis("CoordAxis", "CoordAxisSign", 0));
    let basis = DMat4::from_cols(coord.extend(0.0), up.extend(0.0), front.extend(0.0), glam::DVec4::W);
    let to_yup = basis.transpose();
    if let Some(fps) = fps_of_mode(pf(&gs, "TimeMode").unwrap_or(0.0) as i64, pf(&gs, "CustomFrameRate").unwrap_or(0.0)) {
        s.time.fps = fps;
    }
    let fps = s.time.fps;
    let frame = |ticks: f64| (ticks / KTIME * fps * 1000.0).round() / 1000.0;

    // ---- materials and textures
    let mut mats: HashMap<i64, Id> = HashMap::new();
    for (&oid, o) in obj.iter().filter(|(_, o)| o.name == "Material") {
        let shading = o.str_of("ShadingModel").unwrap_or("lambert").to_ascii_lowercase();
        let ty = if shading.contains("phong") { "phong" } else if shading.contains("blinn") { "blinn" } else { "lambert" };
        let name = obj_name(o);
        let id = s.create_material(ty, Some(if name.is_empty() { "fbxMaterial1" } else { &name }));
        let p = o.props70();
        let r = NodeRef::Material(id.clone());
        let col = |v: DVec3| json!([v.x, v.y, v.z]);
        if let Some(c) = pv3(&p, "DiffuseColor").or_else(|| pv3(&p, "Diffuse")) {
            s.set_attr(&r, "color", col(c));
        }
        if let Some(f) = pf(&p, "DiffuseFactor") {
            s.set_attr(&r, "diffuse", json!(f));
        }
        let tf = pf(&p, "TransparencyFactor").unwrap_or(0.0);
        if let Some(c) = pv3(&p, "TransparentColor").filter(|_| tf > 0.0) {
            s.set_attr(&r, "transparency", col(c * tf));
        } else if let Some(op) = pf(&p, "Opacity").filter(|o| *o < 1.0) {
            s.set_attr(&r, "transparency", col(DVec3::splat(1.0 - op)));
        }
        if let Some(c) = pv3(&p, "EmissiveColor").map(|c| c * pf(&p, "EmissiveFactor").unwrap_or(1.0)).filter(|c| c.length() > 0.0) {
            s.set_attr(&r, "incandescence", col(c));
        }
        if ty != "lambert" {
            if let Some(c) = pv3(&p, "SpecularColor") {
                s.set_attr(&r, "specularColor", col(c * pf(&p, "SpecularFactor").unwrap_or(1.0)));
            }
            if let Some(x) = pf(&p, "ShininessExponent").or_else(|| pf(&p, "Shininess")) {
                s.set_attr(&r, "cosinePower", json!(x.max(2.0)));
            }
        }
        // a file texture on the diffuse colour
        let tex = conns.iter().find(|c| c.2 == oid && class_of(c.1) == "Texture" && (c.3.contains("DiffuseColor") || c.3.contains("baseColor") || c.3 == "Diffuse")).and_then(|c| obj.get(&c.1));
        if let Some(t) = tex {
            let file = t.str_of("FileName").unwrap_or("").to_string();
            let rel = t.str_of("RelativeFilename").unwrap_or("").to_string();
            let resolved = [file.clone()]
                .into_iter()
                .chain(dir.into_iter().flat_map(|d| [d.join(&rel).display().to_string(), d.join(Path::new(&file.replace('\\', "/")).file_name().unwrap_or_default()).display().to_string()]))
                .find(|p| !p.is_empty() && Path::new(p).is_file())
                .unwrap_or(if file.is_empty() { rel } else { file });
            let tid = s.create_texture("file", Some(&obj_name(t)));
            s.set_attr(&NodeRef::Texture(tid.clone()), "fileTextureName", json!(resolved));
            if let Some(m) = s.mats.get_mut(&id) {
                m.maps.insert("color".into(), json!(tid));
            }
        }
        mats.insert(oid, id);
    }

    // ---- models, parents first
    let models: Vec<i64> = objects.kids.iter().filter(|o| o.name == "Model").filter_map(FNode::id).collect();
    let model_parent: HashMap<i64, i64> = models.iter().filter_map(|&m| parents_of(m, "Model").first().map(|&p| (m, p))).collect();
    let mut order: Vec<i64> = vec![];
    let mut stack: Vec<i64> = models.iter().copied().filter(|m| !model_parent.contains_key(m)).rev().collect();
    while let Some(m) = stack.pop() {
        order.push(m);
        let kids: Vec<i64> = models.iter().copied().filter(|k| model_parent.get(k) == Some(&m)).collect();
        stack.extend(kids.into_iter().rev());
    }
    let mut dag: HashMap<i64, Id> = HashMap::new();
    let mut geo_model: HashMap<i64, i64> = HashMap::new();
    let mut made: Vec<Id> = vec![];
    for &mid in &order {
        let o = obj[&mid];
        let name = obj_name(o);
        let name = if name.is_empty() { "fbxNode1".to_string() } else { name };
        let p = o.props70();
        let ty = subtype(o);
        let attr = conns.iter().find(|c| c.2 == mid && class_of(c.1) == "NodeAttribute").and_then(|c| obj.get(&c.1)).copied();
        let geo = kids_of(mid, "Geometry").into_iter().find(|g| subtype(obj[g]) == "Mesh");
        let gm = DMat4::from_scale_rotation_translation(
            pv3(&p, "GeometricScaling").unwrap_or(DVec3::ONE),
            quat_from_euler_deg(pv3(&p, "GeometricRotation").unwrap_or(DVec3::ZERO).to_array(), "xyz"),
            pv3(&p, "GeometricTranslation").unwrap_or(DVec3::ZERO) * unit,
        );
        let mut kind = "";
        let id = if let Some(g) = geo {
            geo_model.insert(g, mid);
            let out = geometry_mesh(obj[&g], unit, gm);
            let slots: Vec<Id> = conns.iter().filter(|c| c.2 == mid && class_of(c.1) == "Material").filter_map(|c| mats.get(&c.1).cloned()).collect();
            let id = s.create_mesh_from(out.pm.clone(), &name, slots.first().map(String::as_str), None);
            if let (Some(fm), true) = (out.face_mats, slots.len() > 1) {
                let per: Vec<Option<String>> = fm.iter().map(|&i| slots.get(i.max(0) as usize).cloned()).collect();
                let mut pm = out.pm;
                pm.fm = Some(per);
                s.bake_mesh(&id, pm);
            }
            id
        } else if matches!(ty, "LimbNode" | "Limb" | "Root") || attr.is_some_and(|a| subtype(a) == "LimbNode") {
            s.create_handle("joint", &name)
        } else if ty == "Camera" || attr.is_some_and(|a| subtype(a) == "Camera") {
            kind = "camera";
            let ap = attr.map(|a| a.props70()).unwrap_or_default();
            let mut cam = CamShape::default();
            if let Some(f) = pf(&ap, "FocalLength") {
                cam.focal_length = f;
            }
            if let Some(w) = pf(&ap, "FilmWidth") {
                cam.horizontal_film_aperture = w;
            }
            if let Some(h) = pf(&ap, "FilmHeight") {
                cam.vertical_film_aperture = h;
            }
            if let Some(n) = pf(&ap, "NearPlane") {
                cam.near = n * unit;
            }
            if let Some(f) = pf(&ap, "FarPlane") {
                cam.far = f * unit;
            }
            cam.ortho = pf(&ap, "CameraProjectionType") == Some(1.0);
            if let Some(z) = pf(&ap, "OrthoZoom") {
                cam.ortho_width = 30.0 * z;
            }
            s.create_camera(&name, cam, false)
        } else if ty == "Light" || attr.is_some_and(|a| subtype(a) == "Light") {
            kind = "light";
            let ap = attr.map(|a| a.props70()).unwrap_or_default();
            let lt = match pf(&ap, "LightType").unwrap_or(0.0) as i64 {
                1 => "directionalLight",
                2 => "spotLight",
                3 => "areaLight",
                _ => "pointLight",
            };
            let id = s.create_light(lt);
            let r = NodeRef::Dag(id.clone());
            if let Some(c) = pv3(&ap, "Color") {
                s.set_attr(&r, "color", json!([c.x, c.y, c.z]));
            }
            if let Some(i) = pf(&ap, "Intensity") {
                s.set_attr(&r, "intensity", json!(i / 100.0));
            }
            if let Some(a) = pf(&ap, "OuterAngle").or_else(|| pf(&ap, "Cone angle")) {
                s.set_attr(&r, "coneAngle", json!(a));
            }
            if let Some(c) = pf(&ap, "CastShadows") {
                s.set_attr(&r, "shadows", json!(c != 0.0));
            }
            crate::commands::rename(s, &id, &name);
            id
        } else {
            s.create_group(&name, None)
        };
        let parent = model_parent.get(&mid).and_then(|p| dag.get(p)).cloned();
        if let Some(pa) = &parent {
            s.reparent(&id, Some(pa), None);
        }
        let fx = FbxXf::read(&p, unit);
        {
            let n = s.nodes.get_mut(&id).unwrap();
            n.xf.ro = ROTATE_ORDERS[fx.ro].into();
            n.xf.jo = fx.pre.to_array();
            if fx.simple() {
                n.xf.t = fx.t.to_array();
                n.xf.r = fx.r.to_array();
                n.xf.s = fx.s.to_array();
                n.xf.p = fx.rp.to_array();
            }
            if pf(&p, "Visibility") == Some(0.0) || pf(&p, "Show") == Some(0.0) {
                n.visible = false;
            }
        }
        if !fx.simple() {
            s.set_local_matrix(&id, fx.matrix());
        }
        if !kind.is_empty() {
            let m = s.local_matrix(&id) * shape_fix(kind);
            s.set_local_matrix(&id, m);
        }
        if parent.is_none() {
            made.push(id.clone());
        }
        dag.insert(mid, id);
    }

    // ---- bind poses: node -> world matrix
    let mut bind_pose: HashMap<i64, DMat4> = HashMap::new();
    let scale_m = |m: DMat4| {
        let mut m = m;
        m.w_axis = (m.w_axis.truncate() * unit).extend(m.w_axis.w);
        m
    };
    for o in objects.kids.iter().filter(|o| o.name == "Pose" && subtype(o) == "BindPose") {
        for pn in o.children("PoseNode") {
            if let (Some(n), Some(m)) = (pn.child("Node").and_then(|x| x.props.first()).and_then(Prop::as_i64), mat16(&pn.arr_f64("Matrix"))) {
                bind_pose.insert(n, scale_m(m));
            }
        }
    }

    // ---- skins
    for (&skid, _) in obj.iter().filter(|(_, o)| o.name == "Deformer" && subtype(o) == "Skin") {
        let Some(geo) = parents_of(skid, "Geometry").first().copied() else { continue };
        let Some(mesh) = geo_model.get(&geo).and_then(|m| dag.get(m)).cloned() else { continue };
        let mesh_model = geo_model[&geo];
        let nv = s.node(&mesh).and_then(|n| n.mesh()).map(|m| m.mesh.v.len()).unwrap_or(0);
        let mut joints: Vec<Id> = vec![];
        let mut bind_inv: Vec<Value> = vec![];
        let mut mesh_bind: Option<DMat4> = None;
        let mut w: Vec<Vec<(usize, f64)>> = vec![vec![]; nv];
        for cl in kids_of(skid, "Deformer") {
            let c = obj[&cl];
            let Some(bone) = kids_of(cl, "Model").first().copied() else { continue };
            let Some(jid) = dag.get(&bone).cloned() else { continue };
            let tl = mat16(&c.arr_f64("TransformLink")).map(scale_m).or_else(|| bind_pose.get(&bone).copied()).unwrap_or_else(|| s.world_matrix(&jid));
            if mesh_bind.is_none() {
                mesh_bind = mat16(&c.arr_f64("Transform")).map(scale_m).map(|t| tl * t).or_else(|| bind_pose.get(&mesh_model).copied());
            }
            let slot = joints.len();
            joints.push(jid);
            bind_inv.push(json!(tl.inverse().to_cols_array()));
            for (i, x) in c.arr_i64("Indexes").iter().zip(c.arr_f64("Weights")) {
                if let Some(e) = w.get_mut(*i as usize).filter(|_| x > 0.0) {
                    e.push((slot, x));
                }
            }
        }
        if joints.is_empty() {
            continue;
        }
        let weights: Vec<Value> = w
            .iter()
            .map(|x| {
                let t: f64 = x.iter().map(|e| e.1).sum::<f64>().max(1e-12);
                json!(x.iter().map(|(j, v)| json!([j, v / t])).collect::<Vec<_>>())
            })
            .collect();
        let mut p = Params::new();
        p.insert("joints".into(), json!(joints));
        p.insert("bindInv".into(), Value::Array(bind_inv));
        p.insert("meshBind".into(), json!(mesh_bind.unwrap_or_else(|| s.world_matrix(&mesh)).to_cols_array()));
        p.insert("weights".into(), Value::Array(weights));
        p.insert("owner".into(), json!(mesh));
        s.add_history(&mesh, "skinCluster", p);
    }

    // ---- animation: first stack, its layers' curve nodes
    let stack = objects.kids.iter().find(|o| o.name == "AnimationStack").and_then(FNode::id);
    let layers: Vec<i64> = match stack {
        Some(st) => kids_of(st, "AnimationLayer"),
        None => vec![],
    };
    let mut keyed = false;
    for cn in layers.iter().flat_map(|l| kids_of(*l, "AnimationCurveNode")) {
        let Some((target, prop)) = conns.iter().find(|c| c.1 == cn && c.0 == "OP" && dag.contains_key(&c.2)).map(|c| (dag[&c.2].clone(), c.3.clone())) else { continue };
        let base = match prop.as_str() {
            "Lcl Translation" => "translate",
            "Lcl Rotation" => "rotate",
            "Lcl Scaling" => "scale",
            "Visibility" => "visibility",
            _ => continue,
        };
        for (cid, ch) in conns.iter().filter(|c| c.2 == cn && class_of(c.1) == "AnimationCurve").map(|c| (c.1, c.3.clone())) {
            let attr = match ch.as_str() {
                "d|X" => format!("{base}X"),
                "d|Y" => format!("{base}Y"),
                "d|Z" => format!("{base}Z"),
                _ if base == "visibility" => base.into(),
                _ => continue,
            };
            let c = obj[&cid];
            let times = c.arr_i64("KeyTime");
            let vals = c.arr_f64("KeyValueFloat");
            // per-key interpolation from KeyAttrFlags expanded by KeyAttrRefCount
            let flags = c.arr_i64("KeyAttrFlags");
            let refs = c.arr_i64("KeyAttrRefCount");
            let mut per_key: Vec<i64> = refs.iter().zip(&flags).flat_map(|(&n, &f)| std::iter::repeat_n(f, n.max(0) as usize)).collect();
            per_key.resize(times.len(), *flags.first().unwrap_or(&0x108));
            for ((&t, &v), &fl) in times.iter().zip(&vals).zip(&per_key) {
                let v = if base == "translate" { v * unit } else { v };
                let f = frame(t as f64);
                s.set_key(&target, &attr, Some(f), Some(v));
                let tan = if fl & 0x2 != 0 { "step" } else if fl & 0x4 != 0 { "linear" } else { "" };
                if let Some(k) = s.anim.curve_mut(&target, &attr, false).and_then(|c| c.find(f).map(|i| &mut c.keys[i])).filter(|_| !tan.is_empty()) {
                    k.it = if tan == "step" { "linear".into() } else { tan.into() };
                    k.ot = tan.into();
                }
                keyed = true;
            }
        }
    }
    if keyed {
        let ks: Vec<f64> = s.anim.curves.values().flat_map(|c| c.keys.iter().map(|k| k.t)).collect();
        let (lo, hi) = ks.iter().fold((f64::MAX, f64::MIN), |(a, b), &t| (a.min(t), b.max(t)));
        if hi > lo {
            s.time.start = lo;
            s.time.end = hi;
            s.time.anim_start = s.time.anim_start.min(lo);
            s.time.anim_end = s.time.anim_end.max(hi);
        }
    }

    // a file that isn't Y-up goes under a group that turns it upright
    if to_yup.abs_diff_eq(DMat4::IDENTITY, 1e-9) || made.is_empty() {
        s.apply_anim();
        s.touch();
        return Ok(made);
    }
    let g = s.create_group(&format!("{file_stem}_yUp"), None);
    for m in &made {
        s.reparent(m, Some(&g), None);
    }
    s.set_local_matrix(&g, to_yup);
    s.apply_anim();
    s.touch();
    Ok(vec![g])
}

// ---------------------------------------------------------------- writing

fn write_prop(o: &mut Vec<u8>, p: &Prop) {
    fn arr(o: &mut Vec<u8>, t: u8, n: usize, raw: Vec<u8>) {
        o.push(t);
        o.extend((n as u32).to_le_bytes());
        if raw.len() > 128 {
            let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            let _ = e.write_all(&raw);
            let z = e.finish().unwrap_or_default();
            o.extend(1u32.to_le_bytes());
            o.extend((z.len() as u32).to_le_bytes());
            o.extend(z);
        } else {
            o.extend(0u32.to_le_bytes());
            o.extend((raw.len() as u32).to_le_bytes());
            o.extend(raw);
        }
    }
    match p {
        Prop::Bool(b) => o.extend([b'C', *b as u8]),
        Prop::I16(x) => {
            o.push(b'Y');
            o.extend(x.to_le_bytes())
        }
        Prop::I32(x) => {
            o.push(b'I');
            o.extend(x.to_le_bytes())
        }
        Prop::I64(x) => {
            o.push(b'L');
            o.extend(x.to_le_bytes())
        }
        Prop::F32(x) => {
            o.push(b'F');
            o.extend(x.to_le_bytes())
        }
        Prop::F64(x) => {
            o.push(b'D');
            o.extend(x.to_le_bytes())
        }
        Prop::Str(s) => {
            o.push(b'S');
            o.extend((s.len() as u32).to_le_bytes());
            o.extend(s.as_bytes());
        }
        Prop::Raw(r) => {
            o.push(b'R');
            o.extend((r.len() as u32).to_le_bytes());
            o.extend(r);
        }
        Prop::ABool(a) => arr(o, b'b', a.len(), a.iter().map(|&b| b as u8).collect()),
        Prop::AI32(a) => arr(o, b'i', a.len(), a.iter().flat_map(|x| x.to_le_bytes()).collect()),
        Prop::AI64(a) => arr(o, b'l', a.len(), a.iter().flat_map(|x| x.to_le_bytes()).collect()),
        Prop::AF32(a) => arr(o, b'f', a.len(), a.iter().flat_map(|x| x.to_le_bytes()).collect()),
        Prop::AF64(a) => arr(o, b'd', a.len(), a.iter().flat_map(|x| x.to_le_bytes()).collect()),
    }
}

const NULL_RECORD: [u8; 13] = [0; 13];

fn write_node(o: &mut Vec<u8>, n: &FNode, is_last: bool) {
    let start = o.len();
    o.extend([0u8; 12]);
    o.push(n.name.len() as u8);
    o.extend(n.name.as_bytes());
    let ps = o.len();
    for p in &n.props {
        write_prop(o, p);
    }
    let plen = o.len() - ps;
    if !n.kids.is_empty() {
        for (i, k) in n.kids.iter().enumerate() {
            write_node(o, k, i + 1 == n.kids.len());
        }
        o.extend(NULL_RECORD);
    } else if (n.props.is_empty() && !is_last) || matches!(n.name.as_str(), "AnimationStack" | "AnimationLayer") {
        // the SDK expects these empty nodes closed with a null record
        o.extend(NULL_RECORD);
    }
    let end = o.len() as u32;
    o[start..start + 4].copy_from_slice(&end.to_le_bytes());
    o[start + 4..start + 8].copy_from_slice(&(n.props.len() as u32).to_le_bytes());
    o[start + 8..start + 12].copy_from_slice(&(plen as u32).to_le_bytes());
}

/// Encode a node tree as binary FBX 7.4.
pub fn write_binary(nodes: &[FNode]) -> Vec<u8> {
    let mut o = MAGIC.to_vec();
    o.extend(7400u32.to_le_bytes());
    for (i, n) in nodes.iter().enumerate() {
        write_node(&mut o, n, i + 1 == nodes.len());
    }
    o.extend(NULL_RECORD);
    // footer, as the FBX SDK writes it (fixed id for the fixed creation time below)
    o.extend(b"\xfa\xbc\xab\x09\xd0\xc8\xd4\x66\xb1\x76\xfb\x83\x1c\xf7\x26\x7e");
    o.extend([0u8; 4]);
    let pad = match (16 - o.len() % 16) % 16 {
        0 => 16,
        p => p,
    };
    o.extend(vec![0u8; pad]);
    o.extend(7400u32.to_le_bytes());
    o.extend([0u8; 120]);
    o.extend(b"\xf8\x5a\x8c\x6a\xde\xf5\xd9\x7e\xec\xe9\x0c\xe3\x75\x8f\x29\x0b");
    o
}

fn st(s: &str) -> Prop {
    Prop::Str(s.into())
}
fn n1(name: &str, p: Prop) -> FNode {
    FNode::new(name).p(p)
}
/// A Properties70 `P` entry.
fn pp(name: &str, ty: &str, label: &str, flags: &str, vals: Vec<Prop>) -> FNode {
    let mut n = FNode::new("P").p(st(name)).p(st(ty)).p(st(label)).p(st(flags));
    n.props.extend(vals);
    n
}
fn pd3(name: &str, ty: &str, flags: &str, v: DVec3) -> FNode {
    pp(name, ty, "", flags, vec![Prop::F64(v.x), Prop::F64(v.y), Prop::F64(v.z)])
}
fn props70(entries: Vec<FNode>) -> FNode {
    FNode { name: "Properties70".into(), props: vec![], kids: entries }
}
fn oname(name: &str, class: &str) -> Prop {
    Prop::Str(format!("{name}\0\x01{class}"))
}
fn conn(kind: &str, child: i64, parent: i64, prop: Option<&str>) -> FNode {
    let n = FNode::new("C").p(st(kind)).p(Prop::I64(child)).p(Prop::I64(parent));
    match prop {
        Some(p) => n.p(st(p)),
        None => n,
    }
}

fn geometry_node(id: i64, pm: &PolyMesh, slots: &[Id]) -> FNode {
    let verts: Vec<f64> = pm.v.iter().flat_map(|v| v.to_array()).collect();
    let pvi: Vec<i32> = pm.f.iter().flat_map(|f| f.iter().enumerate().map(move |(j, &v)| if j + 1 == f.len() { !(v as i32) } else { v as i32 })).collect();
    let normals: Vec<f64> = pm.corner_normals().iter().flatten().flat_map(|n| n.to_array()).collect();
    let mut g = FNode::new("Geometry").p(Prop::I64(id)).p(oname("", "Geometry")).p(st("Mesh"));
    g.kids.push(props70(vec![]));
    g.kids.push(n1("GeometryVersion", Prop::I32(124)));
    g.kids.push(n1("Vertices", Prop::AF64(verts)));
    g.kids.push(n1("PolygonVertexIndex", Prop::AI32(pvi)));
    let layer_el = |ty: &str, name: &str, map: &str, rf: &str| FNode::new(ty).p(Prop::I32(0)).kid(n1("Version", Prop::I32(101))).kid(n1("Name", st(name))).kid(n1("MappingInformationType", st(map))).kid(n1("ReferenceInformationType", st(rf)));
    g.kids.push(layer_el("LayerElementNormal", "", "ByPolygonVertex", "Direct").kid(n1("Normals", Prop::AF64(normals))));
    let mut layer = FNode::new("Layer").p(Prop::I32(0)).kid(n1("Version", Prop::I32(100)));
    let le = |ty: &str| FNode::new("LayerElement").kid(n1("Type", st(ty))).kid(n1("TypedIndex", Prop::I32(0)));
    layer.kids.push(le("LayerElementNormal"));
    if let Some(uv) = &pm.uv {
        let mut uniq: HashMap<[u64; 2], i32> = HashMap::new();
        let mut list: Vec<f64> = vec![];
        let mut idx: Vec<i32> = vec![];
        for (fi, f) in pm.f.iter().enumerate() {
            for j in 0..f.len() {
                let p = uv.get(fi).and_then(|x| x.as_ref()).and_then(|x| x.get(j)).copied().unwrap_or(DVec2::ZERO);
                let k = [p.x.to_bits(), p.y.to_bits()];
                let i = *uniq.entry(k).or_insert_with(|| {
                    list.extend([p.x, p.y]);
                    (list.len() / 2 - 1) as i32
                });
                idx.push(i);
            }
        }
        g.kids.push(layer_el("LayerElementUV", "map1", "ByPolygonVertex", "IndexToDirect").kid(n1("UV", Prop::AF64(list))).kid(n1("UVIndex", Prop::AI32(idx))));
        layer.kids.push(le("LayerElementUV"));
    }
    let per_face: Vec<i32> = pm.f.iter().enumerate().map(|(fi, _)| pm.fm.as_ref().and_then(|fm| fm.get(fi).cloned().flatten()).and_then(|m| slots.iter().position(|x| *x == m)).unwrap_or(0) as i32).collect();
    let all_same = per_face.iter().all(|&x| x == 0);
    g.kids.push(layer_el("LayerElementMaterial", "", if all_same { "AllSame" } else { "ByPolygon" }, "IndexToDirect").kid(n1("Materials", Prop::AI32(if all_same { vec![0] } else { per_face }))));
    layer.kids.push(le("LayerElementMaterial"));
    g.kids.push(layer);
    g
}

/// Write the scene (or the selection and everything under it) as binary FBX 7.4.
pub fn export_fbx(s: &Scene, sel_only: bool) -> Vec<u8> {
    let keep: Option<HashSet<Id>> = sel_only.then(|| s.sel.iter().flat_map(|i| s.subtree(i)).collect());
    let ids: Vec<Id> = s.all_dag(false).into_iter().filter(|i| keep.as_ref().is_none_or(|k| k.contains(i))).filter(|i| s.node(i).is_some_and(|n| !n.startup())).collect();
    let mut next = 1_000_000_000i64;
    let mut new_id = || {
        next += 1;
        next
    };
    let model_id: HashMap<Id, i64> = ids.iter().map(|i| (i.clone(), new_id())).collect();
    let mut objects: Vec<FNode> = vec![];
    let mut conns: Vec<FNode> = vec![];
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let add = |objects: &mut Vec<FNode>, counts: &mut BTreeMap<&'static str, usize>, class: &'static str, n: FNode| {
        *counts.entry(class).or_default() += 1;
        objects.push(n);
    };
    // ---- materials and textures
    let mut mat_id: HashMap<Id, i64> = HashMap::new();
    let used: HashSet<Id> = ids
        .iter()
        .filter_map(|i| s.node(i).and_then(|n| n.mesh()))
        .flat_map(|m| std::iter::once(m.material.clone()).chain(m.mesh.fm.iter().flatten().flatten().cloned()))
        .collect();
    for (mid, m) in s.mats.iter().filter(|(k, _)| used.contains(*k)) {
        let oid = new_id();
        mat_id.insert(mid.clone(), oid);
        let a = |k: &str, d: [f64; 3]| -> DVec3 { m.attrs.get(k).and_then(|v| serde_json::from_value::<[f64; 3]>(v.clone()).ok()).map(DVec3::from_array).unwrap_or(DVec3::from_array(d)) };
        let f = |k: &str, d: f64| m.attrs.get(k).and_then(Value::as_f64).unwrap_or(d);
        let phong = matches!(m.ty.as_str(), "phong" | "blinn" | "phongE");
        let color = if m.ty == "standardSurface" { a("baseColor", [0.8; 3]) } else { a("color", [0.5; 3]) };
        let tr = a("transparency", [0.0; 3]);
        let mut p = vec![
            pp("ShadingModel", "KString", "", "", vec![st(if phong { "phong" } else { "lambert" })]),
            pd3("DiffuseColor", "Color", "A", color),
            pp("DiffuseFactor", "Number", "", "A", vec![Prop::F64(f("diffuse", 0.8))]),
            pd3("TransparentColor", "Color", "A", tr),
            pp("TransparencyFactor", "Number", "", "A", vec![Prop::F64(if tr.max_element() > 0.0 { 1.0 } else { 0.0 })]),
            pd3("EmissiveColor", "Color", "A", a("incandescence", [0.0; 3])),
            pp("Opacity", "double", "Number", "", vec![Prop::F64(1.0 - (tr.x + tr.y + tr.z) / 3.0)]),
        ];
        if phong {
            p.push(pd3("SpecularColor", "Color", "A", a("specularColor", [0.5; 3])));
            p.push(pp("ShininessExponent", "Number", "", "A", vec![Prop::F64(f("cosinePower", 20.0))]));
        }
        let node = FNode::new("Material").p(Prop::I64(oid)).p(oname(&m.name, "Material")).p(st("")).kid(n1("Version", Prop::I32(102))).kid(n1("ShadingModel", st(if phong { "phong" } else { "lambert" }))).kid(n1("MultiLayer", Prop::I32(0))).kid(props70(p));
        add(&mut objects, &mut counts, "Material", node);
        let slot = if m.ty == "standardSurface" { "baseColor" } else { "color" };
        if let Some(t) = m.maps.get(slot).and_then(Value::as_str).and_then(|t| s.texs.get(t)).filter(|t| t.ty == "file") {
            let file = t.attrs.get("fileTextureName").and_then(Value::as_str).unwrap_or("").to_string();
            let rel = Path::new(&file).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            let (tid, vid) = (new_id(), new_id());
            let video = FNode::new("Video").p(Prop::I64(vid)).p(oname(&t.name, "Video")).p(st("Clip")).kid(n1("Type", st("Clip"))).kid(props70(vec![pp("Path", "KString", "XRefUrl", "", vec![st(&file)])])).kid(n1("UseMipMap", Prop::I32(0))).kid(n1("Filename", st(&file))).kid(n1("RelativeFilename", st(&rel)));
            add(&mut objects, &mut counts, "Video", video);
            let tex = FNode::new("Texture")
                .p(Prop::I64(tid))
                .p(oname(&t.name, "Texture"))
                .p(st(""))
                .kid(n1("Type", st("TextureVideoClip")))
                .kid(n1("Version", Prop::I32(202)))
                .kid(n1("TextureName", oname(&t.name, "Texture")))
                .kid(props70(vec![pp("UseMaterial", "bool", "", "", vec![Prop::I32(1)])]))
                .kid(n1("Media", oname(&t.name, "Video")))
                .kid(n1("FileName", st(&file)))
                .kid(n1("RelativeFilename", st(&rel)))
                .kid(FNode::new("ModelUVTranslation").p(Prop::F64(0.0)).p(Prop::F64(0.0)))
                .kid(FNode::new("ModelUVScaling").p(Prop::F64(1.0)).p(Prop::F64(1.0)))
                .kid(n1("Texture_Alpha_Source", st("None")))
                .kid(FNode::new("Cropping").p(Prop::I32(0)).p(Prop::I32(0)).p(Prop::I32(0)).p(Prop::I32(0)));
            add(&mut objects, &mut counts, "Texture", tex);
            conns.push(conn("OO", vid, tid, None));
            conns.push(conn("OP", tid, oid, Some("DiffuseColor")));
        }
    }
    // ---- models and their attributes / geometry
    let mut pose_nodes: Vec<FNode> = vec![];
    let mut posed: HashSet<i64> = HashSet::new();
    for id in &ids {
        let nd = s.node(id).unwrap();
        let mid = model_id[id];
        let kind = match &nd.shape {
            Shape::Camera(_) => "camera",
            Shape::Light(_) => "light",
            _ => "",
        };
        let x = &nd.xf;
        // camera / light orientation into FBX's convention: bake into a plain matrix
        let fixed = if kind.is_empty() { None } else { Some(x.matrix() * shape_fix(kind).inverse()) };
        let (t, r, sc, jo, p) = match fixed {
            Some(m) => {
                let (sc, q, t) = m.to_scale_rotation_translation();
                (t, DVec3::from_array(euler_deg_from_quat(q, "xyz")), sc, DVec3::ZERO, DVec3::ZERO)
            }
            None => (DVec3::from_array(x.t) + DVec3::from_array(x.pt), DVec3::from_array(x.r), DVec3::from_array(x.s), DVec3::from_array(x.jo), DVec3::from_array(x.p)),
        };
        let ro = if fixed.is_some() { 0 } else { ROTATE_ORDERS.iter().position(|o| *o == x.ro).unwrap_or(0) };
        let mut p70 = vec![
            pp("RotationOrder", "enum", "", "", vec![Prop::I32(ro as i32)]),
            pp("InheritType", "enum", "", "", vec![Prop::I32(1)]),
            pp("DefaultAttributeIndex", "int", "Integer", "", vec![Prop::I32(0)]),
            pd3("Lcl Translation", "Lcl Translation", "A", t),
            pd3("Lcl Rotation", "Lcl Rotation", "A", r),
            pd3("Lcl Scaling", "Lcl Scaling", "A", sc),
            pp("Visibility", "Visibility", "", "A", vec![Prop::F64(if nd.visible { 1.0 } else { 0.0 })]),
        ];
        if jo != DVec3::ZERO {
            p70.push(pp("RotationActive", "bool", "", "", vec![Prop::I32(1)]));
            p70.push(pd3("PreRotation", "Vector3D", "Vector", jo));
        }
        if p != DVec3::ZERO {
            p70.push(pd3("RotationPivot", "Vector3D", "Vector", p));
            p70.push(pd3("ScalingPivot", "Vector3D", "Vector", p));
        }
        let ty = match (&nd.shape, nd.kind.as_str()) {
            (Shape::Mesh(_), _) => "Mesh",
            (_, "joint") => "LimbNode",
            (Shape::Camera(_), _) => "Camera",
            (Shape::Light(_), _) => "Light",
            _ => "Null",
        };
        let model = FNode::new("Model").p(Prop::I64(mid)).p(oname(&nd.name, "Model")).p(st(ty)).kid(n1("Version", Prop::I32(232))).kid(props70(p70)).kid(n1("Shading", Prop::Bool(true))).kid(n1("Culling", st("CullingOff")));
        add(&mut objects, &mut counts, "Model", model);
        let parent = nd.parent.as_ref().and_then(|p| model_id.get(p)).copied().unwrap_or(0);
        conns.push(conn("OO", mid, parent, None));
        let attr = |name: &str, sub: &str, flags: &str, p: Vec<FNode>, aid: i64| -> FNode {
            FNode::new("NodeAttribute").p(Prop::I64(aid)).p(oname(name, "NodeAttribute")).p(st(sub)).kid(props70(p)).kid(n1("TypeFlags", st(flags)))
        };
        match &nd.shape {
            Shape::Mesh(m) => {
                let skin = m.history.iter().find(|h| h.ty == "skinCluster");
                let pm = if skin.is_some() { crate::skin::bind_mesh(m) } else { &m.mesh };
                let mut slots = vec![m.material.clone()];
                for f in pm.fm.iter().flatten().flatten() {
                    if !slots.contains(f) {
                        slots.push(f.clone());
                    }
                }
                let gid = new_id();
                add(&mut objects, &mut counts, "Geometry", geometry_node(gid, pm, &slots));
                conns.push(conn("OO", gid, mid, None));
                for sl in &slots {
                    if let Some(&m) = mat_id.get(sl) {
                        conns.push(conn("OO", m, mid, None));
                    }
                }
                // ---- skin cluster
                if let Some(h) = skin {
                    let joints: Vec<String> = h.params.get("joints").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                    let bind_inv: Vec<DMat4> = h.params.get("bindInv").and_then(Value::as_array).map(|a| a.iter().filter_map(|m| serde_json::from_value::<Vec<f64>>(m.clone()).ok().and_then(|v| mat16(&v))).collect()).unwrap_or_default();
                    let mesh_bind = h.params.get("meshBind").and_then(|m| serde_json::from_value::<Vec<f64>>(m.clone()).ok()).and_then(|v| mat16(&v)).unwrap_or_else(|| s.world_matrix(id));
                    let weights = h.params.get("weights").and_then(Value::as_array).cloned().unwrap_or_default();
                    if bind_inv.len() == joints.len() && joints.iter().all(|j| model_id.contains_key(j)) {
                        let skid = new_id();
                        add(&mut objects, &mut counts, "Deformer", FNode::new("Deformer").p(Prop::I64(skid)).p(oname(&format!("{}_skinCluster", nd.name), "Deformer")).p(st("Skin")).kid(n1("Version", Prop::I32(101))).kid(n1("Link_DeformAcuracy", Prop::F64(50.0))));
                        conns.push(conn("OO", skid, gid, None));
                        for (j, jid) in joints.iter().enumerate() {
                            let (mut idx, mut wt) = (vec![], vec![]);
                            for (vi, w) in weights.iter().enumerate() {
                                for e in w.as_array().into_iter().flatten() {
                                    if e.get(0).and_then(Value::as_u64) == Some(j as u64) {
                                        idx.push(vi as i32);
                                        wt.push(e.get(1).and_then(Value::as_f64).unwrap_or(0.0));
                                    }
                                }
                            }
                            let tl = bind_inv[j].inverse();
                            let cid = new_id();
                            let cl = FNode::new("Deformer")
                                .p(Prop::I64(cid))
                                .p(oname(&format!("Cluster {}", s.name_of(jid)), "SubDeformer"))
                                .p(st("Cluster"))
                                .kid(n1("Version", Prop::I32(100)))
                                .kid(FNode::new("UserData").p(st("")).p(st("")))
                                .kid(n1("Indexes", Prop::AI32(idx)))
                                .kid(n1("Weights", Prop::AF64(wt)))
                                .kid(n1("Transform", Prop::AF64((bind_inv[j] * mesh_bind).to_cols_array().to_vec())))
                                .kid(n1("TransformLink", Prop::AF64(tl.to_cols_array().to_vec())));
                            add(&mut objects, &mut counts, "Deformer", cl);
                            conns.push(conn("OO", cid, skid, None));
                            conns.push(conn("OO", model_id[jid], cid, None));
                            if posed.insert(model_id[jid]) {
                                pose_nodes.push(FNode::new("PoseNode").kid(n1("Node", Prop::I64(model_id[jid]))).kid(n1("Matrix", Prop::AF64(tl.to_cols_array().to_vec()))));
                            }
                        }
                        if posed.insert(mid) {
                            pose_nodes.push(FNode::new("PoseNode").kid(n1("Node", Prop::I64(mid))).kid(n1("Matrix", Prop::AF64(mesh_bind.to_cols_array().to_vec()))));
                        }
                    }
                }
            }
            Shape::Camera(c) => {
                let aid = new_id();
                let p = vec![
                    pp("FocalLength", "Number", "", "A", vec![Prop::F64(c.focal_length)]),
                    pp("FilmWidth", "double", "Number", "", vec![Prop::F64(c.horizontal_film_aperture)]),
                    pp("FilmHeight", "double", "Number", "", vec![Prop::F64(c.vertical_film_aperture)]),
                    pp("NearPlane", "double", "Number", "", vec![Prop::F64(c.near)]),
                    pp("FarPlane", "double", "Number", "", vec![Prop::F64(c.far)]),
                    pp("CameraProjectionType", "enum", "", "", vec![Prop::I32(c.ortho as i32)]),
                    pp("OrthoZoom", "double", "Number", "", vec![Prop::F64(c.ortho_width / 30.0)]),
                ];
                add(&mut objects, &mut counts, "NodeAttribute", attr(&nd.name, "Camera", "Camera", p, aid).kid(n1("GeometryVersion", Prop::I32(124))));
                conns.push(conn("OO", aid, mid, None));
            }
            Shape::Light(l) => {
                let aid = new_id();
                let lt = match l.ty.as_str() {
                    "directionalLight" => 1,
                    "spotLight" => 2,
                    "areaLight" => 3,
                    _ => 0,
                };
                let c = l.attrs.get("color").and_then(|v| serde_json::from_value::<[f64; 3]>(v.clone()).ok()).unwrap_or([1.0; 3]);
                let mut p = vec![
                    pp("LightType", "enum", "", "", vec![Prop::I32(lt)]),
                    pd3("Color", "Color", "A", DVec3::from_array(c)),
                    pp("Intensity", "Number", "", "A", vec![Prop::F64(l.attrs.get("intensity").and_then(Value::as_f64).unwrap_or(1.0) * 100.0)]),
                    pp("CastShadows", "bool", "", "", vec![Prop::I32(l.attrs.get("shadows").and_then(Value::as_bool).unwrap_or(false) as i32)]),
                ];
                if let Some(a) = l.attrs.get("coneAngle").and_then(Value::as_f64) {
                    p.push(pp("OuterAngle", "Number", "", "A", vec![Prop::F64(a)]));
                    p.push(pp("InnerAngle", "Number", "", "A", vec![Prop::F64((a - l.attrs.get("penumbraAngle").and_then(Value::as_f64).unwrap_or(0.0).abs()).max(0.0))]));
                }
                add(&mut objects, &mut counts, "NodeAttribute", attr(&nd.name, "Light", "Light", p, aid).kid(n1("GeometryVersion", Prop::I32(124))));
                conns.push(conn("OO", aid, mid, None));
            }
            _ if nd.kind == "joint" => {
                let aid = new_id();
                let size = nd.passthrough.get("rig").and_then(|r| r.get("radius")).and_then(Value::as_f64).unwrap_or(1.0);
                add(&mut objects, &mut counts, "NodeAttribute", attr(&nd.name, "LimbNode", "Skeleton", vec![pp("Size", "double", "Number", "", vec![Prop::F64(size * 100.0)])], aid));
                conns.push(conn("OO", aid, mid, None));
            }
            _ => {
                let aid = new_id();
                add(&mut objects, &mut counts, "NodeAttribute", attr(&nd.name, "Null", "Null", vec![], aid));
                conns.push(conn("OO", aid, mid, None));
            }
        }
    }
    if !pose_nodes.is_empty() {
        let n = pose_nodes.len();
        let mut pose = FNode::new("Pose").p(Prop::I64(new_id())).p(oname("BindPose", "Pose")).p(st("BindPose")).kid(n1("Type", st("BindPose"))).kid(n1("Version", Prop::I32(100))).kid(n1("NbPoseNodes", Prop::I32(n as i32)));
        pose.kids.extend(pose_nodes);
        add(&mut objects, &mut counts, "Pose", pose);
    }
    // ---- animation
    let fps = s.time.fps.max(1.0);
    let kt = |f: f64| (f / fps * KTIME).round() as i64;
    let (a0, a1) = (kt(s.time.start), kt(s.time.end));
    let stack = new_id();
    let layer = new_id();
    let stack_node = FNode::new("AnimationStack").p(Prop::I64(stack)).p(oname("Take 001", "AnimStack")).p(st("")).kid(props70(vec![
        pp("LocalStart", "KTime", "Time", "", vec![Prop::I64(a0)]),
        pp("LocalStop", "KTime", "Time", "", vec![Prop::I64(a1)]),
        pp("ReferenceStart", "KTime", "Time", "", vec![Prop::I64(a0)]),
        pp("ReferenceStop", "KTime", "Time", "", vec![Prop::I64(a1)]),
    ]));
    add(&mut objects, &mut counts, "AnimationStack", stack_node);
    add(&mut objects, &mut counts, "AnimationLayer", FNode::new("AnimationLayer").p(Prop::I64(layer)).p(oname("BaseLayer", "AnimLayer")).p(st("")));
    conns.push(conn("OO", layer, stack, None));
    let mut groups: BTreeMap<(Id, &str), Vec<&crate::anim::AnimCurve>> = BTreeMap::new();
    for c in s.anim.curves.values().filter(|c| model_id.contains_key(&c.node) && !c.keys.is_empty()) {
        let g = match c.attr.as_str() {
            "translateX" | "translateY" | "translateZ" => "T",
            "rotateX" | "rotateY" | "rotateZ" => "R",
            "scaleX" | "scaleY" | "scaleZ" => "S",
            "visibility" => "Visibility",
            _ => continue,
        };
        groups.entry((c.node.clone(), g)).or_default().push(c);
    }
    for ((node, g), curves) in groups {
        let xf = &s.node(&node).unwrap().xf;
        let (prop, def) = match g {
            "T" => ("Lcl Translation", DVec3::from_array(xf.t)),
            "R" => ("Lcl Rotation", DVec3::from_array(xf.r)),
            "S" => ("Lcl Scaling", DVec3::from_array(xf.s)),
            _ => ("Visibility", DVec3::ONE),
        };
        let cn = new_id();
        let p = if g == "Visibility" { vec![pp("d|Visibility", "Visibility", "", "A", vec![Prop::F64(1.0)])] } else { ["X", "Y", "Z"].iter().enumerate().map(|(i, a)| pp(&format!("d|{a}"), "Number", "", "A", vec![Prop::F64(def[i])])).collect() };
        add(&mut objects, &mut counts, "AnimationCurveNode", FNode::new("AnimationCurveNode").p(Prop::I64(cn)).p(oname(g, "AnimCurveNode")).p(st("")).kid(props70(p)));
        conns.push(conn("OO", cn, layer, None));
        conns.push(conn("OP", cn, model_id[&node], Some(prop)));
        for c in curves {
            let ch = if g == "Visibility" { "d|Visibility".to_string() } else { format!("d|{}", &c.attr[c.attr.len() - 1..]) };
            let cid = new_id();
            let flags: Vec<i32> = c
                .keys
                .iter()
                .map(|k| match k.ot.as_str() {
                    "step" => 0x2,
                    "linear" => 0x4,
                    _ => 0x8 | 0x100,
                })
                .collect();
            let n = c.keys.len();
            let curve = FNode::new("AnimationCurve")
                .p(Prop::I64(cid))
                .p(oname("", "AnimCurve"))
                .p(st(""))
                .kid(n1("Default", Prop::F64(c.keys[0].v)))
                .kid(n1("KeyVer", Prop::I32(4009)))
                .kid(n1("KeyTime", Prop::AI64(c.keys.iter().map(|k| kt(k.t)).collect())))
                .kid(n1("KeyValueFloat", Prop::AF32(c.keys.iter().map(|k| k.v as f32).collect())))
                .kid(n1("KeyAttrFlags", Prop::AI32(flags)))
                .kid(n1("KeyAttrDataFloat", Prop::AF32((0..n).flat_map(|_| [0.0, 0.0, f32::from_bits(218434821), 0.0]).collect())))
                .kid(n1("KeyAttrRefCount", Prop::AI32(vec![1; n])));
            add(&mut objects, &mut counts, "AnimationCurve", curve);
            conns.push(conn("OP", cid, cn, Some(&ch)));
        }
    }

    // ---- file
    let header = FNode::new("FBXHeaderExtension")
        .kid(n1("FBXHeaderVersion", Prop::I32(1003)))
        .kid(n1("FBXVersion", Prop::I32(7400)))
        .kid(n1("EncryptionType", Prop::I32(0)))
        .kid(FNode::new("CreationTimeStamp").kid(n1("Version", Prop::I32(1000))).kid(n1("Year", Prop::I32(1970))).kid(n1("Month", Prop::I32(1))).kid(n1("Day", Prop::I32(1))).kid(n1("Hour", Prop::I32(10))).kid(n1("Minute", Prop::I32(0))).kid(n1("Second", Prop::I32(0))).kid(n1("Millisecond", Prop::I32(0))))
        .kid(n1("Creator", st(&format!("Inca {}", env!("CARGO_PKG_VERSION")))));
    let gs = FNode::new("GlobalSettings").kid(n1("Version", Prop::I32(1000))).kid(props70(vec![
        pp("UpAxis", "int", "Integer", "", vec![Prop::I32(1)]),
        pp("UpAxisSign", "int", "Integer", "", vec![Prop::I32(1)]),
        pp("FrontAxis", "int", "Integer", "", vec![Prop::I32(2)]),
        pp("FrontAxisSign", "int", "Integer", "", vec![Prop::I32(1)]),
        pp("CoordAxis", "int", "Integer", "", vec![Prop::I32(0)]),
        pp("CoordAxisSign", "int", "Integer", "", vec![Prop::I32(1)]),
        pp("OriginalUpAxis", "int", "Integer", "", vec![Prop::I32(1)]),
        pp("OriginalUpAxisSign", "int", "Integer", "", vec![Prop::I32(1)]),
        pp("UnitScaleFactor", "double", "Number", "", vec![Prop::F64(1.0)]),
        pp("OriginalUnitScaleFactor", "double", "Number", "", vec![Prop::F64(1.0)]),
        pp("TimeMode", "enum", "", "", vec![Prop::I32(mode_of_fps(fps) as i32)]),
        pp("TimeSpanStart", "KTime", "Time", "", vec![Prop::I64(a0)]),
        pp("TimeSpanStop", "KTime", "Time", "", vec![Prop::I64(a1)]),
        pp("CustomFrameRate", "double", "Number", "", vec![Prop::F64(fps)]),
    ]));
    let doc_id = new_id();
    let documents = FNode::new("Documents").kid(n1("Count", Prop::I32(1))).kid(
        FNode::new("Document")
            .p(Prop::I64(doc_id))
            .p(st(""))
            .p(st("Scene"))
            .kid(props70(vec![pp("SourceObject", "object", "", "", vec![]), pp("ActiveAnimStackName", "KString", "", "", vec![st("Take 001")])]))
            .kid(n1("RootNode", Prop::I64(0))),
    );
    let total: usize = counts.values().sum::<usize>() + 1;
    let mut defs = FNode::new("Definitions").kid(n1("Version", Prop::I32(100))).kid(n1("Count", Prop::I32(total as i32))).kid(FNode::new("ObjectType").p(st("GlobalSettings")).kid(n1("Count", Prop::I32(1))));
    for (class, n) in &counts {
        defs.kids.push(FNode::new("ObjectType").p(st(class)).kid(n1("Count", Prop::I32(*n as i32))));
    }
    let takes = FNode::new("Takes").kid(n1("Current", st("Take 001"))).kid(
        FNode::new("Take").p(st("Take 001")).kid(n1("FileName", st("Take_001.tak"))).kid(FNode::new("LocalTime").p(Prop::I64(a0)).p(Prop::I64(a1))).kid(FNode::new("ReferenceTime").p(Prop::I64(a0)).p(Prop::I64(a1))),
    );
    let nodes = vec![
        header,
        n1("FileId", Prop::Raw(b"\x28\xb3\x2a\xeb\xb6\x24\xcc\xc2\xbf\xc8\xb0\x2a\xa9\x2b\xfc\xf1".to_vec())),
        n1("CreationTime", st("1970-01-01 10:00:00:000")),
        n1("Creator", st(&format!("Inca {}", env!("CARGO_PKG_VERSION")))),
        gs,
        documents,
        FNode::new("References"),
        defs,
        FNode { name: "Objects".into(), props: vec![], kids: objects },
        FNode { name: "Connections".into(), props: vec![], kids: conns },
        takes,
    ];
    write_binary(&nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &mut Scene, mel: &str) {
        crate::mel::Interp::new().run(s, &mut crate::commands::NoHost, mel).unwrap();
    }

    #[test]
    fn binary_round_trip_keeps_meshes_hierarchy_and_keys() {
        let mut s = Scene::new();
        run(&mut s, "polyCube -n box; move 1 2 3 box; rotate 10 20 30 box; polySphere -n ball; parent ball box; setKeyframe -t 1 -at tx box; move 5 2 3 box; setKeyframe -t 12 -at tx box;");
        let bytes = export_fbx(&s, false);
        assert!(bytes.starts_with(MAGIC));
        let mut t = Scene::new();
        import_fbx(&mut t, &bytes, None, "x").unwrap();
        let (a, b) = (s.find("box").unwrap(), t.find("box").unwrap());
        let (ma, mb) = (&s.node(&a).unwrap().mesh().unwrap().mesh, &t.node(&b).unwrap().mesh().unwrap().mesh);
        assert_eq!(ma.f, mb.f);
        assert!(ma.v.iter().zip(&mb.v).all(|(p, q)| p.distance(*q) < 1e-9));
        assert!(mb.uv.is_some());
        let ball = t.find("ball").unwrap();
        assert_eq!(t.node(&ball).unwrap().parent.as_deref(), Some(b.as_str()));
        let r = t.node(&b).unwrap().xf.r;
        assert!((r[0] - 10.0).abs() < 1e-6 && (r[1] - 20.0).abs() < 1e-6 && (r[2] - 30.0).abs() < 1e-6, "{r:?}");
        let c = t.anim.curves.values().find(|c| c.node == b && c.attr == "translateX").expect("tx curve");
        assert_eq!(c.keys.iter().map(|k| (k.t, k.v)).collect::<Vec<_>>(), vec![(1.0, 1.0), (12.0, 5.0)]);
    }

    #[test]
    fn skeleton_and_skin_weights_round_trip() {
        let mut s = Scene::new();
        run(&mut s, "polyCylinder -h 4 -sy 4 -n arm; select -cl; joint -p 0 -2 0 -n j1; joint -p 0 2 0 -n j2; joint -e -oj xyz j1; select -r j1 arm; skinCluster -tsb -mi 2;");
        let jo = s.node(&s.find("j1").unwrap()).unwrap().xf.jo;
        let bytes = export_fbx(&s, false);
        let mut t = Scene::new();
        import_fbx(&mut t, &bytes, None, "x").unwrap();
        let j1 = t.find("j1").unwrap();
        assert_eq!(t.node(&j1).unwrap().kind, "joint");
        let jo2 = t.node(&j1).unwrap().xf.jo;
        assert!(jo.iter().zip(jo2).all(|(a, b)| (a - b).abs() < 1e-6), "{jo:?} {jo2:?}");
        let arm = t.find("arm").unwrap();
        let w = |sc: &Scene, id: &str| sc.node(id).unwrap().mesh().unwrap().history.iter().find(|h| h.ty == "skinCluster").expect("skin").params["weights"].clone();
        let (wa, wb) = (w(&s, &s.find("arm").unwrap()), w(&t, &arm));
        let wa = wa.as_array().unwrap();
        let wb = wb.as_array().unwrap();
        assert_eq!(wa.len(), wb.len());
        for (x, y) in wa.iter().zip(wb) {
            let f = |v: &Value| -> BTreeMap<u64, f64> { v.as_array().unwrap().iter().map(|e| (e[0].as_u64().unwrap(), e[1].as_f64().unwrap())).collect() };
            let (fx, fy) = (f(x), f(y));
            assert_eq!(fx.len(), fy.len());
            for (k, v) in fx {
                assert!((fy[&k] - v).abs() < 1e-3);
            }
        }
        // the skinned mesh is undeformed at the bind pose
        let (pa, pb) = (s.node(&s.find("arm").unwrap()).unwrap().mesh().unwrap().mesh.clone(), t.node(&arm).unwrap().mesh().unwrap().mesh.clone());
        assert!(pa.v.iter().zip(&pb.v).all(|(p, q)| p.distance(*q) < 1e-6));
        // and follows the joints
        t.nodes.get_mut(&j1).unwrap().xf.r[2] = 45.0;
        t.refresh_dependents(&j1);
        t.evaluate(&arm, 0);
        let moved = &t.node(&arm).unwrap().mesh().unwrap().mesh;
        assert!(moved.v.iter().zip(&pb.v).any(|(p, q)| p.distance(*q) > 0.1));
    }

    #[test]
    fn reads_ascii_fbx() {
        let src = r#"; FBX 7.4.0 project file
FBXHeaderExtension:  {
	FBXHeaderVersion: 1003
	FBXVersion: 7400
}
GlobalSettings:  {
	Version: 1000
	Properties70:  {
		P: "UpAxis", "int", "Integer", "",1
		P: "UnitScaleFactor", "double", "Number", "",1
	}
}
Objects:  {
	Geometry: 100, "Geometry::", "Mesh" {
		Vertices: *12 {
			a: -1,0,1,1,0,1,1,0,-1,-1,0,-1
		}
		PolygonVertexIndex: *4 {
			a: 0,1,2,-4
		}
		LayerElementUV: 0 {
			MappingInformationType: "ByPolygonVertex"
			ReferenceInformationType: "IndexToDirect"
			UV: *8 {
				a: 0,0,1,0,1,1,0,1
			}
			UVIndex: *4 {
				a: 0,1,2,3
			}
		}
	}
	Model: 200, "Model::plane", "Mesh" {
		Version: 232
		Properties70:  {
			P: "Lcl Translation", "Lcl Translation", "", "A",0,5,0
		}
		Shading: T
	}
	Material: 300, "Material::red", "" {
		ShadingModel: "lambert"
		Properties70:  {
			P: "DiffuseColor", "Color", "", "A",1,0,0
		}
	}
}
Connections:  {
	C: "OO",200,0
	C: "OO",100,200
	C: "OO",300,200
}
"#;
        let mut s = Scene::new();
        let made = import_fbx(&mut s, src.as_bytes(), None, "x").unwrap();
        assert_eq!(made.len(), 1);
        let n = s.node(&made[0]).unwrap();
        assert_eq!(n.name, "plane");
        assert_eq!(n.xf.t, [0.0, 5.0, 0.0]);
        let m = n.mesh().unwrap();
        assert_eq!(m.mesh.f, vec![vec![0, 1, 2, 3]]);
        assert_eq!(m.mesh.uv.as_ref().unwrap()[0].as_ref().unwrap()[2], DVec2::new(1.0, 1.0));
        assert_eq!(s.mats[&m.material].name, "red");
        assert_eq!(s.mats[&m.material].attrs["color"], json!([1.0, 0.0, 0.0]));
    }

    #[test]
    fn z_up_files_are_stood_up() {
        let mut s = Scene::new();
        run(&mut s, "spaceLocator -n loc; move 0 0 5 loc;");
        let (_, mut nodes) = parse(&export_fbx(&s, false)).unwrap();
        let gs = nodes.iter_mut().find(|n| n.name == "GlobalSettings").unwrap();
        let p70 = gs.kids.iter_mut().find(|k| k.name == "Properties70").unwrap();
        for p in p70.kids.iter_mut() {
            match p.props[0].as_str() {
                Some("UpAxis") => p.props[4] = Prop::I32(2),
                Some("FrontAxis") => p.props[4] = Prop::I32(1),
                Some("FrontAxisSign") => p.props[4] = Prop::I32(-1),
                _ => {}
            }
        }
        let mut t = Scene::new();
        let made = import_fbx(&mut t, &write_binary(&nodes), None, "zfile").unwrap();
        assert_eq!(t.name_of(&made[0]), "zfile_yUp");
        let loc = t.find("loc").unwrap();
        let p = t.world_matrix(&loc).transform_point3(DVec3::ZERO);
        assert!(p.distance(DVec3::new(0.0, 5.0, 0.0)) < 1e-9, "{p}");
    }

    /// Walk the file the way strict readers (the FBX SDK, Blender) do: every node either ends right
    /// after its properties or holds children followed by exactly one null record, ending at its offset.
    fn strict_walk(b: &[u8], mut p: usize, end: usize) -> usize {
        let mut n = 0;
        while p < end {
            let u = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as usize;
            let (e, pl, nl) = (u(p), u(p + 8), b[p + 12] as usize);
            if e == 0 {
                assert_eq!(p + 13, end, "a null record must close the block");
                return n;
            }
            let props_end = p + 13 + nl + pl;
            assert!(e >= props_end && e <= end, "bad end offset");
            if e > props_end {
                assert_eq!(&b[e - 13..e], &NULL_RECORD, "children must end in a null record");
                n += strict_walk(b, props_end, e);
            }
            n += 1;
            p = e;
        }
        n
    }

    #[test]
    fn binary_layout_is_strict() {
        let mut s = Scene::new();
        run(&mut s, "polyCylinder -h 4 -sy 4 -n arm; select -cl; joint -p 0 -2 0 -n j1; joint -p 0 2 0 -n j2; select -r j1 arm; skinCluster -tsb -mi 2; setKeyframe -t 1 -at rz j1; rotate 0 0 40 j1; setKeyframe -t 20 -at rz j1; camera -n cam1; pointLight;");
        let b = export_fbx(&s, false);
        // top-level nodes and their closing null record, then the footer
        let foot = (0..b.len() - 16).rev().find(|&i| b[i..].starts_with(b"\xfa\xbc\xab\x09\xd0")).unwrap();
        assert!(strict_walk(&b, 27, foot) > 50);
        assert_eq!((b.len() - 16 - 120 - 4) % 16, 0, "footer version is 16-byte aligned");
        let (_, nodes) = parse(&b).unwrap();
        let objs = nodes.iter().find(|n| n.name == "Objects").unwrap();
        let count = |c: &str, st: &str| objs.kids.iter().filter(|o| o.name == c && subtype(o) == st).count();
        assert_eq!((count("Deformer", "Skin"), count("Deformer", "Cluster"), count("Pose", "BindPose")), (1, 2, 1));
        assert_eq!(count("Model", "LimbNode"), 2);
        assert_eq!(count("NodeAttribute", "Camera") + count("NodeAttribute", "Light"), 2);
        assert!(count("AnimationCurve", "") >= 1);
    }
}
