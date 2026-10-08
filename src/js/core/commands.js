// Inca — command registry: every action in menus, shelves, hotkeys and scripts.
import * as THREE from 'three';
import { App } from './app.js';
import * as S from './scene.js';
import { Sel } from './selection.js';
import { Undo } from './undo.js';
import { OPS, niceName } from './ops.js';
import { GEN_INFO } from './primitives.js';
import { Curve, nurbsCircle, nurbsSquare } from './curves.js';
import { createMaterial, createTexture, getThreeMaterial, MAT_TYPES, updateMaterial } from './materials.js';
import { Manip, placeProxy } from './manip.js';
import { ekey } from './polymesh.js';

const registry = new Map();
let lastRepeatable = null;
export const Cmds = {
  registry,
  register(id, label, run, o = {}) { registry.set(id, { id, label, run, ...o }); },
  info(id) { const c = registry.get(id); if (!c) return null; return { label: c.label, hotkey: App.hotkeyLabel ? App.hotkeyLabel(id) : (c.hk || ''), icon: c.icon, opt: c.options ? true : null, help: c.help || c.label }; },
  run(id, args) {
    const c = registry.get(id);
    if (!c) { console.warn('unknown command', id); App.emit('error', `Cannot find procedure "${id}".`); return; }
    if (c.enabled && !c.enabled()) return;
    if (c.repeatable !== false && !c.noRepeat) lastRepeatable = { id, args };
    try {
      const r = c.run(args || {});
      App.dirty('outliner', 'channels', 'attr', 'title', 'timeline');
      App.requestRender();
      return r;
    } catch (e) { console.error(e); App.emit('error', `// Error: ${c.label}: ${e.message || e}`); }
  },
  option(id) { const c = registry.get(id); if (c && c.options) App.ui.optionBox(id, c); else Cmds.run(id); },
  repeatLast() { if (lastRepeatable) Cmds.run(lastRepeatable.id, lastRepeatable.args); },
  list() { return [...registry.values()]; },
};
App.cmds = Cmds;
const C = Cmds.register;
const echo = (s) => App.emit('echo', s);
const result = (s) => App.emit('result', s);
const warn = (s) => { App.emit('warning', s); };
const opts = (id) => (App.prefs && App.prefs.opt && App.prefs.opt[id]) || {};
const ck = (label) => Undo.checkpoint(label);
const fx = (n) => +(+n).toFixed(4);

function requireSel(kinds = null) {
  const s = App.sel.filter(o => !kinds || kinds.includes(o.inca.kind));
  if (!s.length) { warn('// Warning: Nothing selected' + (kinds ? ` (${kinds.join('/')})` : '')); return null; }
  return s;
}
function exitComponentMode() { if (App.compMode) Sel.setMode(null); }

// ------------------------------------------------------------------ primitives
export function createPrimitive(gen, params = {}) {
  ck('create ' + gen);
  exitComponentMode();
  const o = S.createMesh(gen, { ...GEN_INFO[gen].params, ...opts(gen), ...params });
  if (App.prefs?.primitivesAtGridCenter === false && App.activeViewport) { }
  Sel.select([o], 'replace', { echo: false });
  const h = o.inca.history[0];
  const sh = GEN_INFO[gen].short; const inv = Object.fromEntries(Object.entries(sh).map(([k, v]) => [v, k]));
  echo(`${gen} ${Object.entries(h.params).map(([k, v]) => `-${inv[k] || k} ${v}`).join(' ')} -ch 1;`);
  result(`${o.inca.name} ${h.name}`);
  App.emit('inViewEditor', h);
  return o;
}
for (const [gen, info] of Object.entries(GEN_INFO)) {
  const label = info.nurbs ? niceName(gen.replace('nurbs', '')).trim() : niceName(gen.replace('poly', '')).trim();
  C(gen, label === 'Platonic' ? 'Platonic Solid' : label, (a) => createPrimitive(gen, a), { icon: gen === 'polyPlatonic' ? 'polyPlatonic' : gen, mel: gen, options: OPS[gen].channels.map(ch => ({ key: ch.k, label: ch.label || niceName(ch.k), type: ch.type === 'enum' ? 'enum' : ch.type, options: ch.options, values: ch.values, value: info.params[ch.k] })), help: `Create a ${info.nurbs ? 'NURBS' : 'polygon'} ${label.toLowerCase()}` });
}
C('nurbsCircle', 'Circle', (a) => { ck('circle'); exitComponentMode(); const o = S.createCurve(nurbsCircle({ radius: 1, sections: 8, ...opts('nurbsCircle'), ...a }), { name: 'nurbsCircle1' }); Sel.select([o]); echo('circle -c 0 0 0 -nr 0 1 0 -sw 360 -r 1 -d 3 -ut 0 -tol 0.01 -s 8 -ch 1;'); result(o.inca.name); return o; }, { icon: 'nurbsCircle', options: [{ key: 'radius', label: 'Radius', type: 'float', value: 1 }, { key: 'sections', label: 'Number of sections', type: 'int', value: 8 }] });
C('nurbsSquare', 'Square', (a) => { ck('square'); exitComponentMode(); const o = S.createCurve(nurbsSquare({ sideLength: 1, ...opts('nurbsSquare'), ...a }), { name: 'nurbsSquare1' }); Sel.select([o]); echo('nurbsSquare -c 0 0 0 -nr 0 1 0 -sl1 1 -sl2 1 -sps 1 -d 3 -ch 1;'); return o; }, { icon: 'nurbsSquare' });

// ------------------------------------------------------------------ lights / camera / misc creation
const LIGHT_LABEL = { ambientLight: 'Ambient Light', directionalLight: 'Directional Light', pointLight: 'Point Light', spotLight: 'Spot Light', areaLight: 'Area Light', skyDomeLight: 'Sky Dome Light' };
for (const [t, l] of Object.entries(LIGHT_LABEL)) {
  C(t, l, () => {
    ck('create ' + t); exitComponentMode();
    const o = S.createLight(t);
    if (t === 'directionalLight' || t === 'spotLight' || t === 'areaLight') { if (t !== 'areaLight') { o.inca.r = [-45, 45, 0]; } }
    if (t === 'spotLight' || t === 'areaLight' || t === 'pointLight') { o.inca.t = t === 'pointLight' ? [0, 4, 0] : [0, 0, 0]; }
    S.updateXform(o); S.syncLight(o);
    Sel.select([o]); echo(`${t === 'skyDomeLight' ? 'createNode skyDomeLight' : 'defaultLight'} -name "${o.inca.shapeName}" ;`); result(o.inca.shapeName);
    if (App.activeViewport && App.activeViewport.opts.lighting !== 'all' && !App._toldLights) { App._toldLights = true; App.activeViewport.flash('Press 7 to view with all lights'); }
    return o;
  }, { icon: t });
}
C('createCamera', 'Camera', () => { ck('camera'); exitComponentMode(); const o = S.createCamera('camera1'); o.inca.t = [0, 2, 10]; S.updateXform(o); Sel.select([o]); echo('camera -centerOfInterest 5 -focalLength 35 -lensSqueezeRatio 1 -cameraScale 1 -horizontalFilmAperture 1.41732 -horizontalFilmOffset 0 -verticalFilmAperture 0.94488 -verticalFilmOffset 0 -filmFit Fill -overscan 1 -motionBlur 0 -shutterAngle 144 -nearClipPlane 0.1 -farClipPlane 10000 -orthographic 0 -orthographicWidth 30 -panZoomEnabled 0 -horizontalPan 0 -verticalPan 0 -zoom 1;'); result(o.inca.name); return o; }, { icon: 'camera' });
C('createLocator', 'Locator', () => { ck('locator'); exitComponentMode(); const o = S.createLocator(); Sel.select([o]); echo('spaceLocator -p 0 0 0;'); result(o.inca.name); return o; }, { icon: 'locator' });
C('createEmptyGroup', 'Empty Group', () => { ck('group'); const o = S.createGroup('null1'); Sel.select([o]); echo('group -em;'); result(o.inca.name); return o; }, { icon: 'group' });
C('cvCurveTool', 'CV Curve Tool', () => App.setTool('cvCurve'), { icon: 'cvCurve', noRepeat: true, help: 'Click to place CVs; press Enter to complete the curve' });
C('epCurveTool', 'EP Curve Tool', () => App.setTool('epCurve'), { icon: 'epCurve', noRepeat: true, help: 'Click to place edit points; press Enter to complete' });
C('createPolygonTool', 'Create Polygon', () => App.setTool('createPoly'), { icon: 'planar', noRepeat: true, help: 'Click to place vertices; press Enter to complete the polygon' });

