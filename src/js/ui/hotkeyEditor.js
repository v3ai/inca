// Inca — Hotkey Editor: keyboard picture + searchable command list with key assignment
import { App } from '../core/app.js';
import { h, FloatWin, confirmDialog, showMenu, toast } from './dom.js';

// ---------------------------------------------------------------- pure helpers
const MOD_KEYS = new Set(['Control', 'Shift', 'Alt', 'Meta', 'AltGraph', 'CapsLock', 'OS', 'Hyper', 'Super']);
const SPECIAL = { ' ': 'Space', Esc: 'Escape', Del: 'Delete', Spacebar: 'Space' };
const SHIFTED = { '`': '~', '1': '!', '2': '@', '3': '#', '4': '$', '5': '%', '6': '^', '7': '&', '8': '*', '9': '(', '0': ')', '-': '_', '=': '+', '[': '{', ']': '}', '\\': '|', ';': ':', "'": '"', ',': '<', '.': '>', '/': '?' };
// key part of a combo from a KeyboardEvent-like {key, code}
export function keyName(e) {
  const code = e.code || '';
  let m = /^Key([A-Z])$/.exec(code); if (m) return m[1];
  m = /^Digit(\d)$/.exec(code); if (m) return m[1];
  const k = e.key;
  if (k === undefined || k === null) return '';
  if (SPECIAL[k]) return SPECIAL[k];
  if (k.length === 1) return k.toUpperCase();
  return k;
}
// "Ctrl+Alt+Shift+K" built from an event; null for a lone modifier press
export function comboFromEvent(e) {
  if (MOD_KEYS.has(e.key)) return null;
  const k = keyName(e); if (!k) return null;
  const parts = [];
  if (e.ctrlKey || e.metaKey) parts.push('Ctrl');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');
  parts.push(k);
  return parts.join('+');
}
// canonical form for comparisons (modifier order Ctrl, Alt, Shift; letters upper-case)
export function normalizeCombo(c) {
  if (!c) return '';
  const s = String(c).trim();
  const parts = s === '+' ? ['+'] : s.split('+').reduce((acc, p, i, arr) => { if (p === '' && i === arr.length - 1) acc.push('+'); else if (p !== '') acc.push(p); return acc; }, []);
  const mods = new Set(); let key = '';
  for (const p of parts) {
    const l = p.toLowerCase();
    if (l === 'ctrl' || l === 'control' || l === 'cmd' || l === 'meta') mods.add('Ctrl');
    else if (l === 'alt' || l === 'option') mods.add('Alt');
    else if (l === 'shift') mods.add('Shift');
    else key = p.length === 1 ? p.toUpperCase() : (SPECIAL[p] || p);
  }
  return [...['Ctrl', 'Alt', 'Shift'].filter(m => mods.has(m)), key].join('+');
}
// key name as seen on the keyboard picture for a given layer
export function layerKey(base, shift) { return shift && !/^\d$/.test(base) && SHIFTED[base] ? SHIFTED[base] : base; }

