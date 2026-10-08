// Inca — primitive generators. All return PolyMesh with per-corner UVs, CCW faces seen from outside.
import { PolyMesh, signedVolume } from './polymesh.js';

function orientByVolume(pm) {
  if (signedVolume(pm) < 0) { pm.f = pm.f.map(f => f.slice().reverse()); if (pm.uv) pm.uv = pm.uv.map(u => u ? u.slice().reverse() : u); }
  pm.invalidate();
}

const TAU = Math.PI * 2;
const R = (n, d) => Math.max(d, Math.round(n || d));

export function polyCube({ width = 1, height = 1, depth = 1, subdivisionsWidth = 1, subdivisionsHeight = 1, subdivisionsDepth = 1 } = {}) {
  const sx = R(subdivisionsWidth, 1), sy = R(subdivisionsHeight, 1), sz = R(subdivisionsDepth, 1);
  const pm = new PolyMesh([], [], []);
  const vmap = new Map();
  const vid = (x, y, z) => {
    const k = x + ',' + y + ',' + z; let i = vmap.get(k);
    if (i === undefined) { i = pm.v.length; pm.v.push([(x / sx - 0.5) * width, (y / sy - 0.5) * height, (z / sz - 0.5) * depth]); vmap.set(k, i); }
    return i;
  };
  // faces: [normal axis, sign, u-axis, v-axis] laid out in a cross UV layout like Maya
  // cross layout cells (u,v origin) for: front, top, back, bottom, right, left
  const sides = [
    { n: 'z+', cell: [0.375, 0.0], f: (u, v) => [u * sx, v * sy, sz], us: sx, vs: sy },          // front
    { n: 'y+', cell: [0.375, 0.25], f: (u, v) => [u * sx, sy, sz - v * sz], us: sx, vs: sz },    // top
    { n: 'z-', cell: [0.375, 0.5], f: (u, v) => [u * sx, sy - v * sy, 0], us: sx, vs: sy },     // back
    { n: 'y-', cell: [0.375, 0.75], f: (u, v) => [u * sx, 0, v * sz], us: sx, vs: sz },          // bottom
    { n: 'x+', cell: [0.625, 0.0], f: (u, v) => [sx, v * sy, sz - u * sz], us: sz, vs: sy },     // right
    { n: 'x-', cell: [0.125, 0.0], f: (u, v) => [0, v * sy, u * sz], us: sz, vs: sy },           // left
  ];
  for (const s of sides) {
    for (let j = 0; j < s.vs; j++) for (let i = 0; i < s.us; i++) {
      const c = [[i, j], [i + 1, j], [i + 1, j + 1], [i, j + 1]];
      const verts = c.map(([a, b]) => vid(...s.f(a / s.us, b / s.vs).map(Math.round)));
      const uvs = c.map(([a, b]) => [s.cell[0] + a / s.us * 0.25, s.cell[1] + b / s.vs * 0.25]);
      pm.f.push(verts); pm.uv.push(uvs);
    }
  }
  pm.softAngle = 30;
  ensureOutward(pm);
  return pm;
}

export function polyPlane({ width = 1, height = 1, subdivisionsWidth = 10, subdivisionsHeight = 10 } = {}) {
  const sw = R(subdivisionsWidth, 1), sh = R(subdivisionsHeight, 1);
  const pm = new PolyMesh([], [], []);
  for (let j = 0; j <= sh; j++) for (let i = 0; i <= sw; i++) pm.v.push([(i / sw - 0.5) * width, 0, (0.5 - j / sh) * height]);
  for (let j = 0; j < sh; j++) for (let i = 0; i < sw; i++) {
    const a = j * (sw + 1) + i;
    pm.f.push([a, a + 1, a + sw + 2, a + sw + 1]);
    pm.uv.push([[i / sw, j / sh], [(i + 1) / sw, j / sh], [(i + 1) / sw, (j + 1) / sh], [i / sw, (j + 1) / sh]]);
  }
  return pm;
}

