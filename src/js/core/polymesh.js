// Inca — PolyMesh: an n-gon polygon mesh with per-face-corner UVs and per-face materials,
// plus all of the polygon modeling operations (extrude, bevel, edge loops, smooth, ...).
import * as THREE from 'three';

const KM = 2097152; // 2^21 — vertex index multiplier for numeric edge keys
export const ekey = (a, b) => (a < b ? a * KM + b : b * KM + a);
export const dkey = (a, b) => a * KM + b; // directed
export const ekeyVerts = (k) => [Math.floor(k / KM), k % KM];

const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const add = (a, b) => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const mul = (a, s) => [a[0] * s, a[1] * s, a[2] * s];
const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const len = (a) => Math.hypot(a[0], a[1], a[2]);
const norm = (a) => { const l = len(a) || 1; return [a[0] / l, a[1] / l, a[2] / l]; };
const lerp3 = (a, b, t) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
const lerp2 = (a, b, t) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
export const V3 = { sub, add, mul, dot, cross, len, norm, lerp3 };

export class PolyMesh {
  constructor(v = [], f = [], uv = null, fm = null) {
    this.v = v;          // [[x,y,z], ...]
    this.f = f;          // [[i0,i1,i2,...], ...] counter-clockwise seen from outside
    this.uv = uv;        // per face: [[u,v], ...] (same length as face) or null
    this.fm = fm;        // per face material id or null
    this.softAngle = 60; // edges with dihedral angle below this are smooth-shaded
    this.edgeHard = null;// {edgeKey: true|false} explicit overrides
    this.displayEdges = null; // optional subset of edges shown as wireframe (NURBS isoparms)
    this._topo = null;
  }
  clone() {
    const m = new PolyMesh(this.v.map(p => p.slice()), this.f.map(f => f.slice()),
      this.uv ? this.uv.map(u => u ? u.map(c => c.slice()) : null) : null, this.fm ? this.fm.slice() : null);
    m.softAngle = this.softAngle;
    m.edgeHard = this.edgeHard ? { ...this.edgeHard } : null;
    m.displayEdges = this.displayEdges ? this.displayEdges.map(e => e.slice()) : null;
    return m;
  }
  toJSON() {
    return { v: this.v.map(p => p.map(n => +n.toFixed(6))), f: this.f, uv: this.uv ? this.uv.map(u => u ? u.map(c => [+c[0].toFixed(5), +c[1].toFixed(5)]) : null) : null,
      fm: this.fm, softAngle: this.softAngle, edgeHard: this.edgeHard, displayEdges: this.displayEdges };
  }
  static fromJSON(d) {
    const m = new PolyMesh(d.v.map(p => p.slice()), d.f.map(f => f.slice()), d.uv ? d.uv.map(u => u ? u.map(c => c.slice()) : null) : null, d.fm ? d.fm.slice() : null);
    m.softAngle = d.softAngle ?? 60; m.edgeHard = d.edgeHard || null; m.displayEdges = d.displayEdges || null;
    return m;
  }
  invalidate() { this._topo = null; this._smooth = null; }

  addFace(verts, uvs = null, mat = null) {
    this.f.push(verts);
    if (this.uv || uvs) {
      if (!this.uv) this.uv = this.f.slice(0, -1).map(() => null);
      this.uv.push(uvs);
    }
    if (this.fm || mat != null) {
      if (!this.fm) this.fm = this.f.slice(0, -1).map(() => null);
      this.fm.push(mat ?? null);
    }
    return this.f.length - 1;
  }

  get topo() {
    if (this._topo) return this._topo;
    const edges = [], edgeMap = new Map(), edgeFaces = [], vertFaces = this.v.map(() => []), vertEdges = this.v.map(() => []);
    const dmap = new Map();
    this.f.forEach((face, fi) => {
      const n = face.length;
      for (let i = 0; i < n; i++) {
        const a = face[i], b = face[(i + 1) % n];
        vertFaces[a].push(fi);
        dmap.set(dkey(a, b), fi);
        const k = ekey(a, b);
        let ei = edgeMap.get(k);
        if (ei === undefined) {
          ei = edges.length; edges.push(a < b ? [a, b] : [b, a]); edgeMap.set(k, ei); edgeFaces.push([]);
          vertEdges[a].push(ei); vertEdges[b].push(ei);
        }
        edgeFaces[ei].push(fi);
      }
    });
    this._topo = { edges, edgeMap, edgeFaces, vertFaces, vertEdges, dmap };
    return this._topo;
  }
  edgeIndex(a, b) { const i = this.topo.edgeMap.get(ekey(a, b)); return i === undefined ? -1 : i; }

  faceNormal(fi) {
    const f = this.f[fi], v = this.v; let nx = 0, ny = 0, nz = 0;
    for (let i = 0; i < f.length; i++) {
      const a = v[f[i]], b = v[f[(i + 1) % f.length]];
      nx += (a[1] - b[1]) * (a[2] + b[2]); ny += (a[2] - b[2]) * (a[0] + b[0]); nz += (a[0] - b[0]) * (a[1] + b[1]);
    }
    return [nx, ny, nz]; // length = 2 * area
  }
  faceCenter(fi) {
    const f = this.f[fi]; const c = [0, 0, 0];
    for (const i of f) { c[0] += this.v[i][0]; c[1] += this.v[i][1]; c[2] += this.v[i][2]; }
    return mul(c, 1 / f.length);
  }
  bbox() {
    const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
    for (const p of this.v) for (let k = 0; k < 3; k++) { if (p[k] < min[k]) min[k] = p[k]; if (p[k] > max[k]) max[k] = p[k]; }
    if (!this.v.length) return { min: [0, 0, 0], max: [0, 0, 0] };
    return { min, max };
  }
  vertUV() { // one representative UV per vertex
    const out = this.v.map(() => null);
    if (!this.uv) return out;
    this.f.forEach((f, fi) => { const u = this.uv[fi]; if (u) f.forEach((v, i) => { if (!out[v]) out[v] = u[i]; }); });
    return out;
  }
  isEdgeSoft(ei, fn) {
    const t = this.topo; const k = ekey(...t.edges[ei]);
    if (this.edgeHard && k in this.edgeHard) return !this.edgeHard[k];
    const fs = t.edgeFaces[ei]; if (fs.length !== 2) return false;
    const a = norm(fn[fs[0]]), b = norm(fn[fs[1]]);
    const ang = Math.acos(Math.max(-1, Math.min(1, dot(a, b)))) * 180 / Math.PI;
    return ang <= this.softAngle + 1e-4;
  }
  stats() {
    let tris = 0; for (const f of this.f) tris += f.length - 2;
    let uvs = 0; if (this.uv) for (const u of this.uv) if (u) uvs += u.length;
    return { verts: this.v.length, edges: this.topo.edges.length, faces: this.f.length, tris, uvs };
  }
}

// ---------------------------------------------------------------------------
// Triangulation of a single face (indices local to the face)
export function triangulateFace(pts, normal) {
  const n = pts.length;
  if (n === 3) return [[0, 1, 2]];
  if (n === 4) {
    const d02 = len(sub(pts[0], pts[2])), d13 = len(sub(pts[1], pts[3]));
    // avoid splitting through a reflex corner
    const nn = normal || [0, 1, 0];
    const conv = (i) => dot(cross(sub(pts[i], pts[(i + 3) % 4]), sub(pts[(i + 1) % 4], pts[i])), nn) >= 0;
    const ok02 = conv(1) && conv(3) || !(conv(0) && conv(2));
    if ((d02 <= d13 && ok02) || !(conv(0) && conv(2))) return [[0, 1, 2], [0, 2, 3]];
    return [[1, 2, 3], [1, 3, 0]];
  }
  const N = norm(normal || [0, 1, 0]);
  let ax = Math.abs(N[0]) < 0.9 ? [1, 0, 0] : [0, 1, 0];
  const U = norm(cross(ax, N)), W = cross(N, U);
  const c2 = pts.map(p => new THREE.Vector2(dot(p, U), dot(p, W)));
  let tris;
  try { tris = THREE.ShapeUtils.triangulateShape(c2, []); } catch (e) { tris = []; }
  if (!tris.length) { tris = []; for (let i = 1; i < n - 1; i++) tris.push([0, i, i + 1]); }
  // fix winding to match the face normal
  return tris.map(t => {
    const tn = cross(sub(pts[t[1]], pts[t[0]]), sub(pts[t[2]], pts[t[0]]));
    return dot(tn, N) < 0 ? [t[0], t[2], t[1]] : t;
  });
}

