//! UV editing math (src/js/ui/editorMath.js): face corners, UV shells, layout and relax.
//!
//! A *corner* is one face-vertex: id = `base[face] + index in face`. UVs live per corner in
//! `PolyMesh::uv`.

use crate::mesh::PolyMesh;
use glam::DVec2;
use std::collections::HashMap;

pub struct CornerIndex {
    pub base: Vec<usize>,
    /// corner -> face, index in face, vertex
    pub cf: Vec<usize>,
    pub cc: Vec<usize>,
    pub cv: Vec<u32>,
    pub by_vert: HashMap<u32, Vec<usize>>,
}

impl CornerIndex {
    pub fn new(pm: &PolyMesh) -> Self {
        let mut base = Vec::with_capacity(pm.f.len() + 1);
        let (mut cf, mut cc, mut cv) = (vec![], vec![], vec![]);
        let mut by_vert: HashMap<u32, Vec<usize>> = HashMap::new();
        for (fi, f) in pm.f.iter().enumerate() {
            base.push(cf.len());
            for (j, &v) in f.iter().enumerate() {
                by_vert.entry(v).or_default().push(cf.len());
                cf.push(fi);
                cc.push(j);
                cv.push(v);
            }
        }
        base.push(cf.len());
        CornerIndex { base, cf, cc, cv, by_vert }
    }
    pub fn len(&self) -> usize {
        self.cf.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cf.is_empty()
    }
}

pub fn corner_uv(pm: &PolyMesh, ci: &CornerIndex, id: usize) -> Option<DVec2> {
    pm.face_uv(*ci.cf.get(id)?).and_then(|u| u.get(ci.cc[id]).copied())
}

fn set_corner_uv(pm: &mut PolyMesh, ci: &CornerIndex, id: usize, p: DVec2) {
    if let Some(Some(u)) = pm.uv.as_mut().and_then(|a| a.get_mut(ci.cf[id])) {
        if let Some(c) = u.get_mut(ci.cc[id]) {
            *c = p;
        }
    }
}

/// Corners with a UV.
pub fn all_corners(pm: &PolyMesh, ci: &CornerIndex) -> Vec<usize> {
    (0..ci.len()).filter(|&id| corner_uv(pm, ci, id).is_some()).collect()
}

/// Corners that are the same UV point as `id` (same vertex, coincident UV).
pub fn coincident(pm: &PolyMesh, ci: &CornerIndex, id: usize) -> Vec<usize> {
    let Some(u0) = corner_uv(pm, ci, id) else { return vec![] };
    ci.by_vert[&ci.cv[id]].iter().copied().filter(|&j| corner_uv(pm, ci, j).is_some_and(|u| (u - u0).abs().max_element() <= 1e-6)).collect()
}

/// UV point groups: corners sharing a vertex and a UV. Returns (group per corner or -1, count).
pub fn uv_groups(pm: &PolyMesh, ci: &CornerIndex) -> (Vec<i64>, usize) {
    let mut g = vec![-1i64; ci.len()];
    let mut n = 0;
    let mut verts: Vec<&u32> = ci.by_vert.keys().collect();
    verts.sort();
    for v in verts {
        let ids = &ci.by_vert[v];
        for a in 0..ids.len() {
            let Some(ua) = corner_uv(pm, ci, ids[a]) else { continue };
            if g[ids[a]] >= 0 {
                continue;
            }
            g[ids[a]] = n as i64;
            for &b in &ids[a + 1..] {
                if g[b] < 0 && corner_uv(pm, ci, b).is_some_and(|ub| (ub - ua).abs().max_element() <= 1e-6) {
                    g[b] = n as i64;
                }
            }
            n += 1;
        }
    }
    (g, n)
}