export function polySphere({ radius = 1, subdivisionsAxis = 20, subdivisionsHeight = 20 } = {}) {
  const A = R(subdivisionsAxis, 3), H = R(subdivisionsHeight, 2);
  const pm = new PolyMesh([], [], []);
  pm.v.push([0, radius, 0]); // top 0
  for (let j = 1; j < H; j++) {
    const th = j / H * Math.PI;
    for (let i = 0; i < A; i++) { const ph = i / A * TAU; pm.v.push([radius * Math.sin(th) * Math.cos(ph), radius * Math.cos(th), -radius * Math.sin(th) * Math.sin(ph)]); }
  }
  pm.v.push([0, -radius, 0]); const bot = pm.v.length - 1;
  const rv = (j, i) => 1 + (j - 1) * A + (i % A);
  for (let i = 0; i < A; i++) { // top cap: top, ring1(i), ring1(i+1)
    pm.f.push([0, rv(1, i), rv(1, i + 1)]);
    pm.uv.push([[(i + 0.5) / A, 1], [i / A, 1 - 1 / H], [(i + 1) / A, 1 - 1 / H]]);
  }
  for (let j = 1; j < H - 1; j++) for (let i = 0; i < A; i++) {
    pm.f.push([rv(j, i), rv(j + 1, i), rv(j + 1, i + 1), rv(j, i + 1)]);
    pm.uv.push([[i / A, 1 - j / H], [i / A, 1 - (j + 1) / H], [(i + 1) / A, 1 - (j + 1) / H], [(i + 1) / A, 1 - j / H]]);
  }
  for (let i = 0; i < A; i++) {
    pm.f.push([rv(H - 1, i), bot, rv(H - 1, i + 1)]);
    pm.uv.push([[i / A, 1 / H], [(i + 0.5) / A, 0], [(i + 1) / A, 1 / H]]);
  }
  pm.softAngle = 180;
  return pm;
}

function ringMesh(pm, profile, A, closedTop, closedBot) {
  // profile: array of [r, y] from top to bottom; revolve around Y with A segments
  const base = pm.v.length; const rows = profile.length;
  for (const [r, y] of profile) for (let i = 0; i < A; i++) { const ph = i / A * TAU; pm.v.push([r * Math.cos(ph), y, -r * Math.sin(ph)]); }
  const id = (j, i) => base + j * A + (i % A);
  const vlen = [0]; let acc = 0;
  for (let j = 1; j < rows; j++) { acc += Math.hypot(profile[j][0] - profile[j - 1][0], profile[j][1] - profile[j - 1][1]); vlen.push(acc); }
  const tot = acc || 1; const v0 = 0.3125, v1 = 0.6875;
  const U = (i) => 0.375 + 0.25 * i / A;
  for (let j = 0; j < rows - 1; j++) for (let i = 0; i < A; i++) {
    pm.f.push([id(j, i), id(j + 1, i), id(j + 1, i + 1), id(j, i + 1)]);
    const va = v1 - (vlen[j] / tot) * (v1 - v0), vb = v1 - (vlen[j + 1] / tot) * (v1 - v0);
    pm.uv.push([[U(i), va], [U(i), vb], [U(i + 1), vb], [U(i + 1), va]]);
  }
  const capUV = (i, cx, cy) => { const ph = i / A * TAU; return [cx + 0.15 * Math.cos(ph), cy + 0.15 * Math.sin(ph)]; };
  if (closedTop) { const f = [], u = []; for (let i = 0; i < A; i++) { f.push(id(0, i)); u.push(capUV(i, 0.5, 0.84)); } pm.f.push(f); pm.uv.push(u); fixCapOrientation(pm, pm.f.length - 1, 1); }
  if (closedBot) { const f = [], u = []; for (let i = 0; i < A; i++) { f.push(id(rows - 1, i)); u.push(capUV(i, 0.5, 0.16)); } pm.f.push(f); pm.uv.push(u); fixCapOrientation(pm, pm.f.length - 1, -1); }
  return { id };
}
function fixCapOrientation(pm, fi, wantY) {
  const n = pm.faceNormal(fi);
  if (Math.sign(n[1]) !== Math.sign(wantY)) { pm.f[fi].reverse(); if (pm.uv && pm.uv[fi]) pm.uv[fi].reverse(); }
}

