//! Maya ASCII (`.ma`) import and export.
//!
//! A `.ma` file is a MEL script of `createNode` / `setAttr` / `connectAttr` / `parent` statements.
//! Import rebuilds transforms and hierarchy, meshes (stored vertex / edge / face data with UVs and
//! tweaks, or primitive creation nodes as live construction history), cameras, lights, joints,
//! materials with their shading-group assignments and file textures, skinClusters with their
//! weights, and keyframe animation. Export writes the same node types.

use crate::math::{DMat4, DVec2, DVec3};
use crate::mesh::PolyMesh;
use crate::params::Params;
use crate::scene::{CamShape, Id, Scene, Shape};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

// ---------------------------------------------------------------- reading

/// One `setAttr`: attribute path (e.g. `.vt[0:7]`), `-type` and the value tokens.
#[derive(Clone, Debug)]
struct SetAttr {
    attr: String,
    ty: Option<String>,
    vals: Vec<String>,
}

#[derive(Clone, Debug, Default)]
struct MaNode {
    ty: String,
    name: String,
    parent: Option<String>,
    attrs: Vec<SetAttr>,
}

impl MaNode {
    /// Last value set for an attribute (any of the given names, short or long).
    fn get(&self, names: &[&str]) -> Option<&SetAttr> {
        self.attrs.iter().rev().find(|a| {
            let n = a.attr.trim_start_matches('.');
            names.contains(&n)
        })
    }
    fn f(&self, names: &[&str]) -> Option<f64> {
        self.get(names)?.vals.first()?.parse().ok()
    }
    fn v3(&self, names: &[&str]) -> Option<[f64; 3]> {
        let a = self.get(names)?;
        let v: Vec<f64> = a.vals.iter().filter_map(|x| x.parse().ok()).collect();
        (v.len() >= 3).then(|| [v[0], v[1], v[2]])
    }
    fn s(&self, names: &[&str]) -> Option<String> {
        self.get(names)?.vals.first().cloned()
    }
}

/// Split the file into statements (on `;` outside quotes) and each into tokens (quotes removed).
fn statements(src: &str) -> Vec<Vec<String>> {
    let mut out = vec![];
    let mut cur: Vec<String> = vec![];
    let mut tok = String::new();
    let mut in_q = false;
    let mut had_q = false;
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        if in_q {
            match c {
                '\\' => {
                    if let Some(n) = chars.next() {
                        tok.push(match n {
                            'n' => '\n',
                            't' => '\t',
                            o => o,
                        });
                    }
                }
                '"' => in_q = false,
                _ => tok.push(c),
            }
            continue;
        }
        match c {
            '"' => {
                in_q = true;
                had_q = true;
            }
            '/' if chars.peek() == Some(&'/') && tok.is_empty() => {
                // comment to end of line
                for n in chars.by_ref() {
                    if n == '\n' {
                        break;
                    }
                }
            }
            ';' => {
                if !tok.is_empty() || had_q {
                    cur.push(std::mem::take(&mut tok));
                }
                had_q = false;
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c if c.is_whitespace() => {
                if !tok.is_empty() || had_q {
                    cur.push(std::mem::take(&mut tok));
                }
                had_q = false;
            }
            _ => tok.push(c),
        }
    }
    out
}

fn is_flag(t: &str) -> bool {
    t.starts_with('-') && t.len() > 1 && !t[1..].starts_with(|c: char| c.is_ascii_digit() || c == '.')
}

/// Flags that take no value in createNode / setAttr / connectAttr.
const BOOL_FLAGS: &[&str] = &["-s", "-ss", "-na", "-f", "-av", "-cb", "-k", "-l", "-ch", "-shared", "-r"];

struct MaFile {
    nodes: Vec<MaNode>,
    by_name: HashMap<String, usize>,
    conns: Vec<(String, String)>,
    angle_rad: bool,
    linear_scale: f64,
}

