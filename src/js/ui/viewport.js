// Inca — model panels (viewports)
import * as THREE from 'three';
import { TransformControls } from 'three/addons/controls/TransformControls.js';
import { computeBoundsTree, disposeBoundsTree, acceleratedRaycast } from 'three-mesh-bvh';
import { App } from '../core/app.js';
import { h, menuBar, iconBtn, fmt, showMenu } from './dom.js';
import { syncCamera, updateXform, findNode, worldBBox, allDag, WIRE_MATS, COLORS, isDag } from '../core/scene.js';
import { Sel } from '../core/selection.js';
import { Manip, placeProxy, manipVisible, beginDrag, applyDrag, endDrag, softWeights } from '../core/manip.js';

THREE.BufferGeometry.prototype.computeBoundsTree = computeBoundsTree;
THREE.BufferGeometry.prototype.disposeBoundsTree = disposeBoundsTree;
THREE.Mesh.prototype.raycast = acceleratedRaycast;

const D2R = Math.PI / 180;
const BG = { 0: [0x6b6b6b, null], 1: [0x000000, null], 2: [0x3a3a3a, null], 3: [0xa0a0a0, null], 4: [0x6b6b6b, [0x8a8f96, 0x3d3f42]] };

// shared overlays: component display colours
const COMP = { vert: new THREE.Color(0xc23ad6), vertSel: new THREE.Color(0xffe600), edge: new THREE.Color(COLORS.hilite), edgeSel: new THREE.Color(0xff8a1e), hover: new THREE.Color(0xff2a2a), cv: new THREE.Color(0xd16ff0), dot: new THREE.Color(0x4a8ad6) };
const pointsMat = new THREE.PointsMaterial({ size: 6, sizeAttenuation: false, vertexColors: true, depthTest: true });
const pointsMatX = new THREE.PointsMaterial({ size: 6, sizeAttenuation: false, vertexColors: true, depthTest: false, transparent: true });
const edgeMat = new THREE.LineBasicMaterial({ vertexColors: true, depthTest: true });
const faceSelMat = new THREE.MeshBasicMaterial({ color: 0xff8a1e, transparent: true, opacity: 0.45, depthTest: true, side: THREE.DoubleSide, polygonOffset: true, polygonOffsetFactor: -1, polygonOffsetUnits: -1 });
const faceHoverMat = new THREE.MeshBasicMaterial({ color: 0xff2a2a, transparent: true, opacity: 0.35, depthTest: true, side: THREE.DoubleSide, polygonOffset: true, polygonOffsetFactor: -1, polygonOffsetUnits: -1 });
const hoverLineMat = new THREE.LineBasicMaterial({ color: 0xff2a2a, depthTest: false });
const toolLineMat = new THREE.LineDashedMaterial({ color: 0xffe14a, dashSize: 0.08, gapSize: 0.05, depthTest: false });
const toolPtMat = new THREE.PointsMaterial({ size: 8, sizeAttenuation: false, color: 0x5cff5c, depthTest: false });

function makeGrid(axis = 'y', size = 12, div = 12, major = 5) {
  const g = new THREE.Group();
  const sub = [], maj = [], ax = [];
  for (let i = -div; i <= div; i++) {
    const t = i * size / div; const arr = i === 0 ? ax : (i % major === 0 ? maj : sub);
    arr.push(t, -size, t, size, -size, t, size, t);
  }
  const mk = (arr, color, lw) => {
    const pos = [];
    for (let i = 0; i < arr.length; i += 4) {
      const [a, b, c, d] = [arr[i], arr[i + 1], arr[i + 2], arr[i + 3]];
      if (axis === 'y') pos.push(a, 0, b, c, 0, d); else if (axis === 'z') pos.push(a, b, 0, c, d, 0); else pos.push(0, b, a, 0, d, c);
    }
    const geo = new THREE.BufferGeometry(); geo.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
    const l = new THREE.LineSegments(geo, new THREE.LineBasicMaterial({ color, depthWrite: false })); return l;
  };
  g.add(mk(sub, 0x5c5c5c), mk(maj, 0x4d4d4d), mk(ax, 0x1e1e1e));
  return g;
}

let defaultLight = null, defaultAmb = null;
export function ensureDefaultLights() {
  if (defaultLight) return;
  defaultLight = new THREE.DirectionalLight(0xffffff, Math.PI); defaultLight.userData.helper = true; defaultLight.name = 'incaDefaultLight';
  defaultAmb = new THREE.AmbientLight(0xffffff, 0); defaultAmb.userData.helper = true;
  App.scene.add(defaultLight, defaultLight.target, defaultAmb);
}

