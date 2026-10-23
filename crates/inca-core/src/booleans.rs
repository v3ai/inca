//! Mesh booleans through `boolmesh` (a pure-Rust port of Manifold).

use crate::math::DVec3;
use crate::mesh::PolyMesh;
use boolmesh::prelude::*;

fn to_manifold(pm: &PolyMesh) -> Result<Manifold, String> {
    let mut pos = Vec::with_capacity(pm.v.len() * 3);
    for p in &pm.v {
        pos.extend_from_slice(&[p.x, p.y, p.z]);
    }
    let mut idx = vec![];
    for fi in 0..pm.f.len() {
        let f = &pm.f[fi];
        for t in pm.triangulate_face(fi) {
            idx.extend_from_slice(&[f[t[0]] as usize, f[t[1]] as usize, f[t[2]] as usize]);
        }
    }
    Manifold::new(&pos, &idx)
}

/// `op`: 0 union, 1 difference (a - b), 2 intersection. Inputs must be closed meshes in a common space.
pub fn boolean(a: &PolyMesh, b: &PolyMesh, op: usize) -> Result<PolyMesh, String> {
    let ma = to_manifold(&crate::mesh::op_merge_vertices(a, None, 1e-7)).map_err(|e| format!("first mesh is not closed: {e}"))?;
    let mb = to_manifold(&crate::mesh::op_merge_vertices(b, None, 1e-7)).map_err(|e| format!("second mesh is not closed: {e}"))?;
    let ty = match op {
        1 => OpType::Subtract,
        2 => OpType::Intersect,
        _ => OpType::Add,
    };
    let r = compute_boolean(&ma, &mb, ty)?;
    let v: Vec<DVec3> = r.ps.iter().map(|p| DVec3::new(p.x, p.y, p.z)).collect();
    let tails: Vec<u32> = r.hs.iter().map(|h| h.tail as u32).collect();
    let f: Vec<Vec<u32>> = tails.chunks(3).map(|c| c.to_vec()).collect();
    let tri = PolyMesh::new(v, f);
    // merge coplanar triangle pairs back into quads for a tidier result
    Ok(crate::mesh::op_quadrangulate(&tri, 0.5))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn cube_minus_cube() {
        let a = primitives::cube(2.0, 2.0, 2.0, 1, 1, 1);
        let b = primitives::cube(2.0, 2.0, 2.0, 1, 1, 1).transformed(&glam::DMat4::from_translation(DVec3::new(1.0, 1.0, 1.0)));
        let u = boolean(&a, &b, 0).unwrap();
        assert!((u.signed_volume() - 15.0).abs() < 1e-6, "{}", u.signed_volume());
        let d = boolean(&a, &b, 1).unwrap();
        assert!((d.signed_volume() - 7.0).abs() < 1e-6, "{}", d.signed_volume());
        let i = boolean(&a, &b, 2).unwrap();
        assert!((i.signed_volume() - 1.0).abs() < 1e-6, "{}", i.signed_volume());
    }
}