fn parse(src: &str) -> MaFile {
    let mut f = MaFile { nodes: vec![], by_name: HashMap::new(), conns: vec![], angle_rad: false, linear_scale: 1.0 };
    let mut current: Option<usize> = None;
    for st in statements(src) {
        let cmd = st[0].as_str();
        let args = &st[1..];
        match cmd {
            "currentUnit" => {
                let mut i = 0;
                while i < args.len() {
                    match args[i].as_str() {
                        "-a" | "-angle" if args.get(i + 1).is_some_and(|a| a.starts_with("rad")) => f.angle_rad = true,
                        "-l" | "-linear" => {
                            f.linear_scale = match args.get(i + 1).map(|s| s.as_str()) {
                                Some("millimeter" | "mm") => 0.1,
                                Some("meter" | "m") => 100.0,
                                Some("inch" | "in") => 2.54,
                                Some("foot" | "ft") => 30.48,
                                Some("yard" | "yd") => 91.44,
                                _ => 1.0,
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            "createNode" => {
                let mut n = MaNode { ty: args.first().cloned().unwrap_or_default(), ..Default::default() };
                let mut i = 1;
                while i < args.len() {
                    match args[i].as_str() {
                        "-n" | "-name" => {
                            n.name = args.get(i + 1).cloned().unwrap_or_default();
                            i += 1;
                        }
                        "-p" | "-parent" => {
                            n.parent = args.get(i + 1).map(|p| p.trim_start_matches('|').rsplit('|').next().unwrap_or(p).to_string());
                            i += 1;
                        }
                        _ => {}
                    }
                    i += 1;
                }
                if n.name.is_empty() {
                    n.name = format!("{}{}", n.ty, f.nodes.len() + 1);
                }
                // `createNode ... -s` re-declares a shared node (persp, lambert1 ...): reuse it
                if let Some(&k) = f.by_name.get(&n.name) {
                    current = Some(k);
                    continue;
                }
                f.by_name.insert(n.name.clone(), f.nodes.len());
                current = Some(f.nodes.len());
                f.nodes.push(n);
            }
            "select" => {
                // select -ne name: later setAttrs apply to that node
                if let Some(name) = args.iter().rev().find(|a| !is_flag(a)) {
                    let name = name.trim_start_matches(':').trim_start_matches('|');
                    let name = name.rsplit('|').next().unwrap_or(name).to_string();
                    current = match f.by_name.get(&name) {
                        Some(&k) => Some(k),
                        None => {
                            let ty = match name.as_str() {
                                "time1" | "renderPartition" | "defaultRenderGlobals" | "defaultResolution" | "hardwareRenderGlobals" => name.clone(),
                                _ => String::new(),
                            };
                            f.by_name.insert(name.clone(), f.nodes.len());
                            f.nodes.push(MaNode { ty, name, ..Default::default() });
                            Some(f.nodes.len() - 1)
                        }
                    };
                }
            }
            "setAttr" => {
                let Some(k) = current else { continue };
                let mut sa = SetAttr { attr: String::new(), ty: None, vals: vec![] };
                let mut i = 0;
                while i < args.len() {
                    let a = &args[i];
                    if sa.attr.is_empty() && is_flag(a) {
                        if a == "-type" || a == "-typ" {
                            sa.ty = args.get(i + 1).cloned();
                            i += 2;
                            continue;
                        }
                        // flags with a value (-s 8, -k off, -l on, -ch 4, -cb on)
                        let takes = matches!(a.as_str(), "-s" | "-size" | "-k" | "-keyable" | "-l" | "-lock" | "-ch" | "-capacityHint" | "-cb" | "-channelBox");
                        i += if takes { 2 } else { 1 };
                        continue;
                    }
                    if sa.attr.is_empty() {
                        sa.attr = a.clone();
                    } else if a == "-type" || a == "-typ" {
                        sa.ty = args.get(i + 1).cloned();
                        i += 1;
                    } else {
                        sa.vals.push(a.clone());
                    }
                    i += 1;
                }
                if sa.attr.starts_with('.') {
                    f.nodes[k].attrs.push(sa);
                } else if let Some((node, attr)) = sa.attr.split_once('.') {
                    // setAttr "node.attr" ...
                    if let Some(&j) = f.by_name.get(node) {
                        sa.attr = format!(".{attr}");
                        f.nodes[j].attrs.push(sa);
                    }
                }
            }
            "connectAttr" => {
                let plugs: Vec<&String> = args.iter().filter(|a| !is_flag(a) && !BOOL_FLAGS.contains(&a.as_str())).collect();
                if plugs.len() >= 2 {
                    f.conns.push((plugs[0].trim_start_matches(':').to_string(), plugs[1].trim_start_matches(':').to_string()));
                }
            }
            "parent" => {
                let names: Vec<&String> = args.iter().filter(|a| !is_flag(a)).collect();
                if names.len() >= 2 {
                    let p = names[names.len() - 1].trim_start_matches('|').to_string();
                    for c in &names[..names.len() - 1] {
                        if let Some(&k) = f.by_name.get(c.trim_start_matches('|')) {
                            f.nodes[k].parent = Some(p.clone());
                        }
                    }
                }
            }
            "rename" => {
                if args.len() >= 2 {
                    if let Some(k) = f.by_name.remove(&args[0]) {
                        f.nodes[k].name = args[1].clone();
                        f.by_name.insert(args[1].clone(), k);
                    }
                }
            }
            _ => {}
        }
    }
    f
}

/// `.vt[0:7]` -> ("vt", Some((0, 7))); `.wl[3].w[0:1]` -> handled by the callers.
fn range_of(attr: &str) -> Option<(String, usize, usize)> {
    let a = attr.trim_start_matches('.');
    let (name, rest) = a.split_once('[')?;
    let inner = rest.strip_suffix(']')?;
    if inner.contains('[') {
        return None;
    }
    let (lo, hi) = match inner.split_once(':') {
        Some((l, h)) => (l.parse().ok()?, h.parse().ok()?),
        None => {
            let i: usize = inner.parse().ok()?;
            (i, i)
        }
    };
    Some((name.to_string(), lo, hi))
}

/// Values of an indexed array attribute gathered from every `setAttr` on it (`.vt[0:3]`, `.vt[4:7]` ...).
fn gather(n: &MaNode, names: &[&str], width: usize) -> BTreeMap<usize, Vec<f64>> {
    let mut out = BTreeMap::new();
    for a in &n.attrs {
        let Some((name, lo, hi)) = range_of(&a.attr) else { continue };
        if !names.contains(&name.as_str()) {
            continue;
        }
        let v: Vec<f64> = a.vals.iter().filter_map(|x| x.parse().ok()).collect();
        for i in lo..=hi {
            let k = (i - lo) * width;
            if k + width <= v.len() {
                out.insert(i, v[k..k + width].to_vec());
            }
        }
    }
    out
}

/// Mesh data stored on a mesh shape: vertices (+ tweaks), edges, polyFaces with UVs.
fn mesh_from_shape(n: &MaNode, unit: f64) -> Option<PolyMesh> {
    let vt = gather(n, &["vt", "vrts"], 3);
    if vt.is_empty() {
        return None;
    }
    let count = vt.keys().last().map(|k| k + 1).unwrap_or(0);
    let tweaks = gather(n, &["pt", "pnts"], 3);
    let v: Vec<DVec3> = (0..count)
        .map(|i| {
            let p = vt.get(&i).map(|p| DVec3::new(p[0], p[1], p[2])).unwrap_or_default();
            let t = tweaks.get(&i).map(|p| DVec3::new(p[0], p[1], p[2])).unwrap_or_default();
            (p + t) * unit
        })
        .collect();
    let ed = gather(n, &["ed", "edge"], 3);
    let edges: Vec<(u32, u32)> = (0..ed.keys().last().map(|k| k + 1).unwrap_or(0)).map(|i| ed.get(&i).map(|e| (e[0] as u32, e[1] as u32)).unwrap_or((0, 0))).collect();
    // UVs: .uvst[0].uvsp[a:b] (map1) or .uvpt
    let mut uvs: BTreeMap<usize, DVec2> = BTreeMap::new();
    for a in &n.attrs {
        let at = a.attr.trim_start_matches('.');
        let sub = at.strip_prefix("uvst[0].uvsp").or_else(|| at.strip_prefix("uvSet[0].uvSetPoints")).or_else(|| at.strip_prefix("uvpt"));
        let Some(sub) = sub else { continue };
        let Some((_, lo, hi)) = range_of(&format!(".x{sub}")) else { continue };
        let vals: Vec<f64> = a.vals.iter().filter_map(|x| x.parse().ok()).collect();
        for i in lo..=hi {
            let k = (i - lo) * 2;
            if k + 1 < vals.len() {
                uvs.insert(i, DVec2::new(vals[k], vals[k + 1]));
            }
        }
    }
    let mut pm = PolyMesh::new(v, vec![]);
    for a in n.attrs.iter().filter(|a| a.ty.as_deref() == Some("polyFaces")) {
        let t = &a.vals;
        let mut i = 0;
        let mut face: Option<Vec<u32>> = None;
        let mut face_uv: Option<Vec<DVec2>> = None;
        let flush = |pm: &mut PolyMesh, face: &mut Option<Vec<u32>>, fuv: &mut Option<Vec<DVec2>>| {
            if let Some(fv) = face.take() {
                if fv.len() >= 3 {
                    let uv = fuv.take().filter(|u| u.len() == fv.len());
                    pm.add_face(fv, uv, None);
                }
            }
            *fuv = None;
        };
        while i < t.len() {
            match t[i].as_str() {
                "f" => {
                    flush(&mut pm, &mut face, &mut face_uv);
                    let k: usize = t.get(i + 1).and_then(|x| x.parse().ok()).unwrap_or(0);
                    let mut fv = vec![];
                    for j in 0..k {
                        let e: i64 = t.get(i + 2 + j).and_then(|x| x.parse().ok()).unwrap_or(0);
                        // negative edges run backwards: -1 is edge 0 reversed
                        let (idx, rev) = if e < 0 { ((-e - 1) as usize, true) } else { (e as usize, false) };
                        if let Some(&(a, b)) = edges.get(idx) {
                            fv.push(if rev { b } else { a });
                        }
                    }
                    face = Some(fv);
                    i += 2 + k;
                }
                "mu" | "mf" => {
                    // mu <uvSet> <n> uv...
                    let set: usize = t.get(i + 1).and_then(|x| x.parse().ok()).unwrap_or(0);
                    let k: usize = t.get(i + 2).and_then(|x| x.parse().ok()).unwrap_or(0);
                    if set == 0 && t[i] == "mu" {
                        face_uv = Some((0..k).filter_map(|j| t.get(i + 3 + j).and_then(|x| x.parse::<usize>().ok()).and_then(|u| uvs.get(&u).copied())).collect());
                    }
                    i += 3 + k;
                }
                "h" => {
                    // a hole: skipped (Inca faces have no holes)
                    flush(&mut pm, &mut face, &mut face_uv);
                    let k: usize = t.get(i + 1).and_then(|x| x.parse().ok()).unwrap_or(0);
                    i += 2 + k;
                }
                "fc" | "mc" => {
                    let k: usize = t.get(i + 1).and_then(|x| x.parse().ok()).unwrap_or(0);
                    i += 2 + k;
                }
                _ => i += 1,
            }
        }
        flush(&mut pm, &mut face, &mut face_uv);
    }
    (!pm.f.is_empty()).then_some(pm)
}

/// Maya creation nodes Inca builds as primitives (Maya short attribute name -> Inca parameter).
fn primitive_params(n: &MaNode) -> Option<(String, Params)> {
    let g = crate::primitives::gen_info(&n.ty)?;
    if g.nurbs {
        return None;
    }
    const EXTRA: &[(&str, &str)] = &[("sw", "subdivisionsWidth"), ("sh", "subdivisionsHeight"), ("sd", "subdivisionsDepth"), ("sc", "subdivisionsCaps"), ("ns", "numberOfSides"), ("t", "thickness")];
    let mut p = Params::new();
    for a in &n.attrs {
        let key = a.attr.trim_start_matches('.');
        let long = g.short.iter().chain(EXTRA.iter()).find(|(s, _)| *s == key).map(|x| x.1).unwrap_or(key);
        let long = if n.ty == "polyCone" && long == "subdivisionsCaps" { "subdivisionsCap" } else { long };
        if g.params.iter().any(|(k, _)| *k == long) {
            if let Some(v) = a.vals.first().and_then(|x| x.parse::<f64>().ok()) {
                p.insert(long.to_string(), json!(v));
            }
        }
    }
    Some((n.ty.clone(), p))
}

fn plug_node(plug: &str) -> &str {
    plug.split('.').next().unwrap_or(plug).trim_start_matches('|').rsplit('|').next().unwrap_or(plug)
}
fn plug_attr(plug: &str) -> &str {
    plug.split_once('.').map(|x| x.1).unwrap_or("")
}

const MAT_TYPES: &[&str] = &["lambert", "blinn", "phong", "standardSurface", "surfaceShader"];
const LIGHTS: &[&str] = &["ambientLight", "directionalLight", "pointLight", "spotLight", "areaLight"];

fn rgb(n: &MaNode, names: &[&str]) -> Option<Value> {
    n.v3(names).map(|c| json!(c))
}

/// Import a Maya ASCII file into the scene; returns the new top-level nodes.
pub fn import_ma(s: &mut Scene, src: &str) -> Result<Vec<Id>, String> {
    if !src.trim_start().starts_with("//Maya ASCII") {
        return Err("not a Maya ASCII file (expected a //Maya ASCII header)".into());
    }
    let f = parse(src);
    let unit = f.linear_scale;
    let ang = |a: [f64; 3]| if f.angle_rad { a.map(f64::to_degrees) } else { a };
    let mut dag: HashMap<String, Id> = HashMap::new();
    let mut made: Vec<Id> = vec![];
    let shape_of: HashMap<&str, &MaNode> = f.nodes.iter().filter(|n| n.parent.is_some() && matches!(n.ty.as_str(), "mesh" | "camera" | "nurbsCurve" | "locator") || LIGHTS.contains(&n.ty.as_str())).map(|n| (n.parent.as_deref().unwrap_or(""), n)).collect();
    let src_of = |dst_node: &str, dst_attrs: &[&str]| -> Option<&str> {
        f.conns.iter().find(|(_, d)| plug_node(d) == dst_node && dst_attrs.contains(&plug_attr(d))).map(|(sp, _)| sp.as_str())
    };
    // ---- materials and textures
    let mut mats: HashMap<String, Id> = HashMap::new();
    for n in f.nodes.iter().filter(|n| MAT_TYPES.contains(&n.ty.as_str())) {
        let id = match n.name.as_str() {
            "lambert1" => s.default_mat.clone(),
            _ => s.create_material(&n.ty, Some(&n.name)),
        };
        let r = crate::scene::NodeRef::Material(id.clone());
        let set = |s: &mut Scene, k: &str, v: Option<Value>| {
            if let Some(v) = v {
                s.set_attr(&r, k, v);
            }
        };
        if n.ty == "standardSurface" {
            set(s, "baseColor", rgb(n, &["bc", "baseColor"]));
            set(s, "base", n.f(&["b", "base"]).map(|x| json!(x)));
            set(s, "metalness", n.f(&["m", "metalness"]).map(|x| json!(x)));
            set(s, "specularRoughness", n.f(&["sr", "specularRoughness"]).map(|x| json!(x)));
            set(s, "emission", n.f(&["e", "emission"]).map(|x| json!(x)));
            set(s, "emissionColor", rgb(n, &["ec", "emissionColor"]));
        } else if n.ty == "surfaceShader" {
            set(s, "outColor", rgb(n, &["oc", "outColor"]));
        } else {
            set(s, "color", rgb(n, &["c", "color"]));
            set(s, "diffuse", n.f(&["dc", "diffuse"]).map(|x| json!(x)));
            set(s, "transparency", rgb(n, &["it", "transparency"]));
            set(s, "incandescence", rgb(n, &["ic", "incandescence"]));
            set(s, "eccentricity", n.f(&["ec", "eccentricity"]).map(|x| json!(x)));
            set(s, "specularColor", rgb(n, &["sc", "specularColor"]));
            set(s, "cosinePower", n.f(&["cp", "cosinePower"]).map(|x| json!(x)));
            set(s, "reflectivity", n.f(&["rfl", "reflectivity"]).map(|x| json!(x)));
        }
        // file / checker textures on the colour
        let col_attrs: &[&str] = if n.ty == "standardSurface" { &["bc", "baseColor"] } else { &["c", "color"] };
        if let Some(t) = src_of(&n.name, col_attrs).map(plug_node).and_then(|tn| f.by_name.get(tn)).map(|&k| &f.nodes[k]) {
            let tex = match t.ty.as_str() {
                "file" => {
                    let id = s.create_texture("file", Some(&t.name));
                    if let Some(p) = t.s(&["ftn", "fileTextureName"]) {
                        s.set_attr(&crate::scene::NodeRef::Texture(id.clone()), "fileTextureName", json!(p));
                    }
                    Some(id)
                }
                "checker" | "ramp" | "noise" | "grid" => Some(s.create_texture(&t.ty, Some(&t.name))),
                _ => None,
            };
            if let (Some(tid), Some(m)) = (tex, s.mats.get_mut(&id)) {
                let slot = if n.ty == "standardSurface" { "baseColor" } else { "color" };
                m.maps.insert(slot.into(), json!(tid));
            }
        }
        mats.insert(n.name.clone(), id);
    }
    // shading group -> material
    let mut sg_mat: HashMap<String, Id> = HashMap::new();
    for n in f.nodes.iter().filter(|n| n.ty == "shadingEngine") {
        if let Some(m) = src_of(&n.name, &["ss", "surfaceShader"]).map(plug_node).and_then(|m| mats.get(m)) {
            sg_mat.insert(n.name.clone(), m.clone());
        }
    }
    if let Some(m) = mats.get("lambert1") {
        sg_mat.entry("initialShadingGroup".into()).or_insert(m.clone());
    }
    // ---- DAG: transforms (and joints) in file order, parents before children
    let order: Vec<&MaNode> = f.nodes.iter().filter(|n| matches!(n.ty.as_str(), "transform" | "joint")).collect();
    let startup = ["persp", "top", "front", "side"];
    for n in &order {
        if startup.contains(&n.name.as_str()) && s.find(&n.name).is_some_and(|i| s.node(&i).is_some_and(|x| matches!(&x.shape, Shape::Camera(c) if c.startup))) {
            // the file's persp view: take its transform
            if n.name == "persp" {
                if let Some(id) = s.find("persp") {
                    if let Some(t) = n.v3(&["t", "translate"]) {
                        s.nodes.get_mut(&id).unwrap().xf.t = t.map(|x| x * unit);
                    }
                    if let Some(r) = n.v3(&["r", "rotate"]) {
                        s.nodes.get_mut(&id).unwrap().xf.r = ang(r);
                    }
                }
            }
            continue;
        }
        let shape = shape_of.get(n.name.as_str()).copied();
        let parent = n.parent.as_ref().and_then(|p| dag.get(p)).cloned();
        let id = match (n.ty.as_str(), shape.map(|x| x.ty.as_str())) {
            ("joint", _) => {
                let id = s.create_handle("joint", &n.name);
                if let Some(r) = n.f(&["radi", "radius"]) {
                    s.nodes.get_mut(&id).unwrap().passthrough.insert("rig".into(), json!({ "radius": r }));
                }
                id
            }
            (_, Some("mesh")) => {
                let sh = shape.unwrap();
                // a primitive creation node feeding the shape keeps live history; else the stored mesh
                let creator = src_of(&sh.name, &["i", "inMesh"]).map(plug_node).and_then(|c| f.by_name.get(c)).map(|&k| &f.nodes[k]);
                let prim = creator.and_then(primitive_params);
                let id = match prim {
                    Some((ty, p)) if unit == 1.0 => s.create_mesh(&ty, &p, Some(&n.name)),
                    _ => {
                        // the shape's own data, or an intermediate (Orig) shape's under the same transform
                        let pm = mesh_from_shape(sh, unit).or_else(|| f.nodes.iter().filter(|x| x.ty == "mesh" && x.parent.as_deref() == Some(n.name.as_str())).find_map(|x| mesh_from_shape(x, unit)));
                        match pm {
                            Some(pm) => s.create_mesh_from(pm, &n.name, None, None),
                            None => {
                                s.warn(format!("{}: mesh data not stored in the file (history Inca can't rebuild); imported as an empty group", n.name));
                                s.create_group(&n.name, None)
                            }
                        }
                    }
                };
                // material from the shading group the shape belongs to
                let sg = f.conns.iter().find(|(sp, d)| plug_node(sp) == sh.name && (plug_attr(sp).starts_with("iog") || plug_attr(sp).starts_with("instObjGroups")) && sg_mat.contains_key(plug_node(d))).map(|(_, d)| plug_node(d).to_string());
                if let Some(m) = sg.and_then(|g| sg_mat.get(&g)).cloned() {
                    if let Some(ms) = s.nodes.get_mut(&id).and_then(|x| x.mesh_mut()) {
                        ms.material = m;
                    }
                }
                id
            }
            (_, Some("camera")) => {
                let sh = shape.unwrap();
                let cam = CamShape { focal_length: sh.f(&["fl", "focalLength"]).unwrap_or(35.0), ortho: sh.s(&["o", "orthographic"]).is_some_and(|v| v == "yes" || v == "1"), ortho_width: sh.f(&["ow", "orthographicWidth"]).unwrap_or(30.0) * unit, coi: sh.f(&["coi", "centerOfInterest"]).unwrap_or(5.0) * unit, ..Default::default() };
                s.create_camera(&n.name, cam, false)
            }
            (_, Some(lt)) if LIGHTS.contains(&lt) => {
                let sh = shape.unwrap();
                let id = s.create_light(lt);
                let r = crate::scene::NodeRef::Dag(id.clone());
                if let Some(c) = rgb(sh, &["cl", "color"]) {
                    s.set_attr(&r, "color", c);
                }
                if let Some(i) = sh.f(&["in", "intensity"]) {
                    s.set_attr(&r, "intensity", json!(i));
                }
                if let Some(c) = sh.f(&["ca", "coneAngle"]) {
                    s.set_attr(&r, "coneAngle", json!(if f.angle_rad { c.to_degrees() } else { c }));
                }
                crate::commands::rename(s, &id, &n.name);
                id
            }
            (_, Some("locator")) => {
                let id = s.create_locator(&n.name);
                crate::commands::rename(s, &id, &n.name);
                id
            }
            _ => s.create_group(&n.name, None),
        };
        if let Some(p) = &parent {
            s.reparent(&id, Some(p), None);
        }
        let x = &mut s.nodes.get_mut(&id).unwrap().xf;
        if let Some(t) = n.v3(&["t", "translate"]) {
            x.t = t.map(|v| v * unit);
        }
        if let Some(r) = n.v3(&["r", "rotate"]) {
            x.r = ang(r);
        }
        if let Some(sc) = n.v3(&["s", "scale"]) {
            x.s = sc;
        }
        if let Some(jo) = n.v3(&["jo", "jointOrient"]) {
            x.jo = ang(jo);
        }
        if let Some(p) = n.v3(&["rp", "rotatePivot"]) {
            x.p = p.map(|v| v * unit);
        }
        if let Some(ro) = n.f(&["ro", "rotateOrder"]) {
            x.ro = ["xyz", "yzx", "zxy", "xzy", "yxz", "zyx"][(ro as usize).min(5)].into();
        }
        if n.s(&["v", "visibility"]).is_some_and(|v| v == "no" || v == "0" || v == "off") {
            s.nodes.get_mut(&id).unwrap().visible = false;
        }
        if parent.is_none() {
            made.push(id.clone());
        }
        dag.insert(n.name.clone(), id);
    }
    // ---- skin clusters: joints from .ma[i] (matrix) connections, weights from .wl[v].w[j]
    for n in f.nodes.iter().filter(|n| n.ty == "skinCluster") {
        let mut joints: BTreeMap<usize, Id> = BTreeMap::new();
        for (sp, d) in &f.conns {
            if plug_node(d) == n.name {
                let a = plug_attr(d);
                if let Some(i) = a.strip_prefix("ma[").or_else(|| a.strip_prefix("matrix[")).and_then(|r| r.strip_suffix(']')).and_then(|r| r.parse::<usize>().ok()) {
                    if let Some(j) = dag.get(plug_node(sp)) {
                        joints.insert(i, j.clone());
                    }
                }
            }
        }
        // the deformed shape: skinCluster.og[0] -> shape.i
        let shape = f.conns.iter().find(|(sp, _)| plug_node(sp) == n.name && (plug_attr(sp).starts_with("og") || plug_attr(sp).starts_with("outputGeometry"))).map(|(_, d)| plug_node(d).to_string());
        let mesh = shape.and_then(|sh| f.by_name.get(&sh).map(|&k| &f.nodes[k])).and_then(|sh| sh.parent.clone()).and_then(|t| dag.get(&t).cloned());
        let (Some(mesh), false) = (mesh, joints.is_empty()) else { continue };
        let slots: Vec<usize> = joints.keys().copied().collect();
        let jl: Vec<Id> = joints.values().cloned().collect();
        let mut p = crate::skin::bind(s, &mesh, &jl, 4);
        let nv = s.node(&mesh).and_then(|x| x.mesh()).map(|m| m.mesh.v.len()).unwrap_or(0);
        let mut w: Vec<Vec<(usize, f64)>> = vec![vec![]; nv];
        for a in &n.attrs {
            // .wl[v].w[j] or .wl[v].w[a:b]
            let at = a.attr.trim_start_matches('.');
            let Some(rest) = at.strip_prefix("wl[").or_else(|| at.strip_prefix("weightList[")) else { continue };
            let Some((vi, rest)) = rest.split_once(']') else { continue };
            let Ok(vi) = vi.parse::<usize>() else { continue };
            let Some((_, lo, hi)) = range_of(&format!(".x{}", rest.trim_start_matches(".weights").trim_start_matches(".w"))) else { continue };
            let vals: Vec<f64> = a.vals.iter().filter_map(|x| x.parse().ok()).collect();
            for (k, j) in (lo..=hi).enumerate() {
                if let (Some(&x), Some(slot)) = (vals.get(k), slots.iter().position(|s| *s == j)) {
                    if vi < nv && x > 0.0 {
                        w[vi].push((slot, x));
                    }
                }
            }
        }
        if w.iter().any(|x| !x.is_empty()) {
            let wj: Vec<Value> = w.iter().enumerate().map(|(i, x)| {
                if x.is_empty() {
                    return p["weights"].as_array().and_then(|a| a.get(i)).cloned().unwrap_or(json!([]));
                }
                let t: f64 = x.iter().map(|e| e.1).sum::<f64>().max(1e-9);
                json!(x.iter().map(|(j, v)| json!([j, v / t])).collect::<Vec<_>>())
            }).collect();
            p.insert("weights".into(), Value::Array(wj));
        }
        s.add_history(&mesh, "skinCluster", p);
    }
    // ---- animation curves: animCurveT? nodes with .ktv[a:b] time/value pairs -> node.attr
    for n in f.nodes.iter().filter(|n| n.ty.starts_with("animCurveT")) {
        let Some(dst) = f.conns.iter().find(|(sp, _)| plug_node(sp) == n.name).map(|(_, d)| d.clone()) else { continue };
        let Some(target) = dag.get(plug_node(&dst)).cloned() else { continue };
        let attr = plug_attr(&dst).to_string();
        let kv = gather(n, &["ktv", "keyTimeValue"], 2);
        let angular = n.ty == "animCurveTA";
        for (_, tv) in kv {
            let v = if angular && f.angle_rad { tv[1].to_degrees() } else if n.ty == "animCurveTL" { tv[1] * unit } else { tv[1] };
            s.set_key(&target, &attr, Some(tv[0]), Some(v));
        }
    }
    s.apply_anim();
    s.touch();
    Ok(made)
}

// ---------------------------------------------------------------- writing

fn q(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}
/// Shortest text that reads back as the same f64.
fn n(x: f64) -> String {
    if x == 0.0 { "0".into() } else { format!("{x}") }
}

/// Write the scene (or the selection) as Maya ASCII.
pub fn export_ma(s: &Scene, sel_only: bool) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "//Maya ASCII 2020 scene");
    let _ = writeln!(o, "//Name: inca_export.ma");
    let _ = writeln!(o, "//Codeset: UTF-8");
    let _ = writeln!(o, "requires maya \"2020\";");
    let _ = writeln!(o, "currentUnit -l centimeter -a degree -t film;");
    let _ = writeln!(o, "fileInfo \"application\" \"maya\";");
    let _ = writeln!(o, "fileInfo \"product\" \"Inca {}\";", env!("CARGO_PKG_VERSION"));
    let keep: Option<HashSet<Id>> = sel_only.then(|| s.sel.iter().flat_map(|i| s.subtree(i)).collect());
    let ids: Vec<Id> = s.all_dag(false).into_iter().filter(|i| keep.as_ref().is_none_or(|k| k.contains(i))).filter(|i| s.node(i).is_some_and(|n| !n.startup())).collect();
    let name = |id: &str| s.name_of(id);
    let mut mat_users: BTreeMap<Id, Vec<String>> = BTreeMap::new();
    let mut skins: Vec<(String, String, Params)> = vec![];
    for id in &ids {
        let nd = s.node(id).unwrap();
        let ty = if nd.kind == "joint" { "joint" } else { "transform" };
        let parent = nd.parent.as_ref().filter(|p| ids.contains(p)).map(|p| format!(" -p {}", q(&name(p)))).unwrap_or_default();
        let _ = writeln!(o, "createNode {ty} -n {}{parent};", q(&nd.name));
        let x = &nd.xf;
        if x.t != [0.0; 3] {
            let _ = writeln!(o, "\tsetAttr \".t\" -type \"double3\" {} {} {} ;", n(x.t[0]), n(x.t[1]), n(x.t[2]));
        }
        if x.r != [0.0; 3] {
            let _ = writeln!(o, "\tsetAttr \".r\" -type \"double3\" {} {} {} ;", n(x.r[0]), n(x.r[1]), n(x.r[2]));
        }
        if x.s != [1.0; 3] {
            let _ = writeln!(o, "\tsetAttr \".s\" -type \"double3\" {} {} {} ;", n(x.s[0]), n(x.s[1]), n(x.s[2]));
        }
        if x.jo != [0.0; 3] {
            let _ = writeln!(o, "\tsetAttr \".jo\" -type \"double3\" {} {} {} ;", n(x.jo[0]), n(x.jo[1]), n(x.jo[2]));
        }
        if x.p != [0.0; 3] {
            let _ = writeln!(o, "\tsetAttr \".rp\" -type \"double3\" {} {} {} ;", n(x.p[0]), n(x.p[1]), n(x.p[2]));
            let _ = writeln!(o, "\tsetAttr \".sp\" -type \"double3\" {} {} {} ;", n(x.p[0]), n(x.p[1]), n(x.p[2]));
        }
        if let Some(ro) = ["xyz", "yzx", "zxy", "xzy", "yxz", "zyx"].iter().position(|r| *r == x.ro).filter(|r| *r != 0) {
            let _ = writeln!(o, "\tsetAttr \".ro\" {ro};");
        }
        if !nd.visible {
            let _ = writeln!(o, "\tsetAttr \".v\" no;");
        }
        let shape = nd.shape_name.clone().unwrap_or(format!("{}Shape", nd.name));
        match &nd.shape {
            Shape::Mesh(m) => {
                write_mesh(&mut o, &shape, &nd.name, &m.mesh);
                mat_users.entry(m.material.clone()).or_default().push(shape.clone());
                if let Some(h) = m.history.iter().find(|h| h.ty == "skinCluster") {
                    skins.push((nd.name.clone(), shape.clone(), h.params.clone()));
                }
            }
            Shape::Camera(c) => {
                let _ = writeln!(o, "createNode camera -n {} -p {};", q(&shape), q(&nd.name));
                let _ = writeln!(o, "\tsetAttr \".fl\" {};", n(c.focal_length));
                let _ = writeln!(o, "\tsetAttr \".coi\" {};", n(c.coi));
                if c.ortho {
                    let _ = writeln!(o, "\tsetAttr \".o\" yes;");
                    let _ = writeln!(o, "\tsetAttr \".ow\" {};", n(c.ortho_width));
                }
            }
            Shape::Light(l) if LIGHTS.contains(&l.ty.as_str()) => {
                let _ = writeln!(o, "createNode {} -n {} -p {};", l.ty, q(&shape), q(&nd.name));
                if let Some(c) = l.attrs.get("color").and_then(|v| v.as_array()) {
                    let c: Vec<f64> = c.iter().filter_map(|x| x.as_f64()).collect();
                    if c.len() == 3 {
                        let _ = writeln!(o, "\tsetAttr \".cl\" -type \"float3\" {} {} {} ;", n(c[0]), n(c[1]), n(c[2]));
                    }
                }
                if let Some(i) = l.attrs.get("intensity").and_then(|v| v.as_f64()) {
                    let _ = writeln!(o, "\tsetAttr \".in\" {};", n(i));
                }
                if let Some(c) = l.attrs.get("coneAngle").and_then(|v| v.as_f64()) {
                    let _ = writeln!(o, "\tsetAttr \".ca\" {};", n(c));
                }
            }
            Shape::Locator if nd.kind == "locator" => {
                let _ = writeln!(o, "createNode locator -n {} -p {};", q(&shape), q(&nd.name));
            }
            _ => {}
        }
    }
    // materials, shading groups and assignments
    let mut conns: Vec<String> = vec![];
    for (mid, shapes) in &mat_users {
        let Some(m) = s.mats.get(mid) else { continue };
        let (mname, sg) = if *mid == s.default_mat { ("lambert1".to_string(), "initialShadingGroup".to_string()) } else { (m.name.clone(), format!("{}SG", m.name)) };
        if *mid != s.default_mat {
            let _ = writeln!(o, "createNode {} -n {};", m.ty, q(&mname));
            let a = &m.attrs;
            let c3 = |o: &mut String, k: &str, short: &str| {
                if let Some(c) = a.get(k).and_then(|v| v.as_array()) {
                    let c: Vec<f64> = c.iter().filter_map(|x| x.as_f64()).collect();
                    if c.len() == 3 {
                        let _ = writeln!(o, "\tsetAttr \".{short}\" -type \"float3\" {} {} {} ;", n(c[0]), n(c[1]), n(c[2]));
                    }
                }
            };
            let f1 = |o: &mut String, k: &str, short: &str| {
                if let Some(v) = a.get(k).and_then(|v| v.as_f64()) {
                    let _ = writeln!(o, "\tsetAttr \".{short}\" {};", n(v));
                }
            };
            match m.ty.as_str() {
                "standardSurface" => {
                    c3(&mut o, "baseColor", "bc");
                    f1(&mut o, "base", "b");
                    f1(&mut o, "metalness", "m");
                    f1(&mut o, "specularRoughness", "sr");
                }
                "surfaceShader" => c3(&mut o, "outColor", "oc"),
                _ => {
                    c3(&mut o, "color", "c");
                    f1(&mut o, "diffuse", "dc");
                    c3(&mut o, "transparency", "it");
                    f1(&mut o, "eccentricity", "ec");
                    c3(&mut o, "specularColor", "sc");
                    f1(&mut o, "cosinePower", "cp");
                }
            }
            let _ = writeln!(o, "createNode shadingEngine -n {};", q(&sg));
            let _ = writeln!(o, "\tsetAttr \".ihi\" 0;");
            let _ = writeln!(o, "\tsetAttr \".ro\" yes;");
            let _ = writeln!(o, "createNode materialInfo -n {};", q(&format!("{}MaterialInfo", m.name)));
            conns.push(format!("connectAttr {} {};", q(&format!("{mname}.oc")), q(&format!("{sg}.ss"))));
            conns.push(format!("connectAttr {} {} -na;", q(&format!("{sg}.pa")), q(":renderPartition.st")));
            conns.push(format!("connectAttr {} {};", q(&format!("{sg}.msg")), q(&format!("{}MaterialInfo.sg", m.name))));
            // a file texture on the colour slot
            if let Some(t) = m.maps.get(m.color_attr()).and_then(|v| v.as_str()).and_then(|t| s.texs.get(t)) {
                let _ = writeln!(o, "createNode {} -n {};", if t.ty == "file" { "file" } else { t.ty.as_str() }, q(&t.name));
                if let Some(p) = t.attrs.get("fileTextureName").and_then(|v| v.as_str()) {
                    let _ = writeln!(o, "\tsetAttr \".ftn\" -type \"string\" {};", q(p));
                }
                let slot = if m.ty == "standardSurface" { "bc" } else { "c" };
                conns.push(format!("connectAttr {} {};", q(&format!("{}.oc", t.name)), q(&format!("{mname}.{slot}"))));
            }
        }
        for sh in shapes {
            conns.push(format!("connectAttr {} {} -na;", q(&format!("{sh}.iog")), q(&format!("{sg}.dsm"))));
        }
    }
    // skin clusters
    for (mesh, shape, p) in &skins {
        let cname = format!("skinCluster_{mesh}");
        let joints: Vec<String> = p.get("joints").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|j| j.as_str()).map(|j| name(j)).collect()).unwrap_or_default();
        let _ = writeln!(o, "createNode skinCluster -n {};", q(&cname));
        if let Some(ws) = p.get("weights").and_then(|v| v.as_array()) {
            for (vi, w) in ws.iter().enumerate() {
                for e in w.as_array().into_iter().flatten() {
                    if let (Some(j), Some(x)) = (e.get(0).and_then(|v| v.as_u64()), e.get(1).and_then(|v| v.as_f64())) {
                        let _ = writeln!(o, "\tsetAttr \".wl[{vi}].w[{j}]\" {};", n(x));
                    }
                }
            }
        }
        if let Some(bi) = p.get("bindInv").and_then(|v| v.as_array()) {
            for (k, m) in bi.iter().enumerate() {
                let m: Vec<String> = m.as_array().into_iter().flatten().filter_map(|x| x.as_f64()).map(n).collect();
                if m.len() == 16 {
                    let _ = writeln!(o, "\tsetAttr \".pm[{k}]\" -type \"matrix\" {};", m.join(" "));
                }
            }
        }
        let _ = writeln!(o, "\tsetAttr -s {} \".ma\";", joints.len());
        for (k, j) in joints.iter().enumerate() {
            conns.push(format!("connectAttr {} {};", q(&format!("{j}.wm")), q(&format!("{cname}.ma[{k}]"))));
        }
        conns.push(format!("connectAttr {} {};", q(&format!("{cname}.og[0]")), q(&format!("{shape}.i"))));
    }
    // animation
    for c in s.anim.curves.values() {
        if !ids.contains(&c.node) || c.keys.is_empty() {
            continue;
        }
        let node = name(&c.node);
        let ty = if c.attr.starts_with("rotate") {
            "animCurveTA"
        } else if c.attr.starts_with("translate") {
            "animCurveTL"
        } else {
            "animCurveTU"
        };
        let cn = format!("{node}_{}", c.attr);
        let _ = writeln!(o, "createNode {ty} -n {};", q(&cn));
        let kv: Vec<String> = c.keys.iter().map(|k| format!("{} {}", n(k.t), n(k.v))).collect();
        let _ = writeln!(o, "\tsetAttr -s {} \".ktv[0:{}]\" {};", c.keys.len(), c.keys.len() - 1, kv.join(" "));
        conns.push(format!("connectAttr {} {};", q(&format!("{cn}.o")), q(&format!("{node}.{}", c.attr))));
    }
    for c in conns {
        let _ = writeln!(o, "{c}");
    }
    let _ = writeln!(o, "// End of inca_export.ma");
    o
}

/// A mesh shape with vertices, edges, UVs and polyFaces (edge-based faces, as Maya stores them).
fn write_mesh(o: &mut String, shape: &str, parent: &str, pm: &PolyMesh) {
    let _ = writeln!(o, "createNode mesh -n {} -p {};", q(shape), q(parent));
    let _ = writeln!(o, "\tsetAttr -k off \".v\";");
    let _ = writeln!(o, "\tsetAttr \".uvst[0].uvsn\" -type \"string\" \"map1\";");
    let _ = writeln!(o, "\tsetAttr \".cuvs\" -type \"string\" \"map1\";");
    // UVs: one per face corner
    let mut uvs: Vec<DVec2> = vec![];
    let mut face_uv: Vec<Option<Vec<usize>>> = vec![];
    for f in 0..pm.f.len() {
        match pm.uv.as_ref().and_then(|u| u.get(f)).and_then(|u| u.as_ref()) {
            Some(list) if list.len() == pm.f[f].len() => {
                let base = uvs.len();
                uvs.extend(list.iter().copied());
                face_uv.push(Some((base..base + list.len()).collect()));
            }
            _ => face_uv.push(None),
        }
    }
    if !uvs.is_empty() {
        let vals: Vec<String> = uvs.iter().map(|u| format!("{} {}", n(u.x), n(u.y))).collect();
        let _ = writeln!(o, "\tsetAttr -s {} \".uvst[0].uvsp[0:{}]\" -type \"float2\" {};", uvs.len(), uvs.len() - 1, vals.join(" "));
    }
    if !pm.v.is_empty() {
        let vals: Vec<String> = pm.v.iter().map(|p| format!("{} {} {}", n(p.x), n(p.y), n(p.z))).collect();
        let _ = writeln!(o, "\tsetAttr -s {} \".vt[0:{}]\" {};", pm.v.len(), pm.v.len() - 1, vals.join(" "));
    }
    let topo = pm.topo();
    if !topo.edges.is_empty() {
        let vals: Vec<String> = topo.edges.iter().map(|(a, b)| format!("{a} {b} 0")).collect();
        let _ = writeln!(o, "\tsetAttr -s {} \".ed[0:{}]\" {};", topo.edges.len(), topo.edges.len() - 1, vals.join(" "));
    }
    if !pm.f.is_empty() {
        let mut faces = vec![];
        for (fi, f) in pm.f.iter().enumerate() {
            let mut es = vec![];
            for k in 0..f.len() {
                let (a, b) = (f[k], f[(k + 1) % f.len()]);
                let e = topo.edge(a, b).unwrap_or(0);
                // stored (a, b): forward; otherwise -(e + 1) for the reversed edge
                es.push(if topo.edges[e].0 == a { e as i64 } else { -(e as i64) - 1 });
            }
            let mut s = format!("f {} {}", f.len(), es.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(" "));
            if let Some(u) = &face_uv[fi] {
                let _ = write!(s, " mu 0 {} {}", u.len(), u.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" "));
            }
            faces.push(s);
        }
        let _ = writeln!(o, "\tsetAttr -s {} \".fc[0:{}]\" -type \"polyFaces\" {};", pm.f.len(), pm.f.len() - 1, faces.join(" "));
    }
}

