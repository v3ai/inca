//! PolyMesh: an n-gon polygon mesh with per-face-corner UVs and per-face materials,
//! plus the polygon modeling operations (extrude, edge loops, smooth, merge, ...).
//!
//! The JSON layout (`v`, `f`, `uv`, `fm`, `softAngle`, `edgeHard`, `displayEdges`) is the same as
//! the JavaScript app's `PolyMesh.toJSON()` so `.inca` scenes are interchangeable.

use crate::math::{Aabb, DMat4, DVec2, DVec3, Ray};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Numeric edge key as used by the JS app (a * 2^21 + b with a < b).
pub const KM: u64 = 2_097_152;
pub fn ekey(a: u32, b: u32) -> u64 {
    let (a, b) = if a < b { (a, b) } else { (b, a) };
    a as u64 * KM + b as u64
}
pub fn epair(a: u32, b: u32) -> (u32, u32) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fn default_soft() -> f64 {
    60.0
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PolyMesh {
    /// vertex positions
    pub v: Vec<DVec3>,
    /// faces: vertex indices, counter-clockwise seen from outside
    pub f: Vec<Vec<u32>>,
    /// per face: one UV per corner (or None)
    #[serde(default)]
    pub uv: Option<Vec<Option<Vec<DVec2>>>>,
    /// per face material id (None = object material)
    #[serde(default)]
    pub fm: Option<Vec<Option<String>>>,
    /// edges with dihedral angle below this are smooth-shaded
    #[serde(default = "default_soft")]
    pub soft_angle: f64,
    /// explicit hard/soft overrides keyed by `ekey` (as a string, like the JSON object keys)
    #[serde(default)]
    pub edge_hard: Option<BTreeMap<String, bool>>,
    /// optional subset of edges drawn as wireframe (NURBS isoparms)
    #[serde(default)]
    pub display_edges: Option<Vec<[u32; 2]>>,
}

/// Edge/face adjacency, computed on demand.
#[derive(Clone, Debug, Default)]
pub struct Topo {
    pub edges: Vec<(u32, u32)>,
    pub edge_map: HashMap<(u32, u32), usize>,
    pub edge_faces: Vec<Vec<usize>>,
    pub vert_faces: Vec<Vec<usize>>,
    pub vert_edges: Vec<Vec<usize>>,
}

impl Topo {
    pub fn edge(&self, a: u32, b: u32) -> Option<usize> {
        self.edge_map.get(&epair(a, b)).copied()
    }
}

/// Result of a modeling op: the new mesh and optionally a component selection to apply.
#[derive(Clone, Debug, Default)]
pub struct OpOut {
    pub mesh: PolyMesh,
    pub select_faces: Option<Vec<usize>>,
    pub select_edges: Option<Vec<(u32, u32)>>,
    pub select_verts: Option<Vec<u32>>,
}

impl From<PolyMesh> for OpOut {
    fn from(mesh: PolyMesh) -> Self {
        OpOut { mesh, ..Default::default() }
    }
}

impl PolyMesh {
    pub fn new(v: Vec<DVec3>, f: Vec<Vec<u32>>) -> Self {
        PolyMesh { v, f, soft_angle: 60.0, ..Default::default() }
    }

    pub fn is_empty(&self) -> bool {
        self.f.is_empty()
    }

    pub fn add_face(&mut self, verts: Vec<u32>, uvs: Option<Vec<DVec2>>, mat: Option<String>) -> usize {
        self.f.push(verts);
        if self.uv.is_some() || uvs.is_some() {
            let n = self.f.len() - 1;
            let uv = self.uv.get_or_insert_with(|| vec![None; n]);
            uv.resize(n, None);
            uv.push(uvs);
        }
        if self.fm.is_some() || mat.is_some() {
            let n = self.f.len() - 1;
            let fm = self.fm.get_or_insert_with(|| vec![None; n]);
            fm.resize(n, None);
            fm.push(mat);
        }
        self.f.len() - 1
    }

    pub fn face_uv(&self, fi: usize) -> Option<&Vec<DVec2>> {
        self.uv.as_ref().and_then(|u| u.get(fi)).and_then(|u| u.as_ref())
    }
    pub fn face_mat(&self, fi: usize) -> Option<&String> {
        self.fm.as_ref().and_then(|m| m.get(fi)).and_then(|m| m.as_ref())
    }

    pub fn topo(&self) -> Topo {
        let mut t = Topo { vert_faces: vec![vec![]; self.v.len()], vert_edges: vec![vec![]; self.v.len()], ..Default::default() };
        for (fi, face) in self.f.iter().enumerate() {
            let n = face.len();
            for i in 0..n {
                let a = face[i];
                let b = face[(i + 1) % n];
                if (a as usize) < t.vert_faces.len() {
                    t.vert_faces[a as usize].push(fi);
                }
                let k = epair(a, b);
                let ei = match t.edge_map.get(&k) {
                    Some(&e) => e,
                    None => {
                        let e = t.edges.len();
                        t.edges.push(k);
                        t.edge_map.insert(k, e);
                        t.edge_faces.push(vec![]);
                        if (k.0 as usize) < t.vert_edges.len() && (k.1 as usize) < t.vert_edges.len() {
                            t.vert_edges[k.0 as usize].push(e);
                            t.vert_edges[k.1 as usize].push(e);
                        }
                        e
                    }
                };
                t.edge_faces[ei].push(fi);
            }
        }
        t
    }

    pub fn face_normal(&self, fi: usize) -> DVec3 {
        newell(&self.f[fi].iter().map(|&i| self.v[i as usize]).collect::<Vec<_>>())
    }
    pub fn face_center(&self, fi: usize) -> DVec3 {
        let f = &self.f[fi];
        f.iter().map(|&i| self.v[i as usize]).sum::<DVec3>() / f.len().max(1) as f64
    }
    pub fn bbox(&self) -> Aabb {
        let mut b = Aabb::EMPTY;
        for p in &self.v {
            b.add(*p);
        }
        b
    }
    pub fn tri_count(&self) -> usize {
        self.f.iter().map(|f| f.len().saturating_sub(2)).sum()
    }
    pub fn edge_count(&self) -> usize {
        self.topo().edges.len()
    }

    /// Triangles of a face as local corner indices.
    pub fn triangulate_face(&self, fi: usize) -> Vec<[usize; 3]> {
        let f = &self.f[fi];
        let pts: Vec<DVec3> = f.iter().map(|&i| self.v[i as usize]).collect();
        triangulate(&pts)
    }

    /// Per-corner normals with smoothing groups from `soft_angle` and `edge_hard`.
    pub fn corner_normals(&self) -> Vec<Vec<DVec3>> {
        let fnorm: Vec<DVec3> = (0..self.f.len()).map(|i| self.face_normal(i).normalize_or_zero()).collect();
        let topo = self.topo();
        let cos_soft = (self.soft_angle.clamp(0.0, 180.0) * crate::math::D2R).cos();
        let hard_edge = |a: u32, b: u32| -> Option<bool> {
            self.edge_hard.as_ref().and_then(|m| m.get(&ekey(a, b).to_string()).copied())
        };
        let mut out = Vec::with_capacity(self.f.len());
        for (fi, face) in self.f.iter().enumerate() {
            let n = face.len();
            let mut cn = Vec::with_capacity(n);
            for ci in 0..n {
                let v = face[ci];
                let prev = face[(ci + n - 1) % n];
                let next = face[(ci + 1) % n];
                // walk around the vertex across soft edges, both directions
                let mut acc = fnorm[fi];
                let mut seen = HashSet::new();
                seen.insert(fi);
                for start_nb in [prev, next] {
                    let mut cur_f = fi;
                    let mut via = start_nb;
                    loop {
                        let Some(e) = topo.edge(v, via) else { break };
                        let other = topo.edge_faces[e].iter().copied().find(|&g| g != cur_f);
                        let Some(g) = other else { break };
                        if seen.contains(&g) {
                            break;
                        }
                        let soft = match hard_edge(v, via) {
                            Some(h) => !h,
                            None => fnorm[cur_f].dot(fnorm[g]) >= cos_soft - 1e-9,
                        };
                        if !soft {
                            break;
                        }
                        seen.insert(g);
                        acc += fnorm[g];
                        // next edge of g around v (the one not equal to via)
                        let gf = &self.f[g];
                        let m = gf.len();
                        let Some(pos) = gf.iter().position(|&x| x == v) else { break };
                        let a = gf[(pos + m - 1) % m];
                        let b = gf[(pos + 1) % m];
                        via = if a == via { b } else { a };
                        cur_f = g;
                    }
                }
                cn.push(acc.normalize_or_zero());
            }
            out.push(cn);
        }
        out
    }

    /// Ray hit against the mesh (object space). Returns (t, face index).
    pub fn raycast(&self, r: &Ray, cull_back: bool) -> Option<(f64, usize)> {
        let mut best: Option<(f64, usize)> = None;
        for fi in 0..self.f.len() {
            let f = &self.f[fi];
            for tri in self.triangulate_face(fi) {
                let a = self.v[f[tri[0]] as usize];
                let b = self.v[f[tri[1]] as usize];
                let c = self.v[f[tri[2]] as usize];
                if let Some(t) = crate::math::ray_triangle(r, a, b, c, cull_back) {
                    if best.is_none_or(|(bt, _)| t < bt) {
                        best = Some((t, fi));
                    }
                }
            }
        }
        best
    }

    pub fn transformed(&self, m: &DMat4) -> PolyMesh {
        let mut o = self.clone();
        for p in &mut o.v {
            *p = m.transform_point3(*p);
        }
        if m.determinant() < 0.0 {
            o = op_reverse(&o, None);
        }
        o
    }

    /// Remove unused vertices and remap faces.
    pub fn compact(&mut self) {
        let mut used = vec![false; self.v.len()];
        for f in &self.f {
            for &i in f {
                used[i as usize] = true;
            }
        }
        let mut map = vec![u32::MAX; self.v.len()];
        let mut nv = Vec::new();
        for (i, p) in self.v.iter().enumerate() {
            if used[i] {
                map[i] = nv.len() as u32;
                nv.push(*p);
            }
        }
        for f in &mut self.f {
            for i in f.iter_mut() {
                *i = map[*i as usize];
            }
        }
        if let Some(de) = &mut self.display_edges {
            de.retain(|e| map[e[0] as usize] != u32::MAX && map[e[1] as usize] != u32::MAX);
            for e in de.iter_mut() {
                *e = [map[e[0] as usize], map[e[1] as usize]];
            }
        }
        self.edge_hard = None;
        self.v = nv;
    }

    /// Keep only faces where `keep[fi]` is true.
    pub fn retain_faces(&mut self, keep: &[bool]) {
        let mut i = 0;
        self.f.retain(|_| {
            i += 1;
            keep[i - 1]
        });
        if let Some(uv) = &mut self.uv {
            let mut i = 0;
            uv.retain(|_| {
                i += 1;
                keep.get(i - 1).copied().unwrap_or(false)
            });
        }
        if let Some(fm) = &mut self.fm {
            let mut i = 0;
            fm.retain(|_| {
                i += 1;
                keep.get(i - 1).copied().unwrap_or(false)
            });
        }
    }

    /// Remove repeated consecutive vertices and faces with fewer than 3 verts.
    pub fn clean_faces(&mut self) {
        let mut keep = vec![true; self.f.len()];
        for (fi, f) in self.f.iter_mut().enumerate() {
            let mut uvs = self.uv.as_mut().and_then(|u| u.get_mut(fi)).and_then(|u| u.as_mut());
            let mut j = 0;
            while f.len() > 1 && j < f.len() {
                let n = f.len();
                if f[j] == f[(j + 1) % n] {
                    f.remove((j + 1) % n);
                    if let Some(u) = uvs.as_mut() {
                        let n2 = u.len();
                        if n2 > 0 {
                            u.remove((j + 1) % n2);
                        }
                    }
                } else {
                    j += 1;
                }
            }
            if f.len() < 3 {
                keep[fi] = false;
            }
        }
        self.retain_faces(&keep);
    }

    /// Ordered boundary loops (vertex index lists), optionally only edges passing `filter`.
    pub fn boundary_loops(&self) -> Vec<Vec<u32>> {
        let topo = self.topo();
        // directed boundary edges: a->b as they appear in their face, loop goes reversed for a cap
        let mut next: HashMap<u32, u32> = HashMap::new();
        for (ei, &(a, b)) in topo.edges.iter().enumerate() {
            if topo.edge_faces[ei].len() == 1 {
                let f = &self.f[topo.edge_faces[ei][0]];
                let n = f.len();
                let p = f.iter().position(|&x| x == a).unwrap();
                if f[(p + 1) % n] == b {
                    next.insert(b, a);
                } else {
                    next.insert(a, b);
                }
            }
        }
        let mut loops = vec![];
        let mut used = HashSet::new();
        let mut keys: Vec<u32> = next.keys().copied().collect();
        keys.sort();
        for s in keys {
            if used.contains(&s) {
                continue;
            }
            let mut l = vec![s];
            used.insert(s);
            let mut c = next[&s];
            while c != s && !used.contains(&c) {
                used.insert(c);
                l.push(c);
                match next.get(&c) {
                    Some(&n) => c = n,
                    None => break,
                }
            }
            if l.len() >= 3 {
                loops.push(l);
            }
        }
        loops
    }

    pub fn components_connected(&self) -> Vec<Vec<usize>> {
        let topo = self.topo();
        let mut seen = vec![false; self.f.len()];
        let mut groups = vec![];
        for s in 0..self.f.len() {
            if seen[s] {
                continue;
            }
            let mut stack = vec![s];
            seen[s] = true;
            let mut g = vec![];
            while let Some(fi) = stack.pop() {
                g.push(fi);
                for &v in &self.f[fi] {
                    for &nf in &topo.vert_faces[v as usize] {
                        if !seen[nf] {
                            seen[nf] = true;
                            stack.push(nf);
                        }
                    }
                }
            }
            g.sort();
            groups.push(g);
        }
        groups
    }

    pub fn signed_volume(&self) -> f64 {
        let mut vol = 0.0;
        for fi in 0..self.f.len() {
            let f = &self.f[fi];
            for t in self.triangulate_face(fi) {
                let a = self.v[f[t[0]] as usize];
                let b = self.v[f[t[1]] as usize];
                let c = self.v[f[t[2]] as usize];
                vol += a.dot(b.cross(c)) / 6.0;
            }
        }
        vol
    }
}

/// Newell's method polygon normal (not normalized).
pub fn newell(pts: &[DVec3]) -> DVec3 {
    let mut n = DVec3::ZERO;
    let k = pts.len();
    for i in 0..k {
        let a = pts[i];
        let b = pts[(i + 1) % k];
        n.x += (a.y - b.y) * (a.z + b.z);
        n.y += (a.z - b.z) * (a.x + b.x);
        n.z += (a.x - b.x) * (a.y + b.y);
    }
    n * 0.5
}

/// Triangulate a planar-ish polygon; convex quads/triangles fan, others go through earcut.
pub fn triangulate(pts: &[DVec3]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return vec![];
    }
    if n == 3 {
        return vec![[0, 1, 2]];
    }
    let nrm = newell(pts).normalize_or_zero();
    if n == 4 {
        // split along the shorter diagonal unless that makes a fold
        let d02 = pts[0].distance_squared(pts[2]);
        let d13 = pts[1].distance_squared(pts[3]);
        let ok = |a: usize, b: usize, c: usize| (pts[b] - pts[a]).cross(pts[c] - pts[a]).dot(nrm) > 0.0;
        let s02 = ok(0, 1, 2) && ok(0, 2, 3);
        let s13 = ok(1, 2, 3) && ok(1, 3, 0);
        if s02 && (d02 <= d13 || !s13) {
            return vec![[0, 1, 2], [0, 2, 3]];
        }
        if s13 {
            return vec![[1, 2, 3], [1, 3, 0]];
        }
        return vec![[0, 1, 2], [0, 2, 3]];
    }
    // project to the polygon plane
    let u = if nrm.x.abs() < 0.9 { DVec3::X.cross(nrm) } else { DVec3::Y.cross(nrm) }.normalize_or_zero();
    let w = nrm.cross(u);
    let mut flat = Vec::with_capacity(n * 2);
    for p in pts {
        flat.push(p.dot(u));
        flat.push(p.dot(w));
    }
    match earcutr::earcut(&flat, &[], 2) {
        Ok(idx) if idx.len() >= 3 => idx
            .chunks(3)
            .map(|c| {
                // keep the polygon's winding
                let (a, b, c) = (c[0], c[1], c[2]);
                if (pts[b] - pts[a]).cross(pts[c] - pts[a]).dot(nrm) < 0.0 {
                    [a, c, b]
                } else {
                    [a, b, c]
                }
            })
            .collect(),
        _ => (1..n - 1).map(|i| [0, i, i + 1]).collect(),
    }
}

