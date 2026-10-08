// Inca — rigging UI: commands, Joint / IK Handle / Lattice Point tools, Set Driven Key window, ghosting & motion trails.
import * as THREE from 'three';
import { App } from '../core/app.js';
import * as S from '../core/scene.js';
import { Sel } from '../core/selection.js';
import { Undo } from '../core/undo.js';
import { Rig, createJoint, createJointAt, orientHierarchy, createIkHandle, smoothBind, wpos, node, setWorldPos } from '../core/rigging.js';
import { NONLINEAR, createNonlinear, createCluster, createSoftMod, createLattice, createBlendShape, deleteDeformersOn, meshesOf, latIndex } from '../core/deformers.js';
import { createConstraint, setDrivenKey, attachToMotionPath, removeConstraints } from '../core/constraints.js';
import { h, FloatWin, toast } from './dom.js';

const ck = (l) => Undo.checkpoint(l);
const echo = (s) => App.emit('echo', s);
const warn = (s) => App.emit('warning', '// Warning: ' + s);
const ptsMat = new THREE.PointsMaterial({ size: 8, sizeAttenuation: false, color: 0x5cff5c, depthTest: false });
const lnMat = new THREE.LineBasicMaterial({ color: 0x5c8cff, depthTest: false });
function overlay(objs) { for (const vp of App.viewports) { for (const c of [...vp.toolScene.children]) { vp.toolScene.remove(c); c.geometry?.dispose(); } for (const o of objs || []) vp.toolScene.add(o.clone()); vp.needsRender = true; } }

