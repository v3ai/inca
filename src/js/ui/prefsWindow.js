// Inca — Preferences window (Windows > Settings/Preferences > Preferences)
import { App } from '../core/app.js';
import { h, FloatWin, numField, toast, confirmDialog, menuBar } from './dom.js';

// ---------------------------------------------------------------- schema
// Each field: { key:'a.b', label, type:'bool'|'int'|'float'|'range'|'enum'|'text', def, target?:'prefs'|'time',
//               options?:[labels], values?:[stored values], min?, max?, step?, suffix?, help? }
const TANGENT_LABELS = ['Auto', 'Spline', 'Clamped', 'Linear', 'Flat', 'Stepped', 'Plateau'];
const TANGENT_VALUES = ['auto', 'spline', 'clamped', 'linear', 'flat', 'step', 'plateau'];

export const CATEGORIES = [
  { name: 'Interface', fields: [
    { section: 'Interface' },
    { key: 'uiScale', label: 'Interface scaling', type: 'range', def: 1, min: 0.8, max: 1.5, step: 0.05, help: 'Scales the entire user interface' },
    { key: 'menuSet', label: 'Default menu set', type: 'enum', def: 'modeling', options: ['Modeling', 'Rigging', 'Animation', 'FX', 'Rendering'], values: ['modeling', 'rigging', 'animation', 'fx', 'rendering'] },
    { key: 'ui.helpLine', label: 'Show help line', type: 'bool', def: true },
    { key: 'ui.toolTips', label: 'Show tooltips', type: 'bool', def: true },
    { key: 'ui.confirmQuit', label: 'Ask before quitting with unsaved changes', type: 'bool', def: true },
    { section: 'Windows' },
    { key: 'ui.rememberWindows', label: 'Remember size and position', type: 'bool', def: true },
  ] },
  { name: 'UI Elements', fields: [
    { section: 'Visible UI Elements' },
    { key: 'ui.show.statusLine', label: 'Status line', type: 'bool', def: true },
    { key: 'ui.show.shelf', label: 'Shelf', type: 'bool', def: true },
    { key: 'ui.show.toolBox', label: 'Tool box', type: 'bool', def: true },
    { key: 'ui.show.timeSlider', label: 'Time slider', type: 'bool', def: true },
    { key: 'ui.show.rangeSlider', label: 'Range slider', type: 'bool', def: true },
    { key: 'ui.show.commandLine', label: 'Command line', type: 'bool', def: true },
    { key: 'ui.show.helpLine', label: 'Help line', type: 'bool', def: true },
    { key: 'ui.show.channelBox', label: 'Channel Box / Layer Editor', type: 'bool', def: true },
  ] },
  { name: 'Display', fields: [
    { section: 'Viewport' },
    { key: 'bgMode', label: 'Background color', type: 'enum', def: 0, options: ['Gray', 'Black', 'Dark Gray', 'Light Gray', 'Gradient'], values: [0, 1, 2, 3, 4] },
    { key: 'orthoTumble', label: 'Allow tumbling orthographic views', type: 'bool', def: false },
    { section: 'Grid' },
    { key: 'grid.size', label: 'Length and width', type: 'float', def: 12, min: 0.01, step: 0.5, suffix: 'units' },
    { key: 'grid.spacing', label: 'Grid lines every', type: 'float', def: 1, min: 0.001, step: 0.1, suffix: 'units (snapping)' },
    { key: 'grid.divisions', label: 'Subdivisions', type: 'int', def: 12, min: 1, max: 200 },
    { key: 'grid.major', label: 'Major line every', type: 'int', def: 5, min: 1, max: 100, suffix: 'subdivisions' },
    { section: 'Heads Up Display' },
    { key: 'hud.polyCount', label: 'Poly count', type: 'bool', def: false },
    { key: 'hud.fps', label: 'Frame rate', type: 'bool', def: false },
    { key: 'hud.frame', label: 'Current frame', type: 'bool', def: false },
    { key: 'hud.camNames', label: 'Camera names', type: 'bool', def: true },
    { key: 'hud.viewAxis', label: 'View axis', type: 'bool', def: true },
  ] },
  { name: 'Kinematics / Manipulators', fields: [
    { section: 'Manipulators' },
    { key: 'manipSize', label: 'Global scale', type: 'range', def: 1, min: 0.2, max: 4, step: 0.05 },
    { key: 'manipActiveOnly', label: 'Show manipulator in active view only', type: 'bool', def: false },
  ] },
  { name: 'Animation', fields: [
    { section: 'Tangents' },
    { key: 'defaultInTangent', label: 'Default in tangent', type: 'enum', def: 'auto', options: TANGENT_LABELS, values: TANGENT_VALUES },
    { key: 'defaultOutTangent', label: 'Default out tangent', type: 'enum', def: 'auto', options: TANGENT_LABELS, values: TANGENT_VALUES },
  ] },
  { name: 'Time Slider', fields: [
    { section: 'Playback' },
    { key: 'playbackSpeed', label: 'Playback speed', type: 'enum', def: 1, options: ['Play every frame', 'Real-time [fps]', 'Half', 'Twice'], values: [0, 1, 0.5, 2] },
    { key: 'loop', target: 'time', label: 'Looping', type: 'enum', def: 'continuous', options: ['Once', 'Oscillate', 'Continuous'], values: ['once', 'oscillate', 'continuous'] },
    { section: 'Time Range' },
    { key: 'start', target: 'time', label: 'Playback start', type: 'int', def: 1 },
    { key: 'end', target: 'time', label: 'Playback end', type: 'int', def: 120 },
    { key: 'animStart', target: 'time', label: 'Animation start', type: 'int', def: 1 },
    { key: 'animEnd', target: 'time', label: 'Animation end', type: 'int', def: 200 },
  ] },
  { name: 'Modeling', fields: [
    { section: 'Smooth Mesh Preview' },
    { key: 'smoothDivisions', label: 'Subdivision levels', type: 'int', def: 2, min: 1, max: 4 },
    { section: 'Primitives' },
    { key: 'primitivesInteractive', label: 'Interactive creation', type: 'bool', def: false },
  ] },
  { name: 'Settings', fields: [
    { section: 'Working Units' },
    { key: 'linearUnit', label: 'Linear', type: 'enum', def: 'cm', options: ['millimeter', 'centimeter', 'meter', 'inch', 'foot', 'yard'], values: ['mm', 'cm', 'm', 'in', 'ft', 'yd'] },
    { key: 'angularUnit', label: 'Angular', type: 'enum', def: 'deg', options: ['degrees', 'radians'], values: ['deg', 'rad'] },
    { key: 'fps', target: 'time', label: 'Time', type: 'enum', def: 24, options: ['24 fps (Film)', '25 fps (PAL)', '30 fps (NTSC)', '48 fps (Show)', '50 fps (PAL Field)', '60 fps (NTSC Field)'], values: [24, 25, 30, 48, 50, 60] },
  ] },
  { name: 'Undo', fields: [
    { section: 'Undo' },
    { key: 'undoOn', label: 'Undo', type: 'enum', def: true, options: ['On', 'Off'], values: [true, false] },
    { key: 'undoLevels', label: 'Queue size (levels)', type: 'int', def: 50, min: 1, max: 1000 },
  ] },
  { name: 'Files/Projects', fields: [
    { section: 'Recent Files' },
    { key: 'recentCount', label: 'Recent files list size', type: 'int', def: 10, min: 1, max: 30 },
    { key: 'recentProjectsCount', label: 'Recent projects list size', type: 'int', def: 5, min: 1, max: 30 },
    { section: 'Projects' },
    { key: 'loadLastProject', label: 'Set last project on startup', type: 'bool', def: true },
  ] },
  { name: 'Save', fields: [
    { section: 'Auto Save' },
    { key: 'autosave.on', label: 'Enable auto save', type: 'bool', def: false },
    { key: 'autosave.interval', label: 'Interval', type: 'float', def: 15, min: 1, max: 600, step: 1, suffix: 'minutes' },
    { key: 'autosave.limit', label: 'Limit auto saves', type: 'int', def: 10, min: 1, max: 100 },
    { section: 'Save' },
    { key: 'incrementalSave', label: 'Incremental save', type: 'bool', def: false },
    { key: 'saveCompressed', label: 'Compress scene files', type: 'bool', def: false },
  ] },
];

