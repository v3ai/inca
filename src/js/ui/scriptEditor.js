// Inca — Script Editor: command history pane + tabbed MEL / JavaScript input
import { App } from '../core/app.js';
import { runMEL, runJS } from '../core/mel.js';
import { h, menuBar, showMenu, FloatWin, toast, promptDialog } from './dom.js';
import { ICONS } from './icons.js';

const STORE_KEY = 'inca.scriptEditor.tabs';
const MAX_ROWS = 4000;
const TYPE_CLS = { echo: 'c', result: 'r', warning: 'w', error: 'e', input: 'i', print: 'c' };

// local toolbar glyphs (used when icons.js has no suitable generic icon)
const sv = (s) => `<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">${s}</svg>`;
const SE_ICONS = {
  open: sv('<path d="M3 6 H9 L11 8 H20 V18 H3Z" fill="#e8c26b" stroke="#222" stroke-width=".8"/><path d="M3 18 L6 11 H22 L19 18Z" fill="#f2d48a" stroke="#222" stroke-width=".8"/>'),
  save: sv('<path d="M4 4 H17 L20 7 V20 H4Z" fill="#5bb3e6" stroke="#222" stroke-width=".8"/><rect x="7" y="4" width="9" height="5" fill="#e8e8e8"/><rect x="7" y="13" width="10" height="7" fill="#2f6f99"/>'),
  clearHist: sv('<rect x="3" y="3" width="18" height="9" fill="#2b2b2b" stroke="#b8b8b8" stroke-width="1"/><path d="M5 6 H15 M5 9 H12" stroke="#9fd59f" stroke-width="1.2"/><rect x="3" y="14" width="18" height="7" fill="none" stroke="#777" stroke-width="1"/><path d="M14 4 L20 11 M20 4 L14 11" stroke="#e5574f" stroke-width="1.8"/>'),
  clearInput: sv('<rect x="3" y="3" width="18" height="7" fill="none" stroke="#777" stroke-width="1"/><rect x="3" y="12" width="18" height="9" fill="#2b2b2b" stroke="#b8b8b8" stroke-width="1"/><path d="M5 15 H13 M5 18 H11" stroke="#8fb8de" stroke-width="1.2"/><path d="M14 13 L20 20 M20 13 L14 20" stroke="#e5574f" stroke-width="1.8"/>'),
  clearAll: sv('<rect x="3" y="3" width="18" height="18" fill="#2b2b2b" stroke="#b8b8b8" stroke-width="1"/><path d="M3 12 H21" stroke="#b8b8b8"/><path d="M7 7 L17 17 M17 7 L7 17" stroke="#e5574f" stroke-width="2"/>'),
  execAll: sv('<path d="M5 4 L13 12 L5 20Z" fill="#7ccf6b" stroke="#222" stroke-width=".8"/><path d="M12 4 L20 12 L12 20Z" fill="#7ccf6b" stroke="#222" stroke-width=".8"/>'),
  exec: sv('<path d="M7 4 L18 12 L7 20Z" fill="#7ccf6b" stroke="#222" stroke-width=".8"/>'),
  shelf: sv('<rect x="2" y="9" width="20" height="9" fill="#5d5d5d" stroke="#222" stroke-width=".8"/><path d="M12 2 V8 M9 5 H15" stroke="#7ccf6b" stroke-width="2"/>'),
};
const seIcon = (name) => SE_ICONS[name] || ICONS[name] || ICONS.help;

