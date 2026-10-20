// Inca — direct attribute connections (Maya's connectAttr): the destination attribute follows the source attribute.
// App.conns = [{ src, srcAttr, dst, dstAttr }] (node ids); saved with the scene, so undo covers them.
// A texture's outColor connected to a material attribute is the material's texture map (mat.maps), as in the Hypershade.
import { App } from './app.js';
import * as S from './scene.js';
import { OPS, niceName } from './ops.js';
import { MAPPABLE, updateMaterial } from './materials.js';
import { Rig } from './rigging.js';

App.conns = App.conns || [];

export const recById = (id) => App.nodes.get(id) || null;
export const recId = (rec) => rec ? (rec.isObject3D ? rec.inca.id : rec.id) : null;
const nm = (rec) => S.nodeName(rec);

// ------------------------------------------------------------------ attribute lists
// { name, label, type, keyable, readable, writable, children?, parent?, mappable? }
// types: float int bool enum (scalars), double3 / float3 (vectors, children X Y Z / R G B), texColor (texture outColor)
const SCALAR = new Set(['float', 'int', 'bool', 'enum']);
const VECTOR = new Set(['double3', 'float3']);
const vec = (name, type, kids, o = {}) => ({ name, type, keyable: true, readable: true, writable: true, ...o, children: kids.map(k => ({ name: name + k, type: 'float', keyable: o.keyable !== false, readable: true, writable: o.writable !== false, parent: name })) });
const sc = (name, type = 'float', o = {}) => ({ name, type, keyable: true, readable: true, writable: true, ...o });
// a value bag (light / camera / material / texture attrs) -> attribute list
function bagAttrs(bag, { keyable = () => true, mappable = [] } = {}) {
  const out = [];
  for (const [k, v] of Object.entries(bag || {})) {
    if (typeof v === 'number') out.push(sc(k, 'float', { keyable: keyable(k), mappable: mappable.includes(k) }));
    else if (typeof v === 'boolean') out.push(sc(k, 'bool', { keyable: keyable(k) }));
    else if (Array.isArray(v) && v.length === 3 && v.every(x => typeof x === 'number')) out.push(vec(k, 'float3', ['R', 'G', 'B'], { keyable: keyable(k), mappable: mappable.includes(k) }));
  }
  return out;
}
export function listAttrs(rec) {
  if (!rec) return [];
  if (rec.isObject3D) {
    const inc = rec.inca; const out = []; const XYZ = ['X', 'Y', 'Z'];
    // read-only outputs (Maya's boundingBox / center), then the transform's own attributes in Maya's order
    const ro = { keyable: false, writable: false };
    out.push(vec('boundingBoxMin', 'double3', XYZ, ro), vec('boundingBoxMax', 'double3', XYZ, ro), vec('boundingBoxSize', 'double3', XYZ, ro), vec('center', 'double3', XYZ, ro));
    out.push(sc('visibility', 'bool'), sc('template', 'bool', { keyable: false }));
    out.push(vec('translate', 'double3', XYZ), vec('rotate', 'double3', XYZ), vec('scale', 'double3', XYZ));
    out.push(vec('rotatePivot', 'double3', XYZ, { keyable: false }), vec('rotatePivotTranslate', 'double3', XYZ, { keyable: false }));
    if (inc.kind === 'joint') out.push(vec('jointOrient', 'double3', XYZ, { keyable: false }), sc('radius', 'float', { keyable: false }));
    for (const [k, v] of Object.entries(inc.extraAttrs || {})) if (typeof v === 'number' || typeof v === 'boolean') out.push(sc(k, typeof v === 'boolean' ? 'bool' : 'float'));
    if (inc.light) out.push(...bagAttrs(inc.light));
    if (inc.cam) out.push(...bagAttrs(inc.cam, { keyable: (k) => k === 'focalLength' }));
    if (inc.kind === 'mesh') out.push(sc('smoothLevel', 'int', { keyable: false }));
    return out;
  }
  if (rec.kind === 'history') {
    const out = [];
    for (const ch of OPS[rec.type]?.channels || []) {
      if (ch.type === 'enum' && ch.values) continue; // string-valued enums can't take a number
      out.push(sc(ch.k, ch.type || 'float', { label: ch.label }));
    }
    return out;
  }
  if (rec.kind === 'material') return bagAttrs(rec.attrs, { mappable: MAPPABLE[rec.type] || [] }).concat(
    (MAPPABLE[rec.type] || []).filter(k => !(k in rec.attrs)).map(k => sc(k, 'float', { mappable: true, writable: false })));
  if (rec.kind === 'texture') return [{ name: 'outColor', type: 'texColor', keyable: false, readable: true, writable: false }, ...bagAttrs(rec.attrs)];
  return [];
}
export function attrInfo(rec, attr) {
  attr = S.resolveAttr(rec, attr);
  for (const a of listAttrs(rec)) { if (a.name === attr) return a; for (const c of a.children || []) if (c.name === attr) return c; }
  return null;
}
export const attrLabel = (a) => a.label || niceName(a.name);
// Hypergraph line colours, as in Maya: green = triple (vector / colour), dark blue = single
export const attrWidth = (a) => !a ? 1 : VECTOR.has(a.type) || a.type === 'texColor' ? 3 : 1;
// can src (an output) drive dst (an input)?
export function compatible(sa, da) {
  if (!sa || !da) return false;
  if (sa.type === 'texColor') return !!da.mappable;
  if (!da.writable) return false;
  if (SCALAR.has(sa.type)) return SCALAR.has(da.type);
  if (VECTOR.has(sa.type)) return VECTOR.has(da.type);
  return false;
}

