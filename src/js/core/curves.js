// Inca — curves (CV B-splines / EP interpolating) and surface generation from curves.
import { PolyMesh, V3 } from './polymesh.js';

const TAU = Math.PI * 2;

// clamped uniform knot vector
function knots(n, p) {
  const m = n + p + 1; const k = [];
  for (let i = 0; i < m; i++) {
    if (i <= p) k.push(0); else if (i >= n) k.push(n - p); else k.push(i - p);
  }
  return k;
}
function deBoor(cvs, p, kv, u) {
  const n = cvs.length;
  let k = p; while (k < n - 1 && u >= kv[k + 1]) k++;
  const d = []; for (let j = 0; j <= p; j++) d.push(cvs[Math.max(0, Math.min(n - 1, j + k - p))].slice());
  for (let r = 1; r <= p; r++) for (let j = p; j >= r; j--) {
    const i = j + k - p; const den = kv[i + p - r + 1] - kv[i];
    const a = den === 0 ? 0 : (u - kv[i]) / den;
    d[j] = V3.lerp3(d[j - 1], d[j], a);
  }
  return d[p];
}

export class Curve {
  constructor(cvs = [], degree = 3, form = 'open', kind = 'cv') {
    this.cvs = cvs; this.degree = degree; this.form = form; this.kind = kind; // kind 'cv' | 'ep'
  }
  clone() { return new Curve(this.cvs.map(c => c.slice()), this.degree, this.form, this.kind); }
  toJSON() { return { cvs: this.cvs, degree: this.degree, form: this.form, kind: this.kind }; }
  static fromJSON(d) { return new Curve(d.cvs.map(c => c.slice()), d.degree, d.form, d.kind); }
  get spans() { return Math.max(1, this.form === 'periodic' ? this.cvs.length : this.cvs.length - (this.kind === 'ep' ? 1 : this.degree)); }
  sample(segPerSpan = 12) {
    const c = this.cvs; if (c.length < 2) return c.map(x => x.slice());
    if (this.kind === 'ep' || this.degree === 1) {
      if (this.degree === 1) { const pts = c.map(x => x.slice()); if (this.form === 'periodic') pts.push(c[0].slice()); return pts; }
      return catmull(c, segPerSpan, this.form === 'periodic');
    }
    let cvs = c; let p = Math.min(this.degree, c.length - 1);
    if (this.form === 'periodic') {
      cvs = c.concat(c.slice(0, p));
      const out = []; const n = cvs.length; const N = c.length * segPerSpan;
      // uniform (unclamped) knots
      const kv = []; for (let i = 0; i < n + p + 1; i++) kv.push(i);
      for (let s = 0; s <= N; s++) { const u = p + (s / N) * (n - p); out.push(deBoor(cvs, p, kv, Math.min(u, n - 1e-9))); }
      return out;
    }
    const n = cvs.length; const kv = knots(n, p); const umax = n - p; const N = Math.max(2, umax * segPerSpan);
    const out = [];
    for (let s = 0; s <= N; s++) out.push(deBoor(cvs, p, kv, Math.min(umax - 1e-9, (s / N) * umax)));
    out[out.length - 1] = cvs[n - 1].slice();
    return out;
  }
  length() { const s = this.sample(8); let L = 0; for (let i = 1; i < s.length; i++) L += V3.len(V3.sub(s[i], s[i - 1])); return L; }
}
function catmull(pts, seg, closed) {
  const n = pts.length; const out = [];
  const P = (i) => closed ? pts[(i + n) % n] : pts[Math.max(0, Math.min(n - 1, i))];
  const segs = closed ? n : n - 1;
  for (let i = 0; i < segs; i++) {
    const p0 = P(i - 1), p1 = P(i), p2 = P(i + 1), p3 = P(i + 2);
    for (let s = 0; s < seg; s++) {
      const t = s / seg, t2 = t * t, t3 = t2 * t;
      out.push([0, 1, 2].map(k => 0.5 * ((2 * p1[k]) + (-p0[k] + p2[k]) * t + (2 * p0[k] - 5 * p1[k] + 4 * p2[k] - p3[k]) * t2 + (-p0[k] + 3 * p1[k] - 3 * p2[k] + p3[k]) * t3)));
    }
  }
  out.push(closed ? pts[0].slice() : pts[n - 1].slice());
  return out;
}

