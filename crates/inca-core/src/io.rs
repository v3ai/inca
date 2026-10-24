//! File IO: native `.inca` scenes plus OBJ (tobj), STL (stl_io) and glTF/GLB (gltf) import,
//! and OBJ/MTL, STL, GLB and `.inca` export.

use crate::math::{DMat4, DVec2, DVec3};
use crate::mesh::PolyMesh;
use crate::scene::{Id, Scene, Shape};
use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;

pub const IMPORT_EXTS: &[&str] = &["inca", "ma", "mb", "obj", "stl", "gltf", "glb"];
pub const EXPORT_EXTS: &[&str] = &["inca", "ma", "obj", "stl", "glb"];

fn ext(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}
fn stem(p: &Path) -> String {
    p.file_stem().and_then(|e| e.to_str()).unwrap_or("import").to_string()
}

pub fn save_scene(s: &mut Scene, path: &Path) -> Result<(), String> {
    let data = s.serialize();
    std::fs::write(path, serde_json::to_string(&data).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    s.file = Some(path.to_path_buf());
    s.modified = false;
    Ok(())
}

pub fn open_scene(s: &mut Scene, path: &Path) -> Result<(), String> {
    // a Maya scene replaces the current one but keeps no .inca path, so Save asks where to write
    if matches!(ext(path).as_str(), "ma" | "mb") {
        if ext(path) == "mb" {
            return Err(crate::maya::MB_MESSAGE.into());
        }
        let txt = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        s.reset();
        crate::maya::import_ma(s, &txt)?;
        s.modified = false;
        s.undo_stack.clear();
        s.redo_stack.clear();
        return Ok(());
    }
    let txt = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
    s.load(&v)?;
    s.file = Some(path.to_path_buf());
    s.modified = false;
    s.undo_stack.clear();
    s.redo_stack.clear();
    Ok(())
}

/// Import a file into the current scene; returns the new top-level nodes.
pub fn import(s: &mut Scene, path: &Path) -> Result<Vec<Id>, String> {
    match ext(path).as_str() {
        "inca" => import_inca(s, path),
        "obj" => import_obj(s, path),
        "stl" => import_stl(s, path),
        "gltf" | "glb" => import_gltf(s, path),
        "ma" => crate::maya::import_ma(s, &std::fs::read_to_string(path).map_err(|e| e.to_string())?),
        "mb" => Err(crate::maya::MB_MESSAGE.into()),
        e => Err(format!("can't import .{e} files")),
    }
}

fn import_inca(s: &mut Scene, path: &Path) -> Result<Vec<Id>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
    // materials first (remapped to fresh ids)
    let mut mat_map = std::collections::HashMap::new();
    if let Some(mats) = v["shading"]["mats"].as_array() {
        for m in mats {
            let name = m["name"].as_str().unwrap_or("material1");
            if name == "lambert1" || name == "standardSurface1" {
                if let Some(id) = s.mats.values().find(|x| x.name == name).map(|x| x.id.clone()) {
                    mat_map.insert(m["id"].as_str().unwrap_or("").to_string(), id);
                }
                continue;
            }
            let id = s.create_material(m["type"].as_str().unwrap_or("lambert"), Some(name));
            if let Some(a) = m["attrs"].as_object() {
                s.mats.get_mut(&id).unwrap().attrs.extend(a.clone());
            }
            mat_map.insert(m["id"].as_str().unwrap_or("").to_string(), id);
        }
    }
    let mut map: std::collections::HashMap<String, Id> = Default::default();
    let mut made = vec![];
    for d in v["nodes"].as_array().ok_or("not an Inca scene")? {
        if d["cam"]["startup"].as_bool() == Some(true) {
            continue;
        }
        let parent = d["parent"].as_str().and_then(|p| map.get(p).cloned());
        if d["parent"].is_string() && parent.is_none() {
            continue;
        }
        let mut d = d.clone();
        if let Some(m) = d["material"].as_str().and_then(|m| mat_map.get(m)) {
            d["material"] = json!(m);
        }
        let id = s.deserialize_node(&d, parent.as_deref(), true);
        map.insert(d["id"].as_str().unwrap_or("").to_string(), id.clone());
        if parent.is_none() {
            made.push(id);
        }
    }
    Ok(made)
}

