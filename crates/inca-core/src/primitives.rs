//! Primitive generators (construction-history roots) and their parameter tables.
//! Parameter names, defaults and MEL short flags follow Maya / the JS app (`GEN_INFO`).

use crate::math::{DVec2, DVec3};
use crate::mesh::PolyMesh;
use crate::params::{num, Params};
use std::f64::consts::{PI, TAU};

/// How a history parameter is edited in the Channel Box / Attribute Editor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChanKind {
    Float,
    Int,
    Bool,
    Enum(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug)]
pub struct Channel {
    pub key: &'static str,
    pub kind: ChanKind,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub label: Option<&'static str>,
}

pub const fn ch(key: &'static str, kind: ChanKind, min: Option<f64>, max: Option<f64>) -> Channel {
    Channel { key, kind, min, max, label: None }
}
const fn f(key: &'static str) -> Channel {
    ch(key, ChanKind::Float, Some(0.0001), None)
}
const fn i(key: &'static str, min: f64, max: f64) -> Channel {
    ch(key, ChanKind::Int, Some(min), Some(max))
}

pub struct GenInfo {
    pub ty: &'static str,
    pub label: &'static str,
    pub base: &'static str,
    pub nurbs: bool,
    pub params: &'static [(&'static str, f64)],
    pub short: &'static [(&'static str, &'static str)],
    pub channels: &'static [Channel],
}

pub const PLATONIC: &[&str] = &["Tetrahedron", "Octahedron", "Icosahedron", "Dodecahedron"];