// ---------------------------------------------------------------------------
// Build render geometry. Returns { geometry, triFace } — geometry is non-indexed with groups per material.
export function buildGeometry(pm, matIndexOf = null) {
  const t = pm.topo;
  const fnRaw = pm.f.map((_, i) => pm.faceNormal(i));
  const fn = fnRaw.map(norm);
  // corner normals via smoothing groups around each vertex
  const cornerN = pm.f.map(f => new Array(f.length));
  const vf = t.vertFaces;
  for (let v = 0; v < pm.v.length; v++) {
    const faces = [...new Set(vf[v])];
    if (!faces.length) continue;
    const parent = new Map(faces.map(f => [f, f]));
    const find = (x) => { while (parent.get(x) !== x) { parent.set(x, parent.get(parent.get(x))); x = parent.get(x); } return x; };
    for (const ei of t.vertEdges[v]) {
      const fs = t.edgeFaces[ei];
      if (fs.length === 2 && pm.isEdgeSoft(ei, fnRaw)) { const a = find(fs[0]), b = find(fs[1]); if (a !== b) parent.set(a, b); }
    }
    const groupN = new Map();
    for (const f of faces) {
      const face = pm.f[f]; const i = face.indexOf(v);
      const p = pm.v[face[(i + face.length - 1) % face.length]], q = pm.v[face[(i + 1) % face.length]], o = pm.v[v];
      const e1 = norm(sub(p, o)), e2 = norm(sub(q, o));
      const ang = Math.acos(Math.max(-1, Math.min(1, dot(e1, e2)))) || 1e-3;
      const r = find(f); const g = groupN.get(r) || [0, 0, 0];
      groupN.set(r, add(g, mul(fn[f], ang)));
    }
    for (const f of faces) {
      const face = pm.f[f]; const r = norm(groupN.get(find(f)));
      face.forEach((vv, i) => { if (vv === v) cornerN[f][i] = r; });
    }
  }
  // group faces by material
  const order = pm.f.map((_, i) => i);
  let matKeys = null;
  if (matIndexOf) {
    const mi = pm.f.map((_, i) => matIndexOf(pm.fm ? pm.fm[i] : null));
    order.sort((a, b) => mi[a] - mi[b]);
    matKeys = mi;
  }
  const pos = [], nor = [], uvs = [], triFace = [];
  const groups = [];
  let curMat = -1, groupStart = 0;
  for (const fi of order) {
    const face = pm.f[fi];
    if (matKeys && matKeys[fi] !== curMat) {
      if (curMat !== -1) groups.push({ start: groupStart, count: pos.length / 3 - groupStart, materialIndex: curMat });
      curMat = matKeys[fi]; groupStart = pos.length / 3;
    }
    const pts = face.map(i => pm.v[i]);
    const tris = triangulateFace(pts, fnRaw[fi]);
    const fuv = pm.uv && pm.uv[fi];
    for (const tri of tris) {
      for (const c of tri) {
        const p = pts[c]; pos.push(p[0], p[1], p[2]);
        const nn = cornerN[fi][c] || fn[fi]; nor.push(nn[0], nn[1], nn[2]);
        if (fuv && fuv[c]) uvs.push(fuv[c][0], fuv[c][1]); else uvs.push(0, 0);
      }
      triFace.push(fi);
    }
  }
  if (matKeys && curMat !== -1) groups.push({ start: groupStart, count: pos.length / 3 - groupStart, materialIndex: curMat });
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  g.setAttribute('normal', new THREE.Float32BufferAttribute(nor, 3));
  g.setAttribute('uv', new THREE.Float32BufferAttribute(uvs, 2));
  for (const gr of groups) g.addGroup(gr.start, gr.count, gr.materialIndex);
  if (!matKeys) g.addGroup(0, pos.length / 3, 0);
  g.computeBoundingSphere(); g.computeBoundingBox();
  return { geometry: g, triFace: Int32Array.from(triFace) };
}

export function buildEdgeGeometry(pm, onlyDisplay = true) {
  const pos = [];
  const list = (onlyDisplay && pm.displayEdges) ? pm.displayEdges : pm.topo.edges;
  for (const [a, b] of list) { const p = pm.v[a], q = pm.v[b]; if (!p || !q) continue; pos.push(p[0], p[1], p[2], q[0], q[1], q[2]); }
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  g.computeBoundingSphere();
  return g;
}

// ---------------------------------------------------------------------------
// Utilities
export function compact(pm) {
  const used = new Int32Array(pm.v.length).fill(-1); const nv = [];
  for (const f of pm.f) for (const i of f) if (used[i] < 0) { used[i] = nv.length; nv.push(pm.v[i]); }
  pm.f = pm.f.map(f => f.map(i => used[i]));
  if (pm.displayEdges) pm.displayEdges = pm.displayEdges.filter(([a, b]) => used[a] >= 0 && used[b] >= 0).map(([a, b]) => [used[a], used[b]]);
  if (pm.edgeHard) {
    const eh = {};
    for (const k in pm.edgeHard) { const [a, b] = ekeyVerts(+k); if (used[a] >= 0 && used[b] >= 0) eh[ekey(used[a], used[b])] = pm.edgeHard[k]; }
    pm.edgeHard = eh;
  }
  pm.v = nv; pm.invalidate();
  return used;
}
// remove repeated consecutive vertices & degenerate faces
export function cleanFaces(pm) {
  const nf = [], nuv = pm.uv ? [] : null, nfm = pm.fm ? [] : null;
  pm.f.forEach((f, fi) => {
    const keep = []; const ku = [];
    const u = pm.uv && pm.uv[fi];
    for (let i = 0; i < f.length; i++) {
      if (f[i] !== f[(i + 1) % f.length]) { keep.push(f[i]); if (u) ku.push(u[i]); }
    }
    if (keep.length >= 3 && new Set(keep).size >= 3) { nf.push(keep); if (nuv) nuv.push(u ? ku : null); if (nfm) nfm.push(pm.fm[fi]); }
  });
  pm.f = nf; if (nuv) pm.uv = nuv; if (nfm) pm.fm = nfm; pm.invalidate();
}
function removeFaces(pm, faceSet) {
  const keep = (_, i) => !faceSet.has(i);
  pm.f = pm.f.filter(keep); if (pm.uv) pm.uv = pm.uv.filter(keep); if (pm.fm) pm.fm = pm.fm.filter(keep);
  pm.invalidate();
}
function uvLookup(pm) { // vertex -> uv
  return pm.vertUV();
}
function faceUVFrom(vuv, verts) {
  if (!vuv) return null;
  return verts.map(v => (vuv[v] ? vuv[v].slice() : [0, 0]));
}
function pushVert(pm, p, vuv, uv) { pm.v.push(p); if (vuv) vuv.push(uv || [0, 0]); return pm.v.length - 1; }

// boundary loops (as directed vertex cycles of the *missing* faces)
export function boundaryLoops(pm, filter = null) {
  const t = pm.topo; const next = new Map();
  pm.f.forEach(f => {
    for (let i = 0; i < f.length; i++) {
      const a = f[i], b = f[(i + 1) % f.length];
      if (!t.dmap.has(dkey(b, a))) { if (!filter || filter(a, b)) next.set(b, a); }
    }
  });
  const loops = []; const seen = new Set();
  for (const start of next.keys()) {
    if (seen.has(start)) continue;
    const loop = []; let cur = start; let guard = 0;
    while (cur !== undefined && !seen.has(cur) && guard++ < 100000) { seen.add(cur); loop.push(cur); cur = next.get(cur); }
    if (loop.length >= 3 && cur === start) loops.push(loop);
  }
  return loops;
}

// ---------------------------------------------------------------------------
// OPERATIONS. Each takes a PolyMesh (not modified) and returns { mesh, select? }
// select: { type:'f'|'e'|'v', ids:[...] } or { type:'e', pairs:[[a,b],...] }

export function opTweak(pm0, { verts, deltas }) {
  const pm = pm0.clone();
  for (let i = 0; i < verts.length; i++) {
    const p = pm.v[verts[i]]; if (!p) continue;
    p[0] += deltas[i * 3]; p[1] += deltas[i * 3 + 1]; p[2] += deltas[i * 3 + 2];
  }
  return { mesh: pm };
}