export function install() {
  if (!App.tools) { App.on('beforeUI', install); return; }
  const C = App.cmds.register;
  const optsOf = (id) => App.prefs?.opt?.[id] || {};
  // ---------------------------------------------------------------- joints
  App.tools.register('jointTool', {
    help: 'Joint Tool: click to place joints (each new joint is parented to the previous one). Enter to complete, Backspace removes the last joint. Shift: snap to a straight line',
    start() { this.made = []; this.parent = App.sel.find(o => o.inca.kind === 'joint') || null; },
    down(vp, e, px) {
      let p = vp.groundPoint(px); if (!p) return true;
      if (!this.made.length) ck('joint tool');
      const prev = this.made[this.made.length - 1] || this.parent;
      if (e.shiftKey && prev) { const q = wpos(prev); const d = p.clone().sub(q); const ax = ['x', 'y', 'z'].reduce((a, b) => Math.abs(d[a]) > Math.abs(d[b]) ? a : b); for (const k of ['x', 'y', 'z']) if (k !== ax) p[k] = q[k]; }
      const j = createJointAt(p, prev, 'joint1');
      this.made.push(j); Sel.select([j], 'replace', { echo: false });
      echo(`joint -p ${p.toArray().map(x => +x.toFixed(3)).join(' ')} ;`);
      return true;
    },
    back() { const j = this.made.pop(); if (j) { S.deleteNode(j); App.dirty('outliner'); App.requestRender(); } },
    canBack() { return this.made?.length > 0; },
    complete() { if (this.made.length) { if (optsOf('jointTool').orient !== false) orientHierarchy([this.made[0]]); Sel.select([this.made[0]], 'replace', { echo: false }); App.emit('result', this.made[0].inca.name); } this.made = []; this.parent = null; App.dirty('outliner', 'channels'); },
    end() { if (this.made?.length) this.complete(); },
    settings: [{ key: 'orient', label: 'Orient joint to child (X)', type: 'bool', get: () => optsOf('jointTool').orient !== false, set: (v) => { (App.prefs.opt.jointTool ||= {}).orient = v; App.savePrefs(); } }],
  }, 'joint');
  C('jointTool', 'Create Joints', () => App.setTool('jointTool'), { icon: 'joint', noRepeat: true, help: 'Click in a view to place joints; Enter completes the chain' });
  C('insertJoint', 'Insert Joint', () => { const j = Sel.lead(); if (!j || j.inca.kind !== 'joint') return warn('Select a joint'); const kid = S.dagChildren(j).find(c => c.inca.kind === 'joint'); if (!kid) return warn('The joint has no child joint'); ck('insert joint'); const mid = wpos(j).lerp(wpos(kid), 0.5); const n = createJointAt(mid, j); S.parentTo(kid, n, true); orientHierarchy([j]); Sel.select([n]); echo('insertJoint ' + j.inca.name + ';'); });
  C('mirrorJoint', 'Mirror Joints', (a) => {
    const root = Sel.lead(); if (!root || root.inca.kind !== 'joint') return warn('Select the root joint to mirror'); ck('mirror joint');
    const axis = { axis: 0, search: 'left', replace: 'right', ...optsOf('mirrorJoint'), ...a };
    const map = new Map();
    root.traverse(c => { if (!c.inca || c.inca.kind !== 'joint') return; const p = wpos(c); p.setComponent(axis.axis, -p.getComponent(axis.axis)); const par = map.get(S.dagParent(c)) || S.dagParent(root === c ? root : null); const n = createJointAt(p, c === root ? S.dagParent(root) : map.get(S.dagParent(c)) || null, c.inca.name.replace(axis.search, axis.replace) + (c.inca.name.includes(axis.search) ? '' : '_mirror')); n.inca.rig.radius = c.inca.rig.radius; map.set(c, n); });
    orientHierarchy([map.get(root)]); Sel.select([map.get(root)]); echo('mirrorJoint -mirrorYZ -mirrorBehavior;');
  }, { icon: 'joint', options: [{ key: 'axis', label: 'Mirror across', type: 'radio', options: ['YZ', 'XZ', 'XY'], values: [0, 1, 2], value: 0 }, { key: 'search', label: 'Search for', type: 'text', value: 'left' }, { key: 'replace', label: 'Replace with', type: 'text', value: 'right' }] });
  C('orientJoint', 'Orient Joint', () => { const js = App.sel.filter(o => o.inca.kind === 'joint'); if (!js.length) return warn('Select joints'); ck('orient joint'); orientHierarchy(js); echo('joint -e -oj xyz -secondaryAxisOrient yup -ch -zso;'); });
  // ---------------------------------------------------------------- IK
  App.tools.register('ikHandleTool', {
    help: 'IK Handle Tool: click the start joint, then the end joint',
    start() { this.startJ = null; },
    down(vp, e, px) {
      const hit = vp.pickObjects(px).find(x => x.o.inca.kind === 'joint'); if (!hit) { App.help('Click on a joint'); return true; }
      if (!this.startJ) { this.startJ = hit.o; Sel.select([hit.o], 'replace', { echo: false }); App.help('IK Handle Tool: now click the end joint'); return true; }
      try { ck('ik handle'); const ikh = createIkHandle(this.startJ, hit.o, { solver: optsOf('ikHandleTool').solver || 'ikRPsolver' }); Sel.select([ikh]); echo(`ikHandle -sol ${ikh.inca.rig.solver} -sj ${this.startJ.inca.name} -ee ${hit.o.inca.name};`); App.emit('result', ikh.inca.name); }
      catch (err) { warn(err.message); }
      this.startJ = null; App.setTool('move'); return true;
    },
    settings: [{ key: 'solver', label: 'Current solver', type: 'enum', options: ['Rotate-Plane Solver', 'Single-Chain Solver'], values: ['ikRPsolver', 'ikSCsolver'], get: () => optsOf('ikHandleTool').solver || 'ikRPsolver', set: (v) => { (App.prefs.opt.ikHandleTool ||= {}).solver = v; App.savePrefs(); } }],
  }, 'joint');
  C('ikHandleTool', 'Create IK Handle', () => {
    const js = App.sel.filter(o => o.inca.kind === 'joint');
    if (js.length >= 2) { try { ck('ik handle'); const ikh = createIkHandle(js[0], js[js.length - 1]); Sel.select([ikh]); echo(`ikHandle -sj ${js[0].inca.name} -ee ${js[js.length - 1].inca.name};`); } catch (e) { warn(e.message); } return; }
    App.setTool('ikHandleTool');
  }, { icon: 'joint', noRepeat: true });
  // ---------------------------------------------------------------- skin
  C('smoothBind', 'Bind Skin', (a) => {
    const joints = []; for (const o of App.sel) if (o.inca.kind === 'joint') o.traverse(c => { if (c.inca?.kind === 'joint' && !joints.includes(c)) joints.push(c); });
    const meshes = meshesOf(App.sel.filter(o => o.inca.kind !== 'joint'));
    if (!joints.length || !meshes.length) return warn('Select a joint hierarchy and one or more meshes');
    ck('bind skin'); const o0 = { maxInfluences: 4, dropoffRate: 4, ...optsOf('smoothBind'), ...a };
    for (const m of meshes) smoothBind(joints, m, o0);
    Rig.evaluate(); echo(`skinCluster -toSelectedBones -bindMethod 0 -skinMethod 0 -normalizeWeights 1 -weightDistribution 0 -mi ${o0.maxInfluences} -dr ${o0.dropoffRate};`);
  }, { icon: 'joint', options: [{ key: 'maxInfluences', label: 'Max influences', type: 'int', value: 4 }, { key: 'dropoffRate', label: 'Dropoff rate', type: 'float', value: 4 }] });
  C('unbindSkin', 'Unbind Skin', () => { const ms = meshesOf(App.sel); if (!ms.length) return warn('Select skinned meshes'); ck('unbind skin'); for (const m of ms) { if (m.inca.rig) delete m.inca.rig.skin; S.rebuildShape(m); } echo('doDetachSkin "2" { "1","1" };'); });
  C('paintWeights', 'Paint Skin Weights', () => App.setTool('paintWeights'), { icon: 'softSelect', noRepeat: true });
  App.tools.register('paintWeights', {
    help: 'Paint Skin Weights: pick the influence joint in Tool Settings, then drag on the skinned mesh. Ctrl subtracts, Shift smooths',
    settings: [
      { key: 'joint', label: 'Influence', type: 'enum', get: () => 0, set: () => {}, options: ['(select a skinned mesh)'] },
      { key: 'value', label: 'Value', type: 'float', min: 0, max: 1, smin: 0, smax: 1, get: () => optsOf('paintWeights').value ?? 0.3, set: (v) => { (App.prefs.opt.paintWeights ||= {}).value = v; } },
      { key: 'radius', label: 'Radius (world)', type: 'float', min: 0.01, smin: 0.05, smax: 5, get: () => optsOf('paintWeights').radius ?? 0.6, set: (v) => { (App.prefs.opt.paintWeights ||= {}).radius = v; } },
    ],
    start() {
      const m = meshesOf(App.sel).find(x => x.inca.rig?.skin); this.mesh = m || null;
      const s = this.settings[0]; if (m) { s.options = m.inca.rig.skin.joints.map(id => node(id)?.inca.name || id); s.values = s.options.map((_, i) => i); s.get = () => this.ji || 0; s.set = (v) => { this.ji = v; this.showWeights(); }; }
      this.ji = 0; this.showWeights(); App.dirty('toolSettings');
    },
    showWeights() {
      const m = this.mesh; if (!m) return; const sk = m.inca.rig.skin; const mesh = m.userData.mesh; const g = mesh.geometry;
      // colour the displayed mesh by the weight of the chosen influence (black→white like Maya)
      const tri = mesh.userData.triFace; const pm = m.userData.deformed || m.inca.mesh; const col = [];
      const wOf = (v) => { const W = sk.w[v] || []; for (let k = 0; k < W.length; k += 2) if (W[k] === this.ji) return W[k + 1]; return 0; };
      for (let f = 0; f < pm.f.length; f++) { } // geometry is per-corner; rebuild colours from face list
      const pos = g.attributes.position; const map = []; const vpos = pm.v;
      for (let i = 0; i < pos.count; i++) { let best = 0, bd = 1e9; const x = pos.getX(i), y = pos.getY(i), z = pos.getZ(i); const fi = tri ? tri[Math.floor(i / 3)] : -1; const cand = fi >= 0 ? pm.f[fi] : vpos.map((_, k) => k); for (const v of cand) { const p = vpos[v]; const d = (p[0] - x) ** 2 + (p[1] - y) ** 2 + (p[2] - z) ** 2; if (d < bd) { bd = d; best = v; } } map.push(best); const w = wOf(best); col.push(w, w, w); }
      g.setAttribute('color', new THREE.Float32BufferAttribute(col, 3)); this._map = map;
      if (!this._mat) this._mat = new THREE.MeshBasicMaterial({ vertexColors: true });
      mesh.material = this._mat; mesh.userData.matsT = mesh.userData.matsU = [this._mat]; App.requestRender();
    },
    down(vp, e, px) {
      if (!this.mesh) { warn('Select a skinned mesh before painting weights'); return true; }
      ck('paint weights'); const m = this.mesh; const sk = m.inca.rig.skin;
      const paint = (q) => {
        const rc = vp.raycaster(q); const hit = vp.rayMesh(rc, m); if (!hit) return;
        m.updateWorldMatrix(true, false); const inv = m.matrixWorld.clone().invert(); const c = hit.point.clone().applyMatrix4(inv);
        const R = optsOf('paintWeights').radius ?? 0.6, V = optsOf('paintWeights').value ?? 0.3; const pm = m.userData.deformed || m.inca.mesh;
        pm.v.forEach((p, i) => {
          const d = Math.hypot(p[0] - c.x, p[1] - c.y, p[2] - c.z); if (d > R) return; const f = (1 - d / R) * V;
          const W = new Map(); const arr = sk.w[i] || []; for (let k = 0; k < arr.length; k += 2) W.set(arr[k], arr[k + 1]);
          const cur = W.get(this.ji) || 0; const nv = Math.max(0, Math.min(1, e.ctrlKey ? cur - f : cur + f));
          const others = [...W.entries()].filter(([j]) => j !== this.ji); const os = others.reduce((s, x) => s + x[1], 0);
          const out = [this.ji, nv]; if (os > 1e-6) for (const [j, w] of others) out.push(j, w / os * (1 - nv)); else if (nv < 1 && others.length === 0 && sk.joints.length > 1) out.push(this.ji === 0 ? 1 : 0, 1 - nv);
          sk.w[i] = out.filter((x, k) => k % 2 === 0 || true);
        });
        sk._v = (sk._v || 0) + 1; S.rebuildShape(m); this.showWeights();
      };
      paint(vp.localPx(e));
      const mm = (ev) => paint(vp.localPx(ev)); const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu); return true;
    },
    end() { if (this.mesh) S.rebuildShape(this.mesh); this.mesh = null; },
  }, 'softSelect');
  // ---------------------------------------------------------------- deformers
  for (const t of Object.keys(NONLINEAR)) C(t + 'Deformer', t[0].toUpperCase() + t.slice(1), () => {
    const ms = meshesOf(App.sel); if (!ms.length) return warn('Select meshes to deform'); ck(t);
    const hnd = createNonlinear(t, ms); Sel.select([hnd]); S.rebuildShape(ms[0]); Rig.evaluate();
    echo(`nonLinear -type ${t};`); App.emit('result', hnd.inca.name);
  }, { icon: { bend: 'polyCylinder', twist: 'polyHelix', flare: 'polyCone', sine: 'polyHelix', squash: 'polySphere', wave: 'polyPlane' }[t] });
  C('createCluster', 'Cluster', () => {
    const entries = App.compMode ? App.hilite.filter(o => o.inca.kind === 'mesh').map(o => ({ mesh: o, verts: Sel.affectedVerts(o) })).filter(e => e.verts.length) : meshesOf(App.sel).map(m => ({ mesh: m, verts: null }));
    if (!entries.length) return warn('Select vertices or meshes'); ck('cluster');
    const c = createCluster(entries); Sel.setMode(null); Sel.select([c]); Rig.evaluate(); echo('cluster;'); App.emit('result', c.inca.name);
  }, { icon: 'locator' });
  C('softModDeformer', 'Soft Modification', () => {
    const ms = App.compMode ? App.hilite.filter(o => o.inca.kind === 'mesh') : meshesOf(App.sel); if (!ms.length) return warn('Select a mesh (or vertices)');
    ck('soft mod'); let center = new THREE.Vector3(); if (App.compMode) { let n = 0; for (const o of ms) for (const v of Sel.affectedVerts(o)) { center.add(new THREE.Vector3(...o.inca.mesh.v[v]).applyMatrix4(o.matrixWorld)); n++; } if (n) center.multiplyScalar(1 / n); } else center = S.worldBBox(ms[0]).getCenter(new THREE.Vector3());
    const s = createSoftMod(ms, center); Sel.setMode(null); Sel.select([s]); Rig.evaluate(); echo('softMod;');
  });
  C('createLattice', 'Lattice', (a) => {
    const ms = meshesOf(App.sel); if (!ms.length) return warn('Select meshes'); ck('lattice'); const o0 = { s: 2, t: 5, u: 2, ...optsOf('createLattice'), ...a };
    const l = createLattice(ms, { divisions: [o0.s, o0.t, o0.u] }); Sel.select([l]); Rig.evaluate(); echo(`lattice -divisions ${o0.s} ${o0.t} ${o0.u} -objectCentered true;`);
    App.help('Lattice created: use the Lattice Point tool (right-click the lattice) to move lattice points');
  }, { icon: 'nurbsCube', options: [{ key: 's', label: 'S divisions', type: 'int', value: 2 }, { key: 't', label: 'T divisions', type: 'int', value: 5 }, { key: 'u', label: 'U divisions', type: 'int', value: 2 }] });
  C('blendShape', 'Blend Shape', () => {
    const ms = App.sel.filter(o => o.inca.kind === 'mesh'); if (ms.length < 2) return warn('Select the target shapes, then the base mesh last');
    ck('blend shape'); const base = ms[ms.length - 1]; const { bs, added } = createBlendShape(base, ms.slice(0, -1)); Sel.select([base]); echo(`blendShape ${ms.map(m => m.inca.name).join(' ')};`); App.emit('result', bs.name); if (added.length) toast(`Blend shape targets ${added.join(', ')} added to ${base.inca.name} (Channel Box)`);
  });
  C('deleteDeformers', 'Delete Deformers on Selected', () => { ck('delete deformers'); const r = deleteDeformersOn(App.sel); Rig.evaluate(); App.emit('result', `${r.removed} deformer handle(s) removed`); App.emit('selectionChanged'); });
  // lattice point editing tool
  App.tools.register('latticePoint', {
    help: 'Lattice Points: click / drag a box to select points, drag a selected point to move it (in the view plane). Q to finish',
    start() { this.lat = App.sel.find(o => o.inca.kind === 'lattice') || null; if (!this.lat) App.help('Select a lattice first'); },
    world(i) { const p = this.lat.inca.rig.pts[i]; this.lat.updateWorldMatrix(true, false); return new THREE.Vector3(...p).applyMatrix4(this.lat.matrixWorld); },
    down(vp, e, px) {
      const L = this.lat; if (!L) return true; const r = L.inca.rig; const n = r.pts.length;
      let near = -1, bd = 10; for (let i = 0; i < n; i++) { const s = vp.project(this.world(i)); const d = Math.hypot(s.x - px.x, s.y - px.y); if (d < bd) { bd = d; near = i; } }
      if (near >= 0 && (r.sel || []).includes(near) && !e.shiftKey && !e.ctrlKey) {
        ck('move lattice points'); const inv = L.matrixWorld.clone().invert(); const start = r.sel.map(i => this.world(i)); const { right, up } = vp.camBasis(); const k = vp.worldPerPixel();
        const mm = (ev) => { const q = vp.localPx(ev); const d = right.clone().multiplyScalar((q.x - px.x) * k).addScaledVector(up, -(q.y - px.y) * k); r.sel.forEach((i, j) => { r.pts[i] = start[j].clone().add(d).applyMatrix4(inv).toArray().map(x => +x.toFixed(5)); }); for (const id of r.targets) { const m = node(id); if (m) S.rebuildShape(m); } App.requestRender(); };
        const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); App.dirty('channels'); };
        addEventListener('mousemove', mm); addEventListener('mouseup', mu); return true;
      }
      const x0 = px.x, y0 = px.y; let box = null;
      const mm = (ev) => { const q = vp.localPx(ev); if (!box && Math.hypot(q.x - x0, q.y - y0) > 4) { box = h('div', { class: 'marquee' }); vp.view.append(box); } if (box) Object.assign(box.style, { left: Math.min(x0, q.x) + 'px', top: Math.min(y0, q.y) + 'px', width: Math.abs(q.x - x0) + 'px', height: Math.abs(q.y - y0) + 'px' }); };
      const mu = (ev) => {
        removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); const q = vp.localPx(ev);
        let ids = [];
        if (box) { box.remove(); const R = { x0: Math.min(x0, q.x), x1: Math.max(x0, q.x), y0: Math.min(y0, q.y), y1: Math.max(y0, q.y) }; for (let i = 0; i < n; i++) { const s = vp.project(this.world(i)); if (s.x >= R.x0 && s.x <= R.x1 && s.y >= R.y0 && s.y <= R.y1) ids.push(i); } }
        else if (near >= 0) ids = [near];
        const cur = new Set(e.shiftKey || e.ctrlKey ? r.sel || [] : []);
        for (const i of ids) { if (e.ctrlKey) cur.delete(i); else if (e.shiftKey && cur.has(i)) cur.delete(i); else cur.add(i); }
        r.sel = [...cur]; App.requestRender();
      };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu); return true;
    },
  }, 'nurbsCube');
  C('latticePointTool', 'Edit Lattice Points', () => App.setTool('latticePoint'), { icon: 'nurbsCube', noRepeat: true });
  // ---------------------------------------------------------------- constraints
  const conOpts = [{ key: 'maintainOffset', label: 'Maintain offset', type: 'bool', value: true }, { key: 'weight', label: 'Weight', type: 'float', value: 1 }];
  for (const t of ['parent', 'point', 'orient', 'scale', 'aim', 'poleVector']) C(t + 'Constraint', t === 'poleVector' ? 'Pole Vector' : t[0].toUpperCase() + t.slice(1), (a) => {
    if (App.sel.length < 2) return warn('Select the target object(s), then the object to constrain last');
    const driven = App.sel[App.sel.length - 1], targets = App.sel.slice(0, -1);
    const o0 = { maintainOffset: t === 'aim' || t === 'poleVector' ? false : (t === 'parent'), ...optsOf(t + 'Constraint'), ...a };
    try { ck(t + ' constraint'); const c = createConstraint(t, targets, driven, o0); Rig.evaluate(); Sel.select([c]); echo(`${t}Constraint ${o0.maintainOffset ? '-mo ' : ''}-weight ${o0.weight ?? 1} ${App.sel.map(o => o.inca.name).join(' ')};`); App.emit('result', c.inca.name); }
    catch (e) { warn(e.message); }
  }, { icon: { parent: 'parent', point: 'move', orient: 'rotate', scale: 'scale', aim: 'camera', poleVector: 'joint' }[t], options: t === 'aim' ? [...conOpts, { key: 'aimVector', label: 'Aim vector', type: 'vec3', value: [1, 0, 0] }, { key: 'upVector', label: 'Up vector', type: 'vec3', value: [0, 1, 0] }, { key: 'worldUpVector', label: 'World up vector', type: 'vec3', value: [0, 1, 0] }] : conOpts });
  C('removeConstraints', 'Remove Constraints', () => { ck('remove constraints'); removeConstraints(App.sel); Rig.evaluate(); App.emit('selectionChanged'); echo('delete -constraints;'); });
  // ---------------------------------------------------------------- motion path
  C('attachToMotionPath', 'Attach to Motion Path', (a) => {
    const curve = App.sel.find(o => o.inca.kind === 'curve'); const objs = App.sel.filter(o => o !== curve);
    if (!curve || !objs.length) return warn('Select the object(s) to animate, then the path curve');
    ck('motion path'); const o0 = { follow: true, frontAxis: 'x', upAxis: 'y', ...optsOf('attachToMotionPath'), ...a };
    for (const o of objs) attachToMotionPath(o, curve, o0);
    App.setTime(App.time.current); Sel.select(objs); echo(`pathAnimation -fractionMode true -follow ${o0.follow} -followAxis ${o0.frontAxis} -upAxis ${o0.upAxis} -worldUpType "vector" -startTimeU ${App.time.start} -endTimeU ${App.time.end} -c ${curve.inca.name};`);
  }, { icon: 'epCurve', options: [{ key: 'follow', label: 'Follow', type: 'bool', value: true }, { key: 'frontAxis', label: 'Front axis', type: 'radio', options: ['X', 'Y', 'Z'], values: ['x', 'y', 'z'], value: 'x' }, { key: 'upAxis', label: 'Up axis', type: 'radio', options: ['X', 'Y', 'Z'], values: ['x', 'y', 'z'], value: 'y' }, { key: 'bank', label: 'Bank', type: 'bool', value: false }] });
  // ---------------------------------------------------------------- set driven key
  C('setDrivenKeyWindow', 'Set Driven Key...', () => sdkWindow(), { icon: 'setKey', noRepeat: true });
  // ---------------------------------------------------------------- ghosting / motion trails
  C('createGhost', 'Ghost Selected', (a) => ghost(App.sel.filter(o => o.inca.kind === 'mesh' || o.inca.kind === 'joint'), { steps: 3, ...optsOf('createGhost'), ...a }), { options: [{ key: 'steps', label: 'Steps before/after', type: 'int', value: 3 }, { key: 'every', label: 'Frame step (0 = keys)', type: 'int', value: 0 }] });
  C('unghostAll', 'Unghost All', () => { clearGhosts(); App.requestRender(); });
  C('motionTrail', 'Create Motion Trail', () => motionTrail(App.sel));
  // marking-menu entry for lattices: RMB on a selected lattice offers Lattice Point
  App.on('selectionChanged', () => { const l = Sel.lead(); if (l && l.inca.kind === 'lattice') App.help('Lattice selected: run "Edit Lattice Points" (Deform menu / RMB) to move its points'); });
  // Deform menu gets the lattice point tool too
  const deform = App.ui.menuSets?.modeling?.Deform; void deform;
}