export class Viewport {
  constructor(camName, host) {
    this.camName = camName;
    this.opts = { shading: 'smooth', wireOnShaded: false, textured: false, lighting: 'default', shadows: false, xray: false, grid: true, isolate: null, resGate: false, filmGate: false, exposure: 0, smoothWire: false, ao: false,
      show: { polymeshes: true, nurbsSurfaces: true, nurbsCurves: true, lights: true, cameras: true, locators: true, grid: true, manipulators: true, hud: true, selectionHighlight: true } };
    this.needsRender = true;
    this.el = h('div', { class: 'panel' });
    this.menubar = h('div', { class: 'panel-menubar' });
    this.toolbar = h('div', { class: 'panel-toolbar' });
    this.view = h('div', { class: 'panel-view', tabindex: 0 });
    this.canvas = h('canvas');
    this.label = h('div', { class: 'panel-label' });
    this.hud = h('div', { class: 'panel-hud' });
    this.hudR = h('div', { class: 'panel-hud-right' });
    this.msg = h('div', { class: 'panel-msg' });
    this.gate = h('div', { style: { position: 'absolute', pointerEvents: 'none', inset: 0 } });
    this.view.append(this.canvas, this.gate, this.label, this.hud, this.hudR, this.msg);
    this.el.append(this.menubar, this.toolbar, this.view);
    host.append(this.el);
    this.renderer = new THREE.WebGLRenderer({ canvas: this.canvas, antialias: true, preserveDrawingBuffer: true, powerPreference: 'high-performance' });
    this.renderer.setPixelRatio(Math.min(2, devicePixelRatio || 1));
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.renderer.shadowMap.type = THREE.PCFShadowMap;
    this.renderer.autoClear = false;
    this.gridScene = new THREE.Scene();
    this.gizmoScene = new THREE.Scene();
    this.axisScene = new THREE.Scene(); this.axisCam = new THREE.OrthographicCamera(-1.4, 1.4, 1.4, -1.4, -10, 10); this.buildAxis();
    this.toolScene = new THREE.Scene();
    this.tc = new TransformControls(new THREE.PerspectiveCamera(), this.canvas);
    this.tc.attach(Manip.proxy);
    this.tc.setSize(App.prefs?.manipSize ?? 1);
    this.gizmoScene.add(this.tc.getHelper());
    this.gizmoScene.add(Manip.proxy.parent ? new THREE.Object3D() : Manip.proxy);
    this.tc.addEventListener('mouseDown', () => { this._tcDown = true; beginDrag(); });
    this.tc.addEventListener('objectChange', () => { this.snapDuringDrag(); applyDrag(); App.requestRender(); });
    this.tc.addEventListener('mouseUp', () => { this._tcDown = false; endDrag(); placeProxy(); App.requestRender(); });
    this.tc.addEventListener('change', () => App.requestRender());
    this.buildMenus();
    this.buildToolbar();
    this.bindEvents();
    this.hover = null;
    this.toolState = null;
    this._ro = new ResizeObserver(() => { this.needsRender = true; }); this._ro.observe(this.view);
  }
  get camNode() { return findNode(this.camName) || findNode('persp'); }
  get camera() { return this.camNode.userData.camera; }
  get isOrtho() { return !!this.camNode.inca.cam.ortho; }
  setCamera(name) { this.camName = name; this.needsRender = true; this.updateLabel(); App.emit('panelCameraChanged', this); }
  updateLabel() {
    const n = this.camNode; this.label.textContent = n ? n.inca.name : '';
    const vis = App.prefs?.hud?.camNames !== false; this.label.style.display = vis ? '' : 'none';
  }
  buildAxis() {
    const mk = (dir, color) => { const g = new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(), dir]); return new THREE.Line(g, new THREE.LineBasicMaterial({ color, depthTest: false })); };
    this.axisScene.add(mk(new THREE.Vector3(1, 0, 0), 0xe0393e), mk(new THREE.Vector3(0, 1, 0), 0x5cd65c), mk(new THREE.Vector3(0, 0, 1), 0x4a84ff));
  }
  // ---------------------------------------------------------------- render
  resize() {
    const w = this.view.clientWidth, hh = this.view.clientHeight;
    if (!w || !hh) return false;
    if (this._w !== w || this._h !== hh) { this.renderer.setSize(w, hh, false); this._w = w; this._h = hh; }
    return true;
  }
  render() {
    if (!this.el.isConnected || this.el.offsetParent === null) return;
    if (!this.resize()) return;
    this.needsRender = false;
    const camNode = this.camNode; if (!camNode) return;
    const cam = this.camera; const aspect = this._w / this._h;
    syncCamera(camNode, aspect); camNode.updateWorldMatrix(true, true);
    const r = this.renderer; const o = this.opts;
    // background
    const bgm = BG[App.prefs?.bgMode ?? 0] || BG[0];
    r.setClearColor(bgm[0], 1);
    this.view.style.background = bgm[1] ? `linear-gradient(${'#' + bgm[1][0].toString(16).padStart(6, '0')}, ${'#' + bgm[1][1].toString(16).padStart(6, '0')})` : '';
    r.setClearAlpha(bgm[1] ? 0 : 1);
    r.toneMapping = o.exposure ? THREE.LinearToneMapping : THREE.NoToneMapping; r.toneMappingExposure = Math.pow(2, o.exposure);
    r.shadowMap.enabled = o.shadows && o.lighting === 'all';
    this.prepareScene();
    r.clear();
    // grid behind everything
    if (o.grid && o.show.grid) {
      if (!this.grid || this.gridAxis !== this.gridOrient()) { if (this.grid) this.gridScene.remove(this.grid); this.gridAxis = this.gridOrient(); const gp = App.prefs?.grid || {}; this.grid = makeGrid(this.gridAxis, gp.size ?? 12, gp.divisions ?? 12, gp.major ?? 5); this.gridScene.add(this.grid); }
      r.render(this.gridScene, cam);
    }
    r.render(App.scene, cam);
    this.restoreScene();
    if (this.toolScene.children.length) { r.clearDepth(); r.render(this.toolScene, cam); }
    // manipulator
    const showManip = o.show.manipulators && manipVisible() && (App.activeViewport === this || !App.prefs?.manipActiveOnly);
    this.tc.camera = cam;
    if (showManip) { this.tc.enabled = true; this.tc.getHelper().visible = true; this.tc.setMode(Manip.pivotMode ? 'translate' : Manip.mode); const gs = App.prefs?.grid?.spacing ?? 1; const stp = Manip.step || {}; this.tc.setTranslationSnap((Manip.snapGrid || Manip._keyGrid) ? gs : (stp.translate || null)); this.tc.setRotationSnap(Manip.stepRotate ? 15 * D2R : (stp.rotate ? stp.rotate * D2R : null)); this.tc.setScaleSnap(stp.scale || null); this.tc.setSpace(Manip.pivotMode ? 'world' : (Manip.mode === 'scale' ? 'local' : Manip.space[Manip.mode] || 'world')); r.clearDepth(); r.render(this.gizmoScene, cam); }
    else { this.tc.enabled = false; this.tc.getHelper().visible = false; }
    // view axis triad
    if (App.prefs?.hud?.viewAxis !== false) {
      const s = 70 * r.getPixelRatio(); r.setViewport(0, 0, 70, 70); r.setScissor(0, 0, 70, 70); r.setScissorTest(true);
      this.axisCam.quaternion.copy(cam.getWorldQuaternion(new THREE.Quaternion())); this.axisCam.position.set(0, 0, 0).add(new THREE.Vector3(0, 0, 5).applyQuaternion(this.axisCam.quaternion));
      this.axisCam.updateMatrixWorld(); r.clearDepth(); r.render(this.axisScene, this.axisCam);
      r.setScissorTest(false); r.setViewport(0, 0, this._w, this._h);
    }
    this.drawGate(); this.drawHud();
  }
  gridOrient() { const n = this.camNode.inca; if (!n.cam.ortho) return 'y'; if (n.name === 'front' || (Math.abs(n.r[0]) < 1 && Math.abs(n.r[1]) < 1)) return 'z'; if (n.name === 'side' || Math.abs(Math.abs(n.r[1]) - 90) < 1) return 'x'; return 'y'; }
  prepareScene() {
    const o = this.opts; const cam = this.camera; const iso = o.isolate;
    App.rig?.beforeRender?.(); // rigging: evaluate constraints / IK / deformers before drawing
    ensureDefaultLights();
    // default light rides with the camera
    const cp = new THREE.Vector3(), cq = new THREE.Quaternion(); cam.getWorldPosition(cp); cam.getWorldQuaternion(cq);
    defaultLight.position.copy(cp).add(new THREE.Vector3(-0.15, 0.2, 0).applyQuaternion(cq).multiplyScalar(1));
    defaultLight.target.position.copy(cp).add(new THREE.Vector3(0, 0, -1).applyQuaternion(cq));
    defaultLight.target.updateMatrixWorld(); defaultLight.updateMatrixWorld();
    const useAll = o.lighting === 'all', none = o.lighting === 'none', flat = o.lighting === 'flat';
    defaultLight.intensity = (o.lighting === 'default') ? Math.PI : 0;
    defaultAmb.intensity = flat ? Math.PI : 0;
    this._saved = [];
    const hidden = [];
    let anySky = null;
    App.world.traverse((n) => {
      if (!n.inca) return;
      const inc = n.inca; const ud = n.userData;
      let vis = n.visible;
      if (iso && !isoContains(iso, n)) { hidden.push(n); n.visible = false; return; }
      if (inc.startup) { if (ud.icon) ud.icon.visible = false; return; }
      const sel = App.sel.includes(n); const lead = sel && App.sel[App.sel.length - 1] === n; const hl = App.hilite.includes(n);
      const lmode = inc.layer ? App.layers.find(l => l.id === inc.layer) : null; const ltemp = lmode && lmode.mode === 'template';
      const wireMat = (inc.template || ltemp) ? WIRE_MATS.template : (lead && o.show.selectionHighlight) ? WIRE_MATS.lead : (sel && o.show.selectionHighlight) ? WIRE_MATS.sel : hl ? WIRE_MATS.hilite : null;
      if (inc.kind === 'mesh') {
        const show = inc.nurbs ? o.show.nurbsSurfaces : o.show.polymeshes;
        ud.mesh.visible = show && o.shading !== 'wire' && o.shading !== 'bbox';
        ud.mesh.material = o.textured ? ud.mesh.userData.matsT : ud.mesh.userData.matsU;
        ud.wire.visible = show && (o.shading === 'wire' || o.wireOnShaded || !!wireMat || inc.template);
        if (ltemp) ud.mesh.visible = false;
        ud.wire.material = wireMat || WIRE_MATS.wire;
        if (ud.comp) ud.comp.visible = show && hl;
        if (o.xray || inc.xray) for (const m of ud.mesh.material) { this._saved.push([m, m.transparent, m.opacity, m.depthWrite]); m.transparent = true; m.opacity = Math.min(m.opacity, 0.5); m.depthWrite = false; }
      } else if (inc.kind === 'light') {
        ud.icon.visible = o.show.lights; ud.icon.material = wireMat || WIRE_MATS.icon;
        const L = ud.light; const real = inc.light.type === 'skyDomeLight' ? 0 : (inc.light.intensity * Math.PI);
        L.intensity = (useAll && n.visible) ? real : 0;
        if (inc.light.type === 'skyDomeLight' && useAll && n.visible) anySky = n;
      } else if (inc.kind === 'camera') {
        if (ud.icon) { ud.icon.visible = o.show.cameras && n !== this.camNode; ud.icon.material = wireMat || WIRE_MATS.icon; }
      } else if (inc.kind === 'curve') {
        ud.line.visible = o.show.nurbsCurves; ud.line.material = wireMat || (inc.template ? WIRE_MATS.template : WIRE_MATS.curve);
        if (ud.comp) ud.comp.visible = hl;
      } else if (inc.kind === 'locator') {
        ud.icon.visible = o.show.locators; ud.icon.material = wireMat || WIRE_MATS.locator;
      } else if (App.rig?.prepareNode) App.rig.prepareNode(n, wireMat, o, this); // rigging: joints, handles, lattices
    });
    this._hidden = hidden;
    // environment / sky
    const sky = anySky ? App.skyEnv?.(anySky) : null;
    this._envSaved = [App.scene.environment, App.scene.background];
    App.scene.environment = sky ? sky.env : null;
    App.scene.background = (sky && sky.bg && anySky.inca.light.showBackground) ? sky.bg : null;
    if (sky) App.scene.environmentIntensity = anySky.inca.light.intensity;
    App.updateComponentOverlays?.();
  }
  restoreScene() {
    for (const [m, t, op, dw] of this._saved) { m.transparent = t; m.opacity = op; m.depthWrite = dw; }
    for (const n of this._hidden) n.visible = true;
    App.scene.environment = this._envSaved[0]; App.scene.background = this._envSaved[1];
  }
  drawGate() {
    const o = this.opts; this.gate.innerHTML = '';
    if (!(o.resGate || o.filmGate)) return;
    const rs = App.renderSettings || { width: 1920, height: 1080 };
    const asp = o.resGate ? rs.width / rs.height : this.camNode.inca.cam.horizontalFilmAperture / this.camNode.inca.cam.verticalFilmAperture;
    const W = this._w, H = this._h; let gw = W * 0.9, gh = gw / asp; if (gh > H * 0.9) { gh = H * 0.9; gw = gh * asp; }
    const x = (W - gw) / 2, y = (H - gh) / 2;
    const mask = 'rgba(0,0,0,0.35)';
    this.gate.append(h('div', { style: { position: 'absolute', left: 0, top: 0, right: 0, height: y + 'px', background: mask } }), h('div', { style: { position: 'absolute', left: 0, bottom: 0, right: 0, height: y + 'px', background: mask } }),
      h('div', { style: { position: 'absolute', left: 0, top: y + 'px', width: x + 'px', height: gh + 'px', background: mask } }), h('div', { style: { position: 'absolute', right: 0, top: y + 'px', width: x + 'px', height: gh + 'px', background: mask } }),
      h('div', { style: { position: 'absolute', left: x + 'px', top: y + 'px', width: gw + 'px', height: gh + 'px', border: '1px solid ' + (o.resGate ? '#ddd' : '#e0b030') } }),
      h('div', { style: { position: 'absolute', left: (x + 4) + 'px', top: (y - 16) + 'px', fontSize: '11px', color: '#eee' }, text: o.resGate ? `${rs.width} x ${rs.height}` : 'Film Gate' }));
  }
  drawHud() {
    const hp = App.prefs?.hud || {};
    let s = '';
    if (hp.polyCount) {
      let tot = { verts: 0, edges: 0, faces: 0, tris: 0, uvs: 0 }, sel = { verts: 0, edges: 0, faces: 0, tris: 0, uvs: 0 };
      for (const n of allDag()) if (n.inca.kind === 'mesh' && n.visible) { const st = n.inca.mesh.stats(); for (const k in tot) tot[k] += st[k]; if (App.sel.includes(n) || App.hilite.includes(n)) for (const k in sel) sel[k] += st[k]; }
      let comp = { verts: 0, edges: 0, faces: 0 };
      for (const n of App.hilite) if (n.inca.compSel) { comp.verts += n.inca.compSel.v.size; comp.edges += n.inca.compSel.e.size; comp.faces += n.inca.compSel.f.size; }
      const row = (l, k, c) => `${l.padEnd(7)}${String(tot[k]).padStart(9)}${String(sel[k]).padStart(9)}${String(c ?? 0).padStart(9)}\n`;
      s += row('Verts:', 'verts', comp.verts) + row('Edges:', 'edges', comp.edges) + row('Faces:', 'faces', comp.faces) + row('Tris:', 'tris', 0) + row('UVs:', 'uvs', 0);
    }
    this.hud.textContent = s;
    let r = '';
    if (hp.frame) r += `Current Frame: ${fmt(App.time.current, 2)}\n`;
    if (hp.fps) r += `${(App.fpsMeasured || 0).toFixed(1)} fps\n`;
    this.hudR.textContent = r;
  }
  flash(text) { this.msg.textContent = text; this.msg.style.opacity = '1'; clearTimeout(this._mt); this._mt = setTimeout(() => this.msg.style.opacity = '0', 900); }

  // ---------------------------------------------------------------- camera navigation
  camBasis() {
    const c = this.camNode; c.updateWorldMatrix(true, false);
    const m = c.matrixWorld; const right = new THREE.Vector3().setFromMatrixColumn(m, 0).normalize(), up = new THREE.Vector3().setFromMatrixColumn(m, 1).normalize(), back = new THREE.Vector3().setFromMatrixColumn(m, 2).normalize();
    const pos = new THREE.Vector3().setFromMatrixPosition(m);
    return { right, up, back, pos };
  }
  coiPoint() { const { back, pos } = this.camBasis(); return pos.clone().addScaledVector(back, -this.camNode.inca.cam.coi); }
  tumble(dx, dy) {
    const c = this.camNode; const inc = c.inca;
    if (inc.cam.ortho && !App.prefs?.orthoTumble) return;
    const target = this.coiPoint();
    inc.r[1] -= dx * 0.4; inc.r[0] -= dy * 0.4;
    updateXform(c);
    const { back } = this.camBasis();
    const p = target.clone().addScaledVector(back, inc.cam.coi);
    inc.t = [p.x, p.y, p.z]; updateXform(c);
    if (inc.cam.ortho && inc.startup) inc._tumbled = true;
    this.needsRender = true;
  }
  track(dx, dy) {
    const c = this.camNode; const inc = c.inca; const { right, up, pos } = this.camBasis();
    let k;
    if (inc.cam.ortho) k = inc.cam.orthoWidth / this._w;
    else { const cam = this.camera; k = 2 * inc.cam.coi * Math.tan(cam.fov * D2R / 2) / this._h; }
    const p = pos.clone().addScaledVector(right, -dx * k).addScaledVector(up, dy * k);
    inc.t = [p.x, p.y, p.z]; updateXform(c); this.needsRender = true;
  }
  dolly(d, cursor = null) {
    const c = this.camNode; const inc = c.inca;
    const f = Math.exp(-d * 0.006);
    if (inc.cam.ortho) {
      const before = cursor ? this.worldAtCursorOrtho(cursor) : null;
      inc.cam.orthoWidth = Math.max(0.01, inc.cam.orthoWidth * f);
      if (before) { syncCamera(c, this._w / this._h); const after = this.worldAtCursorOrtho(cursor); const dlt = before.sub(after); inc.t = [inc.t[0] + dlt.x, inc.t[1] + dlt.y, inc.t[2] + dlt.z]; updateXform(c); }
    } else {
      const target = this.coiPoint();
      const nc = Math.max(0.01, inc.cam.coi * f);
      const { back } = this.camBasis();
      const p = target.clone().addScaledVector(back, nc);
      inc.cam.coi = nc; inc.t = [p.x, p.y, p.z]; updateXform(c);
    }
    this.needsRender = true;
  }
  worldAtCursorOrtho(px) { const nd = this.ndc(px); const v = new THREE.Vector3(nd.x, nd.y, 0).unproject(this.camera); return v; }
  frame(box, all = false) {
    if (!box || box.isEmpty()) { box = new THREE.Box3(new THREE.Vector3(-6, -6, -6), new THREE.Vector3(6, 6, 6)); }
    const c = this.camNode; const inc = c.inca;
    const center = box.getCenter(new THREE.Vector3()); let radius = box.getSize(new THREE.Vector3()).length() / 2; if (radius < 0.01) radius = 1;
    const { back } = this.camBasis();
    if (inc.cam.ortho) { inc.cam.orthoWidth = radius * 2.4 * Math.max(1, this._h ? this._w / this._h / 1.5 : 1); const p = center.clone().addScaledVector(back, 1000); inc.t = [p.x, p.y, p.z]; inc.cam.coi = 1000; }
    else {
      const cam = this.camera; const vf = cam.fov * D2R; const hf = 2 * Math.atan(Math.tan(vf / 2) * cam.aspect);
      const dist = radius / Math.sin(Math.min(vf, hf) / 2) * 1.05;
      const p = center.clone().addScaledVector(back, dist); inc.t = [p.x, p.y, p.z]; inc.cam.coi = dist;
    }
    updateXform(c); this.needsRender = true;
  }
  frameSelection() {
    const box = new THREE.Box3();
    if (App.compMode && Sel.anyComponents()) {
      const v = new THREE.Vector3();
      for (const o of App.hilite) { o.updateWorldMatrix(true, false); const pts = o.inca.kind === 'curve' ? o.inca.curve.cvs : o.inca.mesh.v; for (const i of Sel.affectedVerts(o)) box.expandByPoint(v.set(...pts[i]).applyMatrix4(o.matrixWorld)); }
    } else for (const o of App.sel) box.union(worldBBox(o));
    if (box.isEmpty()) return this.frameAll();
    this.frame(box);
  }
  frameAll() { const box = new THREE.Box3(); for (const o of allDag()) if (o.visible && !o.inca.startup && (o.inca.kind === 'mesh' || o.inca.kind === 'curve' || o.inca.kind === 'light' || o.inca.kind === 'locator' || o.inca.kind === 'camera')) box.union(worldBBox(o)); this.frame(box.isEmpty() ? null : box, true); }
  defaultView() {
    const c = this.camNode; const inc = c.inca;
    if (c.inca.name === 'persp' || !inc.cam.ortho) { inc.t = [28, 21, 28]; inc.r = [-27.938, 45, 0]; inc.cam.coi = 44.82; }
    else { const def = { top: [[0, 1000.1, 0], [-90, 0, 0]], front: [[0, 0, 1000.1], [0, 0, 0]], side: [[1000.1, 0, 0], [0, 90, 0]] }[inc.name]; if (def) { inc.t = def[0].slice(); inc.r = def[1].slice(); } inc.cam.orthoWidth = 30; }
    updateXform(c); this.needsRender = true;
  }
  lookAtSelection() {
    const box = new THREE.Box3(); for (const o of App.sel) box.union(worldBBox(o)); if (box.isEmpty()) return;
    const c = this.camNode; const target = box.getCenter(new THREE.Vector3()); const { pos } = this.camBasis();
    const dir = target.clone().sub(pos); const d = dir.length(); dir.normalize();
    const pitch = Math.asin(dir.y) / D2R; const yaw = Math.atan2(-dir.x, -dir.z) / D2R;
    c.inca.r = [pitch, yaw, 0]; c.inca.cam.coi = d; updateXform(c); this.needsRender = true;
  }

  // ---------------------------------------------------------------- picking helpers
  ndc(px) { return new THREE.Vector2((px.x / this._w) * 2 - 1, -(px.y / this._h) * 2 + 1); }
  localPx(e) { const r = this.view.getBoundingClientRect(); return { x: e.clientX - r.left, y: e.clientY - r.top }; }
  project(v) { const p = v.clone().project(this.camera); return { x: (p.x + 1) / 2 * this._w, y: (1 - p.y) / 2 * this._h, z: p.z }; }
  raycaster(px) { const rc = new THREE.Raycaster(); rc.setFromCamera(this.ndc(px), this.camera); rc.params.Line.threshold = this.worldPerPixel(px) * 6; rc.params.Points.threshold = this.worldPerPixel(px) * 6; return rc; }
  worldPerPixel() { const inc = this.camNode.inca; if (inc.cam.ortho) return inc.cam.orthoWidth / this._w; return 2 * inc.cam.coi * Math.tan(this.camera.fov * D2R / 2) / this._h; }
  visibleNodes() {
    const iso = this.opts.isolate; const out = [];
    App.world.traverse(n => { if (!n.inca || n.inca.startup) return; let v = true; let p = n; while (p && p.inca) { if (!p.visible) { v = false; break; } p = p.parent; } if (!v) return; if (iso && !isoContains(iso, n)) return; if (n.inca.template || layerUnselectable(n)) return; if (App.selMask && App.selMask[n.inca.kind === 'group' ? 'group' : n.inca.kind] === false) return; out.push(n); });
    return out;
  }
  pickObjects(px) {
    const rc = this.raycaster(px); const hits = [];
    const o = this.opts;
    for (const n of this.visibleNodes()) {
      const ud = n.userData; const inc = n.inca;
      if (inc.kind === 'mesh') {
        if (!(inc.nurbs ? o.show.nurbsSurfaces : o.show.polymeshes)) continue;
        const hit = this.rayMesh(rc, n); if (hit) hits.push({ o: n, d: hit.distance, hit });
        else if (o.shading === 'wire') { const lh = rc.intersectObject(ud.wire, false)[0]; if (lh) hits.push({ o: n, d: lh.distance }); }
      } else if (inc.kind === 'curve') { if (!o.show.nurbsCurves) continue; const lh = rc.intersectObject(ud.line, false)[0]; if (lh) hits.push({ o: n, d: lh.distance - 0.001 }); }
      else if (ud.pick) {
        if ((inc.kind === 'light' && !o.show.lights) || (inc.kind === 'camera' && !o.show.cameras) || (inc.kind === 'locator' && !o.show.locators)) continue;
        const ph = rc.intersectObject(ud.pick, false)[0]; if (ph) hits.push({ o: n, d: ph.distance - (ud.pickBias || 0) }); // rigging: pickBias (joints win over surfaces)
      }
    }
    hits.sort((a, b) => a.d - b.d);
    return hits;
  }
  rayMesh(rc, n) {
    const ud = n.userData; let mesh = ud.mesh; let triFace = mesh.userData.triFace;
    if (mesh.userData.pick) { if (!this._pm) this._pm = new THREE.Mesh(); this._pm.geometry = mesh.userData.pick.geometry; this._pm.matrixWorld.copy(mesh.matrixWorld); this._pm.material = new THREE.MeshBasicMaterial({ side: THREE.DoubleSide }); mesh = this._pm; triFace = ud.mesh.userData.pick.triFace; }
    const g = mesh.geometry; if (!g.boundsTree && g.attributes.position && g.attributes.position.count > 300) g.computeBoundsTree();
    const prevSide = Array.isArray(mesh.material) ? null : mesh.material.side;
    const hits = rc.intersectObject(mesh, false);
    const hit = hits[0]; if (!hit) return null;
    hit.baseFace = triFace ? triFace[Math.floor(hit.faceIndex)] : -1;
    if (hit.face && g.index == null) hit.baseFace = triFace[Math.floor(hit.faceIndex)];
    return hit;
  }
  occluded(worldPt, owner) {
    if (this.opts.xray || this.opts.shading === 'wire' || owner?.inca?.kind === 'curve') return false;
    const cp = new THREE.Vector3(); this.camera.getWorldPosition(cp);
    const dir = this.isOrtho ? new THREE.Vector3(0, 0, -1).applyQuaternion(this.camera.getWorldQuaternion(new THREE.Quaternion())) : worldPt.clone().sub(cp).normalize();
    const origin = this.isOrtho ? worldPt.clone().addScaledVector(dir, -2000) : cp;
    const dist = worldPt.distanceTo(origin);
    const rc = new THREE.Raycaster(origin, dir, 0, dist);
    for (const n of this.visibleNodes()) {
      if (n.inca.kind !== 'mesh') continue;
      const hit = this.rayMesh(rc, n);
      if (hit && hit.distance < dist - Math.max(1e-3, dist * 2e-4) * 4) return true;
    }
    return false;
  }
  // component picking: returns {o, id, kind, t?}
  pickComponent(px, kind = App.compMode, { forTool = false } = {}) {
    const best = { d: Infinity };
    const objs = forTool ? App.hilite.concat(App.sel).filter((v, i, a) => a.indexOf(v) === i && v.inca.kind === 'mesh') : App.hilite;
    for (const o of objs) {
      if (!o.visible) continue;
      o.updateWorldMatrix(true, false); const M = o.matrixWorld;
      const pts = o.inca.kind === 'curve' ? o.inca.curve.cvs : o.inca.mesh.v;
      const W = pts.map(p => new THREE.Vector3(p[0], p[1], p[2]).applyMatrix4(M));
      const S = W.map(w => this.project(w));
      if (kind === 'vertex' || kind === 'uv' || o.inca.kind === 'curve') {
        S.forEach((s, i) => { if (s.z > 1 || s.z < -1) return; const d = Math.hypot(s.x - px.x, s.y - px.y); if (d < 12 && (d < best.d - 0.5 || (Math.abs(d - best.d) < 0.5 && s.z < best.z))) { if (!this.occluded(W[i], o)) Object.assign(best, { d, o, id: i, z: s.z, kind: 'vertex' }); } });
      } else if (kind === 'edge') {
        const E = o.inca.mesh.topo.edges;
        E.forEach(([a, b], i) => {
          const A = S[a], B = S[b]; if (A.z > 1 || B.z > 1) return;
          const dx = B.x - A.x, dy = B.y - A.y; const L2 = dx * dx + dy * dy || 1;
          let t = ((px.x - A.x) * dx + (px.y - A.y) * dy) / L2; t = Math.max(0, Math.min(1, t));
          const d = Math.hypot(A.x + dx * t - px.x, A.y + dy * t - px.y);
          if (d < 9 && d < best.d) { const wp = W[a].clone().lerp(W[b], t); if (!this.occluded(wp, o)) Object.assign(best, { d, o, id: i, t, kind: 'edge' }); }
        });
      } else if (kind === 'face') {
        const rc = this.raycaster(px); const hit = this.rayMesh(rc, o);
        if (hit && hit.distance < best.d) Object.assign(best, { d: hit.distance, o, id: hit.baseFace, kind: 'face' });
        if (!hit && (this.opts.shading === 'wire' || this.opts.xray)) {
          // pick by face centre in wireframe
          o.inca.mesh.f.forEach((f, fi) => { const c = new THREE.Vector3(...o.inca.mesh.faceCenter(fi)).applyMatrix4(M); const s = this.project(c); const d = Math.hypot(s.x - px.x, s.y - px.y); if (d < 10 && d + 1e5 < best.d + 1e5 && d < (best.dd ?? 1e9)) Object.assign(best, { dd: d, d: 0, o, id: fi, kind: 'face' }); });
        }
      }
    }
    return best.o ? best : null;
  }
  marqueeComponents(r) {
    const out = [];
    for (const o of App.hilite) {
      if (!o.visible) continue;
      o.updateWorldMatrix(true, false); const M = o.matrixWorld;
      const pts = o.inca.kind === 'curve' ? o.inca.curve.cvs : o.inca.mesh.v;
      const W = pts.map(p => new THREE.Vector3(p[0], p[1], p[2]).applyMatrix4(M));
      const S = W.map(w => this.project(w));
      const inside = (s) => s.z <= 1 && s.x >= r.x0 && s.x <= r.x1 && s.y >= r.y0 && s.y <= r.y1;
      const many = W.length > 4000;
      const ids = [];
      if (App.compMode === 'vertex' || App.compMode === 'uv' || o.inca.kind === 'curve') S.forEach((s, i) => { if (inside(s) && (many || !this.occluded(W[i], o))) ids.push(i); });
      else if (App.compMode === 'edge') o.inca.mesh.topo.edges.forEach(([a, b], i) => { const m = W[a].clone().lerp(W[b], 0.5); const sm = this.project(m); if ((inside(sm) || (inside(S[a]) && inside(S[b]))) && (many || !this.occluded(m, o))) ids.push(i); });
      else if (App.compMode === 'face') {
        const pm = o.inca.mesh;
        pm.f.forEach((f, fi) => {
          const c = new THREE.Vector3(...pm.faceCenter(fi)).applyMatrix4(M); const sc = this.project(c);
          if (!inside(sc)) return;
          // facing test for occlusion in shaded mode
          if (!(this.opts.xray || this.opts.shading === 'wire')) {
            const nrm = new THREE.Vector3(...pm.faceNormal(fi)).transformDirection(M);
            const cp = new THREE.Vector3(); this.camera.getWorldPosition(cp);
            const view = this.isOrtho ? new THREE.Vector3(0, 0, 1).applyQuaternion(this.camera.getWorldQuaternion(new THREE.Quaternion())) : cp.sub(c);
            if (nrm.dot(view) < 0) return;
            if (!many && this.occluded(c.clone().addScaledVector(nrm, 1e-3), o)) return;
          }
          ids.push(fi);
        });
      }
      out.push({ o, ids });
    }
    return out;
  }
  marqueeObjects(r) {
    const out = [];
    for (const n of this.visibleNodes()) {
      const inc = n.inca; n.updateWorldMatrix(true, false);
      let pts = [];
      if (inc.kind === 'mesh') { if (!(inc.nurbs ? this.opts.show.nurbsSurfaces : this.opts.show.polymeshes)) continue; pts = inc.mesh.v.length > 20000 ? inc.mesh.v.filter((_, i) => i % 7 === 0) : inc.mesh.v; }
      else if (inc.kind === 'curve') pts = inc.curve.sample(4);
      else if (n.userData.pick) pts = [[0, 0, 0]];
      else continue;
      const M = n.matrixWorld; const v = new THREE.Vector3();
      let hit = false;
      for (const p of pts) { const s = this.project(v.set(p[0], p[1], p[2]).applyMatrix4(M)); if (s.z <= 1 && s.x >= r.x0 && s.x <= r.x1 && s.y >= r.y0 && s.y <= r.y1) { hit = true; break; } }
      if (!hit && inc.kind === 'mesh') { const c = { x: (r.x0 + r.x1) / 2, y: (r.y0 + r.y1) / 2 }; const rc = this.raycaster(c); if (this.rayMesh(rc, n)) hit = true; }
      if (hit) out.push(n);
    }
    return out;
  }
  groundPoint(px) {
    const rc = this.raycaster(px);
    let plane;
    if (this.isOrtho) { const { back } = this.camBasis(); plane = new THREE.Plane(back.clone(), 0); }
    else plane = new THREE.Plane(new THREE.Vector3(0, 1, 0), 0);
    if (App.liveSurface) { const hit = this.rayMesh(rc, App.liveSurface); if (hit) return hit.point; }
    const p = new THREE.Vector3(); if (!rc.ray.intersectPlane(plane, p)) return null;
    if (Manip.snapGrid || Manip._keyGrid) { const s = App.prefs?.grid?.spacing ?? 1; p.set(Math.round(p.x / s) * s, Math.round(p.y / s) * s, Math.round(p.z / s) * s); }
    if (Manip._keyPoint) { const sp = this.snapPointNear(px); if (sp) p.copy(sp); }
    return p;
  }
  snapPointNear(px, exclude = []) {
    let best = null, bd = 18;
    for (const n of this.visibleNodes()) {
      if (exclude.includes(n)) continue;
      const pts = n.inca.kind === 'mesh' ? n.inca.mesh.v : n.inca.kind === 'curve' ? n.inca.curve.cvs : null; if (!pts) continue;
      n.updateWorldMatrix(true, false);
      for (const p of pts) { const w = new THREE.Vector3(...p).applyMatrix4(n.matrixWorld); const s = this.project(w); const d = Math.hypot(s.x - px.x, s.y - px.y); if (d < bd) { bd = d; best = w; } }
    }
    return best;
  }
  snapDuringDrag() {
    if (!Manip.dragging || Manip.mode !== 'translate') return;
    if ((Manip._keyPoint || Manip.snapPoint) && this._lastPx) {
      const excl = App.compMode ? [] : App.sel;
      const sp = this.snapPointNear(this._lastPx, excl);
      if (sp) {
        const axis = this.tc.axis;
        const p = Manip.proxy.position;
        if (axis === 'XYZ' || !axis) p.copy(sp);
        else { if (axis.includes('X')) p.x = sp.x; if (axis.includes('Y')) p.y = sp.y; if (axis.includes('Z')) p.z = sp.z; }
      }
    }
  }

  // ---------------------------------------------------------------- events
  bindEvents() {
    const v = this.view;
    v.addEventListener('contextmenu', e => e.preventDefault());
    v.addEventListener('mouseenter', () => { App.hoverViewport = this; });
    v.addEventListener('mousedown', (e) => this.onDown(e));
    v.addEventListener('mousemove', (e) => { this._lastPx = this.localPx(e); if (!this._drag) this.onHover(e); });
    v.addEventListener('mouseleave', () => { this.setHover(null); });
    v.addEventListener('wheel', (e) => { e.preventDefault(); this.dolly(-e.deltaY * 0.35, this.localPx(e)); }, { passive: false });
    v.addEventListener('dblclick', (e) => this.onDbl(e));
    this.canvas.addEventListener('pointerdown', () => { App.setActiveViewport(this); }, true);
  }
  onDown(e) {
    App.setActiveViewport(this);
    this.view.focus({ preventScroll: true });
    const px = this.localPx(e); this._lastPx = px;
    // camera navigation (Alt)
    if (e.altKey) {
      e.preventDefault();
      const mode = e.button === 0 ? (e.ctrlKey ? 'boxzoom' : 'tumble') : e.button === 1 ? 'track' : 'dolly';
      let lx = e.clientX, ly = e.clientY;
      const mm = (ev) => { const dx = ev.clientX - lx, dy = ev.clientY - ly; lx = ev.clientX; ly = ev.clientY; if (mode === 'tumble') this.tumble(dx, dy); else if (mode === 'track') this.track(dx, dy); else this.dolly((dx - dy) * 1.2); };
      const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu);
      return;
    }
    if (this._tcDown || (e.button === 0 && this.tc.enabled && this.tc.axis && !App.tools?.isInteractive?.(App.tool))) return; // manipulator handles it
    if (e.button === 2) { App.emit('markingMenu', { e, vp: this, px }); return; }
    if (e.button === 1) {
      // middle drag: drag manipulator from anywhere / virtual slider for selected channels
      if (App.channelSlider && App.channelSlider.active()) { App.channelSlider.drag(e); return; }
      if (manipVisible() && this.tc.enabled) { this.middleDrag(e); }
      return;
    }
    if (e.button !== 0) return;
    if (App.tools && App.tools.handleDown && App.tools.handleDown(this, e, px)) return;
    this.startSelectDrag(e, px);
  }
  middleDrag(e) {
    // Maya: MMB drag moves along the last-used axis (or view plane)
    const px0 = this.localPx(e);
    beginDrag();
    const start = Manip.proxy.position.clone(); const { right, up } = this.camBasis(); const k = this.worldPerPixel();
    const ax = this.tc.axis || this._lastAxis || 'XYZ';
    const mm = (ev) => {
      const px = this.localPx(ev); const dx = px.x - px0.x, dy = px.y - px0.y;
      const d = right.clone().multiplyScalar(dx * k).addScaledVector(up, -dy * k);
      if (Manip.mode === 'translate') {
        if (ax.length === 1) { const a = new THREE.Vector3(ax === 'X' ? 1 : 0, ax === 'Y' ? 1 : 0, ax === 'Z' ? 1 : 0); if (Manip.space.translate === 'local') a.applyQuaternion(Manip.proxy.quaternion); d.copy(a.multiplyScalar(d.dot(a))); }
        Manip.proxy.position.copy(start).add(d);
      } else if (Manip.mode === 'rotate') { Manip.proxy.quaternion.copy(Manip.start.quat).premultiply(new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 1, 0), dx * 0.01)); }
      else { const f = Math.exp(dx * 0.01); Manip.proxy.scale.set(f, f, f); }
      applyDrag(); App.requestRender();
    };
    const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); endDrag(); placeProxy(); App.requestRender(); };
    addEventListener('mousemove', mm); addEventListener('mouseup', mu);
  }
  startSelectDrag(e, px) {
    const shift = e.shiftKey, ctrl = e.ctrlKey || e.metaKey;
    const mode = shift && ctrl ? 'add' : shift ? 'toggle' : ctrl ? 'deselect' : 'replace';
    let marquee = null; const x0 = px.x, y0 = px.y; let moved = false;
    this._drag = true;
    const mm = (ev) => {
      const p = this.localPx(ev);
      if (!moved && Math.hypot(p.x - x0, p.y - y0) > 4) { moved = true; marquee = h('div', { class: 'marquee' }); this.view.append(marquee); }
      if (marquee) { Object.assign(marquee.style, { left: Math.min(x0, p.x) + 'px', top: Math.min(y0, p.y) + 'px', width: Math.abs(p.x - x0) + 'px', height: Math.abs(p.y - y0) + 'px' }); }
    };
    const mu = (ev) => {
      removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); this._drag = false;
      const p = this.localPx(ev);
      if (marquee) {
        marquee.remove();
        const r = { x0: Math.min(x0, p.x), y0: Math.min(y0, p.y), x1: Math.max(x0, p.x), y1: Math.max(y0, p.y) };
        if (App.compMode && App.hilite.length) Sel.selectComponents(this.marqueeComponents(r), mode);
        else Sel.select(this.selectTarget(this.marqueeObjects(r)), mode);
      } else this.clickSelect(p, mode);
      placeProxy(); App.requestRender();
    };
    addEventListener('mousemove', mm); addEventListener('mouseup', mu);
  }
  selectTarget(list) {
    if (App.selectMode !== 'hierarchy') return list;
    return [...new Set(list.map(o => { let r = o; while (r.parent && r.parent.inca) r = r.parent; return r; }))];
  }
  clickSelect(px, mode) {
    if (App.compMode && App.hilite.length) {
      const c = this.pickComponent(px);
      if (c) { Sel.selectComponents([{ o: c.o, ids: [c.id] }], mode); return; }
      // clicking another object while in component mode hilites it (Maya behaviour)
      const hits = this.pickObjects(px);
      if (hits.length && !App.hilite.includes(hits[0].o)) { if (mode === 'replace') Sel.hilite([hits[0].o]); else Sel.hilite([...App.hilite, hits[0].o]); App.sel = [...new Set([...App.sel, hits[0].o])]; return; }
      if (mode === 'replace') Sel.selectComponents([], 'replace');
      return;
    }
    const hits = this.pickObjects(px);
    if (hits.length) {
      // repeated clicks cycle through overlapping objects
      let pick = hits[0].o;
      Sel.select(this.selectTarget([pick]), mode);
    } else if (mode === 'replace') Sel.select([], 'replace');
  }
  onDbl(e) {
    if (e.altKey) return;
    const px = this.localPx(e);
    if (App.compMode === 'edge') { const c = this.pickComponent(px, 'edge'); if (c) Sel.edgeLoop(c.o, c.id, e.shiftKey); }
    else if (App.compMode === 'face') {
      // face loop: pick the nearest edge of the face under cursor
      const c = this.pickComponent(px, 'edge', { forTool: true }); if (c) Sel.faceLoop(c.o, c.id, e.shiftKey);
    } else if (!App.compMode) {
      const hits = this.pickObjects(px); if (hits.length && hits[0].o.inca.kind !== 'camera') { App.cmds.run('toolSettingsForCurrent'); }
    }
  }
  onHover(e) {
    const px = this.localPx(e);
    if (App.tools && App.tools.handleHover && App.tools.handleHover(this, e, px)) return;
    if (!App.compMode || !App.hilite.length || e.buttons) { if (this.hover) this.setHover(null); return; }
    const now = performance.now(); if (now - (this._ht || 0) < 30) return; this._ht = now;
    const c = this.pickComponent(px);
    this.setHover(c);
  }
  setHover(c) {
    const key = c ? c.o.inca.id + ':' + c.kind + ':' + c.id : null;
    if (key === this._hoverKey) return;
    this._hoverKey = key; this.hover = c;
    App.hoverComp = c; App.hoverCompDirty = true; App.requestRender();
  }

  // ---------------------------------------------------------------- menus & toolbar
  buildMenus() {
    const vp = this; const o = this.opts; const rr = () => { vp.needsRender = true; App.dirty('panelUI'); };
    const camItems = () => {
      const cams = allDag(true).filter(n => n.inca.kind === 'camera');
      return cams.filter(c => !c.inca.cam.ortho).map(c => ({ label: c.inca.name, check: () => vp.camName === c.inca.name, fn: () => vp.setCamera(c.inca.name) }));
    };
    const orthoItems = () => allDag(true).filter(n => n.inca.kind === 'camera' && n.inca.cam.ortho).map(c => ({ label: c.inca.name, check: () => vp.camName === c.inca.name, fn: () => vp.setCamera(c.inca.name) }));
    menuBar(this.menubar, [
      { label: 'View', items: () => [
        { label: 'Select Camera', fn: () => Sel.select([vp.camNode]) },
        { label: 'Default View', fn: () => vp.defaultView() },
        { label: 'Look at Selection', fn: () => vp.lookAtSelection() },
        { label: 'Frame All', hk: 'A', fn: () => vp.frameAll() },
        { label: 'Frame Selection', hk: 'F', fn: () => vp.frameSelection() },
        '-',
        { label: 'Camera Settings', sub: [
          { label: 'Resolution Gate', check: () => o.resGate, fn: () => { o.resGate = !o.resGate; o.filmGate = false; rr(); } },
          { label: 'Film Gate', check: () => o.filmGate, fn: () => { o.filmGate = !o.filmGate; o.resGate = false; rr(); } },
          { label: 'Orthographic Tumble', check: () => !!App.prefs?.orthoTumble, fn: () => { App.prefs.orthoTumble = !App.prefs.orthoTumble; App.savePrefs(); } },
        ] },
        { label: 'Camera Attribute Editor...', fn: () => { Sel.select([vp.camNode]); App.cmds.run('attributeEditor'); } },
        '-',
        { label: 'Predefined Bookmarks', sub: [['Perspective', [28, 21, 28], [-27.938, 45, 0]], ['Front', [0, 0, 40], [0, 0, 0]], ['Back', [0, 0, -40], [0, 180, 0]], ['Left', [-40, 0, 0], [0, -90, 0]], ['Right', [40, 0, 0], [0, 90, 0]], ['Top', [0, 40, 0], [-90, 0, 0]], ['Bottom', [0, -40, 0], [90, 0, 0]]].map(([l, t, r]) => ({ label: l, fn: () => { const c = vp.camNode; if (c.inca.cam.ortho) return; c.inca.t = t.slice(); c.inca.r = r.slice(); c.inca.cam.coi = Math.hypot(...t); updateXform(c); vp.needsRender = true; } })) },
      ] },
      { label: 'Shading', items: () => [
        { label: 'Wireframe', hk: '4', check: () => o.shading === 'wire', fn: () => { o.shading = 'wire'; rr(); } },
        { label: 'Smooth Shade All', hk: '5', check: () => o.shading === 'smooth', fn: () => { o.shading = 'smooth'; rr(); } },
        { label: 'Bounding Box', check: () => o.shading === 'bbox', fn: () => { o.shading = 'wire'; rr(); } },
        '-',
        { label: 'Wireframe on Shaded', check: () => o.wireOnShaded, fn: () => { o.wireOnShaded = !o.wireOnShaded; rr(); } },
        { label: 'X-Ray', hk: 'Alt+A', check: () => o.xray, fn: () => { o.xray = !o.xray; rr(); } },
        { label: 'Backface Culling', check: () => !!App.backfaceCulling, fn: () => App.cmds.run('toggleBackface') },
        '-',
        { label: 'Hardware Texturing', hk: '6', check: () => o.textured, fn: () => { o.textured = !o.textured; rr(); } },
      ] },
      { label: 'Lighting', items: () => [
        { label: 'Use Default Lighting', check: () => o.lighting === 'default', fn: () => { o.lighting = 'default'; rr(); } },
        { label: 'Use All Lights', hk: '7', check: () => o.lighting === 'all', fn: () => { o.lighting = 'all'; rr(); } },
        { label: 'Use Flat Lighting', check: () => o.lighting === 'flat', fn: () => { o.lighting = 'flat'; rr(); } },
        { label: 'Use No Lights', check: () => o.lighting === 'none', fn: () => { o.lighting = 'none'; rr(); } },
        '-',
        { label: 'Shadows', check: () => o.shadows, fn: () => { o.shadows = !o.shadows; if (o.shadows && o.lighting !== 'all') vp.flash('Shadows need "Use All Lights" (7)'); rr(); } },
      ] },
      { label: 'Show', items: () => [
        { label: 'Isolate Select', sub: [
          { label: 'View Selected', check: () => !!o.isolate, fn: () => vp.toggleIsolate() },
          { label: 'Add Selected Objects', fn: () => { if (!o.isolate) o.isolate = new Set(); for (const s of App.sel) o.isolate.add(s.inca.id); rr(); } },
          { label: 'Remove Selected Objects', fn: () => { if (o.isolate) for (const s of App.sel) o.isolate.delete(s.inca.id); rr(); } },
        ] },
        '-',
        { label: 'All', fn: () => { for (const k in o.show) o.show[k] = true; rr(); } },
        { label: 'None', fn: () => { for (const k in o.show) if (k !== 'hud' && k !== 'manipulators') o.show[k] = false; rr(); } },
        '-',
        ...[['NURBS Curves', 'nurbsCurves'], ['NURBS Surfaces', 'nurbsSurfaces'], ['Polygons', 'polymeshes'], ['Lights', 'lights'], ['Cameras', 'cameras'], ['Locators', 'locators'], '-', ['Grid', 'grid'], ['Manipulators', 'manipulators'], ['Selection Highlighting', 'selectionHighlight'], ['HUD', 'hud']].map(x => x === '-' ? '-' : ({ label: x[0], check: () => o.show[x[1]], fn: () => { o.show[x[1]] = !o.show[x[1]]; if (x[1] === 'grid') o.grid = o.show.grid; rr(); } })),
      ] },
      { label: 'Renderer', items: () => [
        { label: 'Viewport 2.0 (Inca GL)', check: () => true, fn: () => {} },
        { label: 'Inca Path Tracer (Render View)', fn: () => App.cmds.run('renderView') },
      ] },
      { label: 'Panels', items: () => [
        { label: 'Perspective', sub: () => [...camItems(), '-', { label: 'New', fn: () => { const c = App.cmds.run('createCamera'); if (c) vp.setCamera(c.inca.name); } }] },
        { label: 'Orthographic', sub: orthoItems },
        { label: 'Look Through Selected', fn: () => { const c = App.sel.find(s => s.inca.kind === 'camera'); if (c) vp.setCamera(c.inca.name); else vp.flash('Select a camera'); } },
        '-',
        { label: 'Panel', sub: [{ label: 'Outliner', fn: () => App.cmds.run('outlinerWindow') }, { label: 'Graph Editor', fn: () => App.cmds.run('graphEditor') }, { label: 'Dope Sheet', fn: () => App.cmds.run('dopeSheet') }, { label: 'Hypershade', fn: () => App.cmds.run('hypershade') }, { label: 'Hypergraph Hierarchy', fn: () => App.cmds.run('hypergraph') }, { label: 'Render View', fn: () => App.cmds.run('renderView') }] },
        { label: 'Layouts', sub: [['Single Pane', 'single'], ['Two Panes Side by Side', 'twoSide'], ['Two Panes Stacked', 'twoStack'], ['Three Panes Split Left', 'three'], ['Four Panes', 'four']].map(([l, k]) => ({ label: l, check: () => App.layout === k, fn: () => App.setLayout(k) })) },
      ] },
    ]);
  }
  toggleIsolate() { const o = this.opts; if (o.isolate) o.isolate = null; else { o.isolate = new Set(App.sel.map(s => s.inca.id)); if (!o.isolate.size) { o.isolate = null; this.flash('Select objects to isolate'); } } this.needsRender = true; App.dirty('panelUI'); }
  buildToolbar() {
    const tb = this.toolbar; tb.innerHTML = ''; const o = this.opts; const vp = this;
    const tog = (icon, title, get, set) => { const b = iconBtn(icon, title, () => { set(); vp.needsRender = true; vp.syncToolbar(); }); b._get = get; return b; };
    const sep = () => h('div', { class: 'tsep' });
    this._tbBtns = [
      iconBtn('selectCamera', 'Select camera', () => Sel.select([vp.camNode])),
      iconBtn('camAttrs', 'Camera attributes', () => { Sel.select([vp.camNode]); App.cmds.run('attributeEditor'); }),
      iconBtn('bookmark', 'Default view', () => vp.defaultView()),
      sep(),
      tog('grid', 'Grid', () => o.grid, () => { o.grid = !o.grid; o.show.grid = o.grid; }),
      tog('filmGate', 'Film gate', () => o.filmGate, () => { o.filmGate = !o.filmGate; o.resGate = false; }),
      tog('resGate', 'Resolution gate', () => o.resGate, () => { o.resGate = !o.resGate; o.filmGate = false; }),
      sep(),
      tog('wireframe', 'Wireframe (4)', () => o.shading === 'wire', () => { o.shading = 'wire'; }),
      tog('smoothShade', 'Smooth shade all (5)', () => o.shading === 'smooth', () => { o.shading = 'smooth'; }),
      tog('wireOnShaded', 'Wireframe on shaded', () => o.wireOnShaded, () => { o.wireOnShaded = !o.wireOnShaded; }),
      tog('textured', 'Textured (6)', () => o.textured, () => { o.textured = !o.textured; }),
      tog('useLights', 'Use all lights (7)', () => o.lighting === 'all', () => { o.lighting = o.lighting === 'all' ? 'default' : 'all'; }),
      tog('shadows', 'Shadows', () => o.shadows, () => { o.shadows = !o.shadows; if (o.shadows && o.lighting !== 'all') { o.lighting = 'all'; } }),
      sep(),
      tog('isolate', 'Isolate select', () => !!o.isolate, () => vp.toggleIsolate()),
      tog('xray', 'X-Ray (Alt+A)', () => o.xray, () => { o.xray = !o.xray; }),
      tog('backface', 'Backface culling', () => !!App.backfaceCulling, () => App.cmds.run('toggleBackface')),
      tog('hud', 'Poly count HUD', () => !!App.prefs?.hud?.polyCount, () => { App.prefs.hud.polyCount = !App.prefs.hud.polyCount; App.savePrefs(); App.requestRender(); }),
      sep(),
    ];
    const exp = h('input', { value: '0.00', title: 'Exposure' });
    exp.addEventListener('change', () => { o.exposure = parseFloat(exp.value) || 0; exp.value = o.exposure.toFixed(2); vp.needsRender = true; });
    exp.addEventListener('keydown', e => e.stopPropagation());
    const gam = h('input', { value: '1.00', title: 'Gamma (display)', disabled: true });
    tb.append(...this._tbBtns, h('div', { class: 'ib small', html: '', title: 'Exposure', style: { width: '6px' } }), iconBtn('exposure', 'Exposure'), exp, iconBtn('gamma', 'Gamma'), gam, h('select', { title: 'View transform' }, h('option', { text: 'sRGB gamma' })));
    this.syncToolbar();
  }
  syncToolbar() { for (const b of this._tbBtns || []) if (b._get) b.classList.toggle('on', !!b._get()); this.updateLabel(); }
}

