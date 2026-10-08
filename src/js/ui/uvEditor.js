// Inca — UV Editor (Maya-style 2D UV texture editor)
import { App } from '../core/app.js';
import { Undo } from '../core/undo.js';
import { OPS } from '../core/ops.js';
import * as S from '../core/scene.js';
import { getThreeTexture, proceduralCanvas } from '../core/materials.js';
import { h, iconBtn, menuBar, FloatWin, toast, showMenu } from './dom.js';
import * as M from './editorMath.js';
import { injectEditorStyle, setupWindowKeys } from './graphEditor.js';

// history op that stores a full per-face UV set (hidden in the channel box / history list)
OPS.polyUVSet = {
  hidden: true, base: 'polyUVSet',
  apply: (pm, p) => { const m = pm.clone(); if (p.uv && p.uv.length === m.f.length) m.uv = p.uv.map(u => u ? u.map(c => c.slice()) : null); return { mesh: m }; },
};

const copyUV = (uv) => uv.map(u => u ? u.map(c => [c[0], c[1]]) : null);
// target meshes: component-mode hilite or selected meshes
export function uvTargets() {
  const list = (App.compMode ? (App.hilite.length ? App.hilite : App.sel) : App.sel);
  return list.filter(o => o && o.inca && o.inca.kind === 'mesh' && o.inca.mesh);
}
// make o.inca.mesh a private working copy (so cached history outputs aren't mutated)
export function beginUVEdit(o) {
  const H = o.inca.history || []; const last = H[H.length - 1];
  if (!(last && last.type === 'polyUVSet' && last._out === o.inca.mesh)) o.inca.mesh = o.inca.mesh.clone();
  return o.inca.mesh;
}
// record the current uv of o.inca.mesh in construction history (merging consecutive polyUVSet entries)
export function commitUV(o) {
  const pm = o.inca.mesh; if (!pm.uv) return;
  pm._smooth = null;
  const uv = copyUV(pm.uv);
  const H = o.inca.history || (o.inca.history = []);
  const last = H[H.length - 1];
  if (!H.length) { S.bakeMesh(o, pm); }
  else if (last.type === 'polyUVSet') { last.params = { uv }; last._out = pm; S.rebuildShape(o); }
  else { S.applyOp(o, 'polyUVSet', { uv }); }
  App.requestRender(); App.dirty('attr', 'channels');
}