// ------------------------------------------------------------------ ops

/// Move vertices by deltas (Maya's polyTweak).
pub fn op_tweak(pm: &PolyMesh, verts: &[u32], deltas: &[DVec3]) -> PolyMesh {
    let mut m = pm.clone();
    for (i, &v) in verts.iter().enumerate() {
        if let (Some(p), Some(d)) = (m.v.get_mut(v as usize), deltas.get(i)) {
            *p += *d;
        }
    }
    m
}

pub fn op_set_positions(pm: &PolyMesh, verts: &[u32], pos: &[DVec3]) -> PolyMesh {
    let mut m = pm.clone();
    for (i, &v) in verts.iter().enumerate() {
        if let (Some(p), Some(d)) = (m.v.get_mut(v as usize), pos.get(i)) {
            *p = *d;
        }
    }
    m
}

#[derive(Clone, Debug)]
pub struct ExtrudeParams {
    pub local_translate_z: f64,
    pub translate: DVec3,
    pub offset: f64,
    pub divisions: u32,
    pub keep_faces_together: bool,
}

impl Default for ExtrudeParams {
    fn default() -> Self {
        ExtrudeParams { local_translate_z: 0.0, translate: DVec3::ZERO, offset: 0.0, divisions: 1, keep_faces_together: true }
    }
}

