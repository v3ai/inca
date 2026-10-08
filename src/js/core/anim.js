// Inca — keyframe animation: anim curves with Maya-style tangents and infinity modes.
import { App } from './app.js';
import { getAttr, setAttr, CHANNELS, resolveAttr, nodeName } from './scene.js';

export const TANGENTS = ['auto', 'spline', 'clamped', 'linear', 'flat', 'step', 'plateau', 'fixed'];
export const INFINITY = ['constant', 'linear', 'cycle', 'cycleRelative', 'oscillate'];

export class AnimCurve {
  constructor(node, attr) { this.node = node; this.attr = attr; this.keys = []; this.pre = 'constant'; this.post = 'constant'; this.weighted = false; }
  sort() { this.keys.sort((a, b) => a.t - b.t); }
  find(t) { return this.keys.findIndex(k => Math.abs(k.t - t) < 1e-4); }
  setKey(t, v, o = {}) {
    const i = this.find(t);
    const def = App.prefs?.defaultInTangent || 'auto', defo = App.prefs?.defaultOutTangent || 'auto';
    if (i >= 0) { this.keys[i].v = v; if (o.it) this.keys[i].it = o.it; if (o.ot) this.keys[i].ot = o.ot; return this.keys[i]; }
    const k = { t, v, it: o.it || def, ot: o.ot || (def === 'step' ? 'step' : defo), is: 0, os: 0 };
    this.keys.push(k); this.sort(); return k;
  }
  removeAt(t) { const i = this.find(t); if (i >= 0) this.keys.splice(i, 1); return i >= 0; }
  slopes(i) {
    const K = this.keys, k = K[i], p = K[i - 1], n = K[i + 1];
    const lin = (a, b) => (b && a) ? (b.v - a.v) / (b.t - a.t || 1) : 0;
    const one = (type, side) => {
      if (type === 'fixed') return side === 'in' ? k.is : k.os;
      if (type === 'flat' || type === 'step' || type === 'stepnext') return 0;
      if (type === 'linear') return side === 'in' ? (p ? lin(p, k) : (n ? lin(k, n) : 0)) : (n ? lin(k, n) : (p ? lin(p, k) : 0));
      // spline family
      if (!p && !n) return 0;
      let s = (!p) ? lin(k, n) : (!n) ? lin(p, k) : (n.v - p.v) / (n.t - p.t || 1);
      if (type === 'spline') return s;
      if (!p || !n) { if (type === 'auto' || type === 'plateau' || type === 'clamped') return (type === 'clamped') ? s : 0; return s; }
      const d0 = k.v - p.v, d1 = n.v - k.v;
      if (type === 'clamped') { if (Math.abs(d0) < 1e-6 || Math.abs(d1) < 1e-6 || d0 * d1 < 0) return 0; return s; }
      // auto / plateau: flatten at extremes and prevent overshoot
      if (d0 * d1 <= 0) return 0;
      const m0 = d0 / (k.t - p.t), m1 = d1 / (n.t - k.t);
      const lim = 3 * Math.min(Math.abs(m0), Math.abs(m1));
      if (Math.abs(s) > lim) s = Math.sign(s) * lim;
      return s;
    };
    return [one(k.it, 'in'), one(k.ot, 'out')];
  }
  evalRaw(t) {
    const K = this.keys; const n = K.length;
    if (!n) return 0; if (n === 1) return K[0].v;
    if (t <= K[0].t) return K[0].v; if (t >= K[n - 1].t) return K[n - 1].v;
    let lo = 0, hi = n - 1;
    while (hi - lo > 1) { const m = (lo + hi) >> 1; if (K[m].t <= t) lo = m; else hi = m; }
    const a = K[lo], b = K[hi];
    if (a.ot === 'step') return a.v;
    if (a.ot === 'stepnext') return b.v;
    const dt = b.t - a.t; const s = (t - a.t) / dt;
    const m0 = this.slopes(lo)[1] * dt, m1 = this.slopes(hi)[0] * dt;
    const s2 = s * s, s3 = s2 * s;
    return (2 * s3 - 3 * s2 + 1) * a.v + (s3 - 2 * s2 + s) * m0 + (-2 * s3 + 3 * s2) * b.v + (s3 - s2) * m1;
  }
  eval(t) {
    const K = this.keys; const n = K.length;
    if (!n) return 0; if (n === 1) return K[0].v;
    const t0 = K[0].t, t1 = K[n - 1].t, span = t1 - t0 || 1;
    if (t < t0) return this.inf(t, this.pre, true);
    if (t > t1) return this.inf(t, this.post, false);
    return this.evalRaw(t);
  }
  inf(t, mode, before) {
    const K = this.keys; const n = K.length; const t0 = K[0].t, t1 = K[n - 1].t, span = t1 - t0 || 1;
    const v0 = K[0].v, v1 = K[n - 1].v;
    if (mode === 'linear') { if (before) return v0 + (t - t0) * this.slopes(0)[0]; return v1 + (t - t1) * this.slopes(n - 1)[1]; }
    if (mode === 'cycle' || mode === 'cycleRelative' || mode === 'oscillate') {
      const c = Math.floor((t - t0) / span); let lt = t - c * span;
      if (mode === 'oscillate' && (c % 2 !== 0)) lt = t1 - (lt - t0);
      const off = mode === 'cycleRelative' ? c * (v1 - v0) : 0;
      return this.evalRaw(lt) + off;
    }
    return before ? v0 : v1;
  }
  toJSON() { return { node: this.node, attr: this.attr, keys: this.keys.map(k => ({ t: k.t, v: k.v, it: k.it, ot: k.ot, is: k.is, os: k.os })), pre: this.pre, post: this.post }; }
}