#[allow(clippy::approx_constant)] // Maya's pyramid side length is the literal 1.41421
pub static GEN_INFO: &[GenInfo] = &[
    GenInfo {
        ty: "polySphere", label: "Sphere", base: "pSphere", nurbs: false,
        params: &[("radius", 1.0), ("subdivisionsAxis", 20.0), ("subdivisionsHeight", 20.0)],
        short: &[("r", "radius"), ("sa", "subdivisionsAxis"), ("sh", "subdivisionsHeight"), ("sx", "subdivisionsAxis"), ("sy", "subdivisionsHeight")],
        channels: &[f("radius"), i("subdivisionsAxis", 3.0, 200.0), i("subdivisionsHeight", 2.0, 200.0)],
    },
    GenInfo {
        ty: "polyCube", label: "Cube", base: "pCube", nurbs: false,
        params: &[("width", 1.0), ("height", 1.0), ("depth", 1.0), ("subdivisionsWidth", 1.0), ("subdivisionsHeight", 1.0), ("subdivisionsDepth", 1.0)],
        short: &[("w", "width"), ("h", "height"), ("d", "depth"), ("sx", "subdivisionsWidth"), ("sy", "subdivisionsHeight"), ("sz", "subdivisionsDepth")],
        channels: &[f("width"), f("height"), f("depth"), i("subdivisionsWidth", 1.0, 200.0), i("subdivisionsHeight", 1.0, 200.0), i("subdivisionsDepth", 1.0, 200.0)],
    },
    GenInfo {
        ty: "polyCylinder", label: "Cylinder", base: "pCylinder", nurbs: false,
        params: &[("radius", 1.0), ("height", 2.0), ("subdivisionsAxis", 20.0), ("subdivisionsHeight", 1.0), ("subdivisionsCaps", 0.0)],
        short: &[("r", "radius"), ("h", "height"), ("sa", "subdivisionsAxis"), ("sh", "subdivisionsHeight"), ("sc", "subdivisionsCaps"), ("sx", "subdivisionsAxis"), ("sy", "subdivisionsHeight"), ("sz", "subdivisionsCaps")],
        channels: &[f("radius"), f("height"), i("subdivisionsAxis", 3.0, 200.0), i("subdivisionsHeight", 1.0, 200.0), i("subdivisionsCaps", 0.0, 50.0)],
    },
    GenInfo {
        ty: "polyCone", label: "Cone", base: "pCone", nurbs: false,
        params: &[("radius", 1.0), ("height", 2.0), ("subdivisionsAxis", 20.0), ("subdivisionsHeight", 1.0), ("subdivisionsCap", 0.0)],
        short: &[("r", "radius"), ("h", "height"), ("sa", "subdivisionsAxis"), ("sh", "subdivisionsHeight"), ("sc", "subdivisionsCap"), ("sx", "subdivisionsAxis"), ("sy", "subdivisionsHeight"), ("sz", "subdivisionsCap")],
        channels: &[f("radius"), f("height"), i("subdivisionsAxis", 3.0, 200.0), i("subdivisionsHeight", 1.0, 200.0), i("subdivisionsCap", 0.0, 50.0)],
    },
    GenInfo {
        ty: "polyPlane", label: "Plane", base: "pPlane", nurbs: false,
        params: &[("width", 1.0), ("height", 1.0), ("subdivisionsWidth", 10.0), ("subdivisionsHeight", 10.0)],
        short: &[("w", "width"), ("h", "height"), ("sx", "subdivisionsWidth"), ("sy", "subdivisionsHeight")],
        channels: &[f("width"), f("height"), i("subdivisionsWidth", 1.0, 300.0), i("subdivisionsHeight", 1.0, 300.0)],
    },
    GenInfo {
        ty: "polyTorus", label: "Torus", base: "pTorus", nurbs: false,
        params: &[("radius", 1.0), ("sectionRadius", 0.5), ("twist", 0.0), ("subdivisionsAxis", 20.0), ("subdivisionsHeight", 20.0)],
        short: &[("r", "radius"), ("sr", "sectionRadius"), ("tw", "twist"), ("sa", "subdivisionsAxis"), ("sh", "subdivisionsHeight"), ("sx", "subdivisionsAxis"), ("sy", "subdivisionsHeight")],
        channels: &[f("radius"), f("sectionRadius"), ch("twist", ChanKind::Float, None, None), i("subdivisionsAxis", 3.0, 200.0), i("subdivisionsHeight", 3.0, 200.0)],
    },
    GenInfo {
        ty: "polyPyramid", label: "Pyramid", base: "pPyramid", nurbs: false,
        params: &[("sideLength", 1.41421), ("numberOfSides", 4.0)],
        short: &[("w", "sideLength"), ("ns", "numberOfSides")],
        channels: &[f("sideLength"), i("numberOfSides", 3.0, 5.0)],
    },
    GenInfo {
        ty: "polyPrism", label: "Prism", base: "pPrism", nurbs: false,
        params: &[("length", 2.0), ("sideLength", 2.0), ("numberOfSides", 3.0)],
        short: &[("l", "length"), ("w", "sideLength"), ("ns", "numberOfSides")],
        channels: &[f("length"), f("sideLength"), i("numberOfSides", 3.0, 100.0)],
    },
    GenInfo {
        ty: "polyPipe", label: "Pipe", base: "pPipe", nurbs: false,
        params: &[("radius", 1.0), ("height", 2.0), ("thickness", 0.5), ("subdivisionsAxis", 20.0), ("subdivisionsHeight", 1.0)],
        short: &[("r", "radius"), ("h", "height"), ("t", "thickness"), ("sa", "subdivisionsAxis"), ("sh", "subdivisionsHeight")],
        channels: &[f("radius"), f("height"), f("thickness"), i("subdivisionsAxis", 3.0, 200.0), i("subdivisionsHeight", 1.0, 200.0)],
    },
    GenInfo {
        ty: "polyHelix", label: "Helix", base: "pHelix", nurbs: false,
        params: &[("coils", 3.0), ("height", 2.0), ("width", 2.0), ("radius", 0.4), ("subdivisionsAxis", 8.0), ("subdivisionsCoil", 50.0), ("direction", 1.0)],
        short: &[("c", "coils"), ("h", "height"), ("w", "width"), ("r", "radius"), ("sa", "subdivisionsAxis"), ("sco", "subdivisionsCoil"), ("d", "direction")],
        channels: &[ch("coils", ChanKind::Float, Some(0.1), None), f("height"), f("width"), f("radius"), i("subdivisionsAxis", 3.0, 100.0), i("subdivisionsCoil", 3.0, 1000.0), ch("direction", ChanKind::Enum(&["Clockwise", "Counterclockwise"]), None, None)],
    },
    GenInfo {
        ty: "polyDisc", label: "Disc", base: "pDisc", nurbs: false,
        params: &[("radius", 1.0), ("sides", 20.0), ("subdivisions", 1.0)],
        short: &[("r", "radius"), ("s", "sides"), ("sd", "subdivisions")],
        channels: &[f("radius"), i("sides", 3.0, 200.0), i("subdivisions", 1.0, 50.0)],
    },
    GenInfo {
        ty: "polyPlatonic", label: "Platonic Solid", base: "pSolid", nurbs: false,
        params: &[("primitive", 2.0), ("radius", 1.0)],
        short: &[("p", "primitive"), ("r", "radius")],
        channels: &[ch("primitive", ChanKind::Enum(PLATONIC), None, None), f("radius")],
    },
    GenInfo {
        ty: "nurbsSphere", label: "Sphere", base: "nurbsSphere", nurbs: true,
        params: &[("radius", 1.0), ("sections", 8.0), ("spans", 4.0)],
        short: &[("r", "radius"), ("s", "sections"), ("nsp", "spans")],
        channels: &[f("radius"), i("sections", 4.0, 50.0), i("spans", 2.0, 50.0)],
    },
    GenInfo {
        ty: "nurbsCube", label: "Cube", base: "nurbsCube", nurbs: true,
        params: &[("width", 1.0), ("lengthRatio", 1.0), ("heightRatio", 1.0), ("patchesU", 1.0), ("patchesV", 1.0)],
        short: &[("w", "width"), ("lr", "lengthRatio"), ("hr", "heightRatio"), ("u", "patchesU"), ("v", "patchesV")],
        channels: &[f("width"), f("lengthRatio"), f("heightRatio"), i("patchesU", 1.0, 50.0), i("patchesV", 1.0, 50.0)],
    },
    GenInfo {
        ty: "nurbsCylinder", label: "Cylinder", base: "nurbsCylinder", nurbs: true,
        params: &[("radius", 1.0), ("height", 2.0), ("sections", 8.0), ("spans", 1.0)],
        short: &[("r", "radius"), ("hr", "height"), ("s", "sections"), ("nsp", "spans")],
        channels: &[f("radius"), f("height"), i("sections", 4.0, 50.0), i("spans", 1.0, 50.0)],
    },
    GenInfo {
        ty: "nurbsCone", label: "Cone", base: "nurbsCone", nurbs: true,
        params: &[("radius", 1.0), ("height", 2.0), ("sections", 8.0), ("spans", 1.0)],
        short: &[("r", "radius"), ("hr", "height"), ("s", "sections"), ("nsp", "spans")],
        channels: &[f("radius"), f("height"), i("sections", 4.0, 50.0), i("spans", 1.0, 50.0)],
    },
    GenInfo {
        ty: "nurbsPlane", label: "Plane", base: "nurbsPlane", nurbs: true,
        params: &[("width", 1.0), ("lengthRatio", 1.0), ("patchesU", 1.0), ("patchesV", 1.0)],
        short: &[("w", "width"), ("lr", "lengthRatio"), ("u", "patchesU"), ("v", "patchesV")],
        channels: &[f("width"), f("lengthRatio"), i("patchesU", 1.0, 50.0), i("patchesV", 1.0, 50.0)],
    },
    GenInfo {
        ty: "nurbsTorus", label: "Torus", base: "nurbsTorus", nurbs: true,
        params: &[("radius", 1.0), ("sectionRadius", 0.5), ("sections", 8.0), ("spans", 4.0)],
        short: &[("r", "radius"), ("s", "sections"), ("nsp", "spans")],
        channels: &[f("radius"), f("sectionRadius"), i("sections", 4.0, 50.0), i("spans", 3.0, 50.0)],
    },
    GenInfo {
        ty: "polyType", label: "Type", base: "typeMesh", nurbs: false,
        params: &[("fontSize", 1.0), ("extrudeDepth", 0.2), ("curveResolution", 4.0)],
        short: &[("fs", "fontSize"), ("d", "extrudeDepth"), ("cr", "curveResolution")],
        channels: &[f("fontSize"), f("extrudeDepth"), i("curveResolution", 1.0, 32.0)],
    },
];

