// Inca — marking menus (RMB), hotbox (hold Space), In-View Editor and option boxes
import { App } from '../core/app.js';
import * as S from '../core/scene.js';
import { Sel } from '../core/selection.js';
import { Undo } from '../core/undo.js';
import { OPS, niceName } from '../core/ops.js';
import { MAT_TYPES } from '../core/materials.js';
import { placeProxy } from '../core/manip.js';
import { h, showMenu, closeMenus, menuBar, optionsDialog, fmt } from './dom.js';

// ------------------------------------------------------------------ generic marking menu
// radial: {N,NE,E,SE,S,SW,W,NW: item}, list: items (showMenu format). item: {label, fn|cmd, enabled?, sub?}
const DIRS = { N: -90, NE: -45, E: 0, SE: 45, S: 90, SW: 135, W: 180, NW: -135 };
export function markingMenu(x, y, radial, list, { title = null } = {}) {
  closeMenus();
  const root = h('div', { class: 'mm' }); document.body.append(root);
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg'); svg.setAttribute('class', 'lines'); root.append(svg);
  const line = document.createElementNS('http://www.w3.org/2000/svg', 'line'); line.setAttribute('stroke', '#ddd'); line.setAttribute('stroke-width', '1.5'); svg.append(line);
  root.append(h('div', { class: 'mm-dot', style: { left: x + 'px', top: y + 'px' } }));
  const R = 82; const items = [];
  for (const [d, it] of Object.entries(radial || {})) {
    if (!it) continue;
    const a = DIRS[d] * Math.PI / 180; const ix = x + Math.cos(a) * R * (d === 'E' || d === 'W' ? 1.25 : 1), iy = y + Math.sin(a) * R * (d === 'N' || d === 'S' ? 0.6 : 0.75);
    const en = it.enabled ? it.enabled() : true;
    const el = h('div', { class: 'mm-item' + (en ? '' : ' disabled'), text: it.label + (it.sub ? ' ▸' : ''), style: { left: ix + 'px', top: iy + 'px' } });
    el._it = it; el._dir = d; el._en = en; root.append(el); items.push(el);
  }
  let listEl = null; const rows = [];
  if (list && list.length) {
    listEl = h('div', { class: 'mm-list', style: { left: (x - 85) + 'px', top: (y + R * 0.6 + 22) + 'px' } });
    if (title) listEl.append(h('div', { class: 'mi disabled', text: title, style: { fontStyle: 'italic' } }));
    for (const it of list) {
      if (it === '-') { listEl.append(h('div', { class: 'sep' })); continue; }
      const en = it.enabled ? it.enabled() : true;
      const label = it.label ?? App.cmds.info(it.cmd)?.label ?? it.cmd;
      const r = h('div', { class: 'mi' + (en ? '' : ' disabled'), text: label + (it.sub ? '  ▸' : '') }); r._it = it; r._en = en; listEl.append(r); rows.push(r);
    }
    root.append(listEl);
    const lr = listEl.getBoundingClientRect(); if (lr.bottom > innerHeight) listEl.style.top = Math.max(0, innerHeight - lr.height - 4) + 'px'; if (lr.right > innerWidth) listEl.style.left = (innerWidth - lr.width - 4) + 'px'; if (lr.left < 0) listEl.style.left = '4px';
  }
  let hot = null; let subOpen = null;
  const setHot = (el) => {
    if (hot === el) return; hot?.classList.remove('hot'); hot = el; el?.classList.add('hot');
    if (subOpen) { closeMenus(); subOpen = null; }
    if (el && el._it.sub && el._en) { const r = el.getBoundingClientRect(); subOpen = showMenu(el._it.sub, r.right, r.top, { sub: true }); }
  };
  const move = (ev) => {
    const dx = ev.clientX - x, dy = ev.clientY - y; const dist = Math.hypot(dx, dy);
    line.setAttribute('x1', x); line.setAttribute('y1', y); line.setAttribute('x2', ev.clientX); line.setAttribute('y2', ev.clientY);
    const over = document.elementFromPoint(ev.clientX, ev.clientY);
    if (over && over.closest('.popup')) return; // in a submenu
    const row = over && over.closest('.mm-list .mi'); if (row && row._it) { setHot(row); return; }
    const di = over && over.closest('.mm-item'); if (di) { setHot(di); return; }
    if (dist > 28 && items.length && !(listEl && ev.clientY > listEl.getBoundingClientRect().top - 6 && Math.abs(dx) < 120)) {
      let ang = Math.atan2(dy, dx) * 180 / Math.PI; let best = null, bd = 1e9;
      for (const el of items) { let d = Math.abs(((ang - DIRS[el._dir]) + 540) % 360 - 180); if (d < bd) { bd = d; best = el; } }
      setHot(bd < 28 ? best : null);
    } else if (dist <= 28) setHot(null);
  };
  const run = (el) => { const it = el._it; if (!el._en || it.sub) return false; cleanup(); setTimeout(() => { try { if (it.fn) it.fn(); else if (it.cmd) App.cmds.run(it.cmd, it.args); } catch (e) { App.emit('error', String(e.message || e)); } }, 0); return true; };
  const t0 = performance.now(); let sticky = false;
  const up = (ev) => {
    if (ev.target.closest && ev.target.closest('.popup')) return; // let submenu handle
    const quick = performance.now() - t0 < 260 && Math.hypot(ev.clientX - x, ev.clientY - y) < 6;
    if (quick && !sticky) { sticky = true; return; }
    if (hot && run(hot)) return;
    if (sticky && ev.type === 'mouseup' && !hot) { cleanup(); return; }
    cleanup();
  };
  const key = (e) => { if (e.key === 'Escape') cleanup(); };
  function cleanup() { removeEventListener('mousemove', move, true); removeEventListener('mouseup', up, true); removeEventListener('keydown', key, true); root.remove(); if (subOpen) closeMenus(); }
  addEventListener('mousemove', move, true); addEventListener('mouseup', up, true); addEventListener('keydown', key, true);
  root.addEventListener('contextmenu', e => e.preventDefault());
  return { close: cleanup };
}

