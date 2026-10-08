// Inca — Hypergraph: Hierarchy view of the DAG plus an Input and Output Connections view
import { App } from '../core/app.js';
import { h, menuBar, FloatWin, promptDialog } from './dom.js';
import { icon, ICONS } from './icons.js';
import { dagChildren, dagParent, parentTo, rename, isDag } from '../core/scene.js';
import { Sel } from '../core/selection.js';
import { Undo } from '../core/undo.js';

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
// Left-to-right chain layout for the connections view.
export function layoutChain(n, { w = NW, h: nh = NH, gapX = 60 } = {}) {
  const out = []; for (let i = 0; i < n; i++) out.push({ x: i * (w + gapX), y: 0, w, h: nh });
  return out;
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
.hgwin .hg-node.hist { background: #4f6b52; } .hgwin .hg-node.mat { background: #6b4f6b; } .hgwin .hg-node.shape { background: #4f617a; }
.hgwin .hg-node.sel { background: #c9d9e6; color: #111; border-color: #fff; } .hgwin .hg-node.sel .sub { color: #333; }
.hgwin .hg-node.lead { background: #e9f3fb; }
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

  const st = { mode: mode === 'connections' ? 'connections' : 'hierarchy', showCameras: false, showLights: true, showCurves: true, showLocators: true, showShapes: false, view: { x: 30, y: 30, s: 1 }, framed: false };
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

  const makeNode = (rec, p, { label, sub, iconName, cls = '' }) => {
    const el = h('div', { class: 'hg-node ' + cls + (sub ? ' has-sub' : ''), style: { left: p.x + 'px', top: p.y + 'px' } },
      h('span', { class: 'ico', html: icon(iconName) }), h('span', { class: 'nm', text: label }), sub ? h('span', { class: 'sub', text: sub }) : null);
    el.title = label + (sub ? ' (' + sub + ')' : '');
    el._rec = rec;
    world.append(el); nodeEls.set(rec, el);
    return el;
  };

  const build = () => {
    for (const el of nodeEls.values()) el.remove();
    nodeEls = new Map(); edges.innerHTML = ''; msg.textContent = '';
    if (!App.world) { msg.textContent = 'No scene'; return; }
    if (st.mode === 'hierarchy') buildHierarchy(); else buildConnections();
    edges.setAttribute('width', Math.max(1, bounds.x1 + 20)); edges.setAttribute('height', Math.max(1, bounds.y1 + 20));
    updateSel();
    win.setTitle(st.mode === 'hierarchy' ? 'Hypergraph Hierarchy' : 'Hypergraph InputOutput');
  };
  const buildHierarchy = () => {
    const roots = kidsOf(App.world);
    if (!roots.length) { msg.textContent = 'The scene is empty'; bounds = { x0: 0, y0: 0, x1: NW, y1: NH }; return; }
    const { pos, width, height } = layoutTree(roots, kidsOf);
    bounds = { x0: 0, y0: 0, x1: width, y1: height };
    for (const [o, p] of pos) {
      const el = makeNode(o, p, { label: o.inca.name, sub: st.showShapes && o.inca.shapeName ? o.inca.shapeName : null, iconName: KIND_ICON[o.inca.kind] || 'olGroup' });
      if (o.visible === false || o.inca.visible === false) el.classList.add('hidden-obj');
      for (const c of kidsOf(o)) { const q = pos.get(c); if (q) line(p.x + NW / 2, p.y + NH, q.x + NW / 2, q.y); }
    }
  };
  const buildConnections = () => {
    const o = Sel.lead ? Sel.lead() : App.sel[App.sel.length - 1];
    if (!o) { msg.textContent = 'Select an object to display its input and output connections'; bounds = { x0: 0, y0: 0, x1: NW, y1: NH }; return; }
    const chain = [];
    for (const hh of o.inca.history || []) chain.push({ rec: hh, label: hh.name, sub: hh.type, iconName: ICONS[hh.type] ? hh.type : 'inputLink', cls: 'hist' });
    const shapeRec = { _shapeOf: o };
    chain.push({ rec: shapeRec, label: o.inca.shapeName || (o.inca.name + 'Shape'), sub: o.inca.kind + ' shape', iconName: KIND_ICON[o.inca.kind] || 'olMesh', cls: 'shape' });
    const mid = o.inca.material || (o.inca.kind === 'mesh' ? App.defaultMatId : null);
    const mat = mid && App.mats?.get ? App.mats.get(mid) : null;
    if (mat) chain.push({ rec: mat, label: mat.name, sub: mat.type || 'material', iconName: ICONS[mat.type] ? mat.type : 'olMaterial', cls: 'mat' });
    const ps = layoutChain(chain.length);
    const yOff = NH + 50;
    chain.forEach((c, i) => {
      const p = { ...ps[i], y: ps[i].y + yOff };
      makeNode(c.rec, p, c);
      if (i) { const q = ps[i - 1]; line(q.x + NW, q.y + yOff + NH / 2, p.x - 1, p.y + NH / 2, true, '#2f3a5a'); arrow(p.x - 1, p.y + NH / 2, '#2f3a5a'); }
    });
    // transform above its shape
    const si = chain.findIndex(c => c.rec === shapeRec);
    const tp = { x: ps[si].x, y: 0 };
    makeNode(o, tp, { label: o.inca.name, sub: 'transform', iconName: KIND_ICON[o.inca.kind] || 'olGroup' });
    line(tp.x + NW / 2, NH, tp.x + NW / 2, yOff, false, '#2a2a2a');
    bounds = { x0: 0, y0: 0, x1: ps[ps.length - 1].x + NW, y1: yOff + NH };
    if (!(o.inca.history || []).length && o.inca.kind === 'mesh') msg.textContent = '';
  };
  const updateSel = () => {
    const lead = App.sel[App.sel.length - 1];
    for (const [rec, el] of nodeEls) {
      const obj = rec && rec._shapeOf ? rec._shapeOf : rec;
      const s = isDag(obj) && App.sel.includes(obj);
      el.classList.toggle('sel', !!s); el.classList.toggle('lead', !!s && obj === lead);
    }
    status.textContent = `${App.sel.length} selected` + (st.mode === 'hierarchy' ? '   |   MMB-drag a node onto another to parent (empty space: unparent) · Alt+MMB pan · wheel zoom · F/A frame' : '   |   click a node to edit its attributes');
  };

  // ---- framing
  const frame = (onlySel) => {
    let b = null;
    if (onlySel) {
      for (const [rec, el] of nodeEls) {
        const obj = rec && rec._shapeOf ? rec._shapeOf : rec;
        if (!App.sel.includes(obj)) continue;
        const x = parseFloat(el.style.left), y = parseFloat(el.style.top);
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
  const selectRec = (rec, e) => {
    if (rec && rec._shapeOf) rec = rec._shapeOf;
    if (isDag(rec)) {
      const mode = e.shiftKey ? 'toggle' : e.ctrlKey ? 'deselect' : 'replace';
      Sel.select([rec], mode);
    } else if (rec && App.ui?.showAttr) App.ui.showAttr(rec);
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
    if (nodeEl) { selectRec(nodeEl._rec, e); return; }
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
      for (const [rec, el] of nodeEls) {
        const obj = rec && rec._shapeOf ? rec._shapeOf : rec; if (!isDag(obj)) continue;
        const x = parseFloat(el.style.left), y = parseFloat(el.style.top);
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
  const setMode = (m) => { st.mode = m; build(); frame(false); };
  win._setMode = (m) => setMode(m === 'connections' ? 'connections' : 'hierarchy');
  const tog = (label, key) => ({ label, check: () => st[key], fn: () => { st[key] = !st[key]; build(); } });
  const menuRow = h('div', { class: 'pmenubar' });
  menuBar(menuRow, [
    { label: 'Edit', items: [
      { label: 'Rename', fn: renameSelected },
      { label: 'Unparent', enabled: () => App.sel.some(o => dagParent(o)), fn: () => { Undo.checkpoint('unparent'); for (const o of App.sel.slice()) if (dagParent(o)) parentTo(o, null, true); App.dirty('outliner', 'channels'); build(); } },
      '-',
      { label: 'Select All', fn: () => Sel.select([...nodeEls.keys()].map(r => r && r._shapeOf ? r._shapeOf : r).filter(isDag), 'replace') },
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
  ]);
  win.body.append(menuRow, view, status);

  // ---- events
  const offs = [
    App.on('refresh', (flags) => { if (!flags || flags.has?.('outliner') || flags.has?.('hypershade')) build(); }),
    App.on('selectionChanged', () => { if (st.mode === 'connections') build(); else updateSel(); }),
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