export function nurbsCircle({ radius = 1, sections = 8, normal = [0, 1, 0] } = {}) {
  const cvs = [];
  const r = radius / Math.cos(Math.PI / sections) * 1.0;
  for (let i = 0; i < sections; i++) { const a = i / sections * TAU; cvs.push(orient([r * Math.cos(a) * 0.95, 0, -r * Math.sin(a) * 0.95], normal)); }
  return new Curve(cvs, 3, 'periodic');
}
export function nurbsSquare({ sideLength = 1, normal = [0, 1, 0] } = {}) {
  const h = sideLength / 2;
  return new Curve([[-h, 0, -h], [h, 0, -h], [h, 0, h], [-h, 0, h]].map(p => orient(p, normal)), 1, 'periodic');
}
function orient(p, n) {
  if (n[1] === 1 || (Math.abs(n[1]) > 0.99)) return p;
  if (Math.abs(n[0]) > 0.99) return [p[1], p[0], p[2]];
  return [p[0], -p[2], p[1]];
}

// Surfaces ------------------------------------------------------------------
// revolve: rotate profile points around axis through origin of the curve's space
export function revolve(curve, { axis = 1, startSweep = 0, endSweep = 360, sections = 8, segments = 8, outputSegments = null } = {}) {
  const prof = curve.sample(segments);
  const sweep = (endSweep - startSweep) * Math.PI / 180;
  const full = Math.abs(endSweep - startSweep) >= 359.999;
  const S = Math.max(3, sections * 4);
  const cols = full ? S : S + 1;
  const pm = new PolyMesh([], [], []);
  const rot = (p, a) => {
    const c = Math.cos(a), s = Math.sin(a);
    if (axis === 1) return [p[0] * c + p[2] * s, p[1], -p[0] * s + p[2] * c];
    if (axis === 0) return [p[0], p[1] * c - p[2] * s, p[1] * s + p[2] * c];
    return [p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2]];
  };
  // weld profile points lying on the axis
  const onAxis = (p) => axis === 1 ? Math.hypot(p[0], p[2]) < 1e-5 : axis === 0 ? Math.hypot(p[1], p[2]) < 1e-5 : Math.hypot(p[0], p[1]) < 1e-5;
  const ids = [];
  for (let j = 0; j < prof.length; j++) {
    const row = [];
    if (onAxis(prof[j])) { pm.v.push(prof[j].slice()); const id = pm.v.length - 1; for (let i = 0; i < cols; i++) row.push(id); }
    else for (let i = 0; i < cols; i++) { const a = startSweep * Math.PI / 180 + (i / S) * sweep; pm.v.push(rot(prof[j], a)); row.push(pm.v.length - 1); }
    ids.push(row);
  }
  const de = [];
  for (let j = 0; j + 1 < prof.length; j++) for (let i = 0; i < S; i++) {
    if (!full && i >= cols - 1) continue;
    const i2 = full ? (i + 1) % S : i + 1;
    const q = [ids[j][i], ids[j][i2], ids[j + 1][i2], ids[j + 1][i]];
    const uq = [[i / S, 1 - j / (prof.length - 1)], [(i + 1) / S, 1 - j / (prof.length - 1)], [(i + 1) / S, 1 - (j + 1) / (prof.length - 1)], [i / S, 1 - (j + 1) / (prof.length - 1)]];
    const keep = []; const ku = [];
    q.forEach((v, k) => { if (v !== q[(k + 1) % 4]) { keep.push(v); ku.push(uq[k]); } });
    if (keep.length >= 3) { pm.f.push(keep); pm.uv.push(ku); }
  }
  // isoparms: every 4th section line and every span boundary of the profile
  for (let i = 0; i < S; i += 4) for (let j = 0; j + 1 < prof.length; j++) if (ids[j][i] !== ids[j + 1][i]) de.push([ids[j][i], ids[j + 1][i]]);
  for (let j = 0; j < prof.length; j += segments) for (let i = 0; i < S; i++) { const i2 = full ? (i + 1) % S : i + 1; if (i2 < cols && ids[j][i] !== ids[j][i2]) de.push([ids[j][i], ids[j][i2]]); }
  pm.displayEdges = de;
  pm.softAngle = 70;
  fixOrientation(pm);
  return pm;
}
export function loft(curves, { segments = 8, close = false } = {}) {
  if (curves.length < 2) return new PolyMesh();
  const N = Math.max(...curves.map(c => c.spans)) * segments;
  const rows = curves.map(c => resample(c.sample(segments * 2), N));
  const closedU = curves.every(c => c.form === 'periodic');
  const cols = closedU ? N : N + 1;
  const pm = new PolyMesh([], [], []);
  const ids = rows.map(r => r.slice(0, cols).map(p => { pm.v.push(p); return pm.v.length - 1; }));
  const R = ids.length; const rowsN = close ? R : R - 1;
  for (let j = 0; j < rowsN; j++) for (let i = 0; i < (closedU ? N : N); i++) {
    const j2 = (j + 1) % R; const i2 = closedU ? (i + 1) % N : i + 1;
    pm.f.push([ids[j][i], ids[j][i2], ids[j2][i2], ids[j2][i]]);
    pm.uv.push([[i / N, j / rowsN], [(i + 1) / N, j / rowsN], [(i + 1) / N, (j + 1) / rowsN], [i / N, (j + 1) / rowsN]]);
  }
  const de = [];
  for (let j = 0; j < R; j++) for (let i = 0; i < (closedU ? N : N); i++) { const i2 = closedU ? (i + 1) % N : i + 1; de.push([ids[j][i], ids[j][i2]]); }
  for (let i = 0; i < cols; i += segments) for (let j = 0; j < rowsN; j++) de.push([ids[j][i], ids[(j + 1) % R][i]]);
  pm.displayEdges = de; pm.softAngle = 70;
  return pm;
}
export function extrudeAlong(profile, path, { segments = 8, scale = 1, twist = 0 } = {}) {
  const pp = path.sample(segments); const prof = profile.sample(segments);
  const closedU = profile.form === 'periodic';
  const Np = closedU ? prof.length - 1 : prof.length;
  // profile in its own frame relative to its centroid
  const c = [0, 0, 0]; for (let i = 0; i < Np; i++) for (let k = 0; k < 3; k++) c[k] += prof[i][k] / Np;
  const pm = new PolyMesh([], [], []); const ids = [];
  // parallel transport frames
  let T0 = V3.norm(V3.sub(pp[1], pp[0]));
  let N0 = Math.abs(T0[1]) < 0.9 ? V3.norm(V3.cross(T0, [0, 1, 0])) : V3.norm(V3.cross(T0, [1, 0, 0]));
  // profile basis: assume profile drawn in plane perpendicular to first tangent; map its local coords
  const B0 = V3.cross(T0, N0);
  const local = prof.slice(0, Np).map(p => { const d = V3.sub(p, c); return [V3.dot(d, N0), V3.dot(d, B0), V3.dot(d, T0)]; });
  let T = T0, Nn = N0;
  for (let s = 0; s < pp.length; s++) {
    if (s > 0) {
      const Tn = V3.norm(V3.sub(pp[Math.min(pp.length - 1, s + 1)], pp[s - 1]));
      const ax = V3.cross(T, Tn); const sl = V3.len(ax);
      if (sl > 1e-8) { const ang = Math.asin(Math.min(1, sl)); Nn = rotAxis(Nn, V3.mul(ax, 1 / sl), ang); }
      T = Tn;
    }
    const B = V3.cross(T, Nn); const u = s / (pp.length - 1); const sc = 1 + (scale - 1) * u; const tw = twist * Math.PI / 180 * u;
    const row = [];
    for (const l of local) {
      const x = (l[0] * Math.cos(tw) - l[1] * Math.sin(tw)) * sc, y = (l[0] * Math.sin(tw) + l[1] * Math.cos(tw)) * sc;
      pm.v.push(V3.add(pp[s], V3.add(V3.mul(Nn, x), V3.mul(B, y)))); row.push(pm.v.length - 1);
    }
    ids.push(row);
  }
  const de = [];
  for (let s = 0; s + 1 < ids.length; s++) for (let i = 0; i < (closedU ? Np : Np - 1); i++) {
    const i2 = (i + 1) % Np;
    pm.f.push([ids[s][i], ids[s][i2], ids[s + 1][i2], ids[s + 1][i]]);
    pm.uv.push([[i / Np, s / (ids.length - 1)], [(i + 1) / Np, s / (ids.length - 1)], [(i + 1) / Np, (s + 1) / (ids.length - 1)], [i / Np, (s + 1) / (ids.length - 1)]]);
  }
  for (let s = 0; s < ids.length; s += segments) for (let i = 0; i < (closedU ? Np : Np - 1); i++) de.push([ids[s][i], ids[s][(i + 1) % Np]]);
  for (let i = 0; i < Np; i += Math.max(1, segments / 2)) for (let s = 0; s + 1 < ids.length; s++) de.push([ids[s][i], ids[s + 1][i]]);
  pm.displayEdges = de; pm.softAngle = 70;
  fixOrientation(pm);
  return pm;
}
export function planar(curves, { segments = 8 } = {}) {
  const pm = new PolyMesh([], [], []);
  for (const c of curves) {
    const s = c.sample(segments); if (c.form === 'periodic' || V3.len(V3.sub(s[0], s[s.length - 1])) < 1e-4) s.pop();
    const base = pm.v.length; s.forEach(p => pm.v.push(p));
    pm.f.push(s.map((_, i) => base + i)); pm.uv.push(s.map((_, i) => [0.5 + 0.5 * Math.cos(i / s.length * TAU), 0.5 + 0.5 * Math.sin(i / s.length * TAU)]));
  }
  // face up by default
  pm.f.forEach((f, fi) => { if (pm.faceNormal(fi)[1] < 0) { pm.f[fi] = f.slice().reverse(); pm.uv[fi] = pm.uv[fi].slice().reverse(); } });
  pm.displayEdges = null; pm.softAngle = 30;
  return pm;
}
function resample(pts, N) {
  const L = [0]; for (let i = 1; i < pts.length; i++) L.push(L[i - 1] + V3.len(V3.sub(pts[i], pts[i - 1])));
  const tot = L[L.length - 1] || 1; const out = []; let k = 0;
  for (let s = 0; s <= N; s++) {
    const d = s / N * tot; while (k < L.length - 2 && L[k + 1] < d) k++;
    const seg = (L[k + 1] - L[k]) || 1; out.push(V3.lerp3(pts[k], pts[k + 1], (d - L[k]) / seg));
  }
  return out;
}
function rotAxis(v, k, a) {
  const c = Math.cos(a), s = Math.sin(a); const kv = V3.cross(k, v); const kd = V3.dot(k, v);
  return [v[0] * c + kv[0] * s + k[0] * kd * (1 - c), v[1] * c + kv[1] * s + k[1] * kd * (1 - c), v[2] * c + kv[2] * s + k[2] * kd * (1 - c)];
}
// orient faces so normals point away from the mesh centroid on average
function fixOrientation(pm) {
  const c = [0, 0, 0]; for (const p of pm.v) for (let k = 0; k < 3; k++) c[k] += p[k] / (pm.v.length || 1);
  let score = 0;
  pm.f.forEach((_, fi) => { const n = pm.faceNormal(fi); const fc = pm.faceCenter(fi); score += V3.dot(n, V3.sub(fc, c)); });
  if (score < 0) { pm.f = pm.f.map(f => f.slice().reverse()); if (pm.uv) pm.uv = pm.uv.map(u => u ? u.slice().reverse() : u); }
  pm.invalidate();
}
