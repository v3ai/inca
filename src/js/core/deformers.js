// Inca — deformers: nonlinear (bend / flare / sine / squash / twist / wave), cluster, soft modification,
// lattice (FFD) and blend shapes. Each deformer type registers { apply(handle, mesh, P), sig(handle, mesh) }
// on Rig.deformers; scene.js rebuildShape displays the result without touching construction history.
import * as THREE from 'three';
import { App } from './app.js';
import * as S from './scene.js';
import { Rig, registerKind, makeRigNode, RIG_MATS, lineGeom, circle, matSig, attrSig, node, rigName, fx } from './rigging.js';

const D2R = Math.PI / 180;
const clamp = (x, a, b) => Math.max(a, Math.min(b, x));

// ------------------------------------------------------------------ nonlinear deformer math (handle space)
export const NONLINEAR = {
  bend: { attrs: { envelope: 1, curvature: 0, lowBound: -1, highBound: 1 },
    fn(q, a) { const k = a.curvature; if (Math.abs(k) < 1e-6) return q; const yc = clamp(q.y, a.lowBound, a.highBound); const th = k * yc, R = 1 / k; const c = Math.cos(th), s = Math.sin(th); const ex = q.y - yc; return q.set(R - (R - q.x) * c + ex * s, (R - q.x) * s + ex * c, q.z); } },
  flare: { attrs: { envelope: 1, startFlareX: 1, startFlareZ: 1, endFlareX: 1, endFlareZ: 1, curve: 0, lowBound: -1, highBound: 1 },
    fn(q, a) { const span = (a.highBound - a.lowBound) || 1e-6; const t = clamp((q.y - a.lowBound) / span, 0, 1); const b = a.curve * Math.sin(Math.PI * t); q.x *= a.startFlareX + (a.endFlareX - a.startFlareX) * t + b; q.z *= a.startFlareZ + (a.endFlareZ - a.startFlareZ) * t + b; return q; } },
  sine: { attrs: { envelope: 1, amplitude: 0, wavelength: 2, offset: 0, dropoff: 0, lowBound: -1, highBound: 1 },
    fn(q, a) { const yc = clamp(q.y, a.lowBound, a.highBound); const m = Math.max(Math.abs(a.lowBound), Math.abs(a.highBound)) || 1; const d = a.dropoff >= 0 ? 1 - a.dropoff * Math.abs(yc) / m : 1 + a.dropoff * (1 - Math.abs(yc) / m); q.x += a.amplitude * Math.sin(2 * Math.PI * yc / (a.wavelength || 1e-3) + a.offset) * d; return q; } },
  squash: { attrs: { envelope: 1, factor: 0, expand: 1, maxExpandPos: 0.5, startSmoothness: 0, endSmoothness: 0, lowBound: -1, highBound: 1 },
    fn(q, a) { const sy = Math.max(0.01, 1 + a.factor); const yc = clamp(q.y, a.lowBound, a.highBound); const span = (a.highBound - a.lowBound) || 1e-6; const t = (yc - a.lowBound) / span;
      const e = Math.log(0.5) / Math.log(clamp(a.maxExpandPos, 0.02, 0.98)); let bulge = Math.sin(Math.PI * Math.pow(t, e));
      if (a.startSmoothness > 0 && t < 0.5) bulge *= Math.pow(Math.sin(Math.PI * t), a.startSmoothness); if (a.endSmoothness > 0 && t > 0.5) bulge *= Math.pow(Math.sin(Math.PI * t), a.endSmoothness);
      const sx = 1 + a.expand * (1 / Math.sqrt(sy) - 1) * bulge; q.x *= sx; q.z *= sx; q.y = yc * sy + (q.y - yc); return q; } },
  twist: { attrs: { envelope: 1, startAngle: 0, endAngle: 0, lowBound: -1, highBound: 1 },
    fn(q, a) { const span = (a.highBound - a.lowBound) || 1e-6; const t = clamp((q.y - a.lowBound) / span, 0, 1); const ang = (a.startAngle + (a.endAngle - a.startAngle) * t) * D2R; const c = Math.cos(ang), s = Math.sin(ang); return q.set(q.x * c + q.z * s, q.y, -q.x * s + q.z * c); } },
  wave: { attrs: { envelope: 1, amplitude: 0, wavelength: 1, offset: 0, dropoff: 0, dropoffPosition: 0, minRadius: 0, maxRadius: 1 },
    fn(q, a) { const r = Math.hypot(q.x, q.z); if (r < a.minRadius || r > a.maxRadius) return q; const d = clamp(1 - a.dropoff * Math.abs(r / (a.maxRadius || 1) - a.dropoffPosition), 0, 1); q.y += a.amplitude * Math.sin(2 * Math.PI * r / (a.wavelength || 1e-3) + a.offset) * d; return q; } },
};
function nonlinearApply(h, mesh, P) {
  const r = h.inca.rig, a = h.inca.extraAttrs || {}; const env = a.envelope ?? 1; const def = NONLINEAR[r.deformer]; if (!def || !env) return P;
  h.updateWorldMatrix(true, false); mesh.updateWorldMatrix(true, false);
  const A = h.matrixWorld.clone().invert().multiply(mesh.matrixWorld), B = mesh.matrixWorld.clone().invert().multiply(h.matrixWorld);
  const q = new THREE.Vector3(), o = new THREE.Vector3();
  return P.map(p => { q.set(p[0], p[1], p[2]).applyMatrix4(A); o.copy(q); def.fn(q, a); if (env !== 1) q.lerpVectors(o, q, env); q.applyMatrix4(B); return [q.x, q.y, q.z]; });
}
const handleSig = (h) => matSig(h) + attrSig(h);
function nonlinearDisplay(h) {
  const r = h.inca.rig || {}; const a = h.inca.extraAttrs || {}; const def = NONLINEAR[r.deformer]; if (!def) return;
  const sig = attrSig(h); if (h.userData.dispSig === sig) return; h.userData.dispSig = sig;
  const pos = []; const q = new THREE.Vector3();
  const add = (pts) => { const d = pts.map(p => def.fn(q.set(...p), a).toArray()); for (let i = 0; i + 1 < d.length; i++) pos.push(...d[i], ...d[i + 1]); };
  if (r.deformer === 'wave') {
    for (const rad of [a.minRadius, (a.minRadius + a.maxRadius) / 2, a.maxRadius]) if (rad > 0) add(Array.from({ length: 33 }, (_, i) => [rad * Math.cos(i / 32 * Math.PI * 2), 0, rad * Math.sin(i / 32 * Math.PI * 2)]));
    add(Array.from({ length: 41 }, (_, i) => [a.minRadius + (a.maxRadius - a.minRadius) * i / 40, 0, 0]));
    pos.push(0, 0, 0, 0, 0.3, 0);
  } else {
    const lo = a.lowBound ?? -1, hi = a.highBound ?? 1; const N = 24;
    for (const x of [-0.5, 0, 0.5]) add(Array.from({ length: N + 1 }, (_, i) => [x, lo + (hi - lo) * i / N, 0]));
    for (const y of [lo, (lo + hi) / 2, hi]) add(Array.from({ length: 17 }, (_, i) => [0.5 * Math.cos(i / 16 * Math.PI * 2), y, 0.5 * Math.sin(i / 16 * Math.PI * 2)]));
  }
  h.userData.icon.geometry.dispose(); h.userData.icon.geometry = lineGeom(pos);
}
// pick shape: a slim box along the handle axis
const handlePick = () => new THREE.BoxGeometry(0.25, 2, 0.25);