const CAT_RULES = [
  ['Windows', /(Editor|Window|^attributeEditor|^channelBox|^toolSettings|^modelingToolkit|^outliner|^hypershade|^hypergraph|^renderView|^dopeSheet|^preferences|^about|^hotkeyList|^playblast|^uvEditor|^renderSettings)/],
  ['File', /^(new|open|save|import|export|reference|project|quit|exit)/i],
  ['Edit', /^(undo|redo|repeatLast|delete|duplicate|group|ungroup|parent|unparent|copy|cut|paste|prefix|searchReplace|rename)/],
  ['Create', /^(poly(Cube|Sphere|Cylinder|Cone|Torus|Plane|Disc|Platonic|Pyramid|Prism|Pipe|Helix|Soccer)|nurbs|create|cvCurve|epCurve|camera|.*Light$)/],
  ['Select', /^(select|deselect|invert|grow|shrink|to(Vertices|Edges|Faces)|objectMode|vertexMode|edgeMode|faceMode|uvMode)/],
  ['Tools', /(Tool$|^pivotEdit|^softSelect|^manip)/],
  ['Modify', /^(reset|freeze|center|match|snap|convert)/],
  ['Display', /^(hide|show|template|untemplate|smooth(Off|Cage|On)|shade|toggle|isolate|cycleBackground|frame|hud|layout)/],
  ['Animation', /^(setKey|play|stop|next|prev|goTo|toggleAutoKey|delete?Keys|copyKeys|pasteKeys|bake|key|tan)/],
  ['Rendering', /^(render|ipr|assign|createMaterial|material|standardSurface|lambert|blinn|phong|surfaceShader|checker|fileTex|ramp|noise)/i],
  ['Curves / Surfaces', /^(revolve|loft|extrudeCurve|planarSurface|rebuildCurve|reverseCurve|openCloseCurve)/],
  ['Mesh', /^(extrude|bevel|chamfer|bridge|addDivisions|merge|collapse|poke|triangulate|quadrangulate|reverseNormals|fillHole|mirror|smooth|combine|separate|extract|duplicateFaces|cleanup|connect|soften|harden|softHard|uv|planarMap|autoMap)/],
];
export function categoryOf(c) {
  if (c.category) return c.category;
  if (c.cat) return c.cat;
  const id = c.id || '';
  for (const [name, re] of CAT_RULES) if (re.test(id)) return name;
  return 'Other';
}

// ---------------------------------------------------------------- keyboard layout (widths in key units)
const K = (label, key = label, w = 1, mod = null) => ({ label, key, w, mod });
const ROWS = [
  [K('Esc', 'Escape'), { gap: 0.5 }, ...[1, 2, 3, 4].map(i => K('F' + i)), { gap: 0.25 }, ...[5, 6, 7, 8].map(i => K('F' + i)), { gap: 0.25 }, ...[9, 10, 11, 12].map(i => K('F' + i))],
  [K('`'), ...'1234567890'.split('').map(c => K(c)), K('-'), K('='), K('Bksp', 'Backspace', 2)],
  [K('Tab', 'Tab', 1.5), ...'QWERTYUIOP'.split('').map(c => K(c)), K('['), K(']'), K('\\', '\\', 1.5)],
  [K('Caps', '', 1.75), ...'ASDFGHJKL'.split('').map(c => K(c)), K(';'), K("'"), K('Enter', 'Enter', 2.25)],
  [K('Shift', '', 2.25, 'Shift'), ...'ZXCVBNM'.split('').map(c => K(c)), K(','), K('.'), K('/'), K('Shift', '', 2.75, 'Shift')],
  [K('Ctrl', '', 1.5, 'Ctrl'), K('', '', 1.25), K('Alt', '', 1.25, 'Alt'), K('Space', 'Space', 6.25), K('Alt', '', 1.25, 'Alt'), K('', '', 1.25), K('Ctrl', '', 1.5, 'Ctrl')],
];
const NAV = [
  [K('PrtSc', ''), K('ScrLk', ''), K('Pause', 'Pause')],
  [K('Ins', 'Insert'), K('Home'), K('PgUp', 'PageUp')],
  [K('Del', 'Delete'), K('End'), K('PgDn', 'PageDown')],
  [null, null, null],
  [null, K('↑', 'ArrowUp'), null],
  [K('←', 'ArrowLeft'), K('↓', 'ArrowDown'), K('→', 'ArrowRight')],
];
const U = 30; // px per key unit