/// Faces connected through shared UV points: (shell per face or -1, faces per shell).
pub fn uv_shells(pm: &PolyMesh, ci: &CornerIndex) -> (Vec<i64>, Vec<Vec<usize>>) {
    let (group, count) = uv_groups(pm, ci);
    let mut par: Vec<usize> = (0..pm.f.len()).collect();
    fn find(par: &mut [usize], mut x: usize) -> usize {
        while par[x] != x {
            par[x] = par[par[x]];
            x = par[x];
        }
        x
    }
    let mut owner = vec![usize::MAX; count];
    for id in 0..ci.len() {
        if group[id] < 0 {
            continue;
        }
        let (gi, f) = (group[id] as usize, ci.cf[id]);
        if owner[gi] == usize::MAX {
            owner[gi] = f;
        } else {
            let (a, b) = (find(&mut par, owner[gi]), find(&mut par, f));
            if a != b {
                par[a] = b;
            }
        }
    }
    let mut map: HashMap<usize, usize> = HashMap::new();
    let mut shell_of = vec![-1i64; pm.f.len()];
    let mut shells: Vec<Vec<usize>> = vec![];
    for f in 0..pm.f.len() {
        if pm.face_uv(f).is_none() {
            continue;
        }
        let r = find(&mut par, f);
        let s = *map.entry(r).or_insert_with(|| {
            shells.push(vec![]);
            shells.len() - 1
        });
        shells[s].push(f);
        shell_of[f] = s as i64;
    }
    (shell_of, shells)
}

pub fn corners_of_faces(pm: &PolyMesh, ci: &CornerIndex, faces: &[usize]) -> Vec<usize> {
    faces.iter().filter(|&&f| pm.face_uv(f).is_some()).flat_map(|&f| ci.base[f]..ci.base[f + 1]).collect()
}

/// (min, max) of the corners' UVs.
pub fn uv_bounds(pm: &PolyMesh, ci: &CornerIndex, ids: &[usize]) -> Option<(DVec2, DVec2)> {
    let mut b: Option<(DVec2, DVec2)> = None;
    for &id in ids {
        if let Some(p) = corner_uv(pm, ci, id) {
            b = Some(match b {
                Some((lo, hi)) => (lo.min(p), hi.max(p)),
                None => (p, p),
            });
        }
    }
    b
}

pub fn merge_bounds(a: Option<(DVec2, DVec2)>, b: Option<(DVec2, DVec2)>) -> Option<(DVec2, DVec2)> {
    match (a, b) {
        (Some(a), Some(b)) => Some((a.0.min(b.0), a.1.max(b.1))),
        (a, None) => a,
        (None, b) => b,
    }
}

pub fn xform_corners(pm: &mut PolyMesh, ci: &CornerIndex, ids: &[usize], f: impl Fn(DVec2) -> DVec2) {
    for &id in ids {
        if let Some(p) = corner_uv(pm, ci, id) {
            set_corner_uv(pm, ci, id, f(p));
        }
    }
}

pub fn rotate_about(a: f64, c: DVec2) -> impl Fn(DVec2) -> DVec2 {
    let (s, co) = a.sin_cos();
    move |p: DVec2| {
        let d = p - c;
        c + DVec2::new(d.x * co - d.y * s, d.x * s + d.y * co)
    }
}