export function nextOrder() { let m = 0; App.world.traverse(n => { const o = n.inca?.rig?.order; if (o > m) m = o; }); return m + 1; }
export function meshesOf(list) { const out = []; for (const o of list) o.traverse(c => { if (c.inca && c.inca.kind === 'mesh' && !out.includes(c)) out.push(c); }); return out; }
function unionBox(meshes) { const b = new THREE.Box3(); for (const m of meshes) b.union(S.worldBBox(m)); return b; }
export function createNonlinear(type, meshes, attrs = {}) {
  const name = rigName(type);
  const h = makeRigNode('deformHandle', name + 'Handle', {}, handlePick()); h.userData.pickBias = -0.5;
  h.inca.rig = { deformer: type, name, targets: meshes.map(m => m.inca.id), order: nextOrder() };
  h.inca.extraAttrs = { ...NONLINEAR[type].attrs, ...attrs };
  const b = unionBox(meshes); const c = b.getCenter(new THREE.Vector3()), sz = b.getSize(new THREE.Vector3());
  const sc = type === 'wave' ? Math.max(sz.x, sz.z) / 2 || 1 : sz.y / 2 || 1;
  h.inca.t = c.toArray().map(fx); h.inca.s = [sc, sc, sc].map(fx); S.updateXform(h);
  return h;
}