pub fn gen_info(ty: &str) -> Option<&'static GenInfo> {
    GEN_INFO.iter().find(|g| g.ty == ty)
}

pub fn default_params(ty: &str) -> Params {
    let mut p = Params::new();
    if let Some(g) = gen_info(ty) {
        for (k, v) in g.params {
            p.insert((*k).into(), serde_json::json!(v));
        }
    }
    p
}

/// Run a generator with params (missing params use defaults).
pub fn generate(ty: &str, p: &Params) -> Option<PolyMesh> {
    let g = gen_info(ty)?;
    let n = |k: &str| num(p, k).unwrap_or_else(|| g.params.iter().find(|(pk, _)| *pk == k).map(|x| x.1).unwrap_or(0.0));
    let ni = |k: &str, lo: usize| (n(k).round().max(lo as f64)) as usize;
    let mut m = match ty {
        "polyType" => crate::text::text_mesh(p.get("text").and_then(|v| v.as_str()).unwrap_or("Inca"), n("fontSize"), n("extrudeDepth").max(0.0), ni("curveResolution", 1)),
        "polyCube" => cube(n("width"), n("height"), n("depth"), ni("subdivisionsWidth", 1), ni("subdivisionsHeight", 1), ni("subdivisionsDepth", 1)),
        "polySphere" => sphere(n("radius"), ni("subdivisionsAxis", 3), ni("subdivisionsHeight", 2)),
        "polyCylinder" => cylinder(n("radius"), n("radius"), n("height"), ni("subdivisionsAxis", 3), ni("subdivisionsHeight", 1), ni("subdivisionsCaps", 0)),
        "polyCone" => cylinder(n("radius"), 0.0, n("height"), ni("subdivisionsAxis", 3), ni("subdivisionsHeight", 1), ni("subdivisionsCap", 0)),
        "polyPlane" => plane(n("width"), n("height"), ni("subdivisionsWidth", 1), ni("subdivisionsHeight", 1)),
        "polyTorus" => torus(n("radius"), n("sectionRadius"), n("twist"), ni("subdivisionsAxis", 3), ni("subdivisionsHeight", 3)),
        "polyPyramid" => {
            let s = ni("numberOfSides", 3);
            let side = n("sideLength");
            let r = side / (2.0 * (PI / s as f64).sin());
            let h = side * std::f64::consts::FRAC_1_SQRT_2;
            cylinder_rot(r, 0.0, h, s, 1, 0, PI / s as f64)
        }
        "polyPrism" => {
            let s = ni("numberOfSides", 3);
            let r = n("sideLength") / (2.0 * (PI / s as f64).sin());
            cylinder_rot(r, r, n("length"), s, 1, 0, 0.0)
        }
        "polyPipe" => pipe(n("radius"), n("height"), n("thickness"), ni("subdivisionsAxis", 3), ni("subdivisionsHeight", 1)),
        "polyHelix" => helix(n("coils"), n("height"), n("width"), n("radius"), ni("subdivisionsAxis", 3), ni("subdivisionsCoil", 3), n("direction") >= 0.5),
        "polyDisc" => disc(n("radius"), ni("sides", 3), ni("subdivisions", 1)),
        "polyPlatonic" => platonic(n("primitive").round() as i32, n("radius")),
        "nurbsSphere" => {
            let r = n("radius");
            let (s, sp) = (ni("sections", 4), ni("spans", 2));
            surface(s, sp, 4, true, (true, true), |u, v| {
                let th = u * TAU;
                let ph = PI * (1.0 - v);
                DVec3::new(r * ph.sin() * th.sin(), r * ph.cos(), r * ph.sin() * th.cos())
            })
        }
        "nurbsCylinder" => {
            let (r, h) = (n("radius"), n("height"));
            surface(ni("sections", 4), ni("spans", 1), 4, true, (false, false), |u, v| {
                let th = u * TAU;
                DVec3::new(r * th.sin(), h * (v - 0.5), r * th.cos())
            })
        }
        "nurbsCone" => {
            let (r, h) = (n("radius"), n("height"));
            surface(ni("sections", 4), ni("spans", 1), 4, true, (false, true), |u, v| {
                let th = u * TAU;
                DVec3::new(r * (1.0 - v) * th.sin(), h * (v - 0.5), r * (1.0 - v) * th.cos())
            })
        }
        "nurbsTorus" => {
            let (r, sr) = (n("radius"), n("sectionRadius"));
            let mut m = surface(ni("sections", 4), ni("spans", 3), 4, true, (false, false), |u, v| {
                let th = u * TAU;
                let ph = v * TAU;
                let rr = r + sr * ph.cos();
                DVec3::new(rr * th.sin(), sr * ph.sin(), rr * th.cos())
            });
            m = crate::mesh::op_merge_vertices(&m, None, 1e-7);
            m
        }
        "nurbsPlane" => {
            let (w, lr) = (n("width"), n("lengthRatio"));
            surface(ni("patchesU", 1), ni("patchesV", 1), 4, false, (false, false), |u, v| DVec3::new(w * (u - 0.5), 0.0, -w * lr * (v - 0.5)))
        }
        "nurbsCube" => {
            let (w, lr, hr) = (n("width"), n("lengthRatio"), n("heightRatio"));
            let mut c = cube(w, w * hr, w * lr, ni("patchesU", 1) * 4, ni("patchesV", 1) * 4, ni("patchesU", 1) * 4);
            c.display_edges = None;
            c
        }
        _ => return None,
    };
    m.soft_angle = if g.nurbs { 180.0 } else { m.soft_angle };
    Some(m)
}