// ------------------------------------------------------------------ edit
C('undo', 'Undo', () => { Undo.undo(); placeProxy(); }, { hk: 'Z', icon: 'undo', noRepeat: true });
C('redo', 'Redo', () => { Undo.redo(); placeProxy(); }, { hk: 'Shift+Z', icon: 'redo', noRepeat: true });
C('repeatLast', 'Repeat', () => Cmds.repeatLast(), { hk: 'G', noRepeat: true });
C('deleteSel', 'Delete', () => {
  if (App.compMode && Sel.anyComponents()) return Cmds.run('deleteComponents');
  const s = requireSel(); if (!s) return;
  ck('delete'); const names = s.map(o => o.inca.name);
  for (const o of s) { App.anim.removeNode(o.inca.id); S.deleteNode(o); }
  App.sel = []; App.hilite = App.hilite.filter(o => App.nodes.has(o.inca.id));
  echo(`delete;`); App.emit('selectionChanged');
}, { hk: 'Delete', icon: 'delete' });
C('deleteComponents', 'Delete Components', () => {
  ck('delete components');
  for (const o of App.hilite) {
    if (o.inca.kind !== 'mesh') continue; const cs = o.inca.compSel;
    if (App.compMode === 'face' && cs.f.size) S.applyOp(o, 'deleteComponent', { faces: [...cs.f] });
    else if (App.compMode === 'edge' && cs.e.size) S.applyOp(o, 'deleteComponent', { edges: Sel.edgesAsPairs(o) });
    else if (App.compMode === 'vertex' && cs.v.size) S.applyOp(o, 'deleteComponent', { verts: [...cs.v] });
    else continue;
    cs.v.clear(); cs.e.clear(); cs.f.clear(); o.userData.compDirty = true;
  }
  echo('delete;'); App.emit('selectionChanged');
});
C('deleteEdgeVertex', 'Delete Edge/Vertex', () => {
  ck('delete edge/vertex');
  for (const o of App.hilite) { const cs = o.inca.compSel; if (App.compMode === 'edge' && cs.e.size) S.applyOp(o, 'deleteComponent', { edges: Sel.edgesAsPairs(o), cleanVerts: true }); else if (App.compMode === 'vertex' && cs.v.size) S.applyOp(o, 'deleteComponent', { verts: [...cs.v] }); cs.e.clear(); cs.v.clear(); o.userData.compDirty = true; }
  echo('polyDelEdge -cv true -ch 1;'); App.emit('selectionChanged');
}, { hk: 'Ctrl+Backspace', icon: 'deleteEdge' });
C('duplicate', 'Duplicate', () => {
  const s = requireSel(); if (!s) return; ck('duplicate');
  const o = opts('duplicateSpecial');
  const out = S.duplicateNodes(s, { keepHistory: !!o.keepHistory });
  App._lastDup = { src: s, out, delta: null };
  Sel.select(out, 'replace', { echo: false }); echo('duplicate -rr;'); result(out.map(x => x.inca.name).join(' '));
  return out;
}, { hk: 'Ctrl+D', icon: 'duplicate' });
C('duplicateSpecial', 'Duplicate Special', (a) => {
  const s = requireSel(); if (!s) return; ck('duplicate special');
  const o = { copies: 1, translate: [0, 0, 0], rotate: [0, 0, 0], scale: [1, 1, 1], keepHistory: false, ...opts('duplicateSpecial'), ...a };
  let cur = s; const all = [];
  for (let i = 0; i < Math.max(1, o.copies); i++) {
    const out = S.duplicateNodes(cur, { keepHistory: o.keepHistory });
    out.forEach(n => { for (let k = 0; k < 3; k++) { n.inca.t[k] += o.translate[k]; n.inca.r[k] += o.rotate[k]; n.inca.s[k] *= o.scale[k]; } S.updateXform(n); });
    all.push(...out); cur = out;
  }
  Sel.select(cur, 'replace', { echo: false }); echo(`duplicate -rr; // x${o.copies}`); return all;
}, { hk: 'Ctrl+Shift+D', options: [{ key: 'keepHistory', label: 'Duplicate input graph', type: 'bool', value: false }, { key: 'translate', label: 'Translate', type: 'vec3', value: [0, 0, 0] }, { key: 'rotate', label: 'Rotate', type: 'vec3', value: [0, 0, 0] }, { key: 'scale', label: 'Scale', type: 'vec3', value: [1, 1, 1] }, { key: 'copies', label: 'Number of copies', type: 'int', value: 1 }] });
C('duplicateWithTransform', 'Duplicate with Transform', () => {
  const s = requireSel(); if (!s) return;
  const prev = App._lastDup;
  ck('duplicate with transform');
  const out = S.duplicateNodes(s, { keepHistory: false });
  if (prev && prev.out && prev.out.every((o, i) => s[i] === o) && prev.delta) {
    out.forEach((n, i) => { const d = prev.delta[i] || prev.delta[0]; for (let k = 0; k < 3; k++) { n.inca.t[k] += d.t[k]; n.inca.r[k] += d.r[k]; n.inca.s[k] *= d.s[k]; } S.updateXform(n); });
  }
  App._lastDup = { src: s, out, delta: null, base: out.map(n => ({ t: n.inca.t.slice(), r: n.inca.r.slice(), s: n.inca.s.slice() })) };
  Sel.select(out, 'replace', { echo: false }); echo('duplicate -rr -st;'); return out;
}, { hk: 'Shift+D' });
// record the transform delta applied after a duplicate (for Shift+D)
App.on('refresh', () => {
  const d = App._lastDup; if (!d || !d.out || !d.base) return;
  const src = d.src;
  d.delta = d.out.map((n, i) => { const s = src[i] || src[0]; if (!s || !s.inca) return { t: [0, 0, 0], r: [0, 0, 0], s: [1, 1, 1] }; return { t: n.inca.t.map((v, k) => v - s.inca.t[k]), r: n.inca.r.map((v, k) => v - s.inca.r[k]), s: n.inca.s.map((v, k) => v / (s.inca.s[k] || 1)) }; });
});
C('group', 'Group', () => {
  const s = requireSel(); if (!s) return; ck('group');
  const parent = S.dagParent(s[0]);
  const g = S.createGroup('group1', { parent });
  for (const o of s) S.parentTo(o, g, true);
  if ((opts('group').pivot ?? 'center') === 'center') S.centerPivot(g);
  Sel.select([g], 'replace', { echo: false }); echo('doGroup 0 1 1;'); result(g.inca.name); return g;
}, { hk: 'Ctrl+G', icon: 'group', options: [{ key: 'pivot', label: 'Group pivot', type: 'radio', options: ['Center', 'Origin'], values: ['center', 'origin'], value: 'center' }] });
C('ungroup', 'Ungroup', () => {
  const s = requireSel(['group']); if (!s) return; ck('ungroup'); const kids = [];
  for (const g of s) { const p = S.dagParent(g); for (const c of S.dagChildren(g)) { S.parentTo(c, p, true); kids.push(c); } S.deleteNode(g); }
  Sel.select(kids, 'replace', { echo: false }); echo('ungroup;');
}, { icon: 'ungroup' });
C('parent', 'Parent', () => {
  const s = requireSel(); if (!s || s.length < 2) { warn('// Warning: Select children then the parent last'); return; }
  ck('parent'); const p = s[s.length - 1];
  for (const c of s.slice(0, -1)) if (!S.parentTo(c, p, true)) warn(`// Warning: Cannot parent ${c.inca.name} to ${p.inca.name}`);
  Sel.select([p], 'replace', { echo: false }); echo(`parent ${s.map(o => o.inca.name).join(' ')};`);
}, { hk: 'P', icon: 'parent' });
C('unparent', 'Unparent', () => { const s = requireSel(); if (!s) return; ck('unparent'); for (const c of s) S.parentTo(c, null, true); echo('parent -w;'); App.dirty('outliner'); }, { hk: 'Shift+P' });
let clipboard = null;
C('copy', 'Copy', () => { const s = requireSel(); if (!s) return; clipboard = []; for (const o of s) o.traverse(c => { if (c.inca) clipboard.push(S.serializeNode(c)); }); clipboard.roots = s.map(o => o.inca.id); App.help(`Copied ${s.length} object(s)`); }, { hk: 'Ctrl+C' });
C('cut', 'Cut', () => { Cmds.run('copy'); Cmds.run('deleteSel'); }, { hk: 'Ctrl+X' });
C('paste', 'Paste', () => {
  if (!clipboard) return; ck('paste'); const map = new Map(); const roots = [];
  for (const d of clipboard) { const parent = map.get(d.parent) || null; const c = S.deserializeNode({ ...d, history: d.kind === 'mesh' ? [{ id: 'x', name: 'meshData#', type: 'meshData', params: { data: d.mesh } }] : d.history }, parent, { fresh: true }); map.set(d.id, c); if (clipboard.roots.includes(d.id)) roots.push(c); }
  Sel.select(roots, 'replace', { echo: false }); echo('paste;');
}, { hk: 'Ctrl+V' });
C('deleteHistory', 'History', () => { const s = requireSel(['mesh']); if (!s) return; ck('delete history'); for (const o of s) S.deleteHistory(o); echo('DeleteHistory;'); App.dirty('channels'); }, { hk: 'Alt+Shift+D', icon: 'deleteHistory' });
C('deleteAllHistory', 'History (All)', () => { ck('delete all history'); for (const o of S.allDag()) if (o.inca.kind === 'mesh') S.deleteHistory(o); echo('DeleteAllHistory;'); }, {});
C('deleteChannels', 'Channels', () => { const s = requireSel(); if (!s) return; ck('delete channels'); for (const o of s) App.anim.removeNode(o.inca.id); echo('delete -channels;'); App.dirty('timeline', 'channels', 'graph'); });
C('deleteAllLights', 'Lights (All)', () => { ck('delete lights'); for (const o of S.allDag()) if (o.inca.kind === 'light') S.deleteNode(o); App.emit('selectionChanged'); });
C('deleteAllCameras', 'Cameras (All)', () => { ck('delete cameras'); for (const o of S.allDag()) if (o.inca.kind === 'camera' && !o.inca.startup) S.deleteNode(o); App.emit('selectionChanged'); });
C('deleteAllCurves', 'NURBS Curves (All)', () => { ck('delete curves'); for (const o of S.allDag()) if (o.inca.kind === 'curve') S.deleteNode(o); App.emit('selectionChanged'); });