export function opSetPositions(pm0, { verts, pos }) {
  const pm = pm0.clone();
  for (let i = 0; i < verts.length; i++) if (pm.v[verts[i]]) pm.v[verts[i]] = [pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]];
  return { mesh: pm };
}

function extrudeRegion(pm, sel, thickness, offset, vuv) {
  const t = pm.topo;
  const vmap = new Map();
  const vnorm = new Map(); const vcent = new Map(); const vcount = new Map();
  for (const f of sel) {
    const n = norm(pm.faceNormal(f)); const c = pm.faceCenter(f);
    for (const v of pm.f[f]) {
      if (!vmap.has(v)) vmap.set(v, pushVert(pm, pm.v[v].slice(), vuv, vuv && vuv[v] ? vuv[v].slice() : null));
      vnorm.set(v, add(vnorm.get(v) || [0, 0, 0], n));
      vcent.set(v, add(vcent.get(v) || [0, 0, 0], c)); vcount.set(v, (vcount.get(v) || 0) + 1);
    }
  }
  const sides = [];
  for (const f of sel) {
    const face = pm.f[f]; const n = face.length;
    for (let i = 0; i < n; i++) {
      const a = face[i], b = face[(i + 1) % n];
      const opp = t.dmap.get(dkey(b, a));
      if (opp === undefined || !sel.has(opp)) {
        sides.push({ verts: [a, b, vmap.get(b), vmap.get(a)], src: f, uvA: pm.uv && pm.uv[f] ? pm.uv[f][i] : null, uvB: pm.uv && pm.uv[f] ? pm.uv[f][(i + 1) % n] : null });
      }
    }
  }
  for (const f of sel) pm.f[f] = pm.f[f].map(v => vmap.get(v));
  for (const [v, nv] of vmap) {
    let nrm = norm(vnorm.get(v));
    // keep wall thickness roughly constant on corners
    const p = pm.v[nv];
    let np = add(p, mul(nrm, thickness));
    if (offset) {
      const c = mul(vcent.get(v), 1 / vcount.get(v));
      const dir = sub(c, p);
      np = add(np, mul(dir, Math.max(-1, Math.min(1, offset))));
    }
    pm.v[nv] = np;
  }
  for (const s of sides) {
    let uvs = null;
    if (pm.uv) {
      const ua = s.uvA || [0, 0], ub = s.uvB || [0, 0];
      uvs = [ua.slice(), ub.slice(), ub.slice(), ua.slice()];
      // give the side a little UV height so it isn't degenerate
      const d = [ub[0] - ua[0], ub[1] - ua[1]]; const perp = [-d[1] * 0.25, d[0] * 0.25];
      uvs[2] = [ub[0] + perp[0], ub[1] + perp[1]]; uvs[3] = [ua[0] + perp[0], ua[1] + perp[1]];
    }
    pm.addFace(s.verts, uvs, pm.fm ? pm.fm[s.src] : null);
  }
  pm.invalidate();
}

export function opExtrudeFaces(pm0, { faces, thickness = 0, offset = 0, divisions = 1, keepFacesTogether = true }) {
  const pm = pm0.clone();
  const fs = faces.filter(f => f < pm.f.length);
  if (!fs.length) return { mesh: pm };
  const div = Math.max(1, Math.round(divisions));
  for (let d = 0; d < div; d++) {
    const vuv = null;
    const off = d === 0 ? offset : 0;
    if (keepFacesTogether) extrudeRegion(pm, new Set(fs), thickness / div, off, vuv);
    else for (const f of fs) extrudeRegion(pm, new Set([f]), thickness / div, off, vuv);
  }
  return { mesh: pm, select: { type: 'f', ids: fs } };
}

export function opExtrudeEdges(pm0, { edges, thickness = 0, offset = 0 }) {
  // extrude border/open edges into new quads, moving the new edge along the average face plane outward
  const pm = pm0.clone(); const t = pm.topo;
  const vmap = new Map(); const newPairs = [];
  const dirs = new Map();
  for (const [a, b] of edges) {
    const ei = pm.edgeIndex(a, b); if (ei < 0) continue;
    const fs = t.edgeFaces[ei];
    let n = [0, 1, 0];
    if (fs.length) { const fn = norm(pm.faceNormal(fs[0])); const c = pm.faceCenter(fs[0]); const mid = lerp3(pm.v[a], pm.v[b], 0.5); const outward = norm(sub(mid, c)); n = fs.length === 1 ? outward : fn; }
    for (const v of [a, b]) dirs.set(v, add(dirs.get(v) || [0, 0, 0], n));
  }
  for (const [a, b] of edges) {
    const ei = pm.edgeIndex(a, b); if (ei < 0) continue;
    for (const v of [a, b]) if (!vmap.has(v)) { pm.v.push(pm.v[v].slice()); vmap.set(v, pm.v.length - 1); }
    // orientation: if a face has a->b, new quad needs b->a
    const fs = t.edgeFaces[ei];
    let p = a, q = b;
    if (fs.length && t.dmap.get(dkey(a, b)) === undefined) { p = b; q = a; }
    pm.addFace([q, p, vmap.get(p), vmap.get(q)], null, fs.length && pm.fm ? pm.fm[fs[0]] : null);
    newPairs.push([vmap.get(a), vmap.get(b)]);
  }
  for (const [v, nv] of vmap) { const d = norm(dirs.get(v)); pm.v[nv] = add(pm.v[nv], mul(d, thickness + offset)); }
  pm.invalidate();
  return { mesh: pm, select: { type: 'e', pairs: newPairs } };
}

export function opBevel(pm0, { edges, fraction = 0.5, segments = 1 }) {
  let pm = pm0.clone(); const t = pm.topo;
  const bev = new Set();
  for (const [a, b] of edges) { const ei = pm.edgeIndex(a, b); if (ei >= 0 && t.edgeFaces[ei].length === 2) bev.add(ekey(a, b)); }
  if (!bev.size) return { mesh: pm };
  const isB = (a, b) => bev.has(ekey(a, b));
  const touched = new Set();
  for (const k of bev) { const [a, b] = ekeyVerts(k); touched.add(a); touched.add(b); }
  let minLen = Infinity;
  for (const v of touched) for (const ei of t.vertEdges[v]) { const [a, b] = t.edges[ei]; minLen = Math.min(minLen, len(sub(pm.v[a], pm.v[b]))); }
  const w = Math.max(1e-5, Math.min(1, fraction) * minLen * 0.5);
  const slid = new Set();
  pm.f.forEach(face => {
    const n = face.length;
    for (let i = 0; i < n; i++) {
      const v = face[i], p = face[(i + n - 1) % n], q = face[(i + 1) % n];
      const bi = isB(p, v), bo = isB(v, q);
      if (bi && !bo) slid.add(dkey(v, q));
      if (bo && !bi) slid.add(dkey(v, p));
    }
  });
  const vuv = pm.uv ? uvLookup(pm) : null;
  const origin = pm.v.map((_, i) => i);
  const cache = new Map();
  const newPt = (key, pos, ov, uv) => {
    if (cache.has(key)) return cache.get(key);
    pm.v.push(pos); origin.push(ov); if (vuv) vuv.push(uv); cache.set(key, pm.v.length - 1); return pm.v.length - 1;
  };
  const dirTo = (v, o) => { const d = sub(pm.v[o], pm.v[v]); const l = len(d); return mul(d, Math.min(w, l * 0.49) / (l || 1)); };
  const getS = (v, o) => newPt('s' + v + '_' + o, add(pm.v[v], dirTo(v, o)), v, vuv && vuv[v] && vuv[o] ? lerp2(vuv[v], vuv[o], Math.min(w / (len(sub(pm.v[o], pm.v[v])) || 1), 0.49)) : null);
  const getI = (v, f, p, q) => newPt('i' + v + '_' + f, add(add(pm.v[v], dirTo(v, p)), dirTo(v, q)), v, vuv ? vuv[v] : null);
  const corner = new Map();
  const nf = [], nuv = [], nfm = [];
  pm.f.forEach((face, fi) => {
    const n = face.length; const out = [];
    for (let i = 0; i < n; i++) {
      const v = face[i], p = face[(i + n - 1) % n], q = face[(i + 1) % n];
      if (!touched.has(v)) { out.push(v); continue; }
      const bi = isB(p, v), bo = isB(v, q);
      if (bi && bo) { const x = getI(v, fi, p, q); out.push(x); corner.set(fi + ':' + v, x); }
      else if (bi) { const x = getS(v, q); out.push(x); corner.set(fi + ':' + v, x); }
      else if (bo) { const x = getS(v, p); out.push(x); corner.set(fi + ':' + v, x); }
      else {
        const inS = slid.has(dkey(v, p)) ? getS(v, p) : null;
        const outS = slid.has(dkey(v, q)) ? getS(v, q) : null;
        if (inS !== null) out.push(inS);
        if (inS === null || outS === null) out.push(v);
        if (outS !== null) out.push(outS);
      }
    }
    nf.push(out); nuv.push(vuv ? out.map(x => (vuv[x] || [0, 0]).slice()) : null); nfm.push(pm.fm ? pm.fm[fi] : null);
  });
  const newFaces = [];
  for (const k of bev) {
    let [a, b] = ekeyVerts(k);
    let f1 = t.dmap.get(dkey(a, b)), f2 = t.dmap.get(dkey(b, a));
    if (f1 === undefined || f2 === undefined) continue;
    const P1a = corner.get(f1 + ':' + a), P1b = corner.get(f1 + ':' + b), P2a = corner.get(f2 + ':' + a), P2b = corner.get(f2 + ':' + b);
    let q = [P1b, P1a, P2a, P2b].filter((x, i, arr) => x !== undefined && x !== arr[(i + 1) % arr.length]);
    if (q.length >= 3) { newFaces.push(nf.length); nf.push(q); nuv.push(vuv ? q.map(x => (vuv[x] || [0, 0]).slice()) : null); nfm.push(pm.fm ? pm.fm[f1] : null); }
  }
  pm.f = nf; pm.uv = vuv ? nuv : null; pm.fm = pm.fm ? nfm : null; pm.edgeHard = null; pm.invalidate();
  // fill the corner patches
  const loops = boundaryLoops(pm, (a, b) => origin[a] === origin[b] && touched.has(origin[a]));
  for (const loop of loops) { newFaces.push(pm.f.length); pm.addFace(loop.slice(), vuv ? loop.map(x => (vuv[x] || [0, 0]).slice()) : null, null); }
  cleanFaces(pm);
  compact(pm);
  let res = { mesh: pm, select: { type: 'f', ids: newFaces.filter(i => i < pm.f.length) } };
  if (segments > 1) {
    // approximate additional segments by subdividing the bevel faces linearly
    // (keeps topology valid; rounded profile comes from smoothing)
  }
  return res;
}