// ------------------------------------------------------------------ viewport RMB menus
const matItems = () => [...App.mats.values()].map(mm => ({ label: mm.name, fn: () => App.cmds.run('assignExisting', { id: mm.id }) }));
const newMatItems = () => Object.entries(MAT_TYPES).map(([k, d]) => ({ label: d.label, fn: () => App.cmds.run('assignNewMaterial', { type: k }) }));
App.on('markingMenu', ({ e, vp, px }) => {
  const x = e.clientX, y = e.clientY;
  const hits = App.compMode ? [] : vp.pickObjects(px);
  const under = hits[0]?.o || (App.compMode ? null : null);
  const meshUnder = vp.pickObjects(px)[0]?.o;
  // Shift+RMB: tool marking menus
  if (e.shiftKey && !e.ctrlKey) {
    const cm = App.compMode;
    if (cm === 'face') return markingMenu(x, y, { N: { label: 'Extrude Face', cmd: 'extrude' }, NE: { label: 'Duplicate Face', cmd: 'duplicateFaces' }, E: { label: 'Extract Face', cmd: 'extract' }, S: { label: 'Poke Face', cmd: 'poke' }, W: { label: 'Bridge', cmd: 'bridge' }, NW: { label: 'Bevel Face', cmd: 'bevel' }, SW: { label: 'Add Divisions', cmd: 'addDivisions' }, SE: { label: 'Triangulate', cmd: 'triangulate' } }, [{ cmd: 'multiCutTool' }, { cmd: 'insertEdgeLoopTool' }, '-', { cmd: 'toVertices' }, { cmd: 'toEdges' }, '-', { cmd: 'assignNewMaterial' }]);
    if (cm === 'edge') return markingMenu(x, y, { N: { label: 'Extrude Edge', cmd: 'extrude' }, NE: { label: 'Collapse Edge', cmd: 'collapse' }, E: { label: 'Bridge Edge', cmd: 'bridge' }, S: { label: 'Delete Edge', cmd: 'deleteEdgeVertex' }, W: { label: 'Bevel Edge', cmd: 'bevel' }, NW: { label: 'Merge Edges', cmd: 'mergeToCenter' }, SW: { label: 'Soften/Harden', sub: [{ cmd: 'softenEdge' }, { cmd: 'hardenEdge' }] }, SE: { label: 'Fill Hole', cmd: 'fillHole' } }, [{ cmd: 'insertEdgeLoopTool' }, { cmd: 'multiCutTool' }, { cmd: 'connect' }, '-', { cmd: 'selectEdgeLoop' }, { cmd: 'selectEdgeRing' }, { cmd: 'selectBorder' }]);
    if (cm === 'vertex') return markingMenu(x, y, { N: { label: 'Extrude Vertex', cmd: 'chamferVertex' }, NE: { label: 'Merge Vertices', cmd: 'merge' }, E: { label: 'Connect', cmd: 'connect' }, S: { label: 'Delete Vertex', cmd: 'deleteEdgeVertex' }, W: { label: 'Chamfer Vertex', cmd: 'chamferVertex' }, NW: { label: 'Merge to Center', cmd: 'mergeToCenter' }, SW: { label: 'Target Weld Tool', cmd: 'targetWeldTool' } }, [{ cmd: 'multiCutTool' }, { cmd: 'insertEdgeLoopTool' }, '-', { cmd: 'toEdges' }, { cmd: 'toFaces' }]);
    return markingMenu(x, y, { N: { label: 'Extrude', cmd: 'extrude' }, NE: { label: 'Combine', cmd: 'combine' }, E: { label: 'Separate', cmd: 'separate' }, S: { label: 'Smooth', cmd: 'smooth' }, W: { label: 'Mirror', cmd: 'mirror' }, NW: { label: 'Bevel', cmd: 'bevel' }, SE: { label: 'Delete History', cmd: 'deleteHistory' }, SW: { label: 'Bridge', cmd: 'bridge' } }, [{ cmd: 'multiCutTool' }, { cmd: 'insertEdgeLoopTool' }, { cmd: 'targetWeldTool' }, '-', { cmd: 'centerPivot' }, { cmd: 'freezeTransformations' }, { cmd: 'resetTransformations' }]);
  }
  if (e.ctrlKey) return markingMenu(x, y, { N: { label: 'To Edges', cmd: 'toEdges' }, W: { label: 'To Vertices', cmd: 'toVertices' }, E: { label: 'To Faces', cmd: 'toFaces' }, S: { label: 'Edge Loop', cmd: 'selectEdgeLoop' }, SE: { label: 'Edge Ring', cmd: 'selectEdgeRing' }, SW: { label: 'Border', cmd: 'selectBorder' }, NE: { label: 'Grow', cmd: 'growSelection' }, NW: { label: 'Shrink', cmd: 'shrinkSelection' } }, []);
  const target = meshUnder || (App.compMode ? App.hilite[0] : null);
  if (target && (target.inca.kind === 'mesh' || target.inca.kind === 'curve')) {
    const ensure = (fn) => () => { if (!App.sel.includes(target) && !App.hilite.includes(target)) { if (App.compMode) Sel.setMode(null); Sel.select([target]); } fn(); };
    const mode = (m) => ensure(() => { Sel.setMode(m); App.lastCompMode = m; placeProxy(); });
    const isCurve = target.inca.kind === 'curve';
    const radial = isCurve ? { N: { label: 'Control Vertex', fn: mode('vertex') }, E: { label: 'Object Mode', fn: ensure(() => { Sel.setMode(null); placeProxy(); }) }, W: { label: 'Edit Point', fn: mode('vertex') }, S: { label: 'Curve Point', fn: mode('vertex'), enabled: () => false }, NE: { label: 'Select', fn: ensure(() => Sel.select([target])) } }
      : { N: { label: 'Edge', fn: mode('edge') }, W: { label: 'Vertex', fn: mode('vertex') }, S: { label: 'Face', fn: mode('face') }, E: { label: 'Object Mode', fn: ensure(() => { Sel.setMode(null); placeProxy(); }) }, SE: { label: 'UV', fn: mode('uv') }, NE: { label: 'Select', fn: () => { if (App.compMode) Sel.setMode(null); Sel.select([target]); } }, NW: { label: 'Multi', fn: mode('vertex') }, SW: { label: 'Vertex Face', fn: mode('vertex') } };
    const list = [
      { label: 'Select All', cmd: 'selectAll' }, { label: 'Deselect', cmd: 'deselectAll' }, { label: 'Select Hierarchy', cmd: 'selectHierarchy' }, { label: 'Inverse Selection', cmd: 'invertSelection' }, '-',
      { label: 'Assign New Material...', sub: newMatItems }, { label: 'Assign Existing Material', sub: matItems }, { label: 'Material Attributes...', fn: ensure(() => App.cmds.run('materialAttributes')) }, '-',
      { label: 'Actions', sub: [{ cmd: 'hideSelection' }, { cmd: 'showLastHidden' }, { cmd: 'templateSel' }, { cmd: 'centerPivot' }, { cmd: 'freezeTransformations' }, { cmd: 'deleteHistory' }] },
      { label: 'Inputs', sub: () => (target.inca.history || []).filter(hh => OPS[hh.type] && !OPS[hh.type].hidden).reverse().map(hh => ({ label: hh.name, fn: () => App.ui.showAttr(hh) })) },
      { label: 'Outputs', sub: [{ label: target.inca.shapeName || '(none)', enabled: () => false }] }, '-',
      { label: 'Paint', sub: [{ label: 'Paint Select Tool', fn: () => App.setTool('paint') }, { label: 'Sculpt Tool', cmd: 'sculptTool' }] },
      { label: 'Metadata', enabled: () => false },
    ];
    return markingMenu(x, y, radial, list, { title: target.inca.name + '...' });
  }
  // empty space / other objects
  const o = hits[0]?.o;
  if (o) return markingMenu(x, y, { N: { label: 'Select', fn: () => Sel.select([o]) }, E: { label: 'Attribute Editor', fn: () => App.ui.showAttr(o) }, W: { label: o.inca.kind === 'camera' ? 'Look Through' : 'Frame', fn: () => { if (o.inca.kind === 'camera') vp.setCamera(o.inca.name); else { Sel.select([o]); vp.frameSelection(); } } } }, [{ label: 'Select All', cmd: 'selectAll' }, { label: 'Select Hierarchy', cmd: 'selectHierarchy' }, '-', { cmd: 'hideSelection' }, { cmd: 'deleteSel', label: 'Delete' }], { title: o.inca.name + '...' });
  markingMenu(x, y, { N: { label: 'Select All', cmd: 'selectAll' }, S: { label: 'Complete Tool', fn: () => App.tools?.complete?.() }, E: { label: 'Object Mode', fn: () => Sel.setMode(null) }, W: { label: 'Deselect All', cmd: 'deselectAll' }, NE: { label: 'Frame All', fn: () => vp.frameAll() }, NW: { label: 'Repeat Last', cmd: 'repeatLast' } },
    [{ label: 'Create', sub: () => App.ui.commonMenus.Create() }, '-', { label: 'Select Similar', cmd: 'selectSimilar' }, { label: 'Paste', cmd: 'paste' }, '-', { label: 'Look Through Default', fn: () => vp.setCamera(vp._slot || 'persp') }]);
});

