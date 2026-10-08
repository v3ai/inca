// Inca — Graph Editor (Maya-style animation curve editor)
import { App } from '../core/app.js';
import { Undo } from '../core/undo.js';
import { CHANNELS, nodeName } from '../core/scene.js';
import { h, iconBtn, menuBar, FloatWin, numField, toast, showMenu } from './dom.js';
import * as M from './editorMath.js';

// key selection shared by the Graph Editor and the Dope Sheet: Map<AnimCurve, Set<key>>
export const keySelection = new Map();
const RH = 18; // time ruler height

export const TANGENT_TYPES = [['auto', 'Auto', 'tanAuto'], ['spline', 'Spline', 'tanSpline'], ['clamped', 'Clamped', 'tanClamped'], ['linear', 'Linear', 'tanLinear'], ['flat', 'Flat', 'tanFlat'], ['step', 'Stepped', 'tanStep'], ['plateau', 'Plateau', 'tanPlateau']];
export const INFINITY_TYPES = [['constant', 'Constant'], ['linear', 'Linear'], ['cycle', 'Cycle'], ['cycleRelative', 'Cycle with Offset'], ['oscillate', 'Oscillate']];

// ------------------------------------------------------------------ shared helpers
export function liveCurves() { return new Set(App.anim ? App.anim.curves.values() : []); }
export function pruneSelection() { M.selPrune(keySelection, liveCurves()); }
export function setTimeSafe(f) {
  if (App.setTime) App.setTime(f);
  else { App.time.current = f; App.anim?.evaluate(f); App.emit('timeChanged', f); App.dirty('timeline', 'channels', 'graph'); }
}
export function animChanged() { App.anim.evaluate(); App.dirty('timeline', 'channels', 'graph'); }
export function removeEmptyCurves(curves) { for (const c of curves) if (!c.keys.length) App.anim.curves.delete(c.node + '.' + c.attr); }
const chanOrder = (a) => { const i = CHANNELS.indexOf(a); return i < 0 ? 100 : i; };
export function sortedCurvesOf(node) { return App.anim.curvesOf(node).filter(c => c.keys.length).sort((a, b) => chanOrder(a.attr) - chanOrder(b.attr) || a.attr.localeCompare(b.attr)); }
export function stopKeys(e) { e.stopPropagation(); e.preventDefault(); }
let styleDone = false;
export function injectEditorStyle() {
  if (styleDone || document.getElementById('inca-anim-editors-style')) { styleDone = true; return; }
  styleDone = true;
  document.head.append(h('style', { id: 'inca-anim-editors-style', text: `
.fwin.anim-ed .ptoolbar { border-bottom: 1px solid var(--line); gap: 2px; }
.fwin.anim-ed .ptoolbar .ib.small.txt { width: auto; padding: 0 5px; font-size: 11px; color: #ddd; }
.fwin.anim-ed .tb-sep { width: 1px; height: 16px; background: #555; margin: 0 4px; flex: none; }
.fwin.anim-ed .tb-spacer { flex: 1; }
.fwin.anim-ed .ge-stats label { font-size: 11px; color: var(--text-dim); }
.fwin.anim-ed .ge-list { user-select: none; }
.fwin.anim-ed .ge-list .gi { cursor: default; gap: 5px; }
.fwin.anim-ed .ge-list .gi.chan { padding-left: 20px; }
.fwin.anim-ed .ge-list .gi:hover:not(.sel) { background: #444; }
.fwin.anim-ed .ge-list .gi .sw { width: 9px; height: 9px; flex: none; border: 1px solid #222; }
.fwin.anim-ed .ge-list .gi .tw { width: 10px; flex: none; color: #aaa; font-size: 9px; }
.fwin.anim-ed .ge-list .empty { padding: 8px; color: var(--text-dim); font-size: 11px; white-space: normal; }
.fwin.anim-ed .ge-canvas-wrap { outline: none; overflow: hidden; }
.fwin.anim-ed .ed-status { height: 18px; display: flex; align-items: center; padding: 0 6px; font-size: 11px; color: #aaa; border-top: 1px solid var(--line); flex: none; gap: 14px; }
` }));
}
// Maya window focus routing: keys handled inside our window don't reach global hotkeys
export function setupWindowKeys(win, handler) {
  win.el.tabIndex = -1;
  win.el.addEventListener('mousedown', (e) => { if (!e.target.closest('input,textarea,select')) setTimeout(() => { if (!win.el.contains(document.activeElement) || document.activeElement === document.body) win.el.focus({ preventScroll: true }); }, 0); });
  win.el.addEventListener('keydown', (e) => { if (e.target.closest && e.target.closest('input,textarea,select')) return; if (handler(e)) stopKeys(e); });
}

