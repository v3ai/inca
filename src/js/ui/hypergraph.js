// Inca — Hypergraph: Hierarchy view of the DAG plus an Input and Output Connections view
import { App } from '../core/app.js';
import { h, menuBar, FloatWin, promptDialog, iconBtn } from './dom.js';
import { icon, ICONS } from './icons.js';
import { dagChildren, dagParent, parentTo, rename, isDag, historyOwner } from '../core/scene.js';
import { Sel } from '../core/selection.js';
import { Undo } from '../core/undo.js';
import { OPS } from '../core/ops.js';
import { recById, attrInfo, attrWidth } from '../core/connections.js';

const NW = 112, NH = 46, GX = 16, GY = 34;
const KIND_ICON = { mesh: 'olMesh', group: 'olGroup', light: 'olLight', camera: 'olCamera', curve: 'olCurve', locator: 'olLocator' };

// ---------------------------------------------------------------- pure layout
// Top-down tidy tree: roots side by side, children centred below their parent.
// Returns Map(node -> {x, y, w, h}) and overall {width, height}.
export function layoutTree(roots, childrenOf, { w = NW, h: nh = NH, gapX = GX, gapY = GY } = {}) {
  const pos = new Map(); const widthOf = new Map();
  const measure = (n, depth) => {
    if (depth > 512) return w;
    const kids = childrenOf(n);
    let sum = 0; kids.forEach((k, i) => { sum += measure(k, depth + 1) + (i ? gapX : 0); });
    const wd = Math.max(w, sum); widthOf.set(n, wd); return wd;
  };
  let maxY = 0;
  const place = (n, left, depth) => {
    const wd = widthOf.get(n); const kids = childrenOf(n);
    const y = depth * (nh + gapY);
    pos.set(n, { x: left + (wd - w) / 2, y, w, h: nh });
    maxY = Math.max(maxY, y + nh);
    let sum = 0; kids.forEach((k, i) => { sum += widthOf.get(k) + (i ? gapX : 0); });
    let x = left + (wd - sum) / 2;
    for (const k of kids) { place(k, x, depth + 1); x += widthOf.get(k) + gapX; }
  };
  let x = 0;
  roots.forEach((r, i) => { if (i) x += gapX * 2; measure(r, 0); place(r, x, 0); x += widthOf.get(r); });
  return { pos, width: x, height: maxY };
}
// Fit a bounds rect into a viewport: returns {x, y, s}
export function frameRect(b, vw, vh, pad = 40, maxS = 1.5) {
  const bw = Math.max(1, b.x1 - b.x0), bh = Math.max(1, b.y1 - b.y0);
  const s = Math.max(0.1, Math.min(maxS, (vw - pad * 2) / bw, (vh - pad * 2) / bh));
  return { s, x: vw / 2 - (b.x0 + bw / 2) * s, y: vh / 2 - (b.y0 + bh / 2) * s };
}

function injectStyle() {
  if (document.getElementById('inca-hg-style')) return;
  document.head.append(h('style', { id: 'inca-hg-style', text: `
.hgwin .hg-view { flex: 1; position: relative; overflow: hidden; background: #6b6b6b; outline: none; cursor: default; }
.hgwin .hg-view.grad { background: linear-gradient(#7a7f86, #4a4c50); }
.hgwin .hg-world { position: absolute; left: 0; top: 0; transform-origin: 0 0; }
.hgwin .hg-edges { position: absolute; left: 0; top: 0; overflow: visible; pointer-events: none; }
.hgwin .hg-node { position: absolute; width: ${NW}px; height: ${NH}px; background: #5a5a5a; border: 1px solid #2a2a2a; border-radius: 3px; display: flex; flex-direction: column; align-items: center; justify-content: center; color: #e8e8e8; font-size: 11px; box-shadow: 0 1px 3px rgba(0,0,0,.35); }
.hgwin .hg-node .ico { width: 22px; height: 22px; } .hgwin .hg-node .ico svg { width: 22px; height: 22px; }
.hgwin .hg-node .nm { max-width: ${NW - 8}px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 0 2px; }
.hgwin .hg-node .sub { font-size: 9.5px; color: #bbb; }
.hgwin .hg-node.hidden-obj { opacity: .55; }
.hgwin .hg-node.drop { outline: 2px solid #e6c54a; }
.hgwin .hg-node.sel { background: #b3b300; color: #111; border-color: #e6e600; } .hgwin .hg-node.sel .sub { color: #333; }
.hgwin .hg-node.lead { background: #c8c800; }
.hgwin .hg-view.free .hg-node { cursor: move; }
.hgwin .hg-tools { display: flex; align-items: center; gap: 2px; padding: 2px 4px; flex: none; border-bottom: 1px solid #2a2a2a; }
.hgwin .hg-tools .sep { width: 1px; height: 18px; background: #555; margin: 0 4px; }
.hgwin .hg-tools .ib.on { background: #5d7590; }
.hgwin .hg-edges .hit { pointer-events: stroke; cursor: help; }
.hgwin .hg-node .nm, .hgwin .hg-node .sub { line-height: 13px; }
.hgwin .hg-node.has-sub .ico, .hgwin .hg-node.has-sub .ico svg { width: 17px; height: 17px; }
.hgwin .hg-node input { width: ${NW - 10}px; height: 16px; font-size: 11px; }
.hgwin .hg-msg { position: absolute; left: 0; right: 0; top: 40%; text-align: center; color: #ddd; pointer-events: none; }
.hgwin .hg-marquee { position: absolute; border: 1px dashed #fff; background: rgba(255,255,255,.08); pointer-events: none; }
.hgwin .hg-ghost { position: fixed; z-index: 99999; pointer-events: none; padding: 2px 6px; background: rgba(40,40,40,.9); color: #fff; border: 1px solid #e6c54a; font-size: 11px; border-radius: 2px; }
.hgwin .hg-status { height: 18px; font-size: 11px; color: #aaa; padding: 0 6px; display: flex; align-items: center; flex: none; }
` }));
}

