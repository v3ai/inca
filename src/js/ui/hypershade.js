// Inca — Hypershade: shading node browser, create panel, node-graph work area and property editor.
import { App } from '../core/app.js';
import { h, iconBtn, menuBar, showMenu, FloatWin, numField, colorPicker, colorToCss, toast, promptDialog } from './dom.js';
import { icon } from './icons.js';
import { MAT_TYPES, TEX_TYPES, MAPPABLE, createMaterial, createTexture, deleteShadingNode, updateMaterial, swatch } from '../core/materials.js';
import { assignMaterial } from '../core/commands.js';
import { Undo } from '../core/undo.js';
import { Sel } from '../core/selection.js';
import { allDag, rename, syncLight, rebuildShape } from '../core/scene.js';

// ------------------------------------------------------------------ shared tables
export const MAT_ICON = { standardSurface: 'standardSurface', lambert: 'lambert', blinn: 'blinn', phong: 'phong', surfaceShader: 'surfaceShader' };
export const TEX_ICON = { file: 'fileTex', checker: 'checker', ramp: 'ramp', noise: 'noise', grid: 'gridTex' };
const MAT_ORDER = ['standardSurface', 'lambert', 'blinn', 'phong', 'surfaceShader'];
const TEX_ORDER = ['file', 'checker', 'ramp', 'noise', 'grid'];
const LIGHT_TYPES = { ambientLight: 'Ambient Light', directionalLight: 'Directional Light', pointLight: 'Point Light', spotLight: 'Spot Light', areaLight: 'Area Light', skyDomeLight: 'Sky Dome Light' };
const ENUMS = { colorSpace: ['sRGB', 'Raw'], type: ['V Ramp', 'U Ramp', 'Circular Ramp'] };

// node-editor positions survive closing/reopening the window
const graphPos = new Map();

const niceName = (s) => String(s).replace(/([a-z0-9])([A-Z])/g, '$1 $2').replace(/^./, c => c.toUpperCase()).replace(/\bU V\b/, 'UV').replace(/\bI O R\b/, 'IOR');
const isColor = (v) => Array.isArray(v) && v.length === 3 && v.every(x => typeof x === 'number');
const echo = (s) => App.emit('echo', s);
const warn = (s) => { App.emit('warning', '// Warning: ' + s); toast(s); };

// swatch with a change key so swatches stay valid regardless of who edited the node
const texKey = (t) => t.type + JSON.stringify(t.attrs) + (t._hsLoad || 0);
function matKey(m) { return m.type + JSON.stringify(m.attrs) + JSON.stringify(m.maps || {}) + Object.values(m.maps || {}).map(id => { const t = App.texs.get(id); return t ? texKey(t) : ''; }).join('|'); }
export function nodeSwatch(rec) {
  const k = rec.kind === 'texture' ? texKey(rec) : matKey(rec);
  if (rec._hsKey !== k) { rec._swatch = null; rec._hsKey = k; }
  return swatch(rec, 64);
}
function usersOf(mat) {
  const out = [];
  for (const n of App.nodes.values()) if (n.isObject3D && n.inca && n.inca.kind === 'mesh') { if (n.inca.material === mat.id || (n.inca.mesh?.fm || []).includes(mat.id)) out.push(n); }
  return out;
}
const matsUsingTex = (tid) => [...App.mats.values()].filter(m => Object.values(m.maps || {}).includes(tid));

let cssDone = false;
function injectCss() {
  if (cssDone || document.getElementById('hs-extra-css')) { cssDone = true; return; }
  cssDone = true;
  document.head.append(h('style', { id: 'hs-extra-css', text: `
.hs { font-size: 11.5px; }
.hs .ptoolbar { border-bottom: 1px solid var(--line); gap: 2px; }
.hs .ptoolbar .tsep { width: 1px; height: 16px; background: #5a5a5a; margin: 0 4px; }
.hs .ptoolbar .ib { width: 22px; height: 22px; } .hs .ptoolbar .ib svg { width: 17px; height: 17px; }
.hs-left { width: 330px; flex: none; display: flex; flex-direction: column; border-right: 1px solid var(--line); min-height: 0; }
.hs-left .hs-create { width: auto; border-right: none; height: 230px; min-height: 60px; }
.hs-center { flex: 1; display: flex; flex-direction: column; min-width: 120px; min-height: 0; }
.hs-head { height: 20px; display: flex; align-items: center; gap: 6px; padding: 0 6px; background: #3a3a3a; color: #ddd; font-size: 11px; flex: none; border-bottom: 1px solid #2c2c2c; }
.hs-head .grow { flex: 1; }
.hs-head input { height: 16px; font-size: 11px; width: 110px; }
.hs-tab { padding: 2px 8px 3px; background: #383838; border-radius: 3px 3px 0 0; font-size: 11px; color: #b5b5b5; cursor: default; white-space: nowrap; }
.hs-tab:hover { background: #4a4a4a; }
.hs-tab.on, .hs-wtab { background: #5a5a5a; color: #fff; }
.hs-wtab { padding: 2px 8px 3px; border-radius: 3px 3px 0 0; font-size: 11px; }
.hs-tabs { border-bottom: 1px solid #5a5a5a; }
.hs-grid:focus, .hs-graph canvas:focus { outline: none; }
.hs-sw .ico { width: 64px; height: 64px; background: #2b2b2b; border: 1px solid #222; display: flex; align-items: center; justify-content: center; }
.hs-sw .ico svg { width: 40px; height: 40px; }
.hs-sw.drag { opacity: .5; }
.hs-split { height: 5px; flex: none; cursor: ns-resize; background: var(--bg); border-top: 1px solid var(--line); border-bottom: 1px solid var(--line); }
.hs-split:hover { background: #555; }
.hs-create .ci svg { width: 16px; height: 16px; flex: none; }
.hs-create .ci { cursor: default; }
.hs-create .ci.dim { color: #888; }
.hs-empty { color: #888; padding: 10px; font-style: italic; }
.hs-prop .pe-title { display: flex; align-items: center; gap: 6px; padding: 4px 6px; border-bottom: 1px solid #2c2c2c; flex: none; }
.hs-prop .pe-title svg { width: 18px; height: 18px; }
.hs-prop .pe-title input { flex: 1; }
.hs-prop .pe-body { flex: 1; overflow: auto; min-height: 0; }
.hs-fe-row { display: flex; align-items: center; gap: 4px; min-height: 22px; padding: 0 4px; }
.hs-fe-row > label { width: 108px; text-align: right; color: #c8c8c8; flex: none; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.hs-fe-row input:not([type]) { width: 64px; }
.hs-fe-row input.wide { flex: 1; width: auto; min-width: 40px; }
.hs-fe-row input[type=range] { flex: 1; min-width: 30px; }
.hs-fe-row .csw { width: 44px; height: 16px; border: 1px solid #222; flex: none; }
.hs-fe-row .map { width: 18px; height: 18px; flex: none; } .hs-fe-row .map svg { width: 14px; height: 14px; }
.hs-fe-row .lnk { color: #8fc3ea; text-decoration: underline; cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.hs-drag-ghost { position: fixed; z-index: 30000; pointer-events: none; width: 48px; height: 48px; opacity: .85; border: 1px solid #9cd0f5; box-shadow: 0 2px 8px rgba(0,0,0,.6); background: #2b2b2b; }
.hs-anm .list-box { margin: 6px; flex: 1; }
.hs-anm .li { display: flex; align-items: center; gap: 6px; height: 22px; }
.hs-anm .li svg { width: 16px; height: 16px; }
` }));
}

// ------------------------------------------------------------------ public entry points
let HS = null;
export function openHypershade() {
  injectCss();
  const win = new FloatWin('hypershade', 'Hypershade', { w: 1000, h: 640, minW: 640, minH: 360 });
  if (win.reused && HS) { HS.refreshAll(); return win; }
  HS = new Hypershade(win);
  win.onClose = () => { if (HS) HS.destroy(); HS = null; };
  return win;
}

export function createTextureNode(type = 'file', { graph = true } = {}) {
  if (!TEX_TYPES[type]) type = 'file';
  Undo.checkpoint('create ' + type);
  const t = createTexture(type);
  echo(`shadingNode -asTexture ${type};`); App.emit('result', t.name);
  App.dirty('hypershade');
  if (graph && HS) { HS.graph.ids.add(t.id); HS.select(t.id); }
  return t;
}
export function createMaterialNode(type = 'lambert') {
  if (!MAT_TYPES[type]) type = 'lambert';
  Undo.checkpoint('create ' + type);
  const m = createMaterial(type);
  echo(`shadingNode -asShader ${type};`); App.emit('result', m.name);
  App.dirty('hypershade');
  if (HS) { HS.graphNetwork(m, true); HS.select(m.id); }
  return m;
}

export function assignNewMaterialDialog() {
  injectCss();
  const win = new FloatWin('assignNewMaterial', 'Assign New Material', { w: 300, h: 330, minW: 220, minH: 200 });
  if (win.reused) return win;
  const filter = h('input', { class: 'wide', placeholder: 'Search…' });
  const list = h('div', { class: 'list-box', style: { flex: 1 } });
  const draw = () => {
    list.innerHTML = '';
    const f = filter.value.trim().toLowerCase();
    list.append(h('div', { class: 'li', style: { color: '#ddd', fontWeight: 600 }, text: 'Surface' }));
    for (const t of MAT_ORDER) {
      if (f && !MAT_TYPES[t].label.toLowerCase().includes(f) && !t.toLowerCase().includes(f)) continue;
      list.append(h('div', { class: 'li', title: 'Assign a new ' + MAT_TYPES[t].label + ' to the selection', onclick: () => { win.close(); App.cmds.run('assignNewMaterial', { type: t }); } }, h('span', { html: icon(MAT_ICON[t]) }), MAT_TYPES[t].label));
    }
  };
  filter.addEventListener('input', draw);
  filter.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Escape') win.close(); });
  win.body.append(h('div', { class: 'hs-anm', style: { display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0 } },
    h('div', { class: 'dlg-row', style: { padding: '6px 6px 0' } }, filter), list,
    h('div', { class: 'dlg-buttons' }, h('button', { text: 'Close', onclick: () => win.close() }))));
  draw();
  setTimeout(() => filter.focus(), 10);
  return win;
}

