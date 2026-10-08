// Inca — scene graph: DAG nodes (transform + shape), construction history, attributes, serialization.
import * as THREE from 'three';
import { RectAreaLightUniformsLib } from 'three/addons/lights/RectAreaLightUniformsLib.js';
import { App } from './app.js';
import { PolyMesh, buildGeometry, buildEdgeGeometry, subdivide, combineMeshes, transformMesh, components, opExtractFaces } from './polymesh.js';
import { OPS } from './ops.js';
import { GEN_INFO } from './primitives.js';
import { Curve } from './curves.js';
import { uniqueName, releaseName, renameTo } from './names.js';
import { getThreeMaterial, serializeShading, loadShading, initDefaultMaterials } from './materials.js';

RectAreaLightUniformsLib.init();

export const COLORS = {
  wire: 0x10106e, lead: 0x5cf5a2, sel: 0xffffff, hilite: 0x86c7ff, template: 0x8c8c8c, icon: 0x101010, curve: 0x0d0d66, locator: 0x10106e,
};
export const WIRE_MATS = {};
for (const [k, c] of Object.entries(COLORS)) WIRE_MATS[k] = new THREE.LineBasicMaterial({ color: c, depthTest: true });
WIRE_MATS.leadX = new THREE.LineBasicMaterial({ color: COLORS.lead, depthTest: false, transparent: true, opacity: 0.5 });

const D2R = Math.PI / 180, R2D = 180 / Math.PI;
export const RO = { xyz: 'ZYX', yzx: 'XZY', zxy: 'YXZ', xzy: 'YZX', yxz: 'ZXY', zyx: 'XYZ' };
const _v = new THREE.Vector3(), _v2 = new THREE.Vector3(), _q = new THREE.Quaternion(), _e = new THREE.Euler(), _s = new THREE.Vector3(), _m = new THREE.Matrix4();
const _qjo = new THREE.Quaternion(), _ejo = new THREE.Euler(); // rigging: joint orient
export function joQuat(inc, out = _qjo) { return out.setFromEuler(_ejo.set(inc.jo[0] * D2R, inc.jo[1] * D2R, inc.jo[2] * D2R, 'ZYX')); } // rigging

// ------------------------------------------------------------------ transforms
export function localMatrix(inc, out = new THREE.Matrix4()) {
  _e.set(inc.r[0] * D2R, inc.r[1] * D2R, inc.r[2] * D2R, RO[inc.ro] || 'ZYX');
  _q.setFromEuler(_e);
  if (inc.jo) _q.premultiply(joQuat(inc)); // rigging: joint orient (local = T * JO * R * S)
  out.compose(_v.set(0, 0, 0), _q, _s.set(inc.s[0], inc.s[1], inc.s[2]));
  _v2.set(inc.p[0], inc.p[1], inc.p[2]).applyMatrix4(out); // RS * p
  out.setPosition(inc.t[0] + inc.pt[0] + inc.p[0] - _v2.x, inc.t[1] + inc.pt[1] + inc.p[1] - _v2.y, inc.t[2] + inc.pt[2] + inc.p[2] - _v2.z);
  return out;
}
export function updateXform(o) {
  localMatrix(o.inca, o.matrix);
  o.matrixWorldNeedsUpdate = true;
  o.updateMatrixWorld(true);
  if (o.inca.kind === 'light') syncLight(o);
  if (o.inca.kind === 'curve' || hasCurveDescendant(o)) queueDependents(o);
}
function hasCurveDescendant(o) { let r = false; o.traverse(c => { if (c.inca && c.inca.kind === 'curve') r = true; }); return r; }
export function worldMatrix(o) { o.updateWorldMatrix(true, false); return o.matrixWorld.clone(); }
export function parentWorld(o) { if (o.parent) { o.parent.updateWorldMatrix(true, false); return o.parent.matrixWorld.clone(); } return new THREE.Matrix4(); }
function nearAngle(prev, a) { while (a - prev > 180) a -= 360; while (a - prev < -180) a += 360; return a; }
// set channels so that the local matrix equals m (keeps pivots)
export function setLocalMatrix(o, m) {
  const inc = o.inca;
  const pos = new THREE.Vector3(), q = new THREE.Quaternion(), s = new THREE.Vector3();
  m.decompose(pos, q, s);
  _e.setFromQuaternion(inc.jo ? joQuat(inc, new THREE.Quaternion()).invert().multiply(q) : q, RO[inc.ro] || 'ZYX'); // rigging: remove joint orient
  const r = [_e.x * R2D, _e.y * R2D, _e.z * R2D].map((a, i) => +nearAngle(inc.r[i], a).toFixed(6));
  inc.r = r; inc.s = [s.x, s.y, s.z].map(x => +x.toFixed(6));
  const rs = new THREE.Matrix4().compose(new THREE.Vector3(), q, s);
  const rp = new THREE.Vector3(...inc.p).applyMatrix4(rs);
  inc.t = [pos.x - inc.pt[0] - inc.p[0] + rp.x, pos.y - inc.pt[1] - inc.p[1] + rp.y, pos.z - inc.pt[2] - inc.p[2] + rp.z].map(x => +x.toFixed(6));
  updateXform(o);
}
export function setWorldMatrix(o, wm) {
  const pinv = parentWorld(o).invert();
  setLocalMatrix(o, pinv.multiply(wm));
}
export function worldPivot(o) {
  const inc = o.inca;
  const p = new THREE.Vector3(inc.t[0] + inc.pt[0] + inc.p[0], inc.t[1] + inc.pt[1] + inc.p[1], inc.t[2] + inc.pt[2] + inc.p[2]);
  if (o.parent) { o.parent.updateWorldMatrix(true, false); p.applyMatrix4(o.parent.matrixWorld); }
  return p;
}
export function setPivotLocal(o, np) { // move pivot without moving the object
  const inc = o.inca;
  const m = localMatrix(inc); const rs = m.clone().setPosition(0, 0, 0);
  const d = new THREE.Vector3(inc.p[0] - np[0], inc.p[1] - np[1], inc.p[2] - np[2]);
  const rsd = d.clone().applyMatrix4(rs);
  inc.pt = [inc.pt[0] + d.x - rsd.x, inc.pt[1] + d.y - rsd.y, inc.pt[2] + d.z - rsd.z];
  inc.p = np.slice();
  updateXform(o);
}
export function setPivotWorld(o, wp) {
  o.updateWorldMatrix(true, false);
  const inv = o.matrixWorld.clone().invert();
  const lp = wp.clone().applyMatrix4(inv);
  setPivotLocal(o, [lp.x, lp.y, lp.z]);
}

