// Inca — pure helpers shared by the Graph Editor, Dope Sheet and UV Editor.
// No DOM access at import time (safe to import from Node tests).

// ------------------------------------------------------------------ generic
export const clamp = (x, a, b) => Math.max(a, Math.min(b, x));
// smallest "nice" step (1, 2, 5 x 10^n) that is >= minUnits
export function niceStep(minUnits) {
  if (!(minUnits > 0) || !isFinite(minUnits)) return 1;
  const p = Math.pow(10, Math.floor(Math.log10(minUnits)));
  for (const m of [1, 2, 5, 10]) if (m * p >= minUnits - 1e-12) return m * p;
  return 10 * p;
}
export function fmtNum(n, step = 1) {
  const d = step >= 1 ? 0 : Math.min(6, Math.ceil(-Math.log10(step) + 1e-9));
  const s = n.toFixed(d);
  return s === '-0' || /^-0\.0*$/.test(s) ? s.slice(1) : s;
}
// Maya curve colours: X red, Y green, Z blue, others cyan / yellow / ...
const OTHER = ['#3fd0d8', '#e6d24a', '#d77ae0', '#e8a04a', '#9ad86a'];
export function attrColor(attr = '') {
  if (/^(translate|rotate|scale)[XYZ]$/.test(attr) || /^(rotatePivot|scalePivot)[XYZ]$/.test(attr)) {
    const ax = attr[attr.length - 1];
    return ax === 'X' ? '#e0393e' : ax === 'Y' ? '#3fcf4a' : '#3f86f2';
  }
  if (attr === 'visibility') return '#e6d24a';
  let hsh = 0; for (const ch of attr) hsh = (hsh * 31 + ch.charCodeAt(0)) | 0;
  return OTHER[Math.abs(hsh) % OTHER.length];
}
// size a canvas for HiDPI; returns the 2D context scaled to CSS pixels
export function fitCanvas(cv, w, h) {
  const dpr = (globalThis.devicePixelRatio || 1);
  const W = Math.max(1, Math.round(w * dpr)), H = Math.max(1, Math.round(h * dpr));
  if (cv.width !== W || cv.height !== H) { cv.width = W; cv.height = H; }
  const ctx = cv.getContext('2d');
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  return ctx;
}
// time ruler band (frame numbers) at the top of a canvas
export function drawTimeRuler(ctx, { W, RH, X, T, sx, x0 = 0, bg = '#2f2f2f', fg = '#bdbdbd', range = null }) {
  ctx.fillStyle = bg; ctx.fillRect(x0, 0, W - x0, RH);
  if (range) { // playback range tint
    const a = Math.max(x0, X(range[0])), b = Math.min(W, X(range[1]));
    if (b > a) { ctx.fillStyle = 'rgba(255,255,255,0.05)'; ctx.fillRect(a, 0, b - a, RH); }
  }
  const step = Math.max(1, niceStep(60 / sx)), minor = Math.max(1, niceStep(8 / sx));
  const t0 = Math.floor(T(x0) / minor) * minor, t1 = T(W);
  ctx.strokeStyle = '#777'; ctx.lineWidth = 1; ctx.beginPath();
  for (let t = t0; t <= t1; t += minor) {
    const x = Math.round(X(t)) + 0.5; if (x < x0) continue;
    const major = Math.abs(t / step - Math.round(t / step)) < 1e-6;
    ctx.moveTo(x, RH); ctx.lineTo(x, RH - (major ? 7 : 3));
  }
  ctx.stroke();
  ctx.fillStyle = fg; ctx.font = '10px sans-serif'; ctx.textBaseline = 'top'; ctx.textAlign = 'left';
  for (let t = Math.floor(T(x0) / step) * step; t <= t1; t += step) { const x = X(t); if (x < x0 - 1) continue; ctx.fillText(fmtNum(t, step), Math.round(x) + 2, 2); }
  ctx.strokeStyle = '#222'; ctx.beginPath(); ctx.moveTo(x0, RH + 0.5); ctx.lineTo(W, RH + 0.5); ctx.stroke();
}
export function drawTimeMarker(ctx, { x, RH, H }) {
  const xx = Math.round(x) + 0.5;
  ctx.strokeStyle = '#e8d43c'; ctx.lineWidth = 1; ctx.beginPath(); ctx.moveTo(xx, RH); ctx.lineTo(xx, H); ctx.stroke();
}
export function drawTimeBox(ctx, { x, RH, frame }) {
  const label = fmtNum(frame, Math.abs(frame - Math.round(frame)) < 1e-6 ? 1 : 0.01);
  ctx.font = '10px sans-serif';
  const w = Math.max(14, ctx.measureText(label).width + 6);
  ctx.fillStyle = '#c8302b'; ctx.fillRect(Math.round(x - w / 2), 0, Math.round(w), RH);
  ctx.strokeStyle = '#e8d43c'; ctx.strokeRect(Math.round(x - w / 2) + 0.5, 0.5, Math.round(w) - 1, RH - 1);
  ctx.fillStyle = '#fff'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle'; ctx.fillText(label, x, RH / 2 + 0.5); ctx.textAlign = 'left';
}

