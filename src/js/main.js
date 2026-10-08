// Inca — application bootstrap: prefs, scene, layout, hotkeys, render loop.
import * as THREE from 'three';
import { App } from './core/app.js';
import { Undo } from './core/undo.js';
import * as S from './core/scene.js';
import { Sel } from './core/selection.js';
import { Cmds } from './core/commands.js';
import './core/anim.js';
import { initDefaultMaterials } from './core/materials.js';
import { Manip, placeProxy } from './core/manip.js';
import './core/mel.js';
// editors that register history ops must load before any scene/undo is restored
import './ui/uvEditor.js';
import { Viewport, ensureDefaultLights } from './ui/viewport.js';
import { h, toast, closeMenus, menuIsOpen, promptDialog } from './ui/dom.js';
import { normalizeCombo, comboFromEvent } from './ui/hotkeyEditor.js';
import { ensurePrefDefaults, applyUndoToggle } from './ui/prefsWindow.js';
import './ui/graphEditor.js';
import './ui/dopeSheet.js';
import './ui/scriptEditor.js';
import './ui/shelfEditor.js';
import './ui/hypergraph.js';
import './ui/about.js';
import './ui/hypershade.js';
import './ui/renderView.js';
import './core/io.js';
import './ui/cmdline.js';
import './ui/docks.js';
import './ui/channelBox.js';
import './ui/attrEditor.js';
import './ui/outliner.js';
import './ui/timeline.js';
import './ui/shelf.js';
import './ui/statusline.js';
import './ui/menus.js';
import './ui/markingMenus.js';
import './ui/tools.js';
import './ui/extraCommands.js';
import './core/dynamics.js'; // dynamics: emitters, particles, fields, rigid bodies, fire/smoke
import './core/booleans.js'; // dynamics agent: mesh booleans (polyCBoolOp)
import './ui/quadDraw.js'; // dynamics agent: Quad Draw retopology tool

App.ui = App.ui || {};
App.ui.prompt = (title, label, value) => promptDialog(title, label, value);

// ------------------------------------------------------------------ preferences
const PREF_DEFAULTS = {
  undoLevels: 50, undoOn: true, manipSize: 1, grid: { size: 12, divisions: 12, major: 5, spacing: 1 },
  hud: { polyCount: false, fps: false, frame: false, camNames: true, viewAxis: true },
  bgMode: 0, smoothDivisions: 2, defaultInTangent: 'auto', defaultOutTangent: 'auto', playbackSpeed: 1,
  linearUnit: 'cm', angularUnit: 'deg', uiScale: 1, opt: {}, hotkeys: {}, shelves: null, recent: [], recentProjects: [],
  workspace: 'Maya Classic', orthoTumble: false, menuSet: 'modeling', autosave: { on: false, interval: 10 }, primitivesInteractive: false,
  rightPanel: 'channels', rightDockWidth: 300, outlinerWidth: 240,
};
function mergeDefaults(dst, src) { for (const [k, v] of Object.entries(src)) { if (dst[k] === undefined) dst[k] = JSON.parse(JSON.stringify(v)); else if (v && typeof v === 'object' && !Array.isArray(v) && dst[k] && typeof dst[k] === 'object') mergeDefaults(dst[k], v); } return dst; }
async function loadPrefs() {
  let p = null;
  try { p = window.incaNative ? await window.incaNative.loadPrefs() : JSON.parse(localStorage.getItem('inca-prefs') || 'null'); } catch { p = null; }
  App.prefs = mergeDefaults(p || {}, PREF_DEFAULTS);
  ensurePrefDefaults(App.prefs);
}
let saveT = 0;
App.savePrefs = () => {
  clearTimeout(saveT);
  saveT = setTimeout(() => {
    const data = JSON.parse(JSON.stringify(App.prefs));
    try { if (window.incaNative) window.incaNative.savePrefs(data); else localStorage.setItem('inca-prefs', JSON.stringify(data)); } catch (e) { console.warn('prefs save failed', e); }
  }, 150);
};
App.applyPrefs = () => {
  const p = App.prefs;
  document.body.style.zoom = String(p.uiScale || 1);
  for (const v of App.viewports) { v.tc.setSize(p.manipSize || 1); v.grid && v.gridScene.remove(v.grid); v.grid = null; v.updateLabel(); v.needsRender = true; }
  applyUndoToggle();
  App.dirty('timeline', 'panelUI');
};