// ------------------------------------------------------------------ DAG basics
export const isDag = (o) => !!(o && o.isObject3D && o.inca);
export function dagChildren(o) { return (o || App.world).children.filter(c => c.inca); }
export function allDag(includeStartup = false) {
  const out = []; App.world.traverse(o => { if (o.inca && (includeStartup || !o.inca.startup)) out.push(o); }); return out;
}
export function dagParent(o) { return o.parent && o.parent.inca ? o.parent : null; }
export function findNode(name) {
  if (!name) return null;
  for (const n of App.nodes.values()) { if (n.isObject3D ? n.inca.name === name : n.name === name) return n; }
  // shape name -> transform
  for (const n of App.nodes.values()) if (n.isObject3D && n.inca.shapeName === name) return n;
  return null;
}
export const nodeName = (n) => n ? (n.isObject3D ? n.inca.name : n.name) : '';
export function fullPath(o) { const p = []; let c = o; while (c && c.inca) { p.unshift(c.inca.name); c = c.parent; } return '|' + p.join('|'); }

export function makeDag(kind, name, opts = {}) { // rigging: exported for rig node factories
  const o = new THREE.Group(); o.matrixAutoUpdate = false;
  const id = opts.id || App.newId('d');
  o.inca = { id, name: opts.exactName ? name : uniqueName(name), kind, t: [0, 0, 0], r: [0, 0, 0], s: [1, 1, 1], ro: 'xyz', p: [0, 0, 0], pt: [0, 0, 0], visible: true, layer: null, template: false, locks: {}, shapeName: null };
  o.name = o.inca.name;
  App.nodes.set(id, o);
  (opts.parent || App.world).add(o);
  updateXform(o);
  return o;
}
const shapeNameFor = (n) => n.match(/\d+$/) ? n.replace(/(\d+)$/, 'Shape$1') : n + 'Shape';
export const helper = (obj) => { obj.userData.helper = true; return obj; }; // rigging: exported

// ------------------------------------------------------------------ meshes
export function createMesh(genType, params = {}, opts = {}) {
  const info = GEN_INFO[genType];
  const base = opts.name || ((info ? info.base : 'polySurface') + '1');
  const o = makeDag('mesh', base, opts);
  o.inca.shapeName = opts.shapeName || uniqueName(shapeNameFor(o.inca.name));
  o.inca.material = opts.material || App.defaultMatId;
  o.inca.smoothLevel = 0;
  o.inca.nurbs = !!(info && info.nurbs);
  o.inca.history = [];
  o.inca.compSel = { v: new Set(), e: new Set(), f: new Set() };
  const mesh = new THREE.Mesh(new THREE.BufferGeometry(), getThreeMaterial(o.inca.material));
  mesh.castShadow = true; mesh.receiveShadow = true;
  const wire = new THREE.LineSegments(new THREE.BufferGeometry(), WIRE_MATS.wire);
  wire.renderOrder = 1;
  o.add(helper(mesh)); o.add(helper(wire));
  o.userData.mesh = mesh; o.userData.wire = wire;
  mesh.userData.owner = o;
  if (genType) {
    const op = OPS[genType];
    const defaults = info ? { ...info.params } : {};
    addHistory(o, genType, { ...defaults, ...params }, { noEval: true, name: opts.historyName });
  }
  if (opts.mesh) { o.inca.mesh = opts.mesh; }
  else evaluate(o, 0);
  if (opts.mesh) rebuildShape(o);
  return o;
}
export function addHistory(o, type, params, { noEval = false, name = null, id = null } = {}) {
  const op = OPS[type];
  const h = { id: id || App.newId('h'), name: name ? name : uniqueName((op && op.base ? op.base : type) + '1'), type, params, kind: 'history', owner: o.inca.id };
  if (op && op.hidden) { releaseName(h.name); h.name = (op.base || type) + '#' + h.id; }
  o.inca.history.push(h);
  App.nodes.set(h.id, h);
  if (!noEval) evaluate(o, o.inca.history.length - 1);
  return h;
}
export function historyOwner(h) { return App.nodes.get(h.owner); }
export function evaluate(o, from = 0) {
  const H = o.inca.history;
  if (!H || !H.length) { rebuildShape(o); return null; }
  let pm = null;
  if (from > 0 && H[from - 1]._out) pm = H[from - 1]._out; else from = 0;
  let sel = null;
  for (let i = from; i < H.length; i++) {
    const h = H[i]; const op = OPS[h.type];
    if (!op) continue;
    try { const r = op.apply(pm || new PolyMesh(), h.params); pm = r.mesh; sel = r.select || null; h._out = pm; if (r.error) h._error = r.error; else delete h._error; }
    catch (e) { console.error('op failed', h.type, e); h._error = String(e); h._out = pm; }
  }
  o.inca.mesh = pm || new PolyMesh();
  rebuildShape(o);
  return sel;
}
export function deleteHistory(o) {
  if (!o.inca.history) return;
  for (const h of o.inca.history) { App.nodes.delete(h.id); releaseName(h.name); }
  const h = { id: App.newId('h'), name: 'meshData#', type: 'meshData', params: { data: o.inca.mesh.toJSON() }, kind: 'history', owner: o.inca.id };
  h._out = o.inca.mesh;
  o.inca.history = [h]; App.nodes.set(h.id, h);
}
export function bakeMesh(o, pm) { // replace mesh without history
  for (const h of o.inca.history || []) { App.nodes.delete(h.id); releaseName(h.name); }
  o.inca.history = [];
  o.inca.mesh = pm;
  const h = { id: App.newId('h'), name: 'meshData#', type: 'meshData', params: { data: pm.toJSON() }, kind: 'history', owner: o.inca.id };
  h._out = pm; o.inca.history.push(h); App.nodes.set(h.id, h);
  rebuildShape(o);
}
// apply a modeling op to an object's mesh as new history
export function applyOp(o, type, params) {
  const h = addHistory(o, type, params, { noEval: true });
  const sel = evaluate(o, o.inca.history.length - 1);
  return { h, select: sel };
}
// fast path during interactive component drags
export function setMeshPositions(o, verts, positions) {
  const pm = o.inca.mesh;
  for (let i = 0; i < verts.length; i++) pm.v[verts[i]] = [positions[i * 3], positions[i * 3 + 1], positions[i * 3 + 2]];
  pm.invalidate(); pm._smooth = null;
  rebuildShape(o);
}
export function commitTweak(o, verts, deltas) {
  const H = o.inca.history; const last = H[H.length - 1];
  if (last && last.type === 'polyTweak') {
    // merge into existing tweak
    const map = new Map(); last.params.verts.forEach((v, i) => map.set(v, [last.params.deltas[i * 3], last.params.deltas[i * 3 + 1], last.params.deltas[i * 3 + 2]]));
    verts.forEach((v, i) => { const d = map.get(v) || [0, 0, 0]; map.set(v, [d[0] + deltas[i * 3], d[1] + deltas[i * 3 + 1], d[2] + deltas[i * 3 + 2]]); });
    last.params.verts = [...map.keys()]; last.params.deltas = [...map.values()].flat();
    last._out = o.inca.mesh;
  } else {
    const h = addHistory(o, 'polyTweak', { verts: verts.slice(), deltas: Array.from(deltas) }, { noEval: true });
    h._out = o.inca.mesh;
  }
}