// ------------------------------------------------------------------ select
C('selectAll', 'All', () => Sel.selectAll(), { hk: 'Ctrl+Shift+A' });
C('deselectAll', 'Deselect All', () => { if (App.compMode) Sel.selectComponents([], 'replace'); else Sel.select([], 'replace'); }, {});
C('selectHierarchy', 'Hierarchy', () => Sel.hierarchy());
C('invertSelection', 'Inverse', () => Sel.invert(), { hk: 'Ctrl+Shift+I' });
C('selectAllGeometry', 'All by Type > Geometry', () => Sel.byType('geometry'));
C('selectAllLights', 'All by Type > Lights', () => Sel.byType('light'));
C('selectAllCameras', 'All by Type > Cameras', () => Sel.byType('camera'));
C('selectAllCurves', 'All by Type > NURBS Curves', () => Sel.byType('curve'));
C('growSelection', 'Grow', () => Sel.grow(1), { hk: 'Shift+>' });
C('shrinkSelection', 'Shrink', () => Sel.grow(-1), { hk: 'Shift+<' });
C('selectBorder', 'Select Border Edges', () => Sel.border());
C('selectEdgeLoop', 'Select Edge Loop', () => { for (const o of App.hilite) { const e = [...(o.inca.compSel?.e || [])]; if (e.length) { Sel.edgeLoop(o, e[e.length - 1], true); } } });
C('selectEdgeRing', 'Select Edge Ring', () => { for (const o of App.hilite) { const e = [...(o.inca.compSel?.e || [])]; if (e.length) Sel.edgeRingSel(o, e[e.length - 1], true); } });
C('toVertices', 'To Vertices', () => Sel.convertTo('vertex'), { hk: 'Ctrl+F9' });
C('toEdges', 'To Edges', () => Sel.convertTo('edge'), { hk: 'Ctrl+F10' });
C('toFaces', 'To Faces', () => Sel.convertTo('face'), { hk: 'Ctrl+F11' });
C('objectMode', 'Object/Component', () => { if (App.compMode) Sel.setMode(null); else Sel.setMode(App.lastCompMode || 'vertex'); placeProxy(); }, { hk: 'F8' });
C('vertexMode', 'Vertex', () => { Sel.setMode('vertex'); App.lastCompMode = 'vertex'; placeProxy(); }, { hk: 'F9' });
C('edgeMode', 'Edge', () => { Sel.setMode('edge'); App.lastCompMode = 'edge'; placeProxy(); }, { hk: 'F10' });
C('faceMode', 'Face', () => { Sel.setMode('face'); App.lastCompMode = 'face'; placeProxy(); }, { hk: 'F11' });
C('uvMode', 'UV', () => { Sel.setMode('uv'); placeProxy(); }, { hk: 'F12' });
C('objectModeOnly', 'Object Mode', () => { Sel.setMode(null); placeProxy(); });

// ------------------------------------------------------------------ tools
C('selectTool', 'Select Tool', () => App.setTool('select'), { hk: 'Q', icon: 'select', noRepeat: true });
C('lassoTool', 'Lasso Tool', () => App.setTool('lasso'), { icon: 'lasso', noRepeat: true });
C('moveTool', 'Move Tool', () => App.setTool('move'), { hk: 'W', icon: 'move', noRepeat: true });
C('rotateTool', 'Rotate Tool', () => App.setTool('rotate'), { hk: 'E', icon: 'rotate', noRepeat: true });
C('scaleTool', 'Scale Tool', () => App.setTool('scale'), { hk: 'R', icon: 'scale', noRepeat: true });
C('lastToolUsed', 'Last Tool Used', () => App.setTool(App.lastTool || 'move'), { hk: 'Y', icon: 'lastTool', noRepeat: true });
C('pivotEdit', 'Edit Pivot', () => { Manip.pivotMode = !Manip.pivotMode; if (Manip.pivotMode && Manip.mode === 'none') App.setTool('move'); App.activeViewport?.flash(Manip.pivotMode ? 'Pivot edit: ON' : 'Pivot edit: OFF'); placeProxy(); App.requestRender(); }, { hk: 'D', noRepeat: true });
C('softSelect', 'Soft Select', () => { Manip.soft.on = !Manip.soft.on; for (const o of App.hilite) o.userData.compDirty = true; App.activeViewport?.flash('Soft Select: ' + (Manip.soft.on ? 'ON' : 'OFF')); App.dirty('toolSettings'); }, { hk: 'B', icon: 'softSelect', noRepeat: true });
C('manipBigger', 'Increase Manipulator Size', () => { App.prefs.manipSize = Math.min(4, (App.prefs.manipSize || 1) * 1.15); for (const v of App.viewports) v.tc.setSize(App.prefs.manipSize); App.savePrefs(); }, { hk: '=', noRepeat: true });
C('manipSmaller', 'Decrease Manipulator Size', () => { App.prefs.manipSize = Math.max(0.2, (App.prefs.manipSize || 1) / 1.15); for (const v of App.viewports) v.tc.setSize(App.prefs.manipSize); App.savePrefs(); }, { hk: '-', noRepeat: true });
C('insertEdgeLoopTool', 'Insert Edge Loop', () => App.setTool('edgeLoop'), { icon: 'edgeLoop', noRepeat: true, help: 'Click on an edge to insert an edge loop; drag to slide' });
C('multiCutTool', 'Multi-Cut', () => App.setTool('multiCut'), { icon: 'multiCut', noRepeat: true, help: 'Click on edges to place cut points; Enter/right-click to complete' });
C('targetWeldTool', 'Target Weld', () => App.setTool('targetWeld'), { icon: 'targetWeld', noRepeat: true, help: 'Drag from a vertex to a target vertex to weld them' });

// ------------------------------------------------------------------ modify
C('resetTransformations', 'Reset Transformations', () => { const s = requireSel(); if (!s) return; ck('reset'); for (const o of s) S.resetTransforms(o); echo('makeIdentity -apply false -t 1 -r 1 -s 1;'); placeProxy(); });
C('freezeTransformations', 'Freeze Transformations', () => { const s = requireSel(); if (!s) return; ck('freeze'); for (const o of s) S.freezeTransforms(o); echo('makeIdentity -apply true -t 1 -r 1 -s 1 -n 0 -pn 1;'); placeProxy(); }, { icon: 'freeze' });
C('centerPivot', 'Center Pivot', () => { const s = requireSel(); if (!s) return; ck('center pivot'); for (const o of s) S.centerPivot(o); echo('CenterPivot;'); placeProxy(); }, { icon: 'centerPivot' });
C('matchTranslation', 'Match Translation', () => { const s = requireSel(); if (!s || s.length < 2) return; ck('match'); const tgt = S.worldPivot(s[s.length - 1]); for (const o of s.slice(0, -1)) { const cur = S.worldPivot(o); const d = tgt.clone().sub(cur); const pl = S.parentWorld(o).invert().setPosition(0, 0, 0); d.applyMatrix4(pl); o.inca.t = o.inca.t.map((v, k) => v + d.getComponent(k)); S.updateXform(o); } echo('matchTransform -pos;'); placeProxy(); });
C('matchRotation', 'Match Rotation', () => { const s = requireSel(); if (!s || s.length < 2) return; ck('match'); const t = s[s.length - 1]; for (const o of s.slice(0, -1)) { o.inca.r = t.inca.r.slice(); S.updateXform(o); } placeProxy(); });
C('matchAll', 'Match Transformations', () => { const s = requireSel(); if (!s || s.length < 2) return; ck('match'); const t = s[s.length - 1]; for (const o of s.slice(0, -1)) S.setWorldMatrix(o, S.worldMatrix(t)); placeProxy(); });
C('snapToGround', 'Snap to Ground (Y=0)', () => { const s = requireSel(); if (!s) return; ck('snap'); for (const o of s) { const b = S.worldBBox(o); if (b.isEmpty()) continue; const d = new THREE.Vector3(0, -b.min.y, 0).applyMatrix4(S.parentWorld(o).invert().setPosition(0, 0, 0)); o.inca.t = o.inca.t.map((v, k) => v + d.getComponent(k)); S.updateXform(o); } placeProxy(); });
C('convertNurbsToPoly', 'NURBS to Polygons', () => { const s = requireSel(['mesh']); if (!s) return; ck('convert'); for (const o of s) { if (!o.inca.nurbs) continue; const pm = o.inca.mesh.clone(); pm.displayEdges = null; S.bakeMesh(o, pm); o.inca.nurbs = false; S.rebuildShape(o); } echo('nurbsToPoly -mnd 1 -ch 0 -f 2 -pt 1;'); });
C('prefixHierarchy', 'Prefix Hierarchy Names...', async () => { const s = requireSel(); if (!s) return; const p = await App.ui.prompt('Prefix Hierarchy', 'Prefix:', 'pre_'); if (!p) return; ck('prefix'); for (const o of s) o.traverse(c => { if (c.inca && !c.inca.startup) S.rename(c, p + c.inca.name); }); });
C('searchReplace', 'Search and Replace Names...', async () => { const a = await App.ui.prompt('Search and Replace', 'Search for:', ''); if (!a) return; const b = await App.ui.prompt('Search and Replace', 'Replace with:', ''); if (b === null) return; ck('rename'); for (const o of (App.sel.length ? App.sel : S.allDag())) if (o.inca.name.includes(a)) S.rename(o, o.inca.name.split(a).join(b)); });
C('renameSel', 'Rename...', async () => { const o = Sel.lead(); if (!o) return; const n = await App.ui.prompt('Rename', 'New name:', o.inca.name); if (!n) return; ck('rename'); S.rename(o, n); echo(`rename ${o.inca.name};`); });