fn quad_uv(u0: f64, v0: f64, u1: f64, v1: f64) -> Vec<DVec2> {
    vec![DVec2::new(u0, v0), DVec2::new(u1, v0), DVec2::new(u1, v1), DVec2::new(u0, v1)]
}

/// Maya-like cube: shared corner vertices, UVs in the cross layout.
pub fn cube(w: f64, h: f64, d: f64, sx: usize, sy: usize, sz: usize) -> PolyMesh {
    let mut m = PolyMesh::new(vec![], vec![]);
    let mut idx: std::collections::HashMap<(i64, i64, i64), u32> = Default::default();
    let (hw, hh, hd) = (w / 2.0, h / 2.0, d / 2.0);
    let mut vid = |m: &mut PolyMesh, ix: i64, iy: i64, iz: i64| -> u32 {
        *idx.entry((ix, iy, iz)).or_insert_with(|| {
            m.v.push(DVec3::new(-hw + w * ix as f64 / sx as f64, -hh + h * iy as f64 / sy as f64, -hd + d * iz as f64 / sz as f64));
            (m.v.len() - 1) as u32
        })
    };
    // (normal axis, sign, u axis grid, v axis grid, uv rect)
    // each face is a grid over two axes: map (a,b) -> lattice coords
    struct Side {
        lat: fn(usize, usize, usize, usize, usize) -> (i64, i64, i64),
        na: usize,
        nb: usize,
        uv: (f64, f64, f64, f64),
    }
    let sides = [
        // front +z: a along +x, b along +y
        Side { lat: |a, b, _sx, _sy, sz| (a as i64, b as i64, sz as i64), na: sx, nb: sy, uv: (0.375, 0.0, 0.625, 0.25) },
        // top +y: a along +x, b along -z
        Side { lat: |a, b, _sx, sy, sz| (a as i64, sy as i64, sz as i64 - b as i64), na: sx, nb: sz, uv: (0.375, 0.25, 0.625, 0.5) },
        // back -z: a along +x, b along -y
        Side { lat: |a, b, _sx, sy, _sz| (a as i64, sy as i64 - b as i64, 0), na: sx, nb: sy, uv: (0.375, 0.5, 0.625, 0.75) },
        // bottom -y: a along +x, b along +z
        Side { lat: |a, b, _sx, _sy, _sz| (a as i64, 0, b as i64), na: sx, nb: sz, uv: (0.375, 0.75, 0.625, 1.0) },
        // right +x: a along -z, b along +y
        Side { lat: |a, b, sx, _sy, sz| (sx as i64, b as i64, sz as i64 - a as i64), na: sz, nb: sy, uv: (0.625, 0.0, 0.875, 0.25) },
        // left -x: a along +z, b along +y
        Side { lat: |a, b, _sx, _sy, _sz| (0, b as i64, a as i64), na: sz, nb: sy, uv: (0.125, 0.0, 0.375, 0.25) },
    ];
    for s in &sides {
        for b in 0..s.nb {
            for a in 0..s.na {
                let c = [(a, b), (a + 1, b), (a + 1, b + 1), (a, b + 1)];
                let face: Vec<u32> = c
                    .iter()
                    .map(|&(x, y)| {
                        let (i, j, k) = (s.lat)(x, y, sx, sy, sz);
                        vid(&mut m, i, j, k)
                    })
                    .collect();
                let (u0, v0, u1, v1) = s.uv;
                let du = (u1 - u0) / s.na as f64;
                let dv = (v1 - v0) / s.nb as f64;
                let uv = quad_uv(u0 + du * a as f64, v0 + dv * b as f64, u0 + du * (a + 1) as f64, v0 + dv * (b + 1) as f64);
                m.add_face(face, Some(uv), None);
            }
        }
    }
    m
}

