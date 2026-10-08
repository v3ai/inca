// Inca — assorted commands referenced by menus, hotkeys and shelves
import * as THREE from 'three';
import { App } from '../core/app.js';
import * as S from '../core/scene.js';
import { Sel } from '../core/selection.js';
import { Undo } from '../core/undo.js';
import { Curve } from '../core/curves.js';
import { MEL } from '../core/mel.js';
import { h, FloatWin, promptDialog } from './dom.js';

const C = App.cmds.register;
const echo = (s) => App.emit('echo', s);

for (const [k, l, hk] of [['modeling', 'Modeling', 'F2'], ['rigging', 'Rigging', 'F3'], ['animation', 'Animation', 'F4'], ['fx', 'FX', 'F5'], ['rendering', 'Rendering', 'F6']]) C('menuSet' + l, l + ' Menu Set', () => App.ui.setMenuSet(k), { hk, noRepeat: true });
C('componentTypeCycle', 'Cycle Component Type', () => { const order = ['vertex', 'edge', 'face']; const i = order.indexOf(App.compMode); App.cmds.run(order[(i + 1) % 3] + 'Mode'); }, { noRepeat: true });
C('toggleUIElements', 'Hide/Show UI Elements', () => App.ui.toggleAllElements(), { noRepeat: true });
C('hypergraphConnections', 'Hypergraph: Connections', () => App.ui.hypergraph?.('connections'), { noRepeat: true });
C('renderSequence', 'Render Sequence...', () => App.ui.renderSequence?.(), { noRepeat: true });
C('selectByName', 'Select by Name...', async () => { const p = await promptDialog('Select by Name', 'Name (wildcards *):', '*'); if (!p) return; const re = new RegExp('^' + p.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*').replace(/\?/g, '.') + '$'); Sel.select(S.allDag().filter(o => re.test(o.inca.name))); }, { noRepeat: true });
C('selectSimilar', 'Select Similar', () => { const l = Sel.lead(); if (!l) return; const k = l.inca.kind; Sel.select(S.allDag().filter(o => o.inca.kind === k && (k !== 'light' || o.inca.light.type === l.inca.light.type) && o.visible)); });
const byKind = (k) => S.allDag().filter(o => (k === 'geometry' ? (o.inca.kind === 'mesh' || o.inca.kind === 'curve') : o.inca.kind === k) && !o.inca.startup);
for (const [k, l] of [['light', 'Lights'], ['camera', 'Cameras'], ['geometry', 'Geometry']]) {
  C('hideAll' + l, 'Hide ' + l, () => { Undo.checkpoint('hide'); for (const o of byKind(k)) S.setAttr(o, 'visibility', 0); App.dirty('outliner'); });
  C('showAll' + l, 'Show ' + l, () => { Undo.checkpoint('show'); for (const o of byKind(k)) S.setAttr(o, 'visibility', 1); App.dirty('outliner'); });
}
// normals / border display helpers drawn as overlays on the meshes
function toggleOverlay(key, buildFn) {
  App.displayFlags = App.displayFlags || {}; App.displayFlags[key] = !App.displayFlags[key];
  for (const o of S.allDag()) { if (o.inca.kind !== 'mesh') continue; const ud = o.userData; if (ud[key]) { o.remove(ud[key]); ud[key].geometry.dispose(); ud[key] = null; } }
  if (App.displayFlags[key]) for (const o of (App.sel.length ? App.sel : S.allDag())) { if (o.inca.kind !== 'mesh') continue; const obj = buildFn(o); obj.userData.helper = true; o.add(obj); o.userData[key] = obj; }
  App.requestRender();
}
C('toggleNormals', 'Face Normals', () => toggleOverlay('_normals', (o) => { const pm = o.inca.mesh; const p = []; const b = pm.bbox(); const L = Math.max(0.05, Math.hypot(b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]) * 0.05); pm.f.forEach((_, i) => { const c = pm.faceCenter(i), n = pm.faceNormal(i); p.push(...c, c[0] + n[0] * L, c[1] + n[1] * L, c[2] + n[2] * L); }); const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(p, 3)); return new THREE.LineSegments(g, new THREE.LineBasicMaterial({ color: 0xffff40 })); }), { noRepeat: true });
C('toggleBorderEdges', 'Border Edges', () => toggleOverlay('_border', (o) => { const pm = o.inca.mesh; const t = pm.topo; const p = []; t.edges.forEach(([a, b], i) => { if (t.edgeFaces[i].length === 1) p.push(...pm.v[a], ...pm.v[b]); }); const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(p, 3)); return new THREE.LineSegments(g, new THREE.LineBasicMaterial({ color: 0xff40ff, depthTest: false })); }), { noRepeat: true });
// polygon edges -> curve
C('curveFromEdges', 'Polygon Edges to Curve', () => {
  const o = App.hilite.find(x => x.inca.kind === 'mesh' && x.inca.compSel.e.size); if (!o || App.compMode !== 'edge') return App.emit('warning', '// Warning: Select a chain of edges');
  const pm = o.inca.mesh; const es = [...o.inca.compSel.e].map(e => pm.topo.edges[e]);
  const adj = new Map(); for (const [a, b] of es) { (adj.get(a) || adj.set(a, []).get(a)).push(b); (adj.get(b) || adj.set(b, []).get(b)).push(a); }
  let start = [...adj.keys()].find(v => adj.get(v).length === 1) ?? es[0][0];
  const chain = [start]; const used = new Set(); let cur = start;
  while (true) { const nx = (adj.get(cur) || []).find(v => !used.has(Math.min(cur, v) + ':' + Math.max(cur, v))); if (nx === undefined) break; used.add(Math.min(cur, nx) + ':' + Math.max(cur, nx)); chain.push(nx); cur = nx; if (nx === start) break; }
  const closed = chain.length > 2 && chain[chain.length - 1] === chain[0]; if (closed) chain.pop();
  Undo.checkpoint('edges to curve'); o.updateWorldMatrix(true, false);
  const pts = chain.map(i => new THREE.Vector3(...pm.v[i]).applyMatrix4(o.matrixWorld).toArray());
  const c = S.createCurve(new Curve(pts, 1, closed ? 'periodic' : 'open', 'cv'), { name: 'polyToCurve1' });
  Sel.setMode(null); Sel.select([c]); echo('polyToCurve -form 2 -degree 1;');
});
// escape stops playback
addEventListener('keydown', (e) => { if (e.key === 'Escape' && App.timeline?.playing) { App.timeline.stop(); } });