export function polyCylinder({ radius = 1, height = 2, subdivisionsAxis = 20, subdivisionsHeight = 1, subdivisionsCaps = 0 } = {}) {
  const A = R(subdivisionsAxis, 3), H = R(subdivisionsHeight, 1), C = Math.max(0, Math.round(subdivisionsCaps || 0));
  const pm = new PolyMesh([], [], []);
  const prof = [];
  for (let c = 1; c <= C; c++) prof.push([radius * c / (C + 1), height / 2]);
  for (let j = 0; j <= H; j++) prof.push([radius, height / 2 - j / H * height]);
  for (let c = C; c >= 1; c--) prof.push([radius * c / (C + 1), -height / 2]);
  ringMesh(pm, prof, A, true, true);
  pm.softAngle = 60;
  return pm;
}

export function polyCone({ radius = 1, height = 2, subdivisionsAxis = 20, subdivisionsHeight = 1, subdivisionsCap = 0 } = {}) {
  const A = R(subdivisionsAxis, 3), H = R(subdivisionsHeight, 1), C = Math.max(0, Math.round(subdivisionsCap || 0));
  const pm = new PolyMesh([], [], []);
  pm.v.push([0, height / 2, 0]);
  const prof = [];
  for (let j = 1; j <= H; j++) prof.push([radius * j / H, height / 2 - j / H * height]);
  for (let c = C; c >= 1; c--) prof.push([radius * c / (C + 1), -height / 2]);
  const { id } = ringMesh(pm, prof, A, false, true);
  for (let i = 0; i < A; i++) { pm.f.push([0, id(0, i), id(0, i + 1)]); pm.uv.push([[0.5, 0.69], [0.375 + i / A * 0.25, 0.6], [0.375 + (i + 1) / A * 0.25, 0.6]]); }
  pm.softAngle = 60;
  return pm;
}

export function polyTorus({ radius = 1, sectionRadius = 0.5, twist = 0, subdivisionsAxis = 20, subdivisionsHeight = 20 } = {}) {
  const A = R(subdivisionsAxis, 3), H = R(subdivisionsHeight, 3);
  const pm = new PolyMesh([], [], []);
  for (let i = 0; i < A; i++) {
    const ph = i / A * TAU;
    for (let j = 0; j < H; j++) {
      const th = j / H * TAU + twist * Math.PI / 180;
      const r = radius + sectionRadius * Math.cos(th);
      pm.v.push([r * Math.cos(ph), sectionRadius * Math.sin(th), -r * Math.sin(ph)]);
    }
  }
  const id = (i, j) => (i % A) * H + (j % H);
  for (let i = 0; i < A; i++) for (let j = 0; j < H; j++) {
    pm.f.push([id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)]);
    pm.uv.push([[i / A, j / H], [(i + 1) / A, j / H], [(i + 1) / A, (j + 1) / H], [i / A, (j + 1) / H]]);
  }
  orientByVolume(pm);
  pm.softAngle = 180;
  return pm;
}

export function polyPyramid({ sideLength = 1.41421, numberOfSides = 4 } = {}) {
  const n = Math.max(3, Math.min(5, Math.round(numberOfSides)));
  const r = sideLength / (2 * Math.sin(Math.PI / n));
  const h = sideLength * 0.7071;
  const pm = new PolyMesh([], [], []);
  for (let i = 0; i < n; i++) { const ph = (i / n) * TAU + (n === 4 ? Math.PI / 4 : Math.PI / 2); pm.v.push([r * Math.cos(ph), -h / 2, -r * Math.sin(ph)]); }
  pm.v.push([0, h / 2, 0]);
  const base = []; for (let i = n - 1; i >= 0; i--) base.push(i);
  pm.f.push(base); pm.uv.push(base.map(i => { const ph = i / n * TAU; return [0.5 + 0.25 * Math.cos(ph), 0.25 + 0.25 * Math.sin(ph)]; }));
  for (let i = 0; i < n; i++) { pm.f.push([i, (i + 1) % n, n]); pm.uv.push([[i / n, 0.5], [(i + 1) / n, 0.5], [(i + 0.5) / n, 1]]); }
  ensureOutward(pm);
  pm.softAngle = 30;
  return pm;
}

