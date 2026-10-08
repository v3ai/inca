// Inca — shelves
import { App } from '../core/app.js';
import { h, showMenu, promptDialog, toast } from './dom.js';
import { icon } from './icons.js';
import { runMEL, runJS } from '../core/mel.js';

const it = (cmd, extra = {}) => ({ cmd, ...extra });
const SEP = { sep: true };
export const DEFAULT_SHELVES = () => [
  { name: 'Curves / Surfaces', items: [it('cvCurveTool'), it('epCurveTool'), it('pencilCurveTool'), it('nurbsCircle'), it('nurbsSquare'), SEP, it('nurbsSphere'), it('nurbsCube'), it('nurbsCylinder'), it('nurbsCone'), it('nurbsPlane'), it('nurbsTorus'), SEP, it('revolve'), it('loft'), it('planarSurface'), it('extrudeCurve'), SEP, it('rebuildCurve', { icon: 'cvCurve', label: 'Rbld' }), it('reverseCurve', { icon: 'epCurve', label: 'Rev' }), it('convertNurbsToPoly', { icon: 'polyCube', label: 'N→P' })] },
  { name: 'Poly Modeling', items: [it('polySphere'), it('polyCube'), it('polyCylinder'), it('polyCone'), it('polyTorus'), it('polyPlane'), it('polyDisc'), it('polyPlatonic'), it('polyPyramid'), it('polyPipe'), it('polyHelix'), SEP, it('combine'), it('separate'), it('mirror'), it('smooth'), it('booleanUnion', { icon: 'boolean' }), SEP, it('extrude'), it('bridge'), it('bevel'), it('addDivisions'), SEP, it('multiCutTool'), it('targetWeldTool'), it('insertEdgeLoopTool'), it('quadDrawTool', { icon: 'polyPlane', label: 'Quad' }), SEP, it('planarMap'), it('automaticMap'), it('uvEditor')] },
  { name: 'Sculpting', items: [it('sculptTool', { icon: 'softSelect', label: 'Sculpt' }), it('smoothSculptTool', { icon: 'smooth', label: 'Smooth' }), it('relaxSculptTool', { icon: 'smooth', label: 'Relax' }), it('grabSculptTool', { icon: 'move', label: 'Grab' }), it('pinchSculptTool', { icon: 'mergeCenter', label: 'Pinch' }), it('flattenSculptTool', { icon: 'polyPlane', label: 'Flat' }), it('inflateSculptTool', { icon: 'polySphere', label: 'Infl' }), SEP, it('softSelect')] },
  { name: 'Rigging', items: [it('jointTool', { icon: 'joint' }), it('ikHandleTool', { icon: 'joint', label: 'IK' }), it('smoothBind', { icon: 'joint', label: 'Bind' }), SEP, it('createLocator'), it('createCluster', { icon: 'locator', label: 'Clus' }), it('createLattice', { icon: 'nurbsCube', label: 'Lat' }), SEP, it('parentConstraint', { icon: 'parent', label: 'Par' }), it('pointConstraint', { icon: 'move', label: 'Pt' }), it('orientConstraint', { icon: 'rotate', label: 'Ori' }), it('scaleConstraint', { icon: 'scale', label: 'Scl' }), it('aimConstraint', { icon: 'camera', label: 'Aim' }), SEP, it('bendDeformer', { icon: 'polyCylinder', label: 'Bend' }), it('twistDeformer', { icon: 'polyHelix', label: 'Twist' }), it('flareDeformer', { icon: 'polyCone', label: 'Flare' }), it('sineDeformer', { icon: 'polyHelix', label: 'Sine' }), it('squashDeformer', { icon: 'polySphere', label: 'Sqsh' }), it('waveDeformer', { icon: 'polyPlane', label: 'Wave' })] },
  { name: 'Animation', items: [it('setKey'), it('keyTranslate'), it('keyRotate'), it('keyScale'), SEP, it('graphEditor'), it('dopeSheet'), it('playblast'), SEP, it('toggleAutoKey'), it('setDrivenKeyWindow', { icon: 'setKey', label: 'SDK' }), it('attachToMotionPath', { icon: 'epCurve', label: 'Path' }), it('bakeKeys', { icon: 'setKey', label: 'Bake' })] },
  { name: 'Rendering', items: [it('createCamera'), SEP, it('ambientLight'), it('directionalLight'), it('pointLight'), it('spotLight'), it('areaLight'), it('skyDomeLight'), SEP, it('assignNew_standardSurface'), it('assignNew_lambert'), it('assignNew_blinn'), it('assignNew_phong'), SEP, it('hypershade'), it('renderView'), it('renderCurrent'), it('ipr'), it('renderSettings')] },
  { name: 'FX', items: [it('createEmitter', { icon: 'pointLight', label: 'Emit' }), it('createParticles', { icon: 'polySoccer', label: 'nPart' }), it('gravityField', { icon: 'move', label: 'Grav' }), it('turbulenceField', { icon: 'polyHelix', label: 'Turb' }), it('radialField', { icon: 'pointLight', label: 'Rad' }), it('dragField', { icon: 'stop', label: 'Drag' }), SEP, it('createActiveRigidBody', { icon: 'polyCube', label: 'Act' }), it('createPassiveRigidBody', { icon: 'polyPlane', label: 'Pas' })] },
  { name: 'Custom', items: [] },
];