// ------------------------------------------------------------------ Hypershade window
class Hypershade {
  constructor(win) {
    this.win = win;
    this.tab = 'materials';
    this.selId = null;
    this.filter = '';
    this.showCreate = true; this.propVisible = true;
    this.prop = null; this.propRecId = null;
    this.pendingSw = new Set(); this._swRaf = 0;
    this.offs = [];
    this.build();
    // seed graph with the lead object's material (or the first material)
    const lead = Sel.lead();
    const m = lead && lead.inca.kind === 'mesh' ? App.mats.get(lead.inca.material) : [...App.mats.values()].find(x => x.id !== App.defaultMatId) || App.mats.get(App.defaultMatId);
    if (m) { this.graphNetwork(m, false); this.select(m.id); }
    this.refreshAll();
    requestAnimationFrame(() => this.graph.frameAll());
    this.offs.push(App.on('refresh', (flags) => {
      if (flags.has('hypershade') || flags.has('outliner')) this.refreshAll();
      else if (flags.has('attr') && this.prop) this.prop.refresh();
    }));
    this.offs.push(App.on('materialChanged', (rec) => this.queueSwatch(rec)));
    this.offs.push(App.on('textureLoaded', (t) => { t._hsLoad = (t._hsLoad || 0) + 1; this.queueSwatch(t); for (const m of matsUsingTex(t.id)) this.queueSwatch(m); }));
    this.offs.push(App.on('sceneLoaded', () => { this.resolveAfterLoad(); }));
    this.offs.push(App.on('nodeDeleted', () => { if (this.tab === 'lights' || this.tab === 'cameras') this.drawBrowser(); }));
    win.onResize = () => this.graph.resize();
  }
  destroy() {
    for (const off of this.offs) off();
    this.offs = [];
    if (this.prop) { try { this.prop.destroy(); } catch (e) { console.warn(e); } this.prop = null; }
    this.graph.destroy();
    cancelAnimationFrame(this._swRaf);
  }
  rec(id) { return id ? (App.nodes.get(id) || null) : null; }

