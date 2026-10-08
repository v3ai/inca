// Inca — Dope Sheet (key timing editor). Shares key selection with the Graph Editor.
import { App } from '../core/app.js';
import { Undo } from '../core/undo.js';
import { nodeName } from '../core/scene.js';
import { h, iconBtn, menuBar, FloatWin, toast, showMenu } from './dom.js';
import * as M from './editorMath.js';
import { keySelection, pruneSelection, setTimeSafe, animChanged, removeEmptyCurves, sortedCurvesOf, injectEditorStyle, setupWindowKeys } from './graphEditor.js';

const RH = 18, ROW = 18, NAME_W = 180;

export class DopeView {
  constructor(wrap, { sel = keySelection, onChange = null } = {}) {
    this.wrap = wrap; this.sel = sel; this.onChange = onChange;
    this.cv = h('canvas'); wrap.append(this.cv);
    this.view = { t0: -2, sx: 8 }; this.scrollY = 0; this.rows = []; this.collapsed = new Set();
    this.W = 100; this.H = 100; this.drag = null; this.marquee = null; this._raf = 0;
    this.cv.addEventListener('mousedown', (e) => this.onDown(e));
    this.cv.addEventListener('wheel', (e) => this.onWheel(e), { passive: false });
    this.cv.addEventListener('contextmenu', (e) => e.preventDefault());
  }
  X(t) { return NAME_W + (t - this.view.t0) * this.view.sx; }
  T(x) { return (x - NAME_W) / this.view.sx + this.view.t0; }
  rowY(i) { return RH + i * ROW - this.scrollY; }
  local(e) { const r = this.cv.getBoundingClientRect(); return [e.clientX - r.left, e.clientY - r.top]; }
  request() { if (!this._raf) this._raf = requestAnimationFrame(() => { this._raf = 0; this.draw(); }); }
  resize() { const r = this.wrap.getBoundingClientRect(); this.W = Math.max(10, r.width); this.H = Math.max(10, r.height); this.draw(); }
  setNodes(nodes) {
    const rows = []; const all = [];
    const per = nodes.map(n => ({ n, cs: sortedCurvesOf(n) }));
    for (const p of per) all.push(...p.cs);
    rows.push({ type: 'summary', label: 'Dope Sheet Summary', curves: all, depth: 0 });
    for (const { n, cs } of per) {
      const id = n.inca.id;
      rows.push({ type: 'node', label: nodeName(n), curves: cs, depth: 1, id });
      if (!this.collapsed.has(id)) for (const c of cs) rows.push({ type: 'curve', label: c.attr, curves: [c], depth: 2, curve: c });
    }
    this.rows = rows; this.clampScroll(); this.request();
  }
  clampScroll() { const max = Math.max(0, this.rows.length * ROW - (this.H - RH)); this.scrollY = M.clamp(this.scrollY, 0, max); }
  allCurves() { return this.rows[0] ? this.rows[0].curves : []; }
  rowTimes(r) { const s = new Map(); for (const c of r.curves) for (const k of c.keys) { const t = Math.round(k.t * 1e4) / 1e4; let a = s.get(t); if (!a) s.set(t, a = []); a.push([c, k]); } return s; }
  frameAll() {
    let t0 = Infinity, t1 = -Infinity; for (const c of this.allCurves()) for (const k of c.keys) { t0 = Math.min(t0, k.t); t1 = Math.max(t1, k.t); }
    if (!isFinite(t0)) { t0 = App.time.start; t1 = App.time.end; }
    this.frameRange(t0, t1);
  }
  frameSelection() {
    let t0 = Infinity, t1 = -Infinity; for (const s of M.selFilter(this.sel, this.allCurves()).values()) for (const k of s) { t0 = Math.min(t0, k.t); t1 = Math.max(t1, k.t); }
    if (!isFinite(t0)) return this.frameAll(); this.frameRange(t0, t1);
  }
  frameRange(t0, t1) { if (t1 - t0 < 1) { t0 -= 5; t1 += 5; } const w = this.W - NAME_W - 30; this.view.sx = Math.max(0.05, w / (t1 - t0)); this.view.t0 = t0 - 15 / this.view.sx; this.request(); }
  framePlayback() { this.frameRange(App.time.start, App.time.end); }
  centerTime() { this.view.t0 = App.time.current - (this.W - NAME_W) / 2 / this.view.sx; this.request(); }
  // ---------------------------------------------------------------- draw
  draw() {
    const ctx = M.fitCanvas(this.cv, this.W, this.H); const { W, H } = this;
    ctx.fillStyle = '#3a3a3a'; ctx.fillRect(0, 0, W, H);
    ctx.fillStyle = '#393939'; ctx.fillRect(NAME_W, RH, W - NAME_W, H - RH);
    const pa = Math.max(NAME_W, this.X(App.time.start)), pb = Math.min(W, this.X(App.time.end));
    if (pb > pa) { ctx.fillStyle = '#3e3e3e'; ctx.fillRect(pa, RH, pb - pa, H - RH); }
    // frame grid
    const step = Math.max(1, M.niceStep(60 / this.view.sx)), minor = Math.max(1, M.niceStep(10 / this.view.sx));
    ctx.lineWidth = 1; ctx.beginPath(); ctx.strokeStyle = '#333';
    for (let t = Math.floor(this.T(NAME_W) / minor) * minor; t <= this.T(W); t += minor) { const x = Math.round(this.X(t)) + 0.5; if (x < NAME_W) continue; ctx.moveTo(x, RH); ctx.lineTo(x, H); }
    ctx.stroke();
    ctx.beginPath(); ctx.strokeStyle = '#2c2c2c';
    for (let t = Math.floor(this.T(NAME_W) / step) * step; t <= this.T(W); t += step) { const x = Math.round(this.X(t)) + 0.5; if (x < NAME_W) continue; ctx.moveTo(x, RH); ctx.lineTo(x, H); }
    ctx.stroke();
    // rows
    ctx.save(); ctx.beginPath(); ctx.rect(0, RH, W, H - RH); ctx.clip();
    const bw = M.clamp(this.view.sx * 0.7, 4, 10);
    this.rows.forEach((r, i) => {
      const y = this.rowY(i); if (y + ROW < RH || y > H) return;
      ctx.fillStyle = r.type === 'summary' ? '#4a4a4a' : r.type === 'node' ? '#424242' : (i % 2 ? '#3b3b3b' : '#383838');
      ctx.fillRect(0, y, NAME_W, ROW - 1);
      if (r.type !== 'curve') { ctx.fillStyle = r.type === 'summary' ? 'rgba(255,255,255,0.05)' : 'rgba(255,255,255,0.025)'; ctx.fillRect(NAME_W, y, W - NAME_W, ROW - 1); }
      ctx.fillStyle = '#2d2d2d'; ctx.fillRect(0, y + ROW - 1, W, 1);
      // label
      ctx.font = (r.type === 'curve' ? '' : 'bold ') + '11px sans-serif'; ctx.textBaseline = 'middle'; ctx.fillStyle = r.type === 'curve' ? '#cfcfcf' : '#eee';
      const lx = 6 + r.depth * 12;
      if (r.type === 'node') { ctx.fillStyle = '#aaa'; ctx.font = '9px sans-serif'; ctx.fillText(this.collapsed.has(r.id) ? '▶' : '▼', lx - 11, y + ROW / 2); ctx.font = 'bold 11px sans-serif'; ctx.fillStyle = '#eee'; }
      if (r.type === 'curve') { ctx.fillStyle = M.attrColor(r.curve.attr); ctx.fillRect(lx, y + 5, 7, 7); ctx.fillStyle = '#cfcfcf'; }
      ctx.fillText(r.label, lx + (r.type === 'curve' ? 11 : 0), y + ROW / 2, NAME_W - lx - 14);
      // keys
      for (const [t, pairs] of this.rowTimes(r)) {
        const x = this.X(t); if (x < NAME_W - bw || x > W + bw) continue;
        const on = pairs.every(([c, k]) => M.selHas(this.sel, c, k)), some = !on && pairs.some(([c, k]) => M.selHas(this.sel, c, k));
        ctx.fillStyle = on ? '#ffef1f' : some ? '#c8b84a' : r.type === 'summary' ? '#8f8f8f' : r.type === 'node' ? '#a6a6a6' : '#b8b8b8';
        ctx.fillRect(Math.round(x - bw / 2), y + 3, Math.round(bw), ROW - 7);
        ctx.strokeStyle = '#1e1e1e'; ctx.strokeRect(Math.round(x - bw / 2) + 0.5, y + 3.5, Math.round(bw) - 1, ROW - 8);
      }
    });
    ctx.restore();
    ctx.fillStyle = '#262626'; ctx.fillRect(NAME_W - 1, RH, 1, H - RH);
    // ruler + time
    const tx = this.X(App.time.current);
    ctx.save(); ctx.beginPath(); ctx.rect(NAME_W, 0, W - NAME_W, H); ctx.clip();
    if (tx >= NAME_W) M.drawTimeMarker(ctx, { x: tx, RH, H });
    ctx.restore();
    ctx.fillStyle = '#353535'; ctx.fillRect(0, 0, NAME_W, RH);
    M.drawTimeRuler(ctx, { W, RH, X: (t) => this.X(t), T: (x) => this.T(x), sx: this.view.sx, x0: NAME_W, range: [App.time.start, App.time.end] });
    if (tx >= NAME_W) { ctx.save(); ctx.beginPath(); ctx.rect(NAME_W, 0, W - NAME_W, RH); ctx.clip(); M.drawTimeBox(ctx, { x: tx, RH, frame: App.time.current }); ctx.restore(); }
    if (this.rows.length <= 1 && !this.allCurves().length) { ctx.fillStyle = '#8a8a8a'; ctx.font = '12px sans-serif'; ctx.textAlign = 'center'; ctx.fillText('Select animated objects to display their keys', NAME_W + (W - NAME_W) / 2, (H + RH) / 2); ctx.textAlign = 'left'; }
    if (this.marquee) { const m = this.marquee; ctx.setLineDash([3, 3]); ctx.strokeStyle = '#eee'; ctx.strokeRect(Math.min(m.x0, m.x1) + 0.5, Math.min(m.y0, m.y1) + 0.5, Math.abs(m.x1 - m.x0), Math.abs(m.y1 - m.y0)); ctx.setLineDash([]); }
  }
  // ---------------------------------------------------------------- hit tests
  rowAt(y) { const i = Math.floor((y - RH + this.scrollY) / ROW); return i >= 0 && i < this.rows.length ? i : -1; }
  hitTick(x, y) {
    const i = this.rowAt(y); if (i < 0 || x < NAME_W) return null;
    const r = this.rows[i]; const bw = Math.max(6, M.clamp(this.view.sx * 0.7, 4, 10) / 2 + 2);
    let best = null, bd = bw;
    for (const [t, pairs] of this.rowTimes(r)) { const d = Math.abs(this.X(t) - x); if (d <= bd) { bd = d; best = { row: i, t, pairs }; } }
    return best;
  }
  pairsInRect(x0, y0, x1, y1) {
    const ta = this.T(Math.min(x0, x1)), tb = this.T(Math.max(x0, x1));
    const ra = this.rowAt(Math.max(RH, Math.min(y0, y1))), rb = this.rowAt(Math.max(y0, y1));
    const lo = ra < 0 ? 0 : ra, hi = rb < 0 ? this.rows.length - 1 : rb;
    const out = []; const seen = new Set();
    for (let i = lo; i <= hi; i++) for (const c of this.rows[i]?.curves || []) for (const k of c.keys) if (k.t >= ta - 1e-6 && k.t <= tb + 1e-6 && !seen.has(k)) { seen.add(k); out.push([c, k]); }
    return out;
  }
  changed() { this.request(); App.dirty('graph'); if (this.onChange) this.onChange(); }
  // ---------------------------------------------------------------- mouse
  onDown(e) {
    const [x, y] = this.local(e); e.preventDefault();
    if (e.altKey) {
      this.drag = { type: e.button === 2 ? 'zoom' : 'pan', x0: x, y0: y, view: { ...this.view }, sy: this.scrollY, T0: this.T(x) };
    } else if (y < RH && e.button === 0 && x >= NAME_W) {
      this.drag = { type: 'scrub' }; setTimeSafe(Math.round(this.T(x)));
    } else if (x < NAME_W && e.button === 0) {
      const i = this.rowAt(y); const r = this.rows[i];
      if (r && r.type === 'node') { if (this.collapsed.has(r.id)) this.collapsed.delete(r.id); else this.collapsed.add(r.id); this.rebuild && this.rebuild(); }
      else if (r) { // select all keys of the row
        M.selApply(this.sel, r.curves.flatMap(c => c.keys.map(k => [c, k])), M.modeFromEvent(e)); this.changed();
      }
      return;
    } else if (e.button === 0) {
      const hit = this.hitTick(x, y);
      if (hit) {
        const mode = M.modeFromEvent(e);
        const allOn = hit.pairs.every(([c, k]) => M.selHas(this.sel, c, k));
        if (mode !== 'replace' || !allOn) { M.selApply(this.sel, hit.pairs, mode); this.changed(); }
        const nowOn = hit.pairs.every(([c, k]) => M.selHas(this.sel, c, k));
        this.drag = nowOn && (mode === 'replace' || mode === 'add') ? { type: 'move', x0: x, started: false } : null;
      } else {
        this.marquee = { x0: x, y0: y, x1: x, y1: y, mode: M.modeFromEvent(e) }; this.drag = { type: 'marquee' };
      }
    } else if (e.button === 1) {
      this.drag = { type: 'move', x0: x, started: false };
    } else if (e.button === 2) {
      showMenu([
        { label: 'Delete Keys', fn: () => this.deleteKeys() }, { label: 'Snap Keys', fn: () => this.snapKeys() }, '-',
        { label: 'Select All', fn: () => this.selectAll() }, { label: 'Frame All', hk: 'A', fn: () => this.frameAll() }, { label: 'Frame Selection', hk: 'F', fn: () => this.frameSelection() },
      ], e.clientX, e.clientY);
      return;
    }
    if (!this.drag) return;
    const mm = (ev) => this.onMove(ev);
    const mu = (ev) => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); this.onUp(ev); };
    addEventListener('mousemove', mm); addEventListener('mouseup', mu);
  }
  onMove(e) {
    const d = this.drag; if (!d) return; const [x, y] = this.local(e);
    if (d.type === 'scrub') { setTimeSafe(Math.round(this.T(Math.max(NAME_W, x)))); return; }
    if (d.type === 'pan') { this.view.t0 = d.view.t0 - (x - d.x0) / d.view.sx; this.scrollY = d.sy - (y - d.y0); this.clampScroll(); this.request(); return; }
    if (d.type === 'zoom') { this.view.sx = M.clamp(d.view.sx * Math.exp((x - d.x0) * 0.01), 0.02, 400); this.view.t0 = d.T0 - (d.x0 - NAME_W) / this.view.sx; this.request(); return; }
    if (d.type === 'marquee') { this.marquee.x1 = x; this.marquee.y1 = y; this.request(); return; }
    if (d.type === 'move') {
      const dx = x - d.x0;
      if (!d.started) {
        if (Math.abs(dx) < 3) return;
        const vs = M.selFilter(this.sel, this.allCurves()); if (!M.selCount(vs)) { this.drag = null; return; }
        Undo.checkpoint('Move keys'); d.started = true; d.st = M.beginKeyDrag(vs);
      }
      M.applyKeyDrag(d.st, Math.round(dx / this.view.sx), 0, { snap: true, value: false });
      animChanged(); this.request();
    }
  }
  onUp() {
    const d = this.drag; this.drag = null; if (!d) return;
    if (d.type === 'marquee') {
      const m = this.marquee; this.marquee = null;
      const small = Math.abs(m.x1 - m.x0) < 3 && Math.abs(m.y1 - m.y0) < 3;
      if (small && m.mode !== 'replace') { this.request(); return; }
      M.selApply(this.sel, small ? [] : this.pairsInRect(m.x0, m.y0, m.x1, m.y1), m.mode); this.changed(); return;
    }
    if (d.type === 'move' && d.started) { M.resolveKeyCollisions(d.st.curves, this.sel); animChanged(); this.changed(); return; }
    this.request();
  }
  onWheel(e) {
    e.preventDefault(); const [x] = this.local(e);
    if (x < NAME_W || e.shiftKey) { this.scrollY += e.deltaY; this.clampScroll(); this.request(); return; }
    const T0 = this.T(x); this.view.sx = M.clamp(this.view.sx * Math.pow(1.0015, -e.deltaY), 0.02, 400); this.view.t0 = T0 - (x - NAME_W) / this.view.sx; this.request();
  }
  // ---------------------------------------------------------------- edits
  selectAll() { M.selApply(this.sel, this.allCurves().flatMap(c => c.keys.map(k => [c, k])), 'add'); this.changed(); }
  deleteKeys() {
    const vs = M.selFilter(this.sel, this.allCurves()); if (!M.selCount(vs)) return;
    Undo.checkpoint('Delete keys');
    const { removed, emptied } = M.deleteSelectedKeys(vs); for (const c of vs.keys()) this.sel.delete(c);
    removeEmptyCurves(emptied); App.emit('echo', `cutKey -clear; // ${removed} key(s) deleted`);
    animChanged(); this.changed();
  }
  snapKeys() {
    const vs = M.selFilter(this.sel, this.allCurves()); if (!M.selCount(vs)) return;
    Undo.checkpoint('Snap keys'); for (const s of vs.values()) for (const k of s) k.t = Math.round(k.t);
    M.resolveKeyCollisions([...vs.keys()], this.sel); animChanged(); this.changed();
  }
  shiftKeys(dt) {
    const vs = M.selFilter(this.sel, this.allCurves()); if (!M.selCount(vs)) return;
    Undo.checkpoint('Move keys'); const st = M.beginKeyDrag(vs); M.applyKeyDrag(st, dt, 0, { snap: true, value: false });
    M.resolveKeyCollisions(st.curves, this.sel); animChanged(); this.changed();
  }
}