/// Maya binary files are a proprietary chunked format; Inca asks for ASCII instead.
pub const MB_MESSAGE: &str = "Maya Binary (.mb) files use Autodesk's undocumented binary layout. In Maya, use File > Save Scene As > Maya ASCII (.ma) and open the .ma in Inca.";

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE: &str = r#"//Maya ASCII 2020 scene
requires maya "2020";
currentUnit -l centimeter -a degree -t film;
createNode transform -s -n "persp";
	setAttr ".t" -type "double3" 10 8 10 ;
createNode transform -n "pCube1";
	setAttr ".t" -type "double3" 1 2 3 ;
	setAttr ".r" -type "double3" 0 45 0 ;
createNode mesh -n "pCubeShape1" -p "pCube1";
	setAttr -k off ".v";
createNode transform -n "quad";
createNode mesh -n "quadShape" -p "quad";
	setAttr -s 4 ".uvst[0].uvsp[0:3]" -type "float2" 0 0 1 0 1 1 0 1;
	setAttr -s 4 ".vt[0:3]"  -1 0 1 1 0 1 1 0 -1 -1 0 -1;
	setAttr -s 4 ".ed[0:3]"  0 1 0 1 2 0 2 3 0 3 0 0;
	setAttr -s 1 ".fc[0]" -type "polyFaces" f 4 0 1 2 3 mu 0 4 0 1 2 3;