// ------------------------------------------------------------------ hotbox
let hb = null;
App.ui.hotbox = (show) => {
  if (!show) { if (hb) { hb.remove(); hb = null; } return; }
  if (hb) return;
  const vp = App.hoverViewport || App.activeViewport;
  const pos = App._mouse || { x: innerWidth / 2, y: innerHeight / 2 };
  hb = h('div', { class: 'hotbox' }); document.body.append(hb);
  const rowAt = (y, menus) => { const r = h('div', { class: 'hb-row', style: { left: pos.x + 'px', top: y + 'px' } }); for (const mm of menus) { const it = h('div', { class: 'hb-item', text: mm.label }); it.addEventListener('mouseenter', () => { for (const x of r.parentNode.querySelectorAll('.hb-item.open')) x.classList.remove('open'); it.classList.add('open'); const b = it.getBoundingClientRect(); closeMenus(); showMenu(mm.items, b.left, b.bottom); }); r.append(it); } hb.append(r); return r; };
  const common = Object.entries(App.ui.commonMenus).map(([label, items]) => ({ label, items }));
  const set = Object.entries(App.ui.menuSets[App.prefs.menuSet] || {}).map(([label, items]) => ({ label, items }));
  const others = Object.entries(App.ui.menuSets).filter(([k]) => k !== App.prefs.menuSet).flatMap(([k, ms]) => Object.entries(ms).slice(0, 3).map(([label, items]) => ({ label, items })));
  rowAt(pos.y - 120, [{ label: 'Recent Commands', items: () => App.undo.labels().slice(-12).reverse().map(l => ({ label: l, enabled: () => false })) }, { label: 'Open Script Editor', items: [{ cmd: 'scriptEditor' }] }]);
  rowAt(pos.y - 86, common);
  rowAt(pos.y - 58, set);
  rowAt(pos.y - 30, others.slice(0, 8));
  hb.append(h('div', { class: 'hb-center', text: 'Inca', style: { left: pos.x + 'px', top: pos.y + 'px' } }));
  if (vp) {
    const panelMenus = [...vp.menubar.querySelectorAll('.menu-title')].map(t => ({ label: t.textContent, el: t }));
    const r = h('div', { class: 'hb-row', style: { left: pos.x + 'px', top: (pos.y + 28) + 'px' } });
    for (const pm of panelMenus) { const it = h('div', { class: 'hb-item', text: pm.label }); it.addEventListener('mouseenter', () => { closeMenus(); pm.el.dispatchEvent(new MouseEvent('mousedown', { bubbles: true })); }); r.append(it); }
    hb.append(r);
    const lay = h('div', { class: 'hb-row', style: { left: pos.x + 'px', top: (pos.y + 56) + 'px' } }, ...[['Single', 'single'], ['Four', 'four'], ['Two Side', 'twoSide'], ['Two Stack', 'twoStack'], ['Three', 'three']].map(([l, k]) => { const it = h('div', { class: 'hb-item', text: l + ' Pane' }); it.addEventListener('mouseup', () => App.setLayout(k)); return it; }));
    hb.append(lay);
  }
};
addEventListener('mousemove', (e) => { App._mouse = { x: e.clientX, y: e.clientY }; }, true);