// ---------------------------------------------------------------- path helpers (pure)
export function getPath(obj, path) { let o = obj; for (const k of path.split('.')) { if (o == null) return undefined; o = o[k]; } return o; }
export function setPath(obj, path, v) {
  const ks = path.split('.'); let o = obj;
  for (let i = 0; i < ks.length - 1; i++) { if (o[ks[i]] == null || typeof o[ks[i]] !== 'object') o[ks[i]] = {}; o = o[ks[i]]; }
  o[ks[ks.length - 1]] = v;
}
const clone = (v) => (v === undefined ? undefined : JSON.parse(JSON.stringify(v)));
const allFields = () => CATEGORIES.flatMap(c => c.fields.filter(f => f.key));
// fill missing prefs keys with defaults (does not touch existing values)
export function ensurePrefDefaults(prefs) {
  for (const f of allFields()) if (f.target !== 'time' && getPath(prefs, f.key) === undefined) setPath(prefs, f.key, clone(f.def));
  if (!prefs.opt) prefs.opt = {};
  return prefs;
}

function injectStyle() {
  if (document.getElementById('inca-prefs-style')) return;
  document.head.append(h('style', { id: 'inca-prefs-style', text: `
.prefwin .pf-main { flex: 1; display: flex; min-height: 0; padding: 6px 6px 0; gap: 6px; }
.prefwin .pf-cats { width: 190px; flex: none; }
.prefwin .pf-cats .li { padding: 3px 8px; }
.prefwin .pf-right { flex: 1; display: flex; flex-direction: column; min-width: 0; background: #3c3c3c; border: 1px solid #2a2a2a; }
.prefwin .pf-head { font-weight: 700; color: #eee; padding: 6px 10px; background: #353535; border-bottom: 1px solid #2a2a2a; display: flex; align-items: center; }
.prefwin .pf-fields { flex: 1; overflow: auto; padding: 4px 12px 10px; display: flex; flex-direction: column; gap: 4px; }
.prefwin .dlg-row > label:first-child { width: 230px; }
.prefwin .pf-range { display: flex; align-items: center; gap: 6px; }
.prefwin .pf-range input[type=range] { width: 160px; }
.prefwin .pf-range input:not([type]) { width: 56px; }
.prefwin .dlg-row select { min-width: 150px; }
.prefwin .pf-changed > label:first-child { color: #e6c54a; }
` }));
}

