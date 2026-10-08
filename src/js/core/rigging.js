// Inca — rigging core: joints, IK handles, smooth skinning, the rig evaluation pipeline and the
// attribute hooks used by scene.js (extraAttrs / jointOrient) and the channel box.
// Rig node data lives in serializable `inca.rig` records (and keyable numbers in `inca.extraAttrs`),
// runtime objects (display geometry, solved deformed meshes) are rebuilt on demand.
import * as THREE from 'three';
import { App } from './app.js';
import * as S from './scene.js';
import { PolyMesh } from './polymesh.js';
import { uniqueName } from './names.js';
import { niceName } from './ops.js';

const D2R = Math.PI / 180, R2D = 180 / Math.PI;
const _p = new THREE.Vector3(), _s = new THREE.Vector3(), _q = new THREE.Quaternion(), _m = new THREE.Matrix4();

export const RIG_KINDS = new Set(['joint', 'ikHandle', 'deformHandle', 'clusterHandle', 'lattice', 'constraint']);
export const Rig = {
  kinds: {},          // kind -> { create(name, opts, data), display(o) -> sig, ... }
  deformers: {},      // deformer type -> { apply(mesh, P, handle), sig(handle) }
  evaluators: [],     // ordered evaluation passes: { order, run(ctx) }
  nonKeyable: new Set(['sDivisions', 'tDivisions', 'uDivisions']),
  _busy: false, _order: 1,
};
App.rig = Rig;