// ------------------------------------------------------------------ queries
const isMap = (src, srcAttr, dst) => src?.kind === 'texture' && srcAttr === 'outColor' && dst?.kind === 'material';
// all connections, texture maps included: [{ src, srcAttr, dst, dstAttr, map? }]
export function allConnections() {
  const out = App.conns.slice();
  for (const m of App.mats?.values?.() || []) for (const [a, tid] of Object.entries(m.maps || {})) if (App.nodes.has(tid)) out.push({ src: tid, srcAttr: 'outColor', dst: m.id, dstAttr: a, map: true });
  return out;
}
// the connection driving rec.attr (or one of its children / its parent compound), if any
export function inputConn(rec, attr) {
  const id = recId(rec); attr = S.resolveAttr(rec, attr);
  return allConnections().find(c => c.dst === id && c.dstAttr === attr) || null;
}
export function connectionsOf(rec, { asSource = true, asDest = true } = {}) {
  const id = recId(rec);
  return allConnections().filter(c => (asSource && c.src === id) || (asDest && c.dst === id));
}
export const isConnected = (src, srcAttr, dst, dstAttr) => allConnections().some(c => c.src === recId(src) && c.srcAttr === srcAttr && c.dst === recId(dst) && c.dstAttr === dstAttr);
// is this attribute (or a child / the parent compound) used by a connection on that side?
export function attrConnected(rec, attr, side) {
  const id = recId(rec); const a = attrInfo(rec, attr); const fam = new Set([attr, ...(a?.children || []).map(c => c.name), a?.parent].filter(Boolean));
  return allConnections().some(c => side === 'src' ? c.src === id && fam.has(c.srcAttr) : c.dst === id && fam.has(c.dstAttr));
}

// ------------------------------------------------------------------ make / break
const echo = (s) => App.emit('echo', s);
export function connectAttr(src, srcAttr, dst, dstAttr, { force = false, quiet = false } = {}) {
  srcAttr = S.resolveAttr(src, srcAttr); dstAttr = S.resolveAttr(dst, dstAttr);
  const sa = attrInfo(src, srcAttr), da = attrInfo(dst, dstAttr);
  if (!sa) throw new Error(`The source attribute '${nm(src)}.${srcAttr}' cannot be found.`);
  if (!da) throw new Error(`The destination attribute '${nm(dst)}.${dstAttr}' cannot be found.`);
  if (src === dst && srcAttr === dstAttr) throw new Error(`Cannot connect '${nm(src)}.${srcAttr}' to itself.`);
  if (!compatible(sa, da)) throw new Error(`The data types of '${nm(src)}.${srcAttr}' and '${nm(dst)}.${dstAttr}' are not compatible.`);
  if (S.isLocked(dst, dstAttr)) throw new Error(`The destination attribute '${nm(dst)}.${dstAttr}' is locked.`);
  const cur = inputConn(dst, dstAttr);
  if (cur && cur.src === recId(src) && cur.srcAttr === srcAttr) return false;
  if (cur && !force) throw new Error(`'${nm(dst)}.${dstAttr}' is already connected to '${nm(recById(cur.src))}.${cur.srcAttr}'.`);
  if (cur) breakConn(cur);
  if (isMap(src, srcAttr, dst)) { dst.maps = dst.maps || {}; dst.maps[dstAttr] = src.id; updateMaterial(dst); App.dirty('hypershade'); }
  else App.conns.push({ src: recId(src), srcAttr, dst: recId(dst), dstAttr });
  if (!quiet) echo(`connectAttr${force ? ' -f' : ''} ${nm(src)}.${srcAttr} ${nm(dst)}.${dstAttr};`);
  evaluateConnections(); App.dirty('channels', 'attr', 'connections'); App.requestRender();
  return true;
}
function breakConn(c) {
  if (c.map) { const m = recById(c.dst); if (m?.maps) { delete m.maps[c.dstAttr]; updateMaterial(m); App.dirty('hypershade'); } return; }
  const i = App.conns.findIndex(x => x.src === c.src && x.srcAttr === c.srcAttr && x.dst === c.dst && x.dstAttr === c.dstAttr); if (i >= 0) App.conns.splice(i, 1);
}
export function disconnectAttr(src, srcAttr, dst, dstAttr, { quiet = false } = {}) {
  srcAttr = S.resolveAttr(src, srcAttr); dstAttr = S.resolveAttr(dst, dstAttr);
  const c = allConnections().find(x => x.src === recId(src) && x.srcAttr === srcAttr && x.dst === recId(dst) && x.dstAttr === dstAttr);
  if (!c) throw new Error(`There is no connection from '${nm(src)}.${srcAttr}' to '${nm(dst)}.${dstAttr}' to disconnect.`);
  breakConn(c);
  if (!quiet) echo(`disconnectAttr ${nm(src)}.${srcAttr} ${nm(dst)}.${dstAttr};`);
  App.dirty('channels', 'attr', 'connections'); App.requestRender();
  return true;
}
// break whatever drives rec.attr (Channel Box > Break Connections); the attribute keeps its current value
export function disconnectInput(rec, attr) {
  const c = inputConn(rec, attr) || (attrInfo(rec, attr)?.parent && inputConn(rec, attrInfo(rec, attr).parent)); if (!c) return false;
  disconnectAttr(recById(c.src), c.srcAttr, rec, c.dstAttr);
  return true;
}