/// Extrude faces: new side walls around each region's border; the cap moves along its normal.
pub fn op_extrude_faces(pm: &PolyMesh, faces: &[usize], p: &ExtrudeParams) -> OpOut {
    let mut m = pm.clone();
    let faces: Vec<usize> = faces.iter().copied().filter(|&f| f < pm.f.len()).collect();
    if faces.is_empty() {
        return m.into();
    }
    let regions: Vec<Vec<usize>> = if p.keep_faces_together { vec![faces.clone()] } else { faces.iter().map(|&f| vec![f]).collect() };
    let divs = p.divisions.max(1);
    for region in &regions {
        let set: HashSet<usize> = region.iter().copied().collect();
        // directed edges used by region faces
        let mut dir: HashSet<(u32, u32)> = HashSet::new();
        for &fi in region {
            let f = &m.f[fi];
            for i in 0..f.len() {
                dir.insert((f[i], f[(i + 1) % f.len()]));
            }
        }
        // vertex normals over the region (for local translate)
        let mut vn: HashMap<u32, DVec3> = HashMap::new();
        for &fi in region {
            let n = m.face_normal(fi).normalize_or_zero();
            for &v in &m.f[fi] {
                *vn.entry(v).or_default() += n;
            }
        }
        let mut region_verts: Vec<u32> = vn.keys().copied().collect();
        region_verts.sort();
        let center = region_verts.iter().map(|&v| m.v[v as usize]).sum::<DVec3>() / region_verts.len().max(1) as f64;
        // border directed edges (whose reverse isn't in the region)
        let mut border: Vec<(u32, u32, usize)> = vec![];
        for &fi in region {
            let f = &m.f[fi];
            for i in 0..f.len() {
                let (a, b) = (f[i], f[(i + 1) % f.len()]);
                if !dir.contains(&(b, a)) {
                    border.push((a, b, fi));
                }
            }
        }
        let _ = &set;
        // build rings of new vertices, one per division
        let mut prev: HashMap<u32, u32> = region_verts.iter().map(|&v| (v, v)).collect();
        for d in 1..=divs {
            let t = d as f64 / divs as f64;
            let mut ring: HashMap<u32, u32> = HashMap::new();
            for &v in &region_verts {
                let base = pm.v[v as usize];
                let n = vn[&v].normalize_or_zero();
                let off = if p.offset != 0.0 { (center - base).normalize_or_zero() * p.offset * t } else { DVec3::ZERO };
                let np = base + n * p.local_translate_z * t + p.translate * t + off;
                ring.insert(v, m.v.len() as u32);
                m.v.push(np);
            }
            for &(a, b, fi) in &border {
                let uv = m.face_uv(fi).map(|u| {
                    let f = &pm.f[fi];
                    let ia = f.iter().position(|&x| x == a).unwrap();
                    let ib = f.iter().position(|&x| x == b).unwrap();
                    vec![u[ia], u[ib], u[ib], u[ia]]
                });
                let mat = m.face_mat(fi).cloned();
                m.add_face(vec![prev[&a], prev[&b], ring[&b], ring[&a]], uv, mat);
            }
            prev = ring;
        }
        for &fi in region {
            for v in m.f[fi].iter_mut() {
                *v = prev[v];
            }
        }
    }
    OpOut { mesh: m, select_faces: Some(faces), ..Default::default() }
}

/// Extrude border edges outwards (new quads attached to the edges).
pub fn op_extrude_edges(pm: &PolyMesh, edges: &[(u32, u32)], offset: f64, translate: DVec3) -> OpOut {
    let mut m = pm.clone();
    let topo = pm.topo();
    let mut newv: HashMap<u32, u32> = HashMap::new();
    let mut out_edges = vec![];
    for &(a, b) in edges {
        let Some(ei) = topo.edge(a, b) else { continue };
        let fi = topo.edge_faces[ei].first().copied();
        // orient (a,b) opposite to its face so the new quad has consistent winding
        let (a, b) = match fi {
            Some(fi) => {
                let f = &pm.f[fi];
                let p = f.iter().position(|&x| x == a).unwrap();
                if f[(p + 1) % f.len()] == b {
                    (b, a)
                } else {
                    (a, b)
                }
            }
            None => (a, b),
        };
        let push_dir = fi
            .map(|fi| {
                let n = pm.face_normal(fi).normalize_or_zero();
                let e = pm.v[b as usize] - pm.v[a as usize];
                // outward in the face plane
                e.cross(n).normalize_or_zero()
            })
            .unwrap_or(DVec3::Y);
        for v in [a, b] {
            newv.entry(v).or_insert_with(|| {
                m.v.push(pm.v[v as usize] + push_dir * offset + translate);
                (m.v.len() - 1) as u32
            });
        }
        m.add_face(vec![a, b, newv[&b], newv[&a]], None, None);
        out_edges.push((newv[&a], newv[&b]));
    }
    OpOut { mesh: m, select_edges: Some(out_edges), ..Default::default() }
}

/// Bevel (chamfer) edges with one segment. `fraction` is the bevel width relative to half of the
/// shortest edge at each end.
pub fn op_bevel(pm: &PolyMesh, edges: &[(u32, u32)], fraction: f64) -> OpOut {
    let topo = pm.topo();
    let bev: HashSet<(u32, u32)> = edges.iter().map(|&(a, b)| epair(a, b)).filter(|e| topo.edge(e.0, e.1).is_some_and(|i| topo.edge_faces[i].len() == 2)).collect();
    if bev.is_empty() {
        return pm.clone().into();
    }
    let fraction = fraction.clamp(0.001, 1.0);
    let touched: HashSet<u32> = bev.iter().flat_map(|&(a, b)| [a, b]).collect();
    // bevel width per vertex
    let width = |v: u32| -> f64 {
        let m = topo.vert_edges[v as usize].iter().map(|&e| {
            let (a, b) = topo.edges[e];
            pm.v[a as usize].distance(pm.v[b as usize])
        }).fold(f64::INFINITY, f64::min);
        fraction * 0.5 * m
    };
    // Maya's polyBevel smoothing angle: bevels read as facets
    let mut out = PolyMesh { v: pm.v.clone(), soft_angle: pm.soft_angle.min(30.0), ..Default::default() };
    // points on non-bevelled edges, shared by the faces on both sides
    let mut on_edge: HashMap<(u32, u32), u32> = HashMap::new();
    let mut edge_pt = |out: &mut PolyMesh, v: u32, x: u32| -> u32 {
        *on_edge.entry((v, x)).or_insert_with(|| {
            let d = (pm.v[x as usize] - pm.v[v as usize]).normalize_or_zero();
            out.v.push(pm.v[v as usize] + d * width(v));
            (out.v.len() - 1) as u32
        })
    };
    let mut new_verts: HashSet<u32> = HashSet::new();
    // corner vertex next to a bevelled edge, per (face, vertex)
    let mut corner: HashMap<(usize, u32), u32> = HashMap::new();
    let is_bev = |a: u32, b: u32| bev.contains(&epair(a, b));
    for (fi, f) in pm.f.iter().enumerate() {
        let n = f.len();
        let mut nf = vec![];
        let mut nuv = vec![];
        let fuv = pm.face_uv(fi);
        for i in 0..n {
            let v = f[i];
            let p = f[(i + n - 1) % n];
            let x = f[(i + 1) % n];
            let uvc = fuv.map(|u| u[i]);
            if !touched.contains(&v) {
                nf.push(v);
                nuv.push(uvc);
                continue;
            }
            let (bin, bout) = (is_bev(p, v), is_bev(v, x));
            match (bin, bout) {
                (true, true) => {
                    let d = (pm.v[p as usize] - pm.v[v as usize]).normalize_or_zero() + (pm.v[x as usize] - pm.v[v as usize]).normalize_or_zero();
                    out.v.push(pm.v[v as usize] + d * width(v));
                    let id = (out.v.len() - 1) as u32;
                    corner.insert((fi, v), id);
                    new_verts.insert(id);
                    nf.push(id);
                    nuv.push(uvc);
                }
                (true, false) => {
                    let id = edge_pt(&mut out, v, x);
                    corner.insert((fi, v), id);
                    new_verts.insert(id);
                    nf.push(id);
                    nuv.push(uvc);
                }
                (false, true) => {
                    let id = edge_pt(&mut out, v, p);
                    corner.insert((fi, v), id);
                    new_verts.insert(id);
                    nf.push(id);
                    nuv.push(uvc);
                }
                (false, false) => {
                    let a = edge_pt(&mut out, v, p);
                    let b = edge_pt(&mut out, v, x);
                    new_verts.insert(a);
                    new_verts.insert(b);
                    nf.push(a);
                    nf.push(b);
                    nuv.push(uvc);
                    nuv.push(uvc);
                }
            }
        }
        let uv = if fuv.is_some() { Some(nuv.into_iter().map(|u| u.unwrap_or_default()).collect()) } else { None };
        out.add_face(nf, uv, pm.face_mat(fi).cloned());
    }
    // bevel faces along each edge
    let mut sel = vec![];
    for &(a, b) in &bev {
        let e = topo.edge(a, b).unwrap();
        let (f1, f2) = (topo.edge_faces[e][0], topo.edge_faces[e][1]);
        // orient so f1 runs a -> b
        let f1_ab = {
            let f = &pm.f[f1];
            let i = f.iter().position(|&x| x == a).unwrap();
            f[(i + 1) % f.len()] == b
        };
        let (f1, f2) = if f1_ab { (f1, f2) } else { (f2, f1) };
        let (c1a, c1b, c2a, c2b) = (corner[&(f1, a)], corner[&(f1, b)], corner[&(f2, a)], corner[&(f2, b)]);
        sel.push(out.add_face(vec![c1b, c1a, c2a, c2b], None, pm.face_mat(f1).cloned()));
    }
    // fill the holes left at vertices where several bevelled edges meet
    for l in out.boundary_loops() {
        if l.iter().all(|v| new_verts.contains(v)) {
            sel.push(out.add_face(l, None, None));
        }
    }
    out.compact();
    OpOut { mesh: out, select_faces: Some(sel), ..Default::default() }
}