// ------------------------------------------------------------------ display
C('hideSelection', 'Hide Selection', () => { const s = requireSel(); if (!s) return; ck('hide'); App._lastHidden = s.slice(); for (const o of s) S.setAttr(o, 'visibility', 0); echo('HideSelectedObjects;'); App.dirty('outliner'); }, { hk: 'Ctrl+H' });
C('showSelection', 'Show Selection', () => { const s = requireSel(); if (!s) return; ck('show'); for (const o of s) S.setAttr(o, 'visibility', 1); App.dirty('outliner'); }, { hk: 'Shift+H' });
C('showLastHidden', 'Show Last Hidden', () => { if (!App._lastHidden) return; ck('show'); for (const o of App._lastHidden) if (App.nodes.has(o.inca.id)) S.setAttr(o, 'visibility', 1); App.dirty('outliner'); }, { hk: 'Ctrl+Shift+H' });
C('hideUnselected', 'Hide Unselected Objects', () => { ck('hide'); App._lastHidden = []; for (const o of S.dagChildren(App.world)) { if (o.inca.startup) continue; let keep = false; o.traverse(c => { if (App.sel.includes(c)) keep = true; }); if (!keep) { App._lastHidden.push(o); S.setAttr(o, 'visibility', 0); } } App.dirty('outliner'); }, { hk: 'Alt+H' });
C('showAll', 'Show All', () => { ck('show all'); for (const o of S.allDag()) if (!o.inca.startup) S.setAttr(o, 'visibility', 1); App.dirty('outliner'); });
C('templateSel', 'Template', () => { const s = requireSel(); if (!s) return; ck('template'); for (const o of s) o.inca.template = true; App.sel = []; App.emit('selectionChanged'); });
C('untemplateSel', 'Untemplate', () => { ck('untemplate'); for (const o of S.allDag()) if (o.inca.template && (App.sel.includes(o) || App._olSel?.includes(o))) o.inca.template = false; for (const o of S.allDag()) o.inca.template = o.inca.template && !App.sel.includes(o); App.requestRender(); });
C('untemplateAll', 'Untemplate All', () => { ck('untemplate'); for (const o of S.allDag()) o.inca.template = false; App.requestRender(); });
const setSmooth = (lvl) => { const s = (App.compMode ? App.hilite : App.sel).filter(o => o.inca.kind === 'mesh'); if (!s.length) return; for (const o of s) { o.inca.smoothLevel = lvl; o.inca.mesh._smooth = null; S.rebuildShape(o); } echo(`displaySmoothness -polygonObject ${lvl === 0 ? 1 : lvl === 1 ? 2 : 3};`); };
C('smoothOff', 'Smoothness Rough', () => setSmooth(0), { hk: '1', noRepeat: true });
C('smoothCage', 'Smoothness Medium (cage + smooth)', () => setSmooth(1), { hk: '2', noRepeat: true });
C('smoothOn', 'Smoothness Fine (smooth mesh preview)', () => setSmooth(2), { hk: '3', noRepeat: true });
const vpo = (fn) => () => { const v = App.hoverViewport || App.activeViewport; if (!v) return; fn(v.opts, v); v.needsRender = true; v.syncToolbar(); };
C('shadeWire', 'Wireframe', vpo((o) => { o.shading = 'wire'; }), { hk: '4', icon: 'wireframe', noRepeat: true });
C('shadeSmooth', 'Smooth Shade All', vpo((o) => { o.shading = 'smooth'; }), { hk: '5', icon: 'smoothShade', noRepeat: true });
C('shadeTextured', 'Hardware Texturing', vpo((o) => { o.textured = !o.textured; if (o.shading === 'wire') o.shading = 'smooth'; }), { hk: '6', icon: 'textured', noRepeat: true });
C('shadeAllLights', 'Use All Lights', vpo((o) => { o.lighting = o.lighting === 'all' ? 'default' : 'all'; if (o.shading === 'wire') o.shading = 'smooth'; }), { hk: '7', icon: 'useLights', noRepeat: true });
C('toggleXray', 'X-Ray', vpo((o) => { o.xray = !o.xray; }), { hk: 'Alt+A', icon: 'xray', noRepeat: true });
C('toggleWireOnShaded', 'Wireframe on Shaded', vpo((o) => { o.wireOnShaded = !o.wireOnShaded; }), { icon: 'wireOnShaded', noRepeat: true });
C('toggleGrid', 'Grid', vpo((o) => { o.grid = !o.grid; o.show.grid = o.grid; }), { icon: 'grid', noRepeat: true });
C('isolateSelect', 'Isolate Select', vpo((o, v) => v.toggleIsolate()), { hk: 'Ctrl+1', icon: 'isolate', noRepeat: true });
C('toggleBackface', 'Backface Culling', () => { App.backfaceCulling = !App.backfaceCulling; for (const m of App.mats.values()) updateMaterial(m); App.requestRender(); }, { icon: 'backface', noRepeat: true });
C('cycleBackground', 'Toggle Background Color', () => { App.prefs.bgMode = ((App.prefs.bgMode || 0) + 1) % 5; App.savePrefs(); App.requestRender(); }, { hk: 'Alt+B', noRepeat: true });
C('frameSelected', 'Frame Selection', () => (App.hoverViewport || App.activeViewport)?.frameSelection(), { hk: 'F', icon: 'frameSel', noRepeat: true });
C('frameAll', 'Frame All', () => (App.hoverViewport || App.activeViewport)?.frameAll(), { hk: 'A', icon: 'frameAll', noRepeat: true });
C('frameSelectedAll', 'Frame Selection in All Views', () => App.viewports.forEach(v => v.frameSelection()), { hk: 'Shift+F', noRepeat: true });
C('frameAllAll', 'Frame All in All Views', () => App.viewports.forEach(v => v.frameAll()), { hk: 'Shift+A', noRepeat: true });
C('hudPolyCount', 'Poly Count', () => { App.prefs.hud.polyCount = !App.prefs.hud.polyCount; App.savePrefs(); App.requestRender(); }, { noRepeat: true });
C('hudFrameRate', 'Frame Rate', () => { App.prefs.hud.fps = !App.prefs.hud.fps; App.savePrefs(); App.requestRender(); }, { noRepeat: true });
C('hudCurrentFrame', 'Current Frame', () => { App.prefs.hud.frame = !App.prefs.hud.frame; App.savePrefs(); App.requestRender(); }, { noRepeat: true });
C('hudCameraNames', 'Camera Names', () => { App.prefs.hud.camNames = App.prefs.hud.camNames === false; App.savePrefs(); App.viewports.forEach(v => v.updateLabel()); }, { noRepeat: true });
C('hudViewAxis', 'View Axis', () => { App.prefs.hud.viewAxis = App.prefs.hud.viewAxis === false; App.savePrefs(); App.requestRender(); }, { noRepeat: true });
C('layoutSingle', 'Single Perspective View', () => { App.setLayout('single', 'persp'); }, { icon: 'layoutSingle', noRepeat: true });
C('layoutFour', 'Four View', () => App.setLayout('four'), { icon: 'layoutFour', noRepeat: true });
C('layoutTwo', 'Persp/Outliner', () => { App.setLayout('single', 'persp'); App.ui.toggleOutlinerDock(true); }, { icon: 'layoutOutliner', noRepeat: true });
C('layoutTwoSide', 'Two Panes Side by Side', () => App.setLayout('twoSide'), { icon: 'layoutTwo', noRepeat: true });