export function rebuildShape(o) {
  const inc = o.inca;
  if (inc.kind === 'curve') return rebuildCurve(o);
  if (inc.kind !== 'mesh') return;
  const pm = (inc.rig && App.rig?.deform ? App.rig.deform(o) : null) || inc.mesh || new PolyMesh(); // rigging: deformed display
  const mesh = o.userData.mesh, wire = o.userData.wire;
  let disp = pm;
  if (inc.smoothLevel) { if (!pm._smooth) pm._smooth = subdivide(pm, App.prefs?.smoothDivisions || 2, true); disp = pm._smooth; }
  const ids = [inc.material || App.defaultMatId];
  if (pm.fm) for (const m of pm.fm) if (m && !ids.includes(m) && App.mats.has(m)) ids.push(m);
  const dispFm = disp.fm;
  const { geometry, triFace } = buildGeometry(disp, (m) => { const i = m ? ids.indexOf(m) : 0; return i < 0 ? 0 : i; });
  mesh.geometry.dispose(); mesh.geometry = geometry;
  mesh.userData.triFace = triFace;
  mesh.userData.matIds = ids;
  mesh.userData.matsT = ids.map(id => getThreeMaterial(id, true));
  mesh.userData.matsU = ids.map(id => getThreeMaterial(id, false));
  mesh.material = mesh.userData.matsT;
  if (inc.smoothLevel) { const b = buildGeometry(pm); mesh.userData.pick = b; } else mesh.userData.pick = null;
  wire.geometry.dispose();
  wire.geometry = buildEdgeGeometry(inc.smoothLevel === 2 ? disp : pm, true);
  o.userData.compDirty = true;
  App.requestRender();
}
export function refreshMaterials(o) { if (o.inca.kind === 'mesh') rebuildShape(o); }

// ------------------------------------------------------------------ groups / locators
export function createGroup(name = 'group1', opts = {}) { return makeDag('group', name, opts); }
export function createLocator(name = 'locator1', opts = {}) {
  const o = makeDag('locator', name, opts);
  o.inca.shapeName = uniqueName(shapeNameFor(o.inca.name));
  const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute([-1, 0, 0, 1, 0, 0, 0, -1, 0, 0, 1, 0, 0, 0, -1, 0, 0, 1], 3));
  const l = helper(new THREE.LineSegments(g, WIRE_MATS.locator)); o.add(l); o.userData.wire = l; o.userData.icon = l;
  addPick(o, new THREE.BoxGeometry(2, 2, 2));
  return o;
}
export function addPick(o, geom) { // rigging: exported
  const m = helper(new THREE.Mesh(geom, new THREE.MeshBasicMaterial({ visible: false })));
  m.userData.owner = o; m.userData.isPick = true; o.add(m); o.userData.pick = m;
}