// ------------------------------------------------------------------ cluster / soft modification
function clusterApply(h, mesh, P) {
  const r = h.inca.rig, a = h.inca.extraAttrs || {}; const env = a.envelope ?? 1; if (!env) return P;
  h.updateWorldMatrix(true, false); mesh.updateWorldMatrix(true, false);
  const M = mesh.matrixWorld, Minv = M.clone().invert();
  const X = Minv.clone().multiply(h.matrixWorld).multiply(new THREE.Matrix4().fromArray(r.bind).invert()).multiply(M);
  const out = P.slice(); const q = new THREE.Vector3(), o = new THREE.Vector3();
  if (r.deformer === 'cluster') {
    const W = r.weights?.[mesh.inca.id]; if (!W) return P;
    for (const k in W) { const i = +k; const p = P[i]; if (!p) continue; q.set(p[0], p[1], p[2]); o.copy(q); q.applyMatrix4(X).lerp(o, 1 - W[k] * env); out[i] = [q.x, q.y, q.z]; }
  } else { // softMod: smooth volume falloff around the falloff center
    const C = new THREE.Vector3(...r.center); const R = Math.max(1e-4, a.falloffRadius ?? 5);
    for (let i = 0; i < P.length; i++) { const p = P[i]; q.set(p[0], p[1], p[2]); const d = q.clone().applyMatrix4(M).distanceTo(C) / R; if (d >= 1) continue; const w = (1 - d * d * (3 - 2 * d)) * env; o.copy(q); q.applyMatrix4(X).lerp(o, 1 - w); out[i] = [q.x, q.y, q.z]; }
  }
  return out;
}
function clusterDisplay(h) {
  const r = h.inca.rig || {}; const sig = r.deformer + (h.inca.extraAttrs?.falloffRadius ?? ''); if (h.userData.dispSig === sig) return; h.userData.dispSig = sig;
  const pos = [];
  if (r.deformer === 'softMod') { // "S" glyph + small cross
    const pts = []; for (let i = 0; i <= 12; i++) { const t = i / 12 * Math.PI * 1.5; pts.push([0.18 * Math.cos(t + Math.PI * 0.5) , 0.18 + 0.18 * Math.sin(t + Math.PI * 0.5), 0]); }
    for (let i = 0; i <= 12; i++) { const t = i / 12 * Math.PI * 1.5; pts.push([-0.18 * Math.cos(t), -0.18 + 0.18 * Math.sin(-t), 0]); }
    for (let i = 0; i + 1 < pts.length; i++) pos.push(...pts[i], ...pts[i + 1]);
    pos.push(-0.5, 0, 0, -0.35, 0, 0, 0.35, 0, 0, 0.5, 0, 0);
  } else { const n = 18; for (let i = 0; i < n; i++) { const a0 = Math.PI * 0.25 + i / n * Math.PI * 1.5, a1 = Math.PI * 0.25 + (i + 1) / n * Math.PI * 1.5; pos.push(0.3 * Math.cos(a0), 0.3 * Math.sin(a0), 0, 0.3 * Math.cos(a1), 0.3 * Math.sin(a1), 0); } }
  h.userData.icon.geometry.dispose(); h.userData.icon.geometry = lineGeom(pos);
}
export function createCluster(entries /* [{mesh, verts:[...]|null}] */, { relative = false } = {}) {
  const name = rigName('cluster');
  const h = makeRigNode('clusterHandle', name + 'Handle', {}, new THREE.BoxGeometry(0.7, 0.7, 0.7));
  const c = new THREE.Vector3(); let n = 0; const weights = {};
  for (const { mesh, verts } of entries) {
    mesh.updateWorldMatrix(true, false); const vs = verts || mesh.inca.mesh.v.map((_, i) => i); const W = {};
    for (const i of vs) { const p = mesh.inca.mesh.v[i]; if (!p) continue; c.add(new THREE.Vector3(...p).applyMatrix4(mesh.matrixWorld)); n++; W[i] = 1; }
    weights[mesh.inca.id] = W;
  }
  if (n) c.multiplyScalar(1 / n);
  h.inca.t = c.toArray().map(fx); S.updateXform(h); h.updateWorldMatrix(true, false);
  h.inca.rig = { deformer: 'cluster', name, targets: entries.map(e => e.mesh.inca.id), order: nextOrder(), weights, bind: h.matrixWorld.toArray(), relative };
  h.inca.extraAttrs = { envelope: 1 };
  return h;
}
export function createSoftMod(meshes, center, { falloffRadius = 5 } = {}) {
  const name = rigName('softMod');
  const h = makeRigNode('clusterHandle', name + 'Handle', {}, new THREE.BoxGeometry(0.7, 0.7, 0.7));
  h.inca.t = center.toArray().map(fx); S.updateXform(h); h.updateWorldMatrix(true, false);
  h.inca.rig = { deformer: 'softMod', name, targets: meshes.map(m => m.inca.id), order: nextOrder(), center: center.toArray(), bind: h.matrixWorld.toArray() };
  h.inca.extraAttrs = { envelope: 1, falloffRadius };
  return h;
}