// ------------------------------------------------------------------ polygon ops on components / objects
function compTargets(kind) {
  const k = kind === 'face' ? 'f' : kind === 'edge' ? 'e' : 'v';
  return App.hilite.filter(o => o.inca.kind === 'mesh' && o.inca.compSel && o.inca.compSel[k].size);
}
function afterOp(o, sel) { if (sel) Sel.applyOpSelect(o, sel); else { Sel.pruneInvalid(o); } o.userData.compDirty = true; }
function showHistoryEditor(h) { App.emit('inViewEditor', h); }
C('extrude', 'Extrude', (a) => {
  const o0 = { thickness: 0, offset: 0, divisions: 1, keepFacesTogether: true, ...opts('extrude'), ...a };
  if (App.compMode === 'edge') {
    const ts = compTargets('edge'); if (!ts.length) return warn('// Warning: Select edges to extrude');
    ck('extrude edge'); let lastH;
    for (const o of ts) { const { h, select } = S.applyOp(o, 'polyExtrudeEdge', { edges: Sel.edgesAsPairs(o), thickness: 0, offset: 0 }); afterOp(o, select); lastH = h; }
    echo('polyExtrudeEdge -constructionHistory 1 -keepFacesTogether 1 -divisions 1 -twist 0 -taper 1 -offset 0 -thickness 0;'); result(lastH.name); showHistoryEditor(lastH); App.setTool('move'); placeProxy(); return;
  }
  if (App.compMode !== 'face') { if (!App.compMode && App.sel.some(o => o.inca.kind === 'curve')) return Cmds.run('extrudeCurve'); return warn('// Warning: Select faces or edges to extrude'); }
  const ts = compTargets('face'); if (!ts.length) return warn('// Warning: Select faces to extrude');
  ck('extrude'); let lastH;
  for (const o of ts) { const { h, select } = S.applyOp(o, 'polyExtrudeFace', { faces: [...o.inca.compSel.f], thickness: o0.thickness, offset: o0.offset, divisions: o0.divisions, keepFacesTogether: o0.keepFacesTogether }); afterOp(o, select); lastH = h; }
  echo(`polyExtrudeFacet -constructionHistory 1 -keepFacesTogether ${o0.keepFacesTogether ? 1 : 0} -divisions ${o0.divisions} -twist 0 -taper 1 -off ${o0.offset} -thickness ${o0.thickness} -smoothingAngle 30;`);
  result(lastH.name); showHistoryEditor(lastH);
  App.setTool('move'); placeProxy();
}, { hk: 'Ctrl+E', icon: 'extrude', options: [{ key: 'thickness', label: 'Thickness', type: 'float', value: 0 }, { key: 'offset', label: 'Offset', type: 'float', value: 0 }, { key: 'divisions', label: 'Divisions', type: 'int', value: 1 }, { key: 'keepFacesTogether', label: 'Keep faces together', type: 'bool', value: true }] });
C('bevel', 'Bevel', (a) => {
  const o0 = { fraction: 0.5, segments: 1, ...opts('bevel'), ...a };
  let targets = [];
  if (App.compMode === 'edge') targets = compTargets('edge').map(o => ({ o, edges: Sel.edgesAsPairs(o) }));
  else if (App.compMode === 'face') targets = compTargets('face').map(o => { const pm = o.inca.mesh; const es = new Set(); for (const f of o.inca.compSel.f) { const F = pm.f[f]; for (let i = 0; i < F.length; i++) es.add(ekey(F[i], F[(i + 1) % F.length])); } return { o, edges: [...es].map(k => pm.topo.edges[pm.topo.edgeMap.get(k)]) }; });
  else if (!App.compMode) targets = App.sel.filter(o => o.inca.kind === 'mesh').map(o => ({ o, edges: o.inca.mesh.topo.edges.slice() }));
  else if (App.compMode === 'vertex') return Cmds.run('chamferVertex');
  if (!targets.length) return warn('// Warning: Select edges, faces or objects to bevel');
  ck('bevel'); let lastH;
  for (const { o, edges } of targets) { const { h, select } = S.applyOp(o, 'polyBevel', { edges, fraction: o0.fraction, segments: o0.segments }); if (App.compMode) afterOp(o, select); lastH = h; }
  echo(`polyBevel3 -fraction ${o0.fraction} -offsetAsFraction 1 -autoFit 1 -depth 1 -mitering 0 -miterAlong 0 -chamfer 1 -segments ${o0.segments} -worldSpace 1 -smoothingAngle 30 -subdivideNgons 1 -mergeVertices 1 -mergeVertexTolerance 0.0001 -miteringAngle 180 -angleTolerance 180 -ch 1;`);
  result(lastH.name); showHistoryEditor(lastH); placeProxy();
}, { hk: 'Ctrl+B', icon: 'bevel', options: [{ key: 'fraction', label: 'Fraction', type: 'float', value: 0.5 }] });
C('chamferVertex', 'Chamfer Vertices', (a) => {
  const ts = compTargets('vertex'); if (App.compMode !== 'vertex' || !ts.length) return warn('// Warning: Select vertices to chamfer');
  ck('chamfer'); let lastH; for (const o of ts) { const { h, select } = S.applyOp(o, 'polyChamferVtx', { verts: [...o.inca.compSel.v], width: (opts('chamferVertex').width ?? 0.25) }); afterOp(o, select); lastH = h; }
  echo('polyChamferVtx 1 0.25 0;'); showHistoryEditor(lastH);
}, { icon: 'chamfer' });
C('bridge', 'Bridge', (a) => {
  const o0 = { divisions: 0, ...opts('bridge'), ...a }; let done = false;
  ck('bridge');
  for (const o of App.hilite) {
    if (o.inca.kind !== 'mesh') continue; const cs = o.inca.compSel;
    let r = null;
    if (App.compMode === 'face' && cs.f.size === 2) r = S.applyOp(o, 'polyBridgeEdge', { faces: [...cs.f], divisions: o0.divisions });
    else if (App.compMode === 'edge' && cs.e.size >= 2) r = S.applyOp(o, 'polyBridgeEdge', { edges: Sel.edgesAsPairs(o), divisions: o0.divisions });
    if (r) { if (r.h._error) { warn('// Warning: ' + r.h._error); o.inca.history.pop(); App.nodes.delete(r.h.id); S.evaluate(o, o.inca.history.length); continue; } afterOp(o, r.select); done = true; showHistoryEditor(r.h); }
  }
  if (!done) warn('// Warning: Select two faces, or two border edge loops on one mesh (Combine meshes first)'); else echo(`polyBridgeEdge -ch 1 -divisions ${o0.divisions} -twist 0 -taper 1 -curveType 0 -smoothingAngle 30;`);
}, { icon: 'bridge', options: [{ key: 'divisions', label: 'Divisions', type: 'int', value: 0 }] });
C('addDivisions', 'Add Divisions', (a) => {
  const d = { divisions: 1, ...opts('addDivisions'), ...a }.divisions; ck('add divisions');
  if (App.compMode === 'face') { for (const o of compTargets('face')) { const { select } = S.applyOp(o, 'polySubdFace', { divisions: d, faces: [...o.inca.compSel.f] }); afterOp(o, select); } }
  else for (const o of App.sel.filter(o => o.inca.kind === 'mesh')) S.applyOp(o, 'polySubdFace', { divisions: d });
  echo(`polySubdivideFacet -dv ${d} -m 0 -ch 1;`);
}, { icon: 'addDivisions', options: [{ key: 'divisions', label: 'Subdivision levels', type: 'int', value: 1 }] });
C('merge', 'Merge', (a) => {
  const th = { threshold: 0.001, ...opts('merge'), ...a }.threshold; ck('merge');
  const ts = App.compMode ? App.hilite.filter(o => o.inca.kind === 'mesh') : App.sel.filter(o => o.inca.kind === 'mesh');
  for (const o of ts) { const verts = App.compMode ? Sel.affectedVerts(o) : null; const before = o.inca.mesh.v.length; const { h } = S.applyOp(o, 'polyMergeVert', { verts, threshold: th }); afterOp(o, null); result(`${o.inca.name}: merged ${before - o.inca.mesh.v.length} vertices`); showHistoryEditor(h); }
  echo(`polyMergeVertex -d ${th} -am 1 -ch 1;`);
}, { icon: 'merge', options: [{ key: 'threshold', label: 'Threshold', type: 'float', value: 0.001 }] });
C('mergeToCenter', 'Merge to Center', () => {
  ck('merge to center');
  for (const o of App.hilite.filter(o => o.inca.kind === 'mesh')) {
    const pm = o.inca.mesh; const cs = o.inca.compSel; let groups = [];
    if (App.compMode === 'vertex') groups = [[...cs.v]];
    else if (App.compMode === 'edge') groups = connectedGroups([...cs.e].map(e => pm.topo.edges[e]));
    else if (App.compMode === 'face') groups = connectedGroups([...cs.f].flatMap(f => { const F = pm.f[f]; return F.map((v, i) => [v, F[(i + 1) % F.length]]); }));
    if (!groups.length || !groups[0].length) continue;
    S.applyOp(o, 'polyMergeCenter', { groups }); cs.v.clear(); cs.e.clear(); cs.f.clear(); afterOp(o, null);
  }
  echo('polyMergeVertex -d 1000 -am 1 -ch 1;');
}, { icon: 'mergeCenter' });
C('collapse', 'Collapse', () => {
  ck('collapse');
  for (const o of App.hilite.filter(o => o.inca.kind === 'mesh')) { const pm = o.inca.mesh; const cs = o.inca.compSel; let groups = []; if (App.compMode === 'edge') groups = [...cs.e].map(e => pm.topo.edges[e].slice()); else if (App.compMode === 'face') groups = [...cs.f].map(f => pm.f[f].slice()); if (!groups.length) continue; S.applyOp(o, 'polyMergeCenter', { groups }); cs.e.clear(); cs.f.clear(); afterOp(o, null); }
  echo('polyCollapseEdge;');
});
function connectedGroups(pairs) {
  const parent = new Map(); const find = (x) => { while (parent.get(x) !== x) x = parent.get(x); return x; };
  for (const [a, b] of pairs) { if (!parent.has(a)) parent.set(a, a); if (!parent.has(b)) parent.set(b, b); const ra = find(a), rb = find(b); if (ra !== rb) parent.set(ra, rb); }
  const g = new Map(); for (const v of parent.keys()) { const r = find(v); if (!g.has(r)) g.set(r, []); g.get(r).push(v); }
  return [...g.values()];
}
C('poke', 'Poke', () => { ck('poke'); for (const o of compTargets('face')) { const { select, h } = S.applyOp(o, 'polyPoke', { faces: [...o.inca.compSel.f] }); afterOp(o, select); } echo('polyPoke -ws 1 -ch 1;'); }, { icon: 'poke' });
C('triangulate', 'Triangulate', () => { ck('triangulate'); const ts = App.compMode === 'face' ? compTargets('face') : App.sel.filter(o => o.inca.kind === 'mesh'); for (const o of ts) { S.applyOp(o, 'polyTriangulate', { faces: App.compMode === 'face' ? [...o.inca.compSel.f] : null }); afterOp(o, null); } echo('polyTriangulate -ch 1;'); }, { icon: 'triangulate' });
C('quadrangulate', 'Quadrangulate', () => { ck('quadrangulate'); for (const o of (App.compMode ? App.hilite : App.sel).filter(o => o.inca.kind === 'mesh')) { S.applyOp(o, 'polyQuad', { angle: 30 }); afterOp(o, null); } echo('polyQuad -a 30 -kgb 1 -ktb 1 -khe 1 -ws 1 -ch 1;'); }, { icon: 'quadrangulate' });
C('reverseNormals', 'Reverse', () => { ck('reverse'); const ts = App.compMode === 'face' ? compTargets('face') : App.sel.filter(o => o.inca.kind === 'mesh'); for (const o of ts) S.applyOp(o, 'polyNormal', { faces: App.compMode === 'face' ? [...o.inca.compSel.f] : null }); echo('polyNormal -normalMode 0 -userNormalMode 0 -ch 1;'); }, { icon: 'reverse' });
C('fillHole', 'Fill Hole', () => {
  ck('fill hole'); const ts = App.compMode ? App.hilite.filter(o => o.inca.kind === 'mesh') : App.sel.filter(o => o.inca.kind === 'mesh');
  for (const o of ts) { const edges = App.compMode === 'edge' ? Sel.edgesAsPairs(o) : null; const { select } = S.applyOp(o, 'polyCloseBorder', { edges }); if (App.compMode) afterOp(o, select); }
  echo('polyCloseBorder -ch 1;');
}, { icon: 'fillHole' });
C('mirror', 'Mirror', (a) => {
  const o0 = { axis: 0, mode: 'object', direction: 1, merge: true, threshold: 0.001, ...opts('mirror'), ...a };
  const s = App.sel.filter(o => o.inca.kind === 'mesh'); if (!s.length) return warn('// Warning: Select a polygon object to mirror');
  ck('mirror');
  for (const o of s) {
    let pos = 0;
    if (o0.mode === 'bbox') { const b = o.inca.mesh.bbox(); pos = o0.direction > 0 ? b.max[o0.axis] : b.min[o0.axis]; }
    else if (o0.mode === 'world') { const inv = S.worldMatrix(o).invert(); const p = new THREE.Vector3().applyMatrix4(inv); pos = p.getComponent(o0.axis); }
    const { h } = S.applyOp(o, 'polyMirror', { axis: o0.axis, position: pos, merge: o0.merge, threshold: o0.threshold });
    showHistoryEditor(h);
  }
  echo(`polyMirrorFace -ws 1 -direction ${o0.axis * 2} -mergeMode ${o0.merge ? 1 : 0} -ch 1;`);
}, { icon: 'mirror', options: [{ key: 'mode', label: 'Mirror axis position', type: 'radio', options: ['Bounding Box', 'Object', 'World'], values: ['bbox', 'object', 'world'], value: 'object' }, { key: 'axis', label: 'Mirror axis', type: 'radio', options: ['X', 'Y', 'Z'], values: [0, 1, 2], value: 0 }, { key: 'direction', label: 'Mirror direction', type: 'radio', options: ['+', '-'], values: [1, -1], value: 1 }, { key: 'merge', label: 'Merge border vertices', type: 'bool', value: true }, { key: 'threshold', label: 'Merge threshold', type: 'float', value: 0.001 }] });
C('smooth', 'Smooth', (a) => { const d = { divisions: 1, ...opts('smooth'), ...a }.divisions; const s = App.sel.filter(o => o.inca.kind === 'mesh'); if (!s.length) return warn('// Warning: Select a polygon object to smooth'); ck('smooth'); let lastH; exitComponentMode(); for (const o of s) { lastH = S.applyOp(o, 'polySmoothFace', { divisions: d }).h; } echo(`polySmooth -mth 0 -sdt 2 -ovb 1 -ofb 3 -ofc 0 -ost 0 -ocr 0 -dv ${d} -bnr 1 -c 1 -kb 1 -ksb 1 -khe 0 -kt 1 -kmb 1 -suv 1 -peh 0 -sl 1 -dpe 1 -ps 0.1 -ro 1 -ch 1;`); result(lastH.name); showHistoryEditor(lastH); }, { icon: 'smooth', options: [{ key: 'divisions', label: 'Division levels', type: 'int', value: 1 }] });
C('combine', 'Combine', () => { const s = App.sel.filter(o => { let m = false; o.traverse(c => { if (c.inca?.kind === 'mesh') m = true; }); return m; }); if (s.length < 2) return warn('// Warning: Select at least two polygon objects to combine'); ck('combine'); exitComponentMode(); const o = S.combine(s); Sel.select([o], 'replace', { echo: false }); echo(`polyUnite -ch 1 -mergeUVSets 1 -centerPivot -name ${o.inca.name};`); result(o.inca.name); }, { icon: 'combine' });
C('separate', 'Separate', () => { const s = App.sel.filter(o => o.inca.kind === 'mesh'); if (!s.length) return warn('// Warning: Select a polygon object to separate'); ck('separate'); exitComponentMode(); const out = []; for (const o of s) out.push(...S.separate(o)); Sel.select(out, 'replace', { echo: false }); echo('polySeparate -ch 1;'); }, { icon: 'separate' });
C('extract', 'Extract', () => { const ts = compTargets('face'); if (!ts.length) return warn('// Warning: Select faces to extract'); ck('extract'); const out = []; for (const o of ts) { out.push(S.extractFaces(o, [...o.inca.compSel.f])); o.inca.compSel.f.clear(); } Sel.setMode(null); Sel.select(out); echo('polyChipOff -ch 1 -kft 1 -dup 0 -off 0;'); }, { icon: 'extract' });
C('duplicateFaces', 'Duplicate', () => { const ts = compTargets('face'); if (!ts.length) return warn('// Warning: Select faces to duplicate'); ck('duplicate faces'); const out = []; for (const o of ts) out.push(S.extractFaces(o, [...o.inca.compSel.f], { duplicate: true })); Sel.setMode(null); Sel.select(out); echo('polyChipOff -ch 1 -kft 1 -dup 1 -off 0;'); }, { icon: 'duplicateFace' });
C('cleanup', 'Cleanup', () => { ck('cleanup'); for (const o of App.sel.filter(o => o.inca.kind === 'mesh')) { S.applyOp(o, 'polyMergeVert', { verts: null, threshold: 1e-4 }); } echo('polyCleanupArgList 4 { "0","1","1","0","0","0","0","0","0","1e-05","0","1e-05","0","1e-05","0","-1","0","0" };'); });
C('connect', 'Connect', () => {
  ck('connect');
  for (const o of App.hilite.filter(o => o.inca.kind === 'mesh')) {
    const pm = o.inca.mesh; const cs = o.inca.compSel;
    if (App.compMode === 'vertex' && cs.v.size >= 2) { const vs = [...cs.v]; const pts = []; for (const v of vs) pts.push({ vertex: v }); S.applyOp(o, 'polySplit', { points: pts }); }
    else if (App.compMode === 'edge' && cs.e.size >= 2) {
      // insert a loop-like connection through the midpoints of the selected edges (ordered by shared faces)
      const es = [...cs.e].map(e => pm.topo.edges[e]); const order = [es[0]]; const used = new Set([0]);
      const shares = (a, b) => pm.topo.edgeFaces[pm.edgeIndex(...a)].some(f => pm.topo.edgeFaces[pm.edgeIndex(...b)].includes(f));
      while (order.length < es.length) { const last = order[order.length - 1]; const i = es.findIndex((e, k) => !used.has(k) && shares(last, e)); if (i < 0) break; used.add(i); order.push(es[i]); }
      S.applyOp(o, 'polySplit', { points: order.map(e => ({ edge: e, t: 0.5 })) });
    } else continue;
    cs.v.clear(); cs.e.clear(); afterOp(o, null);
  }
  echo('polyConnectComponents -ch 1;');
});
C('softenEdge', 'Soften Edge', () => softHard(180), { icon: 'softEdge' });
C('hardenEdge', 'Harden Edge', () => softHard(0), { icon: 'hardEdge' });
C('softHardAngle', 'Soften/Harden Edges (30°)', () => softHard(30));
function softHard(angle) {
  ck('soft/hard');
  const ts = App.compMode === 'edge' ? compTargets('edge') : (App.compMode ? App.hilite : App.sel).filter(o => o.inca.kind === 'mesh');
  for (const o of ts) S.applyOp(o, 'polySoftEdge', { angle, edges: App.compMode === 'edge' ? Sel.edgesAsPairs(o) : null });
  echo(`polySoftEdge -a ${angle} -ch 1;`);
}
for (const [id, mode, label, icon] of [['planarMap', 'planar', 'Planar', 'planarMap'], ['cylindricalMap', 'cylindrical', 'Cylindrical', 'planarMap'], ['sphericalMap', 'spherical', 'Spherical', 'planarMap'], ['automaticMap', 'automatic', 'Automatic', 'autoMap']]) {
  C(id, label, (a) => { ck('uv ' + mode); const ts = App.compMode === 'face' ? compTargets('face') : (App.compMode ? App.hilite : App.sel).filter(o => o.inca.kind === 'mesh'); for (const o of ts) { const { h } = S.applyOp(o, 'polyProj', { mode, axis: (a && a.axis) ?? (opts(id).axis ?? 1), faces: App.compMode === 'face' ? [...o.inca.compSel.f] : null }); showHistoryEditor(h); } echo(`polyProjection -ch 1 -type ${label} -ibd on -sf on;`); }, { icon, options: mode === 'planar' ? [{ key: 'axis', label: 'Project from', type: 'radio', options: ['X axis', 'Y axis', 'Z axis'], values: [0, 1, 2], value: 1 }] : null });
}