export function polyPrism({ length = 2, sideLength = 2, numberOfSides = 3 } = {}) {
  const n = Math.max(3, Math.round(numberOfSides));
  const r = sideLength / (2 * Math.sin(Math.PI / n));
  const pm = new PolyMesh([], [], []);
  ringMesh(pm, [[r, length / 2], [r, -length / 2]], n, true, true);
  // rotate so a flat side faces front like Maya
  pm.softAngle = 30;
  return pm;
}

export function polyPipe({ radius = 1, height = 2, thickness = 0.5, subdivisionsAxis = 20, subdivisionsHeight = 1 } = {}) {
  const A = R(subdivisionsAxis, 3), H = R(subdivisionsHeight, 1);
  const ri = Math.max(0.001, radius - thickness);
  const pm = new PolyMesh([], [], []);
  const prof = [];
  for (let j = 0; j <= H; j++) prof.push([radius, height / 2 - j / H * height]);
  for (let j = H; j >= 0; j--) prof.push([ri, height / 2 - j / H * height]);
  prof.push([radius, height / 2]);
  // revolve but weld last ring to first
  const base = pm.v.length; const rows = prof.length - 1;
  for (let j = 0; j < rows; j++) for (let i = 0; i < A; i++) { const ph = i / A * TAU; pm.v.push([prof[j][0] * Math.cos(ph), prof[j][1], -prof[j][0] * Math.sin(ph)]); }
  const id = (j, i) => base + (j % rows) * A + (i % A);
  for (let j = 0; j < rows; j++) for (let i = 0; i < A; i++) {
    pm.f.push([id(j, i), id(j + 1, i), id(j + 1, i + 1), id(j, i + 1)]);
    pm.uv.push([[i / A, 1 - j / rows], [i / A, 1 - (j + 1) / rows], [(i + 1) / A, 1 - (j + 1) / rows], [(i + 1) / A, 1 - j / rows]]);
  }
  orientByVolume(pm);
  pm.softAngle = 60;
  return pm;
}

export function polyHelix({ coils = 3, height = 2, width = 2, radius = 0.4, subdivisionsAxis = 8, subdivisionsCoil = 50, direction = 1 } = {}) {
  const S = R(subdivisionsCoil * coils, 6), A = R(subdivisionsAxis, 3);
  const pm = new PolyMesh([], [], []);
  const W = width / 2;
  for (let s = 0; s <= S; s++) {
    const u = s / S; const ph = u * coils * TAU * (direction >= 0 ? 1 : -1);
    const c = [W * Math.cos(ph), -height / 2 + u * height, -W * Math.sin(ph)];
    const T = [-W * Math.sin(ph), height / (coils * TAU), -W * Math.cos(ph)]; const tl = Math.hypot(...T);
    const t = T.map(x => x / tl);
    const out = [Math.cos(ph), 0, -Math.sin(ph)];
    const b = [t[1] * out[2] - t[2] * out[1], t[2] * out[0] - t[0] * out[2], t[0] * out[1] - t[1] * out[0]];
    for (let a = 0; a < A; a++) {
      const th = a / A * TAU;
      pm.v.push([c[0] + radius * (Math.cos(th) * out[0] + Math.sin(th) * b[0]), c[1] + radius * (Math.cos(th) * out[1] + Math.sin(th) * b[1]), c[2] + radius * (Math.cos(th) * out[2] + Math.sin(th) * b[2])]);
    }
  }
  const id = (s, a) => s * A + (a % A);
  for (let s = 0; s < S; s++) for (let a = 0; a < A; a++) {
    pm.f.push([id(s, a), id(s, a + 1), id(s + 1, a + 1), id(s + 1, a)]);
    pm.uv.push([[s / S, a / A], [s / S, (a + 1) / A], [(s + 1) / S, (a + 1) / A], [(s + 1) / S, a / A]]);
  }
  const c0 = [], c1 = [];
  for (let a = 0; a < A; a++) { c0.push(id(0, a)); c1.push(id(S, a)); }
  pm.f.push(c0.reverse()); pm.uv.push(c0.map((_, a) => [0.5 + 0.2 * Math.cos(a / A * TAU), 0.5 + 0.2 * Math.sin(a / A * TAU)]));
  pm.f.push(c1); pm.uv.push(c1.map((_, a) => [0.5 + 0.2 * Math.cos(a / A * TAU), 0.5 + 0.2 * Math.sin(a / A * TAU)]));
  orientByVolume(pm);
  pm.softAngle = 60;
  return pm;
}