// ------------------------------------------------------------------ key selection: Map<curve, Set<key>>
export function selAdd(sel, c, k) { let s = sel.get(c); if (!s) sel.set(c, s = new Set()); s.add(k); }
export function selHas(sel, c, k) { const s = sel.get(c); return !!(s && s.has(k)); }
export function selDel(sel, c, k) { const s = sel.get(c); if (s) { s.delete(k); if (!s.size) sel.delete(c); } }
export function selCount(sel) { let n = 0; for (const s of sel.values()) n += s.size; return n; }
export function selApply(sel, pairs, mode) { // pairs [[c,k]]; mode replace|toggle|add|deselect
  if (mode === 'replace') sel.clear();
  for (const [c, k] of pairs) {
    if (mode === 'deselect') selDel(sel, c, k);
    else if (mode === 'toggle') { if (selHas(sel, c, k)) selDel(sel, c, k); else selAdd(sel, c, k); }
    else selAdd(sel, c, k);
  }
}
// drop curves / keys that no longer exist
export function selPrune(sel, liveCurves) {
  const live = liveCurves instanceof Set ? liveCurves : new Set(liveCurves);
  for (const [c, s] of [...sel]) {
    if (!live.has(c)) { sel.delete(c); continue; }
    const ks = new Set(c.keys);
    for (const k of [...s]) if (!ks.has(k)) s.delete(k);
    if (!s.size) sel.delete(c);
  }
}
// restrict a selection to a set of curves
export function selFilter(sel, curves) { const out = new Map(); for (const c of curves) { const s = sel.get(c); if (s && s.size) out.set(c, s); } return out; }
export const modeFromEvent = (e) => e.ctrlKey && e.shiftKey ? 'add' : e.ctrlKey ? 'deselect' : e.shiftKey ? 'toggle' : 'replace';

// ------------------------------------------------------------------ key editing (curve = {keys:[{t,v,...}], sort()})
export function beginKeyDrag(sel) {
  const items = [];
  for (const [c, s] of sel) for (const k of s) items.push({ c, k, t0: k.t, v0: k.v });
  return { items, curves: [...sel.keys()] };
}
export function applyKeyDrag(st, dt, dv, { snap = true, time = true, value = true } = {}) {
  for (const it of st.items) {
    let t = it.t0 + (time ? dt : 0);
    if (snap) t = Math.round(t);
    it.k.t = t; it.k.v = it.v0 + (value ? dv : 0);
  }
  for (const c of st.curves) sortKeys(c);
}
export function sortKeys(c) { if (c.sort) c.sort(); else c.keys.sort((a, b) => a.t - b.t); }
// after a move: keys sharing the same time collapse; the selected (moved) key wins
export function resolveKeyCollisions(curves, sel) {
  let removed = 0;
  for (const c of curves) {
    sortKeys(c);
    const out = [];
    for (const k of c.keys) {
      const prev = out[out.length - 1];
      if (prev && Math.abs(prev.t - k.t) < 1e-4) {
        const ps = selHas(sel, c, prev), ks = selHas(sel, c, k);
        if (ks && !ps) { out[out.length - 1] = k; } // moved key replaces stationary key
        if (ks && ps) selDel(sel, c, prev === out[out.length - 1] ? k : prev);
        removed++; continue;
      }
      out.push(k);
    }
    c.keys = out;
  }
  return removed;
}
export function deleteSelectedKeys(sel) {
  let n = 0; const emptied = [];
  for (const [c, s] of sel) { const before = c.keys.length; c.keys = c.keys.filter(k => !s.has(k)); n += before - c.keys.length; if (!c.keys.length) emptied.push(c); }
  sel.clear();
  return { removed: n, emptied };
}
// tangent handle direction in screen space for slope s (value units per frame)
export function handleVec(slope, sx, sy, len = 40) {
  const dx = sx, dy = -slope * sy; const l = Math.hypot(dx, dy) || 1;
  return [dx / l * len, dy / l * len];
}
// slope from a handle position relative to the key (screen px); side 'in' keeps dx < 0, 'out' dx > 0
export function slopeFromHandle(ddx, ddy, sx, sy, side) {
  if (side === 'in') ddx = Math.min(ddx, -0.5); else ddx = Math.max(ddx, 0.5);
  return (-ddy / sy) / (ddx / sx);
}
// bounds of keys (and optionally evaluated curve) -> {t0,t1,v0,v1} or null
export function keyBounds(pairs) {
  let t0 = Infinity, t1 = -Infinity, v0 = Infinity, v1 = -Infinity;
  for (const [, k] of pairs) { t0 = Math.min(t0, k.t); t1 = Math.max(t1, k.t); v0 = Math.min(v0, k.v); v1 = Math.max(v1, k.v); }
  return isFinite(t0) ? { t0, t1, v0, v1 } : null;
}

