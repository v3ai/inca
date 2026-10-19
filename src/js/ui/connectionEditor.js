// Inca — Connection Editor (Windows > General Editors > Connection Editor): two nodes side by side, outputs on one
// side and inputs on the other. Click an output, then a compatible input to connect them (click it again to break).
import { App } from '../core/app.js';
import * as S from '../core/scene.js';
import { Undo } from '../core/undo.js';
import * as C from '../core/connections.js';
import { h, menuBar, FloatWin, showMenu } from './dom.js';

function injectStyle() {
  if (document.getElementById('inca-ce-style')) return;
  document.head.append(h('style', { id: 'inca-ce-style', text: `
.cewin .ce-row { display: flex; gap: 4px; padding: 3px 6px; flex: none; align-items: center; }
.cewin .ce-row > button { flex: 1; }
.cewin .ce-head { display: flex; align-items: center; padding: 0 6px 3px; flex: none; gap: 4px; }
.cewin .ce-head .lbl { flex: 1; text-align: center; color: #ddd; font-weight: bold; font-size: 11.5px; }
.cewin .ce-head button { flex: none; min-width: 92px; }
.cewin .ce-lists { flex: 1; display: flex; gap: 4px; padding: 0 6px; min-height: 0; }
.cewin .ce-list { flex: 1; overflow: auto; background: #2b2b2b; border: 1px solid #1d1d1d; font-size: 11.5px; user-select: none; }
.cewin .ce-list.last { border-color: #5d7590; }
.cewin .ce-item { display: flex; align-items: center; height: 19px; padding-right: 6px; white-space: nowrap; color: #ddd; cursor: default; }
.cewin .ce-item .tw { width: 14px; flex: none; text-align: center; font-size: 9px; color: #aaa; }
.cewin .ce-item.node { color: #fff; font-weight: bold; background: #3a3a3a; }
.cewin .ce-item.child { padding-left: 14px; }
.cewin .ce-item:hover:not(.dis):not(.node) { background: #444; }
.cewin .ce-item.conn .t { font-weight: bold; font-style: italic; }
.cewin .ce-item.dis { color: #6b6b6b; }
.cewin .ce-item.hl { background: #4a5a6c; }
.cewin .ce-item.sel, .cewin .ce-item.sel:hover { background: var(--hi); color: #fff; }
.cewin .ce-foot { display: flex; align-items: center; gap: 4px; padding: 3px 6px; flex: none; }
.cewin .ce-foot .nm { flex: 1; text-align: center; color: #ddd; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cewin .ce-foot button { width: 24px; padding: 0; flex: none; }
.cewin .ce-empty { color: #888; padding: 8px; font-style: italic; }
` }));
}

// ------------------------------------------------------------------ the node network around a node (navigation buttons)
// downstream: what it feeds (connections, the next history node / its object, the object's material);
// upstream: what feeds it
function neighbours(rec, dir) {
  if (!rec) return [];
  const id = C.recId(rec); const out = [];
  for (const c of C.allConnections()) { if (dir === 'down' && c.src === id) out.push(C.recById(c.dst)); if (dir === 'up' && c.dst === id) out.push(C.recById(c.src)); }
  if (rec.kind === 'history') {
    const o = S.historyOwner(rec); const H = o?.inca.history || []; const i = H.indexOf(rec);
    if (dir === 'down') out.push(i >= 0 && i < H.length - 1 ? H[i + 1] : o); else if (i > 0) out.push(H[i - 1]);
  } else if (rec.isObject3D) {
    const H = rec.inca.history || [];
    if (dir === 'up' && H.length) out.push(H[H.length - 1]);
    if (dir === 'down') { const mid = rec.inca.material || (rec.inca.kind === 'mesh' ? App.defaultMatId : null); if (mid && App.nodes.get(mid)) out.push(App.nodes.get(mid)); }
  }
  return [...new Set(out.filter(Boolean))];
}