// ------------------------------------------------------------------ lights
export const LIGHT_DEFAULTS = {
  ambientLight: { color: [1, 1, 1], intensity: 1, ambientShade: 0.45 },
  directionalLight: { color: [1, 1, 1], intensity: 1, shadows: false, shadowRadius: 2, shadowColor: [0, 0, 0] },
  pointLight: { color: [1, 1, 1], intensity: 1, decayRate: 0, shadows: false, shadowRadius: 2 },
  spotLight: { color: [1, 1, 1], intensity: 1, decayRate: 0, coneAngle: 40, penumbraAngle: 0, dropoff: 0, shadows: false, shadowRadius: 2 },
  areaLight: { color: [1, 1, 1], intensity: 1, decayRate: 2, shadows: false },
  skyDomeLight: { color: [1, 1, 1], intensity: 1, textureFile: '', showBackground: true },
};
const DECAY = [0, 1, 2, 3];
export function createLight(type = 'pointLight', opts = {}) {
  const o = makeDag('light', opts.name || type + '1', opts);
  o.inca.shapeName = opts.shapeName || uniqueName(shapeNameFor(o.inca.name));
  o.inca.light = { type, ...JSON.parse(JSON.stringify(LIGHT_DEFAULTS[type] || {})), ...(opts.light || {}) };
  let L;
  if (type === 'ambientLight') L = new THREE.AmbientLight();
  else if (type === 'directionalLight') L = new THREE.DirectionalLight();
  else if (type === 'pointLight') L = new THREE.PointLight();
  else if (type === 'spotLight') L = new THREE.SpotLight();
  else if (type === 'areaLight') L = new THREE.RectAreaLight(0xffffff, 1, 2, 2);
  else if (type === 'skyDomeLight') L = new THREE.HemisphereLight(0xffffff, 0x888888, 0);
  helper(L); L.userData.owner = o;
  if (L.target) { const t = helper(new THREE.Object3D()); t.position.set(0, 0, -1); o.add(t); L.target = t; }
  if (L.shadow) { L.shadow.mapSize.set(2048, 2048); L.shadow.bias = -0.0005; L.shadow.normalBias = 0.02; if (L.shadow.camera.isOrthographicCamera) { const c = L.shadow.camera; c.left = c.bottom = -25; c.right = c.top = 25; c.near = 0.1; c.far = 500; } }
  o.add(L); o.userData.light = L;
  const icon = helper(new THREE.LineSegments(lightIcon(type), WIRE_MATS.icon)); o.add(icon); o.userData.icon = icon; o.userData.wire = icon;
  addPick(o, new THREE.BoxGeometry(1.2, 1.2, type === 'directionalLight' || type === 'spotLight' ? 2 : 1.2));
  syncLight(o);
  return o;
}
export function syncLight(o) {
  const L = o.userData.light; if (!L) return; const a = o.inca.light;
  L.color.setRGB(a.color[0], a.color[1], a.color[2]);
  const scaleI = App.lightScale ?? 1;
  const I = (a.intensity || 0) * Math.PI * scaleI;
  if (a.type === 'skyDomeLight') { L.intensity = 0; o.userData.skyIntensity = I; App.emit('skyChanged', o); }
  else L.intensity = I;
  if (L.isPointLight || L.isSpotLight) { L.decay = DECAY[a.decayRate || 0]; L.distance = 0; }
  if (L.isSpotLight) { L.angle = Math.min(89, a.coneAngle / 2 + Math.max(0, a.penumbraAngle)) * D2R; L.penumbra = Math.max(0, Math.min(1, Math.abs(a.penumbraAngle) / Math.max(1, a.coneAngle / 2 + Math.abs(a.penumbraAngle)) + a.dropoff / 100)); }
  if (L.isRectAreaLight) { L.width = 2 * Math.abs(o.inca.s[0]); L.height = 2 * Math.abs(o.inca.s[1]); }
  if ('castShadow' in L && L.shadow) { L.castShadow = !!a.shadows; L.shadow.radius = a.shadowRadius || 1; }
  App.requestRender();
}
function lightIcon(type) {
  const p = [];
  const seg = (a, b) => p.push(...a, ...b);
  const circle = (r, axis = 'z', z = 0, n = 24) => { for (let i = 0; i < n; i++) { const a = i / n * Math.PI * 2, b = (i + 1) / n * Math.PI * 2; const P = (t) => axis === 'z' ? [r * Math.cos(t), r * Math.sin(t), z] : axis === 'y' ? [r * Math.cos(t), z, r * Math.sin(t)] : [z, r * Math.cos(t), r * Math.sin(t)]; seg(P(a), P(b)); } };
  if (type === 'pointLight') { circle(0.25, 'z'); circle(0.25, 'y'); circle(0.25, 'x'); for (const d of [[1, 0, 0], [0, 1, 0], [0, 0, 1], [0.7, 0.7, 0], [0.7, -0.7, 0], [0, 0.7, 0.7], [0, 0.7, -0.7], [0.7, 0, 0.7], [-0.7, 0, 0.7]]) { seg(d.map(x => x * 0.35), d.map(x => x * 0.6)); seg(d.map(x => -x * 0.35), d.map(x => -x * 0.6)); } }
  else if (type === 'directionalLight') { for (const [x, y] of [[-0.3, 0], [0.3, 0], [0, 0.3]]) { seg([x, y, 0.5], [x, y, -0.5]); seg([x, y, -0.5], [x + 0.08, y, -0.3]); seg([x, y, -0.5], [x - 0.08, y, -0.3]); seg([x, y, -0.5], [x, y + 0.08, -0.3]); seg([x, y, -0.5], [x, y - 0.08, -0.3]); } }
  else if (type === 'spotLight') { circle(0.15, 'z', 0); const r = 0.5, z = -1.2; circle(r, 'z', z); for (let i = 0; i < 4; i++) { const a = i / 4 * Math.PI * 2; seg([0.15 * Math.cos(a), 0.15 * Math.sin(a), 0], [r * Math.cos(a), r * Math.sin(a), z]); } seg([0, 0, 0], [0, 0, -1.5]); }
  else if (type === 'areaLight') { seg([-1, -1, 0], [1, -1, 0]); seg([1, -1, 0], [1, 1, 0]); seg([1, 1, 0], [-1, 1, 0]); seg([-1, 1, 0], [-1, -1, 0]); seg([-1, -1, 0], [1, 1, 0]); seg([1, -1, 0], [-1, 1, 0]); seg([0, 0, 0], [0, 0, -1]); }
  else if (type === 'ambientLight') { circle(0.3, 'z'); circle(0.3, 'x'); circle(0.3, 'y'); circle(0.5, 'y', 0, 6); }
  else { circle(0.6, 'y'); for (let i = 0; i < 8; i++) { const a = i / 8 * Math.PI * 2; seg([0.6 * Math.cos(a), 0, 0.6 * Math.sin(a)], [0.3 * Math.cos(a), 0.5, 0.3 * Math.sin(a)]); } circle(0.3, 'y', 0.5); }
  const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(p, 3)); return g;
}