function injectStyle() {
  if (document.getElementById('inca-hk-style')) return;
  document.head.append(h('style', { id: 'inca-hk-style', text: `
.hkwin .hk-main { flex: 1; display: flex; min-height: 0; gap: 6px; padding: 6px; }
.hkwin .hk-left { flex: 1.35; display: flex; flex-direction: column; min-width: 0; gap: 6px; }
.hkwin .hk-right { flex: 1; display: flex; flex-direction: column; min-width: 240px; gap: 4px; }
.hkwin .hk-kbwrap { background: #2f2f2f; border: 1px solid #222; padding: 8px; overflow: hidden; flex: none; }
.hkwin .hk-kb { position: relative; transform-origin: 0 0; }
.hkwin .hk-key { position: absolute; height: ${U - 3}px; background: #5a5a5a; border: 1px solid #262626; border-radius: 3px; color: #ddd; font-size: 10.5px; display: flex; align-items: flex-start; justify-content: flex-start; padding: 2px 3px; overflow: hidden; white-space: nowrap; box-shadow: inset 0 -2px 0 rgba(0,0,0,.25); }
.hkwin .hk-key.bound { background: #4f7d99; color: #fff; }
.hkwin .hk-key.cmdkey { background: #c58b3a; color: #fff; }
.hkwin .hk-key.modon { background: #8a8a8a; color: #111; }
.hkwin .hk-key.mod { cursor: pointer; }
.hkwin .hk-key.blank { background: #484848; color: #777; }
.hkwin .hk-key:hover { outline: 1px solid #e6c54a; z-index: 1; }
.hkwin .hk-mods { display: flex; align-items: center; gap: 12px; flex: none; }
.hkwin .hk-info { background: #2b2b2b; border: 1px solid #222; padding: 6px 8px; min-height: 52px; line-height: 1.5; flex: none; }
.hkwin .hk-info b { color: #fff; }
.hkwin .hk-layer { flex: 1; min-height: 0; }
.hkwin .hk-layer .li { display: flex; gap: 8px; cursor: default; } .hkwin .hk-layer .li .k { width: 110px; color: #9fc6e0; flex: none; }
.hkwin .hk-ttl { color: #aaa; font-size: 11px; flex: none; }
.hkwin .hk-row .id { color: #8a8a8a; font-size: 10.5px; margin-left: 6px; }
.hkwin .hk-legend { display: flex; gap: 14px; font-size: 11px; color: #aaa; flex: none; align-items: center; }
.hkwin .hk-legend i { display: inline-block; width: 12px; height: 10px; border-radius: 2px; margin-right: 4px; vertical-align: middle; }
.hkwin .hk-search { display: flex; gap: 4px; align-items: center; }
.hkwin .hk-search input { flex: 1; }
.hkwin .hk-list { flex: 1; min-height: 0; }
.hkwin .hk-row { display: flex; padding: 2px 6px; white-space: nowrap; }
.hkwin .hk-row .c1 { flex: 1; overflow: hidden; text-overflow: ellipsis; }
.hkwin .hk-row .c2 { width: 120px; text-align: right; color: #9fc6e0; overflow: hidden; text-overflow: ellipsis; }
.hkwin .hk-row.sel { background: var(--hi); color: #fff; } .hkwin .hk-row.sel .c2 { color: #fff; }
.hkwin .hk-row:hover:not(.sel) { background: #3f3f3f; }
.hkwin .hk-cat { padding: 3px 6px; background: #3a3a3a; color: #eee; font-weight: 600; position: sticky; top: 0; cursor: default; }
.hkwin .hk-colhead { display: flex; padding: 2px 6px; color: #aaa; font-size: 11px; border-bottom: 1px solid #555; }
.hkwin .hk-colhead .c1 { flex: 1; } .hkwin .hk-colhead .c2 { width: 120px; text-align: right; }
.hkwin .hk-btns { display: flex; gap: 4px; flex: none; }
.hkwin .hk-btns button { flex: 1; }
.hkwin .hk-status { min-height: 20px; color: #e6c54a; flex: none; }
.hkwin .hk-status.capture { background: #5a4a1a; color: #fff; padding: 2px 6px; border-radius: 2px; }
` }));
}

