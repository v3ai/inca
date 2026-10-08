// Inca — constraints (parent / point / orient / scale / aim / pole vector), set driven keys and motion paths.
// Constraint nodes are DAG children of the constrained object (kind 'constraint', like Maya) with target
// weights as keyable extra attributes. Driven keys and motion paths live in the driven node's inca.rig.
import * as THREE from 'three';
import { App } from './app.js';
import * as S from './scene.js';
import { AnimCurve } from './anim.js';
import { Rig, registerKind, node, wpos, wquat, wscale, parentQuat, setWorldQuat, setWorldPos, frameQuat, fx } from './rigging.js';

export const CON_TYPES = ['parent', 'point', 'orient', 'scale', 'aim', 'poleVector'];
const V3 = (a) => new THREE.Vector3(a[0], a[1], a[2]);
const AXIS = { x: [1, 0, 0], y: [0, 1, 0], z: [0, 0, 1], '-x': [-1, 0, 0], '-y': [0, -1, 0], '-z': [0, 0, -1] };

function constraintNode(nm, opts) { const o = S.makeDag('constraint', nm, opts); o.inca.rig = o.inca.rig || {}; o.inca.selectable = true; return o; }
export function constrained(c) { return S.dagParent(c); }
const pivotWorld = (o) => S.worldPivot(o);
function blendQuats(qs, ws) { const out = qs[0].clone(); let acc = ws[0]; for (let i = 1; i < qs.length; i++) { acc += ws[i]; if (acc > 1e-9) out.slerp(qs[i].dot(out) < 0 ? qs[i].clone().set(-qs[i].x, -qs[i].y, -qs[i].z, -qs[i].w) : qs[i], ws[i] / acc); } return out; }