// ------------------------------------------------------------------ hotkeys
const HOLD_KEYS = { X: '_keyGrid', V: '_keyPoint', C: '_keyCurve' };
App.hotkeys = {
  map: new Map(), defaults: new Map(),
  buildDefaults() {
    this.defaults.clear();
    for (const c of Cmds.list()) if (c.hk) for (const k of String(c.hk).split(/\s*\/\s*/)) this.defaults.set(normalizeCombo(k), c.id);
    const extra = { 'Ctrl+Z': 'undo', 'Ctrl+Y': 'redo', 'Ctrl+Shift+Z': 'redo', 'Ctrl+N': 'newScene', 'Ctrl+O': 'openScene', 'Ctrl+S': 'saveScene', 'Ctrl+Shift+S': 'saveSceneAs', 'Ctrl+Q': 'quit', 'Insert': 'pivotEdit', 'Ctrl+Shift+Alt+S': 'saveSceneAs', 'Shift+S': 'setKey',
      'Ctrl+Shift+N': 'createEmptyGroup', 'Ctrl+K': 'insertEdgeLoopTool', 'Shift+Q': 'componentTypeCycle', 'Ctrl+Space': 'toggleUIElements', 'Alt+Shift+V': 'goToStart', 'Home': 'frameAll', 'F2': 'menuSetModeling', 'F3': 'menuSetRigging', 'F4': 'menuSetAnimation', 'F6': 'menuSetRendering', 'Ctrl+Backspace': 'deleteEdgeVertex', 'Backspace': 'deleteSel', 'Ctrl+Shift+Q': 'quadDrawTool', 'Ctrl+Shift+X': 'multiCutTool', 'Ctrl+Shift+E': 'extrude', 'Ctrl+Alt+V': 'snapAlign' };
    for (const [k, v] of Object.entries(extra)) if (Cmds.registry.has(v)) this.defaults.set(normalizeCombo(k), v);
  },
  load() {
    this.buildDefaults(); this.map = new Map(this.defaults);
    for (const [k, v] of Object.entries(App.prefs.hotkeys || {})) { const n = normalizeCombo(k); if (v) this.map.set(n, v); else this.map.delete(n); }
  },
  bind(combo, cmd) { this.map.set(normalizeCombo(combo), cmd); this.save(); },
  unbind(combo) { this.map.delete(normalizeCombo(combo)); this.save(); },
  comboOf(cmd) { const r = []; for (const [k, v] of this.map) if (v === cmd) r.push(k); return r[0] || ''; },
  reset() { this.map = new Map(this.defaults); this.save(); },
  save() {
    const o = {};
    for (const [k, v] of this.map) if (this.defaults.get(k) !== v) o[k] = v;
    for (const k of this.defaults.keys()) if (!this.map.has(k)) o[k] = null;
    App.prefs.hotkeys = o; App.savePrefs();
  },
};
App.hotkeyLabel = (id) => App.hotkeys.comboOf(id) || '';

function editableTarget(t) { return t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName)); }
let spaceDown = 0, spaceHotbox = false, spaceTimer = 0;
function onKeyDown(e) {
  if (e.defaultPrevented) return;
  const t = e.target;
  if (editableTarget(t)) return;
  const inWin = t && t.closest && t.closest('.fwin');
  const combo = comboFromEvent(e);
  if (!combo) return;
  if (inWin && !/^Ctrl\+/.test(combo)) return;
  if (menuIsOpen() && e.key !== 'Escape') closeMenus();
  // tools get first crack (Enter / Escape / Backspace while drawing curves etc.)
  if (App.tools?.handleKey && App.tools.handleKey(e, combo)) { e.preventDefault(); return; }
  if (combo === 'Space') {
    e.preventDefault(); if (e.repeat) return;
    spaceDown = performance.now(); spaceHotbox = false;
    clearTimeout(spaceTimer); spaceTimer = setTimeout(() => { if (spaceDown) { spaceHotbox = true; App.ui.hotbox?.(true); } }, 200);
    return;
  }
  if (!e.ctrlKey && !e.altKey && HOLD_KEYS[combo.replace('Shift+', '')] && !App.hotkeys.map.has(combo + '__never')) {
    const k = HOLD_KEYS[combo.replace('Shift+', '')]; if (!Manip[k]) { Manip[k] = true; App.dirty('statusline'); } e.preventDefault(); return;
  }
  if (combo === 'J') { Manip.stepRotate = true; return; }
  if (e.repeat && !/^(Alt\+[,.]|[,.]|Alt\+V)$/.test(combo)) { if (!['Alt+,', 'Alt+.'].includes(combo)) return; }
  const cmd = App.hotkeys.map.get(combo);
  if (cmd) { e.preventDefault(); Cmds.run(cmd); return; }
  if (combo === 'Escape') { App.tools?.cancel?.(); }
}
function onKeyUp(e) {
  const k = (e.code || '').replace(/^Key/, '');
  if (HOLD_KEYS[k] && Manip[HOLD_KEYS[k]]) { Manip[HOLD_KEYS[k]] = false; App.dirty('statusline'); }
  if (k === 'J') Manip.stepRotate = false;
  if (e.code === 'Space' && spaceDown) {
    const dt = performance.now() - spaceDown; spaceDown = 0; clearTimeout(spaceTimer);
    if (spaceHotbox) { App.ui.hotbox?.(false); spaceHotbox = false; }
    else if (dt < 300 && !editableTarget(e.target)) toggleSpaceLayout();
  }
}
addEventListener('keydown', onKeyDown);
addEventListener('keyup', onKeyUp);
addEventListener('blur', () => { for (const k of Object.values(HOLD_KEYS)) Manip[k] = false; if (spaceHotbox) App.ui.hotbox?.(false); spaceDown = 0; spaceHotbox = false; });

