// Inca — dock areas: right dock (Channel Box, Attribute Editor, Tool Settings, Modeling Toolkit) with side tabs,
// left dock (Outliner, Tool Settings), workspaces and UI element visibility.
import { App } from '../core/app.js';
import { h, drag } from './dom.js';

const panels = new Map(); // name -> { name, title, side, build, inst, el }
App.ui = App.ui || {};
const R = { current: null, visible: true }; const L = { list: [] };

export function registerPanel(name, title, build, { side = 'right', short = null } = {}) { panels.set(name, { name, title, side, build, short: short || title, inst: null, el: null }); }
App.ui.registerPanel = registerPanel;
function ensure(p) {
  if (!p.el) {
    p.el = h('div', { class: 'dock-panel', style: { display: 'flex', flexDirection: 'column', flex: '1', minHeight: '0' } });
    p.inst = p.build(p.el) || {};
  }
  return p;
}
function rightBody() { return document.getElementById('right-dock-body'); }
export function showRightPanel(name) {
  const p = panels.get(name); if (!p) return;
  ensure(p);
  const body = rightBody();
  for (const c of [...body.children]) c.remove();
  body.append(p.el);
  R.current = name; R.visible = true;
  document.getElementById('right-dock').classList.remove('hidden');
  p.inst.show?.(); p.inst.refresh?.(new Set(['all']));
  App.prefs.rightPanel = name; App.savePrefs();
  syncTabs(); App.requestRender();
}
export function toggleRightPanel(name) {
  if (R.visible && R.current === name) { document.getElementById('right-dock').classList.add('hidden'); R.visible = false; syncTabs(); App.requestRender(); return; }
  showRightPanel(name);
}
App.ui.showRightPanel = showRightPanel; App.ui.toggleRightPanel = toggleRightPanel;
App.ui.rightPanel = () => (R.visible ? R.current : null);

// left dock: stack of panels with titles
export function showLeftPanel(name, on = true) {
  const p = panels.get(name); if (!p) return;
  const dock = document.getElementById('left-dock');
  if (!on) { L.list = L.list.filter(n => n !== name); if (p.wrap) p.wrap.remove(); }
  else if (!L.list.includes(name)) {
    ensure(p); L.list.push(name);
    p.wrap = h('div', { class: 'ldp', style: { display: 'flex', flexDirection: 'column', flex: '1', minHeight: '0', borderBottom: '1px solid #2a2a2a' } },
      h('div', { class: 'ptitle' }, h('span', { text: p.title, style: { flex: '1' } }), h('span', { class: 'wb', text: '✕', title: 'Close', style: { cursor: 'default', padding: '0 4px' }, onclick: () => showLeftPanel(name, false) })), p.el);
    dock.append(p.wrap); p.inst.show?.(); p.inst.refresh?.(new Set(['all']));
  }
  dock.classList.toggle('hidden', L.list.length === 0);
  App.requestRender(); App.dirty('statusline');
}
App.ui.showLeftPanel = showLeftPanel;
App.ui.leftPanelOpen = (name) => L.list.includes(name);
App.ui.toggleOutlinerDock = (force) => { const on = force === undefined ? !L.list.includes('outliner') : force; showLeftPanel('outliner', on); };
App.ui.toggleToolSettings = (force) => { const on = force === true ? true : !L.list.includes('toolSettings'); showLeftPanel('toolSettings', on); };

function syncTabs() {
  const st = document.getElementById('side-tabs'); if (!st) return;
  for (const t of st.children) t.classList.toggle('active', R.visible && t.dataset.panel === R.current);
}
function buildDocks() {
  const st = document.getElementById('side-tabs');
  for (const p of panels.values()) {
    if (p.side !== 'right') continue;
    const t = h('div', { class: 'side-tab', text: p.short, title: p.title });
    t.dataset.panel = p.name;
    t.addEventListener('click', () => toggleRightPanel(p.name));
    st.append(t);
  }
  const rd = document.getElementById('right-dock'), ld = document.getElementById('left-dock');
  rd.style.width = (App.prefs.rightDockWidth || 300) + 'px'; ld.style.width = (App.prefs.outlinerWidth || 240) + 'px';
  drag(rd.querySelector('.dock-resize'), (dx, dy, s) => { rd.style.width = Math.max(200, Math.min(700, s.w - dx)) + 'px'; App.requestRender(); }, () => ({ w: rd.offsetWidth }), () => { App.prefs.rightDockWidth = rd.offsetWidth; App.savePrefs(); });
  drag(ld.querySelector('.dock-resize'), (dx, dy, s) => { ld.style.width = Math.max(160, Math.min(600, s.w + dx)) + 'px'; App.requestRender(); }, () => ({ w: ld.offsetWidth }), () => { App.prefs.outlinerWidth = ld.offsetWidth; App.savePrefs(); });
  showRightPanel(panels.has(App.prefs.rightPanel) ? App.prefs.rightPanel : 'channels');
}
App.on('uiReady', buildDocks);
App.on('refresh', (d) => {
  for (const p of panels.values()) if (p.inst && p.el && p.el.isConnected && p.inst.refresh) { try { p.inst.refresh(d); } catch (e) { console.error('panel refresh', p.name, e); } }
});