let ds = null;
export function openDopeSheet() {
  injectEditorStyle();
  const win = new FloatWin('dopeSheet', 'Dope Sheet', { w: 860, h: 380 });
  if (win.reused) { ds?.refresh(); return win; }
  win.el.classList.add('anim-ed');
  const state = { autoLoad: true, loaded: [] };
  const menuRow = h('div', { class: 'pmenubar' }), toolbar = h('div', { class: 'ptoolbar' });
  const wrap = h('div', { class: 'ge-canvas-wrap' });
  const status = h('div', { class: 'ed-status' });
  win.body.append(menuRow, toolbar, h('div', { class: 'ge' }, wrap), status);
  const dv = new DopeView(wrap, { onChange: () => updateStatus() });
  const nodes = () => state.autoLoad ? App.sel.filter(o => o && o.inca) : state.loaded.map(id => App.nodes.get(id)).filter(Boolean);
  let sig = '';
  const refresh = (frame = false) => {
    pruneSelection();
    const ns = nodes(); dv.setNodes(ns);
    const s = ns.map(n => n.inca.id + ':' + App.anim.curvesOf(n).length).join('|');
    if (s !== sig) { sig = s; frame = true; }
    if (frame) dv.frameAll();
    updateStatus();
  };
  dv.rebuild = () => refresh();
  function updateStatus() { const n = M.selCount(M.selFilter(dv.sel, dv.allCurves())); status.textContent = ''; status.append(h('span', { text: `${n} key(s) selected` }), h('span', { class: 'dim', text: 'LMB/MMB drag moves keys · Alt+MMB pan · Alt+RMB / wheel zoom · F/A frame · Del deletes' })); }

  const tb = (name, title, fn) => iconBtn(name, title, fn, 'ib small');
  const txt = (label, title, fn) => { const b = h('div', { class: 'ib small txt', title, text: label, onclick: fn }); b.addEventListener('mouseenter', () => App.help(title)); return b; };
  toolbar.append(tb('frameAll', 'Frame all (A)', () => dv.frameAll()), tb('frameSel', 'Frame selection (F)', () => dv.frameSelection()),
    txt('⇔', 'Frame playback range', () => dv.framePlayback()), txt('⊙', 'Center current time', () => dv.centerTime()), h('div', { class: 'tb-sep' }),
    txt('◀ 1', 'Move selected keys one frame earlier', () => dv.shiftKeys(-1)), txt('1 ▶', 'Move selected keys one frame later', () => dv.shiftKeys(1)), h('div', { class: 'tb-sep' }),
    tb('graphEditor', 'Open Graph Editor', () => App.ui.graphEditor && App.ui.graphEditor()));
  menuBar(menuRow, [
    { label: 'Edit', items: [{ label: 'Undo', fn: () => App.undo.undo() }, { label: 'Redo', fn: () => App.undo.redo() }, '-', { label: 'Delete', hk: 'Del', fn: () => dv.deleteKeys() }, { label: 'Select All', fn: () => dv.selectAll() }, { label: 'Select None', fn: () => { dv.sel.clear(); dv.changed(); } }] },
    { label: 'View', items: [{ label: 'Frame All', hk: 'A', fn: () => dv.frameAll() }, { label: 'Frame Selection', hk: 'F', fn: () => dv.frameSelection() }, { label: 'Frame Playback Range', fn: () => dv.framePlayback() }, { label: 'Center Current Time', fn: () => dv.centerTime() }, '-', { label: 'Expand All', fn: () => { dv.collapsed.clear(); refresh(); } }, { label: 'Collapse All', fn: () => { for (const n of nodes()) dv.collapsed.add(n.inca.id); refresh(); } }] },
    { label: 'Keys', items: [{ label: 'Snap', fn: () => dv.snapKeys() }, { label: 'Move Earlier', hk: ',', fn: () => dv.shiftKeys(-1) }, { label: 'Move Later', hk: '.', fn: () => dv.shiftKeys(1) }] },
    { label: 'List', items: [{ label: 'Auto Load Selected Objects', check: () => state.autoLoad, fn: () => { state.autoLoad = !state.autoLoad; if (!state.autoLoad) state.loaded = App.sel.map(o => o.inca.id); refresh(true); } }, { label: 'Load Selected Objects', fn: () => { state.autoLoad = false; state.loaded = App.sel.map(o => o.inca.id); refresh(true); } }] },
    { label: 'Help', items: [{ label: 'Help on Dope Sheet', fn: () => toast('Dope Sheet: click/marquee key ticks (Shift toggle, Ctrl deselect, Ctrl+Shift add) · LMB/MMB drag to retime · Del deletes · Alt+MMB pan · wheel zoom', 6000) }] },
  ]);
  setupWindowKeys(win, (e) => {
    const k = e.key.toLowerCase();
    if (e.key === 'Delete' || e.key === 'Backspace') { dv.deleteKeys(); return true; }
    if (e.ctrlKey || e.metaKey || e.altKey) return false;
    if (k === 'f') { dv.frameSelection(); return true; }
    if (k === 'a') { dv.frameAll(); return true; }
    return false;
  });
  const offs = [
    App.on('refresh', (f) => { if (!f || f.has('graph') || f.has('timeline')) refresh(); }),
    App.on('selectionChanged', () => refresh()),
    App.on('timeChanged', () => dv.request()),
    App.on('sceneLoaded', () => { keySelection.clear(); refresh(true); }),
  ];
  win.onClose = () => { offs.forEach(f => f()); ds = null; };
  win.onResize = () => { dv.resize(); dv.clampScroll(); };
  ds = { win, dv, refresh };
  requestAnimationFrame(() => { dv.resize(); refresh(true); });
  return win;
}

App.ui = App.ui || {};
App.ui.dopeSheet = openDopeSheet;
