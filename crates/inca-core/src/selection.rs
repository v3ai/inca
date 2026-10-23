//! Component selection helpers: grow/shrink, conversion, edge loops/rings, borders.

use crate::mesh::{edge_loop, edge_ring, epair};
use crate::scene::{CompMode, CompSel, Scene};
use std::collections::BTreeSet;

/// Convert the component selection of every hilited mesh to another mode.
pub fn convert(s: &mut Scene, to: CompMode) {
    for id in s.hilite() {
        let Some(pm) = s.node(&id).and_then(|n| n.mesh()).map(|m| m.mesh.clone()) else { continue };
        let c = s.comp_sel(&id);
        let topo = pm.topo();
        let verts: BTreeSet<u32> = s.comp_verts(&id).into_iter().collect();
        let mut out = CompSel::default();
        match to {
            CompMode::Vertex => out.v = verts,
            CompMode::Edge => {
                if !c.f.is_empty() {
                    for &f in &c.f {
                        let face = &pm.f[f];
                        for i in 0..face.len() {
                            out.e.insert(epair(face[i], face[(i + 1) % face.len()]));
                        }
                    }
                } else {
                    for &(a, b) in &topo.edges {
                        if verts.contains(&a) && verts.contains(&b) {
                            out.e.insert((a, b));
                        }
                    }
                }
            }
            CompMode::Face => {
                for (fi, f) in pm.f.iter().enumerate() {
                    if f.iter().all(|v| verts.contains(v)) {
                        out.f.insert(fi);
                    }
                }
            }
        }
        s.comp.insert(id, out);
    }
    s.comp_mode = Some(to);
    s.rev += 1;
}

/// Grow (or shrink) the selection by one ring of neighbours.
pub fn grow(s: &mut Scene, shrink: bool) {
    let Some(mode) = s.comp_mode else { return };
    for id in s.hilite() {
        let Some(pm) = s.node(&id).and_then(|n| n.mesh()).map(|m| m.mesh.clone()) else { continue };
        let topo = pm.topo();
        let c = s.comp_sel(&id);
        let mut out = c.clone();
        match mode {
            CompMode::Vertex => {
                if shrink {
                    // keep vertices whose neighbours are all selected
                    out.v = c.v.iter().copied().filter(|&v| topo.vert_edges[v as usize].iter().all(|&e| {
                        let (a, b) = topo.edges[e];
                        c.v.contains(&a) && c.v.contains(&b)
                    })).collect();
                } else {
                    for &v in &c.v {
                        for &f in &topo.vert_faces[v as usize] {
                            out.v.extend(pm.f[f].iter().copied());
                        }
                    }
                }
            }
            CompMode::Face => {
                if shrink {
                    out.f = c.f.iter().copied().filter(|&f| pm.f[f].iter().all(|v| topo.vert_faces[*v as usize].iter().all(|g| c.f.contains(g)))).collect();
                } else {
                    for &f in &c.f {
                        for v in &pm.f[f] {
                            out.f.extend(topo.vert_faces[*v as usize].iter().copied());
                        }
                    }
                }
            }
            CompMode::Edge => {
                if shrink {
                    out.e = c.e.iter().copied().filter(|&(a, b)| [a, b].iter().all(|v| topo.vert_edges[*v as usize].iter().all(|e| c.e.contains(&topo.edges[*e])))).collect();
                } else {
                    for &(a, b) in &c.e {
                        for v in [a, b] {
                            out.e.extend(topo.vert_edges[v as usize].iter().map(|&e| topo.edges[e]));
                        }
                    }
                }
            }
        }
        s.comp.insert(id, out);
    }
    s.rev += 1;
}

/// Extend selected edges to full loops (`ring` = edge rings).
pub fn loops(s: &mut Scene, ring: bool) {
    for id in s.hilite() {
        let Some(pm) = s.node(&id).and_then(|n| n.mesh()).map(|m| m.mesh.clone()) else { continue };
        let topo = pm.topo();
        let c = s.comp_sel(&id);
        let mut out = c.clone();
        for &(a, b) in &c.e {
            if ring {
                out.e.extend(edge_ring(&pm, &topo, a, b).into_iter().map(|(x, y)| epair(x, y)));
            } else {
                out.e.extend(edge_loop(&pm, &topo, a, b));
            }
        }
        s.comp.insert(id, out);
    }
    s.comp_mode = Some(CompMode::Edge);
    s.rev += 1;
}

/// Select border edges of the hilited meshes.
pub fn border(s: &mut Scene) {
    for id in s.hilite() {
        let Some(pm) = s.node(&id).and_then(|n| n.mesh()).map(|m| m.mesh.clone()) else { continue };
        let topo = pm.topo();
        let e: BTreeSet<(u32, u32)> = topo.edges.iter().enumerate().filter(|(i, _)| topo.edge_faces[*i].len() == 1).map(|(_, e)| *e).collect();
        s.comp.insert(id, CompSel { e, ..Default::default() });
    }
    s.comp_mode = Some(CompMode::Edge);
    s.rev += 1;
}

/// Select all / invert components of the current mode.
pub fn all_components(s: &mut Scene, invert: bool) {
    let Some(mode) = s.comp_mode else { return };
    for id in s.hilite() {
        let Some(pm) = s.node(&id).and_then(|n| n.mesh()).map(|m| m.mesh.clone()) else { continue };
        let c = s.comp_sel(&id);
        let mut out = CompSel::default();
        match mode {
            CompMode::Vertex => out.v = (0..pm.v.len() as u32).filter(|v| !invert || !c.v.contains(v)).collect(),
            CompMode::Face => out.f = (0..pm.f.len()).filter(|f| !invert || !c.f.contains(f)).collect(),
            CompMode::Edge => out.e = pm.topo().edges.into_iter().filter(|e| !invert || !c.e.contains(e)).collect(),
        }
        s.comp.insert(id, out);
    }
    s.rev += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;

    #[test]
    fn loop_and_convert() {
        let mut s = Scene::new();
        let id = s.create_mesh("polyCube", &Params::new(), None);
        s.select(std::slice::from_ref(&id), crate::scene::SelMode::Replace);
        s.set_comp_mode(Some(CompMode::Edge));
        let e = s.node(&id).unwrap().mesh().unwrap().mesh.topo().edges[0];
        s.comp.insert(id.clone(), CompSel { e: [e].into_iter().collect(), ..Default::default() });
        loops(&mut s, true);
        assert_eq!(s.comp_sel(&id).e.len(), 4);
        convert(&mut s, CompMode::Vertex);
        assert_eq!(s.comp_sel(&id).v.len(), 8);
        convert(&mut s, CompMode::Face);
        assert_eq!(s.comp_sel(&id).f.len(), 6);
        grow(&mut s, true);
        assert_eq!(s.comp_sel(&id).f.len(), 6);
    }
}
