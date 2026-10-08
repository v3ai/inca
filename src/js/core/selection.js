// Inca — selection management (objects and components)
import { App } from './app.js';
import { allDag, dagChildren, isDag } from './scene.js';
import { edgeRing, ekey } from './polymesh.js';

const emitSel = () => { App.emit('selectionChanged'); App.dirty('outliner', 'channels', 'attr', 'timeline', 'graph', 'viewport'); };
export const Sel = {
  objects() { return App.sel.slice(); },
  lead() { return App.sel[App.sel.length - 1] || null; },
  meshes() { return App.sel.filter(o => o.inca.kind === 'mesh'); },
  select(list, mode = 'replace', { echo = true } = {}) {
    list = (list || []).filter(o => isDag(o) && !(o.inca.template) && o.inca.selectable !== false);
    if (mode === 'replace') App.sel = [...new Set(list)];
    else if (mode === 'add') { for (const o of list) { App.sel = App.sel.filter(s => s !== o); App.sel.push(o); } }
    else if (mode === 'toggle') { for (const o of list) { if (App.sel.includes(o)) App.sel = App.sel.filter(s => s !== o); else App.sel.push(o); } }
    else if (mode === 'deselect') App.sel = App.sel.filter(s => !list.includes(s));
    if (App.compMode && mode === 'replace' && list.length) { /* changing object selection in component mode re-hilites */ }
    if (echo) App.emit('echo', App.sel.length ? `select -${mode === 'replace' ? 'r' : mode === 'add' ? 'add' : mode === 'toggle' ? 'tgl' : 'd'} ${list.map(o => o.inca.name).join(' ')};` : 'select -cl;');
    emitSel();
  },
  clear() { App.sel = []; if (App.compMode) for (const o of App.hilite) clearComp(o); emitSel(); },
  // ---- component mode ----
  setMode(mode) {
    // mode: null(object) | 'vertex' | 'edge' | 'face' | 'uv'
    if (mode && mode !== 'object') {
      if (!App.compMode) {
        const cand = App.sel.filter(o => o.inca.kind === 'mesh' || o.inca.kind === 'curve');
        const hl = cand.length ? cand : App.hilite;
        if (!hl.length) { App.help('Select an object before entering component mode'); App.compMode = mode; App.hilite = []; emitSel(); return; }
        App.hilite = hl;
      }
      const prev = App.compMode;
      if (prev && prev !== mode) for (const o of App.hilite) convertComp(o, prev, mode === 'uv' ? 'vertex' : mode);
      App.compMode = mode;
      for (const o of App.hilite) { if (!o.inca.compSel) o.inca.compSel = { v: new Set(), e: new Set(), f: new Set() }; o.userData.compDirty = true; }
      App.emit('echo', `selectType -${mode === 'vertex' ? 'v' : mode === 'edge' ? 'e' : mode === 'face' ? 'f' : 'puv'} 1;`);
    } else {
      // back to object mode: objects with components selected stay selected
      if (App.compMode) {
        const keep = App.hilite.slice();
        for (const o of App.hilite) { o.userData.compDirty = true; }
        App.hilite = []; App.compMode = null;
        if (keep.length) App.sel = [...new Set([...App.sel.filter(s => !keep.includes(s)), ...keep])];
      }
      App.compMode = null;
    }
    emitSel();
  },
  hilite(objs) { App.hilite = objs.filter(o => o.inca.kind === 'mesh' || o.inca.kind === 'curve'); for (const o of App.hilite) { if (!o.inca.compSel) o.inca.compSel = { v: new Set(), e: new Set(), f: new Set() }; o.userData.compDirty = true; } emitSel(); },
  compKey(mode = App.compMode) { return mode === 'edge' ? 'e' : mode === 'face' ? 'f' : 'v'; },
  selectComponents(entries /* [{o, ids}] */, mode = 'replace') {
    const k = this.compKey();
    if (mode === 'replace') for (const o of App.hilite) o.inca.compSel[k].clear();
    for (const { o, ids } of entries) {
      if (!App.hilite.includes(o)) { App.hilite.push(o); if (!o.inca.compSel) o.inca.compSel = { v: new Set(), e: new Set(), f: new Set() }; }
      const s = o.inca.compSel[k];
      for (const i of ids) { if (mode === 'deselect') s.delete(i); else if (mode === 'toggle') { if (s.has(i)) s.delete(i); else s.add(i); } else s.add(i); }
      o.userData.compDirty = true;
    }
    for (const o of App.hilite) o.userData.compDirty = true;
    const desc = entries.filter(e => e.ids.length).map(({ o, ids }) => `${o.inca.name}.${k === 'v' ? (o.inca.kind === 'curve' ? 'cv' : 'vtx') : k === 'e' ? 'e' : 'f'}[${compactRange(ids)}]`).join(' ');
    if (desc) App.emit('echo', `select -${mode === 'replace' ? 'r' : mode === 'add' ? 'add' : mode === 'toggle' ? 'tgl' : 'd'} ${desc};`);
    emitSel();
  },
  anyComponents() { const k = this.compKey(); return App.hilite.some(o => o.inca.compSel && o.inca.compSel[k].size); },
  // vertices affected by the current component selection
  affectedVerts(o) {
    const cs = o.inca.compSel; if (!cs) return [];
    if (o.inca.kind === 'curve') return [...cs.v];
    const pm = o.inca.mesh; const s = new Set();
    if (App.compMode === 'vertex' || App.compMode === 'uv') for (const v of cs.v) s.add(v);
    else if (App.compMode === 'edge') { const E = pm.topo.edges; for (const e of cs.e) if (E[e]) { s.add(E[e][0]); s.add(E[e][1]); } }
    else if (App.compMode === 'face') for (const f of cs.f) if (pm.f[f]) for (const v of pm.f[f]) s.add(v);
    return [...s];
  },
  edgesAsPairs(o) { const E = o.inca.mesh.topo.edges; return [...o.inca.compSel.e].filter(e => E[e]).map(e => E[e].slice()); },
  selectAll() {
    if (App.compMode) { const k = this.compKey(); this.selectComponents(App.hilite.map(o => ({ o, ids: allIds(o, k) })), 'replace'); return; }
    this.select(allDag().filter(o => o.visible), 'replace');
  },
  invert() {
    if (App.compMode) { const k = this.compKey(); for (const o of App.hilite) { const s = o.inca.compSel[k]; const all = allIds(o, k); o.inca.compSel[k] = new Set(all.filter(i => !s.has(i))); o.userData.compDirty = true; } emitSel(); return; }
    const top = dagChildren(App.world).filter(o => !o.inca.startup && o.visible);
    this.select(top.filter(o => !App.sel.includes(o)), 'replace');
  },
  byType(kind) { this.select(allDag().filter(o => o.inca.kind === kind || (kind === 'geometry' && (o.inca.kind === 'mesh' || o.inca.kind === 'curve'))), 'replace'); },
  hierarchy() { const out = []; for (const o of App.sel) o.traverse(c => { if (c.inca && !c.inca.startup) out.push(c); }); this.select(out, 'replace'); },
  grow(dir = 1) {
    if (!App.compMode) return;
    for (const o of App.hilite) {
      if (o.inca.kind !== 'mesh') continue;
      const pm = o.inca.mesh; const t = pm.topo; const cs = o.inca.compSel; const k = this.compKey();
      if (dir > 0) {
        if (k === 'v') { const add = new Set(cs.v); for (const v of cs.v) for (const e of t.vertEdges[v] || []) { add.add(t.edges[e][0]); add.add(t.edges[e][1]); } cs.v = add; }
        if (k === 'f') { const vs = new Set(); for (const f of cs.f) for (const v of pm.f[f] || []) vs.add(v); for (const v of vs) for (const f of t.vertFaces[v]) cs.f.add(f); }
        if (k === 'e') { const vs = new Set(); for (const e of cs.e) { vs.add(t.edges[e][0]); vs.add(t.edges[e][1]); } for (const v of vs) for (const e of t.vertEdges[v]) cs.e.add(e); }
      } else {
        // shrink: remove components on the selection border
        if (k === 'v') { const keep = new Set(); for (const v of cs.v) { if ((t.vertEdges[v] || []).every(e => cs.v.has(t.edges[e][0]) && cs.v.has(t.edges[e][1]))) keep.add(v); } cs.v = keep; }
        if (k === 'f') { const keep = new Set(); for (const f of cs.f) { if (pm.f[f].every(v => t.vertFaces[v].every(g => cs.f.has(g)))) keep.add(f); } cs.f = keep; }
        if (k === 'e') { const vc = new Map(); for (const e of cs.e) for (const v of t.edges[e]) vc.set(v, (vc.get(v) || 0) + 1); const keep = new Set(); for (const e of cs.e) { if (t.edges[e].every(v => t.vertEdges[v].every(x => cs.e.has(x)))) keep.add(e); } cs.e = keep; }
      }
      o.userData.compDirty = true;
    }
    emitSel();
  },
  edgeLoop(o, ei, add = false) {
    const ids = edgeLoopIds(o.inca.mesh, ei);
    this.selectComponents([{ o, ids }], add ? 'add' : 'replace');
  },
  edgeRingSel(o, ei, add = false) {
    const pm = o.inca.mesh; const [a, b] = pm.topo.edges[ei];
    const r = edgeRing(pm, a, b); const ids = r.edges.map(e => pm.edgeIndex(e.a, e.b)).filter(i => i >= 0);
    this.selectComponents([{ o, ids }], add ? 'add' : 'replace');
  },
  faceLoop(o, ei, add = false) {
    const pm = o.inca.mesh; const [a, b] = pm.topo.edges[ei]; const r = edgeRing(pm, a, b);
    this.setMode('face'); this.selectComponents([{ o, ids: r.faces }], add ? 'add' : 'replace');
  },
  border() {
    for (const o of App.hilite) {
      if (o.inca.kind !== 'mesh') continue;
      const t = o.inca.mesh.topo; const cs = o.inca.compSel;
      const bes = t.edges.map((_, i) => i).filter(i => t.edgeFaces[i].length === 1);
      if (App.compMode === 'edge') cs.e = new Set(bes);
      else if (App.compMode === 'vertex') { cs.v = new Set(); for (const e of bes) { cs.v.add(t.edges[e][0]); cs.v.add(t.edges[e][1]); } }
      o.userData.compDirty = true;
    }
    emitSel();
  },
  convertTo(target) { // 'vertex' | 'edge' | 'face'
    const from = App.compMode || 'object';
    if (from === 'object') {
      const objs = App.sel.filter(o => o.inca.kind === 'mesh');
      App.hilite = objs; App.compMode = target;
      for (const o of objs) { o.inca.compSel = o.inca.compSel || { v: new Set(), e: new Set(), f: new Set() }; o.inca.compSel[this.compKey(target)] = new Set(allIds(o, this.compKey(target))); o.userData.compDirty = true; }
      emitSel(); return;
    }
    for (const o of App.hilite) convertComp(o, from, target);
    App.compMode = target;
    for (const o of App.hilite) o.userData.compDirty = true;
    emitSel();
  },
  // remap selection after topology changing op: op returns {select}
  applyOpSelect(o, sel) {
    if (!sel || !o.inca.compSel) return;
    const pm = o.inca.mesh; const cs = o.inca.compSel;
    cs.v.clear(); cs.e.clear(); cs.f.clear();
    if (sel.type === 'f') { for (const i of sel.ids) cs.f.add(i); if (App.compMode && App.compMode !== 'face') App.compMode = 'face'; }
    else if (sel.type === 'e') { const ids = sel.pairs ? sel.pairs.map(([a, b]) => pm.edgeIndex(a, b)).filter(i => i >= 0) : sel.ids; for (const i of ids) cs.e.add(i); if (App.compMode && App.compMode !== 'edge') App.compMode = 'edge'; }
    else if (sel.type === 'v') { for (const i of sel.ids) cs.v.add(i); }
    o.userData.compDirty = true;
  },
  pruneInvalid(o) {
    if (!o.inca.compSel || o.inca.kind !== 'mesh') return;
    const pm = o.inca.mesh; const cs = o.inca.compSel;
    cs.v = new Set([...cs.v].filter(i => i < pm.v.length)); cs.f = new Set([...cs.f].filter(i => i < pm.f.length)); cs.e = new Set([...cs.e].filter(i => i < pm.topo.edges.length));
  },
};
function clearComp(o) { if (o.inca.compSel) { o.inca.compSel.v.clear(); o.inca.compSel.e.clear(); o.inca.compSel.f.clear(); o.userData.compDirty = true; } }
function allIds(o, k) {
  if (o.inca.kind === 'curve') return k === 'v' ? o.inca.curve.cvs.map((_, i) => i) : [];
  const pm = o.inca.mesh; const n = k === 'v' ? pm.v.length : k === 'e' ? pm.topo.edges.length : pm.f.length;
  return Array.from({ length: n }, (_, i) => i);
}
function convertComp(o, from, to) {
  if (o.inca.kind !== 'mesh' || from === to) return;
  const pm = o.inca.mesh; const t = pm.topo; const cs = o.inca.compSel;
  const fk = from === 'edge' ? 'e' : from === 'face' ? 'f' : 'v', tk = to === 'edge' ? 'e' : to === 'face' ? 'f' : 'v';
  if (!cs[fk].size) return;
  let verts = new Set();
  if (fk === 'v') verts = new Set(cs.v); else if (fk === 'e') for (const e of cs.e) { if (t.edges[e]) { verts.add(t.edges[e][0]); verts.add(t.edges[e][1]); } } else for (const f of cs.f) for (const v of pm.f[f] || []) verts.add(v);
  const out = new Set();
  if (tk === 'v') verts.forEach(v => out.add(v));
  else if (tk === 'e') {
    if (fk === 'f') { for (const f of cs.f) { const F = pm.f[f]; for (let i = 0; i < F.length; i++) out.add(pm.edgeIndex(F[i], F[(i + 1) % F.length])); } }
    else if (fk === 'v') { t.edges.forEach(([a, b], i) => { if (verts.has(a) && verts.has(b)) out.add(i); }); if (!out.size) for (const v of verts) for (const e of t.vertEdges[v]) out.add(e); }
  } else if (tk === 'f') {
    if (fk === 'e') { for (const e of cs.e) for (const f of t.edgeFaces[e] || []) out.add(f); }
    else { pm.f.forEach((F, i) => { if (F.every(v => verts.has(v))) out.add(i); }); if (!out.size) for (const v of verts) for (const f of t.vertFaces[v]) out.add(f); }
  }
  cs[tk] = out;
}
export function edgeLoopIds(pm, ei) {
  const t = pm.topo; const out = new Set([ei]);
  const walk = (e, from) => {
    let cur = e, v = from, guard = 0;
    while (guard++ < 100000) {
      const [a, b] = t.edges[cur]; const nv = a === v ? b : a; // continue through nv
      const es = t.vertEdges[nv]; if (es.length !== 4) break;
      const curFaces = new Set(t.edgeFaces[cur]);
      const cand = es.filter(x => x !== cur && !t.edgeFaces[x].some(f => curFaces.has(f)));
      if (cand.length !== 1) break;
      const nx = cand[0]; if (out.has(nx)) break;
      out.add(nx); cur = nx; v = nv;
    }
  };
  const [a, b] = t.edges[ei];
  walk(ei, a); walk(ei, b);
  return [...out];
}
function compactRange(ids) {
  const s = [...ids].sort((a, b) => a - b); const parts = []; let i = 0;
  while (i < s.length) { let j = i; while (j + 1 < s.length && s[j + 1] === s[j] + 1) j++; parts.push(i === j ? `${s[i]}` : `${s[i]}:${s[j]}`); i = j + 1; if (parts.length > 4) { parts.push('...'); break; } }
  return parts.join(',');
}
App.selApi = Sel;