// ------------------------------------------------------------------ lattice (free-form deformation)
const bern = (n, i, t) => { let c = 1; for (let k = 1; k <= i; k++) c = c * (n - k + 1) / k; return c * Math.pow(t, i) * Math.pow(1 - t, n - i); };
export function latticeGrid(div) { const [S_, T, U] = div; const pts = []; for (let i = 0; i < S_; i++) for (let j = 0; j < T; j++) for (let k = 0; k < U; k++) pts.push([+(i / (S_ - 1) - 0.5).toFixed(6), +(j / (T - 1) - 0.5).toFixed(6), +(k / (U - 1) - 0.5).toFixed(6)]); return pts; }
export const latIndex = (div, i, j, k) => (i * div[1] + j) * div[2] + k;
function latDiv(h) { const a = h.inca.extraAttrs || {}; return [a.sDivisions || 2, a.tDivisions || 5, a.uDivisions || 2].map(x => Math.max(2, Math.min(12, Math.round(x)))); }
function latticeApply(h, mesh, P) {
  const r = h.inca.rig, a = h.inca.extraAttrs || {}; const env = a.envelope ?? 1; if (!env) return P;
  const div = latDiv(h); if (r.pts.length !== div[0] * div[1] * div[2]) return P;
  h.updateWorldMatrix(true, false); mesh.updateWorldMatrix(true, false);
  const A = new THREE.Matrix4().fromArray(r.base).invert().multiply(mesh.matrixWorld);
  const B = mesh.matrixWorld.clone().invert().multiply(h.matrixWorld);
  const q = new THREE.Vector3(), o = new THREE.Vector3(), L = new THREE.Vector3(); const e = 1e-4;
  const bs = new Array(div[0]), bt = new Array(div[1]), bu = new Array(div[2]);
  return P.map(p => {
    q.set(p[0], p[1], p[2]); o.copy(q); q.applyMatrix4(A);
    const s = q.x + 0.5, t = q.y + 0.5, u = q.z + 0.5;
    if (s < -e || s > 1 + e || t < -e || t > 1 + e || u < -e || u > 1 + e) return p;
    for (let i = 0; i < div[0]; i++) bs[i] = bern(div[0] - 1, i, clamp(s, 0, 1));
    for (let j = 0; j < div[1]; j++) bt[j] = bern(div[1] - 1, j, clamp(t, 0, 1));
    for (let k = 0; k < div[2]; k++) bu[k] = bern(div[2] - 1, k, clamp(u, 0, 1));
    L.set(0, 0, 0);
    for (let i = 0; i < div[0]; i++) for (let j = 0; j < div[1]; j++) { const w0 = bs[i] * bt[j]; if (!w0) continue; for (let k = 0; k < div[2]; k++) { const w = w0 * bu[k]; if (!w) continue; const c = r.pts[latIndex(div, i, j, k)]; L.x += w * c[0]; L.y += w * c[1]; L.z += w * c[2]; } }
    L.applyMatrix4(B); if (env !== 1) L.lerp(o, 1 - env);
    return [L.x, L.y, L.z];
  });
}
function latticeDisplay(h) {
  const r = h.inca.rig || {}; const div = latDiv(h); const pts = r.pts || [];
  const sig = div.join(',') + JSON.stringify(pts) + (r.sel || []).join(','); if (h.userData.dispSig === sig) return; h.userData.dispSig = sig;
  if (pts.length !== div[0] * div[1] * div[2]) return;
  const pos = []; const P = (i, j, k) => pts[latIndex(div, i, j, k)];
  for (let i = 0; i < div[0]; i++) for (let j = 0; j < div[1]; j++) for (let k = 0; k < div[2]; k++) {
    if (i + 1 < div[0]) pos.push(...P(i, j, k), ...P(i + 1, j, k)); if (j + 1 < div[1]) pos.push(...P(i, j, k), ...P(i, j + 1, k)); if (k + 1 < div[2]) pos.push(...P(i, j, k), ...P(i, j, k + 1));
  }
  h.userData.icon.geometry.dispose(); h.userData.icon.geometry = lineGeom(pos);
  const sel = new Set(r.sel || []); const col = []; pts.forEach((_, i) => { const c = sel.has(i) ? [1, 0.9, 0] : [0.76, 0.23, 0.84]; col.push(...c); });
  const g = lineGeom(pts.flat()); g.setAttribute('color', new THREE.Float32BufferAttribute(col, 3));
  if (!h.userData.points) { const p = S.helper(new THREE.Points(g, latPtsMat)); p.renderOrder = 4; h.add(p); h.userData.points = p; } else { h.userData.points.geometry.dispose(); h.userData.points.geometry = g; }
}
const latPtsMat = new THREE.PointsMaterial({ size: 6, sizeAttenuation: false, vertexColors: true, depthTest: false, transparent: true });
export function createLattice(meshes, { divisions = [2, 5, 2] } = {}) {
  const name = rigName('ffd');
  const h = makeRigNode('lattice', name + 'Lattice', {}, new THREE.BoxGeometry(1, 1, 1)); h.userData.pickBias = -1e4;
  const b = unionBox(meshes); const c = b.getCenter(new THREE.Vector3()), sz = b.getSize(new THREE.Vector3()).multiplyScalar(1.02);
  for (const k of ['x', 'y', 'z']) if (sz[k] < 1e-3) sz[k] = 0.1;
  h.inca.t = c.toArray().map(fx); h.inca.s = sz.toArray().map(fx); S.updateXform(h); h.updateWorldMatrix(true, false);
  h.inca.extraAttrs = { envelope: 1, sDivisions: divisions[0], tDivisions: divisions[1], uDivisions: divisions[2] };
  h.inca.rig = { deformer: 'ffd', name, targets: meshes.map(m => m.inca.id), order: nextOrder(), base: h.matrixWorld.toArray(), pts: latticeGrid(divisions), sel: [] };
  return h;
}