// ------------------------------------------------------------------ create
export function createConstraint(type, targets, driven, { maintainOffset = true, aimVector = [1, 0, 0], upVector = [0, 1, 0], worldUpType = 'vector', worldUpVector = [0, 1, 0], worldUpObject = null, weight = 1 } = {}) {
  if (type === 'poleVector' && driven.inca.kind !== 'ikHandle') throw new Error('Pole vector constraint needs an IK handle (rotate plane solver) as the constrained object');
  // an existing constraint of the same type gets the new targets (Maya adds targets)
  let c = S.dagChildren(driven).find(k => k.inca.kind === 'constraint' && k.inca.rig.type === type);
  const fresh = !c;
  if (!c) {
    c = constraintNode(driven.inca.name + '_' + type + 'Constraint1', { parent: driven });
    c.inca.rig = { type, targets: [], wAttrs: [], offsets: [], maintainOffset, aimVector, upVector, worldUpType, worldUpVector, worldUpObject: worldUpObject ? worldUpObject.inca.id : null };
    c.inca.extraAttrs = {};
  }
  const r = c.inca.rig;
  for (const t of targets) {
    if (r.targets.includes(t.inca.id)) continue;
    const i = r.targets.length; const a = t.inca.name + 'W' + i;
    r.targets.push(t.inca.id); r.wAttrs.push(a); c.inca.extraAttrs[a] = weight;
  }
  computeOffsets(c, maintainOffset);
  if (fresh) for (const a of ['translateX', 'translateY', 'translateZ', 'rotateX', 'rotateY', 'rotateZ', 'scaleX', 'scaleY', 'scaleZ']) c.inca.locks[a] = false;
  evalConstraint(c, {});
  return c;
}
function computeOffsets(c, maintain) {
  const r = c.inca.rig; const d = constrained(c); if (!d) return;
  r.maintainOffset = maintain;
  const T = r.targets.map(node);
  if (r.type === 'parent') r.offsets = T.map(t => { if (!t || !maintain) return null; return t.matrixWorld.clone().invert().multiply(S.worldMatrix(d)).toArray(); });
  else if (r.type === 'point') { r.offsets = []; r.offset = [0, 0, 0]; if (maintain) { const avg = weightedPos(c); if (avg) r.offset = pivotWorld(d).sub(avg).toArray(); } }
  else if (r.type === 'orient') r.offsets = T.map(t => maintain && t ? wquat(t).invert().multiply(wquat(d)).toArray() : null);
  else if (r.type === 'scale') { r.offset = [1, 1, 1]; if (maintain) { const s = weightedScale(c); if (s) { const ds = wscale(d); r.offset = [ds.x / (s.x || 1), ds.y / (s.y || 1), ds.z / (s.z || 1)]; } } }
  else if (r.type === 'aim') { r.offsetQ = null; if (maintain) { const q = aimQuat(c); if (q) r.offsetQ = q.invert().multiply(wquat(d)).toArray(); } }
}
function weights(c) { const r = c.inca.rig; const a = c.inca.extraAttrs || {}; return r.targets.map((id, i) => ({ t: node(id), w: Math.max(0, a[r.wAttrs[i]] ?? 1), i })).filter(x => x.t && x.w > 0); }
function weightedPos(c) { const W = weights(c); const sum = W.reduce((s, x) => s + x.w, 0); if (!sum) return null; const p = new THREE.Vector3(); for (const x of W) p.addScaledVector(pivotWorld(x.t), x.w / sum); return p; }
function weightedScale(c) { const W = weights(c); const sum = W.reduce((s, x) => s + x.w, 0); if (!sum) return null; const p = new THREE.Vector3(); for (const x of W) p.addScaledVector(wscale(x.t), x.w / sum); return p; }
function aimQuat(c) {
  const r = c.inca.rig; const d = constrained(c); const A = weightedPos(c); if (!A || !d) return null;
  const P = pivotWorld(d); const dir = A.sub(P); if (dir.lengthSq() < 1e-12) return null;
  let up = V3(r.worldUpVector || [0, 1, 0]);
  const uo = r.worldUpObject && node(r.worldUpObject);
  if (r.worldUpType === 'scene') up = new THREE.Vector3(0, 1, 0);
  else if (r.worldUpType === 'object' && uo) up = pivotWorld(uo).sub(P);
  else if (r.worldUpType === 'objectRotation' && uo) up.applyQuaternion(wquat(uo));
  return frameQuat(dir, up, V3(r.aimVector || [1, 0, 0]), V3(r.upVector || [0, 1, 0]));
}