// ------------------------------------------------------------------ UI elements & workspaces
const ELEMENTS = { menubar: '#menubar', statusline: '#statusline', shelf: '#shelf', toolbox: '#toolbox', timeslider: '#timeslider', rangeslider: '#rangeslider', cmdline: '#cmdline', helpline: '#helpline', sidetabs: '#side-tabs' };
App.ui.setElementVisible = (k, on) => { const el = document.querySelector(ELEMENTS[k]); if (el) el.classList.toggle('hidden', !on); App.prefs.ui = App.prefs.ui || {}; App.prefs.ui.show = App.prefs.ui.show || {}; App.prefs.ui.show[k] = on; App.savePrefs(); App.requestRender(); };
App.ui.elementVisible = (k) => { const el = document.querySelector(ELEMENTS[k]); return el ? !el.classList.contains('hidden') : false; };
App.ui.elements = Object.keys(ELEMENTS);
let allHidden = null;
App.ui.toggleAllElements = () => {
  if (allHidden) { for (const [k, v] of Object.entries(allHidden)) App.ui.setElementVisible(k, v); allHidden = null; return; }
  allHidden = {}; for (const k of Object.keys(ELEMENTS)) { if (k === 'menubar') continue; allHidden[k] = App.ui.elementVisible(k); App.ui.setElementVisible(k, false); }
  document.getElementById('right-dock').classList.add('hidden'); R.visible = false;
};
export const WORKSPACES = {
  'Maya Classic': { right: 'channels', left: [], hide: [], layout: 'single' },
  'Modeling - Standard': { right: 'mtk', left: ['outliner'], hide: [], layout: 'single' },
  'Modeling - Expert': { right: 'channels', left: [], hide: ['shelf', 'statusline', 'toolbox', 'helpline', 'rangeslider'], layout: 'single' },
  'Sculpting': { right: 'toolSettings', left: [], hide: ['rangeslider'], layout: 'single' },
  'Animation': { right: 'channels', left: ['outliner'], hide: [], layout: 'single', after: () => App.ui.graphEditor?.() },
  'Rigging': { right: 'channels', left: ['outliner'], hide: [], layout: 'single' },
  'Rendering': { right: 'attr', left: ['outliner'], hide: [], layout: 'single', after: () => App.ui.hypershade?.() },
  'UV Editing': { right: 'channels', left: [], hide: [], layout: 'single', after: () => App.ui.uvEditor?.() },
};
App.ui.workspaces = WORKSPACES;
App.ui.applyWorkspace = (name, initial = false) => {
  const w = WORKSPACES[name] || WORKSPACES['Maya Classic'];
  App.prefs.workspace = WORKSPACES[name] ? name : 'Maya Classic'; App.savePrefs();
  for (const k of Object.keys(ELEMENTS)) {
    const saved = App.prefs.ui?.show?.[k];
    const on = initial && saved !== undefined ? saved !== false : !w.hide.includes(k);
    const el = document.querySelector(ELEMENTS[k]); if (el) el.classList.toggle('hidden', !on);
  }
  for (const n of [...L.list]) if (!w.left.includes(n)) showLeftPanel(n, false);
  for (const n of w.left) showLeftPanel(n, true);
  if (!initial) { showRightPanel(w.right); App.setLayout(w.layout, 'persp'); w.after?.(); }
  const sel = document.getElementById('workspace-select'); if (sel) sel.value = App.prefs.workspace;
  App.requestRender();
};