// ------------------------------------------------------------------ helpers
export const wpos = (o, out = new THREE.Vector3()) => { o.updateWorldMatrix(true, false); return out.setFromMatrixPosition(o.matrixWorld); };
export function wquat(o, out = new THREE.Quaternion()) { o.updateWorldMatrix(true, false); o.matrixWorld.decompose(_p, out, _s); return out; }
export function wscale(o, out = new THREE.Vector3()) { o.updateWorldMatrix(true, false); o.matrixWorld.decompose(_p, _q, out); return out; }
export function parentQuat(o) { const q = new THREE.Quaternion(); if (o.parent) { o.parent.updateWorldMatrix(true, false); o.parent.matrixWorld.decompose(_p, q, _s); } return q; }
export const fx = (n) => +(+n).toFixed(4);
export const node = (id) => { const n = App.nodes.get(id); return n && n.isObject3D ? n : null; };
export function allRigNodes() { const by = {}; App.world.traverse(n => { if (n.inca) (by[n.inca.kind] ||= []).push(n); }); return by; }
export function matSig(o) { o.updateWorldMatrix(true, false); let s = ''; const e = o.matrixWorld.elements; for (let i = 0; i < 16; i++) s += e[i].toFixed(5) + ','; return s; }
export function attrSig(o) { const a = o.inca.extraAttrs; if (!a) return ''; let s = ''; for (const k in a) s += k + '=' + a[k] + ';'; return s; }
// unique DG-style name across nodes and rig records (skinCluster1, bend1, ...)
export function rigName(base) {
  const used = new Set(App.names.keys());
  App.world.traverse(n => { const r = n.inca && n.inca.rig; if (!r) return; for (const k of ['name']) if (r[k]) used.add(r[k]); if (r.skin) used.add(r.skin.name); if (r.blendShape) used.add(r.blendShape.name); if (r.motionPath) used.add(r.motionPath.name); });
  let i = 1; while (used.has(base + i)) i++; return base + i;
}
// set the rotation channels so the world rotation equals q (respects parent and joint orient)
export function setWorldQuat(o, q) {
  const inc = o.inca;
  const ql = parentQuat(o).invert().multiply(q);
  if (inc.jo) ql.premultiply(S.joQuat(inc, new THREE.Quaternion()).invert());
  const e = new THREE.Euler().setFromQuaternion(ql, S.RO[inc.ro] || 'ZYX');
  inc.r = [e.x, e.y, e.z].map((a, i) => { a *= R2D; while (a - inc.r[i] > 180) a -= 360; while (a - inc.r[i] < -180) a += 360; return +a.toFixed(6); });
  S.updateXform(o);
}
export function setWorldPos(o, P) { // move the rotate pivot to world position P
  const inc = o.inca; const l = P.clone(); if (o.parent) { o.parent.updateWorldMatrix(true, false); l.applyMatrix4(o.parent.matrixWorld.clone().invert()); }
  inc.t = [l.x - inc.pt[0] - inc.p[0], l.y - inc.pt[1] - inc.p[1], l.z - inc.pt[2] - inc.p[2]].map(x => +x.toFixed(6));
  S.updateXform(o);
}
export function rotateWorldAbout(o, P, q) { // apply a world-space rotation q about point P to node o
  o.updateWorldMatrix(true, false);
  const m = new THREE.Matrix4().makeTranslation(P.x, P.y, P.z).multiply(new THREE.Matrix4().makeRotationFromQuaternion(q)).multiply(new THREE.Matrix4().makeTranslation(-P.x, -P.y, -P.z)).multiply(o.matrixWorld);
  S.setWorldMatrix(o, m);
}
// orthonormal frame (as quaternion) whose `a` axis points along dir and `u` axis is as close as possible to up
export function frameQuat(dir, up, aAxis = new THREE.Vector3(1, 0, 0), uAxis = new THREE.Vector3(0, 1, 0)) {
  const x = dir.clone().normalize();
  let y = up.clone().addScaledVector(x, -up.dot(x)); if (y.lengthSq() < 1e-10) { const alt = Math.abs(x.y) < 0.9 ? new THREE.Vector3(0, 1, 0) : new THREE.Vector3(1, 0, 0); y = alt.addScaledVector(x, -alt.dot(x)); }
  y.normalize(); const z = new THREE.Vector3().crossVectors(x, y);
  const W = new THREE.Matrix4().makeBasis(x, y, z);
  const la = aAxis.clone().normalize(); let lu = uAxis.clone().addScaledVector(la, -uAxis.dot(la)); if (lu.lengthSq() < 1e-10) lu = new THREE.Vector3(0, 0, 1).addScaledVector(la, -la.z); lu.normalize();
  const L = new THREE.Matrix4().makeBasis(la, lu, new THREE.Vector3().crossVectors(la, lu));
  return new THREE.Quaternion().setFromRotationMatrix(W.multiply(L.transpose()));
}
const lineMat = (c) => new THREE.LineBasicMaterial({ color: c });
export const RIG_MATS = { joint: lineMat(0x2c4fd0), ik: lineMat(0xc8c8c8), deformer: lineMat(0x1a1a8c), cluster: lineMat(0x101070), lattice: lineMat(0x7a2fa0), latticePts: new THREE.PointsMaterial({ size: 6, sizeAttenuation: false, color: 0xc23ad6, depthTest: false }) };
export function lineGeom(pos) { const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3)); g.computeBoundingSphere(); return g; }
export function circle(pos, r, axis, c = [0, 0, 0], n = 20) {
  for (let i = 0; i < n; i++) {
    const a = i / n * Math.PI * 2, b = (i + 1) / n * Math.PI * 2;
    const P = (t) => axis === 'x' ? [c[0], c[1] + r * Math.cos(t), c[2] + r * Math.sin(t)] : axis === 'y' ? [c[0] + r * Math.cos(t), c[1], c[2] + r * Math.sin(t)] : [c[0] + r * Math.cos(t), c[1] + r * Math.sin(t), c[2]];
    pos.push(...P(a), ...P(b));
  }
}
// common rig DAG node: line icon + pick mesh, display rebuilt from a signature in prepareNode
export function makeRigNode(kind, name, opts = {}, pickGeom = new THREE.BoxGeometry(1, 1, 1)) {
  const o = S.makeDag(kind, name, opts);
  o.inca.rig = o.inca.rig || {};
  const l = S.helper(new THREE.LineSegments(new THREE.BufferGeometry(), RIG_MATS.deformer)); l.userData.owner = o; o.add(l);
  o.userData.icon = l; o.userData.wire = l;
  if (pickGeom) S.addPick(o, pickGeom);
  return o;
}
export function registerKind(kind, impl) { Rig.kinds[kind] = impl; S.DAG_FACTORIES[kind] = (nm, opts, d) => impl.create(nm, opts, d); }