function injectStyle() {
  if (document.getElementById('inca-se-style')) return;
  const css = `
.sewin .ptoolbar { border-bottom: 1px solid #2a2a2a; }
.sewin .se-tb { width: 24px; height: 22px; display: inline-flex; align-items: center; justify-content: center; border-radius: 3px; }
.sewin .se-tb svg { width: 18px; height: 18px; } .sewin .se-tb:hover { background: #575757; }
.sewin .se-tbsep { width: 1px; height: 16px; background: #2a2a2a; margin: 0 4px; }
.sewin .se-hist { margin: 2px 4px 0; min-height: 30px; flex: none; }
.sewin .se-hist .row { min-height: 15px; }
.sewin .se-split { height: 6px; cursor: row-resize; flex: none; }
.sewin .se-split:hover { background: #555; }
.sewin .se-bottom { flex: 1; display: flex; flex-direction: column; min-height: 60px; }
.sewin .se-tab { padding: 2px 12px; background: #3a3a3a; border: 1px solid #2a2a2a; border-bottom: none; border-radius: 3px 3px 0 0; font-size: 11.5px; cursor: default; white-space: nowrap; }
.sewin .se-tab.on { background: #262626; color: #fff; }
.sewin .se-tab:hover:not(.on) { background: #474747; }
.sewin .se-tab input { width: 90px; height: 16px; }
.sewin .se-addtab { padding: 0 8px; font-size: 14px; color: #bbb; cursor: default; }
.sewin .se-addtab:hover { color: #fff; }
.sewin .se-edit { flex: 1; display: flex; margin: 0 4px 4px; background: #262626; min-height: 0; border: 1px solid #222; }
.sewin .se-gutter { flex: none; min-width: 32px; padding: 4px 6px 4px 4px; text-align: right; color: #6f6f6f; background: #2e2e2e; font: 12.5px Consolas, "Courier New", monospace; line-height: 1.35; overflow: hidden; white-space: pre; user-select: none; }
.sewin .se-edit textarea.se-input { margin: 0; border: none; border-radius: 0; flex: 1; white-space: pre; overflow: auto; tab-size: 4; }
.sewin .se-edit textarea.se-input:focus { border: none; }
.sewin .se-status { height: 18px; font-size: 11px; color: #999; padding: 0 6px; display: flex; align-items: center; gap: 12px; flex: none; }
`;
  document.head.append(h('style', { id: 'inca-se-style', text: css }));
}

// ---------------------------------------------------------------- persistent state
function loadTabs() {
  try { const t = JSON.parse(localStorage.getItem(STORE_KEY) || 'null'); if (Array.isArray(t) && t.length && t.every(x => x && typeof x.name === 'string')) return t; } catch { /* ignore */ }
  return [{ name: 'MEL', lang: 'mel', text: '' }, { name: 'JavaScript', lang: 'js', text: '' }];
}
function saveTabs(tabs, active) {
  try { localStorage.setItem(STORE_KEY, JSON.stringify(tabs.map(t => ({ name: t.name, lang: t.lang, text: t.text })))); localStorage.setItem(STORE_KEY + '.active', String(active)); } catch { /* ignore */ }
}
function seOpts() {
  if (!App.prefs) App.prefs = {};
  if (!App.prefs.opt) App.prefs.opt = {};
  const o = App.prefs.opt.scriptEditor || (App.prefs.opt.scriptEditor = {});
  for (const [k, v] of Object.entries({ echoAll: false, stackTrace: false, lineNumbersInErrors: true, showLineNumbers: true, split: 0.5 })) if (!(k in o)) o[k] = v;
  return o;
}
const savePrefs = () => { try { App.savePrefs?.(); } catch { /* ignore */ } };

// ---------------------------------------------------------------- log helpers
export function logPush(entry) {
  if (typeof App.logPush === 'function') { App.logPush(entry); return; }
  if (!App.scriptLog) App.scriptLog = [];
  App.scriptLog.push(entry);
  App.emit('log', entry);
}
// display text for an entry, adding Maya prefixes where the emitter left them out
export function entryText(e) {
  const t = String(e.text ?? '');
  if (e.type === 'result' && !t.startsWith('//')) return '// Result: ' + t + ' //';
  if (e.type === 'error' && !t.startsWith('//')) return '// Error: ' + t;
  if (e.type === 'warning' && !t.startsWith('//')) return '// Warning: ' + t;
  return t;
}
// convert a JS stack line number to a 1-based line of the user's source (see wrapJS)
let lineBase = null;
function jsLineBase() {
  if (lineBase !== null) return lineBase;
  try { new Function('"use strict";\nthrow new Error("x")')(); } catch (e) { const m = /<anonymous>:(\d+):\d+/.exec(e.stack || ''); lineBase = m ? +m[1] : -1; }
  return lineBase;
}
export function jsErrorLine(stack, base) {
  const m = /<anonymous>:(\d+):\d+/.exec(stack || '');
  if (!m || base < 0) return null;
  const n = +m[1] - base;
  return n >= 1 ? n : null;
}