// ------------------------------------------------------------------ In-View Editor
let ivf = null; let ivfRec = null;
function closeIVF() { if (ivf) ivf.remove(); ivf = null; ivfRec = null; }
App.on('inViewEditor', (rec) => {
  closeIVF();
  if (!rec || App.prefs.opt.inViewEditor === false) return;
  const op = OPS[rec.type]; if (!op || !op.channels?.length) return;
  const vp = App.activeViewport; if (!vp) return;
  const r = vp.view.getBoundingClientRect();
  ivfRec = rec;
  ivf = h('div', { class: 'ivf', style: { left: (r.left + 10) + 'px', top: (r.top + 30) + 'px' } });
  const head = h('div', { class: 'ivf-title' }, h('span', { text: rec.name, style: { flex: '1' } }), h('span', { text: '✕', style: { cursor: 'default', padding: '0 4px' }, title: 'Close In-View Editor', onclick: closeIVF }));
  ivf.append(head);
  const rows = [];
  for (const ch of op.channels) {
    const isEnum = ch.type === 'enum', isBool = ch.type === 'bool';
    const n = h('span', { class: 'n', text: ch.label || niceName(ch.k) });
    let inp;
    if (isEnum) { inp = h('select', { style: { height: '16px', fontSize: '11px', width: '86px' } }, ch.options.map((o, i) => h('option', { value: i, text: o }))); inp.selectedIndex = ch.values ? Math.max(0, ch.values.indexOf(rec.params[ch.k])) : (+rec.params[ch.k] || 0); inp.addEventListener('change', () => { Undo.checkpoint('edit ' + rec.name); S.setAttr(rec, ch.k, ch.values ? ch.values[inp.selectedIndex] : inp.selectedIndex); }); }
    else if (isBool) { inp = h('input', { type: 'checkbox', checked: !!rec.params[ch.k] }); inp.addEventListener('change', () => { Undo.checkpoint('edit ' + rec.name); S.setAttr(rec, ch.k, inp.checked); }); }
    else {
      inp = h('input', { value: fmt(+rec.params[ch.k]) });
      inp.addEventListener('keydown', e => { e.stopPropagation(); if (e.key === 'Enter') inp.blur(); });
      inp.addEventListener('change', () => { let v = parseFloat(inp.value); if (isNaN(v)) return; if (ch.type === 'int') v = Math.round(v); if (ch.min !== undefined) v = Math.max(ch.min, v); if (ch.max !== undefined) v = Math.min(ch.max, v); Undo.checkpoint('edit ' + rec.name); S.setAttr(rec, ch.k, v); });
      // drag on the label = virtual slider (Maya: click the attribute name then MMB/LMB drag in the viewport)
      n.addEventListener('mousedown', (e) => {
        e.preventDefault(); Undo.checkpoint('edit ' + rec.name); const x0 = e.clientX; const v0 = +rec.params[ch.k] || 0; row.classList.add('hot');
        const span = ch.type === 'int' ? 0.05 : Math.max(0.002, Math.abs(v0) * 0.004 || 0.01);
        const mm = (ev) => { let v = v0 + (ev.clientX - x0) * span * (ev.shiftKey ? 5 : ev.ctrlKey ? 0.2 : 1); if (ch.type === 'int') v = Math.round(v); if (ch.min !== undefined) v = Math.max(ch.min, v); if (ch.max !== undefined) v = Math.min(ch.max, v); if (v !== rec.params[ch.k]) { S.setAttr(rec, ch.k, v, { silent: true }); inp.value = fmt(v); App.requestRender(); } };
        const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); row.classList.remove('hot'); App.dirty('channels', 'attr'); placeProxy(); };
        addEventListener('mousemove', mm); addEventListener('mouseup', mu);
      });
      n.style.cursor = 'ew-resize'; n.title = 'Drag left/right to change';
    }
    const row = h('div', { class: 'ivf-row' }, n, inp); row._upd = () => { if (!isEnum && !isBool && document.activeElement !== inp) inp.value = fmt(+rec.params[ch.k]); }; rows.push(row);
    ivf.append(row);
  }
  document.body.append(ivf);
  ivf._rows = rows;
});
App.on('refresh', (d) => { if (ivf && (d.has('channels') || d.has('attr'))) { if (!App.nodes.has(ivfRec?.id)) closeIVF(); else for (const r of ivf._rows) r._upd(); } });
App.on('selectionChanged', () => { if (!ivfRec) return; const o = S.historyOwner(ivfRec); if (!o || !(App.sel.includes(o) || App.hilite.includes(o))) closeIVF(); });
App.on('sceneLoaded', closeIVF);
addEventListener('keydown', (e) => { if (e.key === 'Escape' && ivf) closeIVF(); });

// ------------------------------------------------------------------ option boxes
App.ui.optionBox = (id, c) => {
  const saved = App.prefs.opt[id] || {};
  optionsDialog((c.label || id).replace(/\.\.\.$/, '') + ' Options', c.options, (vals) => { App.prefs.opt[id] = vals; App.savePrefs(); App.cmds.run(id, vals); }, { applyLabel: c.applyLabel || (/^poly|^nurbs/.test(id) ? 'Create' : 'Apply'), values: saved, onReset: () => { delete App.prefs.opt[id]; App.savePrefs(); } });
};
