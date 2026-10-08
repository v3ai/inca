// Inca — shared manipulator state: a proxy object that TransformControls drive in every viewport.
import * as THREE from 'three';
import { App } from './app.js';
import { worldPivot, worldMatrix, setWorldMatrix, parentWorld, setMeshPositions, commitTweak, setPivotWorld, updateXform, rebuildShape, worldBBox, localMatrix } from './scene.js';
import { Sel } from './selection.js';
import { Undo } from './undo.js';

export const Manip = {
  proxy: new THREE.Object3D(),
  mode: 'translate', // translate | rotate | scale | none
  pivotMode: false,
  space: { translate: 'world', rotate: 'local', scale: 'local' },
  soft: { on: false, radius: 2, falloff: 'smooth' },
  start: null,
  dragging: false,
  snapGrid: false, snapPoint: false, snapCurve: false, stepRotate: false,
  _keyGrid: false, _keyPoint: false,
};
Manip.proxy.name = 'manipProxy';
App.manip = Manip;

const topSelected = () => App.sel.filter(o => { let p = o.parent; while (p && p.inca) { if (App.sel.includes(p)) return false; p = p.parent; } return true; }).filter(o => !o.inca.startup);

// position/orient the proxy at the current selection
export function placeProxy() {
  const p = Manip.proxy; p.scale.set(1, 1, 1); p.quaternion.identity();
  const lead = Sel.lead();
  if (App.compMode && App.hilite.length) {
    const c = new THREE.Vector3(); let n = 0; const v = new THREE.Vector3();
    for (const o of App.hilite) {
      const verts = Sel.affectedVerts(o); if (!verts.length) continue;
      o.updateWorldMatrix(true, false);
      const pts = o.inca.kind === 'curve' ? o.inca.curve.cvs : o.inca.mesh.v;
      for (const i of verts) { const q = pts[i]; if (!q) continue; v.set(q[0], q[1], q[2]).applyMatrix4(o.matrixWorld); c.add(v); n++; }
      if (Manip.space.translate === 'local' || Manip.mode !== 'translate') o.getWorldQuaternion(p.quaternion);
    }
    if (!n) return false;
    p.position.copy(c.multiplyScalar(1 / n));
    if (Manip.mode === 'translate' && Manip.space.translate === 'world') p.quaternion.identity();
    p.updateMatrixWorld(true);
    return true;
  }
  if (!lead) return false;
  p.position.copy(worldPivot(lead));
  const sp = Manip.pivotMode ? 'world' : Manip.space[Manip.mode] || 'world';
  if (sp === 'local' || Manip.mode === 'scale') lead.getWorldQuaternion(p.quaternion);
  p.updateMatrixWorld(true);
  return true;
}

export function manipVisible() {
  if (Manip.mode === 'none' || App.tool === 'select' || App.tool === 'lasso' || App.tool === 'paint' || App.tool === 'cvCurve' || App.tool === 'epCurve' || App.tool === 'edgeLoop' || App.tool === 'multiCut' || App.tool === 'targetWeld' || App.tool === 'createPoly') return false;
  if (App.compMode) return App.hilite.some(o => Sel.affectedVerts(o).length);
  return topSelected().length > 0;
}

export function beginDrag() {
  Manip.dragging = true;
  Undo.checkpoint(Manip.pivotMode ? 'pivot' : Manip.mode);
  const st = { pos: Manip.proxy.position.clone(), quat: Manip.proxy.quaternion.clone(), scale: Manip.proxy.scale.clone(), objs: [], comps: [] };
  if (Manip.pivotMode) { st.pivotObj = Sel.lead(); }
  else if (App.compMode) {
    for (const o of App.hilite) {
      let verts = Sel.affectedVerts(o); if (!verts.length && !Manip.soft.on) continue;
      o.updateWorldMatrix(true, false);
      const pts = o.inca.kind === 'curve' ? o.inca.curve.cvs : o.inca.mesh.v;
      let weights = null;
      if (Manip.soft.on && o.inca.kind === 'mesh') {
        const w = softWeights(o, verts); verts = [...w.keys()]; weights = verts.map(i => w.get(i));
      }
      const world = verts.map(i => new THREE.Vector3(...pts[i]).applyMatrix4(o.matrixWorld));
      st.comps.push({ o, verts, world, orig: verts.map(i => pts[i].slice()), inv: o.matrixWorld.clone().invert(), weights });
    }
  } else {
    for (const o of topSelected()) st.objs.push({ o, wm: worldMatrix(o), piv: worldPivot(o), t: o.inca.t.slice(), r: o.inca.r.slice(), s: o.inca.s.slice(), pinv: parentWorld(o).invert() });
  }
  Manip.start = st;
}