// ---------------------------------------------------------------- echo-all (wrap command runner)
let origRun = null;
function setEchoAll(on) {
  if (!App.cmds) return;
  if (on && !origRun) {
    origRun = App.cmds.run;
    App.cmds.run = function (id, args) { App.emit('echo', id + ';'); return origRun.call(this, id, args); };
  } else if (!on && origRun) { App.cmds.run = origRun; origRun = null; }
}

// ---------------------------------------------------------------- window
let inst = null;
export function openScriptEditor() {
  if (inst && FloatWin.isOpen('scriptEditor')) { FloatWin.get('scriptEditor').focus(); inst.focusInput(); return inst; }
  injectStyle();
  const opts = seOpts();
  setEchoAll(opts.echoAll);
  const win = new FloatWin('scriptEditor', 'Script Editor', { w: 760, h: 560, minW: 360, minH: 240 });
  win.el.classList.add('sewin');
  const tabs = loadTabs();
  let active = 0;
  try { active = Math.min(tabs.length - 1, Math.max(0, parseInt(localStorage.getItem(STORE_KEY + '.active') || '0', 10) || 0)); } catch { /* ignore */ }

  // ---- history pane
  const hist = h('div', { class: 'se-hist' });
  let rowCount = 0;
  const addRow = (e) => {
    const atBottom = hist.scrollTop + hist.clientHeight >= hist.scrollHeight - 4;
    hist.append(h('div', { class: 'row ' + (TYPE_CLS[e.type] || 'c'), text: entryText(e) }));
    if (++rowCount > MAX_ROWS) { hist.firstChild.remove(); rowCount--; }
    if (atBottom) hist.scrollTop = hist.scrollHeight;
  };
  const renderHistory = () => {
    hist.innerHTML = ''; rowCount = 0;
    const log = App.scriptLog || [];
    const frag = document.createDocumentFragment();
    for (const e of log.slice(-MAX_ROWS)) { frag.append(h('div', { class: 'row ' + (TYPE_CLS[e.type] || 'c'), text: entryText(e) })); rowCount++; }
    hist.append(frag); hist.scrollTop = hist.scrollHeight;
  };
  // capture output emitted while we run a script so nothing is lost if the shell doesn't log it
  let capturing = null;
  const offLog = App.on('log', (e) => { if (capturing) capturing.logs.push(e); addRow(e); });
  const offs = [offLog];
  for (const ev of ['echo', 'result', 'warning', 'error', 'print']) offs.push(App.on(ev, (text) => { if (capturing) capturing.events.push({ type: ev, text: String(text) }); }));

  // ---- splitter
  const split = h('div', { class: 'se-split', title: 'Drag to resize' });

  // ---- tab bar + editor
  const tabBar = h('div', { class: 'se-tabs' });
  const gutter = h('div', { class: 'se-gutter' });
  const ta = h('textarea', { class: 'se-input', spellcheck: 'false', wrap: 'off' });
  const status = h('div', { class: 'se-status' });
  const edit = h('div', { class: 'se-edit' }, gutter, ta);
  const bottom = h('div', { class: 'se-bottom' }, tabBar, edit, status);

  const updateGutter = () => {
    gutter.classList.toggle('hidden', !opts.showLineNumbers);
    if (!opts.showLineNumbers) return;
    const n = ta.value.split('\n').length;
    let s = ''; for (let i = 1; i <= n; i++) s += i + '\n';
    gutter.textContent = s; gutter.scrollTop = ta.scrollTop;
  };
  const updateStatus = () => {
    const pre = ta.value.slice(0, ta.selectionStart);
    const line = pre.split('\n').length, col = ta.selectionStart - pre.lastIndexOf('\n');
    const t = tabs[active];
    status.textContent = `${t.lang === 'js' ? 'JavaScript' : 'MEL'}   Ln ${line}, Col ${col}` + (ta.selectionEnd > ta.selectionStart ? `   (${ta.selectionEnd - ta.selectionStart} selected)` : '');
  };
  const stash = () => { tabs[active].text = ta.value; saveTabs(tabs, active); };
  const renderTabs = () => {
    tabBar.innerHTML = '';
    tabs.forEach((t, i) => {
      const el = h('div', { class: 'se-tab' + (i === active ? ' on' : ''), text: t.name, title: (t.lang === 'js' ? 'JavaScript' : 'MEL') + ' tab — double-click to rename, right-click for options' });
      el.addEventListener('mousedown', (e) => { if (e.button === 0 && i !== active) { stash(); active = i; loadActive(); } });
      el.addEventListener('dblclick', () => renameTab(i, el));
      el.addEventListener('contextmenu', (e) => {
        e.preventDefault();
        showMenu([
          { label: 'Rename Tab…', fn: () => renameTab(i, el) },
          { label: 'MEL', check: () => t.lang === 'mel', fn: () => { t.lang = 'mel'; stash(); renderTabs(); updateStatus(); } },
          { label: 'JavaScript', check: () => t.lang === 'js', fn: () => { t.lang = 'js'; stash(); renderTabs(); updateStatus(); } },
          '-',
          { label: 'Delete Tab', enabled: () => tabs.length > 1, fn: () => deleteTab(i) },
        ], e.clientX, e.clientY);
      });
      tabBar.append(el);
    });
    const add = h('div', { class: 'se-addtab', text: '+', title: 'Add a new tab' });
    add.addEventListener('click', (e) => {
      const r = add.getBoundingClientRect();
      showMenu([{ label: 'New MEL Tab', fn: () => addTab('mel') }, { label: 'New JavaScript Tab', fn: () => addTab('js') }], r.left, r.bottom);
      e.stopPropagation();
    });
    tabBar.append(add);
  };
  const loadActive = () => { ta.value = tabs[active].text || ''; renderTabs(); updateGutter(); updateStatus(); saveTabs(tabs, active); ta.focus(); };
  const addTab = (lang) => {
    stash();
    const base = lang === 'js' ? 'JavaScript' : 'MEL';
    let n = 2; while (tabs.some(t => t.name === base + n)) n++;
    tabs.push({ name: base + n, lang, text: '' }); active = tabs.length - 1; loadActive();
  };
  const deleteTab = (i) => { if (tabs.length < 2) return; stash(); tabs.splice(i, 1); active = Math.min(active, tabs.length - 1); loadActive(); };
  const renameTab = (i, el) => {
    const inp = h('input', { value: tabs[i].name });
    el.textContent = ''; el.append(inp); inp.focus(); inp.select();
    let done = false;
    const fin = (ok) => { if (done) return; done = true; const v = inp.value.trim(); if (ok && v) tabs[i].name = v; saveTabs(tabs, active); renderTabs(); };
    inp.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') fin(true); if (e.key === 'Escape') fin(false); });
    inp.addEventListener('blur', () => fin(true));
    inp.addEventListener('mousedown', (e) => e.stopPropagation());
  };

  // ---- execution
  const execute = (all = false) => {
    const t = tabs[active];
    const hasSel = !all && ta.selectionEnd > ta.selectionStart;
    const src = hasSel ? ta.value.slice(ta.selectionStart, ta.selectionEnd) : ta.value;
    if (!src.trim()) return;
    logPush({ type: 'input', text: src.replace(/\s+$/, '') });
    capturing = { events: [], logs: [] };
    try {
      if (t.lang === 'js') runJSWithOptions(src, opts);
      else runMEL(src);
    } catch (e) { App.emit('error', '// Error: ' + (e.message || e)); }
    const cap = capturing; capturing = null;
    // anything emitted but not logged by the shell: log it ourselves
    if (!cap.logs.length) { for (const ev of cap.events) logPush({ type: ev.type === 'print' ? 'echo' : ev.type, text: ev.text }); }
    else for (const ev of cap.events) if (ev.type === 'print' && !cap.logs.some(l => String(l.text).includes(ev.text))) logPush({ type: 'echo', text: ev.text });
    if (!hasSel) { ta.value = ''; stash(); updateGutter(); updateStatus(); }
  };

  // ---- file ops
  const native = () => window.incaNative;
  const scriptFilters = [{ name: 'Scripts', extensions: ['mel', 'js'] }, { name: 'MEL', extensions: ['mel'] }, { name: 'JavaScript', extensions: ['js'] }, { name: 'All Files', extensions: ['*'] }];
  const pickFile = async () => {
    const n = native();
    if (n?.openDialog) {
      const p = await n.openDialog({ title: 'Load Script', filters: scriptFilters, defaultPath: App.project ? App.project + '/scripts' : undefined });
      if (!p) return null;
      return { name: p.split(/[\\/]/).pop(), text: await n.readText(p) };
    }
    return new Promise((res) => {
      const inp = h('input', { type: 'file', accept: '.mel,.js,.txt' });
      inp.addEventListener('change', () => { const f = inp.files[0]; if (!f) return res(null); f.text().then(text => res({ name: f.name, text })); });
      inp.click();
    });
  };
  const langOf = (name) => /\.js$/i.test(name) ? 'js' : 'mel';
  const loadScript = async () => {
    try {
      const f = await pickFile(); if (!f) return;
      stash();
      const lang = langOf(f.name);
      const cur = tabs[active];
      if (!cur.text.trim() && cur.lang === lang) { cur.text = f.text; cur.name = f.name; }
      else { tabs.push({ name: f.name, lang, text: f.text }); active = tabs.length - 1; }
      loadActive();
    } catch (e) { App.emit('error', '// Error: Could not load script: ' + (e.message || e)); }
  };
  const sourceScript = async () => {
    try {
      const f = await pickFile(); if (!f) return;
      logPush({ type: 'echo', text: `source "${f.name}";` });
      if (langOf(f.name) === 'js') runJSWithOptions(f.text, opts); else runMEL(f.text);
    } catch (e) { App.emit('error', '// Error: Could not source script: ' + (e.message || e)); }
  };
  const saveScript = async () => {
    const t = tabs[active]; stash();
    const ext = t.lang === 'js' ? 'js' : 'mel';
    const def = (/\.(mel|js)$/i.test(t.name) ? t.name : t.name.replace(/\s+/g, '_') + '.' + ext);
    try {
      const n = native();
      if (n?.saveDialog) {
        const p = await n.saveDialog({ title: 'Save Script', defaultPath: (App.project ? App.project + '/scripts/' : '') + def, filters: scriptFilters });
        if (!p) return;
        await n.writeText(p, ta.value);
        t.name = p.split(/[\\/]/).pop(); renderTabs(); saveTabs(tabs, active);
        toast('Saved ' + t.name);
      } else {
        const a = h('a', { href: URL.createObjectURL(new Blob([ta.value], { type: 'text/plain' })), download: def });
        document.body.append(a); a.click(); setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 1000);
      }
    } catch (e) { App.emit('error', '// Error: Could not save script: ' + (e.message || e)); }
  };
  const saveToShelf = async () => {
    const t = tabs[active];
    const hasSel = ta.selectionEnd > ta.selectionStart;
    const src = hasSel ? ta.value.slice(ta.selectionStart, ta.selectionEnd) : ta.value;
    if (!src.trim()) { App.emit('warning', '// Warning: Nothing to save to shelf.'); return; }
    if (!App.shelf || !Array.isArray(App.shelf.shelves)) { App.emit('warning', '// Warning: No shelf available.'); return; }
    const ask = App.ui?.prompt || promptDialog;
    const label = await ask('Save Script to Shelf', 'Enter Name:', 'script');
    if (!label) return;
    const sh = App.shelf.shelves[App.shelf.current ?? 0] || App.shelf.shelves[0];
    if (!sh) return;
    const item = { label, icon: 'scriptEditor' }; item[t.lang === 'js' ? 'js' : 'mel'] = src;
    sh.items.push(item);
    App.shelf.render?.(); App.shelf.save?.();
    App.emit('result', `Added "${label}" to shelf ${sh.name}`);
  };

  // ---- clears
  const clearHistory = () => { if (App.scriptLog) App.scriptLog.length = 0; hist.innerHTML = ''; rowCount = 0; };
  const clearInput = () => { ta.value = ''; stash(); updateGutter(); updateStatus(); ta.focus(); };

  // ---- menus
  const menuRow = h('div', { class: 'pmenubar' });
  const toggle = (k, after) => ({ check: () => !!opts[k], fn: () => { opts[k] = !opts[k]; savePrefs(); after?.(); } });
  menuBar(menuRow, [
    { label: 'File', items: [
      { label: 'Load Script…', fn: loadScript },
      { label: 'Source Script…', fn: sourceScript },
      { label: 'Save Script…', fn: saveScript, hk: 'Ctrl+S' },
      { label: 'Save Script to Shelf…', fn: saveToShelf },
      '-',
      { label: 'Close', fn: () => win.close() },
    ] },
    { label: 'Edit', items: [
      { label: 'Undo', fn: () => { ta.focus(); document.execCommand('undo'); } },
      { label: 'Redo', fn: () => { ta.focus(); document.execCommand('redo'); } },
      '-',
      { label: 'Clear History', fn: clearHistory },
      { label: 'Clear Input', fn: clearInput },
      { label: 'Clear All', fn: () => { clearHistory(); clearInput(); } },
      '-',
      { label: 'Select All', fn: () => { ta.focus(); ta.select(); updateStatus(); }, hk: 'Ctrl+A' },
      { label: 'New Tab', sub: [{ label: 'MEL', fn: () => addTab('mel') }, { label: 'JavaScript', fn: () => addTab('js') }] },
      { label: 'Rename Tab…', fn: () => renameTab(active, tabBar.children[active]) },
      { label: 'Delete Tab', enabled: () => tabs.length > 1, fn: () => deleteTab(active) },
    ] },
    { label: 'History', items: [
      { label: 'Echo All Commands', ...toggle('echoAll', () => setEchoAll(opts.echoAll)) },
      { label: 'Show Stack Trace', ...toggle('stackTrace') },
      { label: 'Line Numbers in Errors', ...toggle('lineNumbersInErrors') },
    ] },
    { label: 'Command', items: [
      { label: 'Execute', fn: () => execute(false), hk: 'Ctrl+Enter' },
      { label: 'Execute All', fn: () => execute(true) },
      '-',
      { label: 'Show Line Numbers', ...toggle('showLineNumbers', updateGutter) },
    ] },
    { label: 'Help', items: [
      { label: 'Help on Script Editor', fn: () => logPush({ type: 'echo', text: '// Script Editor: Ctrl+Enter or keypad Enter executes the selection (or everything). MEL tabs use the Inca MEL interpreter; JavaScript tabs get `cmds`, `inca`, `THREE`, `print(...)` and `mel(src)`.' }) },
      { label: 'MEL Command Reference', fn: () => { const n = App.mel?.MEL ? Object.keys(App.mel.MEL).sort() : []; logPush({ type: 'echo', text: '// MEL commands (' + n.length + '): ' + n.join(', ') }); } },
    ] },
  ]);

  // ---- toolbar
  const tb = (name, title, fn) => { const b = h('div', { class: 'se-tb', title, html: seIcon(name) }); b.addEventListener('click', fn); b.addEventListener('mouseenter', () => App.help(title)); return b; };
  const sep = () => h('div', { class: 'se-tbsep' });
  const toolbar = h('div', { class: 'ptoolbar' },
    tb('open', 'Load Script', loadScript), tb('save', 'Save Script', saveScript), tb('shelf', 'Save Script to Shelf', saveToShelf), sep(),
    tb('clearHist', 'Clear History', clearHistory), tb('clearInput', 'Clear Input', clearInput), tb('clearAll', 'Clear All', () => { clearHistory(); clearInput(); }), sep(),
    tb('execAll', 'Execute All', () => execute(true)), tb('exec', 'Execute (selection or all)', () => execute(false)));

  win.body.append(menuRow, toolbar, hist, split, bottom);

  // ---- layout / splitter
  const layout = () => {
    const avail = win.body.clientHeight - menuRow.offsetHeight - toolbar.offsetHeight - split.offsetHeight;
    if (avail <= 0) return;
    const hh = Math.round(Math.min(avail - 80, Math.max(30, avail * opts.split)));
    hist.style.height = Math.max(30, hh) + 'px';
  };
  split.addEventListener('mousedown', (e) => {
    if (e.button !== 0) return; e.preventDefault();
    const y0 = e.clientY, h0 = hist.offsetHeight;
    const avail = win.body.clientHeight - menuRow.offsetHeight - toolbar.offsetHeight - split.offsetHeight;
    const mm = (ev) => { opts.split = Math.min(0.9, Math.max(0.08, (h0 + ev.clientY - y0) / avail)); layout(); };
    const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); savePrefs(); };
    addEventListener('mousemove', mm); addEventListener('mouseup', mu);
  });
  win.onResize = layout;

  // ---- editor keys
  ta.addEventListener('keydown', (e) => {
    if ((e.key === 'Enter' && (e.ctrlKey || e.metaKey)) || (e.code === 'NumpadEnter' && !e.shiftKey)) { e.preventDefault(); execute(false); }
    else if (e.key === 'Tab' && !e.ctrlKey && !e.altKey) {
      e.preventDefault();
      const s = ta.selectionStart, en = ta.selectionEnd, v = ta.value;
      if (s !== en && v.slice(s, en).includes('\n')) { // block indent / outdent
        const ls = v.lastIndexOf('\n', s - 1) + 1;
        const block = v.slice(ls, en);
        const out = e.shiftKey ? block.replace(/^( {1,4}|\t)/gm, '') : block.replace(/^/gm, '    ');
        ta.setRangeText(out, ls, en, 'select');
      } else if (e.shiftKey) {
        const ls = v.lastIndexOf('\n', s - 1) + 1; const m = /^ {1,4}/.exec(v.slice(ls));
        if (m) { ta.setRangeText('', ls, ls + m[0].length, 'preserve'); }
      } else ta.setRangeText('    ', s, en, 'end');
      stash(); updateGutter();
    } else if (e.key === 's' && (e.ctrlKey || e.metaKey)) { e.preventDefault(); saveScript(); }
    e.stopPropagation();
  });
  ta.addEventListener('input', () => { stash(); updateGutter(); updateStatus(); });
  ta.addEventListener('scroll', () => { gutter.scrollTop = ta.scrollTop; });
  for (const ev of ['keyup', 'click', 'select']) ta.addEventListener(ev, updateStatus);
  // keep global hotkeys from firing while the window has focus
  win.el.addEventListener('keydown', (e) => { if (e.key !== 'Escape' || e.target === ta) e.stopPropagation(); });
  win.el.addEventListener('keyup', (e) => e.stopPropagation());

  win.onClose = () => { for (const off of offs) off(); stash(); inst = null; };

  renderHistory();
  loadActive();
  requestAnimationFrame(layout);
  inst = { win, execute, focusInput: () => ta.focus(), clearHistory, setText: (s) => { ta.value = s; stash(); updateGutter(); } };
  return inst;
}