fn import_obj(s: &mut Scene, path: &Path) -> Result<Vec<Id>, String> {
    let opts = tobj::LoadOptions { triangulate: false, single_index: false, ignore_points: true, ignore_lines: true };
    let (models, mats) = tobj::load_obj(path, &opts).map_err(|e| e.to_string())?;
    let mats = mats.unwrap_or_default();
    let mat_ids: Vec<Id> = mats
        .iter()
        .map(|m| {
            let id = s.create_material(if m.shininess.unwrap_or(0.0) > 1.0 { "blinn" } else { "lambert" }, Some(&m.name));
            if let Some(d) = m.diffuse {
                s.mats.get_mut(&id).unwrap().attrs.insert("color".into(), json!([d[0], d[1], d[2]]));
                s.mats.get_mut(&id).unwrap().attrs.insert("diffuse".into(), json!(1.0));
            }
            if let Some(tex) = &m.diffuse_texture {
                let t = s.create_texture("file", None);
                let full = path.parent().map(|p| p.join(tex)).unwrap_or_else(|| tex.into());
                s.texs.get_mut(&t).unwrap().attrs.insert("fileTextureName".into(), json!(full.to_string_lossy()));
                s.mats.get_mut(&id).unwrap().maps.insert("color".into(), json!(t));
            }
            id
        })
        .collect();
    let mut made = vec![];
    for m in models {
        let me = &m.mesh;
        let v: Vec<DVec3> = me.positions.chunks(3).map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)).collect();
        let mut pm = PolyMesh::new(v, vec![]);
        let arities: Vec<u32> = if me.face_arities.is_empty() { vec![3; me.indices.len() / 3] } else { me.face_arities.clone() };
        let has_uv = !me.texcoords.is_empty() && me.texcoord_indices.len() == me.indices.len();
        let mut k = 0usize;
        for a in arities {
            let a = a as usize;
            let f: Vec<u32> = me.indices[k..k + a].to_vec();
            let uv = has_uv.then(|| me.texcoord_indices[k..k + a].iter().map(|&t| DVec2::new(me.texcoords[t as usize * 2] as f64, me.texcoords[t as usize * 2 + 1] as f64)).collect());
            pm.add_face(f, uv, None);
            k += a;
        }
        pm.compact();
        let mat = me.material_id.and_then(|i| mat_ids.get(i)).cloned();
        let name = if m.name.is_empty() || m.name == "unnamed_object" { stem(path) } else { m.name.clone() };
        let id = s.create_mesh_from(pm, &name, mat.as_deref(), None);
        made.push(id);
    }
    Ok(made)
}

fn import_stl(s: &mut Scene, path: &Path) -> Result<Vec<Id>, String> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let m = stl_io::read_stl(&mut f).map_err(|e| e.to_string())?;
    let v: Vec<DVec3> = m.vertices.iter().map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)).collect();
    let faces: Vec<Vec<u32>> = m.faces.iter().map(|t| t.vertices.iter().map(|&i| i as u32).collect()).collect();
    let pm = crate::mesh::op_quadrangulate(&PolyMesh::new(v, faces), 1.0);
    Ok(vec![s.create_mesh_from(pm, &stem(path), None, None)])
}