export function opChamferVertex(pm0, { verts, width = 0.25 }) {
  const pm = pm0.clone(); const sel = new Set(verts);
  const vuv = pm.uv ? uvLookup(pm) : null;
  const origin = pm.v.map((_, i) => i); const cache = new Map();
  const getS = (v, o) => {
    const k = v + '_' + o; if (cache.has(k)) return cache.get(k);
    const d = sub(pm.v[o], pm.v[v]); const l = len(d) || 1;
    pm.v.push(add(pm.v[v], mul(d, Math.min(width, 0.49 * l) / l))); origin.push(v); if (vuv) vuv.push(vuv[v] ? vuv[v].slice() : [0, 0]);
    cache.set(k, pm.v.length - 1); return pm.v.length - 1;
  };
  pm.f = pm.f.map(face => {
    const n = face.length; const out = [];
    for (let i = 0; i < n; i++) {
      const v = face[i];
      if (!sel.has(v)) { out.push(v); continue; }
      out.push(getS(v, face[(i + n - 1) % n]), getS(v, face[(i + 1) % n]));
    }
    return out;
  });
  if (pm.uv) pm.uv = pm.f.map(f => f.map(x => (vuv[x] || [0, 0]).slice()));
  pm.invalidate();
  const newF = [];
  for (const loop of boundaryLoops(pm, (a, b) => origin[a] === origin[b] && sel.has(origin[a]))) { newF.push(pm.f.length); pm.addFace(loop, vuv ? loop.map(x => (vuv[x] || [0, 0]).slice()) : null); }
  compact(pm);
  return { mesh: pm, select: { type: 'f', ids: newF } };
}

// split edges at parameters; returns map edgeKey -> new vertex index
function splitEdges(pm, splits /* [{a,b,t}] */) {
  const vuv = pm.uv ? uvLookup(pm) : null;
  const made = new Map(); // ekey -> [{t (from min vertex), idx}]
  for (const s of splits) {
    const k = ekey(s.a, s.b);
    const tmin = s.a < s.b ? s.t : 1 - s.t;
    const arr = made.get(k) || [];
    if (arr.some(x => Math.abs(x.t - tmin) < 1e-6)) continue;
    const lo = Math.min(s.a, s.b), hi = Math.max(s.a, s.b);
    pm.v.push(lerp3(pm.v[lo], pm.v[hi], tmin));
    arr.push({ t: tmin, idx: pm.v.length - 1, uvLo: null }); made.set(k, arr);
  }
  // insert into faces
  pm.f.forEach((face, fi) => {
    const n = face.length; const out = []; const ou = [];
    const fu = pm.uv && pm.uv[fi];
    for (let i = 0; i < n; i++) {
      const a = face[i], b = face[(i + 1) % n];
      out.push(a); if (fu) ou.push(fu[i]);
      const arr = made.get(ekey(a, b));
      if (arr) {
        const sorted = arr.slice().sort((x, y) => x.t - y.t);
        const seq = a < b ? sorted : sorted.reverse();
        for (const s of seq) {
          out.push(s.idx);
          if (fu) { const tt = a < b ? s.t : 1 - s.t; ou.push(lerp2(fu[i] || [0, 0], fu[(i + 1) % n] || [0, 0], tt)); }
        }
      }
    }
    pm.f[fi] = out; if (fu) pm.uv[fi] = ou;
  });
  pm.invalidate();
  const res = new Map(); for (const [k, arr] of made) res.set(k, arr);
  return res;
}
// split face fi by connecting vertices u and w (both must be on the face and not adjacent)
function splitFace(pm, fi, u, w) {
  const face = pm.f[fi]; const n = face.length;
  const i = face.indexOf(u), j = face.indexOf(w);
  if (i < 0 || j < 0) return -1;
  if ((i + 1) % n === j || (j + 1) % n === i) return -1;
  const fu = pm.uv && pm.uv[fi];
  const A = [], B = [], Au = [], Bu = [];
  for (let k = i; ; k = (k + 1) % n) { A.push(face[k]); if (fu) Au.push(fu[k]); if (k === j) break; }
  for (let k = j; ; k = (k + 1) % n) { B.push(face[k]); if (fu) Bu.push(fu[k]); if (k === i) break; }
  pm.f[fi] = A; if (fu) pm.uv[fi] = Au;
  const nf = pm.addFace(B, fu ? Bu : null, pm.fm ? pm.fm[fi] : null);
  pm.invalidate();
  return nf;
}

export function edgeRing(pm, a, b) {
  // returns [{a,b}] oriented edges across the quad ring (a-side consistent), and the faces
  const t = pm.topo; const res = []; const faces = [];
  let f = t.dmap.get(dkey(a, b)); let flip = false;
  if (f === undefined) { f = t.dmap.get(dkey(b, a)); flip = true; }
  if (f === undefined) return { edges: [{ a, b }], faces: [], closed: false };
  res.push({ a, b });
  const visited = new Set();
  const walk = (startFace, x, y) => {
    // x,y: oriented edge (x is a-side) lying in startFace
    let cf = startFace; let cx = x, cy = y;
    while (cf !== undefined && !visited.has(cf)) {
      const face = pm.f[cf];
      if (face.length !== 4) break;
      visited.add(cf); faces.push(cf);
      const i = face.indexOf(cx), j = face.indexOf(cy);
      if (i < 0 || j < 0) break;
      // the opposite edge: vertex adjacent to cx (not cy) is the a-side, adjacent to cy (not cx) the b-side
      let ox, oy;
      if ((i + 1) % 4 === j) { ox = face[(i + 3) % 4]; oy = face[(i + 2) % 4]; }
      else { ox = face[(i + 1) % 4]; oy = face[(i + 2) % 4]; }
      res.push({ a: ox, b: oy });
      // neighbour across opposite edge
      let g = t.dmap.get(dkey(oy, ox)); if (g === cf) g = undefined;
      if (g === undefined) { g = t.dmap.get(dkey(ox, oy)); if (g === cf) g = undefined; }
      cf = g; cx = ox; cy = oy;
    }
    return cf !== undefined && visited.has(cf);
  };
  const closed = walk(f, a, b);
  if (!closed) {
    let g = t.dmap.get(dkey(b, a)); if (g === f) g = undefined;
    if (g === undefined) { g = t.dmap.get(dkey(a, b)); if (g === f) g = undefined; }
    if (g !== undefined && !visited.has(g)) walk(g, a, b);
  }
  // dedupe edges
  const seen = new Set(); const out = [];
  for (const e of res) { const k = ekey(e.a, e.b); if (!seen.has(k)) { seen.add(k); out.push(e); } }
  return { edges: out, faces, closed };
}