// ------------------------------------------------------------------ blend shapes (stored on the base mesh)
function targetHash(m) { const v = m.inca.mesh.v; let s = v.length; for (let i = 0; i < v.length; i++) { const p = v[i]; s += (p[0] * 1.31 + p[1] * 1.77 + p[2] * 2.13) * ((i % 7) + 1); } return s.toFixed(6); }
function blendApply(mesh, _m, P) {
  const bs = mesh.inca.rig.blendShape; const a = mesh.inca.extraAttrs || {}; const env = bs.envelope ?? 1;
  let out = null;
  for (const t of bs.targets) {
    const w = (a[t.name] || 0) * env; if (Math.abs(w) < 1e-7) continue;
    if (!out) out = P.map(p => p.slice());
    const tn = node(t.id);
    if (tn && tn.inca.kind === 'mesh' && tn.inca.mesh.v.length === P.length) { const tv = tn.inca.mesh.v; for (let i = 0; i < P.length; i++) { const d = tv[i], b = P[i], o = out[i]; o[0] += w * (d[0] - b[0]); o[1] += w * (d[1] - b[1]); o[2] += w * (d[2] - b[2]); } }
    else for (const [i, dx, dy, dz] of t.d || []) { const o = out[i]; if (o) { o[0] += w * dx; o[1] += w * dy; o[2] += w * dz; } }
  }
  return out || P;
}
function blendSig(mesh) { const bs = mesh.inca.rig.blendShape; const a = mesh.inca.extraAttrs || {}; let s = (bs.envelope ?? 1) + ':'; for (const t of bs.targets) { s += t.name + '=' + (a[t.name] || 0); const tn = node(t.id); if (tn && tn.inca.kind === 'mesh' && Math.abs(a[t.name] || 0) > 1e-7) s += '#' + targetHash(tn); s += ';'; } return s; }
export function createBlendShape(base, targets) {
  const rig = base.inca.rig = base.inca.rig || {};
  const bs = rig.blendShape = rig.blendShape || { name: rigName('blendShape'), envelope: 1, targets: [] };
  base.inca.extraAttrs = base.inca.extraAttrs || {};
  const bv = base.inca.mesh.v; const added = [];
  for (const t of targets) {
    const tv = t.inca.mesh.v; if (tv.length !== bv.length) { App.emit('warning', `// Warning: ${t.inca.name} has a different number of vertices than ${base.inca.name}; skipped.`); continue; }
    const d = []; for (let i = 0; i < bv.length; i++) { const dx = tv[i][0] - bv[i][0], dy = tv[i][1] - bv[i][1], dz = tv[i][2] - bv[i][2]; if (Math.abs(dx) + Math.abs(dy) + Math.abs(dz) > 1e-7) d.push([i, +dx.toFixed(6), +dy.toFixed(6), +dz.toFixed(6)]); }
    let nm = t.inca.name; while (nm in base.inca.extraAttrs) nm += '_';
    bs.targets.push({ name: nm, id: t.inca.id, d }); base.inca.extraAttrs[nm] = 0; added.push(nm);
  }
  S.rebuildShape(base);
  return { bs, added };
}