/// Walk the edge ring (across quads) starting at edge (a,b). Returns directed ring edges,
/// all oriented the same way as (a,b).
pub fn edge_ring(pm: &PolyMesh, topo: &Topo, a: u32, b: u32) -> Vec<(u32, u32)> {
    let Some(e0) = topo.edge(a, b) else { return vec![(a, b)] };
    let mut seen_e = HashSet::new();
    seen_e.insert(epair(a, b));
    let mut sides: Vec<Vec<(u32, u32)>> = vec![];
    for &start_f in topo.edge_faces[e0].iter().take(2) {
        let mut cur = (a, b);
        let mut f = start_f;
        let mut side = vec![];
        loop {
            let face = &pm.f[f];
            if face.len() != 4 {
                break;
            }
            let i = face.iter().position(|&x| x == cur.0).unwrap();
            let j = face.iter().position(|&x| x == cur.1).unwrap();
            let (oa, ob) = if (i + 1) % 4 == j { (face[(i + 3) % 4], face[(j + 1) % 4]) } else { (face[(i + 1) % 4], face[(j + 3) % 4]) };
            if !seen_e.insert(epair(oa, ob)) {
                break;
            }
            side.push((oa, ob));
            let Some(oe) = topo.edge(oa, ob) else { break };
            match topo.edge_faces[oe].iter().copied().find(|&g| g != f) {
                Some(g) => {
                    f = g;
                    cur = (oa, ob);
                }
                None => break,
            }
        }
        sides.push(side);
    }
    let mut ring = vec![];
    if let Some(s) = sides.get(1) {
        ring.extend(s.iter().rev().copied());
    }
    ring.push((a, b));
    if let Some(s) = sides.first() {
        ring.extend(s.iter().copied());
    }
    ring
}

/// Edge loop through the edge (a,b): continue through 4-valent vertices.
pub fn edge_loop(_pm: &PolyMesh, topo: &Topo, a: u32, b: u32) -> Vec<(u32, u32)> {
    let mut out = vec![epair(a, b)];
    let mut seen: HashSet<(u32, u32)> = out.iter().copied().collect();
    // a border edge: follow the connected border (an open rim's vertices have three edges)
    if topo.edge(a, b).is_some_and(|e| topo.edge_faces[e].len() == 1) {
        for (from, to) in [(a, b), (b, a)] {
            let (mut p, mut c) = (from, to);
            loop {
                let borders: Vec<u32> = topo.vert_edges[c as usize]
                    .iter()
                    .filter(|&&e| topo.edge_faces[e].len() == 1)
                    .map(|&e| {
                        let (x, y) = topo.edges[e];
                        if x == c { y } else { x }
                    })
                    .filter(|&n| n != p)
                    .collect();
                // stop where the border branches (two openings touching) or ends
                let [n] = borders[..] else { break };
                if !seen.insert(epair(c, n)) {
                    break;
                }
                out.push(epair(c, n));
                p = c;
                c = n;
            }
        }
        return out;
    }
    for (from, to) in [(a, b), (b, a)] {
        let (mut p, mut c) = (from, to);
        loop {
            let ve = &topo.vert_edges[c as usize];
            if ve.len() != 4 {
                break;
            }
            // faces adjacent to current edge
            let Some(ce) = topo.edge(p, c) else { break };
            let cf: HashSet<usize> = topo.edge_faces[ce].iter().copied().collect();
            // next edge: shares no face with the current edge
            let mut nxt = None;
            for &e in ve {
                let (x, y) = topo.edges[e];
                if e == ce {
                    continue;
                }
                if topo.edge_faces[e].iter().all(|f| !cf.contains(f)) {
                    nxt = Some(if x == c { y } else { x });
                }
            }
            let Some(n) = nxt else { break };
            if !seen.insert(epair(c, n)) {
                break;
            }
            out.push(epair(c, n));
            p = c;
            c = n;
        }
    }
    out
}

/// Insert edge loop(s) across the ring of edge (a,b) at weight t.
pub fn op_insert_edge_loop(pm: &PolyMesh, a: u32, b: u32, t: f64, multiple: u32) -> OpOut {
    let topo = pm.topo();
    let ring = edge_ring(pm, &topo, a, b);
    let mut m = pm.clone();
    let k = multiple.max(1) as usize;
    // new verts per ring edge, keyed by directed edge
    let mut splits: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
    for &(x, y) in &ring {
        let mut ids = vec![];
        for i in 0..k {
            let w = if k == 1 { t } else { (i + 1) as f64 / (k + 1) as f64 };
            m.v.push(pm.v[x as usize].lerp(pm.v[y as usize], w));
            ids.push((m.v.len() - 1) as u32);
        }
        splits.insert((x, y), ids.clone());
        let mut r = ids;
        r.reverse();
        splits.insert((y, x), r);
    }
    let ring_set: HashSet<(u32, u32)> = ring.iter().map(|&(x, y)| epair(x, y)).collect();
    let mut new_edges = vec![];
    let nf = m.f.len();
    let mut keep = vec![true; nf];
    for fi in 0..nf {
        let face = m.f[fi].clone();
        if face.len() != 4 {
            // split ring edges in non-quads by inserting the vertices
            let mut nfv = vec![];
            let mut changed = false;
            for i in 0..face.len() {
                let (x, y) = (face[i], face[(i + 1) % face.len()]);
                nfv.push(x);
                if ring_set.contains(&epair(x, y)) {
                    nfv.extend(splits[&(x, y)].iter().copied());
                    changed = true;
                }
            }
            if changed {
                m.f[fi] = nfv;
                if let Some(uv) = &mut m.uv {
                    uv[fi] = None;
                }
            }
            continue;
        }
        // which edges of the quad are in the ring?
        let hits: Vec<usize> = (0..4).filter(|&i| ring_set.contains(&epair(face[i], face[(i + 1) % 4]))).collect();
        if hits.len() != 2 || (hits[1] - hits[0]) != 2 {
            continue;
        }
        let i0 = hits[0];
        let (p0, p1, p2, p3) = (face[i0], face[(i0 + 1) % 4], face[(i0 + 2) % 4], face[(i0 + 3) % 4]);
        let s01 = &splits[&(p0, p1)];
        let s23 = &splits[&(p2, p3)];
        let uvs = m.face_uv(fi).cloned();
        let mat = m.face_mat(fi).cloned();
        // column strips: p0 .. s01[i] .. p1 and p3 .. s23 reversed .. p2
        let mut left: Vec<u32> = vec![p0];
        left.extend(s01.iter().copied());
        left.push(p1);
        let mut right: Vec<u32> = vec![p3];
        right.extend(s23.iter().rev().copied());
        right.push(p2);
        let luv: Option<Vec<DVec2>> = uvs.as_ref().map(|u| {
            let (a0, a1) = (u[i0], u[(i0 + 1) % 4]);
            (0..=k + 1).map(|i| a0.lerp(a1, if i == 0 { 0.0 } else if i == k + 1 { 1.0 } else if k == 1 { t } else { i as f64 / (k + 1) as f64 })).collect()
        });
        let ruv: Option<Vec<DVec2>> = uvs.as_ref().map(|u| {
            let (a0, a1) = (u[(i0 + 3) % 4], u[(i0 + 2) % 4]);
            (0..=k + 1).map(|i| a0.lerp(a1, if i == 0 { 0.0 } else if i == k + 1 { 1.0 } else if k == 1 { t } else { i as f64 / (k + 1) as f64 })).collect()
        });
        keep[fi] = false;
        for i in 0..=k {
            let quad = vec![left[i], left[i + 1], right[i + 1], right[i]];
            let quv = match (&luv, &ruv) {
                (Some(l), Some(r)) => Some(vec![l[i], l[i + 1], r[i + 1], r[i]]),
                _ => None,
            };
            m.add_face(quad, quv, mat.clone());
            if i < k {
                new_edges.push(epair(left[i + 1], right[i + 1]));
            }
        }
    }
    keep.resize(m.f.len(), true);
    m.retain_faces(&keep);
    OpOut { mesh: m, select_edges: Some(new_edges), ..Default::default() }
}