// ------------------------------------------------------------------ attribute hooks (called from scene.js)
const JO = { jointOrientX: 0, jointOrientY: 1, jointOrientZ: 2 };
Rig.getAttr = (rec, attr) => {
  const inc = rec.inca;
  if (inc.extraAttrs && attr in inc.extraAttrs) return inc.extraAttrs[attr];
  if (inc.kind === 'joint') { if (attr in JO) return (inc.jo || [0, 0, 0])[JO[attr]]; if (attr === 'radius') return inc.rig?.radius ?? 0.5; if (attr === 'jointOrient') return (inc.jo || [0, 0, 0]).slice(); }
  return undefined;
};
Rig.setAttr = (rec, attr, value) => {
  const inc = rec.inca;
  if (inc.extraAttrs && attr in inc.extraAttrs) {
    inc.extraAttrs[attr] = typeof inc.extraAttrs[attr] === 'number' ? +value : value;
    Rig.kinds[inc.kind]?.attrChanged?.(rec, attr);
    App.requestRender(); return true;
  }
  if (inc.kind === 'joint') {
    if (attr in JO) { inc.jo = (inc.jo || [0, 0, 0]).slice(); inc.jo[JO[attr]] = +value; S.updateXform(rec); return true; }
    if (attr === 'jointOrient') { inc.jo = value.map(Number); S.updateXform(rec); return true; }
    if (attr === 'radius') { inc.rig.radius = Math.max(0.01, +value); return true; }
  }
  return false;
};
// extra attributes are keyable (Channel Box "Key Selected", Set Key)
{ const orig = App.anim.keyableAttrs.bind(App.anim);
  App.anim.keyableAttrs = (n) => { const l = orig(n); if (n && n.isObject3D && n.inca.extraAttrs) for (const [k, v] of Object.entries(n.inca.extraAttrs)) if (typeof v === 'number' && !Rig.nonKeyable.has(k) && !n.inca.locks[k]) l.push(k); return l; };
}
const LABELS = { ikBlend: 'Ik Blend', poleVectorX: 'Pole Vector X', poleVectorY: 'Pole Vector Y', poleVectorZ: 'Pole Vector Z', uValue: 'U Value', sDivisions: 'S Divisions', tDivisions: 'T Divisions', uDivisions: 'U Divisions' };
Rig.channelRows = (o) => {
  const a = o.inca.extraAttrs; if (!a) return [];
  const rows = []; const rig = o.inca.rig || {};
  const groups = new Map(); // section -> attrs
  const sectionOf = (k) => {
    if (rig.blendShape && rig.blendShape.targets.some(t => t.name === k)) return rig.blendShape.name;
    if (k === 'uValue' && rig.motionPath) return rig.motionPath.name;
    if (o.inca.kind === 'deformHandle' || o.inca.kind === 'clusterHandle' || o.inca.kind === 'lattice') return rig.name || '';
    return '';
  };
  for (const [k, v] of Object.entries(a)) { if (typeof v !== 'number' && typeof v !== 'boolean') continue; const s = sectionOf(k); if (!groups.has(s)) groups.set(s, []); groups.get(s).push(k); }
  for (const [s, ks] of groups) {
    if (s) rows.push({ section: s });
    for (const k of ks) rows.push({ attr: k, label: LABELS[k] || (rig.blendShape && s === rig.blendShape.name ? k : niceName(k)), opts: typeof a[k] === 'boolean' ? { type: 'bool' } : /Divisions$/.test(k) ? { type: 'int', min: 2, max: 12 } : {} });
  }
  return rows;
};