pub fn plane(w: f64, h: f64, sx: usize, sy: usize) -> PolyMesh {
    let mut m = PolyMesh::new(vec![], vec![]);
    for j in 0..=sy {
        for i in 0..=sx {
            m.v.push(DVec3::new(-w / 2.0 + w * i as f64 / sx as f64, 0.0, h / 2.0 - h * j as f64 / sy as f64));
        }
    }
    let id = |i: usize, j: usize| (j * (sx + 1) + i) as u32;
    for j in 0..sy {
        for i in 0..sx {
            let uv = quad_uv(i as f64 / sx as f64, j as f64 / sy as f64, (i + 1) as f64 / sx as f64, (j + 1) as f64 / sy as f64);
            m.add_face(vec![id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)], Some(uv), None);
        }
    }
    m
}

pub fn sphere(r: f64, sa: usize, sh: usize) -> PolyMesh {
    let mut m = PolyMesh::new(vec![], vec![]);
    m.v.push(DVec3::new(0.0, -r, 0.0));
    for j in 1..sh {
        let ph = PI * j as f64 / sh as f64; // from bottom
        let y = -r * ph.cos();
        let rr = r * ph.sin();
        for i in 0..sa {
            let th = TAU * i as f64 / sa as f64;
            m.v.push(DVec3::new(rr * th.cos(), y, -rr * th.sin()));
        }
    }
    m.v.push(DVec3::new(0.0, r, 0.0));
    let top = (m.v.len() - 1) as u32;
    let ring = |j: usize, i: usize| (1 + (j - 1) * sa + (i % sa)) as u32;
    let uvp = |i: usize, j: usize| DVec2::new(i as f64 / sa as f64, j as f64 / sh as f64);
    for i in 0..sa {
        let uv = vec![DVec2::new((i as f64 + 0.5) / sa as f64, 0.0), uvp(i + 1, 1), uvp(i, 1)];
        m.add_face(vec![0, ring(1, i + 1), ring(1, i)], Some(uv), None);
    }
    for j in 1..sh - 1 {
        for i in 0..sa {
            m.add_face(vec![ring(j, i), ring(j, i + 1), ring(j + 1, i + 1), ring(j + 1, i)], Some(vec![uvp(i, j), uvp(i + 1, j), uvp(i + 1, j + 1), uvp(i, j + 1)]), None);
        }
    }
    for i in 0..sa {
        let uv = vec![uvp(i, sh - 1), uvp(i + 1, sh - 1), DVec2::new((i as f64 + 0.5) / sa as f64, 1.0)];
        m.add_face(vec![ring(sh - 1, i), ring(sh - 1, i + 1), top], Some(uv), None);
    }
    m
}

fn cylinder(r0: f64, r1: f64, h: f64, sa: usize, sh: usize, sc: usize) -> PolyMesh {
    cylinder_rot(r0, r1, h, sa, sh, sc, 0.0)
}