/// Chamfer Vertices: replace each selected vertex by a face cut `width` along its edges (JS opChamferVertex).
pub fn op_chamfer_vertex(pm: &PolyMesh, verts: &[u32], width: f64) -> OpOut {
    let sel: HashSet<u32> = verts.iter().copied().collect();
    let mut m = pm.clone();
    let mut cache: HashMap<(u32, u32), u32> = HashMap::new();
    let mut cut = |m: &mut PolyMesh, v: u32, o: u32| -> u32 {
        *cache.entry((v, o)).or_insert_with(|| {
            let (pv, po) = (pm.v[v as usize], pm.v[o as usize]);
            let l = (po - pv).length().max(1e-12);
            m.v.push(pv + (po - pv) * (width.min(0.49 * l) / l));
            (m.v.len() - 1) as u32
        })
    };
    // per selected vertex: cap edge from the cut toward `next` back to the cut toward `prev`
    let mut cap_next: HashMap<u32, HashMap<u32, u32>> = HashMap::new();
    for fi in 0..pm.f.len() {
        let face = pm.f[fi].clone();
        let n = face.len();
        if !face.iter().any(|v| sel.contains(v)) {
            continue;
        }
        let uv = pm.face_uv(fi).cloned();
        let mut out = vec![];
        let mut out_uv = vec![];
        for i in 0..n {
            let v = face[i];
            if !sel.contains(&v) {
                out.push(v);
                if let Some(u) = &uv {
                    out_uv.push(u[i]);
                }
                continue;
            }
            let (prev, next) = (face[(i + n - 1) % n], face[(i + 1) % n]);
            let a = cut(&mut m, v, prev);
            let b = cut(&mut m, v, next);
            out.push(a);
            out.push(b);
            if let Some(u) = &uv {
                let frac = |o: u32| {
                    let l = (pm.v[o as usize] - pm.v[v as usize]).length().max(1e-12);
                    width.min(0.49 * l) / l
                };
                out_uv.push(u[i].lerp(u[(i + n - 1) % n], frac(prev)));
                out_uv.push(u[i].lerp(u[(i + 1) % n], frac(next)));
            }
            cap_next.entry(v).or_default().insert(b, a);
        }
        m.f[fi] = out;
        if let (Some(all), Some(_)) = (&mut m.uv, &uv) {
            all[fi] = Some(out_uv);
        }
    }
    let mut new_faces = vec![];
    let mut sel_sorted: Vec<u32> = sel.iter().copied().collect();
    sel_sorted.sort();
    for v in sel_sorted {
        let Some(next) = cap_next.get(&v) else { continue };
        let Some(&start) = next.keys().min() else { continue };
        let mut lp = vec![start];
        let mut c = start;
        let closed = loop {
            match next.get(&c) {
                Some(&n) if n == start => break true,
                Some(&n) if !lp.contains(&n) => {
                    lp.push(n);
                    c = n;
                }
                _ => break false,
            }
        };
        // vertices on an open border keep the gap, like the JS boundary loops
        if closed && lp.len() >= 3 {
            new_faces.push(m.add_face(lp, None, None));
        }
    }
    m.compact();
    OpOut { mesh: m, select_faces: Some(new_faces), ..Default::default() }
}

/// A point the Connect tool cuts through: an existing vertex or a parameter along an edge.
#[derive(Clone, Copy, Debug)]
pub enum SplitPoint {
    Vertex(u32),
    Edge(u32, u32, f64),
}

/// Split a face between two of its vertices; false when they are neighbours.
fn split_face(m: &mut PolyMesh, fi: usize, u: u32, w: u32) -> bool {
    let f = m.f[fi].clone();
    let n = f.len();
    let (Some(iu), Some(iw)) = (f.iter().position(|&x| x == u), f.iter().position(|&x| x == w)) else { return false };
    if (iu + 1) % n == iw || (iw + 1) % n == iu {
        return false;
    }
    let walk = |a: usize, b: usize| -> Vec<usize> {
        let mut out = vec![a];
        let mut i = a;
        while i != b {
            i = (i + 1) % n;
            out.push(i);
        }
        out
    };
    let (c1, c2) = (walk(iu, iw), walk(iw, iu));
    let uv = m.face_uv(fi).cloned();
    let mat = m.face_mat(fi).cloned();
    m.f[fi] = c1.iter().map(|&i| f[i]).collect();
    if let (Some(all), Some(u)) = (&mut m.uv, &uv) {
        all[fi] = Some(c1.iter().map(|&i| u[i]).collect());
    }
    m.add_face(c2.iter().map(|&i| f[i]).collect(), uv.as_ref().map(|u| c2.iter().map(|&i| u[i]).collect()), mat);
    true
}

/// Connect: cut edges at the given points and join consecutive points across their shared faces (JS opSplit).
pub fn op_split(pm: &PolyMesh, points: &[SplitPoint]) -> OpOut {
    let mut m = pm.clone();
    let mut made: HashMap<(u32, u32), u32> = HashMap::new();
    let mut ids = vec![];
    for p in points {
        match *p {
            SplitPoint::Vertex(v) => ids.push(v),
            SplitPoint::Edge(a, b, t) => {
                let key = epair(a, b);
                let t = if a < b { t } else { 1.0 - t };
                let id = *made.entry(key).or_insert_with(|| {
                    m.v.push(pm.v[key.0 as usize].lerp(pm.v[key.1 as usize], t));
                    let nv = (m.v.len() - 1) as u32;
                    // insert into every face using the edge
                    for fi in 0..m.f.len() {
                        let f = m.f[fi].clone();
                        let n = f.len();
                        for i in 0..n {
                            let (x, y) = (f[i], f[(i + 1) % n]);
                            if epair(x, y) == key {
                                let mut nf = f.clone();
                                nf.insert(i + 1, nv);
                                m.f[fi] = nf;
                                if let Some(all) = &mut m.uv {
                                    if let Some(Some(u)) = all.get_mut(fi) {
                                        let w = if x == key.0 { t } else { 1.0 - t };
                                        let nu = u[i].lerp(u[(i + 1) % n], w);
                                        u.insert(i + 1, nu);
                                    }
                                }
                                break;
                            }
                        }
                    }
                    nv
                });
                ids.push(id);
            }
        }
    }
    let mut pairs = vec![];
    for w in ids.windows(2) {
        let (u, v) = (w[0], w[1]);
        if u == v {
            continue;
        }
        if let Some(fi) = (0..m.f.len()).find(|&fi| m.f[fi].contains(&u) && m.f[fi].contains(&v)) {
            if split_face(&mut m, fi, u, v) {
                pairs.push(epair(u, v));
            }
        }
    }
    OpOut { mesh: m, select_edges: Some(pairs), ..Default::default() }
}

pub fn op_delete_faces(pm: &PolyMesh, faces: &[usize]) -> PolyMesh {
    let mut m = pm.clone();
    let del: HashSet<usize> = faces.iter().copied().collect();
    let keep: Vec<bool> = (0..m.f.len()).map(|i| !del.contains(&i)).collect();
    m.retain_faces(&keep);
    m.compact();
    m
}

pub fn op_delete_vertices(pm: &PolyMesh, verts: &[u32]) -> PolyMesh {
    let del: HashSet<u32> = verts.iter().copied().collect();
    let faces: Vec<usize> = (0..pm.f.len()).filter(|&i| pm.f[i].iter().any(|v| del.contains(v))).collect();
    op_delete_faces(pm, &faces)
}