const _v = new THREE.Vector3(), _q = new THREE.Quaternion(), _m = new THREE.Matrix4(), _m2 = new THREE.Matrix4();
export function applyDrag() {
  const st = Manip.start; if (!st) return;
  const p = Manip.proxy;
  if (Manip.pivotMode) { if (st.pivotObj) { setPivotWorld(st.pivotObj, p.position); } App.dirty('channels'); return; }
  const dpos = p.position.clone().sub(st.pos);
  const dq = p.quaternion.clone().multiply(st.quat.clone().invert());
  const sc = new THREE.Vector3(p.scale.x / st.scale.x, p.scale.y / st.scale.y, p.scale.z / st.scale.z);
  if (App.compMode) {
    const qInv = st.quat.clone().invert();
    for (const c of st.comps) {
      const out = new Float32Array(c.verts.length * 3);
      c.world.forEach((w, i) => {
        const wt = c.weights ? c.weights[i] : 1;
        let nw;
        if (Manip.mode === 'translate') nw = w.clone().addScaledVector(dpos, wt);
        else if (Manip.mode === 'rotate') { const q = new THREE.Quaternion().slerpQuaternions(new THREE.Quaternion(), dq, wt); nw = w.clone().sub(st.pos).applyQuaternion(q).add(st.pos); }
        else { const l = w.clone().sub(st.pos).applyQuaternion(qInv); const f = new THREE.Vector3(1 + (sc.x - 1) * wt, 1 + (sc.y - 1) * wt, 1 + (sc.z - 1) * wt); l.multiply(f).applyQuaternion(st.quat); nw = l.add(st.pos); }
        nw.applyMatrix4(c.inv); out[i * 3] = nw.x; out[i * 3 + 1] = nw.y; out[i * 3 + 2] = nw.z;
      });
      if (c.o.inca.kind === 'curve') { c.verts.forEach((vi, i) => { c.o.inca.curve.cvs[vi] = [out[i * 3], out[i * 3 + 1], out[i * 3 + 2]]; }); rebuildShape(c.o); }
      else setMeshPositions(c.o, c.verts, out);
      c.o.userData.compDirty = true;
    }
    App.requestRender();
    return;
  }
  for (const s of st.objs) {
    const o = s.o;
    if (Manip.mode === 'translate') {
      // transform the world delta vector by the parent inverse (linear part only)
      const pl = s.pinv.clone().setPosition(0, 0, 0);
      const d = dpos.clone().applyMatrix4(pl);
      o.inca.t = [s.t[0] + d.x, s.t[1] + d.y, s.t[2] + d.z].map(x => +x.toFixed(6));
      updateXform(o);
    } else if (Manip.mode === 'rotate') {
      _m.makeTranslation(s.piv.x, s.piv.y, s.piv.z).multiply(_m2.makeRotationFromQuaternion(dq)).multiply(new THREE.Matrix4().makeTranslation(-s.piv.x, -s.piv.y, -s.piv.z)).multiply(s.wm);
      o.inca.r = s.r.slice();
      setWorldMatrix(o, _m.clone());
    } else if (Manip.mode === 'scale') {
      o.inca.s = [s.s[0] * sc.x, s.s[1] * sc.y, s.s[2] * sc.z].map(x => +x.toFixed(6));
      updateXform(o);
    }
  }
  App.dirty('channels');
}
export function endDrag() {
  const st = Manip.start; Manip.dragging = false; Manip.start = null;
  if (!st) return;
  const p = Manip.proxy;
  if (App.compMode) {
    for (const c of st.comps) {
      if (c.o.inca.kind === 'curve') continue;
      const pm = c.o.inca.mesh; const deltas = [];
      c.verts.forEach((vi, i) => { const a = c.orig[i], b = pm.v[vi]; deltas.push(b[0] - a[0], b[1] - a[1], b[2] - a[2]); });
      commitTweak(c.o, c.verts, deltas);
    }
    const d = p.position.clone().sub(st.pos);
    if (Manip.mode === 'translate') App.emit('echo', `move -r ${fx(d.x)} ${fx(d.y)} ${fx(d.z)} ;`);
  } else if (!Manip.pivotMode) {
    const attrs = Manip.mode === 'translate' ? ['translateX', 'translateY', 'translateZ'] : Manip.mode === 'rotate' ? ['rotateX', 'rotateY', 'rotateZ'] : ['scaleX', 'scaleY', 'scaleZ'];
    for (const s of st.objs) App.anim.autoKey(s.o, attrs);
    const lead = st.objs[st.objs.length - 1];
    if (lead) {
      const o = lead.o;
      if (Manip.mode === 'translate') { const d = p.position.clone().sub(st.pos); App.emit('echo', `move -r ${fx(d.x)} ${fx(d.y)} ${fx(d.z)} ;`); }
      else if (Manip.mode === 'rotate') App.emit('echo', `rotate -r -os -fo ${fx(o.inca.r[0] - lead.r[0])} ${fx(o.inca.r[1] - lead.r[1])} ${fx(o.inca.r[2] - lead.r[2])} ;`);
      else App.emit('echo', `scale -r ${fx(o.inca.s[0] / (lead.s[0] || 1))} ${fx(o.inca.s[1] / (lead.s[1] || 1))} ${fx(o.inca.s[2] / (lead.s[2] || 1))} ;`);
    }
  } else App.emit('echo', `move -a -rpr ${fx(p.position.x)} ${fx(p.position.y)} ${fx(p.position.z)} ;`);
  App.dirty('channels', 'attr', 'timeline');
}
const fx = (n) => +n.toFixed(4);

export function softWeights(o, verts) {
  const pm = o.inca.mesh; const R = Manip.soft.radius;
  const sel = new Set(verts); const w = new Map();
  o.updateWorldMatrix(true, false);
  const P = pm.v.map(p => new THREE.Vector3(...p).applyMatrix4(o.matrixWorld));
  const sp = verts.map(i => P[i]);
  for (let i = 0; i < P.length; i++) {
    if (sel.has(i)) { w.set(i, 1); continue; }
    let d = Infinity; for (const s of sp) { const dd = P[i].distanceTo(s); if (dd < d) d = dd; }
    if (d < R) { const t = 1 - d / R; w.set(i, Manip.soft.falloff === 'linear' ? t : t * t * (3 - 2 * t)); }
  }
  return w;
}