// ------------------------------------------------------------------ joints
export function createJoint(name = 'joint1', opts = {}) {
  const o = makeRigNode('joint', name, opts, new THREE.SphereGeometry(0.5, 8, 6));
  o.inca.rig.radius = o.inca.rig.radius ?? 0.5; o.inca.jo = o.inca.jo || [0, 0, 0];
  o.userData.icon.material = RIG_MATS.joint; o.userData.pickBias = 1000;
  return o;
}
function jointDisplay(o) {
  const r = (o.inca.rig?.radius ?? 0.5) * (App.prefs?.opt?.jointDisplayScale || 1);
  const kids = S.dagChildren(o).filter(c => c.inca.kind === 'joint');
  let sig = r + '|'; for (const c of kids) { const e = c.matrix.elements; sig += e[12].toFixed(4) + ',' + e[13].toFixed(4) + ',' + e[14].toFixed(4) + ';'; }
  if (o.userData.dispSig === sig) return;
  o.userData.dispSig = sig;
  const pos = []; circle(pos, r, 'x'); circle(pos, r, 'y'); circle(pos, r, 'z');
  for (const c of kids) {
    const q = new THREE.Vector3().setFromMatrixPosition(c.matrix); const L = q.length(); if (L < r * 0.5) continue;
    const d = q.clone().normalize(); const p1 = new THREE.Vector3(Math.abs(d.x) < 0.9 ? 1 : 0, Math.abs(d.x) < 0.9 ? 0 : 1, 0).addScaledVector(d, -(Math.abs(d.x) < 0.9 ? d.x : d.y)).normalize(); const p2 = d.clone().cross(p1);
    const base = [p1, p2, p1.clone().negate(), p2.clone().negate()].map(v => v.multiplyScalar(r * 0.55).addScaledVector(d, r * 0.5));
    base.forEach((b, i) => { pos.push(b.x, b.y, b.z, q.x, q.y, q.z); const n = base[(i + 1) % 4]; pos.push(b.x, b.y, b.z, n.x, n.y, n.z); });
  }
  o.userData.icon.geometry.dispose(); o.userData.icon.geometry = lineGeom(pos);
  // pick geometry: sphere + a thin box along each bone so bones can be clicked
  const geoms = [new THREE.SphereGeometry(r, 8, 6)];
  for (const c of kids) { const q = new THREE.Vector3().setFromMatrixPosition(c.matrix); const L = q.length(); if (L < 1e-4) continue; const b = new THREE.BoxGeometry(L, r * 0.5, r * 0.5); b.translate(L / 2, 0, 0); b.applyQuaternion(new THREE.Quaternion().setFromUnitVectors(new THREE.Vector3(1, 0, 0), q.clone().normalize())); geoms.push(b); }
  const merged = mergeGeoms(geoms);
  o.userData.pick.geometry.dispose(); o.userData.pick.geometry = merged;
}
function mergeGeoms(list) {
  const pos = []; const idx = []; let off = 0;
  for (const g of list) { const ng = g.index ? g : g; const p = ng.attributes.position.array; for (let i = 0; i < p.length; i++) pos.push(p[i]); if (ng.index) for (const i of ng.index.array) idx.push(i + off); else for (let i = 0; i < p.length / 3; i++) idx.push(i + off); off += p.length / 3; g.dispose(); }
  const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3)); g.setIndex(idx); g.computeBoundingSphere(); return g;
}
registerKind('joint', { create: (nm, opts) => createJoint(nm, opts), display: jointDisplay, mat: RIG_MATS.joint, show: 'joints' });

// orient one joint so its X axis points at its first child joint (secondary axis Y towards world up)
export function orientJoint(o, { upAxis = new THREE.Vector3(0, 1, 0) } = {}) {
  const kids = S.dagChildren(o);
  const kidsWorld = kids.map(c => S.worldMatrix(c));
  const jkids = kids.filter(c => c.inca.kind === 'joint');
  let Q;
  if (jkids.length) {
    const P = wpos(o); const x = wpos(jkids[0]).sub(P);
    if (x.lengthSq() < 1e-12) Q = parentQuat(o);
    else if (Math.abs(x.clone().normalize().dot(upAxis)) < 0.99) Q = frameQuat(x, upAxis);
    else { // bone parallel to up: keep the Z axis on world Z (front-view chains bend about Z)
      const xn = x.clone().normalize(); let z = new THREE.Vector3(0, 0, 1).addScaledVector(xn, -xn.z); if (z.lengthSq() < 1e-8) z = new THREE.Vector3(1, 0, 0).addScaledVector(xn, -xn.x); z.normalize();
      const y = new THREE.Vector3().crossVectors(z, xn); Q = new THREE.Quaternion().setFromRotationMatrix(new THREE.Matrix4().makeBasis(xn, y, z));
    }
  } else Q = parentQuat(o);
  const inc = o.inca; inc.r = [0, 0, 0];
  const ql = parentQuat(o).invert().multiply(Q);
  const e = new THREE.Euler().setFromQuaternion(ql, 'ZYX');
  inc.jo = [e.x, e.y, e.z].map(a => +(a * R2D).toFixed(6));
  S.updateXform(o);
  kids.forEach((c, i) => S.setWorldMatrix(c, kidsWorld[i]));
}
export function orientHierarchy(roots, opts) { const seen = new Set(); for (const r of roots) r.traverse(c => { if (c.inca && c.inca.kind === 'joint' && !seen.has(c)) { seen.add(c); orientJoint(c, opts); } }); }
export function createJointAt(P, parent = null, name = 'joint1') {
  const o = createJoint(name, { parent: parent || undefined });
  setWorldPos(o, P);
  return o;
}
export function jointChain(start, end) { // [start ... end] or null
  const path = []; let c = end; while (c && c.inca) { path.unshift(c); if (c === start) return path; c = c.parent; }
  return null;
}