// ------------------------------------------------------------------ viewports & layouts
App.layout = 'single';
const LAYOUT_CLASS = { single: 'l1', four: 'l4', twoSide: 'l2h', twoStack: 'l2v', three: 'l3' };
const vpByCam = {};
App.setLayout = (kind = 'four', cam = null) => {
  const host = document.getElementById('viewports');
  const order = { four: ['top', 'persp', 'front', 'side'], twoSide: ['persp', 'top'], twoStack: ['persp', 'front'], three: ['persp', 'top', 'front'] }[kind];
  let show;
  if (kind === 'single') {
    const want = cam ? (App.viewports.find(v => v.camName === cam) || null) : (App.hoverViewport || App.activeViewport || App.viewports[0]);
    show = [want || App.viewports[0]];
  } else show = order.map((c, i) => App.viewports.find(v => v._slot === c) || App.viewports[i]);
  for (const v of App.viewports) v.el.classList.toggle('hidden', !show.includes(v));
  for (const v of show) host.append(v.el);
  host.className = LAYOUT_CLASS[kind] || 'l1';
  App.layout = kind; App.layoutPanels = show;
  if (!show.includes(App.activeViewport)) App.setActiveViewport(show[0]);
  App.requestRender(); App.dirty('panelUI');
};
let lastMulti = 'four';
function toggleSpaceLayout() {
  const v = App.hoverViewport || App.activeViewport;
  if (App.layout === 'single') App.setLayout(lastMulti);
  else { lastMulti = App.layout; App.setLayout('single'); if (v) { for (const x of App.viewports) x.el.classList.toggle('hidden', x !== v); App.layoutPanels = [v]; App.setActiveViewport(v); } }
}
App.setActiveViewport = (vp) => {
  if (App.activeViewport === vp) return;
  App.activeViewport = vp;
  for (const v of App.viewports) v.el.classList.toggle('active', v === vp);
  App.emit('activeViewport', vp);
};

// ------------------------------------------------------------------ tools / time
const TRANSFORM = { move: 'translate', rotate: 'rotate', scale: 'scale' };
const TOOL_HELP = { select: 'Select Tool: select an object', lasso: 'Lasso Tool: draw a lasso to select', paint: 'Paint Selection Tool: drag to paint-select components', move: 'Move Tool: Use manipulator to move object(s). Ctrl+MMB drag to move components along normals. Use D or INSERT to change the pivot position and axis orientation.', rotate: 'Rotate Tool: Use manipulator to rotate object(s). Use D or INSERT to change the pivot position.', scale: 'Scale Tool: Use manipulator to scale object(s). Ctrl+LMB drag on an axis to scale the other two axes.' };
App.setTool = (name) => {
  if (App.tools?.isInteractive?.(App.tool) && App.tool !== name) App.tools.deactivate?.();
  App.tool = name;
  if (TRANSFORM[name]) { Manip.mode = TRANSFORM[name]; App.lastTool = name; }
  else if (['select', 'lasso', 'paint'].includes(name)) Manip.mode = 'none';
  if (App.tools?.isInteractive?.(name)) App.tools.activate(name);
  App.help(TOOL_HELP[name] || Cmds.registry.get(name + 'Tool')?.help || '');
  placeProxy(); App.requestRender();
  App.emit('toolChanged', name); App.dirty('toolSettings', 'statusline');
};
App.setTime = (t) => {
  t = Math.round(t * 1000) / 1000;
  App.time.current = t;
  App.anim.evaluate(t);
  if (App.dynamics?.step) App.dynamics.step(t);
  placeProxy();
  App.emit('timeChanged', t);
  App.dirty('timeline', 'channels');
  App.requestRender();
};