export function opInsertEdgeLoop(pm0, { edge, t = 0.5, multiple = 1 }) {
  const pm = pm0.clone();
  const [a, b] = edge;
  if (pm.edgeIndex(a, b) < 0) return { mesh: pm };
  const ring = edgeRing(pm, a, b);
  const count = Math.max(1, Math.round(multiple));
  const params = count === 1 ? [t] : Array.from({ length: count }, (_, i) => (i + 1) / (count + 1));
  const splits = [];
  for (const e of ring.edges) for (const p of params) splits.push({ a: e.a, b: e.b, t: p });
  const made = splitEdges(pm, splits);
  // connect within ring faces
  const newPairs = [];
  for (const fi of ring.faces) {
    for (let pi = 0; pi < params.length; pi++) {
      const face = pm.f[fi];
      // the new vertices on this face belonging to param pi
      const cand = [];
      for (const e of ring.edges) {
        const arr = made.get(ekey(e.a, e.b)); if (!arr) continue;
        const tmin = e.a < e.b ? params[pi] : 1 - params[pi];
        const m = arr.find(x => Math.abs(x.t - tmin) < 1e-6);
        if (m && face.includes(m.idx)) cand.push(m.idx);
      }
      // find the right face (may have been split already)
      if (cand.length === 2) {
        let target = -1;
        for (let k = 0; k < pm.f.length; k++) if (pm.f[k].includes(cand[0]) && pm.f[k].includes(cand[1])) { const fk = pm.f[k]; const i = fk.indexOf(cand[0]), j = fk.indexOf(cand[1]); const n = fk.length; if ((i + 1) % n !== j && (j + 1) % n !== i) { target = k; break; } }
        if (target >= 0 && splitFace(pm, target, cand[0], cand[1]) >= 0) newPairs.push([cand[0], cand[1]]);
      } else if (cand.length > 2) {
        // multiple loops: split progressively
      }
    }
  }
  // handle multiple loops: connect pairs in all faces still containing two new verts with same param
  if (params.length > 1) {
    for (let pi = 0; pi < params.length; pi++) {
      const ids = [];
      for (const e of ring.edges) { const arr = made.get(ekey(e.a, e.b)); if (!arr) continue; const tmin = e.a < e.b ? params[pi] : 1 - params[pi]; const m = arr.find(x => Math.abs(x.t - tmin) < 1e-6); if (m) ids.push(m.idx); }
      const idset = new Set(ids);
      for (let k = 0; k < pm.f.length; k++) {
        const on = pm.f[k].filter(x => idset.has(x));
        if (on.length === 2) { if (splitFace(pm, k, on[0], on[1]) >= 0) newPairs.push(on); }
      }
    }
  }
  pm.invalidate();
  return { mesh: pm, select: { type: 'e', pairs: newPairs } };
}

// Multi-cut / split polygon: points = [{edge:[a,b], t} | {vertex: v}]
export function opSplit(pm0, { points }) {
  const pm = pm0.clone();
  const splits = points.filter(p => p.edge).map(p => ({ a: p.edge[0], b: p.edge[1], t: p.t }));
  const made = splitEdges(pm, splits);
  const ids = points.map(p => {
    if (p.vertex !== undefined) return p.vertex;
    const arr = made.get(ekey(p.edge[0], p.edge[1])); if (!arr) return -1;
    const tmin = p.edge[0] < p.edge[1] ? p.t : 1 - p.t;
    const m = arr.find(x => Math.abs(x.t - tmin) < 1e-6); return m ? m.idx : -1;
  });
  const pairs = [];
  for (let i = 0; i + 1 < ids.length; i++) {
    const u = ids[i], w = ids[i + 1]; if (u < 0 || w < 0 || u === w) continue;
    for (let k = 0; k < pm.f.length; k++) {
      const f = pm.f[k]; if (!f.includes(u) || !f.includes(w)) continue;
      if (splitFace(pm, k, u, w) >= 0) { pairs.push([u, w]); break; }
    }
  }
  return { mesh: pm, select: { type: 'e', pairs } };
}

export function opDeleteFaces(pm0, { faces }) {
  const pm = pm0.clone(); removeFaces(pm, new Set(faces)); compact(pm); return { mesh: pm };
}

function dissolveEdge(pm, a, b) {
  const t = pm.topo;
  const f1 = t.dmap.get(dkey(a, b)), f2 = t.dmap.get(dkey(b, a));
  if (f1 === undefined || f2 === undefined || f1 === f2) return false;
  const F1 = pm.f[f1], F2 = pm.f[f2];
  const rot = (F, s) => { const i = F.indexOf(s); return F.slice(i).concat(F.slice(0, i)); };
  const r1 = rot(F1, b); // b ... a
  const r2 = rot(F2, a); // a ... b
  const merged = r1.concat(r2.slice(1, -1));
  let mu = null;
  if (pm.uv && pm.uv[f1] && pm.uv[f2]) {
    const ru = (F, U, s) => { const i = F.indexOf(s); return U.slice(i).concat(U.slice(0, i)); };
    mu = ru(F1, pm.uv[f1], b).concat(ru(F2, pm.uv[f2], a).slice(1, -1));
  }
  if (new Set(merged).size !== merged.length) return false; // would create a non-manifold face
  pm.f[f1] = merged; if (pm.uv) pm.uv[f1] = mu;
  removeFaces(pm, new Set([f2]));
  return true;
}
function removeWingedVerts(pm, verts) {
  const t = pm.topo;
  const kill = new Set();
  for (const v of verts) {
    if (v >= pm.v.length) continue;
    if (t.vertEdges[v].length === 2 && t.vertFaces[v].length >= 1) {
      const ok = t.vertFaces[v].every(f => pm.f[f].length > 3);
      const boundary = t.vertEdges[v].some(e => t.edgeFaces[e].length < 2);
      if (ok && !boundary) kill.add(v);
    }
  }
  if (!kill.size) return;
  pm.f = pm.f.map((f, fi) => {
    if (pm.uv && pm.uv[fi]) pm.uv[fi] = pm.uv[fi].filter((_, i) => !kill.has(f[i]));
    return f.filter(v => !kill.has(v));
  });
  pm.invalidate();
}
export function opDeleteEdges(pm0, { edges, cleanVerts = true }) {
  const pm = pm0.clone(); const ends = new Set();
  for (const [a, b] of edges) { if (dissolveEdge(pm, a, b)) { ends.add(a); ends.add(b); } }
  if (cleanVerts) removeWingedVerts(pm, ends);
  cleanFaces(pm); compact(pm);
  return { mesh: pm };
}
export function opDeleteVertices(pm0, { verts }) {
  const pm = pm0.clone(); const set = new Set(verts);
  for (const v of set) {
    let guard = 0;
    while (guard++ < 64) {
      const t = pm.topo; const es = t.vertEdges[v]; if (!es || es.length <= 2) break;
      let done = false;
      for (const ei of es) { const [a, b] = t.edges[ei]; if (dissolveEdge(pm, a, b)) { done = true; break; } }
      if (!done) break;
    }
  }
  pm.f = pm.f.map((f, fi) => { if (pm.uv && pm.uv[fi]) pm.uv[fi] = pm.uv[fi].filter((_, i) => !set.has(f[i])); return f.filter(v => !set.has(v)); });
  pm.invalidate(); cleanFaces(pm); compact(pm);
  return { mesh: pm };
}