  // ---------------------------------------------------------------- DOM
  build() {
    const b = this.win.body;
    this.root = h('div', { class: 'hs' });
    const mb = h('div', { class: 'pmenubar' });
    menuBar(mb, this.menus());
    // toolbar
    const tb = h('div', { class: 'ptoolbar' },
      iconBtn('olMaterial', 'Graph materials on selected objects', () => this.graphSelectedObjects()),
      iconBtn('inputLink', 'Input and output connections of the selected node', () => { const r = this.rec(this.selId); if (r) this.graphNetwork(r.kind === 'texture' ? (matsUsingTex(r.id)[0] || r) : r, false); }),
      iconBtn('trash', 'Clear graph', () => { this.graph.ids.clear(); this.graph.sel.clear(); this.graph.draw(); }),
      h('div', { class: 'tsep' }),
      iconBtn('frameAll', 'Frame all (A)', () => this.graph.frameAll()),
      iconBtn('frameSel', 'Frame selection (F)', () => this.graph.frameSel()),
      iconBtn('grid', 'Rearrange graph', () => this.graph.rearrange()),
      this.swBtn = iconBtn('textured', 'Toggle swatches on graph nodes', () => { this.graph.showSwatches = !this.graph.showSwatches; this.swBtn.classList.toggle('on', this.graph.showSwatches); this.graph.draw(); }),
      h('div', { class: 'tsep' }),
      iconBtn('renderView', 'Render View', () => App.ui.renderView?.()),
      iconBtn('renderFrame', 'Render the current frame', () => App.ui.renderView?.(true)),
      iconBtn('renderSettings', 'Render Settings', () => App.ui.renderSettings?.()));
    this.swBtn.classList.add('on');
    // left column: browser + create
    this.tabsEl = h('div', { class: 'hs-tabs' });
    this.searchEl = h('input', { placeholder: 'Search…', title: 'Filter the browser by name' });
    this.searchEl.addEventListener('input', () => { this.filter = this.searchEl.value.trim().toLowerCase(); this.drawBrowser(); });
    this.gridEl = h('div', { class: 'hs-grid', tabindex: 0 });
    this.gridEl.addEventListener('mousedown', (e) => { if (e.target === this.gridEl && e.button === 0) { this.select(null); } });
    this.gridEl.addEventListener('contextmenu', (e) => e.preventDefault());
    this.browserEl = h('div', { class: 'hs-browser' }, h('div', { class: 'hs-head' }, h('span', { text: 'Browser' }), h('span', { class: 'grow' }), this.searchEl), this.tabsEl, this.gridEl);
    this.createFilter = h('input', { placeholder: 'Filter nodes…' });
    this.createFilter.addEventListener('input', () => this.drawCreate());
    this.createList = h('div', { class: 'cl' });
    this.createEl = h('div', { class: 'hs-create' }, h('div', { class: 'hs-head' }, h('span', { text: 'Create' }), h('span', { class: 'grow' }), this.createFilter), this.createList);
    const split = h('div', { class: 'hs-split', title: 'Drag to resize' });
    split.addEventListener('mousedown', (e) => {
      e.preventDefault(); const y0 = e.clientY, h0 = this.createEl.offsetHeight;
      const mm = (ev) => { this.createEl.style.height = Math.max(60, Math.min(this.leftEl.offsetHeight - 120, h0 - (ev.clientY - y0))) + 'px'; };
      const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu);
    });
    this.splitEl = split;
    this.leftEl = h('div', { class: 'hs-left' }, this.browserEl, split, this.createEl);
    // centre: work area
    const graphHost = h('div', { class: 'hs-graph' });
    this.workTitle = h('span', { text: 'untitled_1' });
    this.centerEl = h('div', { class: 'hs-center' }, h('div', { class: 'hs-head' }, h('span', { text: 'Work Area' }), h('span', { class: 'hs-wtab', style: { marginLeft: '8px' } }, this.workTitle), h('span', { class: 'grow' }),
      h('span', { class: 'dim', text: 'Alt+MMB pan · wheel zoom · drag ports to connect' })), graphHost);
    // right: property editor
    this.peTitle = h('div', { class: 'pe-title' });
    this.peBody = h('div', { class: 'pe-body' });
    this.propEl = h('div', { class: 'hs-prop' }, h('div', { class: 'hs-head' }, h('span', { text: 'Property Editor' })), this.peTitle, this.peBody);
    this.root.append(mb, tb, h('div', { class: 'hs-top' }, this.leftEl, this.centerEl, this.propEl));
    b.append(this.root);
    this.graph = new Graph(this, graphHost);
    // keys inside the window never reach the global hotkeys
    this.win.el.addEventListener('keydown', (e) => this.onKey(e));
    this.drawTabs(); this.drawCreate();
  }
  onKey(e) {
    const tag = e.target.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') { e.stopPropagation(); return; }
    const inGraph = e.target === this.graph.canvas;
    const k = e.key;
    let handled = true;
    if ((k === 'Delete' || k === 'Backspace') && !e.ctrlKey) { if (inGraph && this.graph.sel.size) this.deleteNodes([...new Set([...this.graph.sel].map(id => id.startsWith('sg:') ? id.slice(3) : id))].map(id => this.rec(id)).filter(Boolean)); else if (this.selId) this.deleteNodes([this.rec(this.selId)].filter(Boolean)); }
    else if ((k === 'f' || k === 'F') && !e.ctrlKey) this.graph.frameSel();
    else if ((k === 'a' || k === 'A') && !e.ctrlKey) this.graph.frameAll();
    else if (k === 'F2' && this.selId) this.renameNode(this.rec(this.selId));
    else if (k === 'Escape') { this.graph.sel.clear(); this.graph.draw(); }
    else if (e.ctrlKey && (k === 'z' || k === 'Z' || k === 'y' || k === 'Y')) handled = false; // let global undo/redo through
    else handled = false;
    if (handled) { e.preventDefault(); e.stopPropagation(); }
  }

  menus() {
    const sel = () => this.rec(this.selId);
    const isShading = () => { const r = sel(); return !!r && (r.kind === 'material' || r.kind === 'texture'); };
    const tabItem = (id, label) => ({ label, check: () => this.tab === id, fn: () => this.setTab(id) });
    return [
      { label: 'File', items: [
        { label: 'Import Texture File...', fn: () => this.importTextureFile() },
        '-',
        { label: 'Close', fn: () => this.win.close() },
      ] },
      { label: 'Edit', items: [
        { label: 'Delete', enabled: isShading, fn: () => this.deleteNodes([sel()]) },
        { label: 'Delete Unused Nodes', fn: () => this.deleteUnused() },
        { label: 'Duplicate Shading Network', enabled: () => sel()?.kind === 'material', fn: () => this.duplicateNetwork(sel()) },
        { label: 'Rename...', enabled: isShading, hk: 'F2', fn: () => this.renameNode(sel()) },
        '-',
        { label: 'Select Objects with Materials', enabled: () => sel()?.kind === 'material', fn: () => this.selectObjectsWith(sel()) },
        { label: 'Assign Material to Viewport Selection', enabled: () => sel()?.kind === 'material', fn: () => this.assignToSelection(sel()) },
      ] },
      { label: 'View', items: [
        { label: 'Frame All', hk: 'A', fn: () => this.graph.frameAll() },
        { label: 'Frame Selected', hk: 'F', fn: () => this.graph.frameSel() },
        { label: 'Reset Zoom', fn: () => { this.graph.view.k = 1; this.graph.draw(); } },
        '-',
        { label: 'Show Swatches on Nodes', check: () => this.graph.showSwatches, fn: () => this.swBtn.click() },
      ] },
      { label: 'Create', items: [
        { section: 'Surface' },
        ...MAT_ORDER.map(t => ({ label: MAT_TYPES[t].label, icon: MAT_ICON[t], fn: () => createMaterialNode(t) })),
        { section: '2D Textures' },
        ...TEX_ORDER.map(t => ({ label: TEX_TYPES[t].label, icon: TEX_ICON[t], fn: () => createTextureNode(t) })),
        { section: 'Lights' },
        ...Object.keys(LIGHT_TYPES).map(t => ({ label: LIGHT_TYPES[t], icon: t, fn: () => { App.cmds.run(t); this.setTab('lights'); } })),
        { label: 'Camera', icon: 'camera', fn: () => { App.cmds.run('createCamera'); this.setTab('cameras'); } },
      ] },
      { label: 'Tabs', items: [tabItem('materials', 'Materials'), tabItem('textures', 'Textures'), tabItem('lights', 'Lights'), tabItem('cameras', 'Cameras'), tabItem('sg', 'Shading Groups')] },
      { label: 'Graph', items: [
        { label: 'Graph Network', enabled: isShading, fn: () => this.graphNetwork(sel(), false) },
        { label: 'Graph Materials on Selected Objects', fn: () => this.graphSelectedObjects() },
        { label: 'Add Selected to Graph', enabled: isShading, fn: () => { this.graph.ids.add(this.selId); this.graph.draw(); } },
        { label: 'Remove Selected from Graph', fn: () => { for (const id of this.graph.sel) this.graph.ids.delete(id); this.graph.sel.clear(); this.graph.draw(); } },
        { label: 'Clear Graph', fn: () => { this.graph.ids.clear(); this.graph.sel.clear(); this.graph.draw(); } },
        '-',
        { label: 'Rearrange Graph', fn: () => this.graph.rearrange() },
      ] },
      { label: 'Window', items: [
        { label: 'Attribute Editor', fn: () => { const r = sel(); if (r && App.ui.showAttr) App.ui.showAttr(r); else App.cmds.run('attributeEditor'); } },
        { label: 'Render View', fn: () => App.ui.renderView?.() },
        { label: 'Render Settings', fn: () => App.ui.renderSettings?.() },
      ] },
      { label: 'Options', items: [
        { label: 'Show Create Panel', check: () => this.showCreate, fn: () => { this.showCreate = !this.showCreate; this.createEl.style.display = this.showCreate ? '' : 'none'; this.splitEl.style.display = this.showCreate ? '' : 'none'; } },
        { label: 'Show Property Editor', check: () => this.propVisible, fn: () => { this.propVisible = !this.propVisible; this.propEl.style.display = this.propVisible ? '' : 'none'; this.graph.resize(); } },
      ] },
      { label: 'Help', items: [{ label: 'Hypershade Help', fn: () => toast('Drag swatches onto objects in a viewport to assign. Drag from a texture outColor port to a material attribute to connect.', 4000) }] },
    ];
  }

  setTab(t) { this.tab = t; this.drawTabs(); this.drawBrowser(); }
  drawTabs() {
    this.tabsEl.innerHTML = '';
    for (const [id, label] of [['materials', 'Materials'], ['textures', 'Textures'], ['lights', 'Lights'], ['cameras', 'Cameras'], ['sg', 'Shading Groups']]) {
      this.tabsEl.append(h('div', { class: 'hs-tab' + (this.tab === id ? ' on' : ''), text: label, onmousedown: () => this.setTab(id) }));
    }
  }
  drawCreate() {
    const f = this.createFilter.value.trim().toLowerCase();
    const L = this.createList; L.innerHTML = '';
    const group = (title, items) => {
      const vis = items.filter(it => !f || it.label.toLowerCase().includes(f) || it.key.toLowerCase().includes(f));
      if (!vis.length) return;
      L.append(h('div', { class: 'ch', text: title }));
      for (const it of vis) {
        const row = h('div', { class: 'ci', title: 'Create a ' + it.label + ' node' }, h('span', { html: icon(it.icon) }), it.label);
        row.addEventListener('click', it.fn);
        row.addEventListener('mouseenter', () => App.help('Create ' + it.label));
        L.append(row);
      }
    };
    group('Surface', MAT_ORDER.map(t => ({ key: t, label: MAT_TYPES[t].label, icon: MAT_ICON[t], fn: () => createMaterialNode(t) })));
    group('2D Textures', TEX_ORDER.map(t => ({ key: t, label: TEX_TYPES[t].label, icon: TEX_ICON[t], fn: () => createTextureNode(t) })));
    group('Lights', Object.keys(LIGHT_TYPES).map(t => ({ key: t, label: LIGHT_TYPES[t], icon: t, fn: () => { App.cmds.run(t); this.setTab('lights'); } })));
    if (!L.children.length) L.append(h('div', { class: 'hs-empty', text: 'No matching nodes' }));
  }

  // items for the current browser tab
  browserItems() {
    const f = this.filter;
    const ok = (n) => !f || n.toLowerCase().includes(f);
    if (this.tab === 'materials') return [...App.mats.values()].filter(m => ok(m.name)).map(m => ({ id: m.id, rec: m, name: m.name, img: () => nodeSwatch(m) }));
    if (this.tab === 'textures') return [...App.texs.values()].filter(t => ok(t.name)).map(t => ({ id: t.id, rec: t, name: t.name, img: () => nodeSwatch(t) }));
    if (this.tab === 'sg') return [...App.mats.values()].filter(m => ok(m.name + 'SG')).map(m => ({ id: m.id, rec: m, name: (m.id === App.defaultMatId ? 'initialShadingGroup' : m.name + 'SG'), img: () => nodeSwatch(m), sg: true }));
    if (this.tab === 'lights') return allDag().filter(o => o.inca.kind === 'light' && ok(o.inca.name)).map(o => ({ id: o.inca.id, rec: o, name: o.inca.name, ico: o.inca.light.type }));
    if (this.tab === 'cameras') return allDag(true).filter(o => o.inca.kind === 'camera' && ok(o.inca.name)).map(o => ({ id: o.inca.id, rec: o, name: o.inca.name, ico: 'camera' }));
    return [];
  }
  drawBrowser() {
    const G = this.gridEl;
    const items = this.browserItems();
    // same set of nodes: update in place (keeps double-clicks and scroll position working)
    const sig = this.tab + '|' + items.map(it => it.id + ':' + it.name).join(',');
    if (sig === this._sig && this.tiles) {
      for (const it of items) { const t = this.tiles.get(it.id); if (!t) continue; t.tile.classList.toggle('sel', it.id === this.selId); if (t.pic.tagName === 'IMG') { const src = it.img() || ''; if (t.pic.getAttribute('src') !== src) t.pic.src = src; } }
      return;
    }
    this._sig = sig;
    G.innerHTML = '';
    this.tiles = new Map();
    if (!items.length) { G.append(h('div', { class: 'hs-empty', text: this.filter ? 'No matches' : 'No nodes' })); return; }
    for (const it of items) {
      const pic = it.ico ? h('div', { class: 'ico', html: icon(it.ico) }) : h('img', { src: it.img() || '', draggable: 'false' });
      const tile = h('div', { class: 'hs-sw' + (it.id === this.selId ? ' sel' : ''), title: it.name }, pic, h('span', { text: it.name }));
      tile.addEventListener('mousedown', (e) => this.tileDown(e, it, tile));
      tile.addEventListener('dblclick', () => this.openNode(it.rec));
      tile.addEventListener('contextmenu', (e) => { e.preventDefault(); e.stopPropagation(); this.select(it.id); this.tileMenu(it, e.clientX, e.clientY); });
      tile.addEventListener('mouseenter', () => App.help(it.name + (it.rec.kind === 'material' ? ' — drag onto an object to assign' : '')));
      this.tiles.set(it.id, { tile, pic, it });
      G.append(tile);
    }
  }
  refreshAll() {
    // drop graph ids that no longer exist
    for (const id of [...this.graph.ids]) if (!App.nodes.has(id)) this.graph.ids.delete(id);
    for (const id of [...this.graph.sel]) if (!App.nodes.has(id)) this.graph.sel.delete(id);
    if (this.selId && !App.nodes.has(this.selId)) this.select(null);
    this.drawBrowser();
    this.graph.draw();
    if (this.prop && this.propRecId && this.rec(this.propRecId)) this.prop.refresh();
    this.updatePeTitle();
  }
  resolveAfterLoad() {
    // undo/redo and file loads rebuild every record: re-resolve by id and rebuild panels
    if (this.prop) { try { this.prop.destroy(); } catch { /* ignore */ } this.prop = null; }
    const id = this.selId; this.propRecId = null; this.selId = null; this._sig = null;
    if (id && App.nodes.has(id)) this.select(id); else this.showProp(null);
    this.refreshAll();
  }
  queueSwatch(rec) {
    if (!rec) return;
    this.pendingSw.add(rec.id);
    if (rec.kind === 'texture') for (const m of matsUsingTex(rec.id)) this.pendingSw.add(m.id);
    if (this._swRaf) return;
    this._swRaf = requestAnimationFrame(() => {
      this._swRaf = 0;
      for (const id of this.pendingSw) {
        const t = this.tiles?.get(id); const r = this.rec(id);
        if (t && r && t.pic.tagName === 'IMG') t.pic.src = nodeSwatch(r) || '';
      }
      this.pendingSw.clear();
      this.graph.draw();
      this.updatePeTitle();
    });
  }

  // ---------------------------------------------------------------- selection / property editor
  select(id, { keepGraph = false } = {}) {
    const same = id === this.selId && this.propRecId === id;
    this.selId = id;
    if (this.tiles) for (const [tid, t] of this.tiles) t.tile.classList.toggle('sel', tid === id);
    if (!keepGraph && id && this.graph.ids.has(id)) { this.graph.sel.clear(); this.graph.sel.add(id); }
    if (same) { this.graph.draw(); return; }
    this.graph.draw();
    this.showProp(this.rec(id));
  }
  openNode(rec) {
    if (!rec) return;
    if (rec.isObject3D) Sel.select([rec]);
    this.select(rec.isObject3D ? rec.inca.id : rec.id);
    if (rec.kind === 'material') this.graphNetwork(rec, false);
    if (!this.propVisible) { this.propVisible = true; this.propEl.style.display = ''; this.graph.resize(); }
  }
  showProp(rec) {
    if (this.prop) { try { this.prop.destroy(); } catch (e) { console.warn(e); } this.prop = null; }
    this.peBody.innerHTML = '';
    this.propRecId = rec ? (rec.isObject3D ? rec.inca.id : rec.id) : null;
    this.updatePeTitle();
    if (!rec) { this.peBody.append(h('div', { class: 'hs-empty', text: 'Select a node to edit its attributes.' })); return; }
    if (App.ui && typeof App.ui.attrPanel === 'function') {
      try { this.prop = App.ui.attrPanel(this.peBody, rec); return; } catch (e) { console.warn('attrPanel failed, using fallback', e); this.peBody.innerHTML = ''; }
    }
    this.prop = fallbackEditor(this.peBody, rec, this);
  }
  updatePeTitle() {
    if (this.peTitle.contains(document.activeElement)) return;
    const rec = this.rec(this.propRecId);
    this.peTitle.innerHTML = '';
    if (!rec) { this.peTitle.append(h('span', { class: 'dim', text: 'Nothing selected' })); return; }
    const name = rec.isObject3D ? rec.inca.name : rec.name;
    const typ = rec.isObject3D ? (rec.inca.kind === 'light' ? rec.inca.light.type : rec.inca.kind) : rec.type;
    const ic = rec.kind === 'material' ? MAT_ICON[rec.type] : rec.kind === 'texture' ? TEX_ICON[rec.type] : rec.isObject3D ? (rec.inca.kind === 'light' ? rec.inca.light.type : 'camera') : 'help';
    const inp = h('input', { value: name, title: 'Rename' });
    inp.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') inp.blur(); if (e.key === 'Escape') { inp.value = name; inp.blur(); } });
    inp.addEventListener('change', () => { const v = inp.value.trim(); if (v && v !== name) this.doRename(rec, v); });
    this.peTitle.append(h('span', { html: icon(ic) }), inp, h('span', { class: 'dim', text: typ }));
  }

  // ---------------------------------------------------------------- browser tile interaction
  tileDown(e, it, tile) {
    if (e.button === 2) return;
    if (e.button === 0) { this.select(it.id); if (it.rec.isObject3D) Sel.select([it.rec]); }
    if (it.rec.kind !== 'material' && it.rec.kind !== 'texture') return;
    e.preventDefault();
    this.gridEl.focus({ preventScroll: true });
    const x0 = e.clientX, y0 = e.clientY; let ghost = null;
    const mm = (ev) => {
      if (!ghost && Math.hypot(ev.clientX - x0, ev.clientY - y0) > 5) {
        ghost = h('img', { class: 'hs-drag-ghost', src: nodeSwatch(it.rec) || '' }); document.body.append(ghost); tile.classList.add('drag');
      }
      if (ghost) {
        ghost.style.left = (ev.clientX + 8) + 'px'; ghost.style.top = (ev.clientY + 8) + 'px';
        const tgt = this.dropTarget(ev);
        App.help(tgt?.vp ? (tgt.obj ? `Drop to assign ${it.rec.name} to ${tgt.obj.inca.name}` : 'Drop onto an object to assign') : tgt?.graph ? 'Drop to add to the graph' : '');
      }
    };
    const mu = (ev) => {
      removeEventListener('mousemove', mm, true); removeEventListener('mouseup', mu, true);
      if (!ghost) return;
      ghost.remove(); tile.classList.remove('drag');
      this.drop(it.rec, ev);
    };
    addEventListener('mousemove', mm, true); addEventListener('mouseup', mu, true);
  }
  dropTarget(ev) {
    const el = document.elementFromPoint(ev.clientX, ev.clientY);
    if (!el) return null;
    if (el === this.graph.canvas) return { graph: true };
    for (const vp of App.viewports || []) {
      if (vp.view && vp.view.contains(el)) {
        let obj = null;
        try { const hits = vp.pickObjects(vp.localPx(ev)); const hit = hits.find(hh => hh.o.inca.kind === 'mesh'); obj = hit ? hit.o : null; } catch (e) { console.warn(e); }
        return { vp, obj };
      }
    }
    return null;
  }
  drop(rec, ev) {
    const tgt = this.dropTarget(ev);
    App.help('');
    if (!tgt) return;
    if (tgt.graph) { this.graph.dropAt(rec, ev.clientX, ev.clientY); return; }
    if (!tgt.obj) { App.help('Nothing under the cursor to assign to.'); return; }
    if (rec.kind === 'material') {
      assignMaterial(rec, [tgt.obj]);
      App.dirty('hypershade', 'attr'); App.requestRender();
    } else if (rec.kind === 'texture') {
      // texture dropped on an object: connect to its material's colour
      const mat = App.mats.get(tgt.obj.inca.material) || App.mats.get(App.defaultMatId);
      const attr = (MAPPABLE[mat.type] || [])[0];
      if (!attr) return;
      this.connect(rec.id, mat.id, attr);
    }
  }
  tileMenu(it, x, y) {
    const rec = it.rec;
    let items;
    if (rec.kind === 'material') items = [
      { label: 'Assign Material To Selection', fn: () => this.assignToSelection(rec) },
      { label: 'Select Objects With Material', fn: () => this.selectObjectsWith(rec) },
      { label: 'Graph Network', fn: () => this.graphNetwork(rec, false) },
      { label: 'Add to Graph', fn: () => { this.graph.ids.add(rec.id); this.graph.layout(); this.graph.draw(); } },
      '-',
      { label: 'Duplicate Shading Network', fn: () => this.duplicateNetwork(rec) },
      { label: 'Rename', fn: () => this.renameNode(rec) },
      { label: 'Delete', enabled: () => rec.id !== App.defaultMatId, fn: () => this.deleteNodes([rec]) },
      '-',
      { label: 'Attribute Editor...', fn: () => App.ui.showAttr ? App.ui.showAttr(rec) : this.openNode(rec) },
    ];
    else if (rec.kind === 'texture') items = [
      { label: 'Graph Network', fn: () => this.graphNetwork(matsUsingTex(rec.id)[0] || rec, false) },
      { label: 'Add to Graph', fn: () => { this.graph.ids.add(rec.id); this.graph.layout(); this.graph.draw(); } },
      { label: 'Connect to Material', sub: () => [...App.mats.values()].map(m => ({ label: m.name, sub: (MAPPABLE[m.type] || []).map(a => ({ label: niceName(a), fn: () => this.connect(rec.id, m.id, a) })) })) },
      '-',
      { label: 'Rename', fn: () => this.renameNode(rec) },
      { label: 'Delete', fn: () => this.deleteNodes([rec]) },
    ];
    else items = [
      { label: 'Select', fn: () => Sel.select([rec]) },
      { label: 'Show in Property Editor', fn: () => this.openNode(rec) },
      { label: 'Rename', enabled: () => !rec.inca.startup, fn: () => this.renameNode(rec) },
      ...(rec.inca.kind === 'camera' ? [{ label: 'Look Through', fn: () => App.activeViewport?.setCamera(rec.inca.name) }, { label: 'Render', fn: () => { if (App.renderSettings) App.renderSettings.camera = rec.inca.name; App.ui.renderView?.(true); } }] : []),
    ];
    showMenu(items, x, y);
  }

  // ---------------------------------------------------------------- operations
  assignToSelection(mat) {
    if (!mat) return;
    if (!App.sel.length && !(App.compMode === 'face' && App.hilite.length)) { warn('Select objects or faces to assign ' + mat.name + ' to.'); return; }
    assignMaterial(mat); App.requestRender();
  }
  selectObjectsWith(mat) {
    const list = usersOf(mat);
    Sel.select(list);
    echo(`hyperShade -objects ${mat.name};`);
  }
  graphSelectedObjects() {
    const ids = new Set();
    for (const o of App.sel) o.traverse(c => { if (c.inca?.kind === 'mesh') { ids.add(c.inca.material || App.defaultMatId); for (const m of c.inca.mesh?.fm || []) if (m) ids.add(m); } });
    if (!ids.size) { warn('Select objects to graph their materials.'); return; }
    this.graph.ids.clear(); this.graph.sel.clear();
    for (const id of ids) { const m = App.mats.get(id); if (m) this.addNetwork(m); }
    this.graph.layout(); this.graph.draw(); this.graph.frameAll();
  }
  addNetwork(rec) {
    if (!rec) return;
    this.graph.ids.add(rec.id);
    if (rec.kind === 'material') for (const tid of Object.values(rec.maps || {})) if (App.texs.has(tid)) this.graph.ids.add(tid);
  }
  graphNetwork(rec, keep = false) {
    if (!rec) return;
    if (!keep) { this.graph.ids.clear(); this.graph.sel.clear(); }
    this.addNetwork(rec);
    this.workTitle.textContent = rec.name || 'untitled_1';
    this.graph.layout(); this.graph.draw();
    requestAnimationFrame(() => this.graph.frameAll());
  }
  connect(texId, matId, attr) {
    const mat = App.mats.get(matId), tex = App.texs.get(texId);
    if (!mat || !tex) return false;
    if (!(MAPPABLE[mat.type] || []).includes(attr)) { warn(`${mat.name}.${attr} cannot take a texture connection.`); return false; }
    Undo.checkpoint('connectAttr');
    mat.maps = mat.maps || {};
    mat.maps[attr] = tex.id;
    updateMaterial(mat);
    echo(`connectAttr -force ${tex.name}.outColor ${mat.name}.${attr};`);
    if (this.graph.ids.has(mat.id)) this.graph.ids.add(tex.id);
    App.dirty('hypershade', 'attr');
    return true;
  }
  disconnect(matId, attr, { checkpoint = true } = {}) {
    const mat = App.mats.get(matId); if (!mat || !mat.maps || !mat.maps[attr]) return;
    const tex = App.texs.get(mat.maps[attr]);
    if (checkpoint) Undo.checkpoint('disconnectAttr');
    delete mat.maps[attr];
    updateMaterial(mat);
    echo(`disconnectAttr ${tex ? tex.name : '?'}.outColor ${mat.name}.${attr};`);
    App.dirty('hypershade', 'attr');
  }
  async renameNode(rec) {
    if (!rec) return;
    if (rec.isObject3D && rec.inca.startup) return;
    const cur = rec.isObject3D ? rec.inca.name : rec.name;
    const v = await (App.ui.prompt ? App.ui.prompt('Rename', 'New name:', cur) : promptDialog('Rename', 'New name:', cur));
    if (v && v.trim() && v.trim() !== cur) this.doRename(rec, v.trim());
  }
  doRename(rec, v) {
    const old = rec.isObject3D ? rec.inca.name : rec.name;
    Undo.checkpoint('rename');
    rename(rec, v);
    const nn = rec.isObject3D ? rec.inca.name : rec.name;
    echo(`rename "${old}" "${nn}";`);
    App.dirty('hypershade', 'attr', 'outliner');
  }
  deleteNodes(list) {
    list = list.filter(r => r && (r.kind === 'material' || r.kind === 'texture'));
    if (!list.length) return;
    if (list.some(r => r.id === App.defaultMatId)) { warn('Cannot delete the default material lambert1.'); list = list.filter(r => r.id !== App.defaultMatId); if (!list.length) return; }
    Undo.checkpoint('delete shading node');
    const rebuild = new Set();
    for (const r of list) {
      if (r.kind === 'material') for (const o of usersOf(r)) rebuild.add(o);
      echo(`delete ${r.name};`);
      deleteShadingNode(r);
      this.graph.ids.delete(r.id); this.graph.sel.delete(r.id); graphPos.delete(r.id); graphPos.delete('sg:' + r.id);
    }
    for (const o of rebuild) rebuildShape(o);
    if (this.selId && !App.nodes.has(this.selId)) this.select(null);
    App.dirty('hypershade', 'attr', 'outliner'); App.requestRender();
  }
  deleteUnused() {
    const used = new Set([App.defaultMatId]);
    for (const n of App.nodes.values()) if (n.isObject3D && n.inca?.kind === 'mesh') { used.add(n.inca.material); for (const m of n.inca.mesh?.fm || []) if (m) used.add(m); }
    const mats = [...App.mats.values()].filter(m => !used.has(m.id));
    const usedTex = new Set(); for (const m of App.mats.values()) if (!mats.includes(m)) for (const t of Object.values(m.maps || {})) usedTex.add(t);
    const texs = [...App.texs.values()].filter(t => !usedTex.has(t.id));
    if (!mats.length && !texs.length) { toast('No unused shading nodes.'); return; }
    this.deleteNodes([...mats, ...texs]);
    App.emit('result', `Deleted ${mats.length} materials and ${texs.length} textures.`);
  }
  duplicateNetwork(mat) {
    if (!mat || mat.kind !== 'material') return;
    Undo.checkpoint('duplicate shading network');
    const remap = {};
    for (const [a, tid] of Object.entries(mat.maps || {})) {
      const t = App.texs.get(tid); if (!t) continue;
      if (!remap[tid]) remap[tid] = createTexture(t.type, t.name, { attrs: t.attrs }).id;
    }
    const maps = {}; for (const [a, tid] of Object.entries(mat.maps || {})) if (remap[tid]) maps[a] = remap[tid];
    const m = createMaterial(mat.type, mat.name, { attrs: mat.attrs, maps });
    echo(`duplicate -upstreamNodes ${mat.name};`); App.emit('result', m.name);
    this.graphNetwork(m, true); this.select(m.id);
    App.dirty('hypershade');
  }
  async importTextureFile() {
    const N = window.incaNative;
    if (!N || !N.openDialog) { warn('Importing texture files requires the desktop app.'); return; }
    const p = await N.openDialog({ title: 'Import Texture File', defaultPath: App.project ? App.project.replace(/[\\/]$/, '') + '/sourceimages' : undefined, filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'tga', 'bmp', 'webp', 'gif', 'hdr', 'exr'] }] });
    if (!p) return;
    const t = createTextureNode('file');
    t.attrs.fileTextureName = relToProject(p);
    for (const m of matsUsingTex(t.id)) updateMaterial(m);
    t._swatch = null; t._hsKey = null;
    App.dirty('hypershade', 'attr');
  }
}