/// Delete edges by merging the faces on either side.
pub fn op_delete_edges(pm: &PolyMesh, edges: &[(u32, u32)]) -> PolyMesh {
    let mut m = pm.clone();
    for &(a, b) in edges {
        let topo = m.topo();
        let Some(e) = topo.edge(a, b) else { continue };
        let ef = &topo.edge_faces[e];
        if ef.len() != 2 {
            continue;
        }
        let (f1, f2) = (ef[0], ef[1]);
        let fa = m.f[f1].clone();
        let fb = m.f[f2].clone();
        // fa has a->b or b->a; rotate so the shared edge is last
        let rot = |f: &Vec<u32>, x: u32, y: u32| -> Option<Vec<u32>> {
            let n = f.len();
            let i = f.iter().position(|&v| v == x)?;
            if f[(i + 1) % n] != y {
                return None;
            }
            Some((0..n).map(|k| f[(i + 1 + k) % n]).collect())
        };
        let (ra, rb) = match (rot(&fa, a, b), rot(&fb, b, a)) {
            (Some(ra), Some(rb)) => (ra, rb),
            _ => match (rot(&fa, b, a), rot(&fb, a, b)) {
                (Some(ra), Some(rb)) => (ra, rb),
                _ => continue,
            },
        };
        // ra = [y ... x], rb = [x ... y]
        let mut merged = ra.clone();
        merged.extend(rb[1..rb.len() - 1].iter().copied());
        m.f[f1] = merged;
        if let Some(uv) = &mut m.uv {
            uv[f1] = None;
        }
        let keep: Vec<bool> = (0..m.f.len()).map(|i| i != f2).collect();
        m.retain_faces(&keep);
    }
    m.clean_faces();
    // remove 2-valent verts left on straight lines
    m.compact();
    m
}

/// Weld vertices closer than `threshold` (only among `verts` if given).
pub fn op_merge_vertices(pm: &PolyMesh, verts: Option<&[u32]>, threshold: f64) -> PolyMesh {
    let mut m = pm.clone();
    let cand: Vec<u32> = match verts {
        Some(v) => v.to_vec(),
        None => (0..pm.v.len() as u32).collect(),
    };
    let mut map: Vec<u32> = (0..pm.v.len() as u32).collect();
    let cell = threshold.max(1e-9);
    let mut grid: HashMap<(i64, i64, i64), Vec<u32>> = HashMap::new();
    for &v in &cand {
        let p = pm.v[v as usize];
        let k = ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64, (p.z / cell).floor() as i64);
        let mut found = None;
        'o: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(l) = grid.get(&(k.0 + dx, k.1 + dy, k.2 + dz)) {
                        for &w in l {
                            if pm.v[w as usize].distance(p) <= threshold {
                                found = Some(w);
                                break 'o;
                            }
                        }
                    }
                }
            }
        }
        match found {
            Some(w) => map[v as usize] = w,
            None => grid.entry(k).or_default().push(v),
        }
    }
    for f in &mut m.f {
        for i in f.iter_mut() {
            *i = map[*i as usize];
        }
    }
    m.clean_faces();
    m.compact();
    m
}

/// Collapse each group of vertices to its centre.
pub fn op_merge_to_center(pm: &PolyMesh, groups: &[Vec<u32>]) -> PolyMesh {
    let mut m = pm.clone();
    let mut map: Vec<u32> = (0..pm.v.len() as u32).collect();
    for g in groups {
        if g.is_empty() {
            continue;
        }
        let c = g.iter().map(|&v| pm.v[v as usize]).sum::<DVec3>() / g.len() as f64;
        m.v[g[0] as usize] = c;
        for &v in g {
            map[v as usize] = g[0];
        }
    }
    for f in &mut m.f {
        for i in f.iter_mut() {
            *i = map[*i as usize];
        }
    }
    m.clean_faces();
    m.compact();
    m
}

/// Catmull-Clark subdivision (`smooth`), or linear face splitting (`!smooth`, Add Divisions).
pub fn subdivide(pm: &PolyMesh, levels: u32, smooth: bool) -> PolyMesh {
    let mut m = pm.clone();
    for _ in 0..levels {
        m = subdivide_once(&m, smooth);
    }
    m
}

fn subdivide_once(pm: &PolyMesh, smooth: bool) -> PolyMesh {
    let topo = pm.topo();
    let nv = pm.v.len();
    let ne = topo.edges.len();
    let fpts: Vec<DVec3> = (0..pm.f.len()).map(|i| pm.face_center(i)).collect();
    let boundary_e: Vec<bool> = topo.edge_faces.iter().map(|f| f.len() != 2).collect();
    let epts: Vec<DVec3> = (0..ne)
        .map(|e| {
            let (a, b) = topo.edges[e];
            let mid = (pm.v[a as usize] + pm.v[b as usize]) * 0.5;
            if !smooth || boundary_e[e] {
                mid
            } else {
                let fs = &topo.edge_faces[e];
                (pm.v[a as usize] + pm.v[b as usize] + fpts[fs[0]] + fpts[fs[1]]) * 0.25
            }
        })
        .collect();
    let vpts: Vec<DVec3> = (0..nv)
        .map(|v| {
            let p = pm.v[v];
            if !smooth {
                return p;
            }
            let ve = &topo.vert_edges[v];
            let vf = &topo.vert_faces[v];
            if ve.is_empty() {
                return p;
            }
            let bes: Vec<usize> = ve.iter().copied().filter(|&e| boundary_e[e]).collect();
            if !bes.is_empty() {
                if bes.len() == 2 {
                    let o = |e: usize| {
                        let (a, b) = topo.edges[e];
                        pm.v[if a as usize == v { b } else { a } as usize]
                    };
                    return p * 0.75 + (o(bes[0]) + o(bes[1])) * 0.125;
                }
                return p;
            }
            let n = ve.len() as f64;
            let f_avg = vf.iter().map(|&f| fpts[f]).sum::<DVec3>() / vf.len() as f64;
            let e_avg = ve
                .iter()
                .map(|&e| {
                    let (a, b) = topo.edges[e];
                    (pm.v[a as usize] + pm.v[b as usize]) * 0.5
                })
                .sum::<DVec3>()
                / n;
            (f_avg + e_avg * 2.0 + p * (n - 3.0)) / n
        })
        .collect();
    let mut out = PolyMesh { v: vpts, soft_angle: pm.soft_angle, ..Default::default() };
    out.v.extend(epts.iter().copied());
    let fbase = out.v.len();
    out.v.extend(fpts.iter().copied());
    let has_uv = pm.uv.is_some();
    for (fi, face) in pm.f.iter().enumerate() {
        let n = face.len();
        let fc = (fbase + fi) as u32;
        let fuv = pm.face_uv(fi);
        let cuv = fuv.map(|u| u.iter().copied().sum::<DVec2>() / n as f64);
        let mat = pm.face_mat(fi).cloned();
        for i in 0..n {
            let v = face[i];
            let next = face[(i + 1) % n];
            let prev = face[(i + n - 1) % n];
            let e_next = (nv + topo.edge(v, next).unwrap()) as u32;
            let e_prev = (nv + topo.edge(prev, v).unwrap()) as u32;
            let uvs = match (fuv, cuv) {
                (Some(u), Some(c)) => Some(vec![u[i], (u[i] + u[(i + 1) % n]) * 0.5, c, (u[i] + u[(i + n - 1) % n]) * 0.5]),
                _ => None,
            };
            out.add_face(vec![v, e_next, fc, e_prev], if has_uv { uvs } else { None }, mat.clone());
        }
    }
    out
}

pub fn op_triangulate(pm: &PolyMesh, faces: Option<&[usize]>) -> PolyMesh {
    let sel: Option<HashSet<usize>> = faces.map(|f| f.iter().copied().collect());
    let mut out = PolyMesh { v: pm.v.clone(), soft_angle: pm.soft_angle, edge_hard: pm.edge_hard.clone(), ..Default::default() };
    for fi in 0..pm.f.len() {
        let f = &pm.f[fi];
        let uv = pm.face_uv(fi);
        let mat = pm.face_mat(fi).cloned();
        if sel.as_ref().is_some_and(|s| !s.contains(&fi)) || f.len() == 3 {
            out.add_face(f.clone(), uv.cloned(), mat);
            continue;
        }
        for t in pm.triangulate_face(fi) {
            out.add_face(vec![f[t[0]], f[t[1]], f[t[2]]], uv.map(|u| vec![u[t[0]], u[t[1]], u[t[2]]]), mat.clone());
        }
    }
    out
}

/// Merge pairs of adjacent triangles into quads where they are nearly coplanar.
pub fn op_quadrangulate(pm: &PolyMesh, angle: f64) -> PolyMesh {
    let topo = pm.topo();
    let cosl = (angle * crate::math::D2R).cos();
    let mut used = vec![false; pm.f.len()];
    let mut out = PolyMesh { v: pm.v.clone(), soft_angle: pm.soft_angle, ..Default::default() };
    for fi in 0..pm.f.len() {
        if used[fi] {
            continue;
        }
        let f = &pm.f[fi];
        if f.len() == 3 {
            let n1 = pm.face_normal(fi).normalize_or_zero();
            let mut best: Option<(usize, usize, f64)> = None;
            for i in 0..3 {
                let (a, b) = (f[i], f[(i + 1) % 3]);
                let e = topo.edge(a, b).unwrap();
                for &g in &topo.edge_faces[e] {
                    if g == fi || used[g] || pm.f[g].len() != 3 || pm.face_mat(g) != pm.face_mat(fi) {
                        continue;
                    }
                    let d = n1.dot(pm.face_normal(g).normalize_or_zero());
                    if d >= cosl && best.is_none_or(|b| d > b.2) {
                        best = Some((g, i, d));
                    }
                }
            }
            if let Some((g, i, _)) = best {
                let (a, b) = (f[i], f[(i + 1) % 3]);
                let c = f[(i + 2) % 3];
                let d = *pm.f[g].iter().find(|&&x| x != a && x != b).unwrap();
                used[fi] = true;
                used[g] = true;
                out.add_face(vec![a, d, b, c], None, pm.face_mat(fi).cloned());
                continue;
            }
        }
        used[fi] = true;
        out.add_face(f.clone(), pm.face_uv(fi).cloned(), pm.face_mat(fi).cloned());
    }
    out
}