let ctl = null;
export function openConnectionEditor({ left = undefined, right = undefined } = {}) {
  injectStyle();
  const win = new FloatWin('connectionEditor', 'Connection Editor', { w: 560, h: 520, minW: 380, minH: 260 });
  if (win.reused && ctl) { ctl.load(left, right); return win; }
  win.el.classList.add('cewin');

  const filt = () => ({ mode: 'readable', nonKeyable: true, connectedOnly: false, hidden: false });
  // nodes are kept by id: undo / scene loads replace the node objects
  const st = { left: null, right: null, dir: true, auto: true, out: null, in: null, last: 'left', open: { left: new Set(), right: new Set() }, filt: { left: filt(), right: filt() } };
  const rec = (side) => C.recById(st[side]);
  const role = (side) => (side === 'left') === st.dir ? 'out' : 'in';
  const sideOf = (r) => (r === 'out') === st.dir ? 'left' : 'right';
  const outRec = () => rec(sideOf('out')), inRec = () => rec(sideOf('in'));

  // ---- layout
  const lists = { left: h('div', { class: 'ce-list' }), right: h('div', { class: 'ce-list' }) };
  const names = { left: h('span', { class: 'nm' }), right: h('span', { class: 'nm' }) };
  const headL = h('span', { class: 'lbl' }), headR = h('span', { class: 'lbl' });
  const dirBtn = h('button', { title: 'Swap which side shows outputs and which shows inputs', onclick: () => { st.dir = !st.dir; st.out = st.in = null; render(); } });
  const btn = (text, title, fn) => h('button', { text, title, onclick: fn });
  const breakBtn = btn('Break', 'Break the connection between the selected output and input', () => manual(false));
  const makeBtn = btn('Make', 'Connect the selected output to the selected input', () => manual(true));
  const navUp = h('button', { text: '<', title: 'Upstream: load the node feeding the left node (right-click: choose)' });
  const navDown = h('button', { text: '>', title: 'Downstream: load the node the right node feeds (right-click: choose)' });

  const display = (side) => {
    const f = st.filt[side]; const radio = (label, mode) => ({ label, check: () => f.mode === mode, fn: () => { f.mode = mode; render(); } });
    const tog = (label, k) => ({ label, check: () => f[k], fn: () => { f[k] = !f[k]; render(); } });
    return [radio('Show Readable', 'readable'), radio('Show Outputs Only', 'outputs'), radio('Show Inputs Only', 'inputs'), '-', tog('Show Non-Keyable', 'nonKeyable'), tog('Show Connected Only', 'connectedOnly'), tog('Show Hidden', 'hidden')];
  };
  const menuRow = h('div', { class: 'pmenubar' });
  menuBar(menuRow, [
    { label: 'Options', items: () => [{ label: 'Auto Connect', check: () => st.auto, fn: () => { st.auto = !st.auto; st.in = null; render(); } }] },
    { label: 'Left Display', items: () => display('left') },
    { label: 'Right Display', items: () => display('right') },
    { label: 'Help', items: [{ label: 'Help on Connection Editor...', fn: () => App.help('Click an output attribute, then a highlighted input attribute to connect them; click a connected (bold italic) input again to break it. Reload Left / Right load the selected node.') }] },
  ]);
  win.body.append(
    menuRow,
    h('div', { class: 'ce-row' }, btn('Reload Left', 'Load the selected node into the left side', () => reload('left')), btn('Reload Right', 'Load the selected node into the right side', () => reload('right'))),
    h('div', { class: 'ce-head' }, headL, dirBtn, headR),
    h('div', { class: 'ce-lists' }, lists.left, lists.right),
    h('div', { class: 'ce-foot' }, names.left, navUp, navDown, names.right),
    h('div', { class: 'ce-row' }, btn('Clear All', 'Remove the nodes from both sides', () => { st.left = st.right = null; st.out = st.in = null; render(); }),
      btn('Remove', 'Remove the node from the side last clicked', () => { st[st.last] = null; st.out = st.in = null; render(); }), breakBtn, makeBtn, btn('Close', '', () => win.close())),
  );

  // ---- loading
  const load = (l, r) => {
    if (l !== undefined) st.left = C.recId(l); if (r !== undefined) st.right = C.recId(r);
    st.out = st.in = null; render();
  };
  const reload = (side) => {
    const o = App.sel[App.sel.length - 1]; if (!o) { App.emit('warning', '// Warning: Nothing selected to load into the Connection Editor.'); return; }
    st[side] = C.recId(o); st.last = side; st.out = st.in = null; render();
  };
  const navigate = (dir, pick = null) => {
    const base = dir === 'down' ? (rec('right') || rec('left')) : (rec('left') || rec('right')); if (!base) return;
    const cands = neighbours(base, dir); const next = pick || cands[0];
    if (!next) { App.help(`${S.nodeName(base)} has no ${dir === 'down' ? 'downstream' : 'upstream'} nodes`); return; }
    if (dir === 'down') load(base, next); else load(next, base);
  };
  navUp.addEventListener('click', () => navigate('up'));
  navDown.addEventListener('click', () => navigate('down'));
  for (const [b, dir] of [[navUp, 'up'], [navDown, 'down']]) b.addEventListener('contextmenu', (e) => {
    e.preventDefault(); const base = dir === 'down' ? (rec('right') || rec('left')) : (rec('left') || rec('right')); if (!base) return;
    const cands = neighbours(base, dir);
    showMenu(cands.length ? cands.map(n => ({ label: S.nodeName(n), fn: () => navigate(dir, n) })) : [{ label: '(none)', enabled: () => false }], e.clientX, e.clientY);
  });

  // ---- make / break
  const tryRun = (label, fn) => { try { Undo.checkpoint(label); fn(); } catch (err) { App.emit('error', '// Error: ' + (err.message || err)); } render(); };
  const toggle = (inAttr) => {
    const o = outRec(), i = inRec(); if (!o || !i || !st.out) return;
    if (C.isConnected(o, st.out, i, inAttr)) tryRun('disconnectAttr', () => C.disconnectAttr(o, st.out, i, inAttr));
    else tryRun('connectAttr', () => C.connectAttr(o, st.out, i, inAttr, { force: true }));
  };
  function manual(make) {
    const o = outRec(), i = inRec(); if (!o || !i || !st.out || !st.in) return;
    if (make) { if (!C.isConnected(o, st.out, i, st.in)) tryRun('connectAttr', () => C.connectAttr(o, st.out, i, st.in, { force: true })); }
    else if (C.isConnected(o, st.out, i, st.in)) tryRun('disconnectAttr', () => C.disconnectAttr(o, st.out, i, st.in));
  }

  // ---- lists
  const shown = (side, n, a) => {
    const f = st.filt[side];
    if (f.mode === 'inputs' && !a.writable && !a.mappable) return false;
    if (f.mode === 'outputs' && !a.readable) return false;
    if (!f.nonKeyable && !a.keyable) return false;
    if (f.connectedOnly && !C.attrConnected(n, a.name, 'src') && !C.attrConnected(n, a.name, 'dst')) return false;
    return true;
  };
  function fill(side) {
    const el = lists[side]; el.innerHTML = ''; el.classList.toggle('last', st.last === side);
    const n = rec(side); names[side].textContent = n ? S.nodeName(n) : '';
    if (!n) { el.append(h('div', { class: 'ce-empty', text: `Select a node and click Reload ${side === 'left' ? 'Left' : 'Right'}` })); return; }
    const r = role(side); const other = r === 'out' ? inRec() : outRec();
    const outAttr = r === 'in' && st.out && outRec() ? C.attrInfo(outRec(), st.out) : null;
    const head = h('div', { class: 'ce-item node', title: 'MMB-drag onto the other side to load it there' }, h('span', { class: 'tw', text: '▾' }), h('span', { class: 't', text: S.nodeName(n) }));
    head.addEventListener('mousedown', (e) => { if (e.button !== 1) return; e.preventDefault(); const mu = (ev) => { removeEventListener('mouseup', mu); const tgt = document.elementFromPoint(ev.clientX, ev.clientY)?.closest('.ce-list'); const to = tgt === lists.left ? 'left' : tgt === lists.right ? 'right' : null; if (to && to !== side) { st[to] = st[side]; st.out = st.in = null; render(); } }; addEventListener('mouseup', mu); });
    el.append(head);
    const item = (a, child) => {
      const conn = C.attrConnected(n, a.name, r === 'out' ? 'src' : 'dst');
      const dis = r === 'in' && outAttr ? !C.compatible(outAttr, a) : r === 'in' ? !a.writable && !a.mappable : false;
      let hl = false;
      if (r === 'in' && st.out && other) hl = C.isConnected(other, st.out, n, a.name);
      if (r === 'out' && !st.out && st.in && other) hl = C.isConnected(n, a.name, other, st.in);
      const sel = r === 'out' ? st.out === a.name : st.in === a.name;
      const kids = a.children && !child; const open = kids && st.open[side].has(a.name);
      const row = h('div', { class: 'ce-item' + (child ? ' child' : '') + (conn ? ' conn' : '') + (dis ? ' dis' : '') + (hl ? ' hl' : '') + (sel ? ' sel' : ''), title: a.name },
        h('span', { class: 'tw', text: kids ? (open ? '▾' : '▸') : '' }), h('span', { class: 't', text: C.attrLabel(a) }));
      row.querySelector('.tw').addEventListener('mousedown', (e) => { if (!kids || e.button !== 0) return; e.stopPropagation(); open ? st.open[side].delete(a.name) : st.open[side].add(a.name); render(); });
      row.addEventListener('mousedown', (e) => {
        if (e.button !== 0) return; st.last = side;
        if (r === 'out') { st.out = st.out === a.name ? null : a.name; st.in = null; render(); return; }
        if (dis) return;
        if (st.out && st.auto) { toggle(a.name); return; }
        st.in = st.in === a.name ? null : a.name; render();
      });
      // RMB on an attribute: the nodes it is connected to; picking one loads it on the other side
      row.addEventListener('contextmenu', (e) => {
        e.preventDefault(); const id = C.recId(n); const fam = new Set([a.name, ...(a.children || []).map(c => c.name)]);
        const items = [];
        for (const c of C.allConnections()) {
          if (c.src === id && fam.has(c.srcAttr)) items.push({ label: `${S.nodeName(C.recById(c.dst))}.${c.dstAttr}`, fn: () => { st[side === 'left' ? 'right' : 'left'] = c.dst; st.out = st.in = null; render(); } });
          if (c.dst === id && fam.has(c.dstAttr)) items.push({ label: `${S.nodeName(C.recById(c.src))}.${c.srcAttr}`, fn: () => { st[side === 'left' ? 'right' : 'left'] = c.src; st.out = st.in = null; render(); } });
        }
        showMenu(items.length ? items : [{ label: '(no connections)', enabled: () => false }], e.clientX, e.clientY);
      });
      el.append(row);
      if (open) for (const c of a.children) if (shown(side, n, c)) item(c, true);
    };
    for (const a of C.listAttrs(n)) if (shown(side, n, a) || (a.children || []).some(c => shown(side, n, c))) item(a, false);
  }
  function render() {
    if (st.left && !rec('left')) st.left = null; if (st.right && !rec('right')) st.right = null;
    headL.textContent = st.dir ? 'Outputs' : 'Inputs'; headR.textContent = st.dir ? 'Inputs' : 'Outputs';
    dirBtn.textContent = st.dir ? 'from -> to' : 'to <- from';
    breakBtn.disabled = st.auto || !st.out || !st.in; makeBtn.disabled = st.auto || !st.out || !st.in;
    fill('left'); fill('right');
  }

  // ---- events
  const offs = [
    App.on('refresh', (d) => { if (d.has('connections') || d.has('outliner') || d.has('attr') || d.has('hypershade')) render(); }),
    App.on('sceneLoaded', () => render()),
  ];
  win.onClose = () => { for (const off of offs) off(); ctl = null; };
  ctl = { load };

  if (left === undefined && right === undefined) {
    const sel = App.sel; if (sel.length) load(sel[0], sel.length > 1 ? sel[sel.length - 1] : undefined); else render();
  } else load(left ?? null, right ?? null);
  return win;
}

App.ui = App.ui || {};
App.ui.connectionEditor = openConnectionEditor;