export function relToProject(p) {
  if (!App.project) return p;
  const root = App.project.replace(/[\\/]$/, '').replace(/\\/g, '/');
  const q = p.replace(/\\/g, '/');
  return q.toLowerCase().startsWith(root.toLowerCase() + '/') ? q.slice(root.length + 1) : p;
}

// ------------------------------------------------------------------ node graph (work area)
const NODE_W = 184, HEAD_H = 22, ROW_H = 18, SW_H = 70;
const PORT_COL = { color: '#e8d34f', float: '#7fd0e8', shader: '#7ce07c' };
const imgCache = new Map();
function cachedImg(url, onload) {
  if (!url) return null;
  let im = imgCache.get(url);
  if (!im) { im = new Image(); im.onload = onload; im.src = url; imgCache.set(url, im); if (imgCache.size > 300) imgCache.delete(imgCache.keys().next().value); }
  return im.complete && im.naturalWidth ? im : null;
}

class Graph {
  constructor(hs, host) {
    this.hs = hs; this.host = host;
    this.canvas = h('canvas', { tabindex: 0 });
    host.append(this.canvas);
    this.ctx = this.canvas.getContext('2d');
    this.ids = new Set(); this.sel = new Set();
    this.view = { x: 0, y: 0, k: 1 };
    this.showSwatches = true;
    this.temp = null; this.marquee = null; this.hoverEdge = null; this.hoverPort = null;
    this.nodes = []; this.edges = [];
    this._raf = 0;
    this.bind();
    this._ro = new ResizeObserver(() => this.resize()); this._ro.observe(host);
  }
  destroy() { this._ro.disconnect(); cancelAnimationFrame(this._raf); }
  resize() {
    const w = this.host.clientWidth, hh = this.host.clientHeight, d = devicePixelRatio || 1;
    if (!w || !hh) return;
    this.canvas.width = Math.round(w * d); this.canvas.height = Math.round(hh * d);
    this.canvas.style.width = w + 'px'; this.canvas.style.height = hh + 'px';
    this.drawNow();
  }
  draw() { if (!this._raf) this._raf = requestAnimationFrame(() => { this._raf = 0; this.drawNow(); }); }
  toG(sx, sy) { return { x: (sx - this.view.x) / this.view.k, y: (sy - this.view.y) / this.view.k }; }
  toS(gx, gy) { return { x: gx * this.view.k + this.view.x, y: gy * this.view.k + this.view.y }; }
  local(e) { const r = this.canvas.getBoundingClientRect(); return { x: e.clientX - r.left, y: e.clientY - r.top }; }