// ------------------------------------------------------------------ curves -> surfaces
function curveSel(n = 1) { const c = App.sel.filter(o => o.inca.kind === 'curve'); if (c.length < n) { warn(`// Warning: Select ${n === 1 ? 'a curve' : n + ' or more curves'}`); return null; } return c; }
C('revolve', 'Revolve', (a) => {
  const cs = curveSel(1); if (!cs) return; const o0 = { axis: 1, startSweep: 0, endSweep: 360, sections: 8, segments: 8, ...opts('revolve'), ...a };
  ck('revolve'); const out = [];
  for (const c of cs) { const o = S.createMesh(null, {}, { name: 'revolvedSurface1' }); o.inca.nurbs = true; S.addHistory(o, 'revolve', { curve: c.inca.id, ...o0 }, { noEval: true }); S.evaluate(o, 0); out.push(o); showHistoryEditor(o.inca.history[0]); }
  Sel.select(out, 'replace', { echo: false }); echo(`revolve -ch 1 -po 0 -rn 0 -ssw ${o0.startSweep} -esw ${o0.endSweep} -ut 0 -tol 0.01 -degree 3 -s ${o0.sections} -ulp 1 -ax 0 1 0;`); result(out.map(o => o.inca.name).join(' '));
}, { icon: 'revolve', options: [{ key: 'axis', label: 'Axis', type: 'radio', options: ['X', 'Y', 'Z'], values: [0, 1, 2], value: 1 }, { key: 'startSweep', label: 'Start sweep angle', type: 'float', value: 0 }, { key: 'endSweep', label: 'End sweep angle', type: 'float', value: 360 }, { key: 'sections', label: 'Segments', type: 'int', value: 8 }] });
C('loft', 'Loft', (a) => { const cs = curveSel(2); if (!cs) return; ck('loft'); const o = S.createMesh(null, {}, { name: 'loftedSurface1' }); o.inca.nurbs = true; S.addHistory(o, 'loft', { curves: cs.map(c => c.inca.id), segments: 8, close: false, ...opts('loft'), ...a }, { noEval: true }); S.evaluate(o, 0); Sel.select([o], 'replace', { echo: false }); echo('loft -ch 1 -u 1 -c 0 -ar 1 -d 3 -ss 1 -rn 0 -po 0 -rsn true;'); result(o.inca.name); showHistoryEditor(o.inca.history[0]); }, { icon: 'loft' });
C('extrudeCurve', 'Extrude', (a) => { const cs = curveSel(2); if (!cs) return; ck('extrude'); const o = S.createMesh(null, {}, { name: 'extrudedSurface1' }); o.inca.nurbs = true; S.addHistory(o, 'extrude', { profile: cs[0].inca.id, path: cs[1].inca.id, segments: 8, scale: 1, twist: 0, ...opts('extrudeCurve'), ...a }, { noEval: true }); S.evaluate(o, 0); Sel.select([o], 'replace', { echo: false }); echo('extrude -ch true -rn false -po 0 -et 2 -ucp 1 -fpt 1 -upn 1 -rotation 0 -scale 1 -rsp 1;'); result(o.inca.name); showHistoryEditor(o.inca.history[0]); }, { icon: 'extrudeCurve' });
C('planarSurface', 'Planar', () => { const cs = curveSel(1); if (!cs) return; ck('planar'); const o = S.createMesh(null, {}, { name: 'planarTrimmedSurface1' }); S.addHistory(o, 'planarTrim', { curves: cs.map(c => c.inca.id), segments: 8 }, { noEval: true }); S.evaluate(o, 0); Sel.select([o], 'replace', { echo: false }); echo('planarSrf -ch 1 -d 3 -ko 0 -tol 0.01 -rn 0 -po 0;'); result(o.inca.name); }, { icon: 'planar' });
C('rebuildCurve', 'Rebuild Curve', () => { const cs = curveSel(1); if (!cs) return; ck('rebuild'); for (const c of cs) { const pts = c.inca.curve.sample(4); const n = Math.max(4, Math.round(c.inca.curve.cvs.length)); const step = (pts.length - 1) / (n - 1); c.inca.curve.cvs = Array.from({ length: n }, (_, i) => pts[Math.round(i * step)].slice()); S.rebuildShape(c); } });
C('reverseCurve', 'Reverse Direction', () => { const cs = curveSel(1); if (!cs) return; ck('reverse curve'); for (const c of cs) { c.inca.curve.cvs.reverse(); S.rebuildShape(c); } });
C('openCloseCurve', 'Open/Close', () => { const cs = curveSel(1); if (!cs) return; ck('open/close'); for (const c of cs) { c.inca.curve.form = c.inca.curve.form === 'periodic' ? 'open' : 'periodic'; S.rebuildShape(c); } });