createNode polyCube -n "polyCube1";
	setAttr ".w" 2;
	setAttr ".sw" 3;
createNode lambert -n "red";
	setAttr ".c" -type "float3" 1 0 0 ;
createNode shadingEngine -n "redSG";
createNode animCurveTL -n "pCube1_translateX";
	setAttr -s 2 ".ktv[0:1]"  1 0 24 10;
connectAttr "polyCube1.out" "pCubeShape1.i";
connectAttr "red.oc" "redSG.ss";
connectAttr "quadShape.iog" "redSG.dsm" -na;
connectAttr "pCube1_translateX.o" "pCube1.tx";
"#;

    #[test]
    fn imports_meshes_primitives_materials_and_keys() {
        let mut s = Scene::new();
        let made = import_ma(&mut s, CUBE).unwrap();
        assert_eq!(made.len(), 2);
        let cube = s.find("pCube1").unwrap();
        let m = s.node(&cube).unwrap().mesh().unwrap();
        assert_eq!(m.history[0].ty, "polyCube");
        assert_eq!(m.history[0].params["width"], json!(2.0));
        assert_eq!(m.mesh.f.len(), 14, "3x1x1 subdivided cube");
        let quad = s.find("quad").unwrap();
        let q = &s.node(&quad).unwrap().mesh().unwrap();
        assert_eq!(q.mesh.f, vec![vec![0, 1, 2, 3]]);
        assert!(q.mesh.uv.is_some());
        assert_eq!(s.mats[&q.material].name, "red");
        s.set_time(24.0);
        assert!((s.node(&cube).unwrap().xf.t[0] - 10.0).abs() < 1e-9);
        assert_eq!(s.node(&cube).unwrap().xf.r, [0.0, 45.0, 0.0]);
    }

    #[test]
    fn round_trips_through_export() {
        let mut s = Scene::new();
        crate::mel::Interp::new().run(&mut s, &mut crate::commands::NoHost, "polySphere -n ball; move 1 2 3 ball; polyCube -n box; rotate 0 30 0 box; setKeyframe -t 1 -at tx box; move 5 0 0 box; setKeyframe -t 10 -at tx box;").unwrap();
        let txt = export_ma(&s, false);
        let mut t = Scene::new();
        import_ma(&mut t, &txt).unwrap();
        let (a, b) = (s.find("ball").unwrap(), t.find("ball").unwrap());
        let (ma, mb) = (&s.node(&a).unwrap().mesh().unwrap().mesh, &t.node(&b).unwrap().mesh().unwrap().mesh);
        assert_eq!(ma.f, mb.f);
        assert_eq!(ma.v.len(), mb.v.len());
        for (i,(p,q)) in ma.v.iter().zip(&mb.v).enumerate() { assert!((*p - *q).length() < 1e-9, "{i} {p:?} {q:?}"); }
        assert_eq!(t.node(&b).unwrap().xf.t, [1.0, 2.0, 3.0]);
        let bx = t.find("box").unwrap();
        assert_eq!(t.node(&bx).unwrap().xf.r[1], 30.0);
        assert_eq!(t.anim.curves.values().filter(|c| c.node == bx).count(), 1);
    }

    #[test]
    fn skin_weights_survive() {
        let mut s = Scene::new();
        crate::mel::Interp::new().run(&mut s, &mut crate::commands::NoHost, "polyCylinder -h 4 -sy 4 -n arm; select -cl; joint -p 0 -2 0 -n j1; joint -p 0 2 0 -n j2; select -r j1 arm; skinCluster -tsb -mi 2;").unwrap();
        let txt = export_ma(&s, false);
        let mut t = Scene::new();
        import_ma(&mut t, &txt).unwrap();
        let arm = t.find("arm").unwrap();
        let h = t.node(&arm).unwrap().mesh().unwrap().history.iter().find(|h| h.ty == "skinCluster").cloned().expect("skin");
        let a0 = s.node(&s.find("arm").unwrap()).unwrap().mesh().unwrap().history.iter().find(|h| h.ty == "skinCluster").unwrap().params["weights"].clone();
        let w = &h.params["weights"];
        assert_eq!(w.as_array().unwrap().len(), a0.as_array().unwrap().len());
        // vertex 0's weights match
        let get = |v: &Value| v[0].as_array().unwrap().iter().map(|e| (e[0].as_u64().unwrap(), (e[1].as_f64().unwrap() * 1e4).round())).collect::<Vec<_>>();
        assert_eq!(get(w), get(&a0));
    }

    #[test]
    fn tolerates_real_maya_boilerplate() {
        let src = r#"//Maya ASCII 2024 scene
//Name: test.ma
requires maya "2024";
requires "stereoCamera" "10.0";
currentUnit -l centimeter -a degree -t film;
fileInfo "application" "maya";
fileInfo "osv" "Windows 10 Pro v2009 (Build: 19045)";
createNode transform -s -n "persp";
	rename -uid "C1A2B3C4-0000-0000-0000-000000000001";
	setAttr ".v" no;
	setAttr ".t" -type "double3" 28 21 28 ;
	setAttr ".r" -type "double3" -27.9 45 0 ;
createNode camera -s -n "perspShape" -p "persp";
	setAttr -k off ".v" no;
	setAttr ".fl" 34.999999999999986;
	setAttr ".imn" -type "string" "persp";
createNode transform -n "pSphere1";
	rename -uid "C1A2B3C4-0000-0000-0000-000000000002";
createNode mesh -n "pSphereShape1" -p "pSphere1";
	setAttr -k off ".v";
	setAttr ".vir" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr ".dcc" -type "string" "Ambient+Diffuse";
createNode polySphere -n "polySphere1";
	rename -uid "C1A2B3C4-0000-0000-0000-000000000003";
	setAttr ".r" 3;
createNode lightLinker -s -n "lightLinker1";
	setAttr -s 2 ".lnk";
select -ne :time1;
	setAttr ".o" 1;
	setAttr ".unw" 1;
select -ne :renderPartition;
	setAttr -s 2 ".st";
select -ne :initialShadingGroup;
	setAttr ".ro" yes;
connectAttr "polySphere1.out" "pSphereShape1.i";
relationship "link" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
connectAttr "pSphereShape1.iog" ":initialShadingGroup.dsm" -na;
// End of test.ma
"#;
        let mut s = Scene::new();
        let made = import_ma(&mut s, src).unwrap();
        assert_eq!(made.len(), 1);
        let ball = s.find("pSphere1").unwrap();
        let m = s.node(&ball).unwrap().mesh().unwrap();
        assert_eq!(m.history[0].ty, "polySphere");
        assert_eq!(m.history[0].params["radius"], json!(3.0));
    }
}