/// Cylinder/cone/prism/pyramid: bottom radius r0, top radius r1 (0 = apex), caps with `sc` rings.
fn cylinder_rot(r0: f64, r1: f64, h: f64, sa: usize, sh: usize, sc: usize, rot: f64) -> PolyMesh {
    let mut m = PolyMesh::new(vec![], vec![]);
    let apex = r1 == 0.0;
    let rows = if apex { sh } else { sh + 1 };
    for j in 0..rows {
        let t = j as f64 / sh as f64;
        let y = -h / 2.0 + h * t;
        let rr = r0 + (r1 - r0) * t;
        for i in 0..sa {
            let th = TAU * i as f64 / sa as f64 + rot;
            m.v.push(DVec3::new(rr * th.cos(), y, -rr * th.sin()));
        }
    }
    let id = |j: usize, i: usize| (j * sa + i % sa) as u32;
    let uvp = |i: usize, j: usize| DVec2::new(0.375 + 0.25 * i as f64 / sa as f64, 0.3125 + 0.375 * j as f64 / sh as f64);
    for j in 0..sh {
        for i in 0..sa {
            if apex && j == sh - 1 {
                continue;
            }
            m.add_face(vec![id(j, i), id(j, i + 1), id(j + 1, i + 1), id(j + 1, i)], Some(vec![uvp(i, j), uvp(i + 1, j), uvp(i + 1, j + 1), uvp(i, j + 1)]), None);
        }
    }
    if apex {
        m.v.push(DVec3::new(0.0, h / 2.0, 0.0));
        let a = (m.v.len() - 1) as u32;
        for i in 0..sa {
            m.add_face(vec![id(sh - 1, i), id(sh - 1, i + 1), a], Some(vec![uvp(i, sh - 1), uvp(i + 1, sh - 1), DVec2::new(0.375 + 0.25 * (i as f64 + 0.5) / sa as f64, 0.6875)]), None);
        }
    }
    // caps
    let caps: Vec<(usize, bool, f64)> = if apex { vec![(0, true, 0.25)] } else { vec![(0, true, 0.25), (sh, false, 0.75)] };
    for (row, bottom, vc) in caps {
        let cu = |p: DVec3| DVec2::new(0.5 + 0.12 * p.x / r0.max(1e-9), vc + 0.12 * p.z / r0.max(1e-9) * if bottom { 1.0 } else { -1.0 });
        if sc == 0 {
            let mut f: Vec<u32> = (0..sa).map(|i| id(row, i)).collect();
            if bottom {
                f.reverse();
            }
            let uv = f.iter().map(|&v| cu(m.v[v as usize])).collect();
            m.add_face(f, Some(uv), None);
        } else {
            let y = m.v[id(row, 0) as usize].y;
            let rr = if bottom { r0 } else { r1 };
            let mut prev: Vec<u32> = (0..sa).map(|i| id(row, i)).collect();
            for k in 1..sc {
                let t = 1.0 - k as f64 / sc as f64;
                let ring: Vec<u32> = (0..sa)
                    .map(|i| {
                        let th = TAU * i as f64 / sa as f64 + rot;
                        m.v.push(DVec3::new(rr * t * th.cos(), y, -rr * t * th.sin()));
                        (m.v.len() - 1) as u32
                    })
                    .collect();
                for i in 0..sa {
                    let j = (i + 1) % sa;
                    let mut f = vec![prev[i], prev[j], ring[j], ring[i]];
                    if bottom {
                        f.reverse();
                    }
                    let uv = f.iter().map(|&v| cu(m.v[v as usize])).collect();
                    m.add_face(f, Some(uv), None);
                }
                prev = ring;
            }
            m.v.push(DVec3::new(0.0, y, 0.0));
            let c = (m.v.len() - 1) as u32;
            for i in 0..sa {
                let j = (i + 1) % sa;
                let mut f = vec![prev[i], prev[j], c];
                if bottom {
                    f.reverse();
                }
                let uv = f.iter().map(|&v| cu(m.v[v as usize])).collect();
                m.add_face(f, Some(uv), None);
            }
        }
    }
    // the side walls were built bottom->top counter-clockwise from outside; caps: bottom faces down
    m
}

pub fn torus(r: f64, sr: f64, twist: f64, sa: usize, sh: usize) -> PolyMesh {
    let mut m = PolyMesh::new(vec![], vec![]);
    for i in 0..sa {
        let th = TAU * i as f64 / sa as f64;
        for j in 0..sh {
            let ph = TAU * j as f64 / sh as f64 + twist.to_radians();
            let rr = r + sr * ph.cos();
            m.v.push(DVec3::new(rr * th.cos(), sr * ph.sin(), -rr * th.sin()));
        }
    }
    let id = |i: usize, j: usize| ((i % sa) * sh + (j % sh)) as u32;
    for i in 0..sa {
        for j in 0..sh {
            let uv = quad_uv(i as f64 / sa as f64, j as f64 / sh as f64, (i + 1) as f64 / sa as f64, (j + 1) as f64 / sh as f64);
            m.add_face(vec![id(i, j), id(i, j + 1), id(i + 1, j + 1), id(i + 1, j)], Some(vec![uv[0], uv[3], uv[2], uv[1]]), None);
        }
    }
    if m.signed_volume() < 0.0 {
        m = crate::mesh::op_reverse(&m, None);
    }
    m
}

pub fn pipe(r: f64, h: f64, t: f64, sa: usize, sh: usize) -> PolyMesh {
    let ri = (r - t).max(1e-4);
    let mut m = PolyMesh::new(vec![], vec![]);
    // profile (r,y) loop: outer bottom->top, inner top->bottom
    let mut prof: Vec<(f64, f64)> = vec![];
    for j in 0..=sh {
        prof.push((r, -h / 2.0 + h * j as f64 / sh as f64));
    }
    for j in 0..=sh {
        prof.push((ri, h / 2.0 - h * j as f64 / sh as f64));
    }
    let np = prof.len();
    for i in 0..sa {
        let th = TAU * i as f64 / sa as f64;
        for &(rr, y) in &prof {
            m.v.push(DVec3::new(rr * th.cos(), y, -rr * th.sin()));
        }
    }
    let id = |i: usize, j: usize| ((i % sa) * np + (j % np)) as u32;
    for i in 0..sa {
        for j in 0..np {
            m.add_face(vec![id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)], None, None);
        }
    }
    let mut m = crate::mesh::op_merge_vertices(&m, None, 1e-9);
    if m.signed_volume() < 0.0 {
        m = crate::mesh::op_reverse(&m, None);
    }
    crate::mesh::op_project_uv(&m, "cylindrical", 1, None)
}