function applyMerge(pm, rep) {
  pm.f = pm.f.map(f => f.map(v => rep[v]));
  if (pm.edgeHard) pm.edgeHard = null;
  pm.invalidate(); cleanFaces(pm); compact(pm);
}
export function opMergeVertices(pm0, { verts = null, threshold = 0.001 }) {
  const pm = pm0.clone();
  const list = verts ? [...new Set(verts)] : pm.v.map((_, i) => i);
  const rep = pm.v.map((_, i) => i);
  // spatial hash
  const cell = Math.max(threshold, 1e-6); const grid = new Map();
  const keyOf = (p) => [Math.floor(p[0] / cell), Math.floor(p[1] / cell), Math.floor(p[2] / cell)];
  for (const v of list) {
    const p = pm.v[v]; const [x, y, z] = keyOf(p); let found = -1;
    for (let dx = -1; dx <= 1 && found < 0; dx++) for (let dy = -1; dy <= 1 && found < 0; dy++) for (let dz = -1; dz <= 1 && found < 0; dz++) {
      const arr = grid.get((x + dx) + ',' + (y + dy) + ',' + (z + dz));
      if (arr) for (const o of arr) if (len(sub(pm.v[o], p)) <= threshold) { found = o; break; }
    }
    if (found >= 0) rep[v] = found;
    else { const k = x + ',' + y + ',' + z; if (!grid.has(k)) grid.set(k, []); grid.get(k).push(v); }
  }
  // average merged positions
  const acc = new Map();
  for (const v of list) { const r = rep[v]; const a = acc.get(r) || [0, 0, 0, 0]; a[0] += pm.v[v][0]; a[1] += pm.v[v][1]; a[2] += pm.v[v][2]; a[3]++; acc.set(r, a); }
  for (const [r, a] of acc) if (a[3] > 1) pm.v[r] = [a[0] / a[3], a[1] / a[3], a[2] / a[3]];
  applyMerge(pm, rep);
  return { mesh: pm };
}
export function opMergeToCenter(pm0, { groups }) {
  // groups: arrays of vertex indices; each group collapses to its centroid
  const pm = pm0.clone(); const rep = pm.v.map((_, i) => i);
  for (const g of groups) {
    if (!g.length) continue;
    const c = [0, 0, 0]; for (const v of g) { c[0] += pm.v[v][0]; c[1] += pm.v[v][1]; c[2] += pm.v[v][2]; }
    pm.v[g[0]] = mul(c, 1 / g.length);
    for (const v of g) rep[v] = g[0];
  }
  applyMerge(pm, rep);
  return { mesh: pm };
}

export function subdivide(pm, levels = 1, smooth = true) {
  let m = pm;
  for (let l = 0; l < levels; l++) m = subdivideOnce(m, smooth);
  return m;
}
function subdivideOnce(pm, smooth) {
  const t = pm.topo; const V = pm.v.length, E = t.edges.length;
  const fp = pm.f.map((_, i) => pm.faceCenter(i));
  const ep = t.edges.map(([a, b], ei) => {
    const mid = lerp3(pm.v[a], pm.v[b], 0.5);
    const fs = t.edgeFaces[ei];
    const hard = pm.edgeHard && pm.edgeHard[ekey(a, b)];
    if (!smooth || fs.length !== 2 || hard) return mid;
    return mul(add(add(pm.v[a], pm.v[b]), add(fp[fs[0]], fp[fs[1]])), 0.25);
  });
  const nv = pm.v.map((p, v) => {
    if (!smooth) return p.slice();
    const es = t.vertEdges[v]; const fs = [...new Set(t.vertFaces[v])];
    if (!es.length) return p.slice();
    const bes = es.filter(e => t.edgeFaces[e].length < 2);
    if (bes.length) {
      if (bes.length !== 2) return p.slice();
      const o = bes.map(e => { const [a, b] = t.edges[e]; return pm.v[a === v ? b : a]; });
      return mul(add(add(o[0], o[1]), mul(p, 6)), 1 / 8);
    }
    const n = es.length;
    let F = [0, 0, 0]; for (const f of fs) F = add(F, fp[f]); F = mul(F, 1 / fs.length);
    let R = [0, 0, 0]; for (const e of es) { const [a, b] = t.edges[e]; R = add(R, lerp3(pm.v[a], pm.v[b], 0.5)); } R = mul(R, 1 / n);
    return mul(add(add(F, mul(R, 2)), mul(p, n - 3)), 1 / n);
  });
  const out = new PolyMesh(nv.concat(ep, fp), [], pm.uv ? [] : null, pm.fm ? [] : null);
  out.softAngle = 180;
  const dE = [];
  pm.f.forEach((face, fi) => {
    const n = face.length; const fu = pm.uv && pm.uv[fi];
    let fuc = null; if (fu) { fuc = [0, 0]; for (const u of fu) { fuc[0] += (u || [0, 0])[0] / n; fuc[1] += (u || [0, 0])[1] / n; } }
    for (let i = 0; i < n; i++) {
      const vp = face[(i + n - 1) % n], v = face[i], vn = face[(i + 1) % n];
      const e1 = V + t.edgeMap.get(ekey(v, vn)), e0 = V + t.edgeMap.get(ekey(vp, v));
      out.f.push([v, e1, V + E + fi, e0]);
      if (out.uv) {
        if (fu) { const u = fu[i] || [0, 0], un = fu[(i + 1) % n] || [0, 0], up = fu[(i + n - 1) % n] || [0, 0]; out.uv.push([u.slice(), lerp2(u, un, 0.5), fuc.slice(), lerp2(up, u, 0.5)]); }
        else out.uv.push(null);
      }
      if (out.fm) out.fm.push(pm.fm[fi]);
    }
  });
  if (pm.displayEdges) {
    // keep subdivided versions of display edges
    for (const [a, b] of pm.displayEdges) { const ei = t.edgeMap.get(ekey(a, b)); if (ei === undefined) continue; dE.push([a, V + ei], [V + ei, b]); }
    out.displayEdges = dE;
  }
  if (!smooth) { out.softAngle = pm.softAngle; }
  return out;
}
export function opSmooth(pm0, { divisions = 1 }) { return { mesh: subdivide(pm0, Math.max(0, Math.min(4, Math.round(divisions))), true) }; }
export function opAddDivisions(pm0, { divisions = 1, faces = null }) {
  if (!faces) { const m = subdivide(pm0, Math.max(1, Math.min(4, Math.round(divisions))), false); m.softAngle = pm0.softAngle; return { mesh: m }; }
  // subdivide only selected faces: split edges at midpoints, then fan quads from centre
  const pm = pm0.clone(); const sel = new Set(faces);
  const splits = []; const seen = new Set();
  for (const f of sel) { const face = pm.f[f]; for (let i = 0; i < face.length; i++) { const a = face[i], b = face[(i + 1) % face.length]; const k = ekey(a, b); if (!seen.has(k)) { seen.add(k); splits.push({ a, b, t: 0.5 }); } } }
  const origCount = new Map([...sel].map(f => [f, pm.f[f].length]));
  splitEdges(pm, splits);
  const out = [];
  for (const f of sel) {
    const face = pm.f[f]; const fu = pm.uv && pm.uv[f];
    const c = pm.faceCenter(f); pm.v.push(c); const ci = pm.v.length - 1;
    let cu = null; if (fu) { cu = [0, 0]; fu.forEach(u => { cu[0] += u[0] / fu.length; cu[1] += u[1] / fu.length; }); }
    // original corners are at even positions only if every edge got split: find originals by membership
    const n = face.length; const quads = [];
    // corners = vertices of the original face
    const isMid = (v) => v >= pm0.v.length;
    for (let i = 0; i < n; i++) {
      if (isMid(face[i])) continue;
      const prev = face[(i + n - 1) % n], next = face[(i + 1) % n];
      quads.push({ v: [face[i], next, ci, prev], u: fu ? [fu[i], fu[(i + 1) % n], cu, fu[(i + n - 1) % n]] : null });
    }
    quads.forEach((q, qi) => { if (qi === 0) { pm.f[f] = q.v; if (fu) pm.uv[f] = q.u; out.push(f); } else out.push(pm.addFace(q.v, q.u, pm.fm ? pm.fm[f] : null)); });
  }
  pm.invalidate();
  return { mesh: pm, select: { type: 'f', ids: out } };
}