// ------------------------------------------------------------------ cameras
export function createCamera(name = 'camera1', opts = {}) {
  const o = makeDag('camera', name, opts);
  o.inca.shapeName = opts.shapeName || uniqueName(shapeNameFor(o.inca.name));
  o.inca.cam = { focalLength: 35, near: 0.1, far: 10000, ortho: false, orthoWidth: 30, coi: 10, horizontalFilmAperture: 1.417, verticalFilmAperture: 0.945, startup: false, ...(opts.cam || {}) };
  o.inca.startup = !!o.inca.cam.startup;
  const cam = o.inca.cam.ortho ? new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 10000) : new THREE.PerspectiveCamera(50, 1, 0.1, 10000);
  helper(cam); cam.userData.owner = o; o.add(cam); o.userData.camera = cam;
  if (!o.inca.startup) {
    const icon = helper(new THREE.LineSegments(cameraIcon(), WIRE_MATS.icon)); o.add(icon); o.userData.icon = icon; o.userData.wire = icon;
    addPick(o, new THREE.BoxGeometry(1, 1, 1.6));
  }
  return o;
}
function cameraIcon() {
  const p = []; const seg = (a, b) => p.push(...a, ...b);
  const bx = (x0, y0, z0, x1, y1, z1) => { const c = [[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]]; [[0, 1], [1, 2], [2, 3], [3, 0], [4, 5], [5, 6], [6, 7], [7, 4], [0, 4], [1, 5], [2, 6], [3, 7]].forEach(([a, b]) => seg(c[a], c[b])); };
  bx(-0.25, -0.3, 0.6, 0.25, 0.3, -0.2);
  const r = 0.25; for (let i = 0; i < 4; i++) { const a = i / 4 * Math.PI * 2 + Math.PI / 4; seg([0.12 * Math.cos(a), 0.12 * Math.sin(a), -0.2], [r * Math.cos(a), r * Math.sin(a), -0.6]); seg([r * Math.cos(a), r * Math.sin(a), -0.6], [r * Math.cos(a + Math.PI / 2), r * Math.sin(a + Math.PI / 2), -0.6]); }
  // film reels
  for (const z of [0.15, 0.45]) for (let i = 0; i < 12; i++) { const a = i / 12 * Math.PI * 2, b = (i + 1) / 12 * Math.PI * 2; seg([0, 0.3 + 0.15 + 0.15 * Math.sin(a), z + 0.15 * Math.cos(a) - 0], [0, 0.3 + 0.15 + 0.15 * Math.sin(b), z + 0.15 * Math.cos(b)]); }
  const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(p, 3)); return g;
}
export function syncCamera(o, aspect = 1) {
  const c = o.userData.camera; const a = o.inca.cam;
  if (a.ortho) {
    const w = a.orthoWidth / 2, h = w / aspect;
    c.left = -w; c.right = w; c.top = h; c.bottom = -h; c.near = -10000; c.far = 10000;
  } else {
    const filmAspect = a.horizontalFilmAperture / a.verticalFilmAperture;
    let vfov;
    if (aspect >= filmAspect) { const hf = 2 * Math.atan((a.horizontalFilmAperture * 25.4 / 2) / a.focalLength); vfov = 2 * Math.atan(Math.tan(hf / 2) / aspect); }
    else vfov = 2 * Math.atan((a.verticalFilmAperture * 25.4 / 2) / a.focalLength);
    c.fov = vfov * R2D; c.aspect = aspect; c.near = a.near; c.far = a.far;
  }
  c.updateProjectionMatrix();
}
export function createStartupCameras() {
  const mk = (name, t, r, cam) => { const o = createCamera(name, { exactName: true, cam: { startup: true, ...cam } }); o.inca.t = t; o.inca.r = r; updateXform(o); return o; };
  const persp = mk('persp', [28, 21, 28], [-27.938, 45, 0], { coi: 44.82 });
  const top = mk('top', [0, 1000.1, 0], [-90, 0, 0], { ortho: true, orthoWidth: 30, coi: 1000.1 });
  const front = mk('front', [0, 0, 1000.1], [0, 0, 0], { ortho: true, orthoWidth: 30, coi: 1000.1 });
  const side = mk('side', [1000.1, 0, 0], [0, 90, 0], { ortho: true, orthoWidth: 30, coi: 1000.1 });
  return { persp, top, front, side };
}

// ------------------------------------------------------------------ curves
export function createCurve(curve, opts = {}) {
  const o = makeDag('curve', opts.name || 'curve1', opts);
  o.inca.shapeName = opts.shapeName || uniqueName(shapeNameFor(o.inca.name));
  o.inca.curve = curve;
  o.inca.compSel = { v: new Set(), e: new Set(), f: new Set() };
  const line = helper(new THREE.Line(new THREE.BufferGeometry(), WIRE_MATS.curve)); line.userData.owner = o;
  o.add(line); o.userData.line = line; o.userData.wire = line;
  rebuildCurve(o);
  return o;
}
function rebuildCurve(o) {
  const line = o.userData.line; const pts = o.inca.curve.sample(16);
  const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(pts.flat(), 3)); g.computeBoundingSphere();
  line.geometry.dispose(); line.geometry = g;
  o.userData.compDirty = true;
  queueDependents(o);
  App.requestRender();
}
// surfaces built from curves update when the curve changes
const depQueue = new Set(); let depRaf = 0;
function queueDependents(o) {
  o.traverse(c => { if (c.inca && c.inca.kind === 'curve') depQueue.add(c.inca.id); });
  if (!depRaf && depQueue.size) depRaf = (typeof requestAnimationFrame !== 'undefined' ? requestAnimationFrame : setTimeout)(flushDependents);
}
export function flushDependents() {
  depRaf = 0; if (!depQueue.size) return;
  const ids = new Set(depQueue); depQueue.clear();
  for (const n of App.nodes.values()) {
    if (!n.isObject3D || n.inca.kind !== 'mesh' || !n.inca.history) continue;
    const H = n.inca.history; const idx = H.findIndex(h => { const p = h.params || {}; return ids.has(p.curve) || ids.has(p.profile) || ids.has(p.path) || (p.curves || []).some(c => ids.has(c)); });
    if (idx >= 0) evaluate(n, idx);
  }
}

// ------------------------------------------------------------------ attributes
export const XF_ATTRS = {
  translateX: ['t', 0], translateY: ['t', 1], translateZ: ['t', 2], rotateX: ['r', 0], rotateY: ['r', 1], rotateZ: ['r', 2], scaleX: ['s', 0], scaleY: ['s', 1], scaleZ: ['s', 2],
  rotatePivotX: ['p', 0], rotatePivotY: ['p', 1], rotatePivotZ: ['p', 2],
};
export const ALIAS = { tx: 'translateX', ty: 'translateY', tz: 'translateZ', rx: 'rotateX', ry: 'rotateY', rz: 'rotateZ', sx: 'scaleX', sy: 'scaleY', sz: 'scaleZ', v: 'visibility', vis: 'visibility', rpx: 'rotatePivotX', rpy: 'rotatePivotY', rpz: 'rotatePivotZ', fl: 'focalLength', i: 'intensity' };
export const CHANNELS = ['translateX', 'translateY', 'translateZ', 'rotateX', 'rotateY', 'rotateZ', 'scaleX', 'scaleY', 'scaleZ', 'visibility'];
const COLOR_SUFFIX = { R: 0, G: 1, B: 2 };