// ------------------------------------------------------------------ IK handles
export function createIkHandle(start, end, { solver = 'ikRPsolver', name = 'ikHandle1' } = {}) {
  const chain = jointChain(start, end); if (!chain || chain.length < 2) throw new Error('End joint must be a descendant of the start joint');
  const o = makeRigNode('ikHandle', name, {}, new THREE.BoxGeometry(0.6, 0.6, 0.6));
  o.userData.icon.material = RIG_MATS.ik;
  setWorldPos(o, wpos(end));
  if (chain.length !== 3) solver = 'ikSCsolver';
  o.inca.rig = { start: start.inca.id, end: end.inca.id, chain: chain.map(j => j.inca.id), solver, rest: Object.fromEntries(chain.map(j => [j.inca.id, j.inca.r.slice()])) };
  const ea = { ikBlend: 1, twist: 0 };
  if (solver === 'ikRPsolver') {
    const A = wpos(chain[0]), B = wpos(chain[1]), C = wpos(chain[2]);
    const ac = C.clone().sub(A).normalize(); let pv = B.clone().sub(A); pv.addScaledVector(ac, -pv.dot(ac));
    if (pv.lengthSq() < 1e-8) { const z = new THREE.Vector3(0, 0, 1).applyQuaternion(wquat(chain[0])); pv = new THREE.Vector3().crossVectors(z, ac); }
    pv.normalize();
    Object.assign(ea, { poleVectorX: fx(pv.x), poleVectorY: fx(pv.y), poleVectorZ: fx(pv.z) });
  }
  o.inca.extraAttrs = ea;
  return o;
}
function ikDisplay(o) {
  const st = node(o.inca.rig?.start); const pos = [];
  const s = 0.35; pos.push(-s, 0, 0, s, 0, 0, 0, -s, 0, 0, s, 0, 0, 0, -s, 0, 0, s);
  if (st) { const p = wpos(st); o.updateWorldMatrix(true, false); p.applyMatrix4(o.matrixWorld.clone().invert()); pos.push(0, 0, 0, p.x, p.y, p.z); }
  const sig = pos.map(x => x.toFixed(3)).join(',');
  if (o.userData.dispSig === sig) return; o.userData.dispSig = sig;
  o.userData.icon.geometry.dispose(); o.userData.icon.geometry = lineGeom(pos);
}
registerKind('ikHandle', { create: (nm, opts) => { const o = makeRigNode('ikHandle', nm, opts, new THREE.BoxGeometry(0.6, 0.6, 0.6)); o.userData.icon.material = RIG_MATS.ik; return o; }, display: ikDisplay, mat: RIG_MATS.ik, show: 'ikHandles' });