// ------------------------------------------------------------------ evaluate
function changedTRS(o, before) { const i = o.inca; return ['t', 'r', 's'].some(k => i[k].some((v, j) => Math.abs(v - before[k][j]) > 1e-5)); }
export function evalConstraint(c, ctx) {
  const r = c.inca.rig; const d = constrained(c); if (!d || !r.targets) return;
  const W = weights(c); if (!W.length) return;
  const before = { t: d.inca.t.slice(), r: d.inca.r.slice(), s: d.inca.s.slice() };
  const sum = W.reduce((s, x) => s + x.w, 0);
  if (r.type === 'point') { const p = weightedPos(c); if (p) setWorldPos(d, p.add(V3(r.offset || [0, 0, 0]))); }
  else if (r.type === 'orient') { const qs = W.map(x => { const q = wquat(x.t); const off = r.offsets?.[x.i]; return off ? q.multiply(new THREE.Quaternion().fromArray(off)) : q; }); setWorldQuat(d, blendQuats(qs, W.map(x => x.w / sum))); }
  else if (r.type === 'scale') { const s = weightedScale(c); if (s) { const o = r.offset || [1, 1, 1]; const ps = d.parent && d.parent.inca ? wscale(d.parent) : new THREE.Vector3(1, 1, 1); d.inca.s = [s.x * o[0] / (ps.x || 1), s.y * o[1] / (ps.y || 1), s.z * o[2] / (ps.z || 1)].map(v => +v.toFixed(6)); S.updateXform(d); } }
  else if (r.type === 'aim') { const q = aimQuat(c); if (q) { if (r.offsetQ) q.multiply(new THREE.Quaternion().fromArray(r.offsetQ)); setWorldQuat(d, q); } }
  else if (r.type === 'parent') {
    const ms = W.map(x => { const m = x.t.matrixWorld.clone(); x.t.updateWorldMatrix(true, false); m.copy(x.t.matrixWorld); const off = r.offsets?.[x.i]; if (off) m.multiply(new THREE.Matrix4().fromArray(off)); else { const pw = S.worldPivot(x.t); const q = wquat(x.t); m.compose(pw, q, wscale(d)); } return m; });
    const pos = new THREE.Vector3(); const qs = []; const p = new THREE.Vector3(), q = new THREE.Quaternion(), s = new THREE.Vector3();
    ms.forEach((m, i) => { m.decompose(p, q, s); pos.addScaledVector(p, W[i].w / sum); qs.push(q.clone()); });
    const qb = blendQuats(qs, W.map(x => x.w / sum));
    if (ms.length === 1 && r.offsets?.[W[0].i]) S.setWorldMatrix(d, ms[0]);
    else { setWorldQuat(d, qb); setWorldPos(d, pos); }
  } else if (r.type === 'poleVector') {
    const p = weightedPos(c); const st = node(d.inca.rig?.start); if (p && st && d.inca.extraAttrs) {
      const v = p.sub(wpos(st)); const ea = d.inca.extraAttrs; const old = [ea.poleVectorX, ea.poleVectorY, ea.poleVectorZ];
      ea.poleVectorX = +v.x.toFixed(6); ea.poleVectorY = +v.y.toFixed(6); ea.poleVectorZ = +v.z.toFixed(6);
      if (Math.abs(old[0] - ea.poleVectorX) + Math.abs(old[1] - ea.poleVectorY) + Math.abs(old[2] - ea.poleVectorZ) > 1e-5) ctx.changed = true;
    }
    return;
  }
  if (changedTRS(d, before)) ctx.changed = true;
}
function evalAll(ctx, pole) { for (const c of ctx.by.constraint || []) if ((c.inca.rig.type === 'poleVector') === pole) evalConstraint(c, ctx); }

// ------------------------------------------------------------------ set driven keys (stored on the driven node)
export function setDrivenKey(driven, attr, driver, driverAttr, { dv = null, v = null } = {}) {
  const rig = driven.inca.rig = driven.inca.rig || {};
  const list = rig.sdk = rig.sdk || [];
  let e = list.find(x => x.attr === attr);
  if (!e) { e = { attr, driver: driver.inca.id, driverAttr, keys: [], pre: 'constant', post: 'constant' }; list.push(e); }
  e.driver = driver.inca.id; e.driverAttr = driverAttr;
  const c = curveOf(e); c.setKey(+(dv ?? S.getAttr(driver, driverAttr)), +(v ?? S.getAttr(driven, attr)));
  e.keys = c.keys.map(k => ({ t: k.t, v: k.v, it: k.it, ot: k.ot, is: k.is || 0, os: k.os || 0 }));
  return e;
}
function curveOf(e) { const c = new AnimCurve(null, e.attr); c.keys = e.keys.map(k => ({ ...k })); c.pre = e.pre || 'constant'; c.post = e.post || 'constant'; return c; }
function evalSdk(ctx) {
  App.world.traverse(n => {
    const list = n.inca?.rig?.sdk; if (!list) return;
    for (const e of list) {
      const drv = node(e.driver); if (!drv || !e.keys.length) continue;
      const x = +S.getAttr(drv, e.driverAttr); if (isNaN(x)) continue;
      const v = curveOf(e).eval(x); const cur = S.getAttr(n, e.attr);
      if (typeof cur === 'number' && Math.abs(cur - v) > 1e-7) { S.setAttr(n, e.attr, e.attr === 'visibility' ? (v >= 0.5 ? 1 : 0) : v, { silent: true, force: true }); ctx.changed = true; }
    }
  });
}