fn import_gltf(s: &mut Scene, path: &Path) -> Result<Vec<Id>, String> {
    let (doc, buffers, _images) = gltf::import(path).map_err(|e| e.to_string())?;
    let mats: Vec<Id> = doc
        .materials()
        .map(|m| {
            let id = s.create_material("standardSurface", Some(m.name().unwrap_or("gltfMaterial1")));
            let pbr = m.pbr_metallic_roughness();
            let c = pbr.base_color_factor();
            let a = &mut s.mats.get_mut(&id).unwrap().attrs;
            a.insert("baseColor".into(), json!([c[0], c[1], c[2]]));
            a.insert("base".into(), json!(1.0));
            a.insert("metalness".into(), json!(pbr.metallic_factor()));
            a.insert("specularRoughness".into(), json!(pbr.roughness_factor()));
            let e = m.emissive_factor();
            if e.iter().any(|x| *x > 0.0) {
                a.insert("emission".into(), json!(1.0));
                a.insert("emissionColor".into(), json!(e));
            }
            id
        })
        .collect();
    let mut made = vec![];
    fn walk(s: &mut Scene, n: gltf::Node, parent: Option<&str>, buffers: &[gltf::buffer::Data], mats: &[Id], made: &mut Vec<Id>) {
        let m = DMat4::from_cols_array_2d(&n.transform().matrix().map(|c| c.map(|x| x as f64)));
        let name = n.name().unwrap_or(if n.mesh().is_some() { "mesh1" } else { "transform1" }).to_string();
        let id = if let Some(mesh) = n.mesh() {
            let mut pm = PolyMesh::new(vec![], vec![]);
            let mut mat = None;
            for prim in mesh.primitives() {
                let r = prim.reader(|b| Some(&buffers[b.index()]));
                let Some(pos) = r.read_positions() else { continue };
                let base = pm.v.len() as u32;
                pm.v.extend(pos.map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)));
                let uvs: Option<Vec<DVec2>> = r.read_tex_coords(0).map(|t| t.into_f32().map(|u| DVec2::new(u[0] as f64, 1.0 - u[1] as f64)).collect());
                let idx: Vec<u32> = match r.read_indices() {
                    Some(i) => i.into_u32().collect(),
                    None => (0..(pm.v.len() as u32 - base)).collect(),
                };
                let pmat = prim.material().index().and_then(|i| mats.get(i)).cloned();
                if mat.is_none() {
                    mat = pmat.clone();
                }
                for t in idx.chunks(3) {
                    if t.len() < 3 {
                        continue;
                    }
                    let uv = uvs.as_ref().map(|u| t.iter().map(|&i| u[i as usize]).collect());
                    let fm = if pmat != mat { pmat.clone() } else { None };
                    pm.add_face(t.iter().map(|&i| i + base).collect(), uv, fm);
                }
            }
            let pm = crate::mesh::op_merge_vertices(&pm, None, 1e-7);
            let pm = crate::mesh::op_quadrangulate(&pm, 1.0);
            let id = s.create_mesh_from(pm, &name, mat.as_deref(), parent);
            id
        } else {
            s.create_group(&name, parent)
        };
        s.set_local_matrix(&id, m);
        if parent.is_none() {
            made.push(id.clone());
        }
        for c in n.children() {
            walk(s, c, Some(&id), buffers, mats, made);
        }
    }
    let scene = doc.default_scene().or_else(|| doc.scenes().next()).ok_or("glTF has no scene")?;
    for n in scene.nodes() {
        walk(s, n, None, &buffers, &mats, &mut made);
    }
    Ok(made)
}

/// Meshes to export (selection with descendants, or everything).
fn export_meshes(s: &Scene, sel_only: bool) -> Vec<Id> {
    let roots: Vec<Id> = if sel_only { s.sel.clone() } else { s.roots.clone() };
    let mut out = vec![];
    for r in roots {
        for i in s.subtree(&r) {
            if s.nodes[&i].mesh().is_some() && s.is_visible(&i) && !out.contains(&i) {
                out.push(i);
            }
        }
    }
    out
}