App.shelf = {
  shelves: [], current: 0,
  load() { const s = App.prefs.shelves; this.shelves = Array.isArray(s) && s.length ? JSON.parse(JSON.stringify(s)) : DEFAULT_SHELVES(); this.current = Math.min(this.shelves.length - 1, App.prefs.shelfCurrent || 1); },
  save() { App.prefs.shelves = JSON.parse(JSON.stringify(this.shelves)); App.prefs.shelfCurrent = this.current; App.savePrefs(); },
  addToCurrent(cmd, label) { const sh = this.shelves[this.current]; sh.items.push({ cmd }); this.save(); this.render(); toast(`Added "${label || cmd}" to ${sh.name} shelf`); App.emit('echo', `// Added ${cmd} to shelf ${sh.name}`); },
  runItem(item) {
    if (item.mel) return runMEL(item.mel);
    if (item.js) return runJS(item.js);
    if (item.cmd) return App.cmds.run(item.cmd);
  },
  reset() { this.shelves = DEFAULT_SHELVES(); this.save(); this.render(); },
  render() { renderShelf(); },
};
function itemInfo(item) {
  const c = item.cmd ? App.cmds.registry.get(item.cmd) : null;
  return { label: item.label || c?.label || item.cmd || 'script', icon: item.icon || c?.icon || (item.mel || item.js ? 'scriptEditor' : 'help'), help: c?.help || c?.label || item.label || (item.mel || item.js || ''), missing: item.cmd && !c, hasOpt: !!c?.options, short: item.label && (item.icon || !c?.icon) ? item.label : (item.label && c?.icon && item.icon ? item.label : null) };
}
function renderShelf() {
  const tabs = document.getElementById('shelf-tabs'); const items = document.getElementById('shelf-items');
  if (!tabs) return;
  tabs.innerHTML = ''; items.innerHTML = '';
  const S = App.shelf;
  S.shelves.forEach((sh, i) => {
    const t = h('div', { class: 'shelf-tab' + (i === S.current ? ' active' : ''), text: sh.name });
    t.addEventListener('click', () => { S.current = i; S.save(); renderShelf(); });
    t.addEventListener('dblclick', async () => { const n = await promptDialog('Rename Shelf', 'Shelf name:', sh.name); if (n) { sh.name = n; S.save(); renderShelf(); } });
    t.addEventListener('contextmenu', (e) => { e.preventDefault(); S.current = i; renderShelf(); shelfMenu(e.clientX, e.clientY); });
    tabs.append(t);
  });
  const sh = S.shelves[S.current]; if (!sh) return;
  sh.items.forEach((item, idx) => {
    if (item.sep) { items.append(h('div', { class: 'shelf-sep' })); return; }
    const inf = itemInfo(item);
    const el = h('div', { class: 'shelf-item', html: icon(inf.icon), title: inf.label + (inf.hasOpt ? '\nDouble-click for options' : '') });
    if (inf.short) el.append(h('span', { class: 'lab', text: inf.short }));
    if (inf.missing) el.style.opacity = '0.4';
    el.addEventListener('mouseenter', () => App.help(inf.help));
    el.addEventListener('click', () => { if (inf.missing) return App.emit('warning', `// Warning: ${item.cmd} is not available in this version of Inca`); S.runItem(item); });
    el.addEventListener('dblclick', () => { if (inf.hasOpt) App.cmds.option(item.cmd); });
    el.addEventListener('contextmenu', (e) => { e.preventDefault(); showMenu([
      { label: 'Open', fn: () => S.runItem(item) }, { label: 'Option Box', enabled: () => inf.hasOpt, fn: () => App.cmds.option(item.cmd) },
      '-', { label: 'Edit...', fn: () => App.ui.shelfEditor?.() }, { label: 'Move Left', fn: () => { if (idx > 0) { [sh.items[idx - 1], sh.items[idx]] = [sh.items[idx], sh.items[idx - 1]]; S.save(); renderShelf(); } } }, { label: 'Move Right', fn: () => { if (idx < sh.items.length - 1) { [sh.items[idx + 1], sh.items[idx]] = [sh.items[idx], sh.items[idx + 1]]; S.save(); renderShelf(); } } }, { label: 'Add Separator After', fn: () => { sh.items.splice(idx + 1, 0, { sep: true }); S.save(); renderShelf(); } },
      '-', { label: 'Delete', fn: () => { sh.items.splice(idx, 1); S.save(); renderShelf(); } },
    ], e.clientX, e.clientY); });
    // MMB drag to reorder (drop outside the shelf deletes, like dragging to the trash)
    el.addEventListener('mousedown', (e) => {
      if (e.button !== 1) return; e.preventDefault();
      const ghost = h('div', { class: 'shelf-item', html: icon(inf.icon), style: { position: 'fixed', zIndex: 20000, pointerEvents: 'none', opacity: '0.7', background: '#666' } }); document.body.append(ghost);
      const mm = (ev) => { ghost.style.left = ev.clientX - 17 + 'px'; ghost.style.top = ev.clientY - 17 + 'px'; };
      const mu = (ev) => {
        removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); ghost.remove();
        const host = document.getElementById('shelf-items'); const r = host.getBoundingClientRect();
        if (ev.clientY < r.top - 30 || ev.clientY > r.bottom + 30) { sh.items.splice(idx, 1); S.save(); renderShelf(); return; }
        const kids = [...host.children]; let to = kids.findIndex(k => { const kr = k.getBoundingClientRect(); return ev.clientX < kr.left + kr.width / 2; }); if (to < 0) to = kids.length;
        const [x] = sh.items.splice(idx, 1); if (to > idx) to--; sh.items.splice(to, 0, x); S.save(); renderShelf();
      };
      mm(e); addEventListener('mousemove', mm); addEventListener('mouseup', mu);
    });
    items.append(el);
  });
}
function shelfMenu(x, y) {
  const S = App.shelf;
  showMenu([
    { label: 'Shelf Tabs', check: () => !document.getElementById('shelf-tabs').classList.contains('hidden'), fn: () => document.getElementById('shelf-tabs').classList.toggle('hidden') },
    { label: 'Shelf Editor...', fn: () => App.ui.shelfEditor?.() }, '-',
    { label: 'Navigate Shelves', sub: () => S.shelves.map((s, i) => ({ label: s.name, check: () => i === S.current, fn: () => { S.current = i; S.save(); renderShelf(); } })) },
    { label: 'New Shelf', fn: async () => { const n = await promptDialog('Create New Shelf', 'Shelf name:', 'shelf' + (S.shelves.length + 1)); if (n) { S.shelves.push({ name: n, items: [] }); S.current = S.shelves.length - 1; S.save(); renderShelf(); } } },
    { label: 'Delete Shelf', fn: () => { if (S.shelves.length <= 1) return; S.shelves.splice(S.current, 1); S.current = Math.max(0, S.current - 1); S.save(); renderShelf(); } },
    { label: 'Load Shelf...', fn: async () => { const N = window.incaNative; if (!N) return; const p = await N.openDialog({ filters: [{ name: 'Inca Shelf', extensions: ['json'] }] }); if (!p) return; try { const d = JSON.parse(await N.readText(p)); S.shelves.push(d); S.current = S.shelves.length - 1; S.save(); renderShelf(); } catch (e) { App.emit('error', 'Could not load shelf: ' + e.message); } } },
    { label: 'Save Shelf...', fn: async () => { const N = window.incaNative; if (!N) return; const sh = S.shelves[S.current]; const p = await N.saveDialog({ defaultPath: 'shelf_' + sh.name.replace(/\W+/g, '_') + '.json', filters: [{ name: 'Inca Shelf', extensions: ['json'] }] }); if (p) N.writeText(p, JSON.stringify(sh, null, 2)); } },
    { label: 'Save All Shelves', fn: () => { S.save(); toast('Shelves saved'); } },
    '-', { label: 'Restore Default Shelves', fn: () => S.reset() },
  ], x, y);
}
App.on('beforeUI', () => {
  App.shelf.load();
  const btn = document.getElementById('shelf-menu-btn');
  btn.innerHTML = icon('gear'); btn.title = 'Shelf menu';
  btn.addEventListener('mousedown', (e) => { e.preventDefault(); const r = btn.getBoundingClientRect(); shelfMenu(r.left, r.bottom); });
  renderShelf();
});
App.on('refresh', (d) => { if (d.has('shelf')) renderShelf(); });