pub fn op_reverse(pm: &PolyMesh, faces: Option<&[usize]>) -> PolyMesh {
    let mut m = pm.clone();
    let sel: Option<HashSet<usize>> = faces.map(|f| f.iter().copied().collect());
    for fi in 0..m.f.len() {
        if sel.as_ref().is_some_and(|s| !s.contains(&fi)) {
            continue;
        }
        m.f[fi].reverse();
        if let Some(Some(u)) = m.uv.as_mut().map(|u| &mut u[fi]) {
            u.reverse();
        }
    }
    m
}

pub fn op_poke(pm: &PolyMesh, faces: &[usize]) -> OpOut {
    let mut m = pm.clone();
    let set: HashSet<usize> = faces.iter().copied().collect();
    let mut keep = vec![true; pm.f.len()];
    let mut sel = vec![];
    for &fi in faces {
        if fi >= pm.f.len() {
            continue;
        }
        let f = pm.f[fi].clone();
        m.v.push(pm.face_center(fi));
        let c = (m.v.len() - 1) as u32;
        let uv = pm.face_uv(fi).cloned();
        let cuv = uv.as_ref().map(|u| u.iter().copied().sum::<DVec2>() / u.len() as f64);
        keep[fi] = false;
        for i in 0..f.len() {
            let j = (i + 1) % f.len();
            let tuv = match (&uv, cuv) {
                (Some(u), Some(c)) => Some(vec![u[i], u[j], c]),
                _ => None,
            };
            sel.push(m.add_face(vec![f[i], f[j], c], tuv, pm.face_mat(fi).cloned()));
        }
    }
    let _ = set;
    keep.resize(m.f.len(), true);
    // remap selection after retain
    let removed_before = |i: usize| keep[..i].iter().filter(|k| !**k).count();
    let sel: Vec<usize> = sel.iter().map(|&i| i - removed_before(i)).collect();
    m.retain_faces(&keep);
    OpOut { mesh: m, select_faces: Some(sel), ..Default::default() }
}

/// Fill boundary holes (all, or those touching `edges`).
pub fn op_fill_hole(pm: &PolyMesh, edges: Option<&[(u32, u32)]>) -> PolyMesh {
    let mut m = pm.clone();
    let want: Option<HashSet<(u32, u32)>> = edges.map(|e| e.iter().map(|&(a, b)| epair(a, b)).collect());
    for l in pm.boundary_loops() {
        if let Some(w) = &want {
            let n = l.len();
            if !(0..n).any(|i| w.contains(&epair(l[i], l[(i + 1) % n]))) {
                continue;
            }
        }
        m.add_face(l, None, None);
    }
    m
}

/// Mirror across a plane perpendicular to `axis` at `position`, optionally welding the seam.
pub fn op_mirror(pm: &PolyMesh, axis: usize, position: f64, merge: bool, threshold: f64) -> PolyMesh {
    let mut m = pm.clone();
    let base = pm.v.len() as u32;
    for p in &pm.v {
        let mut q = *p;
        q[axis] = 2.0 * position - q[axis];
        m.v.push(q);
    }
    for fi in 0..pm.f.len() {
        let mut f: Vec<u32> = pm.f[fi].iter().map(|&i| i + base).collect();
        f.reverse();
        let uv = pm.face_uv(fi).map(|u| {
            let mut u = u.clone();
            u.reverse();
            u
        });
        m.add_face(f, uv, pm.face_mat(fi).cloned());
    }
    if merge {
        let seam: Vec<u32> = (0..m.v.len() as u32).filter(|&i| (m.v[i as usize][axis] - position).abs() <= threshold.max(1e-6)).collect();
        m = op_merge_vertices(&m, Some(&seam), threshold.max(1e-6));
    }
    m
}

/// Bridge two border edge loops (or two faces) with quads.
pub fn op_bridge(pm: &PolyMesh, faces: Option<&[usize]>, edges: Option<&[(u32, u32)]>, divisions: u32) -> PolyMesh {
    let mut m = pm.clone();
    let (la, lb): (Vec<u32>, Vec<u32>) = if let Some(fs) = faces.filter(|f| f.len() == 2) {
        let a = pm.f[fs[0]].clone();
        let b = pm.f[fs[1]].clone();
        m = op_delete_faces_keep_verts(&m, fs);
        (a, b)
    } else if let Some(es) = edges {
        let set: HashSet<(u32, u32)> = es.iter().map(|&(a, b)| epair(a, b)).collect();
        let loops: Vec<Vec<u32>> = pm
            .boundary_loops()
            .into_iter()
            .filter(|l| {
                let n = l.len();
                (0..n).any(|i| set.contains(&epair(l[i], l[(i + 1) % n])))
            })
            .collect();
        if loops.len() != 2 {
            return m;
        }
        (loops[0].clone(), loops[1].clone())
    } else {
        return m;
    };
    if la.len() != lb.len() || la.is_empty() {
        return m;
    }
    let n = la.len();
    // align: the loops face each other, so walk b reversed; pick the rotation with the shortest total distance
    let lb_rev: Vec<u32> = lb.iter().rev().copied().collect();
    let mut best = (f64::INFINITY, 0);
    for s in 0..n {
        let d: f64 = (0..n).map(|i| m.v[la[i] as usize].distance(m.v[lb_rev[(i + s) % n] as usize])).sum();
        if d < best.0 {
            best = (d, s);
        }
    }
    let lb2: Vec<u32> = (0..n).map(|i| lb_rev[(i + best.1) % n]).collect();
    let steps = divisions + 1;
    let mut rows: Vec<Vec<u32>> = vec![la.clone()];
    for d in 1..steps {
        let t = d as f64 / steps as f64;
        let row: Vec<u32> = (0..n)
            .map(|i| {
                m.v.push(m.v[la[i] as usize].lerp(m.v[lb2[i] as usize], t));
                (m.v.len() - 1) as u32
            })
            .collect();
        rows.push(row);
    }
    rows.push(lb2);
    for r in 0..rows.len() - 1 {
        for i in 0..n {
            let j = (i + 1) % n;
            // la is in face winding of the removed/boundary side: side faces go b->a
            m.add_face(vec![rows[r][j], rows[r][i], rows[r + 1][i], rows[r + 1][j]], None, None);
        }
    }
    m
}

fn op_delete_faces_keep_verts(pm: &PolyMesh, faces: &[usize]) -> PolyMesh {
    let mut m = pm.clone();
    let del: HashSet<usize> = faces.iter().copied().collect();
    let keep: Vec<bool> = (0..m.f.len()).map(|i| !del.contains(&i)).collect();
    m.retain_faces(&keep);
    m
}

/// Soften/harden edges: set explicit hardness from an angle threshold.
pub fn op_soft_edge(pm: &PolyMesh, angle: f64, edges: Option<&[(u32, u32)]>) -> PolyMesh {
    let mut m = pm.clone();
    match edges {
        None => {
            m.soft_angle = angle;
            m.edge_hard = None;
        }
        Some(es) => {
            let topo = pm.topo();
            let cosl = (angle * crate::math::D2R).cos();
            let eh = m.edge_hard.get_or_insert_with(BTreeMap::new);
            for &(a, b) in es {
                let Some(e) = topo.edge(a, b) else { continue };
                let fs = &topo.edge_faces[e];
                let hard = if fs.len() == 2 {
                    pm.face_normal(fs[0]).normalize_or_zero().dot(pm.face_normal(fs[1]).normalize_or_zero()) < cosl
                } else {
                    true
                };
                eh.insert(ekey(a, b).to_string(), hard);
            }
        }
    }
    m
}

pub fn op_extract_faces(pm: &PolyMesh, faces: &[usize]) -> (PolyMesh, PolyMesh) {
    let set: HashSet<usize> = faces.iter().copied().collect();
    let keep: Vec<bool> = (0..pm.f.len()).map(|i| !set.contains(&i)).collect();
    let take: Vec<bool> = keep.iter().map(|k| !k).collect();
    let mut a = pm.clone();
    a.retain_faces(&keep);
    a.compact();
    let mut b = pm.clone();
    b.retain_faces(&take);
    b.compact();
    (a, b)
}