// ------------------------------------------------------------------ help windows
const HELP_HTML = `
<h3>Navigating the viewport</h3>
<p><b>Alt + Left drag</b> tumble &nbsp; · &nbsp; <b>Alt + Middle drag</b> track (pan) &nbsp; · &nbsp; <b>Alt + Right drag</b> or <b>wheel</b> dolly (zoom)</p>
<p><b>F</b> frame selection · <b>A</b> frame all · <b>Space</b> (tap) toggles single/four view · <b>Space</b> (hold) shows the hotbox</p>
<h3>Selecting & transforming</h3>
<p><b>Q</b> select · <b>W</b> move · <b>E</b> rotate · <b>R</b> scale · <b>Y</b> last tool · <b>D</b>/<b>Insert</b> edit pivot · <b>+/-</b> manipulator size</p>
<p>Click to select, <b>Shift</b> toggle, <b>Ctrl</b> deselect, <b>Ctrl+Shift</b> add, drag for marquee. Middle-drag moves along the last used axis.</p>
<p>Hold <b>X</b> to snap to grid, <b>V</b> to snap to points, <b>J</b> for step rotation.</p>
<h3>Components</h3>
<p><b>Right-click</b> an object for its marking menu (Vertex / Edge / Face / Object Mode). <b>F8</b> object/component, <b>F9</b> vertex, <b>F10</b> edge, <b>F11</b> face. <b>Shift+Right-click</b> gives modeling tools. Double-click an edge to select its loop.</p>
<p><b>Ctrl+E</b> extrude · <b>Ctrl+B</b> bevel · <b>Ctrl+D</b> duplicate · <b>Ctrl+G</b> group · <b>P</b> parent · <b>B</b> soft select · <b>1/2/3</b> smooth preview</p>
<h3>Display</h3>
<p><b>4</b> wireframe · <b>5</b> shaded · <b>6</b> textured · <b>7</b> use all lights · <b>Alt+A</b> X-ray · <b>Ctrl+1</b> isolate select · <b>Ctrl+H</b> hide</p>
<h3>Animation</h3>
<p><b>S</b> set key · <b>Shift+W/E/R</b> key translate/rotate/scale · <b>Alt+V</b> play · <b>Esc</b> stop · <b>,</b>/<b>.</b> previous/next key · <b>Alt+,</b>/<b>Alt+.</b> previous/next frame</p>
<h3>Files</h3>
<p>Set a project first (<i>File > Project Window</i>) so scenes, textures and renders are organised in the standard folders.</p>`;
C('gettingStarted', 'Getting Started', () => { const w = new FloatWin('gettingStarted', 'Getting Started with Inca', { w: 640, h: 560 }); if (w.reused) return; w.body.append(h('div', { class: 'dlg-body', style: { lineHeight: '1.5', userSelect: 'text' }, html: HELP_HTML })); }, { noRepeat: true });
C('mouseHelp', 'Mouse & Navigation', () => App.cmds.run('gettingStarted'), { noRepeat: true });
C('melReference', 'MEL Command Reference', () => {
  const w = new FloatWin('melRef', 'MEL Command Reference', { w: 520, h: 560 }); if (w.reused) return;
  const names = Object.keys(MEL).sort(); const q = h('input', { class: 'wide', placeholder: 'filter...' }); const list = h('div', { class: 'list-box', style: { flex: '1', margin: '4px', fontFamily: 'Consolas, monospace' } });
  const draw = () => { list.innerHTML = ''; for (const n of names.filter(n => n.toLowerCase().includes(q.value.toLowerCase()))) list.append(h('div', { class: 'li', text: n })); };
  q.addEventListener('input', draw); q.addEventListener('keydown', e => e.stopPropagation());
  w.body.append(h('div', { style: { padding: '4px' } }, q), list, h('div', { class: 'dim', style: { padding: '4px 8px' }, text: 'All Inca menu commands can also be run by name, e.g. "polyCube -w 2;", "move -r 0 1 0;", or in JavaScript: cmds.polyCube({w:2})' }));
  draw();
}, { noRepeat: true });