export class UVView {
  constructor(wrap, { onChange = null } = {}) {
    this.wrap = wrap; this.onChange = onChange;
    this.cv = h('canvas'); wrap.append(this.cv);
    this.view = { cu: 0.5, cv: 0.5, s: 400 };
    this.sel = new Map(); // object id -> Set<corner id>
    this.tool = 'move'; this.showGrid = true; this.showImage = true; this.showChecker = false; this.shade = false; this.showPoints = true;
    this.W = 100; this.H = 100; this.drag = null; this.marquee = null; this._raf = 0; this._rebuild = new Set(); this._texCache = new Map();
    this.cv.addEventListener('mousedown', (e) => this.onDown(e));
    this.cv.addEventListener('wheel', (e) => this.onWheel(e), { passive: false });
    this.cv.addEventListener('contextmenu', (e) => e.preventDefault());
  }
  X(u) { return this.W / 2 + (u - this.view.cu) * this.view.s; }
  Y(v) { return this.H / 2 - (v - this.view.cv) * this.view.s; }
  U(x) { return (x - this.W / 2) / this.view.s + this.view.cu; }
  Vv(y) { return this.view.cv - (y - this.H / 2) / this.view.s; }
  local(e) { const r = this.cv.getBoundingClientRect(); return [e.clientX - r.left, e.clientY - r.top]; }
  request() { if (!this._raf) this._raf = requestAnimationFrame(() => { this._raf = 0; this.draw(); }); }
  resize() { const r = this.wrap.getBoundingClientRect(); this.W = Math.max(10, r.width); this.H = Math.max(10, r.height); this.draw(); }
  targets() { return uvTargets(); }
  selOf(o) { return this.sel.get(o.inca.id); }
  pruneSel() {
    const ts = new Map(this.targets().map(o => [o.inca.id, o]));
    for (const [id, s] of [...this.sel]) {
      const o = ts.get(id); if (!o) { this.sel.delete(id); continue; }
      const ci = M.cornerIndex(o.inca.mesh);
      for (const c of [...s]) if (c >= ci.n || !M.cornerUV(o.inca.mesh, ci, c)) s.delete(c);
      if (!s.size) this.sel.delete(id);
    }
  }
  selCount() { let n = 0; for (const s of this.sel.values()) n += s.size; return n; }
  // ---------------------------------------------------------------- framing
  frameBounds(b) {
    if (!b) b = { u0: 0, u1: 1, v0: 0, v1: 1 };
    const w = Math.max(b.u1 - b.u0, 1e-3), hh = Math.max(b.v1 - b.v0, 1e-3);
    this.view.s = Math.min((this.W - 60) / w, (this.H - 60) / hh);
    this.view.cu = (b.u0 + b.u1) / 2; this.view.cv = (b.v0 + b.v1) / 2; this.request();
  }
  boundsAll() { let b = null; for (const o of this.targets()) { const pm = o.inca.mesh; if (!pm.uv) continue; const ci = M.cornerIndex(pm); b = M.mergeBounds(b, M.uvBounds(pm, [...Array(ci.n).keys()])); } return b; }
  boundsSel() { let b = null; for (const o of this.targets()) { const s = this.selOf(o); if (s && s.size) b = M.mergeBounds(b, M.uvBounds(o.inca.mesh, s)); } return b; }
  frameAll() { const b = this.boundsAll(); this.frameBounds(b ? M.mergeBounds(b, { u0: 0, u1: 1, v0: 0, v1: 1 }) : null); }
  frameSelection() { const b = this.boundsSel(); if (b) this.frameBounds({ u0: b.u0 - 0.02, u1: b.u1 + 0.02, v0: b.v0 - 0.02, v1: b.v1 + 0.02 }); else this.frameAll(); }
  // ---------------------------------------------------------------- texture
  textureImage() {
    for (const o of this.targets()) {
      const mat = App.mats.get(o.inca.material); if (!mat) continue;
      const tid = mat.maps && (mat.maps.baseColor || mat.maps.color); if (!tid) continue;
      const t = App.texs.get(tid); if (!t) continue;
      try {
        if (t.type === 'file') { const tex = getThreeTexture(tid); const img = tex && tex.image; if (img && (img.naturalWidth || img.width) && (img.complete !== false)) return img; continue; }
        const key = tid + JSON.stringify(t.attrs); let cv = this._texCache.get(tid);
        if (!cv || cv.key !== key) { cv = { key, canvas: proceduralCanvas(t, 256) }; this._texCache.set(tid, cv); }
        return cv.canvas;
      } catch (e) { /* texture not ready */ }
    }
    return null;
  }
  checker() {
    if (this._checker) return this._checker;
    const c = document.createElement('canvas'); c.width = c.height = 256; const x = c.getContext('2d');
    for (let j = 0; j < 8; j++) for (let i = 0; i < 8; i++) { x.fillStyle = (i + j) % 2 ? '#6d6d6d' : '#b4b4b4'; x.fillRect(i * 32, j * 32, 32, 32); }
    x.fillStyle = '#e04040'; x.font = 'bold 10px sans-serif'; for (let j = 0; j < 8; j++) for (let i = 0; i < 8; i++) x.fillText(String.fromCharCode(65 + i) + (8 - j), i * 32 + 3, j * 32 + 12);
    return (this._checker = c);
  }
  // ---------------------------------------------------------------- draw
  draw() {
    const ctx = M.fitCanvas(this.cv, this.W, this.H); const { W, H } = this; const s = this.view.s;
    ctx.fillStyle = '#323232'; ctx.fillRect(0, 0, W, H);
    const x0 = this.X(0), y1 = this.Y(1);
    // 0..1 tile
    ctx.fillStyle = '#3c3c3c'; ctx.fillRect(x0, y1, s, s);
    const img = this.showChecker ? this.checker() : this.showImage ? this.textureImage() : null;
    if (img) { ctx.globalAlpha = 0.85; ctx.imageSmoothingEnabled = true; try { ctx.drawImage(img, x0, y1, s, s); } catch (e) { /* not drawable */ } ctx.globalAlpha = 1; }
    if (this.showGrid) {
      const step = M.niceStep(40 / s);
      ctx.lineWidth = 1; ctx.strokeStyle = img ? 'rgba(0,0,0,0.25)' : '#464646'; ctx.beginPath();
      for (let u = Math.floor(this.U(0) / step) * step; u <= this.U(W); u += step) { const x = Math.round(this.X(u)) + 0.5; ctx.moveTo(x, 0); ctx.lineTo(x, H); }
      for (let v = Math.floor(this.Vv(H) / step) * step; v <= this.Vv(0); v += step) { const y = Math.round(this.Y(v)) + 0.5; ctx.moveTo(0, y); ctx.lineTo(W, y); }
      ctx.stroke();
      // unit tile borders
      ctx.strokeStyle = '#5b5b5b'; ctx.beginPath();
      for (let u = Math.floor(this.U(0)); u <= Math.ceil(this.U(W)); u++) { const x = Math.round(this.X(u)) + 0.5; ctx.moveTo(x, 0); ctx.lineTo(x, H); }
      for (let v = Math.floor(this.Vv(H)); v <= Math.ceil(this.Vv(0)); v++) { const y = Math.round(this.Y(v)) + 0.5; ctx.moveTo(0, y); ctx.lineTo(W, y); }
      ctx.stroke();
      ctx.strokeStyle = '#7d7d7d'; ctx.strokeRect(Math.round(x0) + 0.5, Math.round(y1) + 0.5, Math.round(s), Math.round(s));
      // labels
      const ls = M.niceStep(70 / s);
      ctx.fillStyle = '#9a9a9a'; ctx.font = '10px sans-serif'; ctx.textBaseline = 'top';
      for (let u = Math.floor(this.U(0) / ls) * ls; u <= this.U(W); u += ls) ctx.fillText(M.fmtNum(u, ls), Math.round(this.X(u)) + 2, Math.min(H - 12, Math.max(2, this.Y(0) + 2)));
      ctx.textBaseline = 'bottom';
      for (let v = Math.floor(this.Vv(H) / ls) * ls; v <= this.Vv(0); v += ls) { if (Math.abs(v) < 1e-9) continue; ctx.fillText(M.fmtNum(v, ls), Math.min(W - 30, Math.max(2, this.X(0) + 2)), Math.round(this.Y(v)) - 1); }
    }
    // shells
    let nUV = 0, nSelUV = 0;
    const faceSelMode = App.compMode === 'face';
    for (const o of this.targets()) {
      const pm = o.inca.mesh; if (!pm.uv) continue;
      const fsel = faceSelMode && o.inca.compSel ? o.inca.compSel.f : null;
      // fills
      if (this.shade || (fsel && fsel.size)) {
        for (let f = 0; f < pm.f.length; f++) {
          const u = pm.uv[f]; if (!u) continue;
          const isSel = fsel && fsel.has(f);
          if (!this.shade && !isSel) continue;
          ctx.fillStyle = isSel ? 'rgba(255,140,40,0.35)' : M.faceUVArea(u) >= 0 ? 'rgba(70,110,220,0.28)' : 'rgba(230,60,60,0.35)';
          ctx.beginPath(); u.forEach((p, i) => { const x = this.X(p[0]), y = this.Y(p[1]); if (i) ctx.lineTo(x, y); else ctx.moveTo(x, y); }); ctx.closePath(); ctx.fill();
        }
      }
      ctx.strokeStyle = App.sel.includes(o) || App.hilite.includes(o) ? '#8fb4ef' : '#7f9cc8'; ctx.lineWidth = 1; ctx.beginPath();
      for (let f = 0; f < pm.f.length; f++) {
        const u = pm.uv[f]; if (!u) continue;
        u.forEach((p, i) => { const x = this.X(p[0]), y = this.Y(p[1]); if (i) ctx.lineTo(x, y); else ctx.moveTo(x, y); }); ctx.closePath();
      }
      ctx.stroke();
      // points
      const ci = M.cornerIndex(pm); const ss = this.selOf(o); nUV += ci.n; nSelUV += ss ? ss.size : 0;
      if (this.showPoints && ci.n < 200000) {
        ctx.fillStyle = '#c856c8';
        for (let id = 0; id < ci.n; id++) { if (ss && ss.has(id)) continue; const p = M.cornerUV(pm, ci, id); if (!p) continue; ctx.fillRect(Math.round(this.X(p[0])) - 1.5, Math.round(this.Y(p[1])) - 1.5, 3, 3); }
      }
      if (ss && ss.size) { ctx.fillStyle = '#3cff3c'; for (const id of ss) { const p = M.cornerUV(pm, ci, id); if (!p) continue; ctx.fillRect(Math.round(this.X(p[0])) - 2, Math.round(this.Y(p[1])) - 2, 4, 4); } }
    }
    this.drawManip(ctx);
    if (!this.targets().length) { ctx.fillStyle = '#9a9a9a'; ctx.font = '12px sans-serif'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle'; ctx.fillText('Select a polygon mesh to edit its UVs', W / 2, 24); ctx.textAlign = 'left'; }
    if (this.marquee) { const m = this.marquee; ctx.setLineDash([3, 3]); ctx.strokeStyle = '#eee'; ctx.strokeRect(Math.min(m.x0, m.x1) + 0.5, Math.min(m.y0, m.y1) + 0.5, Math.abs(m.x1 - m.x0), Math.abs(m.y1 - m.y0)); ctx.setLineDash([]); }
    this.stats = { nUV, nSelUV };
    if (this.onChange) this.onChange('draw');
  }
  pivot() { const b = this.boundsSel(); return b ? { u: b.cu, v: b.cv, b } : null; }
  drawManip(ctx) {
    if (this.tool === 'select' || !this.selCount()) return;
    const pv = (this.drag && this.drag.pivot) || this.pivot(); if (!pv) return;
    const x = this.X(pv.u), y = this.Y(pv.v);
    ctx.lineWidth = 2;
    if (this.tool === 'move') {
      ctx.strokeStyle = '#e24040'; ctx.beginPath(); ctx.moveTo(x, y); ctx.lineTo(x + 50, y); ctx.stroke();
      ctx.fillStyle = '#e24040'; ctx.beginPath(); ctx.moveTo(x + 58, y); ctx.lineTo(x + 48, y - 5); ctx.lineTo(x + 48, y + 5); ctx.fill();
      ctx.strokeStyle = '#40d040'; ctx.beginPath(); ctx.moveTo(x, y); ctx.lineTo(x, y - 50); ctx.stroke();
      ctx.fillStyle = '#40d040'; ctx.beginPath(); ctx.moveTo(x, y - 58); ctx.lineTo(x - 5, y - 48); ctx.lineTo(x + 5, y - 48); ctx.fill();
      ctx.strokeStyle = '#e8e040'; ctx.lineWidth = 1.5; ctx.strokeRect(x - 5, y - 5, 10, 10);
    } else if (this.tool === 'rotate') {
      ctx.strokeStyle = '#4080ff'; ctx.beginPath(); ctx.arc(x, y, 45, 0, Math.PI * 2); ctx.stroke();
      ctx.fillStyle = '#e8e040'; ctx.fillRect(x - 2, y - 2, 4, 4);
    } else if (this.tool === 'scale') {
      ctx.strokeStyle = '#e24040'; ctx.beginPath(); ctx.moveTo(x, y); ctx.lineTo(x + 50, y); ctx.stroke(); ctx.fillStyle = '#e24040'; ctx.fillRect(x + 46, y - 4, 8, 8);
      ctx.strokeStyle = '#40d040'; ctx.beginPath(); ctx.moveTo(x, y); ctx.lineTo(x, y - 50); ctx.stroke(); ctx.fillStyle = '#40d040'; ctx.fillRect(x - 4, y - 54, 8, 8);
      ctx.fillStyle = '#e8e040'; ctx.fillRect(x - 5, y - 5, 10, 10);
    }
    ctx.lineWidth = 1;
  }
  // ---------------------------------------------------------------- picking
  hitPoint(x, y, r = 6) {
    let best = null, bd = r * r;
    for (const o of this.targets()) {
      const pm = o.inca.mesh; if (!pm.uv) continue; const ci = M.cornerIndex(pm);
      for (let id = 0; id < ci.n; id++) { const p = M.cornerUV(pm, ci, id); if (!p) continue; const dx = this.X(p[0]) - x, dy = this.Y(p[1]) - y, d = dx * dx + dy * dy; if (d < bd) { bd = d; best = { o, id }; } }
    }
    return best;
  }
  pointsInRect(x0, y0, x1, y1) {
    const ua = this.U(Math.min(x0, x1)), ub = this.U(Math.max(x0, x1)), va = this.Vv(Math.max(y0, y1)), vb = this.Vv(Math.min(y0, y1));
    const out = [];
    for (const o of this.targets()) {
      const pm = o.inca.mesh; if (!pm.uv) continue; const ci = M.cornerIndex(pm); const ids = [];
      for (let id = 0; id < ci.n; id++) { const p = M.cornerUV(pm, ci, id); if (p && p[0] >= ua && p[0] <= ub && p[1] >= va && p[1] <= vb) ids.push(id); }
      if (ids.length) out.push({ o, ids });
    }
    return out;
  }
  applySel(entries, mode) {
    if (mode === 'replace') this.sel.clear();
    for (const { o, ids } of entries) {
      const key = o.inca.id; let s = this.sel.get(key); if (!s) this.sel.set(key, s = new Set());
      for (const i of ids) { if (mode === 'deselect') s.delete(i); else if (mode === 'toggle') { if (s.has(i)) s.delete(i); else s.add(i); } else s.add(i); }
      if (!s.size) this.sel.delete(key);
    }
    this.selChanged();
  }
  selChanged() { this.request(); if (this.onChange) this.onChange('sel'); }
  insideSelBox(x, y, pad = 10) {
    const b = this.boundsSel(); if (!b) return false;
    return x >= this.X(b.u0) - pad && x <= this.X(b.u1) + pad && y >= this.Y(b.v1) - pad && y <= this.Y(b.v0) + pad;
  }
  // ---------------------------------------------------------------- mouse
  onDown(e) {
    const [x, y] = this.local(e); e.preventDefault();
    if (e.altKey) { this.drag = { type: e.button === 2 ? 'zoom' : 'pan', x0: x, y0: y, view: { ...this.view }, U0: this.U(x), V0: this.Vv(y) }; }
    else if (e.button === 0) {
      const mode = M.modeFromEvent(e);
      const hit = this.hitPoint(x, y);
      if (this.tool !== 'select' && mode === 'replace' && (hit || this.insideSelBox(x, y))) {
        if (hit && !(this.selOf(hit.o) && this.selOf(hit.o).has(hit.id))) this.applySel([{ o: hit.o, ids: M.coincident(hit.o.inca.mesh, hit.id) }], 'replace');
        this.drag = { type: 'xform', tool: this.tool, x0: x, y0: y, started: false };
      } else {
        this.marquee = { x0: x, y0: y, x1: x, y1: y, mode, hit }; this.drag = { type: 'marquee' };
      }
    } else if (e.button === 1) {
      this.drag = { type: 'xform', tool: this.tool === 'select' ? 'move' : this.tool, x0: x, y0: y, started: false };
    } else if (e.button === 2) { this.contextMenu(e); return; }
    if (!this.drag) return;
    const mm = (ev) => this.onMove(ev);
    const mu = (ev) => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); this.onUp(ev); };
    addEventListener('mousemove', mm); addEventListener('mouseup', mu);
  }
  onMove(e) {
    const d = this.drag; if (!d) return; const [x, y] = this.local(e);
    if (d.type === 'pan') { this.view.cu = d.view.cu - (x - d.x0) / d.view.s; this.view.cv = d.view.cv + (y - d.y0) / d.view.s; this.request(); return; }
    if (d.type === 'zoom') {
      const f = Math.exp(((x - d.x0) - (y - d.y0)) * 0.006); this.view.s = M.clamp(d.view.s * f, 5, 1e6);
      this.view.cu = d.U0 - (d.x0 - this.W / 2) / this.view.s; this.view.cv = d.V0 + (d.y0 - this.H / 2) / this.view.s; this.request(); return;
    }
    if (d.type === 'marquee') { this.marquee.x1 = x; this.marquee.y1 = y; this.request(); return; }
    if (d.type === 'xform') {
      const dx = x - d.x0, dy = y - d.y0;
      if (!d.started) {
        if (Math.hypot(dx, dy) < 3) return;
        if (!this.selCount()) { this.drag = null; return; }
        Undo.checkpoint('UV ' + d.tool);
        d.started = true; d.pivot = this.pivot(); d.items = [];
        for (const o of this.targets()) { const s = this.selOf(o); if (!s || !s.size) continue; const pm = beginUVEdit(o); const ids = [...s]; d.items.push({ o, pm, ids, snap: M.snapshotCorners(pm, ids) }); }
        d.m0 = [this.U(d.x0), this.Vv(d.y0)];
      }
      const p = d.pivot; const m = [this.U(x), this.Vv(y)];
      let fn;
      if (d.tool === 'move') {
        let du = dx / this.view.s, dv = -dy / this.view.s;
        if (e.shiftKey) { if (!d.axis && Math.hypot(dx, dy) > 5) d.axis = Math.abs(dx) > Math.abs(dy) ? 'u' : 'v'; if (d.axis === 'u') dv = 0; else if (d.axis === 'v') du = 0; } else d.axis = null;
        if (e.ctrlKey) { const g = M.niceStep(10 / this.view.s); du = Math.round(du / g) * g; dv = Math.round(dv / g) * g; }
        fn = M.uvXf.translate(du, dv);
      } else if (d.tool === 'rotate') {
        let a = Math.atan2(m[1] - p.v, m[0] - p.u) - Math.atan2(d.m0[1] - p.v, d.m0[0] - p.u);
        if (e.shiftKey) { const st = Math.PI / 12; a = Math.round(a / st) * st; }
        fn = M.uvXf.rotate(a, p.u, p.v);
      } else {
        let fu, fv;
        if (e.shiftKey) { // non-uniform: scale along the dominant drag axis
          if (!d.axis && Math.hypot(dx, dy) > 5) d.axis = Math.abs(dx) > Math.abs(dy) ? 'u' : 'v';
          fu = d.axis === 'u' ? Math.exp(dx * 0.01) : 1; fv = d.axis === 'v' ? Math.exp(-dy * 0.01) : 1;
        } else { const f = Math.exp((dx - dy) * 0.006); fu = fv = f; d.axis = null; }
        fn = M.uvXf.scale(fu, fv, p.u, p.v);
      }
      for (const it of d.items) { M.restoreCorners(it.pm, it.ids, it.snap); M.xformCorners(it.pm, it.ids, fn); it.pm._smooth = null; this._rebuild.add(it.o); }
      this.flushRebuild(); this.request();
    }
  }
  flushRebuild() {
    if (this._rbRaf) return;
    this._rbRaf = requestAnimationFrame(() => { this._rbRaf = 0; for (const o of this._rebuild) S.rebuildShape(o); this._rebuild.clear(); App.requestRender(); });
  }
  onUp(e) {
    const d = this.drag; this.drag = null; if (!d) return;
    if (d.type === 'marquee') {
      const m = this.marquee; this.marquee = null;
      const small = Math.abs(m.x1 - m.x0) < 3 && Math.abs(m.y1 - m.y0) < 3;
      if (small) { const hit = m.hit || this.hitPoint(m.x0, m.y0); this.applySel(hit ? [{ o: hit.o, ids: M.coincident(hit.o.inca.mesh, hit.id) }] : [], m.mode === 'replace' || hit ? m.mode : 'add'); }
      else this.applySel(this.pointsInRect(m.x0, m.y0, m.x1, m.y1), m.mode);
      return;
    }
    if (d.type === 'xform' && d.started) {
      for (const it of d.items) commitUV(it.o);
      this._rebuild.clear();
      const names = d.items.map(it => it.o.inca.name).join(' ');
      App.emit('echo', d.tool === 'move' ? `polyEditUV -u ... -v ... ${names};` : d.tool === 'rotate' ? `polyEditUV -pu ${M.fmtNum(d.pivot.u, 0.001)} -pv ${M.fmtNum(d.pivot.v, 0.001)} -a ... ${names};` : `polyEditUV -su ... -sv ... ${names};`);
      this.request(); if (this.onChange) this.onChange('edit');
      return;
    }
    this.request();
  }
  onWheel(e) {
    e.preventDefault(); const [x, y] = this.local(e); const U0 = this.U(x), V0 = this.Vv(y);
    this.view.s = M.clamp(this.view.s * Math.pow(1.0015, -e.deltaY), 5, 1e6);
    this.view.cu = U0 - (x - this.W / 2) / this.view.s; this.view.cv = V0 + (y - this.H / 2) / this.view.s; this.request();
  }
  contextMenu(e) {
    showMenu([
      { label: 'Select All UVs', fn: () => this.selectAll() }, { label: 'Select Shell', fn: () => this.selectShell() }, { label: 'Clear Selection', fn: () => this.applySel([], 'replace') }, '-',
      { label: 'Flip U', fn: () => this.op('flipU') }, { label: 'Flip V', fn: () => this.op('flipV') }, { label: 'Rotate 90° CCW', fn: () => this.op('rot90') }, { label: 'Rotate 90° CW', fn: () => this.op('rot-90') }, '-',
      { label: 'Normalize', fn: () => this.op('normalize') }, { label: 'Layout', fn: () => this.op('layout') }, { label: 'Unfold (Relax)', fn: () => this.op('relax') }, '-',
      { label: 'Frame All', hk: 'A', fn: () => this.frameAll() }, { label: 'Frame Selection', hk: 'F', fn: () => this.frameSelection() },
    ], e.clientX, e.clientY);
  }
  // ---------------------------------------------------------------- selection helpers
  selectAll() { this.applySel(this.targets().filter(o => o.inca.mesh.uv).map(o => ({ o, ids: [...Array(M.cornerIndex(o.inca.mesh).n).keys()].filter(id => M.cornerUV(o.inca.mesh, M.cornerIndex(o.inca.mesh), id)) })), 'replace'); }
  selectShell() {
    const entries = [];
    for (const o of this.targets()) {
      const s = this.selOf(o); if (!s || !s.size) continue; const pm = o.inca.mesh; const ci = M.cornerIndex(pm);
      const { shellOfFace, shells } = M.uvShells(pm); const want = new Set(); for (const id of s) { const sh = shellOfFace[ci.cf[id]]; if (sh >= 0) want.add(sh); }
      entries.push({ o, ids: M.cornersOfFaces(pm, [...want].flatMap(i => shells[i])) });
    }
    this.applySel(entries, 'add');
  }
  // convert the scene component selection (vertex/uv/face/edge) to UVs
  fromScene() {
    const entries = [];
    for (const o of this.targets()) {
      const cs = o.inca.compSel; const pm = o.inca.mesh; if (!cs || !pm.uv) continue; const ci = M.cornerIndex(pm);
      let ids = [];
      if (App.compMode === 'face') ids = M.cornersOfFaces(pm, [...cs.f]);
      else {
        let verts = cs.v;
        if (App.compMode === 'edge') { verts = new Set(); const E = pm.topo.edges; for (const e of cs.e) if (E[e]) { verts.add(E[e][0]); verts.add(E[e][1]); } }
        for (const v of verts) for (const id of ci.byVert.get(v) || []) if (M.cornerUV(pm, ci, id)) ids.push(id);
      }
      if (ids.length) entries.push({ o, ids });
    }
    this.sel.clear(); this.applySel(entries, 'add');
  }
  // ---------------------------------------------------------------- operations
  // operate on selected UVs' corners, or (scope 'shell') their shells, or everything when nothing is selected
  op(kind) {
    const ts = this.targets().filter(o => o.inca.mesh.uv);
    if (!ts.length) { App.help('Select a mesh with UVs'); return; }
    const any = this.selCount() > 0;
    Undo.checkpoint('UV ' + kind);
    // global pivot over all affected corners
    const work = ts.map(o => {
      const pm = beginUVEdit(o); const ci = M.cornerIndex(pm);
      const s = this.selOf(o);
      let ids = any ? (s ? [...s] : []) : [...Array(ci.n).keys()].filter(id => M.cornerUV(pm, ci, id));
      return { o, pm, ids };
    }).filter(w => w.ids.length);
    if (!work.length) return;
    let b = null; for (const w of work) b = M.mergeBounds(b, M.uvBounds(w.pm, w.ids));
    for (const w of work) {
      const { pm, ids } = w;
      if (kind === 'flipU') M.xformCorners(pm, ids, M.uvXf.flipU(b.cu));
      else if (kind === 'flipV') M.xformCorners(pm, ids, M.uvXf.flipV(b.cv));
      else if (kind === 'rot90') M.xformCorners(pm, ids, M.uvXf.rotate(Math.PI / 2, b.cu, b.cv));
      else if (kind === 'rot-90') M.xformCorners(pm, ids, M.uvXf.rotate(-Math.PI / 2, b.cu, b.cv));
      else if (kind === 'normalize') {
        const w0 = b.u1 - b.u0 || 1, h0 = b.v1 - b.v0 || 1; const sc = 1 / Math.max(w0, h0);
        M.xformCorners(pm, ids, (p) => [(p[0] - b.u0) * sc, (p[1] - b.v0) * sc]);
      } else if (kind === 'layout' || kind === 'relax') {
        const ci = M.cornerIndex(pm); const { shellOfFace, shells } = M.uvShells(pm);
        const want = new Set(); for (const id of ids) { const sh = shellOfFace[ci.cf[id]]; if (sh >= 0) want.add(sh); }
        const faceShells = [...want].map(i => shells[i]);
        if (kind === 'layout') M.layoutShells(pm, faceShells);
        else M.relaxUVs(pm, M.cornersOfFaces(pm, faceShells.flat()), 60);
      }
    }
    if (kind === 'layout' && work.length > 1) { /* each object laid out into its own 0..1 */ }
    for (const w of work) commitUV(w.o);
    App.emit('echo', { flipU: 'polyFlipUV -flipType 0;', flipV: 'polyFlipUV -flipType 1;', rot90: 'polyRotateUVs 90;', 'rot-90': 'polyRotateUVs -90;', normalize: 'polyNormalizeUV -normalizeType 1 -preserveAspectRatio on;', layout: 'polyMultiLayoutUV -layoutMethod 1;', relax: 'u3dUnfold -ite 1;' }[kind] || kind);
    this.request(); if (this.onChange) this.onChange('edit');
  }
  project(cmd) {
    if (!App.cmds) return;
    try { App.cmds.run(cmd); } catch (e) { App.emit('error', String(e.message || e)); }
    this.sel.clear(); setTimeout(() => { this.request(); this.frameAll(); }, 0);
  }
}