export function polyDisc({ radius = 1, sides = 20, subdivisions = 1 } = {}) {
  const n = R(sides, 3), S = R(subdivisions, 1);
  const pm = new PolyMesh([], [], []);
  if (S === 1) {
    const f = [], u = [];
    for (let i = 0; i < n; i++) { const ph = i / n * TAU; pm.v.push([radius * Math.cos(ph), 0, -radius * Math.sin(ph)]); f.push(i); u.push([0.5 + 0.5 * Math.cos(ph), 0.5 + 0.5 * Math.sin(ph)]); }
    pm.f.push(f); pm.uv.push(u);
  } else {
    pm.v.push([0, 0, 0]);
    for (let s = 1; s <= S; s++) for (let i = 0; i < n; i++) { const ph = i / n * TAU; const r = radius * s / S; pm.v.push([r * Math.cos(ph), 0, -r * Math.sin(ph)]); }
    const id = (s, i) => s === 0 ? 0 : 1 + (s - 1) * n + (i % n);
    const uv = (s, i) => { const ph = i / n * TAU; const r = 0.5 * s / S; return [0.5 + r * Math.cos(ph), 0.5 + r * Math.sin(ph)]; };
    for (let i = 0; i < n; i++) { pm.f.push([0, id(1, i), id(1, i + 1)]); pm.uv.push([[0.5, 0.5], uv(1, i), uv(1, i + 1)]); }
    for (let s = 1; s < S; s++) for (let i = 0; i < n; i++) { pm.f.push([id(s, i), id(s + 1, i), id(s + 1, i + 1), id(s, i + 1)]); pm.uv.push([uv(s, i), uv(s + 1, i), uv(s + 1, i + 1), uv(s, i + 1)]); }
  }
  ensureUp(pm);
  return pm;
}

export function polyPlatonic({ primitive = 2, radius = 1 } = {}) {
  // 0 tetrahedron, 1 octahedron, 2 icosahedron, 3 dodecahedron
  const pm = new PolyMesh([], [], null);
  const p = (Math.sqrt(5) + 1) / 2;
  let V, F;
  if (primitive === 0) { V = [[1, 1, 1], [-1, -1, 1], [-1, 1, -1], [1, -1, -1]]; F = [[0, 1, 3], [0, 2, 1], [0, 3, 2], [1, 2, 3]]; }
  else if (primitive === 1) { V = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]]; F = [[0, 2, 4], [2, 1, 4], [1, 3, 4], [3, 0, 4], [2, 0, 5], [1, 2, 5], [3, 1, 5], [0, 3, 5]]; }
  else {
    V = [[-1, p, 0], [1, p, 0], [-1, -p, 0], [1, -p, 0], [0, -1, p], [0, 1, p], [0, -1, -p], [0, 1, -p], [p, 0, -1], [p, 0, 1], [-p, 0, -1], [-p, 0, 1]];
    F = [[0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11], [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8], [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9], [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1]];
    if (primitive === 3) {
      // dual: vertices = face centres of icosahedron, faces = around each icosa vertex
      const cen = F.map(f => { const c = [0, 0, 0]; for (const i of f) for (let k = 0; k < 3; k++) c[k] += V[i][k] / 3; return c; });
      const nf = [];
      for (let vi = 0; vi < V.length; vi++) {
        const around = F.map((f, fi) => [f, fi]).filter(([f]) => f.includes(vi));
        // order faces around vertex
        const ordered = [around[0]]; const used = new Set([around[0][1]]);
        while (ordered.length < around.length) {
          const [lf] = ordered[ordered.length - 1]; const i = lf.indexOf(vi); const nxt = lf[(i + 2) % 3];
          const cand = around.find(([f, fi]) => !used.has(fi) && f.includes(nxt));
          if (!cand) break; ordered.push(cand); used.add(cand[1]);
        }
        nf.push(ordered.map(([, fi]) => fi));
      }
      V = cen; F = nf;
    }
  }
  for (const v of V) { const l = Math.hypot(...v); pm.v.push(v.map(x => x / l * radius)); }
  pm.f = F.map(f => f.slice());
  ensureOutward(pm);
  pm.uv = pm.f.map(f => f.map((_, i) => [0.5 + 0.5 * Math.cos(i / f.length * TAU), 0.5 + 0.5 * Math.sin(i / f.length * TAU)]));
  pm.softAngle = 20;
  return pm;
}