// ------------------------------------------------------------------ remove deformers
export function deleteDeformersOn(objs) {
  const meshes = meshesOf(objs.filter(o => o.inca.kind === 'mesh' || o.inca.kind === 'group'));
  const handles = []; App.world.traverse(n => { if (n.inca?.rig?.deformer) handles.push(n); });
  const kill = new Set(objs.filter(o => o.inca.rig?.deformer));
  for (const m of meshes) {
    const r = m.inca.rig; if (r) { if (r.blendShape) { for (const t of r.blendShape.targets) { delete m.inca.extraAttrs?.[t.name]; App.anim.deleteCurve(m, t.name); } delete r.blendShape; } delete r.skin; }
    for (const h of handles) { const t = h.inca.rig.targets; const i = t.indexOf(m.inca.id); if (i >= 0) { t.splice(i, 1); if (!t.length) kill.add(h); } }
  }
  for (const h of kill) S.deleteNode(h);
  for (const m of meshes) S.rebuildShape(m);
  return { meshes, removed: kill.size };
}

export function install() {
  registerKind('deformHandle', { create: (nm, opts) => { const o = makeRigNode('deformHandle', nm, opts, handlePick()); o.userData.pickBias = -0.5; return o; }, display: nonlinearDisplay, mat: RIG_MATS.deformer, show: 'deformers' });
  registerKind('clusterHandle', { create: (nm, opts) => makeRigNode('clusterHandle', nm, opts, new THREE.BoxGeometry(0.7, 0.7, 0.7)), display: clusterDisplay, mat: RIG_MATS.cluster, show: 'deformers' });
  registerKind('lattice', {
    create: (nm, opts) => { const o = makeRigNode('lattice', nm, opts, new THREE.BoxGeometry(1, 1, 1)); o.userData.pickBias = -1e4; return o; },
    display: latticeDisplay, mat: RIG_MATS.lattice, show: 'deformers',
    attrChanged(h, attr) { if (/^[stu]Divisions$/.test(attr)) { h.inca.rig.pts = latticeGrid(latDiv(h)); h.inca.rig.sel = []; } },
  });
  for (const t of Object.keys(NONLINEAR)) Rig.deformers[t] = { apply: nonlinearApply, sig: handleSig };
  Rig.deformers.cluster = { apply: clusterApply, sig: handleSig };
  Rig.deformers.softMod = { apply: clusterApply, sig: handleSig };
  Rig.deformers.ffd = { apply: latticeApply, sig: (h) => handleSig(h) + JSON.stringify(h.inca.rig.pts) };
  Rig.deformers.blendShape = { apply: blendApply, sig: blendSig };
}