// ------------------------------------------------------------------ window
let uve = null;
export function openUVEditor() {
  injectEditorStyle();
  const win = new FloatWin('uvEditor', 'UV Editor', { w: 760, h: 620 });
  if (win.reused) { uve?.view.request(); return win; }
  win.el.classList.add('anim-ed');
  const menuRow = h('div', { class: 'pmenubar' }), toolbar = h('div', { class: 'ptoolbar' });
  const wrap = h('div', { class: 'ge-canvas-wrap', style: { flex: '1' } });
  const status = h('div', { class: 'ed-status' });
  win.body.append(menuRow, toolbar, h('div', { class: 'ge' }, wrap), status);
  let pushing = false;
  const uv = new UVView(wrap, { onChange: (why) => { if (why === 'sel') pushToScene(); updateStatus(); } });
  // push UV selection to the scene as UV/vertex components when in UV component mode
  function pushToScene() {
    if (App.compMode !== 'uv') return;
    pushing = true;
    try {
      for (const o of uv.targets()) {
        if (!o.inca.compSel) continue; const s = uv.selOf(o); const ci = M.cornerIndex(o.inca.mesh);
        o.inca.compSel.v = new Set(s ? [...s].map(id => ci.cv[id]) : []); o.userData.compDirty = true;
      }
      App.emit('selectionChanged'); App.dirty('viewport', 'channels');
    } finally { pushing = false; }
  }
  function updateStatus() {
    const st = uv.stats || { nUV: 0, nSelUV: 0 };
    status.textContent = '';
    status.append(h('span', { text: `${uv.targets().length} mesh(es)` }), h('span', { text: `UV corners: ${st.nSelUV} / ${st.nUV} selected` }),
      h('span', { class: 'dim', text: 'Q/W/E/R tools · Alt+MMB pan · Alt+RMB / wheel zoom · Shift constrains · F/A frame' }));
  }
  const tb = (name, title, fn) => iconBtn(name, title, fn, 'ib small');
  const txt = (label, title, fn) => { const b = h('div', { class: 'ib small txt', title, text: label, onclick: fn }); b.addEventListener('mouseenter', () => App.help(title)); return b; };
  const sep = () => h('div', { class: 'tb-sep' });
  const tools = { select: tb('select', 'Select UVs (Q)', () => setTool('select')), move: tb('move', 'Move UVs (W)', () => setTool('move')), rotate: tb('rotate', 'Rotate UVs (E)', () => setTool('rotate')), scale: tb('scale', 'Scale UVs (R)', () => setTool('scale')) };
  const setTool = (t) => { uv.tool = t; for (const [k, b] of Object.entries(tools)) b.classList.toggle('on', k === t); uv.request(); };
  const toggles = {};
  const tog = (key, icon, title, label = null) => { const b = label ? txt(label, title, null) : tb(icon, title, null); b.addEventListener('click', () => { uv[key] = !uv[key]; if (key === 'showChecker' && uv.showChecker) uv.showImage = false; if (key === 'showImage' && uv.showImage) uv.showChecker = false; syncToggles(); uv.request(); }); toggles[key] = b; return b; };
  const syncToggles = () => { for (const [k, b] of Object.entries(toggles)) b.classList.toggle('on', !!uv[k]); };
  toolbar.append(tools.select, tools.move, tools.rotate, tools.scale, sep(),
    txt('Planar', 'Planar mapping', () => uv.project('planarMap')), txt('Cyl', 'Cylindrical mapping', () => uv.project('cylindricalMap')),
    txt('Sph', 'Spherical mapping', () => uv.project('sphericalMap')), tb('autoMap', 'Automatic mapping', () => uv.project('automaticMap')), sep(),
    txt('Flip U', 'Flip selected UVs in U', () => uv.op('flipU')), txt('Flip V', 'Flip selected UVs in V', () => uv.op('flipV')),
    txt('↺90', 'Rotate UVs 90° counter-clockwise', () => uv.op('rot90')), txt('↻90', 'Rotate UVs 90° clockwise', () => uv.op('rot-90')), sep(),
    txt('Normalize', 'Fit selected (or all) UVs into 0..1', () => uv.op('normalize')), txt('Layout', 'Lay out UV shells in 0..1', () => uv.op('layout')), txt('Unfold', 'Relax UVs (borders pinned)', () => uv.op('relax')), sep(),
    tog('showGrid', 'grid', 'Toggle grid'), tog('showImage', 'textured', 'Display texture image'), tog('showChecker', 'checker', 'Checker tile'), tog('shade', 'smoothShade', 'Shade UVs (blue = front, red = flipped)'),
    h('div', { class: 'tb-spacer' }), tb('frameAll', 'Frame all (A)', () => uv.frameAll()), tb('frameSel', 'Frame selection (F)', () => uv.frameSelection()));
  setTool('move'); syncToggles();
  const chk = (key) => ({ check: () => !!uv[key], fn: () => { uv[key] = !uv[key]; if (key === 'showChecker' && uv.showChecker) uv.showImage = false; if (key === 'showImage' && uv.showImage) uv.showChecker = false; syncToggles(); uv.request(); } });
  menuBar(menuRow, [
    { label: 'Edit', items: [{ label: 'Undo', fn: () => App.undo.undo() }, { label: 'Redo', fn: () => App.undo.redo() }] },
    { label: 'Select', items: [{ label: 'Select All UVs', fn: () => uv.selectAll() }, { label: 'Select Shell', fn: () => uv.selectShell() }, { label: 'Convert Scene Selection to UVs', fn: () => uv.fromScene() }, { label: 'Clear Selection', fn: () => uv.applySel([], 'replace') }] },
    { label: 'Create', items: [{ label: 'Planar', fn: () => uv.project('planarMap') }, { label: 'Cylindrical', fn: () => uv.project('cylindricalMap') }, { label: 'Spherical', fn: () => uv.project('sphericalMap') }, { label: 'Automatic', fn: () => uv.project('automaticMap') }] },
    { label: 'Modify', items: [{ label: 'Flip U', fn: () => uv.op('flipU') }, { label: 'Flip V', fn: () => uv.op('flipV') }, { label: 'Rotate 90° CCW', fn: () => uv.op('rot90') }, { label: 'Rotate 90° CW', fn: () => uv.op('rot-90') }, '-', { label: 'Normalize', fn: () => uv.op('normalize') }, { label: 'Layout', fn: () => uv.op('layout') }, { label: 'Unfold (Relax)', fn: () => uv.op('relax') }] },
    { label: 'Tool', items: ['select', 'move', 'rotate', 'scale'].map((t, i) => ({ label: t[0].toUpperCase() + t.slice(1) + ' Tool', hk: 'QWER'[i], check: () => uv.tool === t, fn: () => setTool(t) })) },
    { label: 'View', items: [{ label: 'Frame All', hk: 'A', fn: () => uv.frameAll() }, { label: 'Frame Selection', hk: 'F', fn: () => uv.frameSelection() }, '-', { label: 'Grid', ...chk('showGrid') }, { label: 'Display Image', ...chk('showImage') }, { label: 'Checker Map', ...chk('showChecker') }, { label: 'Shade UVs', ...chk('shade') }, { label: 'UV Points', ...chk('showPoints') }] },
    { label: 'Help', items: [{ label: 'Help on UV Editor', fn: () => toast('UV Editor: click/marquee UVs (Shift toggle, Ctrl deselect, Ctrl+Shift add) · W/E/R + LMB drag on the selection (or MMB anywhere) to move/rotate/scale · Shift constrains, Ctrl snaps · Alt+MMB pan · wheel zoom', 7000) }] },
  ]);
  setupWindowKeys(win, (e) => {
    const k = e.key.toLowerCase();
    if (e.ctrlKey || e.metaKey || e.altKey) return false;
    if (k === 'f') { uv.frameSelection(); return true; }
    if (k === 'a') { uv.frameAll(); return true; }
    const t = { q: 'select', w: 'move', e: 'rotate', r: 'scale' }[k]; if (t) { setTool(t); return true; }
    return false;
  });
  const offs = [
    App.on('refresh', () => { uv.pruneSel(); uv.request(); }),
    App.on('selectionChanged', () => { if (pushing) return; uv.pruneSel(); if (App.compMode) uv.fromScene(); uv.request(); }),
    App.on('sceneLoaded', () => { uv.sel.clear(); uv.request(); }),
    App.on('textureLoaded', () => uv.request()),
    App.on('materialChanged', () => uv.request()),
  ];
  win.onClose = () => { offs.forEach(f => f()); uve = null; };
  win.onResize = () => uv.resize();
  uve = { win, view: uv };
  requestAnimationFrame(() => { uv.resize(); uv.frameAll(); if (App.compMode) uv.fromScene(); updateStatus(); });
  return win;
}

App.ui = App.ui || {};
App.ui.uvEditor = openUVEditor;