function solveIk(h, ctx) {
  const rig = h.inca.rig; const a = h.inca.extraAttrs || {};
  const chain = (rig.chain || []).map(node); if (chain.length < 2 || chain.some(j => !j)) return;
  const before = chain.map(j => j.inca.r.slice());
  const fk = chain.map(j => j.inca.r.slice());
  // start from the rest pose so the solve is stateless (undo / scrubbing safe)
  for (const j of chain) { const r = rig.rest?.[j.inca.id]; if (r) { j.inca.r = r.slice(); S.updateXform(j); } }
  const blend = Math.max(0, Math.min(1, a.ikBlend ?? 1));
  if (blend > 0) {
    const T = wpos(h);
    if (rig.solver === 'ikRPsolver' && chain.length === 3) solveTwoBone(chain, T, new THREE.Vector3(a.poleVectorX || 0, a.poleVectorY || 0, a.poleVectorZ || 0), (a.twist || 0) * D2R);
    else solveCCD(chain, T);
    if (blend < 1) for (let i = 0; i < chain.length; i++) {
      const j = chain[i]; const ro = S.RO[j.inca.ro] || 'ZYX';
      const qi = new THREE.Quaternion().setFromEuler(new THREE.Euler(...j.inca.r.map(x => x * D2R), ro));
      const qf = new THREE.Quaternion().setFromEuler(new THREE.Euler(...fk[i].map(x => x * D2R), ro));
      const e = new THREE.Euler().setFromQuaternion(qf.slerp(qi, blend), ro); j.inca.r = [e.x * R2D, e.y * R2D, e.z * R2D]; S.updateXform(j);
    }
  } else for (let i = 0; i < chain.length; i++) { chain[i].inca.r = fk[i]; S.updateXform(chain[i]); }
  chain.forEach((j, i) => { if (j.inca.r.some((v, k) => Math.abs(v - before[i][k]) > 1e-5)) ctx.changed = true; });
}
function solveTwoBone(chain, T, pole, twist) {
  const [J0, J1, J2] = chain;
  const A = wpos(J0), B = wpos(J1), C = wpos(J2);
  const l1 = B.distanceTo(A), l2 = C.distanceTo(B); if (l1 < 1e-6 || l2 < 1e-6) return;
  let toT = T.clone().sub(A); let d = toT.length(); if (d < 1e-6) return;
  const u = toT.clone().normalize();
  d = Math.max(Math.abs(l1 - l2) + 1e-5, Math.min(l1 + l2 - 1e-5, d));
  let pv = pole.clone(); if (twist) pv.applyAxisAngle(u, twist);
  let n = pv.addScaledVector(u, -pv.dot(u));
  if (n.lengthSq() < 1e-10) { n = B.clone().sub(A); n.addScaledVector(u, -n.dot(u)); if (n.lengthSq() < 1e-10) n = new THREE.Vector3(0, 1, 0).addScaledVector(u, -u.y); }
  n.normalize();
  const cosA = Math.max(-1, Math.min(1, (l1 * l1 + d * d - l2 * l2) / (2 * l1 * d))); const sinA = Math.sqrt(1 - cosA * cosA);
  const Bn = A.clone().addScaledVector(u, l1 * cosA).addScaledVector(n, l1 * sinA);
  // rotate the start joint: map rest (bone dir, bend-plane normal) onto the solved ones
  const bR = B.clone().sub(A).normalize(); let nR = new THREE.Vector3().crossVectors(C.clone().sub(A), B.clone().sub(A));
  if (nR.lengthSq() < 1e-10) nR = new THREE.Vector3(0, 0, 1).applyQuaternion(wquat(J0)); nR.normalize();
  const bN = Bn.clone().sub(A).normalize(); const nN = new THREE.Vector3().crossVectors(u, n).normalize(); // normal of plane (A,T,pole), oriented like rest cross(C-A, B-A)
  const fr = (b, m) => { const mm = m.clone().addScaledVector(b, -m.dot(b)).normalize(); return new THREE.Matrix4().makeBasis(b, mm, new THREE.Vector3().crossVectors(b, mm)); };
  const R1 = fr(bN, nN).multiply(fr(bR, nR).transpose());
  rotateWorldAbout(J0, A, new THREE.Quaternion().setFromRotationMatrix(R1));
  // rotate the middle joint so the end reaches the (clamped) target
  const B1 = wpos(J1), C1 = wpos(J2); const Tc = A.clone().addScaledVector(u, d);
  const q2 = new THREE.Quaternion().setFromUnitVectors(C1.clone().sub(B1).normalize(), Tc.clone().sub(B1).normalize());
  rotateWorldAbout(J1, B1, q2);
}
function solveCCD(chain, T) {
  const end = chain[chain.length - 1];
  for (let it = 0; it < 24; it++) {
    for (let i = chain.length - 2; i >= 0; i--) {
      const P = wpos(chain[i]), E = wpos(end);
      const a = E.sub(P), b = T.clone().sub(P); if (a.lengthSq() < 1e-12 || b.lengthSq() < 1e-12) continue;
      const q = new THREE.Quaternion().setFromUnitVectors(a.normalize(), b.normalize());
      if (Math.abs(q.w) > 0.9999999) continue;
      rotateWorldAbout(chain[i], P, q);
    }
    if (wpos(end).distanceTo(T) < 1e-4) break;
  }
}