export function resolveAttr(rec, attr) {
  attr = ALIAS[attr] || attr;
  return attr;
}
export function getAttr(rec, attr) {
  attr = resolveAttr(rec, attr);
  if (!rec) return undefined;
  if (rec.isObject3D) {
    const inc = rec.inca;
    if (XF_ATTRS[attr]) { const [k, i] = XF_ATTRS[attr]; return inc[k][i]; }
    if (attr === 'translate' || attr === 'rotate' || attr === 'scale') return inc[attr[0]].slice();
    if (attr === 'visibility') return inc.visible ? 1 : 0;
    if (attr === 'rotateOrder') return inc.ro;
    const sub = inc.light || inc.cam;
    if (sub) {
      if (attr in sub) return sub[attr];
      const m = attr.match(/^(.*)([RGB])$/); if (m && Array.isArray(sub[m[1]])) return sub[m[1]][COLOR_SUFFIX[m[2]]];
    }
    if (inc.kind === 'mesh' && attr === 'smoothLevel') return inc.smoothLevel;
    if (App.rig?.getAttr) return App.rig.getAttr(rec, attr); // rigging: extraAttrs / jointOrient
    return undefined;
  }
  const bag = rec.kind === 'history' ? rec.params : rec.attrs;
  if (!bag) return undefined;
  if (attr in bag) return bag[attr];
  const m = attr.match(/^(.*)([RGB])$/); if (m && Array.isArray(bag[m[1]])) return bag[m[1]][COLOR_SUFFIX[m[2]]];
  return undefined;
}
export function isLocked(rec, attr) { return !!(rec && rec.isObject3D && rec.inca.locks[resolveAttr(rec, attr)]); }
export function setAttr(rec, attr, value, opts = {}) {
  attr = resolveAttr(rec, attr);
  if (!rec) return false;
  if (!opts.force && isLocked(rec, attr)) return false;
  if (rec.isObject3D) {
    const inc = rec.inca;
    if (XF_ATTRS[attr]) { const [k, i] = XF_ATTRS[attr]; inc[k][i] = +value; updateXform(rec); }
    else if (attr === 'translate' || attr === 'rotate' || attr === 'scale') { inc[attr[0]] = value.map(Number); updateXform(rec); }
    else if (attr === 'visibility') { inc.visible = !!(+value); applyVisibility(rec); }
    else if (attr === 'rotateOrder') { inc.ro = value; updateXform(rec); }
    else if (inc.kind === 'mesh' && attr === 'smoothLevel') { inc.smoothLevel = value; rebuildShape(rec); }
    else if (App.rig?.setAttr && App.rig.setAttr(rec, attr, value)) { /* rigging: extraAttrs / jointOrient */ }
    else {
      const sub = inc.light || inc.cam; if (!sub) return false;
      const m = attr.match(/^(.*)([RGB])$/);
      if (attr in sub) sub[attr] = Array.isArray(sub[attr]) ? value.slice() : value;
      else if (m && Array.isArray(sub[m[1]])) sub[m[1]][COLOR_SUFFIX[m[2]]] = +value;
      else return false;
      if (inc.light) syncLight(rec);
    }
    App.requestRender();
    if (!opts.silent) App.dirty('channels', 'attr');
    return true;
  }
  const bag = rec.kind === 'history' ? rec.params : rec.attrs; if (!bag) return false;
  const m = attr.match(/^(.*)([RGB])$/);
  if (attr in bag) bag[attr] = Array.isArray(bag[attr]) ? value.slice() : value;
  else if (m && Array.isArray(bag[m[1]])) bag[m[1]][COLOR_SUFFIX[m[2]]] = +value;
  else return false;
  if (rec.kind === 'history') { const o = historyOwner(rec); if (o) { const i = o.inca.history.indexOf(rec); evaluate(o, Math.max(0, i)); } }
  else App.emit('shadingAttr', rec);
  if (!opts.silent) App.dirty('channels', 'attr', 'outliner');
  return true;
}

export function applyVisibility(o) {
  const lay = o.inca.layer ? App.layers.find(l => l.id === o.inca.layer) : null;
  o.visible = o.inca.visible && (!lay || lay.visible);
  App.requestRender();
}