// ------------------------------------------------------------------ motion paths
export function attachToMotionPath(o, curve, { follow = true, frontAxis = 'x', upAxis = 'y', worldUpVector = [0, 1, 0], inverseFront = false, inverseUp = false, bank = false, start = App.time.start, end = App.time.end } = {}) {
  const rig = o.inca.rig = o.inca.rig || {};
  rig.motionPath = { name: rigName2('motionPath'), curve: curve.inca.id, follow, frontAxis, upAxis, worldUpVector, inverseFront, inverseUp, fractionMode: true };
  o.inca.extraAttrs = o.inca.extraAttrs || {}; o.inca.extraAttrs.uValue = 0;
  App.anim.setKey(o, 'uValue', { t: start, v: 0 }); App.anim.setKey(o, 'uValue', { t: end, v: 1 });
  App.anim.evaluate(App.time.current);
  return rig.motionPath;
}
function rigName2(b) { let i = 1; const used = new Set(); App.world.traverse(n => { const m = n.inca?.rig?.motionPath; if (m) used.add(m.name); }); while (used.has(b + i)) i++; return b + i; }
export function curveFrame(curve, u) { // world point + tangent at arc-length fraction u
  const pts = curve.inca.curve.sample(16).map(p => new THREE.Vector3(...p)); curve.updateWorldMatrix(true, false);
  for (const p of pts) p.applyMatrix4(curve.matrixWorld);
  if (pts.length < 2) return null;
  const L = [0]; for (let i = 1; i < pts.length; i++) L.push(L[i - 1] + pts[i].distanceTo(pts[i - 1]));
  const total = L[L.length - 1] || 1; const closed = curve.inca.curve.form !== 'open';
  let s = u * total; if (closed) s = ((s % total) + total) % total; else s = Math.max(0, Math.min(total, s));
  let i = 1; while (i < L.length - 1 && L[i] < s) i++;
  const f = (s - L[i - 1]) / ((L[i] - L[i - 1]) || 1);
  return { p: pts[i - 1].clone().lerp(pts[i], f), t: pts[i].clone().sub(pts[i - 1]).normalize() };
}
function evalMotionPaths(ctx) {
  App.world.traverse(n => {
    const mp = n.inca?.rig?.motionPath; if (!mp) return;
    const c = node(mp.curve); if (!c || c.inca.kind !== 'curve') return;
    const u = n.inca.extraAttrs?.uValue ?? 0; const fr = curveFrame(c, u); if (!fr) return;
    const before = { t: n.inca.t.slice(), r: n.inca.r.slice(), s: n.inca.s.slice() };
    setWorldPos(n, fr.p);
    if (mp.follow) {
      const fa = V3(AXIS[mp.frontAxis] || AXIS.x).multiplyScalar(mp.inverseFront ? -1 : 1), ua = V3(AXIS[mp.upAxis] || AXIS.y).multiplyScalar(mp.inverseUp ? -1 : 1);
      setWorldQuat(n, frameQuat(fr.t, V3(mp.worldUpVector || [0, 1, 0]), fa, ua));
    }
    if (changedTRS(n, before)) ctx.changed = true;
  });
}

export function removeConstraints(objs) {
  const kill = new Set();
  for (const o of objs) { if (o.inca.kind === 'constraint') kill.add(o); for (const c of S.dagChildren(o)) if (c.inca.kind === 'constraint') kill.add(c); }
  for (const c of kill) S.deleteNode(c);
  return kill.size;
}

export function install() {
  registerKind('constraint', { create: (nm, opts) => constraintNode(nm, opts), show: 'constraints' });
  Rig.addPass(10, evalSdk);
  Rig.addPass(20, evalMotionPaths);
  Rig.addPass(30, (ctx) => evalAll(ctx, false));
  Rig.addPass(40, (ctx) => evalAll(ctx, true));
  Rig.addPass(60, (ctx) => { if ((ctx.by.ikHandle || []).length) evalAll(ctx, false); });
}