// ------------------------------------------------------------------ animation
C('setKey', 'Set Key', () => {
  const s = App.compMode ? [] : App.sel; if (!s.length) return warn('// Warning: Nothing selected to key');
  ck('set key'); for (const o of s) App.anim.keyNode(o);
  echo(`setKeyframe -breakdown 0 -preserveCurveShape 0 -hierarchy none -controlPoints 0 -shape 0 {"${s.map(o => o.inca.name).join('","')}"};`);
  result(String(s.length * 10));
}, { hk: 'S', icon: 'setKey' });
const keyAttrs = (label, attrs, hk, icon) => C('key' + label, 'Key ' + label, () => { const s = App.sel; if (!s.length) return; ck('key'); for (const o of s) App.anim.keyNode(o, attrs); echo(`setKeyframe -at ${attrs[0].replace(/X$/, '')};`); }, { hk, icon });
keyAttrs('Translate', ['translateX', 'translateY', 'translateZ'], 'Shift+W', 'keyTranslate');
keyAttrs('Rotate', ['rotateX', 'rotateY', 'rotateZ'], 'Shift+E', 'keyRotate');
keyAttrs('Scale', ['scaleX', 'scaleY', 'scaleZ'], 'Shift+R', 'keyScale');
C('playToggle', 'Play/Stop', () => App.timeline?.togglePlay(), { hk: 'Alt+V', icon: 'playFwd', noRepeat: true });
C('playBackwards', 'Play Backwards', () => App.timeline?.togglePlay(-1), { noRepeat: true });
C('stopPlayback', 'Stop', () => App.timeline?.stop(), { noRepeat: true });
C('nextFrame', 'Next Frame', () => App.setTime(App.time.current + 1 > App.time.end ? App.time.start : App.time.current + 1), { hk: 'Alt+.', noRepeat: true });
C('prevFrame', 'Previous Frame', () => App.setTime(App.time.current - 1 < App.time.start ? App.time.end : App.time.current - 1), { hk: 'Alt+,', noRepeat: true });
C('nextKey', 'Next Key', () => { const t = App.anim.nextKeyTime(App.sel, App.time.current, 1); if (t !== null) App.setTime(t); }, { hk: '.', noRepeat: true });
C('prevKey', 'Previous Key', () => { const t = App.anim.nextKeyTime(App.sel, App.time.current, -1); if (t !== null) App.setTime(t); }, { hk: ',', noRepeat: true });
C('goToStart', 'Go to Start', () => App.setTime(App.time.start), { hk: 'Shift+Alt+V', noRepeat: true });
C('goToEnd', 'Go to End', () => App.setTime(App.time.end), { noRepeat: true });
C('toggleAutoKey', 'Auto Key', () => { App.time.autoKey = !App.time.autoKey; App.dirty('timeline'); }, { icon: 'autoKey', noRepeat: true });
C('deleteKeysCurrent', 'Delete Keys (current frame)', () => { ck('delete keys'); App.anim.deleteKeysAt(App.sel, App.time.current); App.anim.evaluate(); });
C('copyKeys', 'Copy Keys', () => App.anim.copyKeys(App.sel, null), { noRepeat: true });
C('pasteKeys', 'Paste Keys', () => { ck('paste keys'); App.anim.pasteKeys(App.sel); App.anim.evaluate(); });
C('bakeKeys', 'Bake Simulation', () => { const s = requireSel(); if (!s) return; ck('bake'); for (let t = App.time.start; t <= App.time.end; t++) { App.anim.evaluate(t); for (const o of s) App.anim.keyNode(o, null, { t }); } App.anim.evaluate(); });