// ------------------------------------------------------------------ hierarchy ops
export function parentTo(child, parent, keepWorld = true) {
  if (child === parent) return false;
  let p = parent; while (p) { if (p === child) return false; p = p.parent; }
  const wm = worldMatrix(child);
  (parent || App.world).add(child);
  if (keepWorld) setWorldMatrix(child, wm); else updateXform(child);
  return true;
}
export function deleteNode(o) {
  if (!o || !o.inca || o.inca.startup) return;
  for (const c of dagChildren(o)) deleteNode(c);
  // surfaces that depend on a deleted curve keep their current shape
  if (o.inca.kind === 'curve') {
    for (const n of App.nodes.values()) if (n.isObject3D && n.inca.kind === 'mesh' && n.inca.history?.some(h => { const p = h.params || {}; return p.curve === o.inca.id || p.profile === o.inca.id || p.path === o.inca.id || (p.curves || []).includes(o.inca.id); })) bakeMesh(n, n.inca.mesh);
  }
  if (o.inca.history) for (const h of o.inca.history) { App.nodes.delete(h.id); releaseName(h.name); }
  App.emit('nodeDeleted', o);
  o.parent && o.parent.remove(o);
  o.traverse(c => { if (c.geometry && c.userData.helper) c.geometry.dispose(); });
  App.nodes.delete(o.inca.id); releaseName(o.inca.name); if (o.inca.shapeName) releaseName(o.inca.shapeName);
  App.sel = App.sel.filter(s => s !== o); App.hilite = App.hilite.filter(s => s !== o);
}
export function rename(rec, want) {
  if (!rec || !want) return;
  if (rec.isObject3D) {
    if (rec.inca.startup) return;
    rec.inca.name = renameTo(rec.inca.name, want); rec.name = rec.inca.name;
    if (rec.inca.shapeName) { rec.inca.shapeName = renameTo(rec.inca.shapeName, shapeNameFor(rec.inca.name)); }
  } else rec.name = renameTo(rec.name, want);
  App.dirty('outliner', 'channels', 'attr');
}
export function localBBox(o, includeChildren = true) {
  // bbox of shape (and descendants) in o's local space
  const box = new THREE.Box3();
  o.updateWorldMatrix(true, true);
  const inv = o.matrixWorld.clone().invert();
  o.traverse(c => {
    let pts = null;
    if (c.inca && c.inca.kind === 'mesh' && c.inca.mesh) pts = c.inca.mesh.v;
    else if (c.inca && c.inca.kind === 'curve') pts = c.inca.curve.cvs;
    if (!pts || (!includeChildren && c !== o)) return;
    const m = inv.clone().multiply(c.matrixWorld);
    for (const p of pts) box.expandByPoint(_v.set(p[0], p[1], p[2]).applyMatrix4(m));
  });
  return box;
}
export function worldBBox(o) {
  const box = new THREE.Box3(); o.updateWorldMatrix(true, true);
  o.traverse(c => {
    let pts = null;
    if (c.inca && c.inca.kind === 'mesh' && c.inca.mesh) pts = c.inca.mesh.v;
    else if (c.inca && c.inca.kind === 'curve') pts = c.inca.curve.cvs;
    if (c.inca && !pts) box.expandByPoint(_v.setFromMatrixPosition(c.matrixWorld));
    if (!pts || !c.visible) return;
    for (const p of pts) box.expandByPoint(_v.set(p[0], p[1], p[2]).applyMatrix4(c.matrixWorld));
  });
  return box;
}
export function centerPivot(o) {
  const b = localBBox(o);
  if (b.isEmpty()) return;
  const c = b.getCenter(new THREE.Vector3());
  // pivot is expressed in the object's local (pre-transform) space; bbox is in local space already
  setPivotLocal(o, [c.x, c.y, c.z]);
}
export function freezeTransforms(o) {
  const inc = o.inca;
  const M = localMatrix(inc);
  const wp = [inc.t[0] + inc.pt[0] + inc.p[0], inc.t[1] + inc.pt[1] + inc.p[1], inc.t[2] + inc.pt[2] + inc.p[2]];
  if (inc.kind === 'mesh') {
    const pm = transformMesh(inc.mesh, M);
    bakeMesh(o, pm);
  } else if (inc.kind === 'curve') {
    const v = new THREE.Vector3();
    inc.curve.cvs = inc.curve.cvs.map(p => { v.set(p[0], p[1], p[2]).applyMatrix4(M); return [v.x, v.y, v.z]; });
  } else if (inc.kind !== 'group' && inc.kind !== 'locator') return;
  for (const c of dagChildren(o)) { const cm = M.clone().multiply(c.matrix); setLocalMatrix(c, cm); }
  inc.t = [0, 0, 0]; inc.r = [0, 0, 0]; inc.s = [1, 1, 1]; inc.pt = [0, 0, 0]; inc.p = wp;
  updateXform(o);
  if (inc.kind === 'curve') rebuildCurve(o);
}
export function resetTransforms(o) { o.inca.t = [0, 0, 0]; o.inca.r = [0, 0, 0]; o.inca.s = [1, 1, 1]; updateXform(o); }