/// Combine meshes (already transformed into a common space).
pub fn combine_meshes(list: &[(PolyMesh, Option<String>)]) -> PolyMesh {
    let mut out = PolyMesh { soft_angle: list.first().map(|l| l.0.soft_angle).unwrap_or(60.0), ..Default::default() };
    let any_uv = list.iter().any(|l| l.0.uv.is_some());
    let mats: HashSet<Option<String>> = list.iter().map(|l| l.1.clone()).collect();
    let per_face_mat = mats.len() > 1 || list.iter().any(|l| l.0.fm.is_some());
    for (pm, mat) in list {
        let base = out.v.len() as u32;
        out.v.extend(pm.v.iter().copied());
        for fi in 0..pm.f.len() {
            let f = pm.f[fi].iter().map(|&i| i + base).collect();
            let uv = if any_uv { pm.face_uv(fi).cloned() } else { None };
            let m = if per_face_mat { pm.face_mat(fi).cloned().or(mat.clone()) } else { None };
            out.add_face(f, uv, m);
        }
    }
    out
}

/// UV projection: "planar" | "cylindrical" | "spherical" | "automatic".
pub fn op_project_uv(pm: &PolyMesh, mode: &str, axis: usize, faces: Option<&[usize]>) -> PolyMesh {
    let mut m = pm.clone();
    let bb = pm.bbox();
    let c = bb.center();
    let size = bb.size().max(DVec3::splat(1e-9));
    let sel: Option<HashSet<usize>> = faces.map(|f| f.iter().copied().collect());
    let n = m.f.len();
    let uv = m.uv.get_or_insert_with(|| vec![None; n]);
    uv.resize(n, None);
    let (ua, va) = match axis {
        0 => (2, 1),
        2 => (0, 1),
        _ => (0, 2),
    };
    for fi in 0..n {
        if sel.as_ref().is_some_and(|s| !s.contains(&fi)) {
            continue;
        }
        let fnrm = pm.face_normal(fi).normalize_or_zero();
        let corner: Vec<DVec2> = pm.f[fi]
            .iter()
            .map(|&vi| {
                let p = pm.v[vi as usize] - c;
                match mode {
                    "cylindrical" => {
                        let a = p.x.atan2(p.z);
                        DVec2::new(a / std::f64::consts::TAU + 0.5, p.y / size.y + 0.5)
                    }
                    "spherical" => {
                        let d = p.normalize_or_zero();
                        DVec2::new(d.x.atan2(d.z) / std::f64::consts::TAU + 0.5, d.y.clamp(-1.0, 1.0).asin() / std::f64::consts::PI + 0.5)
                    }
                    "automatic" => {
                        let ax = fnrm.abs();
                        let (a, b) = if ax.x >= ax.y && ax.x >= ax.z {
                            (2, 1)
                        } else if ax.y >= ax.z {
                            (0, 2)
                        } else {
                            (0, 1)
                        };
                        DVec2::new(p[a] / size.max_element() + 0.5, p[b] / size.max_element() + 0.5)
                    }
                    _ => DVec2::new(p[ua] / size[ua] + 0.5, p[va] / size[va] + 0.5),
                }
            })
            .collect();
        uv[fi] = Some(corner);
    }
    m
}

/// Planar polygon from points (Create Polygon tool).
pub fn polygon_from_points(pts: &[DVec3]) -> PolyMesh {
    let mut m = PolyMesh::new(pts.to_vec(), vec![(0..pts.len() as u32).collect()]);
    if m.face_normal(0).y < 0.0 {
        m.f[0].reverse();
    }
    m
}

#[cfg(test)]
mod chamfer_split_tests {
    use super::*;
    use crate::primitives;

    fn cube() -> PolyMesh {
        primitives::generate("polyCube", &crate::params::Params::new()).unwrap()
    }

    #[test]
    fn chamfer_corner_makes_a_triangle() {
        let pm = cube();
        let r = op_chamfer_vertex(&pm, &[0], 0.25);
        // a corner of three quads: 3 new verts replace 1, one triangle added
        assert_eq!(r.mesh.v.len(), pm.v.len() + 2);
        assert_eq!(r.mesh.f.len(), pm.f.len() + 1);
        let tri = r.select_faces.unwrap()[0];
        assert_eq!(r.mesh.f[tri].len(), 3);
        assert!(r.mesh.boundary_loops().is_empty(), "still closed");
    }

    #[test]
    fn connect_two_edge_midpoints_splits_a_quad() {
        let pm = cube();
        let f = pm.f[0].clone();
        let r = op_split(&pm, &[SplitPoint::Edge(f[0], f[1], 0.5), SplitPoint::Edge(f[2], f[3], 0.5)]);
        assert_eq!(r.mesh.f.len(), pm.f.len() + 1);
        assert_eq!(r.mesh.v.len(), pm.v.len() + 2);
        assert_eq!(r.select_edges.unwrap().len(), 1);
        assert!(r.mesh.boundary_loops().is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    fn cube() -> PolyMesh {
        primitives::generate("polyCube", &serde_json::Map::new()).unwrap()
    }

    fn is_closed(m: &PolyMesh) -> bool {
        m.topo().edge_faces.iter().all(|f| f.len() == 2)
    }

    #[test]
    fn cube_topology() {
        let c = cube();
        assert_eq!(c.v.len(), 8);
        assert_eq!(c.f.len(), 6);
        assert_eq!(c.edge_count(), 12);
        assert!(is_closed(&c));
        assert!((c.signed_volume() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn extrude_keeps_closed() {
        let c = cube();
        let r = op_extrude_faces(&c, &[0], &ExtrudeParams { local_translate_z: 1.0, ..Default::default() });
        assert_eq!(r.mesh.f.len(), 10);
        assert!(is_closed(&r.mesh));
        assert!((r.mesh.signed_volume() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn smooth_cube() {
        let s = subdivide(&cube(), 1, true);
        assert_eq!(s.f.len(), 24);
        assert!(is_closed(&s));
        assert!(s.signed_volume() < 1.0 && s.signed_volume() > 0.3);
    }

    #[test]
    fn edge_loop_insert() {
        let c = cube();
        let f0 = &c.f[0];
        let r = op_insert_edge_loop(&c, f0[0], f0[1], 0.5, 1);
        assert_eq!(r.mesh.f.len(), 10);
        assert!(is_closed(&r.mesh));
        assert!((r.mesh.signed_volume() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn delete_and_fill() {
        let c = cube();
        let d = op_delete_faces(&c, &[0]);
        assert_eq!(d.boundary_loops().len(), 1);
        let f = op_fill_hole(&d, None);
        assert!(is_closed(&f));
        assert!((f.signed_volume() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn merge_and_mirror() {
        let c = cube();
        let m = op_mirror(&c, 0, 0.5, true, 0.001);
        assert_eq!(m.v.len(), 12);
        let t = op_triangulate(&c, None);
        assert_eq!(t.f.len(), 12);
        let q = op_quadrangulate(&t, 30.0);
        assert_eq!(q.f.len(), 6);
    }

    #[test]
    fn bevel_edges_stays_closed() {
        let c = cube();
        let topo = c.topo();
        // one edge
        let r = op_bevel(&c, &[topo.edges[0]], 0.5);
        assert!(is_closed(&r.mesh));
        assert_eq!(r.mesh.f.len(), 7);
        assert!(r.mesh.signed_volume() < 1.0 && r.mesh.signed_volume() > 0.9);
        // all edges: 6 faces + 12 bevels + 8 corner triangles
        let all: Vec<(u32, u32)> = topo.edges.clone();
        let r = op_bevel(&c, &all, 0.5);
        assert!(is_closed(&r.mesh), "not closed");
        assert_eq!(r.mesh.f.len(), 26);
        assert!(r.mesh.signed_volume() > 0.0);
    }

    #[test]
    fn delete_edge_merges_faces() {
        let p = primitives::generate("polyPlane", &serde_json::from_str::<serde_json::Map<_, _>>(r#"{"subdivisionsWidth":2,"subdivisionsHeight":1}"#).unwrap()).unwrap();
        assert_eq!(p.f.len(), 2);
        let topo = p.topo();
        let inner = topo.edges.iter().enumerate().find(|(e, _)| topo.edge_faces[*e].len() == 2).unwrap().1;
        let r = op_delete_edges(&p, &[*inner]);
        assert_eq!(r.f.len(), 1);
    }
}

#[cfg(test)]
mod border_loop_tests {
    use super::*;

    #[test]
    fn border_edge_selects_the_whole_rim() {
        // issue #27: an open cylinder's rim
        let mut pm = crate::primitives::generate("polyCylinder", &Default::default()).unwrap();
        let top = (0..pm.f.len()).find(|&f| pm.f[f].len() > 4 && pm.face_normal(f).y > 0.5).unwrap();
        pm.f.remove(top);
        let topo = pm.topo();
        let e = (0..topo.edges.len()).find(|&e| topo.edge_faces[e].len() == 1).unwrap();
        let (a, b) = topo.edges[e];
        let l = edge_loop(&pm, &topo, a, b);
        assert_eq!(l.len(), 20, "the full 20-edge rim");
        assert!(l.iter().all(|&(x, y)| topo.edge(x, y).is_some_and(|e| topo.edge_faces[e].len() == 1)));
    }
}