// ------------------------------------------------------------------ GraphView: canvas curve editor
export class GraphView {
  constructor(wrap, { sel = keySelection, onSelChange = null, onEdit = null } = {}) {
    this.wrap = wrap; this.sel = sel; this.onSelChange = onSelChange; this.onEdit = onEdit;
    this.cv = h('canvas'); wrap.append(this.cv);
    this.view = { t0: -5, sx: 6, vTop: 12, sy: 12 };
    this.curves = []; this.W = 100; this.H = 100;
    this.showInfinity = false; this.showTangents = true; this.snap = true; this.tool = 'select';
    this.drag = null; this.marquee = null; this._raf = 0;
    this.cv.addEventListener('mousedown', (e) => this.onDown(e));
    this.cv.addEventListener('wheel', (e) => this.onWheel(e), { passive: false });
    this.cv.addEventListener('contextmenu', (e) => e.preventDefault());
    this.cv.addEventListener('mousemove', (e) => { if (!this.drag) this.hover(e); });
  }
  // coordinate mapping
  X(t) { return (t - this.view.t0) * this.view.sx; }
  T(x) { return x / this.view.sx + this.view.t0; }
  Y(v) { return (this.view.vTop - v) * this.view.sy; }
  V(y) { return this.view.vTop - y / this.view.sy; }
  local(e) { const r = this.cv.getBoundingClientRect(); return [e.clientX - r.left, e.clientY - r.top]; }
  setCurves(list) { this.curves = list; this.request(); }
  selectedPairs(curves = this.curves) { const out = []; for (const c of curves) { const s = this.sel.get(c); if (s) for (const k of s) out.push([c, k]); } return out; }
  visibleSel() { return M.selFilter(this.sel, this.curves); }
  resize() { const r = this.wrap.getBoundingClientRect(); this.W = Math.max(10, r.width); this.H = Math.max(10, r.height); this.draw(); }
  request() { if (!this._raf) this._raf = requestAnimationFrame(() => { this._raf = 0; this.draw(); }); }
  // ---------------------------------------------------------------- framing
  frameBounds(b) {
    if (!b) { const t = App.time; b = { t0: t.start, t1: t.end, v0: -1, v1: 1 }; }
    let { t0, t1, v0, v1 } = b;
    if (t1 - t0 < 1e-6) { t0 -= 5; t1 += 5; }
    if (v1 - v0 < 1e-6) { v0 -= 1; v1 += 1; }
    const L = 44, R = 16, TOP = RH + 14, BOT = 14;
    const sx = (this.W - L - R) / (t1 - t0), sy = (this.H - TOP - BOT) / (v1 - v0);
    this.view = { sx: Math.max(1e-4, sx), sy: Math.max(1e-6, sy), t0: t0 - L / sx, vTop: v1 + TOP / sy };
    this.request();
  }
  curveBounds(curves) {
    const pairs = []; for (const c of curves) for (const k of c.keys) pairs.push([c, k]);
    const b = M.keyBounds(pairs);
    if (b) for (const c of curves) { // include curve overshoot between keys
      if (c.keys.length < 2) continue;
      const n = Math.min(400, Math.max(20, (c.keys[c.keys.length - 1].t - c.keys[0].t) * 2));
      for (let i = 0; i <= n; i++) { const v = c.eval(c.keys[0].t + (c.keys[c.keys.length - 1].t - c.keys[0].t) * i / n); if (isFinite(v)) { b.v0 = Math.min(b.v0, v); b.v1 = Math.max(b.v1, v); } }
    }
    return b;
  }
  frameAll() { this.frameBounds(this.curveBounds(this.curves)); }
  frameSelection() { const p = this.selectedPairs(); if (!p.length) return this.frameAll(); this.frameBounds(M.keyBounds(p)); }
  framePlayback() { const b = this.curveBounds(this.curves) || { v0: -1, v1: 1 }; this.frameBounds({ t0: App.time.start, t1: App.time.end, v0: b.v0, v1: b.v1 }); }
  centerTime() { const t = App.time.current; this.view.t0 = t - this.W / 2 / this.view.sx; this.request(); }
  // ---------------------------------------------------------------- drawing
  draw() {
    const ctx = this.ctx = M.fitCanvas(this.cv, this.W, this.H); const { W, H } = this; const v = this.view;
    ctx.fillStyle = '#393939'; ctx.fillRect(0, 0, W, H);
    // playback range tint
    const pa = Math.max(0, this.X(App.time.start)), pb = Math.min(W, this.X(App.time.end));
    if (pb > pa) { ctx.fillStyle = '#3d3d3d'; ctx.fillRect(pa, RH, pb - pa, H - RH); }
    // grid
    const tStep = Math.max(1e-3, M.niceStep(60 / v.sx)), vStep = M.niceStep(32 / v.sy);
    ctx.lineWidth = 1; ctx.strokeStyle = '#323232'; ctx.beginPath();
    for (let t = Math.floor(this.T(0) / tStep) * tStep; t <= this.T(W); t += tStep) { const x = Math.round(this.X(t)) + 0.5; ctx.moveTo(x, RH); ctx.lineTo(x, H); }
    for (let val = Math.floor(this.V(H) / vStep) * vStep; val <= this.V(RH); val += vStep) { const y = Math.round(this.Y(val)) + 0.5; ctx.moveTo(0, y); ctx.lineTo(W, y); }
    ctx.stroke();
    // zero axes
    ctx.strokeStyle = '#262626'; ctx.beginPath();
    const y0 = Math.round(this.Y(0)) + 0.5; if (y0 > RH && y0 < H) { ctx.moveTo(0, y0); ctx.lineTo(W, y0); }
    const x0 = Math.round(this.X(0)) + 0.5; if (x0 > 0 && x0 < W) { ctx.moveTo(x0, RH); ctx.lineTo(x0, H); }
    ctx.stroke();
    // value labels
    ctx.font = '10px sans-serif'; ctx.fillStyle = '#a8a8a8'; ctx.textBaseline = 'bottom'; ctx.textAlign = 'left';
    for (let val = Math.floor(this.V(H) / vStep) * vStep; val <= this.V(RH); val += vStep) { const y = this.Y(val); if (y < RH + 12) continue; ctx.fillText(M.fmtNum(val, vStep), 3, Math.round(y) - 1); }
    // curves
    for (const c of this.curves) this.drawCurve(ctx, c);
    // keys + tangents
    for (const c of this.curves) this.drawKeys(ctx, c);
    // current time
    const tx = this.X(App.time.current);
    M.drawTimeMarker(ctx, { x: tx, RH, H });
    M.drawTimeRuler(ctx, { W, RH, X: (t) => this.X(t), T: (x) => this.T(x), sx: v.sx, range: [App.time.start, App.time.end] });
    if (tx > -20 && tx < W + 20) M.drawTimeBox(ctx, { x: tx, RH, frame: App.time.current });
    if (!this.curves.length) { ctx.fillStyle = '#8a8a8a'; ctx.font = '12px sans-serif'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle'; ctx.fillText('Select animated objects to display their curves', W / 2, (H + RH) / 2); ctx.textAlign = 'left'; }
    // marquee
    if (this.marquee) { const m = this.marquee; ctx.setLineDash([3, 3]); ctx.strokeStyle = '#eee'; ctx.strokeRect(Math.min(m.x0, m.x1) + 0.5, Math.min(m.y0, m.y1) + 0.5, Math.abs(m.x1 - m.x0), Math.abs(m.y1 - m.y0)); ctx.setLineDash([]); }
  }
  drawCurve(ctx, c) {
    const K = c.keys; if (!K.length) return;
    const W = this.W; const tl = this.T(-2), tr = this.T(W + 2);
    const k0 = K[0].t, k1 = K[K.length - 1].t;
    const col = M.attrColor(c.attr);
    const hasSel = this.sel.has(c);
    ctx.strokeStyle = col; ctx.lineWidth = hasSel ? 1.6 : 1.2;
    // body
    ctx.beginPath(); let started = false;
    const pt = (t, val) => { const x = this.X(t), y = this.Y(val); if (!started) { ctx.moveTo(x, y); started = true; } else ctx.lineTo(x, y); };
    if (K.length === 1) { pt(Math.max(tl, k0 - 1e6), K[0].v); pt(Math.min(tr, k0 + 1e6), K[0].v); }
    for (let i = 0; i < K.length - 1; i++) {
      const a = K[i], b = K[i + 1]; if (b.t < tl || a.t > tr) { started = false; continue; }
      if (a.ot === 'step') { pt(a.t, a.v); pt(b.t, a.v); pt(b.t, b.v); continue; }
      const px = (b.t - a.t) * this.view.sx; const n = Math.max(2, Math.min(400, Math.ceil(px / 3)));
      const s0 = Math.max(0, Math.floor((tl - a.t) / (b.t - a.t) * n)), s1 = Math.min(n, Math.ceil((tr - a.t) / (b.t - a.t) * n));
      for (let j = s0; j <= s1; j++) { const t = a.t + (b.t - a.t) * j / n; pt(t, c.evalRaw(t)); }
    }
    ctx.stroke();
    // infinity
    if (K.length > 1 && (this.showInfinity || c.pre !== 'constant' || c.post !== 'constant')) {
      ctx.setLineDash([5, 4]); ctx.lineWidth = 1; ctx.globalAlpha = 0.85;
      const seg = (ta, tb) => { if (tb <= ta) return; ctx.beginPath(); const n = Math.max(2, Math.min(1500, Math.ceil((tb - ta) * this.view.sx / 2))); for (let j = 0; j <= n; j++) { const t = ta + (tb - ta) * j / n; const x = this.X(t), y = this.Y(c.eval(t)); if (j) ctx.lineTo(x, y); else ctx.moveTo(x, y); } ctx.stroke(); };
      if (this.showInfinity || c.pre !== 'constant') seg(tl, Math.min(k0, tr));
      if (this.showInfinity || c.post !== 'constant') seg(Math.max(k1, tl), tr);
      ctx.setLineDash([]); ctx.globalAlpha = 1;
    }
  }
  handles(c, i) {
    const k = c.keys[i]; const [si, so] = c.slopes(i);
    const x = this.X(k.t), y = this.Y(k.v);
    const a = M.handleVec(si, this.view.sx, this.view.sy), b = M.handleVec(so, this.view.sx, this.view.sy);
    return { x, y, in: [x - a[0], y - a[1]], out: [x + b[0], y + b[1]] };
  }
  drawKeys(ctx, c) {
    const s = this.sel.get(c);
    const W = this.W, H = this.H;
    if (s && this.showTangents) {
      ctx.lineWidth = 1;
      c.keys.forEach((k, i) => {
        if (!s.has(k)) return; const hd = this.handles(c, i);
        if (hd.x < -60 || hd.x > W + 60) return;
        const showIn = i > 0 || this.showInfinity, showOut = (i < c.keys.length - 1 || this.showInfinity) && k.ot !== 'step';
        ctx.strokeStyle = '#b88a4c'; ctx.beginPath();
        if (showIn) { ctx.moveTo(hd.in[0], hd.in[1]); ctx.lineTo(hd.x, hd.y); }
        if (showOut) { ctx.moveTo(hd.x, hd.y); ctx.lineTo(hd.out[0], hd.out[1]); }
        ctx.stroke();
        const dot = (p, active) => { ctx.fillStyle = active ? '#ffef4a' : '#6b4a2a'; ctx.strokeStyle = '#d9a35f'; ctx.beginPath(); ctx.arc(p[0], p[1], 3, 0, 7); ctx.fill(); ctx.stroke(); };
        const act = this.drag && this.drag.type === 'handle' && this.drag.k === k ? this.drag.side : null;
        if (showIn) dot(hd.in, act === 'in'); if (showOut) dot(hd.out, act === 'out');
      });
    }
    for (const k of c.keys) {
      const x = this.X(k.t), y = this.Y(k.v); if (x < -5 || x > W + 5 || y < RH - 5 || y > H + 5) continue;
      const on = s && s.has(k);
      ctx.fillStyle = on ? '#ffef1f' : '#0b0b0b'; ctx.fillRect(Math.round(x) - 2.5, Math.round(y) - 2.5, 5, 5);
      if (!on) { ctx.strokeStyle = 'rgba(255,255,255,0.18)'; ctx.lineWidth = 1; ctx.strokeRect(Math.round(x) - 3, Math.round(y) - 3, 6, 6); }
    }
  }
  // ---------------------------------------------------------------- hit tests
  hitKey(x, y, r = 6) {
    let best = null, bd = r * r;
    for (const c of this.curves) for (const k of c.keys) { const dx = this.X(k.t) - x, dy = this.Y(k.v) - y, d = dx * dx + dy * dy; if (d <= bd) { bd = d; best = { c, k }; } }
    return best;
  }
  hitHandle(x, y, r = 6) {
    if (!this.showTangents) return null;
    for (const c of this.curves) { const s = this.sel.get(c); if (!s) continue; for (let i = 0; i < c.keys.length; i++) { const k = c.keys[i]; if (!s.has(k)) continue; const hd = this.handles(c, i); for (const side of ['in', 'out']) { if (side === 'out' && k.ot === 'step') continue; if (side === 'in' && i === 0 && !this.showInfinity) continue; if (side === 'out' && i === c.keys.length - 1 && !this.showInfinity) continue; const p = hd[side]; if ((p[0] - x) ** 2 + (p[1] - y) ** 2 <= r * r) return { c, k, side }; } } }
    return null;
  }
  keysInRect(x0, y0, x1, y1) {
    const a = Math.min(x0, x1), b = Math.max(x0, x1), c0 = Math.min(y0, y1), d = Math.max(y0, y1); const out = [];
    for (const c of this.curves) for (const k of c.keys) { const x = this.X(k.t), y = this.Y(k.v); if (x >= a && x <= b && y >= c0 && y <= d) out.push([c, k]); }
    return out;
  }
  hover(e) {
    const [x, y] = this.local(e);
    this.cv.style.cursor = y < RH ? 'col-resize' : (this.hitHandle(x, y) || this.hitKey(x, y)) ? 'pointer' : 'default';
  }
  selChanged() { this.request(); App.dirty('graph'); if (this.onSelChange) this.onSelChange(); }
  edited(final) { animChanged(); this.request(); if (this.onEdit) this.onEdit(final); }
  // ---------------------------------------------------------------- mouse
  onDown(e) {
    const [x, y] = this.local(e); e.preventDefault();
    const mods = { shift: e.shiftKey, ctrl: e.ctrlKey || e.metaKey };
    if (e.altKey) {
      this.drag = { type: e.button === 2 ? 'zoom' : 'pan', x0: x, y0: y, view: { ...this.view }, axis: e.shiftKey ? null : 'both', T0: this.T(x), V0: this.V(y) };
    } else if (y < RH && e.button === 0) {
      this.drag = { type: 'scrub' }; setTimeSafe(Math.round(this.T(x)));
    } else if (e.button === 0) {
      const hh = this.hitHandle(x, y);
      if (hh) { this.drag = { type: 'handle', ...hh, x0: x, y0: y, started: false }; }
      else {
        const hk = this.hitKey(x, y);
        if (hk) {
          const mode = M.modeFromEvent(e);
          if (mode !== 'replace' || !M.selHas(this.sel, hk.c, hk.k)) { M.selApply(this.sel, [[hk.c, hk.k]], mode); this.selChanged(); }
          this.drag = (mode === 'replace' || mode === 'add') && M.selHas(this.sel, hk.c, hk.k) ? { type: 'move', x0: x, y0: y, started: false, axis: null } : null;
        } else if (this.tool === 'move' && !mods.shift && !mods.ctrl && M.selCount(this.visibleSel())) {
          this.drag = { type: 'move', x0: x, y0: y, started: false, axis: null, clickClears: true };
        } else {
          this.marquee = { x0: x, y0: y, x1: x, y1: y, mode: M.modeFromEvent(e) }; this.drag = { type: 'marquee' };
        }
      }
    } else if (e.button === 1) {
      this.drag = { type: 'move', x0: x, y0: y, started: false, axis: null };
    } else if (e.button === 2) {
      this.contextMenu(e); return;
    }
    if (!this.drag) return;
    const mm = (ev) => this.onMove(ev);
    const mu = (ev) => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); this.onUp(ev); };
    addEventListener('mousemove', mm); addEventListener('mouseup', mu);
  }
  onMove(e) {
    const d = this.drag; if (!d) return; const [x, y] = this.local(e);
    if (d.type === 'scrub') { setTimeSafe(Math.round(this.T(x))); return; }
    if (d.type === 'pan') {
      const dx = x - d.x0, dy = y - d.y0; let ax = d.axis;
      if (!ax && Math.hypot(dx, dy) > 4) ax = d.axis = Math.abs(dx) > Math.abs(dy) ? 'x' : 'y';
      this.view.t0 = d.view.t0 - (ax === 'y' ? 0 : dx / d.view.sx);
      this.view.vTop = d.view.vTop + (ax === 'x' ? 0 : dy / d.view.sy);
      this.request(); return;
    }
    if (d.type === 'zoom') {
      const dx = x - d.x0, dy = y - d.y0; let ax = d.axis;
      if (!ax && Math.hypot(dx, dy) > 4) ax = d.axis = Math.abs(dx) > Math.abs(dy) ? 'x' : 'y';
      const fx = ax === 'y' ? 1 : Math.exp(dx * 0.01), fy = ax === 'x' ? 1 : Math.exp(-dy * 0.01);
      this.view.sx = d.view.sx * fx; this.view.sy = d.view.sy * fy;
      this.view.t0 = d.T0 - d.x0 / this.view.sx; this.view.vTop = d.V0 + d.y0 / this.view.sy;
      this.request(); return;
    }
    if (d.type === 'marquee') { this.marquee.x1 = x; this.marquee.y1 = y; this.request(); return; }
    if (d.type === 'move') {
      const dx = x - d.x0, dy = y - d.y0;
      if (!d.started) {
        if (Math.hypot(dx, dy) < 3) return;
        const vs = this.visibleSel(); if (!M.selCount(vs)) { this.drag = null; return; }
        Undo.checkpoint('Move keys'); d.started = true; d.st = M.beginKeyDrag(vs);
      }
      if (e.shiftKey && !d.axis && Math.hypot(dx, dy) > 4) d.axis = Math.abs(dx) > Math.abs(dy) ? 'time' : 'value';
      if (!e.shiftKey) d.axis = null;
      M.applyKeyDrag(d.st, dx / this.view.sx, -dy / this.view.sy, { snap: this.snap, time: d.axis !== 'value', value: d.axis !== 'time' });
      this.edited(false); return;
    }
    if (d.type === 'handle') {
      if (!d.started) { if (Math.hypot(x - d.x0, y - d.y0) < 2) return; Undo.checkpoint('Tangent'); d.started = true; }
      const kx = this.X(d.k.t), ky = this.Y(d.k.v);
      const s = M.slopeFromHandle(x - kx, y - ky, this.view.sx, this.view.sy, d.side);
      const k = d.k;
      if (k.broken) { if (d.side === 'in') { k.it = 'fixed'; k.is = s; } else { k.ot = 'fixed'; k.os = s; } }
      else { k.it = 'fixed'; k.is = s; if (k.ot !== 'step') { k.ot = 'fixed'; k.os = s; } }
      this.edited(false);
    }
  }
  onUp(e) {
    const d = this.drag; this.drag = null; if (!d) return;
    if (d.type === 'marquee') {
      const m = this.marquee; this.marquee = null;
      const small = Math.abs(m.x1 - m.x0) < 3 && Math.abs(m.y1 - m.y0) < 3;
      const pairs = small ? [] : this.keysInRect(m.x0, m.y0, m.x1, m.y1);
      if (small && m.mode !== 'replace') { this.request(); return; }
      M.selApply(this.sel, pairs, m.mode); this.selChanged(); return;
    }
    if (d.type === 'move') {
      if (!d.started) { if (d.clickClears) { this.sel.clear(); this.selChanged(); } return; }
      M.resolveKeyCollisions(d.st.curves, this.sel); this.edited(true); this.selChanged(); return;
    }
    if (d.type === 'handle' && d.started) { this.edited(true); return; }
    this.request();
  }
  onWheel(e) {
    e.preventDefault(); const [x, y] = this.local(e);
    const f = Math.pow(1.0015, -e.deltaY);
    const T0 = this.T(x), V0 = this.V(y);
    if (!e.ctrlKey) this.view.sx *= f;
    if (!e.shiftKey) this.view.sy *= f;
    this.view.t0 = T0 - x / this.view.sx; this.view.vTop = V0 + y / this.view.sy;
    this.request();
  }
  contextMenu(e) {
    const items = [
      ...TANGENT_TYPES.map(([t, l]) => ({ label: l, fn: () => this.setTangents(t) })), '-',
      { label: 'Break Tangents', fn: () => this.breakTangents(true) }, { label: 'Unify Tangents', fn: () => this.breakTangents(false) }, '-',
      { label: 'Insert Key', fn: () => this.insertKey() }, { label: 'Delete Keys', fn: () => this.deleteKeys(), enabled: () => M.selCount(this.visibleSel()) > 0 }, '-',
      { label: 'Frame All', hk: 'A', fn: () => this.frameAll() }, { label: 'Frame Selection', hk: 'F', fn: () => this.frameSelection() },
    ];
    showMenu(items, e.clientX, e.clientY);
  }
  // ---------------------------------------------------------------- edits
  selectAll() { M.selApply(this.sel, this.curves.flatMap(c => c.keys.map(k => [c, k])), 'add'); this.selChanged(); }
  selectNone() { this.sel.clear(); this.selChanged(); }
  deleteKeys() {
    const vs = this.visibleSel(); if (!M.selCount(vs)) return;
    Undo.checkpoint('Delete keys');
    const { removed, emptied } = M.deleteSelectedKeys(vs);
    for (const c of vs.keys()) this.sel.delete(c);
    removeEmptyCurves(emptied);
    App.emit('echo', `cutKey -clear; // ${removed} key(s) deleted`);
    this.edited(true); this.selChanged();
  }
  setTangents(type, side = 'both') {
    const pairs = this.selectedPairs(); if (!pairs.length) { App.help('Select keys to change their tangents'); return; }
    Undo.checkpoint('Tangents ' + type);
    for (const [, k] of pairs) {
      if (side !== 'out') k.it = type;
      if (side !== 'in') k.ot = type;
    }
    App.emit('echo', `keyTangent -itt ${type} -ott ${type};`);
    this.edited(true);
  }
  breakTangents(on) {
    const pairs = this.selectedPairs(); if (!pairs.length) return;
    Undo.checkpoint(on ? 'Break tangents' : 'Unify tangents');
    for (const [c, k] of pairs) {
      if (on) k.broken = true;
      else { delete k.broken; if (k.it === 'fixed' || k.ot === 'fixed') { const i = c.keys.indexOf(k); const [si, so] = c.slopes(i); const s = (si + so) / 2; k.it = 'fixed'; k.is = s; if (k.ot !== 'step') { k.ot = 'fixed'; k.os = s; } } }
    }
    this.edited(true);
  }
  insertKey(t = App.time.current) {
    const targets = this.curves; if (!targets.length) { App.help('Select a curve to insert a key on'); return; }
    Undo.checkpoint('Insert key');
    const pairs = [];
    for (const c of targets) { const v = c.eval(t); const k = c.setKey(t, v); pairs.push([c, k]); }
    M.selApply(this.sel, pairs, 'replace');
    App.emit('echo', `setKeyframe -insert -time ${t};`);
    this.edited(true); this.selChanged();
  }
  snapKeys() {
    const vs = this.visibleSel(); if (!M.selCount(vs)) return;
    Undo.checkpoint('Snap keys');
    for (const [, s] of vs) for (const k of s) k.t = Math.round(k.t);
    M.resolveKeyCollisions([...vs.keys()], this.sel); this.edited(true); this.selChanged();
  }
  setInfinity(which, mode) {
    const cs = this.curves.filter(c => this.sel.has(c)); const targets = cs.length ? cs : this.curves;
    if (!targets.length) return;
    Undo.checkpoint('Infinity');
    for (const c of targets) { if (which !== 'post') c.pre = mode; if (which !== 'pre') c.post = mode; }
    if (mode !== 'constant') this.showInfinity = true;
    App.emit('echo', `setInfinity -${which === 'pre' ? 'pri' : 'poi'} ${mode};`);
    this.edited(true);
  }
  setSelectedKeys({ t = null, v = null }) {
    const vs = this.visibleSel(); if (!M.selCount(vs)) return;
    for (const [c, s] of vs) { for (const k of s) { if (t !== null) k.t = t; if (v !== null) k.v = v; } M.sortKeys(c); }
    if (t !== null) M.resolveKeyCollisions([...vs.keys()], this.sel);
  }
  // clipboard
  copyKeys() {
    const vs = this.visibleSel(); if (!M.selCount(vs)) return false;
    let t0 = Infinity; for (const s of vs.values()) for (const k of s) t0 = Math.min(t0, k.t);
    GraphView.clip = [...vs].map(([c, s]) => ({ attr: c.attr, keys: [...s].map(k => ({ ...k, t: k.t - t0 })) }));
    App.help(`Copied ${M.selCount(vs)} key(s)`); return true;
  }
  pasteKeys(t = App.time.current) {
    const clip = GraphView.clip; if (!clip || !this.curves.length) return;
    Undo.checkpoint('Paste keys'); const pairs = [];
    const selCurves = this.curves.filter(c => this.sel.has(c)); const targets = selCurves.length ? selCurves : this.curves;
    targets.forEach((c, i) => {
      const src = clip.find(x => x.attr === c.attr) || (clip.length === 1 ? clip[0] : clip[i % clip.length]);
      for (const k of src.keys) { const nk = c.setKey(k.t + t, k.v); nk.it = k.it; nk.ot = k.ot; nk.is = k.is; nk.os = k.os; if (k.broken) nk.broken = true; pairs.push([c, nk]); }
    });
    M.selApply(this.sel, pairs, 'replace'); this.edited(true); this.selChanged();
  }
}
GraphView.clip = null;