// ------------------------------------------------------------------ Set Driven Key window (Maya layout)
function sdkWindow() {
  const win = new FloatWin('sdk', 'Set Driven Key', { w: 640, h: 460 }); if (win.reused) return;
  const st = { driver: null, driverAttr: null, driven: [], drivenAttr: null };
  const attrsOf = (o) => { const l = [...App.anim.keyableAttrs(o)]; for (const k of Object.keys(o.inca.extraAttrs || {})) if (!l.includes(k)) l.push(k); return l; };
  const list = (rows, sel, onPick) => { const box = h('div', { class: 'list-box', style: { flex: '1', minHeight: '0' } }); for (const r of rows) { const el = h('div', { class: 'li' + (r === sel ? ' sel' : ''), text: r }); el.addEventListener('click', () => onPick(r)); box.append(el); } return box; };
  const body = h('div', { style: { flex: '1', display: 'flex', flexDirection: 'column', padding: '6px', gap: '6px', minHeight: '0' } });
  win.body.append(body, h('div', { class: 'dlg-buttons' },
    h('button', { text: 'Key', onclick: () => {
      if (!st.driver || !st.driverAttr || !st.driven.length || !st.drivenAttr) return toast('Load a driver and driven object and pick both attributes');
      ck('set driven key'); for (const d of st.driven) setDrivenKey(d, st.drivenAttr, st.driver, st.driverAttr);
      Rig.evaluate(); echo(`setDrivenKeyframe -cd ${st.driver.inca.name}.${st.driverAttr} ${st.driven.map(d => d.inca.name + '.' + st.drivenAttr).join(' ')};`); toast(`Driven key: ${st.driverAttr} = ${+(+S.getAttr(st.driver, st.driverAttr)).toFixed(3)} → ${st.drivenAttr}`);
    } }),
    h('button', { text: 'Load Driver', onclick: () => { st.driver = App.sel[App.sel.length - 1] || null; st.driverAttr = null; draw(); } }),
    h('button', { text: 'Load Driven', onclick: () => { st.driven = App.sel.slice(); st.drivenAttr = null; draw(); } }),
    h('button', { text: 'Close', onclick: () => win.close() })));
  function draw() {
    body.innerHTML = '';
    const sect = (title, objs, attrs, selAttr, pick) => h('div', { style: { flex: '1', display: 'flex', flexDirection: 'column', minHeight: '0' } }, h('div', { class: 'dlg-sect', text: title }), h('div', { style: { display: 'flex', gap: '6px', flex: '1', minHeight: '0' } }, list(objs, objs[0], () => {}), list(attrs, selAttr, pick)));
    body.append(sect('Driver', st.driver ? [st.driver.inca.name] : [], st.driver ? attrsOf(st.driver) : [], st.driverAttr, (a) => { st.driverAttr = a; draw(); }),
      sect('Driven', st.driven.map(d => d.inca.name), st.driven[0] ? attrsOf(st.driven[0]) : [], st.drivenAttr, (a) => { st.drivenAttr = a; draw(); }));
  }
  draw();
}