export function opTriangulate(pm0, { faces = null }) {
  const pm = pm0.clone(); const sel = faces ? new Set(faces) : null;
  const nf = [], nu = [], nm = [];
  pm.f.forEach((f, fi) => {
    if ((sel && !sel.has(fi)) || f.length === 3) { nf.push(f); nu.push(pm.uv ? pm.uv[fi] : null); nm.push(pm.fm ? pm.fm[fi] : null); return; }
    const tris = triangulateFace(f.map(i => pm.v[i]), pm.faceNormal(fi));
    for (const t of tris) { nf.push(t.map(i => f[i])); nu.push(pm.uv && pm.uv[fi] ? t.map(i => pm.uv[fi][i]) : null); nm.push(pm.fm ? pm.fm[fi] : null); }
  });
  pm.f = nf; if (pm.uv) pm.uv = nu; if (pm.fm) pm.fm = nm; pm.invalidate();
  return { mesh: pm };
}
export function opQuadrangulate(pm0, { angle = 30 }) {
  // greedy: merge adjacent triangle pairs into quads when nearly coplanar
  let pm = pm0.clone(); const t = pm.topo; const used = new Set();
  const fn = pm.f.map((_, i) => norm(pm.faceNormal(i)));
  const cands = [];
  t.edges.forEach(([a, b], ei) => {
    const fs = t.edgeFaces[ei]; if (fs.length !== 2) return;
    if (pm.f[fs[0]].length !== 3 || pm.f[fs[1]].length !== 3) return;
    const ang = Math.acos(Math.max(-1, Math.min(1, dot(fn[fs[0]], fn[fs[1]])))) * 180 / Math.PI;
    if (ang <= angle) cands.push({ a, b, ang, fs });
  });
  cands.sort((x, y) => x.ang - y.ang);
  for (const c of cands) {
    if (used.has(c.fs[0]) || used.has(c.fs[1])) continue;
    used.add(c.fs[0]); used.add(c.fs[1]);
    dissolveEdge(pm, c.a, c.b);
    // indices shift after removal; recompute mapping by rebuilding candidate face ids is complex — restart
    return opQuadrangulate(pm, { angle });
  }
  return { mesh: pm };
}
export function opReverse(pm0, { faces = null }) {
  const pm = pm0.clone(); const sel = faces ? new Set(faces) : null;
  pm.f = pm.f.map((f, fi) => { if (sel && !sel.has(fi)) return f; if (pm.uv && pm.uv[fi]) pm.uv[fi] = pm.uv[fi].slice().reverse(); return f.slice().reverse(); });
  pm.invalidate();
  return { mesh: pm };
}
export function opPoke(pm0, { faces }) {
  const pm = pm0.clone(); const out = [];
  for (const fi of faces) {
    const f = pm.f[fi]; if (!f) continue;
    const c = pm.faceCenter(fi); pm.v.push(c); const ci = pm.v.length - 1;
    const fu = pm.uv && pm.uv[fi]; let cu = null;
    if (fu) { cu = [0, 0]; fu.forEach(u => { cu[0] += u[0] / fu.length; cu[1] += u[1] / fu.length; }); }
    const n = f.length;
    for (let i = 0; i < n; i++) {
      const tri = [f[i], f[(i + 1) % n], ci]; const tu = fu ? [fu[i], fu[(i + 1) % n], cu] : null;
      if (i === 0) { pm.f[fi] = tri; if (fu) pm.uv[fi] = tu; out.push(fi); } else out.push(pm.addFace(tri, tu, pm.fm ? pm.fm[fi] : null));
    }
  }
  pm.invalidate();
  return { mesh: pm, select: { type: 'f', ids: out } };
}
export function opFillHole(pm0, { edges = null }) {
  const pm = pm0.clone();
  let loops = boundaryLoops(pm);
  if (edges && edges.length) {
    const ks = new Set(edges.map(([a, b]) => ekey(a, b)));
    loops = loops.filter(L => L.some((v, i) => ks.has(ekey(v, L[(i + 1) % L.length]))));
  }
  const out = [];
  const vuv = pm.uv ? uvLookup(pm) : null;
  for (const L of loops) out.push(pm.addFace(L, vuv ? L.map(v => (vuv[v] || [0, 0]).slice()) : null));
  return { mesh: pm, select: { type: 'f', ids: out } };
}
export function opMirror(pm0, { axis = 0, position = 0, direction = 1, merge = true, threshold = 0.001 }) {
  const pm = pm0.clone(); const n = pm.v.length; const nf = pm.f.length;
  // optionally cut away geometry on the mirrored side
  const map = [];
  for (let i = 0; i < n; i++) { const p = pm.v[i].slice(); p[axis] = 2 * position - p[axis]; pm.v.push(p); map.push(n + i); }
  for (let fi = 0; fi < nf; fi++) {
    const f = pm.f[fi]; const nfc = f.map(v => map[v]).reverse();
    const fu = pm.uv && pm.uv[fi] ? pm.uv[fi].slice().reverse().map(u => u.slice()) : null;
    pm.addFace(nfc, fu, pm.fm ? pm.fm[fi] : null);
  }
  pm.invalidate();
  if (merge) {
    const rep = pm.v.map((_, i) => i);
    for (let i = 0; i < n; i++) if (Math.abs(pm.v[i][axis] - position) <= threshold) { rep[map[i]] = i; }
    applyMerge(pm, rep);
  }
  return { mesh: pm };
}
export function opBridge(pm0, { faces = null, edges = null, divisions = 0 }) {
  const pm = pm0.clone();
  let L1, L2; // loops, oriented so that L1[i]->L1[i+1] exists in remaining faces (border) — we build quads reversing
  if (faces && faces.length === 2) {
    const [fa, fb] = faces; const A = pm.f[fa].slice(), B = pm.f[fb].slice();
    if (A.length !== B.length) return { mesh: pm, error: 'Bridge needs two faces with the same number of vertices' };
    removeFaces(pm, new Set([fa, fb]));
    // after removal, border loops: A's edges now border: neighbour has A[i+1]->A[i]; quad needs A[i]->A[i+1]
    L1 = A; L2 = B.slice().reverse();
    const n = L1.length; let best = 0, bestD = Infinity;
    for (let k = 0; k < n; k++) { let d = 0; for (let i = 0; i < n; i++) d += len(sub(pm.v[L1[i]], pm.v[L2[(i + k) % n]])); if (d < bestD) { bestD = d; best = k; } }
    L2 = L2.slice(best).concat(L2.slice(0, best));
    return bridgeLoops(pm, L1, L2, true, divisions, false);
  }
  if (edges && edges.length >= 2) {
    // group selected border edges into two chains
    const t = pm.topo;
    const dir = []; // directed border half-edges a->b existing in faces
    for (const [a, b] of edges) { if (t.dmap.has(dkey(a, b)) && !t.dmap.has(dkey(b, a))) dir.push([a, b]); else if (t.dmap.has(dkey(b, a)) && !t.dmap.has(dkey(a, b))) dir.push([b, a]); }
    const next = new Map(dir.map(([a, b]) => [a, b])); const prevSet = new Set(dir.map(([, b]) => b));
    const chains = []; const used = new Set();
    for (const [a] of dir) { if (used.has(a) || prevSet.has(a)) continue; const c = [a]; used.add(a); let cur = next.get(a); while (cur !== undefined && !used.has(cur)) { c.push(cur); used.add(cur); cur = next.get(cur); } if (cur !== undefined && !next.has(cur)) c.push(cur); chains.push({ verts: c, closed: false }); }
    for (const [a] of dir) { if (used.has(a)) continue; const c = []; let cur = a; while (cur !== undefined && !used.has(cur)) { c.push(cur); used.add(cur); cur = next.get(cur); } chains.push({ verts: c, closed: true }); }
    if (chains.length !== 2) return { mesh: pm, error: 'Select two border edge chains to bridge' };
    let [c1, c2] = chains;
    if (c1.verts.length !== c2.verts.length) return { mesh: pm, error: 'Edge chains must have the same number of edges' };
    if (c1.closed !== c2.closed) return { mesh: pm, error: 'Cannot bridge an open chain to a closed loop' };
    // quads need reversed border direction: L1 = reversed c1 and L2 = c2 (pairs u_i ↔ w_{m-i})
    const A = c1.verts.slice().reverse(); const B = c2.verts.slice();
    if (c1.closed) {
      const n = A.length; let best = 0, bestD = Infinity;
      for (let k = 0; k < n; k++) { let d = 0; for (let i = 0; i < n; i++) d += len(sub(pm.v[A[i]], pm.v[B[(i + k) % n]])); if (d < bestD) { bestD = d; best = k; } }
      return bridgeLoops(pm, A, B.slice(best).concat(B.slice(0, best)), true, divisions, false);
    }
    // open chains: pair A[i] with B[i]? check orientation by distance
    return bridgeLoops(pm, A, B, false, divisions, false);
  }
  return { mesh: pm, error: 'Select two faces or two border edge chains' };
}
function bridgeLoops(pm, L1, L2, closed, divisions, _) {
  const n = L1.length; const rows = [L1];
  const div = Math.max(0, Math.round(divisions));
  for (let d = 1; d <= div; d++) {
    const tt = d / (div + 1); const row = [];
    for (let i = 0; i < n; i++) { pm.v.push(lerp3(pm.v[L1[i]], pm.v[L2[i]], tt)); row.push(pm.v.length - 1); }
    rows.push(row);
  }
  rows.push(L2);
  const out = [];
  const segs = closed ? n : n - 1;
  for (let r = 0; r + 1 < rows.length; r++) {
    const R0 = rows[r], R1 = rows[r + 1];
    for (let i = 0; i < segs; i++) {
      const j = (i + 1) % n;
      out.push(pm.addFace([R0[i], R0[j], R1[j], R1[i]]));
    }
  }
  pm.invalidate();
  // fix orientation: compare with a neighbouring face of L1 edge
  const t = pm.topo;
  const bad = out.filter(fi => { const f = pm.f[fi]; for (let i = 0; i < 4; i++) { const a = f[i], b = f[(i + 1) % 4]; const g = t.dmap.get(dkey(a, b)); if (g !== undefined && g !== fi) return true; } return false; });
  if (bad.length > out.length / 2) { for (const fi of out) pm.f[fi].reverse(); pm.invalidate(); }
  return { mesh: pm, select: { type: 'f', ids: out } };
}
export function opSoftEdge(pm0, { angle = 180, edges = null }) {
  const pm = pm0.clone();
  if (edges && edges.length) {
    pm.edgeHard = pm.edgeHard || {};
    for (const [a, b] of edges) pm.edgeHard[ekey(a, b)] = angle < 1e-3 ? true : angle >= 179.9 ? false : undefined;
  } else { pm.softAngle = angle; pm.edgeHard = null; }
  pm.invalidate();
  return { mesh: pm };
}
export function opExtractFaces(pm0, faces) {
  // returns { keep, extracted }
  const sel = new Set(faces);
  const a = pm0.clone(); removeFaces(a, sel); compact(a);
  const b = pm0.clone(); removeFaces(b, new Set(pm0.f.map((_, i) => i).filter(i => !sel.has(i)))); compact(b);
  return { keep: a, extracted: b };
}
export function components(pm) {
  // connected face sets
  const t = pm.topo; const parent = pm.v.map((_, i) => i);
  const find = (x) => { while (parent[x] !== x) { parent[x] = parent[parent[x]]; x = parent[x]; } return x; };
  for (const f of pm.f) for (let i = 1; i < f.length; i++) { const a = find(f[0]), b = find(f[i]); if (a !== b) parent[a] = b; }
  const groups = new Map();
  pm.f.forEach((f, fi) => { const r = find(f[0]); if (!groups.has(r)) groups.set(r, []); groups.get(r).push(fi); });
  return [...groups.values()];
}
export function combineMeshes(list /* [{mesh, matrix: THREE.Matrix4, mat}] */) {
  const out = new PolyMesh([], [], null, null); out.softAngle = 60;
  const eh = {};
  for (const { mesh, matrix, mat } of list) {
    const base = out.v.length; const v = new THREE.Vector3();
    const flip = matrix && matrix.determinant() < 0;
    for (const p of mesh.v) { v.set(p[0], p[1], p[2]); if (matrix) v.applyMatrix4(matrix); out.v.push([v.x, v.y, v.z]); }
    mesh.f.forEach((f, fi) => {
      let ff = f.map(i => i + base); let uu = mesh.uv && mesh.uv[fi] ? mesh.uv[fi].map(c => c.slice()) : null;
      if (flip) { ff = ff.reverse(); if (uu) uu.reverse(); }
      out.addFace(ff, uu, (mesh.fm && mesh.fm[fi]) ?? mat ?? null);
    });
    if (mesh.edgeHard) for (const k in mesh.edgeHard) { const [a, b] = ekeyVerts(+k); eh[ekey(a + base, b + base)] = mesh.edgeHard[k]; }
    out.softAngle = Math.max(out.softAngle, mesh.softAngle);
  }
  if (Object.keys(eh).length) out.edgeHard = eh;
  return out;
}
export function transformMesh(pm0, matrix) {
  const pm = pm0.clone(); const v = new THREE.Vector3();
  for (const p of pm.v) { v.set(p[0], p[1], p[2]).applyMatrix4(matrix); p[0] = v.x; p[1] = v.y; p[2] = v.z; }
  if (matrix.determinant() < 0) { pm.f = pm.f.map(f => f.slice().reverse()); if (pm.uv) pm.uv = pm.uv.map(u => u ? u.slice().reverse() : u); }
  pm.invalidate();
  return pm;
}