// ------------------------------------------------------------------ smooth skin (linear blend skinning)
export function computeSkinWeights(mesh, joints, { maxInfluences = 4, dropoffRate = 4 } = {}) {
  mesh.updateWorldMatrix(true, false);
  const segs = joints.map(j => { const P = wpos(j); const kids = S.dagChildren(j).filter(c => c.inca.kind === 'joint').map(c => wpos(c)); return { P, kids }; });
  const v = new THREE.Vector3(), tmp = new THREE.Vector3();
  const segDist = (p, a, b) => { const ab = tmp.copy(b).sub(a); const L2 = ab.lengthSq() || 1e-12; let t = p.clone().sub(a).dot(ab) / L2; t = Math.max(0, Math.min(1, t)); return p.distanceTo(a.clone().addScaledVector(ab, t)); };
  return mesh.inca.mesh.v.map(p => {
    v.set(p[0], p[1], p[2]).applyMatrix4(mesh.matrixWorld);
    const ws = segs.map((s, ji) => { let d = s.kids.length ? Math.min(...s.kids.map(k => segDist(v, s.P, k))) : v.distanceTo(s.P); return [ji, 1 / Math.pow(Math.max(d, 1e-3), dropoffRate)]; });
    ws.sort((a, b) => b[1] - a[1]); const top = ws.slice(0, Math.max(1, maxInfluences)); const sum = top.reduce((s, x) => s + x[1], 0) || 1;
    const out = []; for (const [ji, w] of top) { const nw = +(w / sum).toFixed(5); if (nw > 1e-4) out.push(ji, nw); }
    return out;
  });
}
export function smoothBind(joints, mesh, opts = {}) {
  const w = computeSkinWeights(mesh, joints, opts);
  mesh.updateWorldMatrix(true, false);
  const rig = mesh.inca.rig = mesh.inca.rig || {};
  rig.skin = { name: rigName('skinCluster'), joints: joints.map(j => j.inca.id), bindInv: joints.map(j => { j.updateWorldMatrix(true, false); return j.matrixWorld.clone().invert().toArray().map(x => +x.toFixed(7)); }), bindShape: mesh.matrixWorld.toArray(), w, maxInfluences: opts.maxInfluences || 4, envelope: 1 };
  S.rebuildShape(mesh);
  return rig.skin;
}
function skinSig(mesh) { const sk = mesh.inca.rig.skin; let s = 'sk' + sk.w.length + (sk._v || 0) + ':'; for (const id of sk.joints) { const j = node(id); s += j ? matSig(j) : 'x'; } return s + matSig(mesh); }
function applySkin(mesh, P) {
  const sk = mesh.inca.rig.skin; if (!sk || sk.w.length !== P.length) return P;
  mesh.updateWorldMatrix(true, false);
  const Minv = mesh.matrixWorld.clone().invert(); const BS = new THREE.Matrix4().fromArray(sk.bindShape);
  const K = sk.joints.map((id, i) => { const j = node(id); const m = new THREE.Matrix4(); if (!j) return m.copy(Minv).multiply(BS); j.updateWorldMatrix(true, false); return m.copy(Minv).multiply(j.matrixWorld).multiply(new THREE.Matrix4().fromArray(sk.bindInv[i])).multiply(BS); }).map(m => m.elements);
  const out = new Array(P.length);
  for (let i = 0; i < P.length; i++) {
    const p = P[i], W = sk.w[i]; let x = 0, y = 0, z = 0, ws = 0;
    for (let k = 0; k < W.length; k += 2) { const e = K[W[k]]; if (!e) continue; const w = W[k + 1]; ws += w; x += w * (e[0] * p[0] + e[4] * p[1] + e[8] * p[2] + e[12]); y += w * (e[1] * p[0] + e[5] * p[1] + e[9] * p[2] + e[13]); z += w * (e[2] * p[0] + e[6] * p[1] + e[10] * p[2] + e[14]); }
    out[i] = ws > 1e-6 ? [x / ws, y / ws, z / ws] : p;
  }
  return out;
}