// ------------------------------------------------------------------ serialization
export const DAG_FACTORIES = {}; // rigging: kind -> (name, opts, data) => DAG object (registered by core/rigging.js)
export function serializeNode(o) {
  const inc = o.inca;
  const d = { id: inc.id, name: inc.name, kind: inc.kind, parent: dagParent(o) ? dagParent(o).inca.id : null, t: inc.t, r: inc.r, s: inc.s, ro: inc.ro, p: inc.p, pt: inc.pt, visible: inc.visible, layer: inc.layer, template: inc.template, locks: inc.locks, shapeName: inc.shapeName, extra: inc.extra || null };
  if (inc.kind === 'mesh') {
    d.material = inc.material; d.smoothLevel = inc.smoothLevel; d.nurbs = inc.nurbs;
    d.history = inc.history.map(h => ({ id: h.id, name: h.name, type: h.type, params: h.params }));
    d.mesh = inc.mesh.toJSON();
  }
  if (inc.light) d.light = inc.light;
  if (inc.cam) d.cam = inc.cam;
  if (inc.curve) d.curve = inc.curve.toJSON();
  if (inc.rig) d.rig = JSON.parse(JSON.stringify(inc.rig)); // rigging
  if (inc.extraAttrs) d.extraAttrs = { ...inc.extraAttrs }; // rigging
  if (inc.jo) d.jo = inc.jo.slice(); // rigging
  return d;
}
export function deserializeNode(d, parentObj = null, { remap = null, fresh = false } = {}) {
  const opts = { id: fresh ? null : d.id, parent: parentObj, exactName: !fresh && !!d.cam?.startup };
  let o;
  const nm = d.name;
  if (d.kind === 'mesh') {
    o = createMesh(null, {}, { ...opts, name: nm, shapeName: fresh ? null : d.shapeName, material: d.material, mesh: PolyMesh.fromJSON(d.mesh) });
    o.inca.nurbs = !!d.nurbs;
    o.inca.history = [];
    for (const h of d.history || []) {
      const rec = { id: fresh ? App.newId('h') : h.id, name: h.name.includes('#') ? h.name : uniqueName(h.name), type: h.type, params: JSON.parse(JSON.stringify(h.params)), kind: 'history', owner: o.inca.id };
      if (remap) for (const k of ['curve', 'profile', 'path']) if (rec.params[k] && remap[rec.params[k]]) rec.params[k] = remap[rec.params[k]];
      o.inca.history.push(rec); App.nodes.set(rec.id, rec);
    }
    const last = o.inca.history[o.inca.history.length - 1]; if (last) last._out = o.inca.mesh;
    o.inca.smoothLevel = d.smoothLevel || 0;
  } else if (d.kind === 'light') o = createLight(d.light.type, { ...opts, name: nm, shapeName: fresh ? null : d.shapeName, light: d.light });
  else if (d.kind === 'camera') o = createCamera(nm, { ...opts, shapeName: fresh ? null : d.shapeName, cam: d.cam });
  else if (d.kind === 'curve') o = createCurve(Curve.fromJSON(d.curve), { ...opts, name: nm, shapeName: fresh ? null : d.shapeName });
  else if (d.kind === 'locator') o = createLocator(nm, opts);
  else if (DAG_FACTORIES[d.kind]) o = DAG_FACTORIES[d.kind](nm, opts, d); // rigging: joints, handles, lattices, constraints
  else o = createGroup(nm, opts);
  const inc = o.inca;
  inc.t = d.t.slice(); inc.r = d.r.slice(); inc.s = d.s.slice(); inc.ro = d.ro || 'xyz'; inc.p = (d.p || [0, 0, 0]).slice(); inc.pt = (d.pt || [0, 0, 0]).slice();
  inc.visible = d.visible !== false; inc.layer = d.layer || null; inc.template = !!d.template; inc.locks = { ...(d.locks || {}) }; inc.extra = d.extra || null;
  if (d.rig) inc.rig = JSON.parse(JSON.stringify(d.rig)); if (d.extraAttrs) inc.extraAttrs = { ...d.extraAttrs }; if (d.jo) inc.jo = d.jo.slice(); // rigging
  updateXform(o); applyVisibility(o);
  if (inc.kind === 'mesh') rebuildShape(o);
  return o;
}
export function serializeScene() {
  const nodes = []; App.world.traverse(o => { if (o.inca) nodes.push(serializeNode(o)); });
  return {
    format: 'inca-scene', version: App.version, idCounter: App.idCounter,
    nodes, shading: serializeShading(), layers: App.layers.map(l => ({ ...l })),
    anim: App.anim ? App.anim.serialize() : null,
    time: { ...App.time, playing: false },
    render: App.renderSettings || null,
    sel: App.sel.map(o => o.inca.id),
    project: App.project,
  };
}
export function clearScene() {
  for (const o of [...dagChildren(App.world)]) { o.inca.startup = false; deleteNode(o); }
  App.nodes.clear(); App.names.clear(); App.sel = []; App.hilite = []; App.compMode = null; App.layers = [];
  if (App.anim) App.anim.clear();
}
export function loadScene(data, { keepCameras = false } = {}) {
  clearScene();
  App.idCounter = Math.max(App.idCounter, data.idCounter || 1);
  if (data.shading) loadShading(data.shading); else initDefaultMaterials();
  App.layers = (data.layers || []).map(l => ({ ...l }));
  const byId = new Map();
  for (const d of data.nodes) { const parent = d.parent ? byId.get(d.parent) : null; const o = deserializeNode(d, parent); byId.set(d.id, o); }
  if (!findNode('persp')) createStartupCameras();
  for (const o of byId.values()) if (o.inca.kind === 'mesh') rebuildShape(o);
  if (App.anim && data.anim) App.anim.load(data.anim);
  if (data.time) Object.assign(App.time, data.time, { playing: false });
  if (data.render) App.renderSettings = { ...App.renderSettings, ...data.render };
  App.sel = (data.sel || []).map(id => App.nodes.get(id)).filter(Boolean);
  App.emit('sceneLoaded');
  App.dirty('outliner', 'channels', 'attr', 'layers', 'timeline', 'title', 'hypershade');
}

// duplicate objects (fresh ids/names), keeping hierarchy below each
export function duplicateNodes(list, { keepHistory = false } = {}) {
  const out = [];
  for (const o of list) {
    const datas = []; o.traverse(c => { if (c.inca) datas.push(serializeNode(c)); });
    const map = new Map();
    let rootCopy = null;
    for (const d of datas) {
      const parent = d.id === o.inca.id ? dagParent(o) : map.get(d.parent);
      if (!keepHistory && d.kind === 'mesh') { d.history = [{ id: 'x', name: 'meshData#', type: 'meshData', params: { data: d.mesh } }]; }
      const c = deserializeNode(d, parent, { fresh: true });
      map.set(d.id, c); if (!rootCopy) rootCopy = c;
    }
    out.push(rootCopy);
  }
  return out;
}
export function combine(list) {
  const meshes = []; list.forEach(o => o.traverse(c => { if (c.inca && c.inca.kind === 'mesh') meshes.push(c); }));
  if (meshes.length < 2) return null;
  const pm = combineMeshes(meshes.map(m => ({ mesh: m.inca.mesh, matrix: worldMatrix(m), mat: m.inca.material })));
  const mat = meshes[meshes.length - 1].inca.material;
  // per-face materials where they differ from the result's object material
  if (pm.fm) pm.fm = pm.fm.map(m => m === mat ? null : m);
  const o = createMesh(null, {}, { name: 'polySurface1', mesh: pm, material: mat });
  bakeMesh(o, pm);
  const wp = new THREE.Box3(); for (const m of meshes) wp.union(worldBBox(m));
  setPivotLocal(o, wp.getCenter(new THREE.Vector3()).toArray());
  for (const m of meshes) { let top = m; deleteNode(m); }
  for (const g of list) if (g.parent && g.inca && g.inca.kind === 'group' && !dagChildren(g).length) deleteNode(g);
  return o;
}
export function separate(o) {
  const groups = components(o.inca.mesh);
  if (groups.length < 2) return [o];
  const out = []; const parentGrp = createGroup(o.inca.name, { parent: dagParent(o) });
  const wm = worldMatrix(o);
  groups.forEach((fs) => {
    const { extracted } = opExtractFaces(o.inca.mesh, fs);
    const n = createMesh(null, {}, { name: 'polySurface1', mesh: extracted, material: o.inca.material, parent: parentGrp });
    bakeMesh(n, extracted); setWorldMatrix(n, wm); centerPivot(n); out.push(n);
  });
  deleteNode(o);
  return out;
}
export function extractFaces(o, faces, { duplicate = false } = {}) {
  const { keep, extracted } = opExtractFaces(o.inca.mesh, faces);
  const n = createMesh(null, {}, { name: 'polySurface1', mesh: extracted, material: o.inca.material, parent: dagParent(o) });
  bakeMesh(n, extracted); setLocalMatrix(n, o.matrix.clone()); centerPivot(n);
  if (!duplicate) { applyOp(o, 'deleteComponent', { faces: faces.slice() }); }
  return n;
}
export { createStartupCameras as _cams };