/// Pack shells (each scaled to unit size) in rows inside 0..1.
pub fn layout_shells(pm: &mut PolyMesh, ci: &CornerIndex, face_shells: &[Vec<usize>]) {
    let (pad, margin) = (0.04, 0.01);
    struct Item {
        ids: Vec<usize>,
        lo: DVec2,
        s: f64,
        w: f64,
        h: f64,
        x: f64,
        y: f64,
    }
    let mut items: Vec<Item> = face_shells
        .iter()
        .filter_map(|faces| {
            let ids = corners_of_faces(pm, ci, faces);
            let (lo, hi) = uv_bounds(pm, ci, &ids)?;
            let s = 1.0 / (hi - lo).max_element().max(1e-9);
            Some(Item { ids, lo, s, w: (hi.x - lo.x) * s, h: (hi.y - lo.y) * s, x: 0.0, y: 0.0 })
        })
        .collect();
    if items.is_empty() {
        return;
    }
    let area: f64 = items.iter().map(|it| (it.w + pad) * (it.h + pad)).sum();
    let row_w = (1.0 + pad).max(area.sqrt() * 1.15);
    items.sort_by(|a, b| b.h.total_cmp(&a.h));
    let (mut x, mut y, mut row_h, mut max_w) = (0.0, 0.0, 0.0f64, 0.0f64);
    for it in items.iter_mut() {
        if x > 0.0 && x + it.w > row_w {
            y += row_h + pad;
            x = 0.0;
            row_h = 0.0;
        }
        it.x = x;
        it.y = y;
        x += it.w + pad;
        row_h = row_h.max(it.h);
        max_w = max_w.max(x - pad);
    }
    let total_h = y + row_h;
    let sc = (1.0 - 2.0 * margin) / max_w.max(total_h).max(1e-9);
    for it in &items {
        let (lo, s, ox, oy) = (it.lo, it.s, it.x, it.y);
        xform_corners(pm, ci, &it.ids, |p| DVec2::new(margin + (ox + (p.x - lo.x) * s) * sc, margin + (oy + (p.y - lo.y) * s) * sc));
    }
}