// ------------------------------------------------------------------ deformation (called from scene.js rebuildShape)
// ordered deformer stack of a mesh: blend shape -> skin cluster -> handle deformers in creation order
export function deformerStack(mesh, handles = null) {
  const rig = mesh.inca.rig || {}; const st = [];
  if (rig.blendShape && Rig.deformers.blendShape) st.push({ type: 'blendShape', h: mesh });
  if (rig.skin) st.push({ type: 'skin', h: mesh });
  const id = mesh.inca.id; const hs = [];
  for (const h of handles || allHandles()) { const r = h.inca.rig; if (r && r.targets && r.targets.includes(id) && Rig.deformers[r.deformer]) hs.push(h); }
  hs.sort((a, b) => (a.inca.rig.order || 0) - (b.inca.rig.order || 0));
  for (const h of hs) st.push({ type: h.inca.rig.deformer, h });
  return st;
}
let _handles = null;
export function allHandles() { if (_handles) return _handles; const out = []; App.world.traverse(n => { if (n.inca && n.inca.rig && n.inca.rig.deformer && n.inca.rig.targets) out.push(n); }); return out; }
function stackSig(mesh, st) {
  let s = '';
  for (const d of st) s += d.type + ':' + (d.type === 'skin' ? skinSig(mesh) : Rig.deformers[d.type].sig(d.h, mesh)) + '|';
  if (st.some(d => d.type !== 'skin' && d.type !== 'blendShape')) s += matSig(mesh);
  return s;
}
Rig.deform = (o) => {
  const base = o.inca.mesh; if (!base) return null;
  const st = deformerStack(o);
  o.userData.rigSig = st.length ? stackSig(o, st) : '';
  if (!st.length) { o.userData.deformed = null; return null; }
  let P = base.v;
  for (const d of st) { try { P = (d.type === 'skin' ? applySkin(o, P) : Rig.deformers[d.type].apply(d.h, o, P)) || P; } catch (e) { console.error('deformer', d.type, e); } }
  const dm = new PolyMesh(P, base.f, base.uv, base.fm);
  dm.softAngle = base.softAngle; dm.edgeHard = base.edgeHard; dm.displayEdges = base.displayEdges; dm._topo = base.topo;
  o.userData.deformed = dm;
  return dm;
};
export const displayedMesh = (o) => o.userData.deformed || o.inca.mesh;

// ------------------------------------------------------------------ evaluation pipeline
// passes (lower order first): 10 driven keys, 20 motion paths, 30 constraints, 40 pole vectors, 50 IK, 60 constraints (2nd pass), 90 deformers
Rig.addPass = (order, run) => { Rig.evaluators.push({ order, run }); Rig.evaluators.sort((a, b) => a.order - b.order); };
Rig.addPass(50, (ctx) => { for (const h of ctx.by.ikHandle || []) if (h.visible !== false || true) solveIk(h, ctx); });
Rig.addPass(90, (ctx) => {
  _handles = allHandles();
  try {
    for (const m of ctx.by.mesh || []) {
      const st = deformerStack(m, _handles);
      if (!st.length) { if (m.userData.deformed) { m.userData.deformed = null; m.userData.rigSig = ''; S.rebuildShape(m); } continue; }
      const sig = stackSig(m, st);
      if (sig !== m.userData.rigSig) S.rebuildShape(m);
    }
  } finally { _handles = null; }
});
Rig.evaluate = () => {
  if (Rig._busy) return; Rig._busy = true;
  const ctx = { by: allRigNodes(), changed: false };
  try { for (const p of Rig.evaluators) { try { p.run(ctx); } catch (e) { console.error('rig pass', e); } } }
  finally { Rig._busy = false; }
  if (ctx.changed) { App.dirty('channels'); }
  App.requestRender();
  return ctx.changed;
};
let _frameDone = false;
Rig.beforeRender = () => {
  if (_frameDone || Rig._busy) return; _frameDone = true;
  requestAnimationFrame(() => { _frameDone = false; });
  const changed = Rig.evaluate.call(Rig);
  if (changed) for (const v of App.viewports) v.needsRender = true;
};
// run after keyframe animation (App.setTime -> anim.evaluate -> rig)
{ const orig = App.anim.evaluate.bind(App.anim); App.anim.evaluate = (t) => { orig(t); Rig.evaluate(); }; }
App.on('sceneLoaded', () => { for (const m of allRigNodes().mesh || []) m.userData.rigSig = null; Rig.evaluate(); });

// ------------------------------------------------------------------ viewport display hook (viewport.js prepareScene)
Rig.prepareNode = (n, wireMat, vopts) => {
  const k = Rig.kinds[n.inca.kind]; if (!k) return;
  const ud = n.userData;
  if (k.display) k.display(n, vopts);
  const show = vopts.show[k.show] !== false;
  if (ud.icon) { ud.icon.visible = show; ud.icon.material = wireMat || k.mat || RIG_MATS.deformer; }
  if (ud.points) ud.points.visible = show;
};

// ------------------------------------------------------------------ submodules
// (installed after this module's body ran: they use the helpers above)
import { install as installDeformers } from './deformers.js';
import { install as installConstraints } from './constraints.js';
import { install as installTools } from '../ui/rigTools.js';
installDeformers(); installConstraints(); installTools();
export { uniqueName };