// ------------------------------------------------------------------ title
App.on('refresh', (d) => {
  if (!d.has('title')) return;
  const t = `${App.sceneName}${App.modified ? '*' : ''} - Inca${App.project ? ' - ' + App.project : ''}`;
  document.title = t; window.incaNative?.setTitle(t);
});

// ------------------------------------------------------------------ render loop
let frames = 0, lastFpsT = performance.now();
function loop() {
  requestAnimationFrame(loop);
  try {
    if (App.timeline?.tick) App.timeline.tick();
    S.flushDependents();
    for (const v of App.layoutPanels || App.viewports) if (v.needsRender) { v.render(); frames++; }
    const now = performance.now();
    if (now - lastFpsT > 500) { App.fpsMeasured = frames * 1000 / (now - lastFpsT) / Math.max(1, (App.layoutPanels || []).length); frames = 0; lastFpsT = now; if (App.prefs.hud.fps) App.requestRender(); }
  } catch (e) { console.error('render loop', e); }
}

// ------------------------------------------------------------------ boot
function splash() {
  const s = h('div', { class: 'splash' }, h('div', { class: 'card' }, h('img', { src: '../build/icon256.png', width: 112, height: 112 }), h('h1', { text: 'INCA' }), h('p', { text: 'Free 3D modeling · animation · rendering' }), h('p', { class: 'dim', text: 'Version ' + App.version })));
  document.body.append(s); return s;
}
async function boot() {
  const sp = splash();
  await loadPrefs();
  App.scene = new THREE.Scene();
  App.world = new THREE.Group(); App.world.name = 'world'; App.world.matrixAutoUpdate = false;
  App.scene.add(App.world);
  App.scene.add(Manip.proxy);
  initDefaultMaterials();
  S.createStartupCameras();
  App.hotkeys.load();
  App.emit('beforeUI');
  App.ui.buildShell?.();
  // viewports
  const host = document.getElementById('viewports');
  for (const cam of ['persp', 'top', 'front', 'side']) { const v = new Viewport(cam, host); v._slot = cam; App.viewports.push(v); vpByCam[cam] = v; }
  ensureDefaultLights();
  App.setActiveViewport(App.viewports[0]);
  App.setLayout(App.prefs.startLayout || 'single', 'persp');
  App.applyPrefs();
  App.ui.applyWorkspace?.(App.prefs.workspace, true);
  App.on('viewportsRebind', () => { for (const v of App.viewports) { v.updateLabel(); v.needsRender = true; } });
  App.on('selectionChanged', () => { placeProxy(); App.requestRender(); });
  App.on('sceneLoaded', () => { placeProxy(); for (const v of App.viewports) { v.updateLabel(); v.needsRender = true; } });
  App.emit('uiReady');
  App.modified = false; App.dirty('outliner', 'channels', 'attr', 'layers', 'timeline', 'title', 'statusline', 'shelf');
  requestAnimationFrame(loop);
  setTimeout(() => { sp.style.opacity = '0'; setTimeout(() => sp.remove(), 500); }, 350);
  // native integration
  if (window.incaNative) {
    window.incaNative.onRequestClose(() => Cmds.run('quit'));
    window.incaNative.onOpenFile((p) => App.io?.openPath(p));
    const pf = await window.incaNative.pendingFile(); if (pf) App.io?.openPath(pf);
    else if (App.prefs.loadLastProject !== false && App.prefs.lastProject) App.io?.setProject(App.prefs.lastProject, { quiet: true });
  }
  App.emit('echo', '// Inca ' + App.version + ' ready. Hold Space for the hotbox, right-click for marking menus.');
  if (window.__incaTest) window.__incaTest();
}
window.addEventListener('error', (e) => { App.emit('error', '// Error: ' + (e.message || e.error)); });
window.addEventListener('unhandledrejection', (e) => { App.emit('error', '// Error: ' + (e.reason?.message || e.reason)); });
boot().catch(e => { console.error(e); document.body.append(h('pre', { style: { color: '#f88', position: 'fixed', top: '40px', left: '40px', zIndex: 99999 }, text: 'Inca failed to start:\n' + (e.stack || e) })); });
export { App };