/// Laplacian relax; border points and points outside `movable` stay pinned.
pub fn relax_uvs(pm: &mut PolyMesh, ci: &CornerIndex, movable: Option<&[usize]>, iterations: usize) {
    let (group, count) = uv_groups(pm, ci);
    let mut nb: Vec<Vec<usize>> = vec![vec![]; count];
    let mut edge_use: HashMap<(usize, usize), u32> = HashMap::new();
    for f in 0..pm.f.len() {
        if pm.face_uv(f).is_none() {
            continue;
        }
        let n = pm.f[f].len();
        for j in 0..n {
            let (a, b) = (group[ci.base[f] + j], group[ci.base[f] + (j + 1) % n]);
            if a < 0 || b < 0 || a == b {
                continue;
            }
            let (a, b) = (a as usize, b as usize);
            if !nb[a].contains(&b) {
                nb[a].push(b);
                nb[b].push(a);
            }
            *edge_use.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    let mut pinned = vec![false; count];
    for (&(a, b), &n) in &edge_use {
        if n == 1 {
            pinned[a] = true;
            pinned[b] = true;
        }
    }
    if let Some(mv) = movable {
        let mut m = vec![false; count];
        for &id in mv {
            if group[id] >= 0 {
                m[group[id] as usize] = true;
            }
        }
        for g in 0..count {
            pinned[g] |= !m[g];
        }
    }
    let mut pos = vec![DVec2::ZERO; count];
    let mut members: Vec<Vec<usize>> = vec![vec![]; count];
    for id in 0..ci.len() {
        if group[id] >= 0 {
            let g = group[id] as usize;
            members[g].push(id);
            pos[g] = corner_uv(pm, ci, id).unwrap_or_default();
        }
    }
    for _ in 0..iterations {
        let next: Vec<DVec2> = (0..count).map(|g| if pinned[g] || nb[g].is_empty() { pos[g] } else { nb[g].iter().map(|&o| pos[o]).sum::<DVec2>() / nb[g].len() as f64 }).collect();
        pos = next;
    }
    for g in 0..count {
        for &id in &members[g] {
            set_corner_uv(pm, ci, id, pos[g]);
        }
    }
}

/// Signed UV area of a face (negative = flipped).
pub fn face_uv_area(u: &[DVec2]) -> f64 {
    (0..u.len()).map(|i| u[i].perp_dot(u[(i + 1) % u.len()])).sum::<f64>() / 2.0
}

/// A UV Editor operation on corners (all corners when `ids` is empty).
pub fn uv_op(pm: &PolyMesh, kind: &str, ids: &[usize]) -> PolyMesh {
    let mut m = pm.clone();
    let ci = CornerIndex::new(&m);
    let ids: Vec<usize> = if ids.is_empty() { all_corners(&m, &ci) } else { ids.to_vec() };
    let Some((lo, hi)) = uv_bounds(&m, &ci, &ids) else { return m };
    let c = (lo + hi) * 0.5;
    match kind {
        "flipU" => xform_corners(&mut m, &ci, &ids, |p| DVec2::new(2.0 * c.x - p.x, p.y)),
        "flipV" => xform_corners(&mut m, &ci, &ids, |p| DVec2::new(p.x, 2.0 * c.y - p.y)),
        "rot90" => xform_corners(&mut m, &ci, &ids, rotate_about(std::f64::consts::FRAC_PI_2, c)),
        "rot-90" => xform_corners(&mut m, &ci, &ids, rotate_about(-std::f64::consts::FRAC_PI_2, c)),
        "normalize" => {
            let s = 1.0 / (hi - lo).max_element().max(1e-12);
            xform_corners(&mut m, &ci, &ids, |p| (p - lo) * s);
        }
        "layout" | "relax" => {
            let (shell_of, shells) = uv_shells(&m, &ci);
            let mut want: Vec<usize> = ids.iter().filter_map(|&id| usize::try_from(shell_of[ci.cf[id]]).ok()).collect();
            want.sort();
            want.dedup();
            let faces: Vec<Vec<usize>> = want.iter().map(|&s| shells[s].clone()).collect();
            if kind == "layout" {
                layout_shells(&mut m, &ci, &faces);
            } else {
                let movable = corners_of_faces(&m, &ci, &faces.concat());
                relax_uvs(&mut m, &ci, Some(&movable), 60);
            }
        }
        _ => {}
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;

    fn plane() -> PolyMesh {
        let mut p = Params::new();
        p.insert("subdivisionsWidth".into(), serde_json::json!(2));
        p.insert("subdivisionsHeight".into(), serde_json::json!(2));
        crate::primitives::generate("polyPlane", &p).unwrap()
    }

    #[test]
    fn plane_is_one_shell_and_ops_keep_bounds() {
        let pm = plane();
        let ci = CornerIndex::new(&pm);
        let (_, shells) = uv_shells(&pm, &ci);
        assert_eq!(shells.len(), 1);
        let b0 = uv_bounds(&pm, &ci, &all_corners(&pm, &ci)).unwrap();
        let f = uv_op(&pm, "flipU", &[]);
        assert_eq!(uv_bounds(&f, &ci, &all_corners(&f, &ci)).unwrap(), b0);
        // flipping mirrors the faces in UV space
        let a0 = face_uv_area(pm.face_uv(0).unwrap());
        assert!((face_uv_area(f.face_uv(0).unwrap()) + a0).abs() < 1e-9);
        let n = uv_op(&uv_op(&pm, "rot90", &[]), "normalize", &[]);
        let (lo, hi) = uv_bounds(&n, &ci, &all_corners(&n, &ci)).unwrap();
        assert!(lo.abs().max_element() < 1e-9 && (hi.max_element() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn relax_pins_the_border() {
        let mut pm = plane();
        let ci = CornerIndex::new(&pm);
        // push the centre vertex off, relax brings it back to the middle of its neighbours
        let center = ci.by_vert.keys().copied().find(|v| ci.by_vert[v].len() == 4).unwrap();
        let ids = ci.by_vert[&center].clone();
        xform_corners(&mut pm, &ci, &ids, |p| p + DVec2::new(0.2, 0.1));
        let border_before = corner_uv(&pm, &ci, 0).unwrap();
        let r = uv_op(&pm, "relax", &[]);
        assert_eq!(corner_uv(&r, &ci, 0).unwrap(), border_before);
        assert!((corner_uv(&r, &ci, ids[0]).unwrap() - DVec2::splat(0.5)).length() < 1e-6);
    }
}