// ------------------------------------------------------------------ ghosts & trails
let ghosts = [];
function clearGhosts() { for (const g of ghosts) { g.parent?.remove(g); g.traverse(c => { c.geometry?.dispose(); }); } ghosts = []; }
function ghost(objs, { steps = 3, every = 0 } = {}) {
  if (!objs.length) return warn('Select animated objects to ghost');
  clearGhosts(); const t0 = App.time.current;
  const times = new Set();
  if (every > 0) for (let i = 1; i <= steps; i++) { times.add(t0 - i * every); times.add(t0 + i * every); }
  else { const ks = App.anim.keyTimes(objs); const before = ks.filter(t => t < t0).slice(-steps), after = ks.filter(t => t > t0).slice(0, steps); for (const t of [...before, ...after]) times.add(t); }
  for (const t of times) {
    App.anim.evaluate(t);
    for (const o of objs) {
      if (o.inca.kind !== 'mesh') continue; o.updateWorldMatrix(true, false);
      const g = new THREE.Mesh(o.userData.mesh.geometry.clone(), new THREE.MeshBasicMaterial({ color: t < t0 ? 0x5577ff : 0xff7755, transparent: true, opacity: 0.18 + 0.1 / (1 + Math.abs(t - t0) / 10), depthWrite: false }));
      g.matrixAutoUpdate = false; g.matrix.copy(o.matrixWorld); g.userData.helper = true; App.scene.add(g); ghosts.push(g);
    }
  }
  App.anim.evaluate(t0); App.requestRender(); echo('ghosting;');
}
let trails = [];
function motionTrail(objs) {
  for (const t of trails) { t.parent?.remove(t); t.geometry.dispose(); } trails = [];
  if (!objs.length) { App.requestRender(); return warn('Select animated objects'); }
  const t0 = App.time.current; const pts = objs.map(() => []);
  for (let t = App.time.start; t <= App.time.end; t++) { App.anim.evaluate(t); objs.forEach((o, i) => pts[i].push(S.worldPivot(o))); }
  App.anim.evaluate(t0);
  objs.forEach((o, i) => { const g = new THREE.BufferGeometry().setFromPoints(pts[i]); const l = new THREE.Line(g, lnMat); l.userData.helper = true; App.scene.add(l); trails.push(l); const p = new THREE.Points(g.clone(), ptsMat); p.userData.helper = true; App.scene.add(p); trails.push(p); });
  App.requestRender(); echo('snapshot -motionTrail 1 -increment 1;');
}
App.on('sceneLoaded', () => { clearGhosts(); for (const t of trails) t.parent?.remove(t); trails = []; });