// ---------------------------------------------------------------- window
export function openHotkeyEditor() {
  injectStyle();
  const win = new FloatWin('hotkeyEditor', 'Hotkey Editor', { w: 880, h: 600, minW: 600, minH: 380 });
  if (win.reused) return win;
  win.el.classList.add('hkwin');
  const HK = App.hotkeys;
  const cmds = () => (App.cmds?.list?.() || []).filter(c => c && c.id).sort((a, b) => categoryOf(a).localeCompare(categoryOf(b)) || String(a.label).localeCompare(String(b.label)));
  const labelOf = (id) => { const c = App.cmds?.registry?.get?.(id) || (App.cmds?.list?.() || []).find(x => x.id === id); return c ? c.label : id; };

  // reverse map cmdId -> combos; combo(normalized) -> {combo, cmd}
  let byCmd = new Map(), byCombo = new Map();
  const rebuildMaps = () => {
    byCmd = new Map(); byCombo = new Map();
    if (HK && HK.map) {
      for (const [combo, id] of HK.map) { byCombo.set(normalizeCombo(combo), { combo, cmd: id }); if (!byCmd.has(id)) byCmd.set(id, []); byCmd.get(id).push(combo); }
    } else {
      for (const c of App.cmds?.list?.() || []) if (c.hk) { byCombo.set(normalizeCombo(c.hk), { combo: c.hk, cmd: c.id }); byCmd.set(c.id, [c.hk]); }
    }
  };
  const combosOf = (id) => byCmd.get(id) || [];

  const layer = { Ctrl: false, Alt: false, Shift: false };
  let selected = null; let capturing = false; let filter = ''; let catFilter = '';
  const collapsed = new Set();

  // ---- left: keyboard
  const kb = h('div', { class: 'hk-kb' });
  const kbWrap = h('div', { class: 'hk-kbwrap' }, kb);
  const info = h('div', { class: 'hk-info', html: '<span class="dim">Hover over a key to see its command. Toggle Ctrl / Alt / Shift (or click the modifier keys) to view those layers.</span>' });
  const modBox = (m) => { const cb = h('input', { type: 'checkbox', onchange: () => { layer[m] = cb.checked; drawKeyboard(); } }); cb.dataset.mod = m; return h('label', {}, cb, m); };
  const mods = h('div', { class: 'hk-mods' }, h('span', { class: 'dim', text: 'Modifier layer:' }), modBox('Ctrl'), modBox('Alt'), modBox('Shift'));
  const legend = h('div', { class: 'hk-legend' },
    h('span', {}, h('i', { style: { background: '#4f7d99' } }), 'Assigned'),
    h('span', {}, h('i', { style: { background: '#c58b3a' } }), 'Selected command'),
    h('span', {}, h('i', { style: { background: '#5a5a5a' } }), 'Free'));
  const layerTtl = h('div', { class: 'hk-ttl' });
  const layerList = h('div', { class: 'list-box hk-layer' });
  const left = h('div', { class: 'hk-left' }, mods, kbWrap, legend, info, layerTtl, layerList);
  const renderLayerList = () => {
    const want = ['Ctrl', 'Alt', 'Shift'].filter(m => layer[m]).join('+');
    const rows = [...byCombo.values()].filter(b => { const n = normalizeCombo(b.combo).split('+'); n.pop(); return n.join('+') === want; })
      .sort((a, b) => normalizeCombo(a.combo).localeCompare(normalizeCombo(b.combo)));
    layerTtl.textContent = `Hotkeys in the ${want || 'unmodified'} layer (${rows.length})`;
    layerList.innerHTML = '';
    for (const b of rows) {
      const li = h('div', { class: 'li' + (b.cmd === selected ? ' sel' : '') }, h('span', { class: 'k', text: b.combo }), h('span', { text: labelOf(b.cmd) }));
      li.addEventListener('mousedown', () => { selected = b.cmd; renderList(true); drawKeyboard(); });
      layerList.append(li);
    }
  };

  const comboForKey = (base) => {
    const parts = []; if (layer.Ctrl) parts.push('Ctrl'); if (layer.Alt) parts.push('Alt'); if (layer.Shift) parts.push('Shift');
    parts.push(layerKey(base, layer.Shift));
    return parts.join('+');
  };
  let kbW = 0, kbH = 0;
  const drawKeyboard = () => {
    kb.innerHTML = '';
    const selCombos = new Set(selected ? combosOf(selected).map(normalizeCombo) : []);
    const addKey = (k, x, y) => {
      const el = h('div', { class: 'hk-key', style: { left: x * U + 'px', top: y * U + 'px', width: (k.w * U - 3) + 'px' } });
      if (k.mod) {
        el.classList.add('mod'); if (layer[k.mod]) el.classList.add('modon');
        el.textContent = k.label;
        el.addEventListener('click', () => { layer[k.mod] = !layer[k.mod]; mods.querySelector(`[data-mod=${k.mod}]`).checked = layer[k.mod]; drawKeyboard(); });
      } else if (!k.key) { el.classList.add('blank'); el.textContent = k.label; }
      else {
        const shown = layerKey(k.key, layer.Shift);
        el.textContent = k.label.length === 1 && k.key.length === 1 ? shown : k.label;
        const combo = comboForKey(k.key);
        const b = byCombo.get(normalizeCombo(combo));
        if (b) el.classList.add(selCombos.has(normalizeCombo(combo)) ? 'cmdkey' : 'bound');
        el.title = combo + (b ? ': ' + labelOf(b.cmd) : ' (unassigned)');
        el.addEventListener('mouseenter', () => {
          info.innerHTML = '';
          info.append(h('div', {}, h('b', { text: combo })), b ? h('div', {}, labelOf(b.cmd), h('span', { class: 'dim', text: '  (' + b.cmd + ')' })) : h('div', { class: 'dim', text: 'No command assigned' }));
        });
        el.addEventListener('click', () => { if (b) { selected = b.cmd; catFilter = ''; collapsed.delete(categoryOf({ id: b.cmd, ...(App.cmds?.registry?.get?.(b.cmd) || {}) })); renderList(true); drawKeyboard(); } });
      }
      kb.append(el);
    };
    let maxX = 0;
    ROWS.forEach((row, y) => {
      let x = 0;
      for (const k of row) { if (k.gap) { x += k.gap; continue; } addKey(k, x, y + (y > 0 ? 0.3 : 0)); x += k.w; }
      maxX = Math.max(maxX, x);
    });
    const nx = maxX + 0.4;
    NAV.forEach((row, y) => row.forEach((k, i) => { if (k) addKey(k, nx + i, y + (y > 0 ? 0.3 : 0)); }));
    kbW = (nx + 3) * U; kbH = (ROWS.length + 0.3) * U;
    kb.style.width = kbW + 'px'; kb.style.height = kbH + 'px';
    fit(); renderLayerList();
  };
  const fit = () => {
    const avail = kbWrap.clientWidth - 16;
    if (avail <= 0 || !kbW) return;
    const s = Math.min(1.3, avail / kbW);
    kb.style.transform = `scale(${s})`;
    kbWrap.style.height = (kbH * s + 16) + 'px';
  };

  // ---- right: search + list
  const search = h('input', { placeholder: 'Search commands or keys…' });
  const catSel = h('select', { title: 'Category' });
  const list = h('div', { class: 'list-box hk-list' });
  const status = h('div', { class: 'hk-status' });
  const btnAssign = h('button', { text: 'Assign', title: 'Assign a new hotkey to the selected command' });
  const btnRemove = h('button', { text: 'Remove', title: 'Remove a hotkey from the selected command' });
  const btnRestore = h('button', { text: 'Restore Defaults', title: 'Restore every hotkey to its default' });
  const right = h('div', { class: 'hk-right' },
    h('div', { class: 'hk-search' }, search, catSel),
    h('div', { class: 'hk-colhead' }, h('span', { class: 'c1', text: 'Command' }), h('span', { class: 'c2', text: 'Hotkey' })),
    list, status, h('div', { class: 'hk-btns' }, btnAssign, btnRemove, btnRestore));

  const renderCats = () => {
    const cats = [...new Set(cmds().map(categoryOf))].sort();
    catSel.innerHTML = '';
    catSel.append(h('option', { value: '', text: 'All categories' }), ...cats.map(c => h('option', { value: c, text: c })));
    catSel.value = catFilter;
  };
  const renderList = (scrollToSel = false) => {
    list.innerHTML = '';
    const q = filter.trim().toLowerCase();
    const groups = new Map();
    for (const c of cmds()) {
      const cat = categoryOf(c);
      if (catFilter && cat !== catFilter) continue;
      const keys = combosOf(c.id);
      if (q && !(String(c.label).toLowerCase().includes(q) || c.id.toLowerCase().includes(q) || keys.some(k => k.toLowerCase().includes(q)) || cat.toLowerCase().includes(q))) continue;
      if (!groups.has(cat)) groups.set(cat, []);
      groups.get(cat).push(c);
    }
    let selEl = null;
    for (const [cat, items] of groups) {
      const dupe = new Set(); const seen = new Set(); for (const c of items) { if (seen.has(c.label)) dupe.add(c.label); seen.add(c.label); }
      const closed = collapsed.has(cat) && !q;
      const head = h('div', { class: 'hk-cat', text: (closed ? '▸ ' : '▾ ') + cat + '  (' + items.length + ')' });
      head.addEventListener('click', () => { if (collapsed.has(cat)) collapsed.delete(cat); else collapsed.add(cat); renderList(); });
      list.append(head);
      if (closed) continue;
      for (const c of items) {
        const keys = combosOf(c.id);
        const row = h('div', { class: 'hk-row' + (c.id === selected ? ' sel' : ''), title: c.id + (c.help && c.help !== c.label ? ' — ' + c.help : '') },
          h('span', { class: 'c1' }, c.label, dupe.has(c.label) ? h('span', { class: 'id', text: c.id }) : null), h('span', { class: 'c2', text: keys.join(', ') }));
        row.addEventListener('mousedown', () => { selected = c.id; for (const r of list.querySelectorAll('.hk-row.sel')) r.classList.remove('sel'); row.classList.add('sel'); updateButtons(); drawKeyboard(); });
        row.addEventListener('dblclick', () => startCapture());
        if (c.id === selected) selEl = row;
        list.append(row);
      }
    }
    if (!groups.size) list.append(h('div', { class: 'li dim', text: 'No matching commands' }));
    if (scrollToSel && selEl) selEl.scrollIntoView({ block: 'center' });
    updateButtons();
  };
  const updateButtons = () => {
    const ok = !!HK && !!selected && !capturing;
    btnAssign.disabled = !ok; btnRemove.disabled = !ok || !combosOf(selected).length; btnRestore.disabled = !HK || capturing;
    if (!capturing) {
      status.classList.remove('capture');
      status.textContent = !HK ? 'Hotkey system not available (read-only view).' : selected ? `${labelOf(selected)}: ${combosOf(selected).join(', ') || 'no hotkey'}` : '';
    }
  };
  const changed = () => { try { HK.save?.(); } catch (e) { console.error(e); } rebuildMaps(); renderList(); drawKeyboard(); App.dirty('title'); };

  // ---- capture
  let captureHandler = null;
  const stopCapture = () => { if (captureHandler) { removeEventListener('keydown', captureHandler, true); captureHandler = null; } capturing = false; updateButtons(); };
  const startCapture = () => {
    if (!HK || !selected || capturing) return;
    capturing = true; updateButtons();
    status.classList.add('capture');
    status.textContent = `Press a key combination for "${labelOf(selected)}" (Esc to cancel)…`;
    const id = selected;
    captureHandler = async (e) => {
      e.preventDefault(); e.stopPropagation(); e.stopImmediatePropagation();
      if (e.key === 'Escape' && !e.ctrlKey && !e.altKey && !e.shiftKey) { stopCapture(); status.textContent = 'Assignment cancelled.'; return; }
      const combo = comboFromEvent(e);
      if (!combo) return; // waiting for a non-modifier key
      stopCapture();
      const ex = byCombo.get(normalizeCombo(combo));
      if (ex && ex.cmd === id) { status.textContent = `${combo} is already assigned to ${labelOf(id)}.`; return; }
      if (ex) {
        const r = await confirmDialog('Hotkey Conflict', `"${combo}" is currently assigned to "${labelOf(ex.cmd)}". Assign it to "${labelOf(id)}" instead?`, ['Override', 'Cancel']);
        if (r !== 0) { status.textContent = 'Assignment cancelled.'; return; }
        HK.unbind(ex.combo);
      }
      HK.bind(combo, id);
      changed();
      status.textContent = `Assigned ${combo} to ${labelOf(id)}.`;
      App.emit('echo', `hotkey -keyShortcut "${combo}" -name "${id}";`);
    };
    addEventListener('keydown', captureHandler, true);
  };
  btnAssign.addEventListener('click', startCapture);
  btnRemove.addEventListener('click', (e) => {
    if (!HK || !selected) return;
    const keys = combosOf(selected);
    const rm = (k) => { HK.unbind(k); changed(); status.textContent = `Removed ${k}.`; };
    if (keys.length === 1) rm(keys[0]);
    else if (keys.length > 1) {
      const r = btnRemove.getBoundingClientRect();
      showMenu([...keys.map(k => ({ label: k, fn: () => rm(k) })), '-', { label: 'Remove All', fn: () => { for (const k of keys) HK.unbind(k); changed(); } }], r.left, r.top - 4 - 22 * (keys.length + 2));
    }
  });
  btnRestore.addEventListener('click', async () => {
    if (!HK) return;
    if (await confirmDialog('Restore Default Hotkeys', 'Restore all hotkeys to their default settings? Your custom hotkeys will be lost.', ['Restore', 'Cancel']) !== 0) return;
    HK.reset?.(); changed(); toast('Default hotkeys restored');
  });
  search.addEventListener('input', () => { filter = search.value; renderList(); });
  catSel.addEventListener('change', () => { catFilter = catSel.value; renderList(); });

  // keys inside the window never reach global hotkeys
  win.el.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.target === search) { if (e.key === 'Escape') { search.value = ''; filter = ''; renderList(); } return; }
    if (e.key === 'Enter' && selected) startCapture();
    else if (e.key === 'Delete' && selected) btnRemove.click();
    else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
      const rows = [...list.querySelectorAll('.hk-row')]; const i = rows.findIndex(r => r.classList.contains('sel'));
      const n = rows[Math.max(0, Math.min(rows.length - 1, i + (e.key === 'ArrowDown' ? 1 : -1)))];
      if (n) { n.dispatchEvent(new MouseEvent('mousedown')); n.scrollIntoView({ block: 'nearest' }); }
      e.preventDefault();
    } else if (e.key === 'f' && (e.ctrlKey || e.metaKey)) { search.focus(); search.select(); e.preventDefault(); }
  });
  win.el.addEventListener('keyup', (e) => e.stopPropagation());
  win.onResize = fit;
  win.onClose = () => stopCapture();

  win.body.append(h('div', { class: 'hk-main' }, left, right));
  rebuildMaps(); renderCats(); renderList(); drawKeyboard();
  requestAnimationFrame(fit);
  setTimeout(() => search.focus(), 20);
  return win;
}

App.ui = App.ui || {};
App.ui.hotkeyEditor = openHotkeyEditor;