// ------------------------------------------------------------------ Graph Editor window
let editor = null;
export function openGraphEditor() {
  injectEditorStyle();
  const win = new FloatWin('graphEditor', 'Graph Editor', { w: 900, h: 480 });
  if (win.reused) { editor?.refreshAll(); return win; }
  win.el.classList.add('anim-ed');
  const state = { listSel: new Set(), lastClick: -1, autoLoad: true, loaded: [], filter: { translate: true, rotate: true, scale: true, other: true }, autoFrame: true, sig: '' };
  const menuRow = h('div', { class: 'pmenubar' });
  const toolbar = h('div', { class: 'ptoolbar' });
  const list = h('div', { class: 'ge-list' });
  const wrap = h('div', { class: 'ge-canvas-wrap' });
  const status = h('div', { class: 'ed-status' });
  win.body.append(menuRow, toolbar, h('div', { class: 'ge' }, list, wrap), status);
  const gv = new GraphView(wrap, { onSelChange: () => updateStats(), onEdit: () => updateStats() });

  // ---- outliner
  const nodes = () => {
    if (state.autoLoad) return App.sel.filter(o => o && o.inca);
    return state.loaded.map(id => App.nodes.get(id)).filter(Boolean);
  };
  const passFilter = (attr) => { const f = state.filter; if (attr.startsWith('translate')) return f.translate; if (attr.startsWith('rotate') && attr.length === 7) return f.rotate; if (attr.startsWith('scale') && attr.length === 6) return f.scale; return f.other; };
  const rows = () => {
    const out = [];
    for (const n of nodes()) {
      const cs = sortedCurvesOf(n).filter(c => passFilter(c.attr));
      out.push({ key: 'n:' + n.inca.id, node: n, curves: cs, label: nodeName(n) });
      for (const c of cs) out.push({ key: 'c:' + c.node + '.' + c.attr, node: n, curves: [c], label: c.attr, curve: c });
    }
    return out;
  };
  let curRows = [];
  const visibleCurves = () => {
    if (!state.listSel.size) return curRows.filter(r => r.curve).map(r => r.curve);
    const s = new Set();
    for (const r of curRows) if (state.listSel.has(r.key)) for (const c of r.curves) s.add(c);
    return [...s];
  };
  const renderList = () => {
    const keys = new Set(curRows.map(r => r.key));
    for (const k of [...state.listSel]) if (!keys.has(k)) state.listSel.delete(k);
    list.innerHTML = '';
    if (!curRows.length) list.append(h('div', { class: 'empty', text: 'Select objects with animation to list their curves.' }));
    curRows.forEach((r, i) => {
      const el = h('div', { class: 'gi ' + (r.curve ? 'chan' : 'node') + (state.listSel.has(r.key) ? ' sel' : ''), title: r.curve ? App.anim.curveName(r.curve) : r.label },
        r.curve ? h('span', { class: 'sw', style: { background: M.attrColor(r.curve.attr) } }) : h('span', { class: 'tw', text: '▼' }),
        h('span', { text: r.curve ? niceAttr(r.label) : r.label }));
      el.addEventListener('mousedown', (e) => {
        e.preventDefault();
        if (e.shiftKey && state.lastClick >= 0) { const [a, b] = [Math.min(state.lastClick, i), Math.max(state.lastClick, i)]; if (!e.ctrlKey) state.listSel.clear(); for (let j = a; j <= b; j++) state.listSel.add(curRows[j].key); }
        else if (e.ctrlKey || e.metaKey) { if (state.listSel.has(r.key)) state.listSel.delete(r.key); else state.listSel.add(r.key); state.lastClick = i; }
        else { state.listSel.clear(); state.listSel.add(r.key); state.lastClick = i; }
        renderList(); applyCurves(true);
      });
      el.addEventListener('dblclick', () => { gv.frameAll(); });
      list.append(el);
    });
  };
  const applyCurves = (frame = false) => {
    gv.setCurves(visibleCurves());
    if (frame && state.autoFrame) gv.frameAll();
    updateStats();
  };
  const signature = () => nodes().map(n => n.inca.id + ':' + sortedCurvesOf(n).filter(c => passFilter(c.attr)).map(c => c.attr).join(',')).join('|');
  const refreshAll = (forceFrame = false) => {
    pruneSelection();
    curRows = rows();
    const sig = signature();
    const changed = sig !== state.sig; state.sig = sig;
    if (changed) renderList();
    applyCurves(forceFrame || (changed && state.autoFrame));
  };

  // ---- stats
  const fT = numField(0, (n, live) => statEdit({ t: Math.round(n * 1000) / 1000 }, live), { step: 1, onStart: () => { statCk = true; Undo.checkpoint('Edit keys'); } });
  const fV = numField(0, (n, live) => statEdit({ v: n }, live), { step: 0.05, onStart: () => { statCk = true; Undo.checkpoint('Edit keys'); } });
  let statCk = false;
  const statEdit = (o, live) => {
    if (!M.selCount(gv.visibleSel())) return;
    if (!statCk) Undo.checkpoint('Edit keys');
    gv.setSelectedKeys(o); animChanged(); gv.request();
    if (!live) { statCk = false; updateStats(); }
  };
  function updateStats() {
    const pairs = gv.selectedPairs();
    const same = (f) => pairs.length > 0 && pairs.every(p => Math.abs(f(p[1]) - f(pairs[0][1])) < 1e-6);
    const put = (fld, ok, val) => { if (document.activeElement === fld) return; if (ok) fld.set(val); else { fld.dataset.v = ''; fld.value = ''; } };
    put(fT, same(k => k.t), pairs[0]?.[1].t); put(fV, same(k => k.v), pairs[0]?.[1].v);
    fT.disabled = fV.disabled = !pairs.length;
    const tan = pairs.length ? [...new Set(pairs.map(p => p[1].it + '/' + p[1].ot))] : [];
    status.textContent = '';
    status.append(...[h('span', { text: `${gv.curves.length} curve(s)` }), h('span', { text: `${pairs.length} key(s) selected` }),
      tan.length === 1 ? h('span', { text: 'Tangents: ' + tan[0] }) : null, h('span', { class: 'dim', text: 'Alt+MMB pan · Alt+RMB zoom · MMB drag move keys · F/A frame' })].filter(Boolean));
  }

  // ---- toolbar
  const tb = (name, title, fn) => iconBtn(name, title, fn, 'ib small');
  const txt = (label, title, fn) => { const b = h('div', { class: 'ib small txt', title, text: label, onclick: fn }); b.addEventListener('mouseenter', () => App.help(title)); return b; };
  const sep = () => h('div', { class: 'tb-sep' });
  const toolBtns = { select: tb('select', 'Select keys (Q)', () => setTool('select')), move: tb('move', 'Move keys tool (W)', () => setTool('move')) };
  const setTool = (t) => { gv.tool = t; for (const [k, b] of Object.entries(toolBtns)) b.classList.toggle('on', k === t); };
  const snapBtn = tb('snapGrid', 'Snap keys to whole frames', () => { gv.snap = !gv.snap; snapBtn.classList.toggle('on', gv.snap); });
  const infBtn = txt('∞', 'View infinity', () => { gv.showInfinity = !gv.showInfinity; infBtn.classList.toggle('on', gv.showInfinity); gv.request(); });
  toolbar.append(toolBtns.select, toolBtns.move, sep(),
    h('div', { class: 'ge-stats' }, h('label', { text: 'Stats' }), fT, fV), sep(),
    ...TANGENT_TYPES.map(([t, l, ic]) => tb(ic, l + ' tangents', () => gv.setTangents(t))), sep(),
    txt('Break', 'Break tangents', () => gv.breakTangents(true)), txt('Unify', 'Unify tangents', () => gv.breakTangents(false)), sep(),
    snapBtn, infBtn, h('div', { class: 'tb-spacer' }),
    tb('frameAll', 'Frame all (A)', () => gv.frameAll()), tb('frameSel', 'Frame selection (F)', () => gv.frameSelection()),
    txt('⇔', 'Frame playback range', () => gv.framePlayback()), txt('⊙', 'Center current time', () => gv.centerTime()));
  setTool('select'); snapBtn.classList.add('on');

  // ---- menus
  const chk = (fnGet, fnSet) => ({ check: fnGet, fn: fnSet });
  const infItems = (which) => INFINITY_TYPES.map(([m, l]) => ({ label: l, fn: () => gv.setInfinity(which, m), check: () => { const c = gv.curves.find(c => gv.sel.has(c)) || gv.curves[0]; return !!c && c[which] === m; } }));
  const tanItems = (side) => TANGENT_TYPES.map(([t, l]) => ({ label: l, fn: () => gv.setTangents(t, side) }));
  menuBar(menuRow, [
    { label: 'Edit', items: [
      { label: 'Undo', hk: 'Z', fn: () => App.undo.undo() }, { label: 'Redo', hk: 'Shift+Z', fn: () => App.undo.redo() }, '-',
      { label: 'Cut', hk: 'Ctrl+X', fn: () => { if (gv.copyKeys()) gv.deleteKeys(); } }, { label: 'Copy', hk: 'Ctrl+C', fn: () => gv.copyKeys() },
      { label: 'Paste', hk: 'Ctrl+V', fn: () => gv.pasteKeys(), enabled: () => !!GraphView.clip }, { label: 'Delete', hk: 'Del', fn: () => gv.deleteKeys() }, '-',
      { label: 'Select All Keys', fn: () => gv.selectAll() }, { label: 'Select None', fn: () => gv.selectNone() }] },
    { label: 'View', items: [
      { label: 'Frame All', hk: 'A', fn: () => gv.frameAll() }, { label: 'Frame Selection', hk: 'F', fn: () => gv.frameSelection() },
      { label: 'Frame Playback Range', fn: () => gv.framePlayback() }, { label: 'Center Current Time', fn: () => gv.centerTime() }, '-',
      { label: 'Infinity', ...chk(() => gv.showInfinity, () => { gv.showInfinity = !gv.showInfinity; infBtn.classList.toggle('on', gv.showInfinity); gv.request(); }) },
      { label: 'Show Tangents', ...chk(() => gv.showTangents, () => { gv.showTangents = !gv.showTangents; gv.request(); }) },
      { label: 'Auto Frame', ...chk(() => state.autoFrame, () => { state.autoFrame = !state.autoFrame; }) }] },
    { label: 'Select', items: [
      { label: 'All Keys', fn: () => gv.selectAll() }, { label: 'Deselect All', fn: () => gv.selectNone() },
      { label: 'Keys at Current Time', fn: () => { M.selApply(gv.sel, gv.curves.flatMap(c => c.keys.filter(k => Math.abs(k.t - App.time.current) < 1e-4).map(k => [c, k])), 'replace'); gv.selChanged(); } }] },
    { label: 'Curves', items: [
      { label: 'Pre Infinity', sub: () => infItems('pre') }, { label: 'Post Infinity', sub: () => infItems('post') }, '-',
      { label: 'Delete Curves', fn: () => { const cs = gv.curves; if (!cs.length) return; Undo.checkpoint('Delete curves'); for (const c of cs) { App.anim.curves.delete(c.node + '.' + c.attr); keySelection.delete(c); } animChanged(); refreshAll(); } }] },
    { label: 'Keys', items: [
      { label: 'Insert Key', hk: 'I', fn: () => gv.insertKey() }, { label: 'Delete Keys', hk: 'Del', fn: () => gv.deleteKeys() }, '-',
      { label: 'Snap', fn: () => gv.snapKeys() }, '-',
      { label: 'Break Tangents', fn: () => gv.breakTangents(true) }, { label: 'Unify Tangents', fn: () => gv.breakTangents(false) }, '-',
      { label: 'Snap Keys to Frames While Moving', ...chk(() => gv.snap, () => { gv.snap = !gv.snap; snapBtn.classList.toggle('on', gv.snap); }) }] },
    { label: 'Tangents', items: [...tanItems('both'), '-', { label: 'In Tangent', sub: () => tanItems('in') }, { label: 'Out Tangent', sub: () => tanItems('out') }] },
    { label: 'List', items: [
      { label: 'Auto Load Selected Objects', ...chk(() => state.autoLoad, () => { state.autoLoad = !state.autoLoad; if (!state.autoLoad) state.loaded = App.sel.map(o => o.inca.id); refreshAll(true); }) },
      { label: 'Load Selected Objects', fn: () => { state.autoLoad = false; state.loaded = App.sel.map(o => o.inca.id); refreshAll(true); } }] },
    { label: 'Show', items: [
      { label: 'Translate', ...chk(() => state.filter.translate, () => { state.filter.translate = !state.filter.translate; refreshAll(); }) },
      { label: 'Rotate', ...chk(() => state.filter.rotate, () => { state.filter.rotate = !state.filter.rotate; refreshAll(); }) },
      { label: 'Scale', ...chk(() => state.filter.scale, () => { state.filter.scale = !state.filter.scale; refreshAll(); }) },
      { label: 'Other Attributes', ...chk(() => state.filter.other, () => { state.filter.other = !state.filter.other; refreshAll(); }) }, '-',
      { label: 'Show All', fn: () => { Object.keys(state.filter).forEach(k => state.filter[k] = true); refreshAll(); } }] },
    { label: 'Help', items: [{ label: 'Help on Graph Editor', fn: () => toast('Graph Editor: LMB select/marquee (Shift toggle, Ctrl deselect, Ctrl+Shift add) · LMB/MMB drag moves keys (Shift constrains) · Alt+MMB pan · Alt+RMB / wheel zoom · F/A frame · Del deletes', 6000) }] },
  ]);

  // ---- keys
  setupWindowKeys(win, (e) => {
    const k = e.key.toLowerCase(), ctrl = e.ctrlKey || e.metaKey;
    if (e.key === 'Delete' || e.key === 'Backspace') { gv.deleteKeys(); return true; }
    if (ctrl && k === 'c') { gv.copyKeys(); return true; }
    if (ctrl && k === 'v') { gv.pasteKeys(); return true; }
    if (ctrl && k === 'x') { if (gv.copyKeys()) gv.deleteKeys(); return true; }
    if (ctrl || e.altKey) return false;
    if (k === 'f') { gv.frameSelection(); return true; }
    if (k === 'a') { gv.frameAll(); return true; }
    if (k === 'w') { setTool('move'); return true; }
    if (k === 'q') { setTool('select'); return true; }
    if (k === 'i') { gv.insertKey(); return true; }
    return false;
  });

  // ---- events
  const offs = [
    App.on('refresh', (flags) => { if (!flags || flags.has('graph') || flags.has('timeline')) refreshAll(); }),
    App.on('selectionChanged', () => refreshAll()),
    App.on('timeChanged', () => gv.request()),
    App.on('sceneLoaded', () => { keySelection.clear(); refreshAll(true); }),
  ];
  win.onClose = () => { offs.forEach(f => f()); editor = null; };
  win.onResize = () => gv.resize();
  editor = { win, gv, refreshAll };
  requestAnimationFrame(() => { gv.resize(); refreshAll(true); });
  return win;
}
function niceAttr(a) { return a.replace(/([A-Z])/g, ' $1').replace(/^./, c => c.toUpperCase()); }

App.ui = App.ui || {};
App.ui.graphEditor = openGraphEditor;