// ------------------------------------------------------------------ UVs (pm = {f:[[vi..]], uv:[[[u,v]..]|null]})
const _ci = new WeakMap();
// corner index: corner id = base[fi] + ci
export function cornerIndex(pm) {
  let c = _ci.get(pm);
  if (c && c.nf === pm.f.length) return c;
  const base = new Int32Array(pm.f.length + 1); let n = 0;
  for (let i = 0; i < pm.f.length; i++) { base[i] = n; n += pm.f[i].length; }
  base[pm.f.length] = n;
  const cf = new Int32Array(n), cc = new Int32Array(n), cv = new Int32Array(n);
  for (let i = 0; i < pm.f.length; i++) for (let j = 0; j < pm.f[i].length; j++) { cf[base[i] + j] = i; cc[base[i] + j] = j; cv[base[i] + j] = pm.f[i][j]; }
  const byVert = new Map();
  for (let id = 0; id < n; id++) { const v = cv[id]; let a = byVert.get(v); if (!a) byVert.set(v, a = []); a.push(id); }
  c = { nf: pm.f.length, n, base, cf, cc, cv, byVert };
  _ci.set(pm, c);
  return c;
}
export function cornerUV(pm, ci, id) { const u = pm.uv && pm.uv[ci.cf[id]]; return u ? u[ci.cc[id]] : null; }
// group corners that are the same UV point (same vertex + coincident uv)
export function uvGroups(pm, eps = 1e-6) {
  const ci = cornerIndex(pm); const g = new Int32Array(ci.n).fill(-1); let ng = 0;
  for (const ids of ci.byVert.values()) {
    for (let a = 0; a < ids.length; a++) {
      const ua = cornerUV(pm, ci, ids[a]); if (!ua || g[ids[a]] >= 0) continue;
      g[ids[a]] = ng;
      for (let b = a + 1; b < ids.length; b++) {
        if (g[ids[b]] >= 0) continue; const ub = cornerUV(pm, ci, ids[b]);
        if (ub && Math.abs(ub[0] - ua[0]) <= eps && Math.abs(ub[1] - ua[1]) <= eps) g[ids[b]] = ng;
      }
      ng++;
    }
  }
  return { group: g, count: ng };
}
// corners coincident with corner id (same vertex, same uv)
export function coincident(pm, id, eps = 1e-6) {
  const ci = cornerIndex(pm); const u0 = cornerUV(pm, ci, id); if (!u0) return [];
  return (ci.byVert.get(ci.cv[id]) || []).filter(j => { const u = cornerUV(pm, ci, j); return u && Math.abs(u[0] - u0[0]) <= eps && Math.abs(u[1] - u0[1]) <= eps; });
}
// shells: faces connected through shared UV points. returns { shellOfFace: Int32Array (-1 = no uv), shells: [[fi..]] }
export function uvShells(pm) {
  const ci = cornerIndex(pm); const { group, count } = uvGroups(pm);
  const par = new Int32Array(pm.f.length).map((_, i) => i);
  const find = (x) => { while (par[x] !== x) { par[x] = par[par[x]]; x = par[x]; } return x; };
  const owner = new Int32Array(count).fill(-1);
  for (let id = 0; id < ci.n; id++) {
    const gi = group[id]; if (gi < 0) continue;
    const f = ci.cf[id];
    if (owner[gi] < 0) owner[gi] = f; else { const a = find(owner[gi]), b = find(f); if (a !== b) par[a] = b; }
  }
  const map = new Map(); const shellOfFace = new Int32Array(pm.f.length).fill(-1); const shells = [];
  for (let f = 0; f < pm.f.length; f++) {
    if (!(pm.uv && pm.uv[f])) continue;
    const r = find(f); let s = map.get(r);
    if (s === undefined) { s = shells.length; map.set(r, s); shells.push([]); }
    shellOfFace[f] = s; shells[s].push(f);
  }
  return { shellOfFace, shells };
}
export function cornersOfFaces(pm, faces) { const ci = cornerIndex(pm); const out = []; for (const f of faces) if (pm.uv && pm.uv[f]) for (let j = 0; j < pm.f[f].length; j++) out.push(ci.base[f] + j); return out; }
export function uvBounds(pm, ids) {
  const ci = cornerIndex(pm); let u0 = Infinity, u1 = -Infinity, v0 = Infinity, v1 = -Infinity;
  for (const id of ids) { const p = cornerUV(pm, ci, id); if (!p) continue; u0 = Math.min(u0, p[0]); u1 = Math.max(u1, p[0]); v0 = Math.min(v0, p[1]); v1 = Math.max(v1, p[1]); }
  return isFinite(u0) ? { u0, u1, v0, v1, cu: (u0 + u1) / 2, cv: (v0 + v1) / 2 } : null;
}
export function mergeBounds(a, b) { if (!a) return b; if (!b) return a; const u0 = Math.min(a.u0, b.u0), u1 = Math.max(a.u1, b.u1), v0 = Math.min(a.v0, b.v0), v1 = Math.max(a.v1, b.v1); return { u0, u1, v0, v1, cu: (u0 + u1) / 2, cv: (v0 + v1) / 2 }; }
// apply fn([u,v]) -> [u,v] to corners (in place)
export function xformCorners(pm, ids, fn) {
  const ci = cornerIndex(pm);
  for (const id of ids) { const p = cornerUV(pm, ci, id); if (!p) continue; const r = fn(p); p[0] = r[0]; p[1] = r[1]; }
}
export const uvXf = {
  translate: (du, dv) => (p) => [p[0] + du, p[1] + dv],
  scale: (fu, fv, cu, cv) => (p) => [cu + (p[0] - cu) * fu, cv + (p[1] - cv) * fv],
  rotate: (ang, cu, cv) => { const c = Math.cos(ang), s = Math.sin(ang); return (p) => { const x = p[0] - cu, y = p[1] - cv; return [cu + x * c - y * s, cv + x * s + y * c]; }; },
  flipU: (cu) => (p) => [2 * cu - p[0], p[1]],
  flipV: (cv) => (p) => [p[0], 2 * cv - p[1]],
};
// snapshot / restore corner uvs (for interactive drags)
export function snapshotCorners(pm, ids) { const ci = cornerIndex(pm); return ids.map(id => { const p = cornerUV(pm, ci, id); return p ? [p[0], p[1]] : null; }); }
export function restoreCorners(pm, ids, snap) { const ci = cornerIndex(pm); ids.forEach((id, i) => { const p = cornerUV(pm, ci, id); if (p && snap[i]) { p[0] = snap[i][0]; p[1] = snap[i][1]; } }); }
// fit corners into the 0..1 square (uniform scale, keeps aspect)
export function normalizeCorners(pm, ids, { keepAspect = true } = {}) {
  const b = uvBounds(pm, ids); if (!b) return;
  const w = b.u1 - b.u0 || 1, hh = b.v1 - b.v0 || 1;
  const su = keepAspect ? 1 / Math.max(w, hh) : 1 / w, sv = keepAspect ? su : 1 / hh;
  xformCorners(pm, ids, (p) => [(p[0] - b.u0) * su, (p[1] - b.v0) * sv]);
}
// simple layout: normalize each shell (max dimension = 1), shelf-pack into rows, fit result into 0..1
export function layoutShells(pm, faceShells, { pad = 0.04, margin = 0.01 } = {}) {
  const items = [];
  for (const faces of faceShells) {
    const ids = cornersOfFaces(pm, faces); const b = uvBounds(pm, ids); if (!b) continue;
    const s = 1 / Math.max(b.u1 - b.u0, b.v1 - b.v0, 1e-9);
    items.push({ ids, b, s, w: (b.u1 - b.u0) * s, h: (b.v1 - b.v0) * s });
  }
  if (!items.length) return;
  const area = items.reduce((a, it) => a + (it.w + pad) * (it.h + pad), 0);
  const rowW = Math.max(1 + pad, Math.sqrt(area) * 1.15);
  items.sort((a, b) => b.h - a.h);
  let x = 0, y = 0, rowH = 0, maxW = 0;
  for (const it of items) {
    if (x > 0 && x + it.w > rowW) { y += rowH + pad; x = 0; rowH = 0; }
    it.x = x; it.y = y; x += it.w + pad; rowH = Math.max(rowH, it.h); maxW = Math.max(maxW, x - pad);
  }
  const totalH = y + rowH;
  const S = (1 - 2 * margin) / Math.max(maxW, totalH, 1e-9);
  for (const it of items) xformCorners(pm, it.ids, (p) => [margin + (it.x + (p[0] - it.b.u0) * it.s) * S, margin + (it.y + (p[1] - it.b.v0) * it.s) * S]);
}
// Laplacian relax of UV points; border points (and points not in `movable`) stay pinned
export function relaxUVs(pm, movableIds = null, iterations = 40) {
  const ci = cornerIndex(pm); const { group, count } = uvGroups(pm);
  const nb = Array.from({ length: count }, () => new Set());
  const edgeUse = new Map();
  for (let f = 0; f < pm.f.length; f++) {
    if (!(pm.uv && pm.uv[f])) continue; const n = pm.f[f].length;
    for (let j = 0; j < n; j++) {
      const a = group[ci.base[f] + j], b = group[ci.base[f] + (j + 1) % n]; if (a < 0 || b < 0 || a === b) continue;
      nb[a].add(b); nb[b].add(a);
      const k = a < b ? a + ',' + b : b + ',' + a; edgeUse.set(k, (edgeUse.get(k) || 0) + 1);
    }
  }
  const pinned = new Uint8Array(count);
  for (const [k, n] of edgeUse) if (n === 1) { const [a, b] = k.split(',').map(Number); pinned[a] = 1; pinned[b] = 1; }
  if (movableIds) { const mv = new Uint8Array(count); for (const id of movableIds) if (group[id] >= 0) mv[group[id]] = 1; for (let g = 0; g < count; g++) if (!mv[g]) pinned[g] = 1; }
  const pos = new Float64Array(count * 2); const members = Array.from({ length: count }, () => []);
  for (let id = 0; id < ci.n; id++) { const g = group[id]; if (g < 0) continue; members[g].push(id); const p = cornerUV(pm, ci, id); pos[g * 2] = p[0]; pos[g * 2 + 1] = p[1]; }
  const nxt = new Float64Array(pos);
  for (let it = 0; it < iterations; it++) {
    for (let g = 0; g < count; g++) {
      if (pinned[g] || !nb[g].size) { nxt[g * 2] = pos[g * 2]; nxt[g * 2 + 1] = pos[g * 2 + 1]; continue; }
      let su = 0, sv = 0; for (const o of nb[g]) { su += pos[o * 2]; sv += pos[o * 2 + 1]; }
      nxt[g * 2] = su / nb[g].size; nxt[g * 2 + 1] = sv / nb[g].size;
    }
    pos.set(nxt);
  }
  for (let g = 0; g < count; g++) for (const id of members[g]) { const p = cornerUV(pm, ci, id); p[0] = pos[g * 2]; p[1] = pos[g * 2 + 1]; }
}
// signed area of a face in UV space (negative = flipped)
export function faceUVArea(uvs) { let a = 0; for (let i = 0; i < uvs.length; i++) { const p = uvs[i], q = uvs[(i + 1) % uvs.length]; a += p[0] * q[1] - q[0] * p[1]; } return a / 2; }
