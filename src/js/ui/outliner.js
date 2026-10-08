// Inca — Outliner
import { App } from '../core/app.js';
import * as S from '../core/scene.js';
import { Undo } from '../core/undo.js';
import { Sel } from '../core/selection.js';
import { h, menuBar, showMenu } from './dom.js';
import { icon } from './icons.js';
import { registerPanel } from './docks.js';

const KIND_ICON = { mesh: 'olMesh', group: 'olGroup', light: 'olLight', camera: 'olCamera', curve: 'olCurve', locator: 'olLocator', joint: 'joint', ikHandle: 'olLocator', deformHandle: 'olLocator', clusterHandle: 'olLocator', lattice: 'nurbsCube', constraint: 'parent' }; // rigging: rig kinds
function build(container) {
  const opts = App.prefs.opt.outliner || (App.prefs.opt.outliner = { showShapes: false, showSets: true, dagOnly: false, sortAlpha: false, showMaterials: false });
  const expanded = new Set();
  const menus = h('div', { class: 'pmenubar' });
  const search = h('input', { class: 'ol-search', placeholder: 'Search...' });
  const list = h('div', { class: 'pbody', tabindex: 0 });
  container.append(menus, search, list);
  search.addEventListener('keydown', e => e.stopPropagation());
  search.addEventListener('input', () => render());
  menuBar(menus, [
    { label: 'Edit', items: [{ label: 'Rename', cmd: 'renameSel' }, { label: 'Select Hierarchy', cmd: 'selectHierarchy' }, '-', { label: 'Group', cmd: 'group' }, { label: 'Ungroup', cmd: 'ungroup' }, { label: 'Delete', cmd: 'deleteSel' }, '-', { label: 'Expand All', fn: () => { S.allDag().forEach(o => expanded.add(o.inca.id)); render(); } }, { label: 'Collapse All', fn: () => { expanded.clear(); render(); } }] },
    { label: 'Display', items: () => [
      { label: 'DAG Objects Only', check: () => opts.dagOnly, fn: () => { opts.dagOnly = !opts.dagOnly; save(); } },
      { label: 'Shapes', check: () => opts.showShapes, fn: () => { opts.showShapes = !opts.showShapes; save(); } },
      { label: 'Sets', check: () => opts.showSets, fn: () => { opts.showSets = !opts.showSets; save(); } },
      { label: 'Materials', check: () => opts.showMaterials, fn: () => { opts.showMaterials = !opts.showMaterials; save(); } },
      '-', { label: 'Sort Alphabetical', check: () => opts.sortAlpha, fn: () => { opts.sortAlpha = !opts.sortAlpha; save(); } },
    ] },
    { label: 'Show', items: [{ label: 'Show Selected', fn: () => { for (const o of App.sel) { let p = S.dagParent(o); while (p) { expanded.add(p.inca.id); p = S.dagParent(p); } } render(); scrollToSel(); } }, { label: 'Frame Selection', fn: () => { App.cmds.run('frameSelected'); } }] },
    { label: 'Help', items: [{ label: 'Help on Outliner', fn: () => App.help('Click to select, Ctrl to toggle, Shift for range. Middle- or left-drag onto another item to parent; drop on empty space to unparent. Double-click to rename.') }] },
  ]);
  function save() { App.savePrefs(); render(); }
  let rows = []; // flattened visible items in order
  const matchFilter = (o) => { const q = search.value.trim().toLowerCase(); if (!q) return true; let m = false; o.traverse(c => { if (c.inca && c.inca.name.toLowerCase().includes(q.replace(/\*/g, ''))) m = true; }); return m; };
  function render() {
    list.innerHTML = ''; rows = [];
    const roots = S.dagChildren(App.world);
    const startup = roots.filter(o => o.inca.startup).sort((a, b) => ['persp', 'top', 'front', 'side'].indexOf(a.inca.name) - ['persp', 'top', 'front', 'side'].indexOf(b.inca.name));
    let others = roots.filter(o => !o.inca.startup);
    if (opts.sortAlpha) others = others.slice().sort((a, b) => a.inca.name.localeCompare(b.inca.name));
    for (const o of [...startup, ...others]) addRow(o, 0);
    if (!opts.dagOnly && opts.showSets) {
      for (const n of ['defaultLightSet', 'defaultObjectSet']) list.append(h('div', { class: 'ol-row set', style: { paddingLeft: '4px' } }, h('span', { class: 'ol-exp' }), h('span', { class: 'ol-ico', html: icon('olSet') }), h('span', { class: 'ol-name', text: n })));
    }
    if (opts.showMaterials) for (const m of App.mats.values()) { const r = h('div', { class: 'ol-row', style: { paddingLeft: '4px' } }, h('span', { class: 'ol-exp' }), h('span', { class: 'ol-ico', html: icon('olMaterial') }), h('span', { class: 'ol-name', text: m.name })); r.addEventListener('click', () => App.ui.showAttr(m)); list.append(r); }
  }
  function addRow(o, depth) {
    if (!matchFilter(o)) return;
    const inc = o.inca; const kids = S.dagChildren(o);
    const hasKids = kids.length > 0 || (opts.showShapes && inc.shapeName);
    const open = expanded.has(inc.id) || (search.value.trim() && hasKids);
    const sel = App.sel.includes(o) || App.hilite.includes(o);
    let childSel = false; if (!sel) o.traverse(c => { if (c !== o && App.sel.includes(c)) childSel = true; });
    let hiddenUp = false; let p = o; while (p && p.inca) { if (!p.inca.visible || (p.inca.layer && App.layers.find(l => l.id === p.inca.layer && !l.visible))) hiddenUp = true; p = p.parent; }
    const exp = h('span', { class: 'ol-exp', text: hasKids ? (open ? '▾' : '▸') : '' });
    exp.addEventListener('mousedown', (e) => { e.stopPropagation(); if (!hasKids) return; if (e.shiftKey) { const set = !open; o.traverse(c => { if (c.inca) { if (set) expanded.add(c.inca.id); else expanded.delete(c.inca.id); } }); } else if (open) expanded.delete(inc.id); else expanded.add(inc.id); render(); });
    const nm = h('span', { class: 'ol-name', text: inc.name });
    const r = h('div', { class: 'ol-row' + (sel ? ' sel' : '') + (childSel ? ' child-sel' : '') + (inc.startup ? ' startup' : '') + (hiddenUp ? ' ol-hidden' : ''), style: { paddingLeft: (4 + depth * 16) + 'px' }, title: inc.template ? inc.name + ' (template)' : inc.name },
      exp, h('span', { class: 'ol-ico', html: icon(inc.kind === 'light' ? (inc.light.type === 'ambientLight' ? 'ambientLight' : inc.light.type) : KIND_ICON[inc.kind] || 'olGroup') }), nm);
    r._o = o; rows.push(o);
    r.addEventListener('mousedown', (e) => onRowDown(e, o, r));
    r.addEventListener('dblclick', () => startRename(o, nm));
    r.addEventListener('contextmenu', (e) => { e.preventDefault(); if (!App.sel.includes(o)) Sel.select([o]); ctxMenu(e); });
    list.append(r);
    if (open) {
      if (opts.showShapes && inc.shapeName) list.append(h('div', { class: 'ol-row', style: { paddingLeft: (4 + (depth + 1) * 16) + 'px' } }, h('span', { class: 'ol-exp' }), h('span', { class: 'ol-ico', html: icon(KIND_ICON[inc.kind] || 'olGroup') }), h('span', { class: 'ol-name', text: inc.shapeName, style: { color: '#aaa' } })));
      let ks = kids; if (opts.sortAlpha) ks = ks.slice().sort((a, b) => a.inca.name.localeCompare(b.inca.name));
      for (const c of ks) addRow(c, depth + 1);
    }
  }
  let anchor = null;
  function onRowDown(e, o, r) {
    list.focus({ preventScroll: true });
    if (e.button === 2) return;
    const x0 = e.clientX, y0 = e.clientY; let dragging = false; let ghost = null; let dropRow = null; let dropMode = null;
    const isMid = e.button === 1;
    const doSelect = () => {
      if (o.inca.startup && o.inca.kind === 'camera') { Sel.select([o], e.ctrlKey ? 'toggle' : 'replace'); return; }
      if (e.shiftKey && anchor && rows.includes(anchor)) { const a = rows.indexOf(anchor), b = rows.indexOf(o); Sel.select(rows.slice(Math.min(a, b), Math.max(a, b) + 1), e.ctrlKey ? 'add' : 'replace'); return; }
      if (e.ctrlKey) Sel.select([o], 'toggle'); else { Sel.select([o], 'replace'); anchor = o; }
      if (!e.shiftKey && !e.ctrlKey) anchor = o;
    };
    if (!isMid && !App.sel.includes(o)) doSelect();
    const mm = (ev) => {
      if (!dragging && Math.hypot(ev.clientX - x0, ev.clientY - y0) > 5) {
        dragging = true;
        const names = (App.sel.includes(o) ? App.sel : [o]).map(n => n.inca.name).join(', ');
        ghost = h('div', { class: 'toast', style: { position: 'fixed', transform: 'none', top: 0, left: 0, padding: '2px 8px', opacity: '.9' }, text: names }); document.body.append(ghost);
      }
      if (!dragging) return;
      ghost.style.left = ev.clientX + 12 + 'px'; ghost.style.top = ev.clientY + 6 + 'px';
      for (const x of list.querySelectorAll('.drop, .drop-line')) x.classList.remove('drop', 'drop-line');
      const el = document.elementFromPoint(ev.clientX, ev.clientY)?.closest('.ol-row');
      dropRow = el && el._o ? el : null; dropMode = null;
      if (dropRow) { const rr = dropRow.getBoundingClientRect(); dropMode = (ev.clientY - rr.top) > rr.height * 0.75 ? 'after' : 'into'; dropRow.classList.add(dropMode === 'into' ? 'drop' : 'drop-line'); }
    };
    const mu = () => {
      removeEventListener('mousemove', mm); removeEventListener('mouseup', mu);
      ghost?.remove(); for (const x of list.querySelectorAll('.drop, .drop-line')) x.classList.remove('drop', 'drop-line');
      if (!dragging) { if (!isMid && App.sel.includes(o) && !(App.sel.length === 1 && App.sel[0] === o)) doSelect(); else if (!isMid && (e.ctrlKey || e.shiftKey)) { } return; }
      const moving = (App.sel.includes(o) ? App.sel : [o]).filter(n => !n.inca.startup);
      if (!moving.length) return;
      const overList = list.matches(':hover');
      Undo.checkpoint('parent');
      if (dropRow && dropMode === 'into') { const p = dropRow._o; if (p.inca.startup) return; for (const m of moving) S.parentTo(m, p, true); App.emit('echo', `parent ${moving.map(m => m.inca.name).join(' ')} ${p.inca.name};`); expanded.add(p.inca.id); }
      else if (dropRow && dropMode === 'after') {
        const target = dropRow._o; const par = S.dagParent(target);
        for (const m of moving) { if (m === target) continue; if (S.dagParent(m) !== par) S.parentTo(m, par, true); const holder = par || App.world; holder.remove(m); const idx = holder.children.indexOf(target); holder.children.splice(idx + 1, 0, m); m.parent = holder; }
        App.emit('echo', `reorder -relative ...;`);
      } else if (overList) { for (const m of moving) S.parentTo(m, null, true); App.emit('echo', `parent -w ${moving.map(m => m.inca.name).join(' ')};`); }
      App.dirty('outliner', 'channels'); App.requestRender();
    };
    addEventListener('mousemove', mm); addEventListener('mouseup', mu);
  }
  function startRename(o, nm) {
    if (o.inca.startup) return;
    const inp = h('input', { class: 'ol-rename', value: o.inca.name }); nm.replaceWith(inp); inp.focus(); inp.select();
    let done = false;
    const fin = (ok) => { if (done) return; done = true; if (ok && inp.value.trim() && inp.value.trim() !== o.inca.name) { Undo.checkpoint('rename'); const old = o.inca.name; S.rename(o, inp.value.trim()); App.emit('echo', `rename "${old}" "${o.inca.name}";`); } render(); };
    inp.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') fin(true); if (e.key === 'Escape') fin(false); });
    inp.addEventListener('blur', () => fin(true));
  }
  function ctxMenu(e) {
    showMenu([
      { label: 'Rename', fn: () => { const r = [...list.children].find(x => x._o === Sel.lead()); if (r) startRename(Sel.lead(), r.querySelector('.ol-name')); } },
      { label: 'Select Hierarchy', cmd: 'selectHierarchy' }, '-',
      { label: 'Group', cmd: 'group' }, { label: 'Ungroup', cmd: 'ungroup' }, { label: 'Duplicate', cmd: 'duplicate' }, { label: 'Unparent', cmd: 'unparent' }, { label: 'Delete', cmd: 'deleteSel' }, '-',
      { label: 'Hide', cmd: 'hideSelection' }, { label: 'Show', cmd: 'showSelection' }, { label: 'Isolate Select', cmd: 'isolateSelect' }, '-',
      { label: 'Frame Selection', cmd: 'frameSelected' }, { label: 'Attribute Editor', cmd: 'attributeEditor' }, '-',
      { label: 'Expand All', fn: () => { App.sel.forEach(o => o.traverse(c => c.inca && expanded.add(c.inca.id))); render(); } }, { label: 'Collapse All', fn: () => { expanded.clear(); render(); } },
    ], e.clientX, e.clientY);
  }
  function scrollToSel() { const r = list.querySelector('.ol-row.sel'); r?.scrollIntoView({ block: 'nearest' }); }
  list.addEventListener('mousedown', (e) => { if (e.target === list && e.button === 0) Sel.select([], 'replace'); });
  list.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); e.stopPropagation(); const lead = Sel.lead(); let i = rows.indexOf(lead); i = Math.max(0, Math.min(rows.length - 1, i + (e.key === 'ArrowDown' ? 1 : -1))); if (rows[i]) Sel.select([rows[i]], e.shiftKey ? 'add' : 'replace'); scrollToSel(); }
    else if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') { e.preventDefault(); e.stopPropagation(); for (const o of App.sel) { if (e.key === 'ArrowRight') expanded.add(o.inca.id); else expanded.delete(o.inca.id); } render(); }
  });
  App.on('selectionChanged', () => { if (list.isConnected) { render(); scrollToSel(); } });
  return { refresh(d) { if (d.has('all') || d.has('outliner') || d.has('layers')) render(); }, show: render };
}
registerPanel('outliner', 'Outliner', build, { side: 'left' });