// Undo on/off: core/undo.js has no switch, so turn checkpoints into no-ops while undo is off
let origCheckpoint = null;
export function applyUndoToggle() {
  const U = App.undo; if (!U) return;
  const off = App.prefs?.undoOn === false;
  if (off && !origCheckpoint) { origCheckpoint = U.checkpoint; U.checkpoint = () => {}; U.clear?.(); }
  else if (!off && origCheckpoint) { U.checkpoint = origCheckpoint; origCheckpoint = null; }
}

// ---------------------------------------------------------------- window
export function openPreferences(category) {
  injectStyle();
  if (!App.prefs) App.prefs = {};
  ensurePrefDefaults(App.prefs);
  const win = new FloatWin('preferences', 'Preferences', { w: 820, h: 560, minW: 520, minH: 300 });
  if (win.reused) { if (category && win._selectCat) win._selectCat(category); return win; }
  win.el.classList.add('prefwin');

  // working copies (edited in place; committed on Save)
  const draft = { prefs: clone(App.prefs), time: { fps: App.time.fps, loop: App.time.loop, start: App.time.start, end: App.time.end, animStart: App.time.animStart, animEnd: App.time.animEnd } };
  if (draft.prefs.undoOn === undefined) draft.prefs.undoOn = true;
  const orig = clone(draft);
  const tgt = (f) => (f.target === 'time' ? draft.time : draft.prefs);
  const getV = (f) => { const v = getPath(tgt(f), f.key); return v === undefined ? f.def : v; };
  const setV = (f, v) => { setPath(tgt(f), f.key, v); };

  let cur = Math.max(0, CATEGORIES.findIndex(c => c.name === category));
  const catList = h('div', { class: 'list-box pf-cats' });
  const head = h('div', { class: 'pf-head' });
  const fieldsEl = h('div', { class: 'pf-fields' });

  const renderCats = () => {
    catList.innerHTML = '';
    CATEGORIES.forEach((c, i) => catList.append(h('div', { class: 'li' + (i === cur ? ' sel' : ''), text: c.name, onclick: () => { cur = i; renderCats(); renderFields(); } })));
  };
  const makeInput = (f, row) => {
    const v = getV(f);
    const mark = () => row.classList.toggle('pf-changed', JSON.stringify(getV(f)) !== JSON.stringify(getPath(f.target === 'time' ? orig.time : orig.prefs, f.key) ?? f.def));
    const clamp = (n) => { if (f.min !== undefined) n = Math.max(f.min, n); if (f.max !== undefined) n = Math.min(f.max, n); return n; };
    if (f.type === 'bool') return h('input', { type: 'checkbox', checked: !!v, onchange: (e) => { setV(f, e.target.checked); mark(); } });
    if (f.type === 'enum') {
      const sel = h('select', { onchange: () => { setV(f, f.values ? f.values[sel.selectedIndex] : sel.selectedIndex); mark(); } }, f.options.map((o, i) => h('option', { value: i, text: o })));
      const idx = f.values ? f.values.findIndex(x => x === v) : v;
      sel.selectedIndex = Math.max(0, idx);
      return sel;
    }
    if (f.type === 'range') {
      const nf = numField(v, (n) => { n = clamp(n); setV(f, n); rng.value = n; nf.set(n); mark(); }, { step: f.step || 0.01 });
      const rng = h('input', { type: 'range', min: f.min, max: f.max, step: f.step || 0.01, value: v });
      rng.addEventListener('input', () => { const n = +rng.value; setV(f, n); nf.set(n); mark(); });
      return h('span', { class: 'pf-range' }, nf, rng);
    }
    if (f.type === 'text') return h('input', { class: 'wide', value: v ?? '', onchange: (e) => { setV(f, e.target.value); mark(); } });
    const nf = numField(v, (n) => { n = clamp(f.type === 'int' ? Math.round(n) : n); setV(f, n); nf.set(n); mark(); }, { step: f.step || (f.type === 'int' ? 1 : 0.1), int: f.type === 'int' });
    return nf;
  };
  const renderFields = () => {
    const c = CATEGORIES[cur];
    head.innerHTML = '';
    head.append(h('span', { text: c.name + ': ' + c.name + ' Preferences' }), h('span', { class: 'spacer' }));
    fieldsEl.innerHTML = '';
    for (const f of c.fields) {
      if (f.section) { fieldsEl.append(h('div', { class: 'dlg-sect', text: f.section })); continue; }
      const row = h('div', { class: 'dlg-row', title: f.help || '' });
      row.append(h('label', { text: f.label }), makeInput(f, row));
      if (f.suffix) row.append(h('span', { class: 'dim', text: f.suffix }));
      if (f.help) row.addEventListener('mouseenter', () => App.help(f.help));
      fieldsEl.append(row);
    }
    if (c.name === 'Settings') fieldsEl.append(h('div', { class: 'dim', style: { padding: '8px 0 0 236px', lineHeight: '1.4' }, text: 'Linear and angular units are stored with your preferences; scene values are always in centimeters and degrees.' }));
  };
  win._selectCat = (name) => { const i = CATEGORIES.findIndex(c => c.name === name); if (i >= 0) { cur = i; renderCats(); renderFields(); } };

  const resetCat = (all) => {
    const cats = all ? CATEGORIES : [CATEGORIES[cur]];
    for (const c of cats) for (const f of c.fields) if (f.key) setV(f, clone(f.def));
    renderFields();
  };
  const commit = () => {
    for (const f of allFields()) {
      const v = clone(getV(f));
      if (f.target === 'time') App.time[f.key] = v; else setPath(App.prefs, f.key, v);
    }
    App.time.playEvery = App.prefs.playbackSpeed === 0;
    if (App.time.end < App.time.start) App.time.end = App.time.start;
    if (App.time.animStart > App.time.start) App.time.animStart = App.time.start;
    if (App.time.animEnd < App.time.end) App.time.animEnd = App.time.end;
    applyUndoToggle();
    // keep the live viewports in sync even before the shell's applyPrefs runs
    for (const vp of App.viewports || []) {
      try { vp.tc?.setSize?.(App.prefs.manipSize); vp.gridAxis = null; vp.updateLabel?.(); } catch { /* ignore */ }
    }
    try { App.applyPrefs?.(); } catch (e) { console.error(e); }
    try { App.savePrefs?.(); } catch (e) { console.error(e); }
    App.dirty('timeline', 'title', 'channels');
    App.requestRender();
    if (App.emit) App.emit('prefsChanged', App.prefs);
  };

  const menuRow = h('div', { class: 'pmenubar' });
  menuBar(menuRow, [
    { label: 'Edit', items: [
      { label: 'Restore Default Settings (this category)', fn: () => resetCat(false) },
      { label: 'Restore Default Settings (all)', fn: async () => { if (await confirmDialog('Restore Defaults', 'Reset ALL preferences to their default values?', ['Reset', 'Cancel']) === 0) resetCat(true); } },
    ] },
    { label: 'Help', items: [{ label: 'Help on Preferences', fn: () => toast('Edit preferences, then click Save. Cancel discards changes.') }] },
  ]);

  win.body.append(menuRow,
    h('div', { class: 'pf-main' }, catList, h('div', { class: 'pf-right' }, head, fieldsEl)),
    h('div', { class: 'dlg-buttons' },
      h('button', { text: 'Save', onclick: () => { commit(); win.close(); toast('Preferences saved'); } }),
      h('button', { text: 'Reset Category', title: 'Restore this category\'s defaults', onclick: () => resetCat(false) }),
      h('button', { text: 'Reset All', title: 'Restore all defaults', onclick: async () => { if (await confirmDialog('Restore Defaults', 'Reset ALL preferences to their default values?', ['Reset', 'Cancel']) === 0) resetCat(true); } }),
      h('button', { text: 'Cancel', onclick: () => win.close() })));

  win.el.addEventListener('keydown', (e) => {
    const t = e.target;
    if (t && (t.tagName === 'INPUT' || t.tagName === 'SELECT' || t.tagName === 'TEXTAREA')) { e.stopPropagation(); return; }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { cur = (cur + (e.key === 'ArrowDown' ? 1 : CATEGORIES.length - 1)) % CATEGORIES.length; renderCats(); renderFields(); e.preventDefault(); e.stopPropagation(); }
    else if (e.key === 'Enter') { commit(); win.close(); e.stopPropagation(); }
  });
  win.onResize = () => {};
  renderCats(); renderFields();
  return win;
}

App.ui = App.ui || {};
App.ui.preferences = openPreferences;
App.ui.applyUndoPrefs = applyUndoToggle;
App.ui.ensurePrefDefaults = () => ensurePrefDefaults(App.prefs || (App.prefs = {}));