pub fn export(s: &Scene, path: &Path, sel_only: bool) -> Result<(), String> {
    match ext(path).as_str() {
        "obj" => export_obj(s, path, sel_only),
        "stl" => export_stl(s, path, sel_only),
        "glb" => export_glb(s, path, sel_only),
        "ma" => std::fs::write(path, crate::maya::export_ma(s, sel_only)).map_err(|e| e.to_string()),
        "inca" => {
            let mut data = s.serialize();
            if sel_only {
                let keep: std::collections::HashSet<String> = s.sel.iter().flat_map(|i| s.subtree(i)).collect();
                let nodes: Vec<Value> = data["nodes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|d| keep.contains(d["id"].as_str().unwrap_or("")) || d["cam"]["startup"].as_bool() == Some(true))
                    .map(|d| {
                        let mut d = d.clone();
                        if !d["parent"].as_str().is_some_and(|p| keep.contains(p)) {
                            d["parent"] = Value::Null;
                        }
                        d
                    })
                    .collect();
                data["nodes"] = json!(nodes);
            }
            std::fs::write(path, data.to_string()).map_err(|e| e.to_string())
        }
        e => Err(format!("can't export .{e} files")),
    }
}

fn export_obj(s: &Scene, path: &Path, sel_only: bool) -> Result<(), String> {
    let mut o = String::from("# Inca OBJ export\n");
    let mtl_name = format!("{}.mtl", stem(path));
    o += &format!("mtllib {mtl_name}\n");
    let (mut vbase, mut tbase) = (1usize, 1usize);
    let mut used_mats = vec![];
    for id in export_meshes(s, sel_only) {
        let n = &s.nodes[&id];
        let ms = n.mesh().unwrap();
        let pm = ms.mesh.transformed(&s.world_matrix(&id));
        o += &format!("o {}\n", n.name);
        for p in &pm.v {
            o += &format!("v {} {} {}\n", p.x, p.y, p.z);
        }
        let mut tcount = 0;
        for fi in 0..pm.f.len() {
            if let Some(u) = pm.face_uv(fi) {
                for t in u {
                    o += &format!("vt {} {}\n", t.x, t.y);
                }
                tcount += u.len();
            }
        }
        let mut cur_mat = String::new();
        let mut tk = tbase;
        for fi in 0..pm.f.len() {
            let mat = pm.face_mat(fi).cloned().unwrap_or(ms.material.clone());
            if mat != cur_mat {
                let name = s.mats.get(&mat).map(|m| m.name.clone()).unwrap_or("default".into());
                o += &format!("usemtl {name}\n");
                if !used_mats.contains(&mat) {
                    used_mats.push(mat.clone());
                }
                cur_mat = mat;
            }
            o += "f";
            let has_uv = pm.face_uv(fi).is_some();
            for &vi in &pm.f[fi] {
                if has_uv {
                    o += &format!(" {}/{}", vi as usize + vbase, tk);
                    tk += 1;
                } else {
                    o += &format!(" {}", vi as usize + vbase);
                }
            }
            o += "\n";
        }
        vbase += pm.v.len();
        tbase += tcount;
    }
    std::fs::write(path, o).map_err(|e| e.to_string())?;
    let mut m = String::new();
    for id in used_mats {
        let Some(mat) = s.mats.get(&id) else { continue };
        let p = mat.pbr();
        m += &format!("newmtl {}\nKd {} {} {}\nKs {s} {s} {s}\nNs {}\nd {}\n\n", mat.name, p.base[0], p.base[1], p.base[2], (1.0 - p.roughness) * 200.0, p.opacity, s = p.specular * 0.5);
    }
    std::fs::write(path.with_file_name(mtl_name), m).map_err(|e| e.to_string())
}

fn triangles_world(s: &Scene, sel_only: bool) -> Vec<[DVec3; 3]> {
    let mut tris = vec![];
    for id in export_meshes(s, sel_only) {
        let pm = s.mesh_world(&id).unwrap();
        for fi in 0..pm.f.len() {
            let f = &pm.f[fi];
            for t in pm.triangulate_face(fi) {
                tris.push([pm.v[f[t[0]] as usize], pm.v[f[t[1]] as usize], pm.v[f[t[2]] as usize]]);
            }
        }
    }
    tris
}

fn export_stl(s: &Scene, path: &Path, sel_only: bool) -> Result<(), String> {
    let tris: Vec<stl_io::Triangle> = triangles_world(s, sel_only)
        .into_iter()
        .map(|t| {
            let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
            let c = |p: DVec3| stl_io::Vector::new([p.x as f32, p.y as f32, p.z as f32]);
            stl_io::Triangle { normal: c(n), vertices: [c(t[0]), c(t[1]), c(t[2])] }
        })
        .collect();
    let mut f = std::fs::File::create(path).map_err(|e| e.to_string())?;
    stl_io::write_stl(&mut f, tris.iter()).map_err(|e| e.to_string())
}

/// Binary glTF: one mesh per object (flat-shaded corners), world transforms baked into nodes.
fn export_glb(s: &Scene, path: &Path, sel_only: bool) -> Result<(), String> {
    let mut bin: Vec<u8> = vec![];
    let mut views = vec![];
    let mut accessors = vec![];
    let mut meshes = vec![];
    let mut nodes = vec![];
    let mut materials = vec![];
    let mut mat_index: std::collections::HashMap<String, usize> = Default::default();
    let push = |bin: &mut Vec<u8>, data: &[u8]| -> (usize, usize) {
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let off = bin.len();
        bin.extend_from_slice(data);
        (off, data.len())
    };
    for id in export_meshes(s, sel_only) {
        let n = &s.nodes[&id];
        let ms = n.mesh().unwrap();
        let pm = &ms.mesh;
        let normals = pm.corner_normals();
        let mut pos: Vec<f32> = vec![];
        let mut nrm: Vec<f32> = vec![];
        let mut uvs: Vec<f32> = vec![];
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for fi in 0..pm.f.len() {
            let f = &pm.f[fi];
            let uv = pm.face_uv(fi);
            for t in pm.triangulate_face(fi) {
                for &c in &t {
                    let p = pm.v[f[c] as usize];
                    let q = [p.x as f32, p.y as f32, p.z as f32];
                    for k in 0..3 {
                        lo[k] = lo[k].min(q[k]);
                        hi[k] = hi[k].max(q[k]);
                    }
                    pos.extend_from_slice(&q);
                    let nn = normals[fi][c];
                    nrm.extend_from_slice(&[nn.x as f32, nn.y as f32, nn.z as f32]);
                    let u = uv.map(|u| u[c]).unwrap_or_default();
                    uvs.extend_from_slice(&[u.x as f32, 1.0 - u.y as f32]);
                }
            }
        }
        if pos.is_empty() {
            continue;
        }
        let count = pos.len() / 3;
        let mut attr = serde_json::Map::new();
        for (name, data, ty) in [("POSITION", &pos, "VEC3"), ("NORMAL", &nrm, "VEC3"), ("TEXCOORD_0", &uvs, "VEC2")] {
            let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
            let (off, len) = push(&mut bin, &bytes);
            views.push(json!({ "buffer": 0, "byteOffset": off, "byteLength": len, "target": 34962 }));
            let mut acc = json!({ "bufferView": views.len() - 1, "componentType": 5126, "count": count, "type": ty });
            if name == "POSITION" {
                acc["min"] = json!(lo);
                acc["max"] = json!(hi);
            }
            accessors.push(acc);
            attr.insert(name.into(), json!(accessors.len() - 1));
        }
        let mi = *mat_index.entry(ms.material.clone()).or_insert_with(|| {
            let m = s.mats.get(&ms.material);
            let p = m.map(|m| m.pbr()).unwrap_or_default();
            materials.push(json!({
                "name": m.map(|m| m.name.clone()).unwrap_or_default(),
                "pbrMetallicRoughness": { "baseColorFactor": [p.base[0], p.base[1], p.base[2], p.opacity], "metallicFactor": p.metalness, "roughnessFactor": p.roughness },
                "emissiveFactor": [p.emission[0].min(1.0), p.emission[1].min(1.0), p.emission[2].min(1.0)],
            }));
            materials.len() - 1
        });
        meshes.push(json!({ "name": n.shape_name, "primitives": [{ "attributes": attr, "material": mi }] }));
        let wm = s.world_matrix(&id).to_cols_array();
        nodes.push(json!({ "name": n.name, "mesh": meshes.len() - 1, "matrix": wm }));
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let gltf = json!({
        "asset": { "version": "2.0", "generator": "Inca" },
        "scene": 0,
        "scenes": [{ "nodes": (0..nodes.len()).collect::<Vec<_>>() }],
        "nodes": nodes, "meshes": meshes, "materials": materials, "accessors": accessors, "bufferViews": views,
        "buffers": [{ "byteLength": bin.len() }],
    });
    let mut js = gltf.to_string().into_bytes();
    while js.len() % 4 != 0 {
        js.push(b' ');
    }
    let total = 12 + 8 + js.len() + 8 + bin.len();
    let mut f = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut w = |b: &[u8]| f.write_all(b).map_err(|e| e.to_string());
    w(b"glTF")?;
    w(&2u32.to_le_bytes())?;
    w(&(total as u32).to_le_bytes())?;
    w(&(js.len() as u32).to_le_bytes())?;
    w(b"JSON")?;
    w(&js)?;
    w(&(bin.len() as u32).to_le_bytes())?;
    w(b"BIN\0")?;
    w(&bin)
}

/// Shapes other than meshes are skipped by mesh exporters; this counts them for a warning.
pub fn non_mesh_count(s: &Scene) -> usize {
    s.nodes.values().filter(|n| matches!(n.shape, Shape::Curve(_))).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;

    #[test]
    fn obj_stl_glb_round_trip() {
        let dir = std::env::temp_dir().join(format!("inca-io-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Scene::new();
        let c = s.create_mesh("polyCube", &Params::new(), None);
        s.set_attr_f(&c, "tx", 3.0);
        for e in ["obj", "stl", "glb", "inca"] {
            let p = dir.join(format!("t.{e}"));
            export(&s, &p, false).unwrap();
            let mut s2 = Scene::new();
            let made = import(&mut s2, &p).unwrap();
            assert_eq!(made.len(), 1, "{e}");
            let b = s2.world_bbox(&made[0]);
            assert!((b.center().x - 3.0).abs() < 1e-4, "{e}: {:?}", b);
            let n = &s2.nodes[&made[0]];
            assert_eq!(n.mesh().unwrap().mesh.f.len(), 6, "{e}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[cfg(test)]
mod compat_tests {
    use super::*;

    /// A scene in the exact layout the JS app's serializeScene() writes.
    const JS_SCENE: &str = r#"{"format":"inca-scene","version":"0.1.12","idCounter":40,
      "nodes":[
        {"id":"d3","name":"persp","kind":"camera","parent":null,"t":[28,21,28],"r":[-27.938,45,0],"s":[1,1,1],"ro":"xyz","p":[0,0,0],"pt":[0,0,0],"visible":true,"layer":null,"template":false,"locks":{},"shapeName":"perspShape","extra":null,
         "cam":{"focalLength":35,"near":0.1,"far":10000,"ortho":false,"orthoWidth":30,"coi":44.82,"horizontalFilmAperture":1.417,"verticalFilmAperture":0.945,"startup":true}},
        {"id":"d10","name":"grp","kind":"group","parent":null,"t":[0,1,0],"r":[0,0,0],"s":[1,1,1],"ro":"xyz","p":[0,0,0],"pt":[0,0,0],"visible":true,"layer":"l1","template":false,"locks":{"tx":true},"shapeName":null,"extra":null},
        {"id":"d11","name":"pCube1","kind":"mesh","parent":"d10","t":[2,0,0],"r":[0,30,0],"s":[1,1,1],"ro":"xyz","p":[0,0,0],"pt":[0,0,0],"visible":true,"layer":null,"template":false,"locks":{},"shapeName":"pCubeShape1","extra":null,
         "material":"m1","smoothLevel":0,"nurbs":false,
         "history":[{"id":"h12","name":"polyCube1","type":"polyCube","params":{"width":1,"height":1,"depth":1,"subdivisionsWidth":1,"subdivisionsHeight":1,"subdivisionsDepth":1}}],
         "mesh":{"v":[[-0.5,-0.5,0.5],[0.5,-0.5,0.5],[-0.5,0.5,0.5],[0.5,0.5,0.5],[-0.5,0.5,-0.5],[0.5,0.5,-0.5],[-0.5,-0.5,-0.5],[0.5,-0.5,-0.5]],
                 "f":[[0,1,3,2],[2,3,5,4],[4,5,7,6],[6,7,1,0],[1,7,5,3],[6,0,2,4]],"uv":null,"fm":null,"softAngle":60,"edgeHard":null,"displayEdges":null}},
        {"id":"d20","name":"pointLight1","kind":"light","parent":null,"t":[0,4,0],"r":[0,0,0],"s":[1,1,1],"ro":"xyz","p":[0,0,0],"pt":[0,0,0],"visible":true,"layer":null,"template":false,"locks":{},"shapeName":"pointLightShape1","extra":null,
         "light":{"type":"pointLight","color":[1,0.5,0.25],"intensity":2,"decayRate":0,"shadows":false,"shadowRadius":2}},
        {"id":"d30","name":"joint1","kind":"joint","parent":null,"t":[0,0,0],"r":[0,0,0],"s":[1,1,1],"ro":"xyz","p":[0,0,0],"pt":[0,0,0],"visible":true,"layer":null,"template":false,"locks":{},"shapeName":null,"extra":null,"rig":{"radius":0.5},"jo":[0,0,90]}
      ],
      "shading":{"mats":[{"id":"m1","name":"lambert1","type":"lambert","attrs":{"color":[0.5,0.5,0.5],"transparency":[0,0,0],"ambientColor":[0,0,0],"incandescence":[0,0,0],"diffuse":0.8,"bump":0},"maps":{}}],"texs":[],"defaultMatId":"m1"},
      "layers":[{"id":"l1","name":"layer1","visible":true,"mode":"normal","color":null}],
      "anim":[{"node":"d11","attr":"translateY","keys":[{"t":1,"v":0,"it":"auto","ot":"auto","is":0,"os":0},{"t":10,"v":5,"it":"auto","ot":"auto","is":0,"os":0}],"pre":"constant","post":"constant"}],
      "conns":[],"time":{"current":1,"start":1,"end":120,"animStart":1,"animEnd":200,"fps":24,"playing":false,"loop":"continuous","autoKey":false,"playEvery":false},
      "render":null,"sel":["d11"],"project":null}"#;

    #[test]
    fn loads_js_scene_and_writes_it_back() {
        let v: Value = serde_json::from_str(JS_SCENE).unwrap();
        let mut s = Scene::new();
        s.load(&v).unwrap();
        let c = s.find("pCube1").unwrap();
        assert_eq!(s.node(&c).unwrap().parent.as_deref(), Some("d10"));
        assert_eq!(s.node(&c).unwrap().mesh().unwrap().history[0].name, "polyCube1");
        assert_eq!(s.sel, vec!["d11".to_string()]);
        // history is live after loading
        let r = s.resolve("polyCube1").unwrap();
        s.set_attr(&r, "subdivisionsWidth", json!(2));
        assert_eq!(s.node(&c).unwrap().mesh().unwrap().mesh.f.len(), 10);
        // animation, layers, locks, light attributes and rig data survive
        s.set_time(10.0);
        assert!((s.get_attr_f(&c, "ty") - 5.0).abs() < 1e-9);
        let g = s.find("grp").unwrap();
        assert!(s.node(&g).unwrap().is_locked("tx"));
        assert_eq!(s.node(&g).unwrap().layer.as_deref(), Some("l1"));
        let l = s.find("pointLight1").unwrap();
        assert_eq!(s.get_attr_f(&l, "intensity"), 2.0);
        let out = s.serialize();
        let j = out["nodes"].as_array().unwrap().iter().find(|n| n["name"] == "joint1").unwrap();
        assert_eq!(j["kind"], "joint");
        assert_eq!(j["rig"]["radius"], 0.5);
        assert_eq!(serde_json::from_value::<[f64; 3]>(j["jo"].clone()).unwrap(), [0.0, 0.0, 90.0]);
        // the startup camera keeps its id and name
        assert!(out["nodes"].as_array().unwrap().iter().any(|n| n["id"] == "d3" && n["name"] == "persp"));
    }
}