// ------------------------------------------------------------------ evaluation (a rig pass: after keys, before constraints)
const same = (a, b) => Array.isArray(a) ? Array.isArray(b) && a.length === b.length && a.every((x, i) => Math.abs(x - b[i]) < 1e-9) : Math.abs(+a - +b) < 1e-9;
export function evaluateConnections() {
  let changed = false;
  // a few sweeps so chains (a -> b -> c) settle within one evaluation
  for (let sweep = 0; sweep < 4; sweep++) {
    let any = false;
    for (let i = App.conns.length - 1; i >= 0; i--) {
      const c = App.conns[i]; const src = recById(c.src), dst = recById(c.dst);
      if (!src || !dst) { App.conns.splice(i, 1); continue; } // a node was deleted
      let v = S.getAttr(src, c.srcAttr); if (v === undefined) continue;
      const cur = S.getAttr(dst, c.dstAttr); const da = attrInfo(dst, c.dstAttr);
      if (Array.isArray(v)) { if (!Array.isArray(cur)) continue; v = v.slice(); }
      else { v = +v; if (Array.isArray(cur) || isNaN(v)) continue; if (da?.type === 'int' || da?.type === 'enum') v = Math.round(v); if (typeof cur === 'boolean') v = v >= 0.5; }
      if (cur !== undefined && same(v, cur)) continue;
      S.setAttr(dst, c.dstAttr, c.dstAttr === 'visibility' ? (v >= 0.5 ? 1 : 0) : v, { silent: true, force: true });
      any = changed = true;
    }
    if (!any) break;
  }
  return changed;
}
Rig.addPass(5, (ctx) => { if (App.conns.length && evaluateConnections()) ctx.changed = true; });
// again after constraints / IK, for sources they move
Rig.addPass(65, (ctx) => { if (App.conns.length && evaluateConnections()) ctx.changed = true; });

// DAG objects driven by `objs` through connections or constraints, followed downstream (Maya draws them purple)
export function affectedBy(objs) {
  const owner = (r) => r?.isObject3D ? r : r?.kind === 'history' ? S.historyOwner(r) : null;
  const edges = [];
  for (const c of App.conns) { const a = owner(recById(c.src)), b = owner(recById(c.dst)); if (a && b && a !== b) edges.push([a, b]); }
  for (const o of S.allDag()) if (o.inca.kind === 'constraint' && o.inca.rig?.targets) { const d = S.dagParent(o); if (d) for (const t of o.inca.rig.targets) { const a = App.nodes.get(t); if (a && a !== d) edges.push([a, d]); } }
  const out = new Set(); if (!edges.length) return out;
  const todo = [...objs]; const seen = new Set(objs);
  while (todo.length) { const a = todo.pop(); for (const [x, y] of edges) if (x === a && !seen.has(y)) { seen.add(y); out.add(y); todo.push(y); } }
  for (const o of objs) out.delete(o);
  return out;
}

App.on('nodeDeleted', (o) => {
  const ids = new Set([o.inca.id, ...(o.inca.history || []).map(h => h.id)]);
  App.conns = App.conns.filter(c => !ids.has(c.src) && !ids.has(c.dst));
});