  // ---- model
  buildModel() {
    const nodes = [], byId = new Map();
    const swH = this.showSwatches ? SW_H : 0;
    const mk = (id, rec, kind, title, sub, ports) => {
      const p = graphPos.get(id) || { x: 0, y: 0 };
      const n = { id, rec, kind, title, sub, ports, x: p.x, y: p.y, w: NODE_W, h: HEAD_H + swH + ports.length * ROW_H + 6 };
      ports.forEach((pt, i) => { pt.node = n; pt.i = i; });
      nodes.push(n); byId.set(id, n); return n;
    };
    for (const id of this.ids) {
      const rec = App.nodes.get(id); if (!rec) continue;
      if (rec.kind === 'texture') mk(id, rec, 'texture', rec.name, TEX_TYPES[rec.type]?.label || rec.type, [{ name: 'outColor', dir: 'out', type: 'color' }, { name: 'outAlpha', dir: 'out', type: 'float' }]);
      else if (rec.kind === 'material') {
        const ins = (MAPPABLE[rec.type] || []).map(a => ({ name: a, dir: 'in', type: isColor(rec.attrs[a]) ? 'color' : 'float' }));
        mk(id, rec, 'material', rec.name, MAT_TYPES[rec.type]?.label || rec.type, [{ name: 'outColor', dir: 'out', type: 'color' }, ...ins]);
        mk('sg:' + id, rec, 'sg', rec.id === App.defaultMatId ? 'initialShadingGroup' : rec.name + 'SG', 'Shading Group', [{ name: 'surfaceShader', dir: 'in', type: 'shader' }]);
      }
    }
    const edges = [];
    for (const n of nodes) {
      if (n.kind === 'material') {
        for (const [attr, tid] of Object.entries(n.rec.maps || {})) {
          const src = byId.get(tid); if (!src) continue;
          const to = n.ports.find(p => p.name === attr); if (!to) continue;
          const from = (to.type === 'float' ? src.ports[1] : src.ports[0]);
          edges.push({ from, to, mat: n.rec.id, attr, type: to.type });
        }
        const sg = byId.get('sg:' + n.id); if (sg) edges.push({ from: n.ports[0], to: sg.ports[0], sg: true, type: 'shader' });
      }
    }
    this.nodes = nodes; this.edges = edges; this.byId = byId;
    return { nodes, edges };
  }
  portPos(p) {
    const n = p.node; const swH = this.showSwatches ? SW_H : 0;
    return { x: p.dir === 'in' ? n.x : n.x + n.w, y: n.y + HEAD_H + swH + 3 + p.i * ROW_H + ROW_H / 2 };
  }
  // assign default positions to nodes without one (Maya-like left-to-right layout)
  layout(force = false) {
    const { nodes } = this.buildModel();
    const mats = nodes.filter(n => n.kind === 'material');
    const placed = new Set();
    let y = 0;
    if (!force) for (const n of nodes) if (graphPos.has(n.id)) y = Math.max(y, graphPos.get(n.id).y + n.h + 50);
    for (const m of mats) {
      const texs = Object.values(m.rec.maps || {}).map(id => this.byId.get(id)).filter(Boolean).filter((t, i, a) => a.indexOf(t) === i);
      const needs = force || !graphPos.has(m.id);
      let base = needs ? y : graphPos.get(m.id).y;
      if (needs) graphPos.set(m.id, { x: 0, y: base });
      const mp = graphPos.get(m.id);
      if (force || !graphPos.has('sg:' + m.id)) graphPos.set('sg:' + m.id, { x: mp.x + NODE_W + 70, y: mp.y });
      let ty = mp.y;
      for (const t of texs) {
        if (placed.has(t.id)) continue;
        if (force || !graphPos.has(t.id)) graphPos.set(t.id, { x: mp.x - NODE_W - 90, y: ty });
        placed.add(t.id);
        ty += t.h + 24;
      }
      if (needs) y = Math.max(base + m.h, ty) + 50;
    }
    for (const t of nodes.filter(n => n.kind === 'texture' && !placed.has(n.id))) {
      if (force || !graphPos.has(t.id)) { graphPos.set(t.id, { x: -NODE_W - 90, y }); y += t.h + 24; }
    }
    this.buildModel();
  }
  rearrange() { this.layout(true); this.draw(); this.frameAll(); }
  bbox(list) {
    if (!list.length) return null;
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const n of list) { x0 = Math.min(x0, n.x); y0 = Math.min(y0, n.y); x1 = Math.max(x1, n.x + n.w); y1 = Math.max(y1, n.y + n.h); }
    return { x0, y0, x1, y1 };
  }
  frame(list) {
    this.layout();
    const b = this.bbox(list || this.nodes);
    const W = this.host.clientWidth, H = this.host.clientHeight;
    if (!b || !W || !H) { this.view = { x: W / 2, y: H / 2, k: 1 }; this.draw(); return; }
    const k = Math.max(0.15, Math.min(1.25, Math.min((W - 60) / (b.x1 - b.x0), (H - 60) / (b.y1 - b.y0))));
    this.view = { k, x: W / 2 - (b.x0 + b.x1) / 2 * k, y: H / 2 - (b.y0 + b.y1) / 2 * k };
    this.draw();
  }
  frameAll() { this.frame(null); }
  frameSel() { this.buildModel(); const s = this.nodes.filter(n => this.sel.has(n.id)); this.frame(s.length ? s : null); }

  // ---- hit testing (screen coords)
  nodeAt(sx, sy) {
    const g = this.toG(sx, sy);
    for (let i = this.nodes.length - 1; i >= 0; i--) { const n = this.nodes[i]; if (g.x >= n.x && g.x <= n.x + n.w && g.y >= n.y && g.y <= n.y + n.h) return n; }
    return null;
  }
  portAt(sx, sy) {
    let best = null, bd = 9;
    for (const n of this.nodes) for (const p of n.ports) {
      const q = this.portPos(p); const s = this.toS(q.x, q.y);
      const d = Math.hypot(s.x - sx, s.y - sy);
      if (d < bd) { bd = d; best = p; }
    }
    if (best) return best;
    // clicking the label row of an input/output also counts
    const n = this.nodeAt(sx, sy); if (!n) return null;
    const g = this.toG(sx, sy); const swH = this.showSwatches ? SW_H : 0;
    const i = Math.floor((g.y - n.y - HEAD_H - swH - 3) / ROW_H);
    const p = n.ports[i];
    if (!p) return null;
    if (p.dir === 'out' && g.x > n.x + n.w - 16) return p;
    if (p.dir === 'in' && g.x < n.x + 16) return p;
    return null;
  }
  bez(a, b) { const dx = Math.max(40, Math.abs(b.x - a.x) * 0.5); return [a, { x: a.x + dx, y: a.y }, { x: b.x - dx, y: b.y }, b]; }
  edgeAt(sx, sy) {
    let best = null, bd = 6;
    for (const e of this.edges) {
      const [p0, p1, p2, p3] = this.bez(this.portPos(e.from), this.portPos(e.to)).map(p => this.toS(p.x, p.y));
      for (let i = 0; i <= 32; i++) {
        const t = i / 32, u = 1 - t;
        const x = u * u * u * p0.x + 3 * u * u * t * p1.x + 3 * u * t * t * p2.x + t * t * t * p3.x;
        const y = u * u * u * p0.y + 3 * u * u * t * p1.y + 3 * u * t * t * p2.y + t * t * t * p3.y;
        const d = Math.hypot(x - sx, y - sy); if (d < bd) { bd = d; best = e; }
      }
    }
    return best;
  }

  // ---- drawing
  drawNow() {
    const c = this.ctx, d = devicePixelRatio || 1, W = this.canvas.width / d, H = this.canvas.height / d;
    if (!W || !H) return;
    this.layout();
    c.setTransform(d, 0, 0, d, 0, 0);
    c.fillStyle = '#3f3f3f'; c.fillRect(0, 0, W, H);
    // grid
    const k = this.view.k; const step = 40 * k;
    if (step > 8) {
      c.strokeStyle = '#464646'; c.lineWidth = 1; c.beginPath();
      for (let x = ((this.view.x % step) + step) % step; x < W; x += step) { c.moveTo(Math.round(x) + 0.5, 0); c.lineTo(Math.round(x) + 0.5, H); }
      for (let y = ((this.view.y % step) + step) % step; y < H; y += step) { c.moveTo(0, Math.round(y) + 0.5); c.lineTo(W, Math.round(y) + 0.5); }
      c.stroke();
    }
    if (!this.nodes.length) {
      c.fillStyle = '#8a8a8a'; c.font = '12px ' + getComputedStyle(document.body).fontFamily; c.textAlign = 'center';
      c.fillText('Select a material and choose Graph > Graph Network, or create a node from the Create panel.', W / 2, H / 2);
      c.textAlign = 'left';
      return;
    }
    c.setTransform(d * k, 0, 0, d * k, d * this.view.x, d * this.view.y);
    // edges
    for (const e of this.edges) {
      const [a, b1, b2, b] = this.bez(this.portPos(e.from), this.portPos(e.to));
      const hot = e === this.hoverEdge;
      c.strokeStyle = hot ? '#ffffff' : (PORT_COL[e.type] || '#ccc'); c.lineWidth = (hot ? 3 : 2) / Math.max(0.6, k);
      c.beginPath(); c.moveTo(a.x, a.y); c.bezierCurveTo(b1.x, b1.y, b2.x, b2.y, b.x, b.y); c.stroke();
    }
    if (this.temp) {
      const a = this.portPos(this.temp.port); const g = this.toG(this.temp.sx, this.temp.sy);
      const [p0, p1, p2, p3] = this.temp.port.dir === 'out' ? this.bez(a, g) : this.bez(g, a);
      c.strokeStyle = '#ffffff'; c.setLineDash([6, 4]); c.lineWidth = 1.5 / Math.max(0.6, k);
      c.beginPath(); c.moveTo(p0.x, p0.y); c.bezierCurveTo(p1.x, p1.y, p2.x, p2.y, p3.x, p3.y); c.stroke(); c.setLineDash([]);
    }
    const font = getComputedStyle(document.body).fontFamily;
    const swH = this.showSwatches ? SW_H : 0;
    for (const n of this.nodes) {
      const sel = this.sel.has(n.id);
      // shadow + body
      c.fillStyle = 'rgba(0,0,0,0.35)'; rr(c, n.x + 3, n.y + 4, n.w, n.h, 5); c.fill();
      c.fillStyle = '#353535'; rr(c, n.x, n.y, n.w, n.h, 5); c.fill();
      // header
      c.fillStyle = n.kind === 'sg' ? '#4f5b45' : n.kind === 'texture' ? '#4f4f63' : '#5b5049';
      rr(c, n.x, n.y, n.w, HEAD_H, [5, 5, 0, 0]); c.fill();
      c.fillStyle = '#eeeeee'; c.font = `600 11.5px ${font}`; c.textBaseline = 'middle';
      c.fillText(clip(c, n.title, n.w - 12), n.x + 7, n.y + HEAD_H / 2 + 0.5);
      // swatch
      if (swH) {
        const url = n.kind === 'sg' ? null : nodeSwatch(n.rec);
        const im = url ? cachedImg(url, () => this.draw()) : null;
        const sx = n.x + (n.w - 64) / 2, sy = n.y + HEAD_H + 3;
        c.fillStyle = '#2b2b2b'; c.fillRect(sx, sy, 64, 64);
        if (im) c.drawImage(im, sx, sy, 64, 64);
        else if (n.kind === 'sg') { c.strokeStyle = '#7ce07c'; c.lineWidth = 1; c.strokeRect(sx + 16.5, sy + 16.5, 31, 31); c.fillStyle = '#9a9a9a'; c.font = `10px ${font}`; c.textAlign = 'center'; c.fillText('SG', sx + 32, sy + 32); c.textAlign = 'left'; }
        c.fillStyle = '#9a9a9a'; c.font = `10px ${font}`; c.textAlign = 'right'; c.fillText(n.sub, n.x + n.w - 6, n.y + HEAD_H + 10); c.textAlign = 'left';
      }
      // ports
      for (const p of n.ports) {
        const q = this.portPos(p);
        const connected = this.edges.some(e => e.from === p || e.to === p);
        c.fillStyle = '#c8c8c8'; c.font = `11px ${font}`;
        if (p.dir === 'in') c.fillText(niceName(p.name), q.x + 10, q.y + 0.5);
        else { c.textAlign = 'right'; c.fillText(niceName(p.name), q.x - 10, q.y + 0.5); c.textAlign = 'left'; }
        c.beginPath(); c.arc(q.x, q.y, this.hoverPort === p ? 5.5 : 4.5, 0, Math.PI * 2);
        c.fillStyle = connected ? (PORT_COL[p.type] || '#ccc') : '#2a2a2a'; c.fill();
        c.lineWidth = 1.5 / Math.max(0.6, k); c.strokeStyle = PORT_COL[p.type] || '#ccc'; c.stroke();
      }
      // outline
      c.lineWidth = (sel ? 2 : 1) / Math.max(0.6, k);
      c.strokeStyle = sel ? '#ffd24a' : '#1e1e1e'; rr(c, n.x, n.y, n.w, n.h, 5); c.stroke();
    }
    if (this.marquee) {
      const m = this.marquee; const a = this.toG(m.x0, m.y0), b = this.toG(m.x1, m.y1);
      c.fillStyle = 'rgba(120,170,220,0.12)'; c.strokeStyle = '#9cd0f5'; c.lineWidth = 1 / k;
      c.fillRect(Math.min(a.x, b.x), Math.min(a.y, b.y), Math.abs(b.x - a.x), Math.abs(b.y - a.y));
      c.strokeRect(Math.min(a.x, b.x), Math.min(a.y, b.y), Math.abs(b.x - a.x), Math.abs(b.y - a.y));
    }
  }

  // ---- interaction
  bind() {
    const cv = this.canvas;
    cv.addEventListener('contextmenu', (e) => e.preventDefault());
    cv.addEventListener('wheel', (e) => {
      e.preventDefault();
      const p = this.local(e); const g = this.toG(p.x, p.y);
      const k = Math.max(0.12, Math.min(3, this.view.k * Math.exp(-e.deltaY * 0.0015)));
      this.view.k = k; this.view.x = p.x - g.x * k; this.view.y = p.y - g.y * k; this.draw();
    }, { passive: false });
    cv.addEventListener('mousemove', (e) => {
      if (this._dragging) return;
      const p = this.local(e);
      const port = this.portAt(p.x, p.y); const edge = port ? null : this.edgeAt(p.x, p.y);
      if (port !== this.hoverPort || edge !== this.hoverEdge) { this.hoverPort = port; this.hoverEdge = edge; this.draw(); }
      cv.style.cursor = port ? 'crosshair' : '';
    });
    cv.addEventListener('mouseleave', () => { if (this.hoverPort || this.hoverEdge) { this.hoverPort = this.hoverEdge = null; this.draw(); } });
    cv.addEventListener('dblclick', (e) => {
      const p = this.local(e); const n = this.nodeAt(p.x, p.y);
      if (n && n.kind !== 'sg') { this.hs.openNode(n.rec); if (App.ui.showAttr) App.ui.showAttr(n.rec); }
    });
    cv.addEventListener('mousedown', (e) => this.down(e));
  }
  drag(onMove, onUp) {
    this._dragging = true;
    const mm = (ev) => onMove(ev);
    const mu = (ev) => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); this._dragging = false; onUp && onUp(ev); };
    addEventListener('mousemove', mm); addEventListener('mouseup', mu);
  }
  down(e) {
    this.canvas.focus({ preventScroll: true });
    e.preventDefault();
    const p = this.local(e);
    // pan: Alt+MMB, plain MMB too; zoom: Alt+RMB
    if (e.button === 1 || (e.altKey && e.button === 0 && e.shiftKey)) {
      const v0 = { ...this.view }; const x0 = e.clientX, y0 = e.clientY;
      this.canvas.style.cursor = 'grabbing';
      this.drag((ev) => { this.view.x = v0.x + ev.clientX - x0; this.view.y = v0.y + ev.clientY - y0; this.draw(); }, () => { this.canvas.style.cursor = ''; });
      return;
    }
    if (e.button === 2 && e.altKey) {
      const v0 = { ...this.view }; const x0 = e.clientX; const g = this.toG(p.x, p.y);
      this.drag((ev) => { const k = Math.max(0.12, Math.min(3, v0.k * Math.exp((ev.clientX - x0) * 0.006))); this.view.k = k; this.view.x = p.x - g.x * k; this.view.y = p.y - g.y * k; this.draw(); });
      return;
    }
    if (e.button === 2) { this.contextMenu(e, p); return; }
    if (e.button !== 0) return;
    const port = this.portAt(p.x, p.y);
    if (port) { this.startConnect(port, e); return; }
    const n = this.nodeAt(p.x, p.y);
    if (n) {
      const id = n.id;
      if (e.shiftKey || e.ctrlKey) { if (this.sel.has(id) && e.ctrlKey) this.sel.delete(id); else if (this.sel.has(id) && e.shiftKey) this.sel.delete(id); else this.sel.add(id); }
      else if (!this.sel.has(id)) { this.sel.clear(); this.sel.add(id); }
      const recId = n.kind === 'sg' ? n.rec.id : id;
      if (this.hs.selId !== recId) this.hs.select(recId, { keepGraph: true });
      // move selected nodes
      const start = [...this.sel].map(sid => ({ id: sid, p: { ...(graphPos.get(sid) || { x: 0, y: 0 }) } }));
      const x0 = e.clientX, y0 = e.clientY; let moved = false;
      this.drag((ev) => {
        const dx = (ev.clientX - x0) / this.view.k, dy = (ev.clientY - y0) / this.view.k;
        if (!moved && Math.hypot(ev.clientX - x0, ev.clientY - y0) < 3) return;
        moved = true;
        for (const s of start) graphPos.set(s.id, { x: Math.round(s.p.x + dx), y: Math.round(s.p.y + dy) });
        this.draw();
      }, () => {
        if (!moved && !e.shiftKey && !e.ctrlKey) { this.sel.clear(); this.sel.add(id); }
        this.draw();
      });
      this.draw();
      return;
    }
    // marquee
    if (!e.shiftKey && !e.ctrlKey) this.sel.clear();
    const base = new Set(this.sel);
    this.marquee = { x0: p.x, y0: p.y, x1: p.x, y1: p.y };
    this.draw();
    this.drag((ev) => {
      const q = this.local(ev); this.marquee.x1 = q.x; this.marquee.y1 = q.y;
      const a = this.toG(Math.min(p.x, q.x), Math.min(p.y, q.y)), b = this.toG(Math.max(p.x, q.x), Math.max(p.y, q.y));
      this.sel = new Set(base);
      for (const n2 of this.nodes) if (n2.x < b.x && n2.x + n2.w > a.x && n2.y < b.y && n2.y + n2.h > a.y) { if (e.ctrlKey) this.sel.delete(n2.id); else this.sel.add(n2.id); }
      this.draw();
    }, () => {
      const moved = Math.hypot(this.marquee.x1 - this.marquee.x0, this.marquee.y1 - this.marquee.y0) > 3;
      this.marquee = null;
      if (!moved && !e.shiftKey && !e.ctrlKey) this.hs.select(null);
      else { const last = [...this.sel].pop(); if (last) this.hs.select(last.startsWith('sg:') ? last.slice(3) : last, { keepGraph: true }); }
      this.draw();
    });
  }
  startConnect(port, e) {
    let from = port;
    // grabbing a connected input picks up the connection from its source
    if (port.dir === 'in') {
      const ed = this.edges.find(x => x.to === port && !x.sg);
      if (ed) {
        Undo.checkpoint('disconnectAttr');
        this.hs.disconnect(ed.mat, ed.attr, { checkpoint: false });
        this.buildModel();
        from = this.byId.get(ed.from.node.id)?.ports[ed.from.i] || null;
        if (!from) return;
      }
    }
    const p = this.local(e);
    this.temp = { port: from, sx: p.x, sy: p.y };
    this.draw();
    this.drag((ev) => { const q = this.local(ev); this.temp.sx = q.x; this.temp.sy = q.y; this.hoverPort = this.portAt(q.x, q.y); this.draw(); }, (ev) => {
      const q = this.local(ev); const tgt = this.portAt(q.x, q.y);
      const src = this.temp.port; this.temp = null; this.hoverPort = null;
      if (tgt && tgt !== src && tgt.dir !== src.dir) {
        const out = src.dir === 'out' ? src : tgt, inp = src.dir === 'out' ? tgt : src;
        this.connectPorts(out, inp);
      } else if (!tgt) {
        // released on empty space with a texture output: offer to connect via menu of material attrs
      }
      this.draw();
    });
  }
  connectPorts(out, inp) {
    const on = out.node, inn = inp.node;
    if (on.kind === 'texture' && inn.kind === 'material') { this.hs.connect(on.rec.id, inn.rec.id, inp.name); return; }
    if (on.kind === 'material' && inn.kind === 'sg') {
      if (on.rec.id === inn.rec.id) return;
      // reroute the shading group's members to this material
      const users = usersOf(inn.rec).filter(o => o.inca.material === inn.rec.id);
      if (!users.length) { warn(`${inn.title} has no members.`); return; }
      assignMaterial(on.rec, users); App.dirty('hypershade', 'attr'); return;
    }
    warn(`Cannot connect ${on.title}.${out.name} to ${inn.title}.${inp.name}.`);
  }
  dropAt(rec, cx, cy) {
    const r = this.canvas.getBoundingClientRect(); const sx = cx - r.left, sy = cy - r.top;
    this.buildModel();
    const port = this.portAt(sx, sy);
    if (rec.kind === 'texture' && port && port.dir === 'in' && port.node.kind === 'material') { this.ids.add(rec.id); this.hs.connect(rec.id, port.node.rec.id, port.name); return; }
    const g = this.toG(sx, sy);
    if (!this.ids.has(rec.id)) {
      this.ids.add(rec.id);
      graphPos.set(rec.id, { x: Math.round(g.x - NODE_W / 2), y: Math.round(g.y - 20) });
      if (rec.kind === 'material') graphPos.set('sg:' + rec.id, { x: Math.round(g.x + NODE_W / 2 + 70), y: Math.round(g.y - 20) });
    }
    this.sel.clear(); this.sel.add(rec.id); this.hs.select(rec.id, { keepGraph: true });
    this.layout(); this.draw();
  }
  contextMenu(e, p) {
    const port = this.portAt(p.x, p.y);
    const edge = port ? null : this.edgeAt(p.x, p.y);
    const n = this.nodeAt(p.x, p.y);
    let items;
    if (port && port.dir === 'in' && port.node.kind === 'material') {
      const mat = port.node.rec; const cur = mat.maps?.[port.name];
      items = [
        { section: `${mat.name}.${port.name}` },
        ...(cur ? [{ label: 'Break Connection', fn: () => this.hs.disconnect(mat.id, port.name) }] : []),
        { label: 'Connect New Texture', sub: TEX_ORDER.map(t => ({ label: TEX_TYPES[t].label, icon: TEX_ICON[t], fn: () => { const tex = createTextureNode(t, { graph: true }); this.hs.connect(tex.id, mat.id, port.name); this.layout(); } })) },
        { label: 'Connect Existing Texture', enabled: () => App.texs.size > 0, sub: () => [...App.texs.values()].map(t => ({ label: t.name, fn: () => this.hs.connect(t.id, mat.id, port.name) })) },
      ];
    } else if (edge) {
      items = edge.sg ? [{ section: 'Shading group connection' }, { label: 'Break Connection', enabled: () => false }] : [
        { section: `${App.texs.get(edge.from.node.id)?.name}.${edge.from.name} → ${App.mats.get(edge.mat)?.name}.${edge.attr}` },
        { label: 'Break Connection', fn: () => this.hs.disconnect(edge.mat, edge.attr) },
      ];
    } else if (n) {
      const rec = n.rec;
      if (!this.sel.has(n.id)) { this.sel.clear(); this.sel.add(n.id); this.hs.select(rec.id, { keepGraph: true }); }
      items = [
        { section: n.title },
        { label: 'Graph Network', fn: () => this.hs.graphNetwork(rec.kind === 'texture' ? (matsUsingTex(rec.id)[0] || rec) : rec, false) },
        ...(rec.kind === 'material' ? [
          { label: 'Assign Material To Selection', fn: () => this.hs.assignToSelection(rec) },
          { label: 'Select Objects With Material', fn: () => this.hs.selectObjectsWith(rec) },
        ] : []),
        { label: 'Rename', fn: () => this.hs.renameNode(rec) },
        { label: 'Remove From Graph', fn: () => { for (const id of this.sel) { this.ids.delete(id.startsWith('sg:') ? id.slice(3) : id); } this.sel.clear(); this.draw(); } },
        { label: 'Delete', enabled: () => rec.id !== App.defaultMatId, fn: () => this.hs.deleteNodes([...this.sel].map(id => App.nodes.get(id.startsWith('sg:') ? id.slice(3) : id))) },
        '-',
        { label: 'Attribute Editor...', fn: () => App.ui.showAttr ? App.ui.showAttr(rec) : this.hs.openNode(rec) },
      ];
    } else {
      const g = this.toG(p.x, p.y);
      const place = (rec) => { graphPos.set(rec.id, { x: Math.round(g.x - NODE_W / 2), y: Math.round(g.y) }); if (rec.kind === 'material') graphPos.set('sg:' + rec.id, { x: Math.round(g.x + NODE_W / 2 + 70), y: Math.round(g.y) }); this.ids.add(rec.id); this.layout(); this.draw(); };
      items = [
        { label: 'Create Material', sub: MAT_ORDER.map(t => ({ label: MAT_TYPES[t].label, icon: MAT_ICON[t], fn: () => { const m = createMaterialNode(t); place(m); } })) },
        { label: 'Create Texture', sub: TEX_ORDER.map(t => ({ label: TEX_TYPES[t].label, icon: TEX_ICON[t], fn: () => { const tx = createTextureNode(t); place(tx); } })) },
        '-',
        { label: 'Frame All', hk: 'A', fn: () => this.frameAll() },
        { label: 'Rearrange Graph', fn: () => this.rearrange() },
        { label: 'Clear Graph', fn: () => { this.ids.clear(); this.sel.clear(); this.draw(); } },
      ];
    }
    showMenu(items, e.clientX, e.clientY);
  }
}
function rr(c, x, y, w, hh, r) { c.beginPath(); if (c.roundRect) c.roundRect(x, y, w, hh, r); else c.rect(x, y, w, hh); }
function clip(c, s, w) { if (c.measureText(s).width <= w) return s; let t = s; while (t.length > 1 && c.measureText(t + '…').width > w) t = t.slice(0, -1); return t + '…'; }