// make all faces point away from the centroid (works for star-shaped primitives)
function ensureOutward(pm) {
  const c = [0, 0, 0]; for (const p of pm.v) for (let k = 0; k < 3; k++) c[k] += p[k] / pm.v.length;
  pm.f.forEach((f, fi) => {
    const n = pm.faceNormal(fi); const fc = pm.faceCenter(fi);
    let d = (fc[0] - c[0]) * n[0] + (fc[1] - c[1]) * n[1] + (fc[2] - c[2]) * n[2];
    if (d < 0) { pm.f[fi] = f.slice().reverse(); if (pm.uv && pm.uv[fi]) pm.uv[fi] = pm.uv[fi].slice().reverse(); }
  });
  pm.invalidate();
}
function ensureUp(pm) {
  pm.f.forEach((f, fi) => { if (pm.faceNormal(fi)[1] < 0) { pm.f[fi] = f.slice().reverse(); if (pm.uv && pm.uv[fi]) pm.uv[fi] = pm.uv[fi].slice().reverse(); } });
}

// --------- NURBS-style surfaces: smooth tessellations that show only isoparms as wire -----------
function withIso(pm, rowsEvery, colsEvery, rowCount, colCount, idFn, wrapCols = true) {
  const de = [];
  for (let j = 0; j < rowCount; j += rowsEvery) for (let i = 0; i < colCount; i++) { const a = idFn(j, i), b = idFn(j, wrapCols ? (i + 1) % colCount : i + 1); if (a !== undefined && b !== undefined && a !== b) de.push([a, b]); }
  for (let i = 0; i < colCount; i += colsEvery) for (let j = 0; j + 1 < rowCount; j++) { const a = idFn(j, i), b = idFn(j + 1, i); if (a !== undefined && b !== undefined && a !== b) de.push([a, b]); }
  pm.displayEdges = de;
  return pm;
}
export function nurbsSphere({ radius = 1, sections = 8, spans = 4 } = {}) {
  const k = 4; const A = sections * k, H = spans * k;
  const pm = polySphere({ radius, subdivisionsAxis: A, subdivisionsHeight: H });
  const id = (j, i) => j === 0 ? 0 : j === H ? pm.v.length - 1 : 1 + (j - 1) * A + (i % A);
  const de = [];
  for (let j = k; j < H; j += k) for (let i = 0; i < A; i++) de.push([id(j, i), id(j, i + 1)]);
  for (let i = 0; i < A; i += k) for (let j = 0; j < H; j++) de.push([id(j, i), id(j + 1, i)]);
  pm.displayEdges = de; pm.softAngle = 180;
  return pm;
}
export function nurbsCylinder({ radius = 1, height = 2, sections = 8, spans = 1 } = {}) {
  const k = 4; const A = sections * k;
  const pm = new PolyMesh([], [], []);
  const prof = []; for (let j = 0; j <= spans; j++) prof.push([radius, height / 2 - j / spans * height]);
  ringMesh(pm, prof, A, false, false);
  withIso(pm, 1, k, spans + 1, A, (j, i) => j * A + (i % A));
  pm.softAngle = 180;
  return pm;
}
export function nurbsCone({ radius = 1, height = 2, sections = 8, spans = 1 } = {}) {
  const k = 4; const A = sections * k; const S = spans * k;
  const pm = new PolyMesh([], [], []);
  const prof = []; for (let j = 0; j <= S; j++) prof.push([Math.max(1e-4, radius * j / S), height / 2 - j / S * height]);
  ringMesh(pm, prof, A, false, false);
  withIso(pm, k, k, S + 1, A, (j, i) => j * A + (i % A));
  pm.softAngle = 180;
  return pm;
}
export function nurbsTorus({ radius = 1, sectionRadius = 0.5, sections = 8, spans = 4 } = {}) {
  const k = 4; const A = sections * k, H = spans * k;
  const pm = polyTorus({ radius, sectionRadius, subdivisionsAxis: A, subdivisionsHeight: H });
  const de = []; const id = (i, j) => (i % A) * H + (j % H);
  for (let i = 0; i < A; i += k) for (let j = 0; j < H; j++) de.push([id(i, j), id(i, j + 1)]);
  for (let j = 0; j < H; j += k) for (let i = 0; i < A; i++) de.push([id(i, j), id(i + 1, j)]);
  pm.displayEdges = de;
  return pm;
}
export function nurbsPlane({ width = 1, lengthRatio = 1, patchesU = 1, patchesV = 1 } = {}) {
  const k = 4;
  const pm = polyPlane({ width, height: width * lengthRatio, subdivisionsWidth: patchesU * k, subdivisionsHeight: patchesV * k });
  const W = patchesU * k + 1, H = patchesV * k + 1;
  const de = []; const id = (j, i) => j * W + i;
  for (let j = 0; j < H; j += k) for (let i = 0; i + 1 < W; i++) de.push([id(j, i), id(j, i + 1)]);
  for (let i = 0; i < W; i += k) for (let j = 0; j + 1 < H; j++) de.push([id(j, i), id(j + 1, i)]);
  pm.displayEdges = de;
  return pm;
}
export function nurbsCube({ width = 1, lengthRatio = 1, heightRatio = 1, patchesU = 1, patchesV = 1 } = {}) {
  const pm = polyCube({ width, height: width * heightRatio, depth: width * lengthRatio, subdivisionsWidth: patchesU, subdivisionsHeight: patchesV, subdivisionsDepth: patchesU });
  return pm;
}