export function openHypergraph(mode) {
  injectStyle();
  const win = new FloatWin('hypergraph', 'Hypergraph Hierarchy', { w: 800, h: 520, minW: 360, minH: 240 });
  if (win.reused) { if (mode && win._setMode) win._setMode(mode); return win; }
  win.el.classList.add('hgwin');

  // ioIds: the objects the Input and Output Connections graph shows; it is kept until that graph is asked for again
  // (Graph menu / toolbar), so clicking nodes or empty space doesn't replace it. pos: node positions moved by hand
  // (Freeform Layout); activeKey: the node last clicked, highlighted instead of its object's transform
  const st = { mode: mode === 'connections' ? 'connections' : 'hierarchy', showCameras: false, showLights: true, showCurves: true, showLocators: true, showShapes: false, view: { x: 30, y: 30, s: 1 }, framed: false,
    ioIds: mode === 'connections' ? App.sel.filter(isDag).map(o => o.inca.id) : [], freeform: false, pos: new Map(), activeKey: null, selecting: false };
  const view = h('div', { class: 'hg-view', tabindex: '0' });
  const world = h('div', { class: 'hg-world' });
  const svgNS = 'http://www.w3.org/2000/svg';
  const edges = document.createElementNS(svgNS, 'svg'); edges.setAttribute('class', 'hg-edges');
  const msg = h('div', { class: 'hg-msg' });
  const status = h('div', { class: 'hg-status' });
  world.append(edges); view.append(world, msg);
  let nodeEls = new Map(); // obj/rec -> element
  let bounds = { x0: 0, y0: 0, x1: 0, y1: 0 };

  const applyView = () => { world.style.transform = `translate(${st.view.x}px, ${st.view.y}px) scale(${st.view.s})`; };
  const visibleKind = (o) => {
    const k = o.inca.kind;
    if (k === 'camera') return o.inca.startup ? st.showCameras : true;
    if (k === 'light') return st.showLights;
    if (k === 'curve') return st.showCurves;
    if (k === 'locator') return st.showLocators;
    return true;
  };
  // children shown in the graph: hidden kinds are skipped but their visible descendants are lifted up
  const kidsOf = (o) => {
    const out = [];
    for (const c of dagChildren(o)) { if (visibleKind(c)) out.push(c); else out.push(...kidsOf(c)); }
    return out;
  };
  const line = (x1, y1, x2, y2, curved = false, color = '#2a2a2a') => {
    const p = document.createElementNS(svgNS, 'path');
    const d = curved ? `M${x1},${y1} C${(x1 + x2) / 2},${y1} ${(x1 + x2) / 2},${y2} ${x2},${y2}` : `M${x1},${y1} C${x1},${(y1 + y2) / 2} ${x2},${(y1 + y2) / 2} ${x2},${y2}`;
    p.setAttribute('d', d); p.setAttribute('fill', 'none'); p.setAttribute('stroke', color); p.setAttribute('stroke-width', '1.4');
    edges.append(p); return p;
  };
  const arrow = (x, y, color = '#2a2a2a') => { const p = document.createElementNS(svgNS, 'path'); p.setAttribute('d', `M${x - 7},${y - 4} L${x},${y} L${x - 7},${y + 4}Z`); p.setAttribute('fill', color); edges.append(p); };

  // key: stable id of the node across rebuilds ('xf:' / 'sh:' + object id, or the history / material id);
  // obj: the DAG object the node belongs to. With Freeform Layout, a node moved by hand keeps its position.
  const makeNode = (rec, p, { label, sub, iconName, cls = '', key, obj = null }) => {
    const fp = st.freeform && st.pos.get(st.mode + '|' + key); if (fp) { p.x = fp.x; p.y = fp.y; }
    const el = h('div', { class: 'hg-node ' + cls + (sub ? ' has-sub' : ''), style: { left: p.x + 'px', top: p.y + 'px' } },
      h('span', { class: 'ico', html: icon(iconName) }), h('span', { class: 'nm', text: label }), sub ? h('span', { class: 'sub', text: sub }) : null);
    el.title = label + (sub ? ' (' + sub + ')' : '');
    el._rec = rec; el._key = key; el._obj = obj;
    world.append(el); nodeEls.set(rec, el);
    return el;
  };
  let loopX = Infinity; // leftmost point of the connection loops drawn beside a column (kept inside the framed area)
  const nodeRect = (el) => ({ x: parseFloat(el.style.left), y: parseFloat(el.style.top) });

  const build = () => {
    for (const el of nodeEls.values()) el.remove();
    nodeEls = new Map(); edges.innerHTML = ''; msg.textContent = ''; loopX = Infinity;
    if (!App.world) { msg.textContent = 'No scene'; return; }
    if (st.mode === 'hierarchy') buildHierarchy(); else buildConnections();
    if (nodeEls.size) {
      bounds = { x0: Infinity, y0: Infinity, x1: -Infinity, y1: -Infinity };
      for (const el of nodeEls.values()) { const { x, y } = nodeRect(el); bounds = { x0: Math.min(bounds.x0, x), y0: Math.min(bounds.y0, y), x1: Math.max(bounds.x1, x + NW), y1: Math.max(bounds.y1, y + NH) }; }
      if (st.mode === 'connections') bounds.x0 = Math.min(bounds.x0, loopX);
    }
    edges.setAttribute('width', Math.max(1, bounds.x1 + 20)); edges.setAttribute('height', Math.max(1, bounds.y1 + 20));
    view.classList.toggle('free', st.freeform);
    syncTools();
    updateSel();
    win.setTitle(st.mode === 'hierarchy' ? 'Hypergraph Hierarchy' : 'Hypergraph InputOutput');
  };
  const buildHierarchy = () => {
    const roots = kidsOf(App.world);
    if (!roots.length) { msg.textContent = 'The scene is empty'; bounds = { x0: 0, y0: 0, x1: NW, y1: NH }; return; }
    const { pos, width, height } = layoutTree(roots, kidsOf);
    bounds = { x0: 0, y0: 0, x1: width, y1: height };
    for (const [o, p] of pos) {
      const el = makeNode(o, p, { label: o.inca.name, sub: st.showShapes && o.inca.shapeName ? o.inca.shapeName : null, iconName: KIND_ICON[o.inca.kind] || 'olGroup', key: 'xf:' + o.inca.id, obj: o });
      if (o.visible === false || o.inca.visible === false) el.classList.add('hidden-obj');
    }
    for (const [o, p] of pos) for (const c of kidsOf(o)) { const q = pos.get(c); if (q) line(p.x + NW / 2, p.y + NH, q.x + NW / 2, q.y); }
  };
  // one row per graphed object, as in Maya: the transform on its own, then history -> shape -> material. The shapes
  // line up in one column and a material shared by several objects is drawn once, with every shape connecting to it
  const DATA = '#d040d0', SINGLE = '#2848d8', TRIPLE = '#3ccf3c'; // Maya's line colours: geometry / data, single, triple
  const buildConnections = () => {
    st.ioIds = st.ioIds.filter(id => isDag(App.nodes.get(id)));
    const objs = st.ioIds.map(id => App.nodes.get(id));
    // objects wired to the graphed ones by direct connections (Connection Editor) get rows too
    const ownerOf = (r) => r?.isObject3D ? r : r?.kind === 'history' ? App.nodes.get(r.owner) : null;
    for (const c of App.conns || []) {
      const a = ownerOf(recById(c.src)), b = ownerOf(recById(c.dst));
      if (a && b && isDag(a) && isDag(b)) { if (objs.includes(a) && !objs.includes(b)) objs.push(b); else if (objs.includes(b) && !objs.includes(a)) objs.push(a); }
    }
    if (!objs.length) { msg.textContent = 'Select objects, then Graph > Input and Output Connections'; bounds = { x0: 0, y0: 0, x1: NW, y1: NH }; return; }
    const COL = NW + 50, ROWH = NH + 40;
    const maxHist = Math.max(...objs.map(o => (o.inca.history || []).length));
    const shapeX = (maxHist + 1) * COL, matX = shapeX + COL;
    const link = (a, b, color = DATA) => { line(a.x + NW, a.y + NH / 2, b.x - 1, b.y + NH / 2, true, color); arrow(b.x - 1, b.y + NH / 2, color); };
    const matUsers = new Map(); // material -> shape positions
    objs.forEach((o, r) => {
      const y = r * ROWH; const hist = o.inca.history || [];
      makeNode(o, { x: 0, y }, { label: o.inca.name, sub: 'transform', iconName: KIND_ICON[o.inca.kind] || 'olGroup', key: 'xf:' + o.inca.id, obj: o });
      let prev = null;
      hist.forEach((hh, i) => {
        const p = { x: (maxHist - hist.length + i + 1) * COL, y };
        makeNode(hh, p, { label: hh.name, sub: hh.type, iconName: ICONS[hh.type] ? hh.type : 'inputLink', key: hh.id, obj: o });
        if (prev) link(prev, p); prev = p;
      });
      const sp = { x: shapeX, y };
      makeNode({ _shapeOf: o }, sp, { label: o.inca.shapeName || (o.inca.name + 'Shape'), sub: OPS[hist[0]?.type]?.nurbs ? 'nurbsSurface' : o.inca.kind + ' shape', iconName: KIND_ICON[o.inca.kind] || 'olMesh', key: 'sh:' + o.inca.id, obj: o });
      if (prev) link(prev, sp);
      const mid = o.inca.material || (o.inca.kind === 'mesh' ? App.defaultMatId : null);
      const mat = mid && App.mats?.get ? App.mats.get(mid) : null;
      if (mat) { if (!matUsers.has(mat)) matUsers.set(mat, []); matUsers.get(mat).push(sp); }
    });
    // each material level with the middle of its users, pushed down so materials don't overlap
    let minY = -Infinity;
    for (const [mat, users] of matUsers) {
      const y = Math.max(minY, users.reduce((a, u) => a + u.y, 0) / users.length);
      const p = { x: matX, y }; minY = y + NH + 20;
      makeNode(mat, p, { label: mat.name, sub: mat.type || 'material', iconName: ICONS[mat.type] ? mat.type : 'olMaterial', cls: 'mat', key: mat.id });
      for (const u of users) link(u, p);
    }
    // direct connections between nodes on the graph: one line per connection, with links between the same pair of
    // nodes stacked in parallel (like Maya); hover a line for its attributes
    const pairs = new Map();
    for (const c of App.conns || []) {
      const a = nodeEls.get(recById(c.src)), b = nodeEls.get(recById(c.dst)); if (!a || !b || a === b) continue;
      const k = a._key + '>' + b._key; if (!pairs.has(k)) pairs.set(k, { a, b, list: [] });
      pairs.get(k).list.push(c);
    }
    for (const { a, b, list } of pairs.values()) {
      const { x: ax, y: ay } = nodeRect(a), { x: bx, y: by } = nodeRect(b);
      const n = list.length, gap = n > 1 ? Math.min(9, (NH - 12) / (n - 1)) : 0;
      // same column: loop out to the left of both nodes; otherwise from one node's facing side to the other's
      const same = Math.abs(bx - ax) < NW, fwd = same || bx >= ax;
      const x1 = same ? ax : fwd ? ax + NW : ax, x2 = same ? bx - 1 : fwd ? bx - 1 : bx + NW + 1;
      const lx = Math.min(ax, bx) - 45; if (same) loopX = Math.min(loopX, lx - 6);
      list.forEach((c, i) => {
        const dy = (i - (n - 1) / 2) * gap;
        const col = attrWidth(attrInfo(recById(c.src), c.srcAttr)) === 3 ? TRIPLE : SINGLE;
        const y1 = ay + NH / 2 + dy, y2 = by + NH / 2 + dy;
        const d = same ? `M${x1},${y1} C${lx},${y1} ${lx},${y2} ${x2},${y2}` : `M${x1},${y1} C${(x1 + x2) / 2},${y1} ${(x1 + x2) / 2},${y2} ${x2},${y2}`;
        const path = (stroke, w, cls) => { const q = document.createElementNS(svgNS, 'path'); q.setAttribute('d', d); q.setAttribute('fill', 'none'); q.setAttribute('stroke', stroke); q.setAttribute('stroke-width', w); if (cls) q.setAttribute('class', cls); edges.append(q); return q; };
        path(col, '1.6');
        const hit = path('transparent', '9', 'hit');
        const t = document.createElementNS(svgNS, 'title'); t.textContent = `${a.title.split(' (')[0]}.${c.srcAttr} → ${b.title.split(' (')[0]}.${c.dstAttr}`; hit.append(t);
        const p = document.createElementNS(svgNS, 'path'); const dd = fwd ? 1 : -1;
        p.setAttribute('d', `M${x2 - 7 * dd},${y2 - 4} L${x2},${y2} L${x2 - 7 * dd},${y2 + 4}Z`); p.setAttribute('fill', col); edges.append(p);
      });
    }
  };
  // graph the selected objects' input and output connections (Graph menu / toolbar)
  const graphSelection = () => { st.ioIds = App.sel.filter(isDag).map(o => o.inca.id); st.activeKey = null; st.mode = 'connections'; build(); frame(false); };
  // hierarchy: the selected objects. Input / output: per selected object, the node last clicked on it (its transform
  // unless a history, shape or material node was clicked)
  const updateSel = () => {
    const lead = App.sel[App.sel.length - 1];
    const act = st.activeKey ? [...nodeEls.values()].find(el => el._key === st.activeKey) : null;
    for (const el of nodeEls.values()) {
      const obj = el._obj; let s;
      if (st.mode === 'hierarchy') s = !!obj && App.sel.includes(obj);
      else if (el === act) s = !obj || App.sel.includes(obj);
      else s = !!obj && el._key === 'xf:' + obj.inca.id && App.sel.includes(obj) && !(act && (act._obj === obj || !act._obj));
      el.classList.toggle('sel', s); el.classList.toggle('lead', s && !!obj && obj === lead);
    }
    status.textContent = `${App.sel.length} selected` + (st.mode === 'hierarchy' ? '   |   MMB-drag a node onto another to parent (empty space: unparent) · Alt+MMB pan · wheel zoom · F/A frame' : '   |   click a node to edit its attributes · MMB-drag a node onto another to connect them · Graph > Input and Output Connections shows the selection');
  };

  // ---- framing
  const frame = (onlySel) => {
    let b = null;
    if (onlySel) {
      for (const el of nodeEls.values()) {
        if (!el.classList.contains('sel')) continue;
        const { x, y } = nodeRect(el);
        b = b ? { x0: Math.min(b.x0, x), y0: Math.min(b.y0, y), x1: Math.max(b.x1, x + NW), y1: Math.max(b.y1, y + NH) } : { x0: x, y0: y, x1: x + NW, y1: y + NH };
      }
    }
    b = b || bounds;
    st.view = frameRect(b, view.clientWidth || 600, view.clientHeight || 400, 40, onlySel ? 1.5 : 1.2);
    applyView();
  };

  // ---- interactions
  const toWorld = (cx, cy) => { const r = view.getBoundingClientRect(); return { x: (cx - r.left - st.view.x) / st.view.s, y: (cy - r.top - st.view.y) / st.view.s }; };
  const nodeAt = (target) => { const el = target && target.closest ? target.closest('.hg-node') : null; return el && nodeEls.get(el._rec) === el ? el : null; };
  // clicking a node selects its object and shows that node's tab in the Attribute Editor; the graph stays as it is
  const selectRec = (el, e) => {
    const rec = el._rec, obj = el._obj;
    st.activeKey = el._key;
    if (obj) {
      st.selecting = true;
      try { Sel.select([obj], e.shiftKey ? 'toggle' : e.ctrlKey ? 'deselect' : 'replace'); } finally { st.selecting = false; }
    }
    if (st.mode === 'connections' && App.ui?.showAttr && (!obj || App.sel.includes(obj))) {
      if (rec?._shapeOf) App.ui.showAttr(obj, 'shape');
      else App.ui.showAttr(rec);
    }
    updateSel();
  };

  view.addEventListener('contextmenu', (e) => e.preventDefault());
  view.addEventListener('mousedown', (e) => {
    view.focus({ preventScroll: true });
    const nodeEl = nodeAt(e.target);
    if (e.target.tagName === 'INPUT') return;
    // Alt+MMB / Alt+LMB pan, Alt+RMB zoom
    if (e.altKey && (e.button === 1 || e.button === 0 || e.button === 2)) {
      e.preventDefault();
      const x0 = e.clientX, y0 = e.clientY, v0 = { ...st.view };
      const r = view.getBoundingClientRect(); const cx = x0 - r.left, cy = y0 - r.top;
      const mm = (ev) => {
        if (e.button === 2) { const s = Math.max(0.1, Math.min(4, v0.s * Math.exp((ev.clientX - x0 + ev.clientY - y0) * 0.005))); st.view = { s, x: cx - (cx - v0.x) * s / v0.s, y: cy - (cy - v0.y) * s / v0.s }; }
        else st.view = { ...v0, x: v0.x + ev.clientX - x0, y: v0.y + ev.clientY - y0 };
        applyView();
      };
      const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu);
      return;
    }
    if (e.button === 1) {
      e.preventDefault();
      // connections view: MMB-drag a node onto another to connect them in the Connection Editor (dragged = outputs)
      if (nodeEl && st.mode === 'connections') {
        const src = nodeEl._rec?._shapeOf || nodeEl._rec;
        const ghost = h('div', { class: 'hg-ghost', text: nodeEl.title }); win.el.append(ghost);
        let over = null;
        const mm = (ev) => {
          ghost.style.left = ev.clientX + 12 + 'px'; ghost.style.top = ev.clientY + 8 + 'px';
          const t = nodeAt(document.elementFromPoint(ev.clientX, ev.clientY)); const tgt = t && t !== nodeEl ? t : null;
          if (over !== tgt) { over?.classList.remove('drop'); over = tgt; over?.classList.add('drop'); }
        };
        const mu = () => {
          removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); ghost.remove(); over?.classList.remove('drop');
          if (over) App.ui.connectionEditor?.({ left: src, right: over._rec?._shapeOf || over._rec });
        };
        addEventListener('mousemove', mm); addEventListener('mouseup', mu);
        return;
      }
      if (!nodeEl || st.mode !== 'hierarchy' || !isDag(nodeEl._rec)) { // MMB on empty space pans as well
        const x0 = e.clientX, y0 = e.clientY, v0 = { ...st.view };
        const mm = (ev) => { st.view = { ...v0, x: v0.x + ev.clientX - x0, y: v0.y + ev.clientY - y0 }; applyView(); };
        const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); };
        addEventListener('mousemove', mm); addEventListener('mouseup', mu);
        return;
      }
      // MMB drag a node: drop on a node to parent, on empty space to unparent
      const child = nodeEl._rec;
      const ghost = h('div', { class: 'hg-ghost', text: child.inca.name }); win.el.append(ghost);
      let over = null; let moved = false;
      const mm = (ev) => {
        moved = true;
        ghost.style.left = ev.clientX + 12 + 'px'; ghost.style.top = ev.clientY + 8 + 'px';
        const t = nodeAt(document.elementFromPoint(ev.clientX, ev.clientY));
        const tgt = t && t !== nodeEl ? t : null;
        if (over !== tgt) { over?.classList.remove('drop'); over = tgt; over?.classList.add('drop'); }
        ghost.textContent = child.inca.name + (over ? '  →  ' + over._rec.inca.name : (view.contains(document.elementFromPoint(ev.clientX, ev.clientY)) ? '  →  world' : ''));
      };
      const mu = (ev) => {
        removeEventListener('mousemove', mm); removeEventListener('mouseup', mu);
        ghost.remove(); over?.classList.remove('drop');
        if (!moved) return;
        const inView = view.contains(document.elementFromPoint(ev.clientX, ev.clientY));
        if (!inView) return;
        const parent = over ? over._rec : null;
        if (parent === dagParent(child) || (!parent && !dagParent(child))) return;
        if (parent && parent.inca.startup) return;
        Undo.checkpoint('parent');
        if (parentTo(child, parent, true)) {
          App.emit('echo', parent ? `parent ${child.inca.name} ${parent.inca.name};` : `parent -w ${child.inca.name};`);
          App.dirty('outliner', 'channels', 'attr');
          App.requestRender();
          build();
        } else App.emit('warning', `// Warning: Cannot parent ${child.inca.name} to ${parent ? parent.inca.name : 'world'}.`);
      };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu);
      return;
    }
    if (e.button !== 0) return;
    if (nodeEl) {
      selectRec(nodeEl, e);
      if (!st.freeform) return;
      // Freeform Layout: LMB-drag a node to move it
      const key = nodeEl._key, p0 = nodeRect(nodeEl), x0 = e.clientX, y0 = e.clientY; let moved = false;
      const mm = (ev) => {
        if (!moved && Math.abs(ev.clientX - x0) + Math.abs(ev.clientY - y0) < 4) return;
        moved = true; st.pos.set(st.mode + '|' + key, { x: p0.x + (ev.clientX - x0) / st.view.s, y: p0.y + (ev.clientY - y0) / st.view.s }); build();
      };
      const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu);
      return;
    }
    // marquee on empty space
    const p0 = toWorld(e.clientX, e.clientY);
    const mq = h('div', { class: 'hg-marquee' }); world.append(mq);
    let dragged = false;
    const mm = (ev) => {
      const p1 = toWorld(ev.clientX, ev.clientY); dragged = dragged || Math.abs(ev.clientX - e.clientX) + Math.abs(ev.clientY - e.clientY) > 3;
      Object.assign(mq.style, { left: Math.min(p0.x, p1.x) + 'px', top: Math.min(p0.y, p1.y) + 'px', width: Math.abs(p1.x - p0.x) + 'px', height: Math.abs(p1.y - p0.y) + 'px' });
    };
    const mu = (ev) => {
      removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); mq.remove();
      if (!dragged) { if (!e.shiftKey && !e.ctrlKey) Sel.select([], 'replace'); return; }
      const p1 = toWorld(ev.clientX, ev.clientY);
      const x0 = Math.min(p0.x, p1.x), x1 = Math.max(p0.x, p1.x), y0 = Math.min(p0.y, p1.y), y1 = Math.max(p0.y, p1.y);
      const hits = [];
      for (const el of nodeEls.values()) {
        const obj = el._obj; if (!isDag(obj)) continue;
        const { x, y } = nodeRect(el);
        if (x < x1 && x + NW > x0 && y < y1 && y + NH > y0 && !hits.includes(obj)) hits.push(obj);
      }
      Sel.select(hits, e.shiftKey ? 'toggle' : e.ctrlKey ? 'deselect' : 'replace');
    };
    addEventListener('mousemove', mm); addEventListener('mouseup', mu);
  });
  view.addEventListener('auxclick', (e) => { if (e.button === 1) e.preventDefault(); });
  view.addEventListener('wheel', (e) => {
    e.preventDefault();
    const r = view.getBoundingClientRect(); const cx = e.clientX - r.left, cy = e.clientY - r.top;
    const s = Math.max(0.1, Math.min(4, st.view.s * Math.exp(-e.deltaY * 0.0015)));
    st.view = { s, x: cx - (cx - st.view.x) * s / st.view.s, y: cy - (cy - st.view.y) * s / st.view.s };
    applyView();
  }, { passive: false });
  view.addEventListener('dblclick', (e) => {
    const el = nodeAt(e.target); if (!el) return;
    const rec = el._rec && el._rec._shapeOf ? null : el._rec; if (!rec) return;
    startRename(rec, el);
  });
  const startRename = (rec, el) => {
    if (isDag(rec) && rec.inca.startup) return;
    const nm = el.querySelector('.nm'); if (!nm) return;
    const old = isDag(rec) ? rec.inca.name : rec.name;
    const inp = h('input', { value: old });
    nm.replaceWith(inp); inp.focus(); inp.select();
    let done = false;
    const fin = (ok) => {
      if (done) return; done = true;
      const v = inp.value.trim();
      if (ok && v && v !== old) {
        Undo.checkpoint('rename');
        rename(rec, v);
        App.emit('echo', `rename "${old}" "${isDag(rec) ? rec.inca.name : rec.name}";`);
        App.dirty('outliner', 'channels', 'attr', 'hypershade');
      }
      build();
    };
    inp.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') fin(true); if (e.key === 'Escape') fin(false); });
    inp.addEventListener('blur', () => fin(true));
    inp.addEventListener('mousedown', (e) => e.stopPropagation());
  };
  const renameSelected = async () => {
    const o = App.sel[App.sel.length - 1]; if (!o) return;
    const el = nodeEls.get(o); if (el) { startRename(o, el); return; }
    const v = await (App.ui?.prompt || promptDialog)('Rename', 'New name:', o.inca.name);
    if (v && v !== o.inca.name) { Undo.checkpoint('rename'); rename(o, v); build(); }
  };

  win.el.addEventListener('keydown', (e) => {
    if (e.target.tagName === 'INPUT') { e.stopPropagation(); return; }
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    const k = e.key.toLowerCase();
    if (k === 'f') { frame(true); e.stopPropagation(); e.preventDefault(); }
    else if (k === 'a') { frame(false); e.stopPropagation(); e.preventDefault(); }
    else if (e.key === 'ArrowUp' || e.key === 'ArrowDown' || e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
      // pick-walk within the graph
      const o = App.sel[App.sel.length - 1]; if (!o || !isDag(o)) return;
      let n = null;
      if (e.key === 'ArrowUp') n = dagParent(o);
      else if (e.key === 'ArrowDown') n = kidsOf(o)[0];
      else { const sib = dagParent(o) ? kidsOf(dagParent(o)) : kidsOf(App.world); const i = sib.indexOf(o); n = sib[i + (e.key === 'ArrowRight' ? 1 : -1)]; }
      if (n) Sel.select([n], 'replace');
      e.stopPropagation(); e.preventDefault();
    }
  });

  // ---- menus
  const setMode = (m) => { if (m === 'connections') { graphSelection(); return; } st.mode = m; build(); frame(false); };
  win._setMode = (m) => setMode(m === 'connections' ? 'connections' : 'hierarchy');
  const setFreeform = (on) => { st.freeform = on; build(); };
  const tog = (label, key) => ({ label, check: () => st[key], fn: () => { st[key] = !st[key]; build(); } });
  const menuRow = h('div', { class: 'pmenubar' });
  menuBar(menuRow, [
    { label: 'Edit', items: [
      { label: 'Rename', fn: renameSelected },
      { label: 'Unparent', enabled: () => App.sel.some(o => dagParent(o)), fn: () => { Undo.checkpoint('unparent'); for (const o of App.sel.slice()) if (dagParent(o)) parentTo(o, null, true); App.dirty('outliner', 'channels'); build(); } },
      '-',
      { label: 'Select All', fn: () => Sel.select([...new Set([...nodeEls.values()].map(el => el._obj).filter(isDag))], 'replace') },
    ] },
    { label: 'View', items: [
      { label: 'Frame All', hk: 'A', fn: () => frame(false) },
      { label: 'Frame Selection', hk: 'F', fn: () => frame(true) },
      { label: 'Reset Zoom', fn: () => { st.view.s = 1; applyView(); } },
      '-',
      { label: 'Gradient Background', check: () => view.classList.contains('grad'), fn: () => view.classList.toggle('grad') },
    ] },
    { label: 'Show', items: [
      tog('Cameras', 'showCameras'), tog('Lights', 'showLights'), tog('Curves', 'showCurves'), tog('Locators', 'showLocators'), '-', tog('Shape Names', 'showShapes'),
    ] },
    { label: 'Graph', items: [
      { label: 'Scene Hierarchy', check: () => st.mode === 'hierarchy', fn: () => setMode('hierarchy') },
      { label: 'Input and Output Connections', check: () => st.mode === 'connections', fn: () => setMode('connections') },
      '-',
      { label: 'Rebuild', fn: () => { build(); frame(false); } },
    ] },
    { label: 'Options', items: [
      { label: 'Layout', sub: [
        { label: 'Automatic Layout', check: () => !st.freeform, fn: () => setFreeform(false) },
        { label: 'Freeform Layout', check: () => st.freeform, fn: () => setFreeform(true) },
      ] },
      { label: 'Reset Node Positions', enabled: () => st.pos.size > 0, fn: () => { for (const k of [...st.pos.keys()]) if (k.startsWith(st.mode + '|')) st.pos.delete(k); build(); } },
    ] },
  ]);
  const tb = (name, title, fn) => iconBtn(name, title, fn, 'ib small');
  const tools = h('div', { class: 'hg-tools' });
  const btnHier = tb('outliner', 'Scene Hierarchy', () => setMode('hierarchy'));
  const btnIO = tb('inputLink', 'Input and Output Connections: graph the selected objects', () => setMode('connections'));
  const btnFree = tb('move', 'Freeform Layout: LMB-drag nodes to move them (off: automatic layout)', () => setFreeform(!st.freeform));
  tools.append(tb('frameAll', 'Frame All (A)', () => frame(false)), tb('frameSel', 'Frame Selection (F)', () => frame(true)), h('span', { class: 'sep' }), btnHier, btnIO, h('span', { class: 'sep' }), btnFree);
  function syncTools() { btnHier.classList.toggle('on', st.mode === 'hierarchy'); btnIO.classList.toggle('on', st.mode === 'connections'); btnFree.classList.toggle('on', st.freeform); }
  win.body.append(menuRow, tools, view, status);

  // ---- events
  const offs = [
    App.on('refresh', (flags) => { if (!flags || flags.has?.('outliner') || flags.has?.('hypershade') || (st.mode === 'connections' && flags.has?.('connections'))) build(); }),
    App.on('selectionChanged', () => {
      if (!st.selecting) st.activeKey = null;
      // an empty Input and Output graph picks up the first selection; after that it stays until graphed again
      if (st.mode === 'connections' && !st.ioIds.length && App.sel.some(isDag)) { graphSelection(); return; }
      updateSel();
    }),
    App.on('sceneLoaded', () => { build(); frame(false); }),
  ];
  win.onClose = () => { for (const off of offs) off(); };
  let firstFit = true;
  win.onResize = () => { if (firstFit && view.clientWidth > 0) { firstFit = false; frame(false); } };

  build(); applyView();
  requestAnimationFrame(() => { if (view.clientWidth) { firstFit = false; frame(false); } });
  return win;
}

App.ui = App.ui || {};
App.ui.hypergraph = openHypergraph;
App.ui.hypergraphConnections = () => openHypergraph('connections');