// ------------------------------------------------------------------ fallback property editor
// used when the shell's App.ui.attrPanel is not available
function fallbackEditor(container, rec, hs) {
  const root = h('div', { class: 'hs-fe', style: { padding: '4px 0' } });
  container.append(root);
  let setters = [];
  let dragging = false;
  const target = () => rec.isObject3D ? (rec.inca.kind === 'light' ? rec.inca.light : rec.inca.kind === 'camera' ? rec.inca.cam : null) : rec.attrs;
  const commit = () => {
    if (rec.kind === 'material') updateMaterial(rec);
    else if (rec.kind === 'texture') {
      if (rec._tex) { rec._tex.dispose(); rec._tex = null; }
      rec._swatch = null; rec._hsKey = null;
      for (const m of matsUsingTex(rec.id)) updateMaterial(m);
      hs?.queueSwatch(rec);
    } else if (rec.isObject3D && rec.inca.kind === 'light') { syncLight(rec); App.dirty('channels'); }
    App.requestRender();
  };
  const ck = (key) => { if (!dragging) Undo.checkpoint('setAttr ' + key); };
  const setVal = (key, v, live) => {
    const T = target(); if (!T) return;
    if (!live && !dragging) ck(key);
    T[key] = v;
    if (!live) dragging = false;
    commit();
  };
  const row = (label, ...els) => h('div', { class: 'hs-fe-row' }, h('label', { text: label, title: label }), ...els);
  const frame = (title, body) => {
    const f = h('div', { class: 'frame' }); const head = h('div', { class: 'frame-head' }, h('span', { class: 'tri', text: '▼' }), title);
    head.addEventListener('click', () => { f.classList.toggle('closed'); head.firstChild.textContent = f.classList.contains('closed') ? '▶' : '▼'; });
    f.append(head, h('div', { class: 'frame-body' }, body)); return f;
  };
  const build = () => {
    root.innerHTML = ''; setters = [];
    const T = target();
    if (!T) { root.append(h('div', { class: 'hs-empty', text: 'No editable attributes.' })); return; }
    const mappable = rec.kind === 'material' ? (MAPPABLE[rec.type] || []) : [];
    const rows = [];
    for (const key of Object.keys(T)) {
      if (key === 'type' && rec.isObject3D) continue;
      if (key === 'startup') continue;
      const v = T[key];
      const label = niceName(key);
      const mapEls = [];
      if (mappable.includes(key)) {
        const tid = rec.maps?.[key]; const tex = tid && App.texs.get(tid);
        if (tex) {
          mapEls.push(h('span', { class: 'lnk', text: tex.name, title: 'Select connected texture', onclick: () => hs ? hs.openNode(tex) : null }));
          mapEls.push(iconBtn('trash', 'Break connection', () => { hs ? hs.disconnect(rec.id, key) : null; build(); }, 'ib map'));
        } else {
          const b = iconBtn('mapBtn', 'Connect a texture', (e) => {
            showMenu([...TEX_ORDER.map(t => ({ label: TEX_TYPES[t].label, icon: TEX_ICON[t], fn: () => { const tx = createTextureNode(t); if (hs) hs.connect(tx.id, rec.id, key); else { rec.maps[key] = tx.id; updateMaterial(rec); } build(); } })),
              ...(App.texs.size ? ['-', { label: 'Existing', sub: [...App.texs.values()].map(t => ({ label: t.name, fn: () => { if (hs) hs.connect(t.id, rec.id, key); build(); } })) }] : [])], e.clientX, e.clientY);
          }, 'ib map');
          mapEls.push(b);
        }
      }
      if (typeof v === 'number') {
        const nf = numField(v, (n, live) => setVal(key, n, live), { step: Math.abs(v) > 10 ? 0.5 : 0.01, onStart: () => { ck(key); dragging = true; } });
        const isUnit = !rec.isObject3D && v >= 0 && v <= 1 && !/repeat|offset|rotate|frequency|cosine|bump|IOR/i.test(key);
        let slider = null;
        if (isUnit) {
          slider = h('input', { type: 'range', min: 0, max: 1000, value: v * 1000 });
          slider.addEventListener('mousedown', () => { ck(key); dragging = true; });
          slider.addEventListener('input', () => { const n = slider.value / 1000; nf.set(n); setVal(key, n, true); });
          slider.addEventListener('change', () => { setVal(key, slider.value / 1000, false); dragging = false; });
        }
        setters.push(() => { nf.set(T[key]); if (slider && document.activeElement !== slider) slider.value = T[key] * 1000; });
        rows.push(row(label, nf, slider, ...mapEls));
      } else if (isColor(v)) {
        const sw = h('div', { class: 'csw', style: { background: colorToCss(v) } });
        sw.addEventListener('click', (e) => {
          const orig = T[key].slice(); ck(key); dragging = true;
          colorPicker(e.clientX, e.clientY, orig, (c) => { T[key] = c.slice(); sw.style.background = colorToCss(c); commit(); }, () => { dragging = false; build(); });
        });
        const lum = h('input', { type: 'range', min: 0, max: 1000, value: Math.max(...v) * 1000, title: 'Value' });
        lum.addEventListener('mousedown', () => { ck(key); dragging = true; });
        lum.addEventListener('input', () => { const cur = T[key]; const mx = Math.max(...cur) || 1; const nv = lum.value / 1000; T[key] = Math.max(...cur) > 0 ? cur.map(x => x / mx * nv) : [nv, nv, nv]; sw.style.background = colorToCss(T[key]); commit(); });
        lum.addEventListener('change', () => { dragging = false; });
        setters.push(() => { sw.style.background = colorToCss(T[key]); if (document.activeElement !== lum) lum.value = Math.max(...T[key]) * 1000; });
        rows.push(row(label, sw, lum, ...mapEls));
      } else if (typeof v === 'boolean') {
        const cb = h('input', { type: 'checkbox', checked: v });
        cb.addEventListener('change', () => setVal(key, cb.checked, false));
        setters.push(() => { cb.checked = !!T[key]; });
        rows.push(row(label, cb));
      } else if (typeof v === 'string' && ENUMS[key] && !(rec.isObject3D)) {
        const s = h('select', {}, ENUMS[key].map(o => h('option', { value: o, text: o })));
        s.value = v; s.addEventListener('change', () => setVal(key, s.value, false));
        setters.push(() => { s.value = T[key]; });
        rows.push(row(label, s));
      } else if (typeof v === 'string') {
        const inp = h('input', { class: 'wide', value: v });
        inp.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') inp.blur(); });
        inp.addEventListener('change', () => setVal(key, inp.value, false));
        setters.push(() => { if (document.activeElement !== inp) inp.value = T[key]; });
        const extra = [];
        if (/file|texture/i.test(key)) extra.push(iconBtn('openScene', 'Browse…', async () => {
          const N = window.incaNative; if (!N || !N.openDialog) { toast('File browsing requires the desktop app.'); return; }
          const p = await N.openDialog({ title: 'Open', defaultPath: App.project ? App.project.replace(/[\\/]$/, '') + '/sourceimages' : undefined, filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'tga', 'bmp', 'webp', 'gif', 'hdr', 'exr'] }] });
          if (p) { const rel = relToProject(p); inp.value = rel; setVal(key, rel, false); }
        }, 'ib map'));
        rows.push(row(label, inp, ...extra));
      } else if (key === 'colors' && Array.isArray(v)) {
        // ramp colour entries: [[pos,[r,g,b]], ...]
        const box = h('div', { style: { flex: 1, display: 'flex', flexDirection: 'column', gap: '2px' } });
        v.forEach((entry, i) => {
          const pos = numField(entry[0], (n, live) => { if (!live && !dragging) ck(key); T[key][i][0] = Math.max(0, Math.min(1, n)); if (!live) dragging = false; commit(); }, { step: 0.01, width: '44px', onStart: () => { ck(key); dragging = true; } });
          const sw = h('div', { class: 'csw', style: { background: colorToCss(entry[1]) } });
          sw.addEventListener('click', (e) => { ck(key); dragging = true; colorPicker(e.clientX, e.clientY, T[key][i][1].slice(), (c) => { T[key][i][1] = c.slice(); sw.style.background = colorToCss(c); commit(); }, () => { dragging = false; }); });
          const del = iconBtn('trash', 'Remove entry', () => { if (T[key].length <= 1) return; ck(key); T[key].splice(i, 1); commit(); build(); }, 'ib map');
          box.append(h('div', { style: { display: 'flex', gap: '4px', alignItems: 'center' } }, pos, sw, del));
        });
        box.append(h('button', { text: 'Add Entry', style: { height: '18px', alignSelf: 'flex-start' }, onclick: () => { ck(key); T[key].push([1, [1, 1, 1]]); T[key].sort((a, b) => a[0] - b[0]); commit(); build(); } }));
        rows.push(row(label, box));
      }
    }
    const title = rec.isObject3D ? (rec.inca.kind === 'light' ? niceName(rec.inca.light.type) + ' Attributes' : 'Camera Attributes') : (MAT_TYPES[rec.type]?.label || TEX_TYPES[rec.type]?.label || rec.type) + ' Attributes';
    root.append(frame(title, rows));
    if (rec.kind === 'material') {
      const users = usersOf(rec);
      root.append(frame('Shading Group', [row('Members', h('span', { class: 'dim', text: users.length ? users.map(o => o.inca.name).join(', ') : '(none)' }))]));
    }
  };
  build();
  return {
    refresh() { if (!App.nodes.has(rec.isObject3D ? rec.inca.id : rec.id)) return; for (const s of setters) { try { s(); } catch { /* ignore */ } } },
    destroy() { root.remove(); },
  };
}

// ------------------------------------------------------------------ registration
App.ui = App.ui || {};
App.ui.hypershade = openHypershade;
App.ui.assignNewMaterialDialog = assignNewMaterialDialog;
App.ui.createTextureNode = createTextureNode;
App.ui.createMaterialNode = createMaterialNode;