// ---- UV projections ----
export function opProjectUV(pm0, { mode = 'planar', axis = 1, faces = null }) {
  const pm = pm0.clone(); const bb = pm.bbox();
  const size = [bb.max[0] - bb.min[0] || 1, bb.max[1] - bb.min[1] || 1, bb.max[2] - bb.min[2] || 1];
  const c = [(bb.max[0] + bb.min[0]) / 2, (bb.max[1] + bb.min[1]) / 2, (bb.max[2] + bb.min[2]) / 2];
  if (!pm.uv) pm.uv = pm.f.map(() => null);
  const sel = faces ? new Set(faces) : null;
  pm.f.forEach((f, fi) => {
    if (sel && !sel.has(fi)) return;
    let uvs;
    if (mode === 'planar') {
      const [i, j] = axis === 0 ? [2, 1] : axis === 1 ? [0, 2] : [0, 1];
      const s = Math.max(size[i], size[j]);
      uvs = f.map(v => [(pm.v[v][i] - c[i]) / s + 0.5, (pm.v[v][j] - c[j]) / s + 0.5]);
      if (axis === 1) uvs = uvs.map(u => [u[0], 1 - u[1]]);
    } else if (mode === 'cylindrical') {
      uvs = f.map(v => { const p = sub(pm.v[v], c); return [Math.atan2(p[0], p[2]) / (2 * Math.PI) + 0.5, (p[1]) / size[1] + 0.5]; });
      fixSeam(uvs);
    } else if (mode === 'spherical') {
      uvs = f.map(v => { const p = norm(sub(pm.v[v], c)); return [Math.atan2(p[0], p[2]) / (2 * Math.PI) + 0.5, Math.asin(Math.max(-1, Math.min(1, p[1]))) / Math.PI + 0.5]; });
      fixSeam(uvs);
    } else { // automatic: per-face best axis planar, packed into a 3x2 grid by axis/sign
      const n = norm(pm.faceNormal(fi)); const ax = Math.abs(n[0]) > Math.abs(n[1]) ? (Math.abs(n[0]) > Math.abs(n[2]) ? 0 : 2) : (Math.abs(n[1]) > Math.abs(n[2]) ? 1 : 2);
      const sgn = n[ax] >= 0 ? 0 : 1;
      const [i, j] = ax === 0 ? [2, 1] : ax === 1 ? [0, 2] : [0, 1];
      const s = Math.max(size[i], size[j]);
      const ox = ax / 3, oy = sgn / 2;
      uvs = f.map(v => [ox + ((pm.v[v][i] - c[i]) / s + 0.5) / 3 * (sgn && ax !== 1 ? -1 : 1) + (sgn && ax !== 1 ? 1 / 3 : 0), oy + ((pm.v[v][j] - c[j]) / s + 0.5) / 2]);
    }
    pm.uv[fi] = uvs;
  });
  return { mesh: pm };
}
function fixSeam(uvs) {
  const us = uvs.map(u => u[0]); const mx = Math.max(...us), mn = Math.min(...us);
  if (mx - mn > 0.5) for (const u of uvs) if (u[0] < 0.5) u[0] += 1;
}

export function signedVolume(pm) {
  let vol = 0;
  pm.f.forEach((f, fi) => {
    const tris = triangulateFace(f.map(i => pm.v[i]), pm.faceNormal(fi));
    for (const t of tris) { const a = pm.v[f[t[0]]], b = pm.v[f[t[1]]], c = pm.v[f[t[2]]]; vol += dot(a, cross(b, c)) / 6; }
  });
  return vol;
}