// ------------------------------------------------------------------ materials
C('assignNewMaterial', 'Assign New Material...', (a) => { const type = a.type; if (!type) return App.ui.assignNewMaterialDialog(); return assignMaterial(createMaterial(type)); }, { icon: 'standardSurface' });
for (const t of Object.keys(MAT_TYPES)) C('assignNew_' + t, MAT_TYPES[t].label, () => Cmds.run('assignNewMaterial', { type: t }), { icon: t });
C('createMaterial', 'Create Material', (a) => { ck('create material'); const m = createMaterial(a.type || 'lambert'); App.dirty('hypershade'); echo(`shadingNode -asShader ${m.type};`); result(m.name); return m; }, { noRepeat: true });
export function assignMaterial(mat, targets = null) {
  ck('assign material');
  if (App.compMode === 'face' && !targets) {
    for (const o of compTargets('face')) { const pm = o.inca.mesh; if (!pm.fm) pm.fm = pm.f.map(() => null); for (const f of o.inca.compSel.f) pm.fm[f] = mat.id === o.inca.material ? null : mat.id; const H = o.inca.history; const last = H[H.length - 1]; if (last) last._out = pm; bakeFaceMats(o); S.rebuildShape(o); }
  } else {
    const list = targets || App.sel.flatMap(o => { const r = []; o.traverse(c => { if (c.inca?.kind === 'mesh') r.push(c); }); return r; });
    for (const o of list) { o.inca.material = mat.id; if (o.inca.mesh.fm) o.inca.mesh.fm = null; bakeFaceMats(o); S.rebuildShape(o); }
  }
  echo(`hyperShade -assign ${mat.name};`); App.dirty('hypershade', 'attr');
  return mat;
}
// face material assignments must survive history re-evaluation: store them as a history entry
function bakeFaceMats(o) {
  const H = o.inca.history; const fm = o.inca.mesh.fm ? o.inca.mesh.fm.slice() : null;
  const last = H[H.length - 1];
  if (last && last.type === 'meshData') { last.params.data = o.inca.mesh.toJSON(); return; }
  if (last && last.type === 'polyTweak') { /* keep */ }
  // add a hidden face-material node
  const prev = H.find(h => h.type === 'faceMats');
  if (prev) { prev.params.fm = fm; const i = H.indexOf(prev); if (i !== H.length - 1) { H.splice(i, 1); H.push(prev); } }
  else S.addHistory(o, 'faceMats', { fm }, { noEval: true });
  H[H.length - 1]._out = o.inca.mesh;
}
OPS.faceMats = { hidden: true, apply: (pm, p) => { const m = pm.clone(); m.fm = p.fm && p.fm.length === m.f.length ? p.fm.slice() : (p.fm ? m.f.map((_, i) => p.fm[i] ?? null) : null); return { mesh: m }; } };
C('assignExisting', 'Assign Existing Material', (a) => { const m = App.mats.get(a.id); if (m) assignMaterial(m); });
C('materialAttributes', 'Material Attributes...', () => { const o = Sel.lead(); if (!o || o.inca.kind !== 'mesh') return; App.ui.showAttr(App.mats.get(o.inca.material)); });

// ------------------------------------------------------------------ windows
C('attributeEditor', 'Attribute Editor', () => App.ui.toggleRightPanel('attr'), { hk: 'Ctrl+A', icon: 'attrEditor', noRepeat: true });
C('channelBox', 'Channel Box / Layer Editor', () => App.ui.toggleRightPanel('channels'), { icon: 'channelBox', noRepeat: true });
C('toolSettings', 'Tool Settings', () => App.ui.toggleToolSettings(), { icon: 'toolSettings', noRepeat: true });
C('toolSettingsForCurrent', 'Tool Settings', () => App.ui.toggleToolSettings(true), { noRepeat: true });
C('modelingToolkit', 'Modeling Toolkit', () => App.ui.toggleRightPanel('mtk'), { icon: 'modelingToolkit', noRepeat: true });
C('outlinerWindow', 'Outliner', () => App.ui.toggleOutlinerDock(), { icon: 'outliner', noRepeat: true });
C('hypershade', 'Hypershade', () => App.ui.hypershade(), { icon: 'hypershade', noRepeat: true });
C('renderView', 'Render View', () => App.ui.renderView(), { icon: 'renderView', noRepeat: true });
C('renderCurrent', 'Render Current Frame...', () => App.ui.renderView(true), { icon: 'renderFrame', noRepeat: true });
C('ipr', 'IPR Render Current Frame...', () => App.ui.renderView(true, { ipr: true }), { icon: 'ipr', noRepeat: true });
C('renderSettings', 'Render Settings...', () => App.ui.renderSettings(), { icon: 'renderSettings', noRepeat: true });
C('graphEditor', 'Graph Editor', () => App.ui.graphEditor(), { icon: 'graphEditor', noRepeat: true });
C('dopeSheet', 'Dope Sheet', () => App.ui.dopeSheet(), { icon: 'dopeSheet', noRepeat: true });
C('scriptEditor', 'Script Editor', () => App.ui.scriptEditor(), { icon: 'scriptEditor', noRepeat: true });
C('hypergraph', 'Hypergraph: Hierarchy', () => App.ui.hypergraph(), { noRepeat: true });
C('preferences', 'Preferences...', () => App.ui.preferences(), { noRepeat: true });
C('hotkeyEditor', 'Hotkey Editor...', () => App.ui.hotkeyEditor(), { noRepeat: true });
C('shelfEditor', 'Shelf Editor...', () => App.ui.shelfEditor(), { noRepeat: true });
C('uvEditor', 'UV Editor', () => App.ui.uvEditor(), { icon: 'uvEditor', noRepeat: true });
C('playblast', 'Playblast', () => App.ui.playblast(), { icon: 'playblast', noRepeat: true });
C('about', 'About Inca', () => App.ui.about(), { noRepeat: true });
C('hotkeyList', 'Keyboard Shortcuts', () => App.ui.hotkeyEditor(), { noRepeat: true });

// file commands are registered in io.js
export { requireSel, echo, result, warn, opts, ck };