pub fn helix(coils: f64, h: f64, w: f64, r: f64, sa: usize, sco: usize, ccw: bool) -> PolyMesh {
    let mut m = PolyMesh::new(vec![], vec![]);
    let n = ((sco as f64) * coils).round().max(2.0) as usize;
    let rad = w / 2.0;
    let dir = if ccw { -1.0 } else { 1.0 };
    let center = |t: f64| {
        let a = TAU * coils * t * dir;
        DVec3::new(rad * a.cos(), -h / 2.0 + h * t, rad * a.sin())
    };
    for k in 0..=n {
        let t = k as f64 / n as f64;
        let c = center(t);
        let tan = (center((t + 1e-4).min(1.0)) - center((t - 1e-4).max(0.0))).normalize();
        let radial = DVec3::new(c.x, 0.0, c.z).normalize_or(DVec3::X);
        let up = tan.cross(radial).normalize();
        let side = up.cross(tan).normalize();
        for i in 0..sa {
            let a = TAU * i as f64 / sa as f64;
            m.v.push(c + (side * a.cos() + up * a.sin()) * r);
        }
    }
    let id = |k: usize, i: usize| (k * sa + i % sa) as u32;
    for k in 0..n {
        for i in 0..sa {
            m.add_face(vec![id(k, i), id(k + 1, i), id(k + 1, i + 1), id(k, i + 1)], None, None);
        }
    }
    m.add_face((0..sa).map(|i| id(0, i)).collect(), None, None);
    m.add_face((0..sa).rev().map(|i| id(n, i)).collect(), None, None);
    if m.signed_volume() < 0.0 {
        m = crate::mesh::op_reverse(&m, None);
    }
    crate::mesh::op_project_uv(&m, "cylindrical", 1, None)
}

pub fn disc(r: f64, sides: usize, subd: usize) -> PolyMesh {
    let mut m = PolyMesh::new(vec![DVec3::ZERO], vec![]);
    for k in 1..=subd {
        let rr = r * k as f64 / subd as f64;
        for i in 0..sides {
            let th = TAU * i as f64 / sides as f64;
            m.v.push(DVec3::new(rr * th.cos(), 0.0, -rr * th.sin()));
        }
    }
    let id = |k: usize, i: usize| (1 + (k - 1) * sides + i % sides) as u32;
    for i in 0..sides {
        m.add_face(vec![0, id(1, i), id(1, i + 1)], None, None);
    }
    for k in 1..subd {
        for i in 0..sides {
            m.add_face(vec![id(k, i), id(k + 1, i), id(k + 1, i + 1), id(k, i + 1)], None, None);
        }
    }
    crate::mesh::op_project_uv(&m, "planar", 1, None)
}

pub fn platonic(kind: i32, r: f64) -> PolyMesh {
    let phi = (1.0 + 5f64.sqrt()) / 2.0;
    let (v, f): (Vec<[f64; 3]>, Vec<Vec<u32>>) = match kind {
        0 => (vec![[1.0, 1.0, 1.0], [-1.0, -1.0, 1.0], [-1.0, 1.0, -1.0], [1.0, -1.0, -1.0]], vec![vec![0, 1, 3], vec![0, 2, 1], vec![0, 3, 2], vec![1, 2, 3]]),
        1 => (
            vec![[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]],
            vec![vec![0, 2, 4], vec![2, 1, 4], vec![1, 3, 4], vec![3, 0, 4], vec![2, 0, 5], vec![1, 2, 5], vec![3, 1, 5], vec![0, 3, 5]],
        ),
        3 => return dodecahedron(r),
        _ => (
            vec![
                [-1.0, phi, 0.0], [1.0, phi, 0.0], [-1.0, -phi, 0.0], [1.0, -phi, 0.0],
                [0.0, -1.0, phi], [0.0, 1.0, phi], [0.0, -1.0, -phi], [0.0, 1.0, -phi],
                [phi, 0.0, -1.0], [phi, 0.0, 1.0], [-phi, 0.0, -1.0], [-phi, 0.0, 1.0],
            ],
            vec![
                vec![0, 11, 5], vec![0, 5, 1], vec![0, 1, 7], vec![0, 7, 10], vec![0, 10, 11],
                vec![1, 5, 9], vec![5, 11, 4], vec![11, 10, 2], vec![10, 7, 6], vec![7, 1, 8],
                vec![3, 9, 4], vec![3, 4, 2], vec![3, 2, 6], vec![3, 6, 8], vec![3, 8, 9],
                vec![4, 9, 5], vec![2, 4, 11], vec![6, 2, 10], vec![8, 6, 7], vec![9, 8, 1],
            ],
        ),
    };
    let mut m = PolyMesh::new(v.iter().map(|p| DVec3::from_array(*p).normalize() * r).collect(), f);
    if m.signed_volume() < 0.0 {
        m = crate::mesh::op_reverse(&m, None);
    }
    crate::mesh::op_project_uv(&m, "automatic", 1, None)
}