function isoContains(iso, n) { let p = n; while (p && p.inca) { if (iso.has(p.inca.id)) return true; p = p.parent; } let found = false; n.traverse(c => { if (c.inca && iso.has(c.inca.id)) found = true; }); return found; }
function layerUnselectable(n) { const l = n.inca.layer && App.layers.find(x => x.id === n.inca.layer); return !!(l && l.mode && l.mode !== 'normal'); }

// ------------------------------------------------------------------- component overlays (shared by all viewports)
export function updateComponentOverlays() {
  const showFor = new Set(App.hilite);
  App.world.traverse(n => {
    if (!n.inca || (n.inca.kind !== 'mesh' && n.inca.kind !== 'curve')) return;
    const ud = n.userData;
    if (!showFor.has(n) || !App.compMode) { if (ud.comp) { ud.comp.visible = false; } return; }
    if (!ud.comp) { ud.comp = new THREE.Group(); ud.comp.userData.helper = true; n.add(ud.comp); ud.compDirty = true; }
    ud.comp.visible = true;
    if (!ud.compDirty && ud.compMode === App.compMode && ud.compSoft === (Manip.soft.on ? Manip.soft.radius : 0)) return;
    ud.compDirty = false; ud.compMode = App.compMode; ud.compSoft = Manip.soft.on ? Manip.soft.radius : 0;
    for (const c of [...ud.comp.children]) { c.geometry && c.geometry.dispose(); ud.comp.remove(c); }
    const cs = n.inca.compSel || { v: new Set(), e: new Set(), f: new Set() };
    if (n.inca.kind === 'curve') {
      const cvs = n.inca.curve.cvs; const pos = cvs.flat(); const col = [];
      cvs.forEach((_, i) => { const c = cs.v.has(i) ? COMP.vertSel : COMP.cv; col.push(c.r, c.g, c.b); });
      const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3)); g.setAttribute('color', new THREE.Float32BufferAttribute(col, 3));
      ud.comp.add(new THREE.Points(g, pointsMatX));
      const hull = new THREE.BufferGeometry(); hull.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
      ud.comp.add(new THREE.Line(hull, new THREE.LineDashedMaterial({ color: 0xc070ff, dashSize: 0.1, gapSize: 0.08 })).computeLineDistances());
      return;
    }
    const pm = n.inca.mesh; const t = pm.topo;
    const mode = App.compMode;
    // edges (always drawn in component mode, coloured by selection in edge mode)
    { const pos = [], col = [];
      t.edges.forEach(([a, b], i) => { const p = pm.v[a], q = pm.v[b]; pos.push(p[0], p[1], p[2], q[0], q[1], q[2]); const c = (mode === 'edge' && cs.e.has(i)) ? COMP.edgeSel : COMP.edge; col.push(c.r, c.g, c.b, c.r, c.g, c.b); });
      const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3)); g.setAttribute('color', new THREE.Float32BufferAttribute(col, 3));
      const ls = new THREE.LineSegments(g, edgeMat); ls.renderOrder = 2; ud.comp.add(ls); }
    if (mode === 'vertex' || mode === 'uv') {
      const pos = pm.v.flat(); const col = [];
      let w = null; if (Manip.soft.on && cs.v.size) w = softWeights(n, [...cs.v]);
      pm.v.forEach((_, i) => { let c = cs.v.has(i) ? COMP.vertSel : COMP.vert; if (w && w.has(i) && !cs.v.has(i)) { const k = w.get(i); c = new THREE.Color().setRGB(1, k * 0.9, 0).lerp(COMP.vert, 0.0); c = new THREE.Color(0x111111).lerp(new THREE.Color(1, 0.85 * k + 0.1, 0), 0.3 + 0.7 * k); } col.push(c.r, c.g, c.b); });
      const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3)); g.setAttribute('color', new THREE.Float32BufferAttribute(col, 3));
      const pts = new THREE.Points(g, pointsMat); pts.renderOrder = 3; ud.comp.add(pts);
    }
    if (mode === 'face') {
      const pos = [];
      for (const fi of cs.f) { const f = pm.f[fi]; if (!f) continue; for (let i = 1; i + 1 < f.length; i++) for (const k of [0, i, i + 1]) pos.push(...pm.v[f[k]]); }
      if (pos.length) { const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3)); const m = new THREE.Mesh(g, faceSelMat); m.renderOrder = 2; ud.comp.add(m); }
      // face centre dots
      const dp = []; const dc = [];
      pm.f.forEach((_, fi) => { dp.push(...pm.faceCenter(fi)); const c = cs.f.has(fi) ? COMP.edgeSel : COMP.dot; dc.push(c.r, c.g, c.b); });
      const g2 = new THREE.BufferGeometry(); g2.setAttribute('position', new THREE.Float32BufferAttribute(dp, 3)); g2.setAttribute('color', new THREE.Float32BufferAttribute(dc, 3));
      const dots = new THREE.Points(g2, new THREE.PointsMaterial({ size: 4, sizeAttenuation: false, vertexColors: true })); ud.comp.add(dots);
    }
  });
  // hover (pre-selection highlight)
  if (App.hoverCompDirty) {
    App.hoverCompDirty = false;
    if (App._hoverObj) { App._hoverObj.parent && App._hoverObj.parent.remove(App._hoverObj); App._hoverObj.geometry.dispose(); App._hoverObj = null; }
    const c = App.hoverComp;
    if (c && c.o && (App.compMode || App.tool)) {
      const pm = c.o.inca.mesh; let obj = null;
      if (c.kind === 'vertex') { const p = c.o.inca.kind === 'curve' ? c.o.inca.curve.cvs[c.id] : pm.v[c.id]; if (p) { const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(p, 3)); obj = new THREE.Points(g, new THREE.PointsMaterial({ size: 9, sizeAttenuation: false, color: 0xff2a2a, depthTest: false })); } }
      else if (c.kind === 'edge' && pm) { const e = pm.topo.edges[c.id]; if (e) { const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute([...pm.v[e[0]], ...pm.v[e[1]]], 3)); obj = new THREE.LineSegments(g, hoverLineMat); } }
      else if (c.kind === 'face' && pm) { const f = pm.f[c.id]; if (f) { const pos = []; for (let i = 1; i + 1 < f.length; i++) for (const k of [0, i, i + 1]) pos.push(...pm.v[f[k]]); const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3)); obj = new THREE.Mesh(g, faceHoverMat); } }
      if (obj) { obj.userData.helper = true; obj.renderOrder = 5; c.o.add(obj); App._hoverObj = obj; }
    }
  }
}
App.updateComponentOverlays = updateComponentOverlays;
export { toolLineMat, toolPtMat };