export const GENERATORS = {
  polyCube, polyPlane, polySphere, polyCylinder, polyCone, polyTorus, polyPyramid, polyPrism, polyPipe, polyHelix, polyDisc, polyPlatonic,
  nurbsSphere, nurbsCylinder, nurbsCone, nurbsTorus, nurbsPlane, nurbsCube,
};

export const GEN_INFO = {
  polyCube: { base: 'pCube', params: { width: 1, height: 1, depth: 1, subdivisionsWidth: 1, subdivisionsHeight: 1, subdivisionsDepth: 1 }, short: { w: 'width', h: 'height', d: 'depth', sx: 'subdivisionsWidth', sy: 'subdivisionsHeight', sz: 'subdivisionsDepth' } },
  polySphere: { base: 'pSphere', params: { radius: 1, subdivisionsAxis: 20, subdivisionsHeight: 20 }, short: { r: 'radius', sa: 'subdivisionsAxis', sh: 'subdivisionsHeight', sx: 'subdivisionsAxis', sy: 'subdivisionsHeight' } },
  polyCylinder: { base: 'pCylinder', params: { radius: 1, height: 2, subdivisionsAxis: 20, subdivisionsHeight: 1, subdivisionsCaps: 0 }, short: { r: 'radius', h: 'height', sa: 'subdivisionsAxis', sh: 'subdivisionsHeight', sc: 'subdivisionsCaps', sx: 'subdivisionsAxis', sy: 'subdivisionsHeight' } },
  polyCone: { base: 'pCone', params: { radius: 1, height: 2, subdivisionsAxis: 20, subdivisionsHeight: 1, subdivisionsCap: 0 }, short: { r: 'radius', h: 'height', sa: 'subdivisionsAxis', sh: 'subdivisionsHeight', sc: 'subdivisionsCap', sx: 'subdivisionsAxis', sy: 'subdivisionsHeight' } },
  polyPlane: { base: 'pPlane', params: { width: 1, height: 1, subdivisionsWidth: 10, subdivisionsHeight: 10 }, short: { w: 'width', h: 'height', sx: 'subdivisionsWidth', sy: 'subdivisionsHeight' } },
  polyTorus: { base: 'pTorus', params: { radius: 1, sectionRadius: 0.5, twist: 0, subdivisionsAxis: 20, subdivisionsHeight: 20 }, short: { r: 'radius', sr: 'sectionRadius', tw: 'twist', sa: 'subdivisionsAxis', sh: 'subdivisionsHeight', sx: 'subdivisionsAxis', sy: 'subdivisionsHeight' } },
  polyPyramid: { base: 'pPyramid', params: { sideLength: 1.41421, numberOfSides: 4 }, short: { w: 'sideLength', ns: 'numberOfSides' } },
  polyPrism: { base: 'pPrism', params: { length: 2, sideLength: 2, numberOfSides: 3 }, short: { l: 'length', w: 'sideLength', ns: 'numberOfSides' } },
  polyPipe: { base: 'pPipe', params: { radius: 1, height: 2, thickness: 0.5, subdivisionsAxis: 20, subdivisionsHeight: 1 }, short: { r: 'radius', h: 'height', t: 'thickness', sa: 'subdivisionsAxis', sh: 'subdivisionsHeight' } },
  polyHelix: { base: 'pHelix', params: { coils: 3, height: 2, width: 2, radius: 0.4, subdivisionsAxis: 8, subdivisionsCoil: 50, direction: 1 }, short: { c: 'coils', h: 'height', w: 'width', r: 'radius', sa: 'subdivisionsAxis', sco: 'subdivisionsCoil', d: 'direction' } },
  polyDisc: { base: 'pDisc', params: { radius: 1, sides: 20, subdivisions: 1 }, short: { r: 'radius', s: 'sides', sd: 'subdivisions' } },
  polyPlatonic: { base: 'pSolid', params: { primitive: 2, radius: 1 }, short: { p: 'primitive', r: 'radius' } },
  nurbsSphere: { base: 'nurbsSphere', nurbs: true, params: { radius: 1, sections: 8, spans: 4 }, short: { r: 'radius', s: 'sections', nsp: 'spans' } },
  nurbsCylinder: { base: 'nurbsCylinder', nurbs: true, params: { radius: 1, height: 2, sections: 8, spans: 1 }, short: { r: 'radius', hr: 'height', s: 'sections', nsp: 'spans' } },
  nurbsCone: { base: 'nurbsCone', nurbs: true, params: { radius: 1, height: 2, sections: 8, spans: 1 }, short: { r: 'radius', hr: 'height', s: 'sections', nsp: 'spans' } },
  nurbsTorus: { base: 'nurbsTorus', nurbs: true, params: { radius: 1, sectionRadius: 0.5, sections: 8, spans: 4 }, short: { r: 'radius', s: 'sections', nsp: 'spans' } },
  nurbsPlane: { base: 'nurbsPlane', nurbs: true, params: { width: 1, lengthRatio: 1, patchesU: 1, patchesV: 1 }, short: { w: 'width', lr: 'lengthRatio', u: 'patchesU', v: 'patchesV' } },
  nurbsCube: { base: 'nurbsCube', nurbs: true, params: { width: 1, lengthRatio: 1, heightRatio: 1, patchesU: 1, patchesV: 1 }, short: { w: 'width', lr: 'lengthRatio', hr: 'heightRatio', u: 'patchesU', v: 'patchesV' } },
};