fn dodecahedron(r: f64) -> PolyMesh {
    // dual of the icosahedron: face centres become vertices
    let ico = platonic(2, 1.0);
    let topo = ico.topo();
    let verts: Vec<DVec3> = (0..ico.f.len()).map(|i| ico.face_center(i).normalize() * r).collect();
    let mut faces = vec![];
    for v in 0..ico.v.len() {
        // order faces around vertex v
        let fs = &topo.vert_faces[v];
        let n = ico.v[v].normalize();
        let c = ico.v[v];
        let a0 = (ico.face_center(fs[0]) - c).normalize();
        let b0 = n.cross(a0);
        let mut ord: Vec<(f64, u32)> = fs
            .iter()
            .map(|&f| {
                let d = ico.face_center(f) - c;
                (d.dot(b0).atan2(d.dot(a0)), f as u32)
            })
            .collect();
        ord.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        faces.push(ord.into_iter().map(|x| x.1).collect());
    }
    let mut m = PolyMesh::new(verts, faces);
    if m.signed_volume() < 0.0 {
        m = crate::mesh::op_reverse(&m, None);
    }
    crate::mesh::op_project_uv(&m, "automatic", 1, None)
}

/// Tessellated parametric surface (NURBS primitive display): `spans_u` x `spans_v` patches,
/// `k` segments per patch; isoparms become `display_edges`. `wrap_u` closes in u; `poles` collapse
/// the v=0 / v=1 rows.
fn surface(spans_u: usize, spans_v: usize, k: usize, wrap_u: bool, poles: (bool, bool), f: impl Fn(f64, f64) -> DVec3) -> PolyMesh {
    let nu = spans_u * k;
    let nv = spans_v * k;
    let cols = if wrap_u { nu } else { nu + 1 };
    let mut m = PolyMesh::new(vec![], vec![]);
    let mut ids = vec![vec![0u32; cols]; nv + 1];
    for (j, row) in ids.iter_mut().enumerate() {
        let v = j as f64 / nv as f64;
        let pole = (j == 0 && poles.0) || (j == nv && poles.1);
        for (i, id) in row.iter_mut().enumerate() {
            if pole && i > 0 {
                *id = (m.v.len() - 1) as u32;
                continue;
            }
            m.v.push(f(i as f64 / nu as f64, v));
            *id = (m.v.len() - 1) as u32;
        }
    }
    let at = |i: usize, j: usize| ids[j][if wrap_u { i % nu } else { i }];
    let mut de = vec![];
    for j in 0..nv {
        for i in 0..nu {
            let q = vec![at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            let uv = quad_uv(i as f64 / nu as f64, j as f64 / nv as f64, (i + 1) as f64 / nu as f64, (j + 1) as f64 / nv as f64);
            let mut face = vec![];
            let mut fuv = vec![];
            for (c, &vi) in q.iter().enumerate() {
                if !face.contains(&vi) {
                    face.push(vi);
                    fuv.push(uv[c]);
                }
            }
            if face.len() >= 3 {
                m.add_face(face, Some(fuv), None);
            }
            if i % k == 0 && at(i, j) != at(i, j + 1) {
                de.push([at(i, j), at(i, j + 1)]);
            }
            if j % k == 0 && at(i, j) != at(i + 1, j) {
                de.push([at(i, j), at(i + 1, j)]);
            }
        }
        if !wrap_u {
            de.push([at(nu, j), at(nu, j + 1)]);
        }
    }
    for i in 0..nu {
        if at(i, nv) != at(i + 1, nv) {
            de.push([at(i, nv), at(i + 1, nv)]);
        }
    }
    m.display_edges = Some(de);
    if m.signed_volume() < -1e-9 || (m.signed_volume().abs() < 1e-9 && m.face_normal(0).y < 0.0) {
        m = crate::mesh::op_reverse(&m, None);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_generators_produce_outward_meshes() {
        for g in GEN_INFO {
            let m = generate(g.ty, &Params::new()).unwrap();
            assert!(!m.f.is_empty(), "{}", g.ty);
            for f in &m.f {
                for &i in f {
                    assert!((i as usize) < m.v.len(), "{} index", g.ty);
                }
            }
            let vol = m.signed_volume();
            assert!(vol >= -1e-9, "{} has inward faces ({vol})", g.ty);
        }
    }

    #[test]
    fn closed_solids() {
        for ty in ["polyCube", "polySphere", "polyCylinder", "polyCone", "polyTorus", "polyPyramid", "polyPrism", "polyPipe", "polyPlatonic"] {
            let m = generate(ty, &Params::new()).unwrap();
            let t = m.topo();
            assert!(t.edge_faces.iter().all(|f| f.len() == 2), "{ty} not closed");
        }
        let s = generate("polySphere", &Params::new()).unwrap();
        assert_eq!(s.v.len(), 20 * 19 + 2);
        assert_eq!(s.f.len(), 20 * 20);
    }
}