const key = (id, attr) => id + '.' + attr;
export const Anim = {
  curves: new Map(),
  clear() { this.curves.clear(); },
  serialize() { return [...this.curves.values()].map(c => c.toJSON()); },
  load(list) {
    this.curves.clear();
    for (const d of list || []) { const c = new AnimCurve(d.node, d.attr); c.keys = d.keys.map(k => ({ ...k })); c.pre = d.pre || 'constant'; c.post = d.post || 'constant'; this.curves.set(key(d.node, d.attr), c); }
  },
  curve(node, attr, create = false) {
    const id = node.isObject3D ? node.inca.id : node.id; attr = resolveAttr(node, attr);
    let c = this.curves.get(key(id, attr));
    if (!c && create) { c = new AnimCurve(id, attr); this.curves.set(key(id, attr), c); }
    return c || null;
  },
  curvesOf(node) { const id = node.isObject3D ? node.inca.id : node.id; return [...this.curves.values()].filter(c => c.node === id); },
  isKeyed(node, attr) { const c = this.curve(node, attr); return !!(c && c.keys.length); },
  onKey(node, attr, t = App.time.current) { const c = this.curve(node, attr); return !!(c && c.find(t) >= 0); },
  setKey(node, attr, opts = {}) {
    const t = opts.t ?? App.time.current;
    const v = opts.v ?? getAttr(node, attr);
    if (typeof v !== 'number' && typeof v !== 'boolean') return null;
    const c = this.curve(node, attr, true);
    c.setKey(t, +v, opts);
    App.dirty('timeline', 'channels', 'graph');
    return c;
  },
  keyableAttrs(node) {
    if (node.isObject3D) {
      const list = CHANNELS.slice();
      if (node.inca.light) list.push('intensity');
      return list.filter(a => !node.inca.locks[a]);
    }
    return [];
  },
  keyNode(node, attrs = null, opts = {}) { for (const a of attrs || this.keyableAttrs(node)) this.setKey(node, a, opts); },
  deleteCurve(node, attr) { const id = node.isObject3D ? node.inca.id : node.id; this.curves.delete(key(id, resolveAttr(node, attr))); App.dirty('timeline', 'channels', 'graph'); },
  removeNode(id) { for (const k of [...this.curves.keys()]) if (k.startsWith(id + '.')) this.curves.delete(k); },
  keyTimes(nodes) {
    const s = new Set();
    for (const n of nodes) for (const c of this.curvesOf(n)) for (const k of c.keys) s.add(k.t);
    return [...s].sort((a, b) => a - b);
  },
  deleteKeysAt(nodes, t) { for (const n of nodes) for (const c of this.curvesOf(n)) { c.removeAt(t); if (!c.keys.length) this.curves.delete(key(c.node, c.attr)); } App.dirty('timeline', 'graph', 'channels'); },
  moveKeys(nodes, from, to) {
    for (const n of nodes) for (const c of this.curvesOf(n)) { const i = c.find(from); if (i >= 0) { c.removeAt(to); const k = c.keys.find(k => Math.abs(k.t - from) < 1e-4); if (k) k.t = to; c.sort(); } }
    App.dirty('timeline', 'graph');
  },
  _clip: null,
  copyKeys(nodes, t = null) {
    this._clip = []; for (const n of nodes) for (const c of this.curvesOf(n)) { const ks = t === null ? c.keys : c.keys.filter(k => Math.abs(k.t - t) < 1e-4); if (ks.length) this._clip.push({ attr: c.attr, keys: ks.map(k => ({ ...k })) }); }
    if (this._clip.length) { const t0 = Math.min(...this._clip.flatMap(c => c.keys.map(k => k.t))); for (const c of this._clip) for (const k of c.keys) k.t -= t0; }
  },
  pasteKeys(nodes, t = App.time.current) {
    if (!this._clip) return;
    for (const n of nodes) for (const c of this._clip) { const ac = this.curve(n, c.attr, true); for (const k of c.keys) { const nk = ac.setKey(k.t + t, k.v); nk.it = k.it; nk.ot = k.ot; } }
    App.dirty('timeline', 'graph', 'channels');
  },
  evaluate(t = App.time.current) {
    let any = false;
    for (const c of this.curves.values()) {
      if (!c.keys.length) continue;
      const node = App.nodes.get(c.node); if (!node) continue;
      let v = c.eval(t);
      if (c.attr === 'visibility') v = v >= 0.5 ? 1 : 0;
      setAttr(node, c.attr, v, { silent: true, force: true });
      any = true;
    }
    if (any) App.dirty('channels');
  },
  autoKey(node, attrs) {
    if (!App.time.autoKey) return;
    for (const a of attrs) { if (this.isKeyed(node, a)) this.setKey(node, a); }
  },
  nextKeyTime(nodes, t, dir = 1) {
    const ts = this.keyTimes(nodes); if (!ts.length) return null;
    if (dir > 0) { const n = ts.find(x => x > t + 1e-4); return n ?? ts[0]; }
    const p = [...ts].reverse().find(x => x < t - 1e-4); return p ?? ts[ts.length - 1];
  },
  curveName(c) { const n = App.nodes.get(c.node); return nodeName(n) + '_' + c.attr; },
};
App.anim = Anim;