function runJSWithOptions(src, opts) {
  if (!opts.stackTrace && !opts.lineNumbersInErrors) return runJS(src);
  const base = jsLineBase();
  globalThis.__incaSEErr = (e) => {
    const line = opts.lineNumbersInErrors ? jsErrorLine(e && e.stack, base) : null;
    App.emit('error', '// Error: ' + (line ? `Line ${line}: ` : '') + (e && e.message || e));
    if (opts.stackTrace && e && e.stack) {
      // keep frames up to the last one in user code; show user-code positions as script lines
      const all = String(e.stack).split('\n').slice(1).map(l => l.trim()).filter(Boolean);
      let last = -1; all.forEach((l, i) => { if (/eval at runJS/.test(l)) last = i; });
      const frames = last >= 0 ? all.slice(0, last + 1) : all.filter(l => !/scriptEditor\.js/.test(l));
      for (const f of frames) {
        const t = f.replace(/^at eval \(eval at runJS \([^)]*\), <anonymous>:(\d+):(\d+)\)$/, (m, l, c) => `at <script> (line ${+l - base}:${c})`)
          .replace(/\(eval at runJS \([^)]*\), <anonymous>:(\d+):(\d+)\)/, (m, l, c) => `(line ${+l - base}:${c})`);
        App.emit('warning', '//   ' + t);
      }
    }
  };
  // src starts on the 3rd body line (after "use strict" + try), which matches jsLineBase()'s reference line + 1
  try { return runJS(`try {\n${src}\n} catch (__e) { globalThis.__incaSEErr(__e); return undefined; }`); }
  finally { delete globalThis.__incaSEErr; }
}

App.ui = App.ui || {};
App.ui.scriptEditor = openScriptEditor;
