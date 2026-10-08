// Inca — mesh booleans (Mesh > Booleans): union / difference / intersection using three-bvh-csg.
// The result is a new mesh whose construction history is a 'polyCBoolOp' node referencing the two input meshes
// (which are hidden, like Maya). Changing the Operation channel re-evaluates; deleting an input bakes the result.
import * as THREE from 'three';
import { Brush, Evaluator, ADDITION, SUBTRACTION, INTERSECTION } from 'three-bvh-csg';
import { App } from './app.js';
import * as S from './scene.js';
import { OPS } from './ops.js';
import { PolyMesh, buildGeometry, compact } from './polymesh.js';
import { Undo } from './undo.js';
import { Sel } from './selection.js';

const OPNAMES = ['union', 'difference', 'intersection'];
const CSG_OP = [ADDITION, SUBTRACTION, INTERSECTION];
const lastGood = new Map(); // history id -> last good PolyMesh (fallback if an input is missing)

function brushOf(o, slot) {
  o.updateWorldMatrix(true, false);
  const { geometry } = buildGeometry(o.inca.mesh);
  geometry.clearGroups(); geometry.addGroup(0, geometry.attributes.position.count, 0);
  geometry.applyMatrix4(o.matrixWorld);
  if (o.matrixWorld.determinant() < 0) { // mirrored: flip winding
    const p = geometry.attributes.position.array, n = geometry.attributes.normal.array, u = geometry.attributes.uv.array;
    for (let i = 0; i < p.length; i += 9) { for (let k = 0; k < 3; k++) { const t = p[i + 3 + k]; p[i + 3 + k] = p[i + 6 + k]; p[i + 6 + k] = t; const tn = n[i + 3 + k]; n[i + 3 + k] = n[i + 6 + k]; n[i + 6 + k] = tn; } const j = i / 9 * 6; for (let k = 0; k < 2; k++) { const t = u[j + 2 + k]; u[j + 2 + k] = u[j + 4 + k]; u[j + 4 + k] = t; } }
  }
  const mat = new THREE.MeshBasicMaterial(); mat.userData.matId = o.inca.material; mat.userData.slot = slot;
  const b = new Brush(geometry, mat); b.updateMatrixWorld();
  return b;
}

// merge connected coplanar triangles into n-gons (only when the merged region has a single simple boundary loop)
export function mergeCoplanar(pm, tol = 1e-4) {
  const F = pm.f; const nF = F.length;
  const nrm = F.map((_, i) => { const n = pm.faceNormal(i); const l = Math.hypot(...n) || 1; return [n[0] / l, n[1] / l, n[2] / l]; });
  const dOf = (i) => { const p = pm.v[F[i][0]]; const n = nrm[i]; return n[0] * p[0] + n[1] * p[1] + n[2] * p[2]; };
  const par = Array.from({ length: nF }, (_, i) => i); const find = (x) => { while (par[x] !== x) { par[x] = par[par[x]]; x = par[x]; } return x; };
  const t = pm.topo;
  t.edges.forEach((_, ei) => {
    const fs = t.edgeFaces[ei]; if (fs.length !== 2) return; const [a, b] = fs;
    if ((pm.fm ? pm.fm[a] : null) !== (pm.fm ? pm.fm[b] : null)) return;
    const na = nrm[a], nb = nrm[b]; if (na[0] * nb[0] + na[1] * nb[1] + na[2] * nb[2] < 1 - tol) return;
    if (Math.abs(dOf(a) - dOf(b)) > 1e-4 * (1 + Math.abs(dOf(a)))) return;
    const ra = find(a), rb = find(b); if (ra !== rb) par[ra] = rb;
  });
  const groups = new Map(); for (let i = 0; i < nF; i++) { const r = find(i); if (!groups.has(r)) groups.set(r, []); groups.get(r).push(i); }
  const nf = [], nuv = pm.uv ? [] : null, nfm = pm.fm ? [] : null;
  const keep = (i) => { nf.push(F[i].slice()); if (nuv) nuv.push(pm.uv[i] ? pm.uv[i].map(c => c.slice()) : null); if (nfm) nfm.push(pm.fm[i]); };
  for (const fs of groups.values()) {
    if (fs.length === 1) { keep(fs[0]); continue; }
    const dir = new Set(); const uvAt = new Map();
    for (const fi of fs) { const f = F[fi]; f.forEach((v, k) => { dir.add(v + ',' + f[(k + 1) % f.length]); if (pm.uv && pm.uv[fi] && !uvAt.has(v)) uvAt.set(v, pm.uv[fi][k]); }); }
    const next = new Map(); let bad = false; let count = 0;
    for (const s of dir) { const [a, b] = s.split(',').map(Number); if (dir.has(b + ',' + a)) continue; if (next.has(a)) { bad = true; break; } next.set(a, b); count++; }
    if (bad || count < 3) { fs.forEach(keep); continue; }
    const start = next.keys().next().value; const loop = [start]; let cur = next.get(start); let guard = 0;
    while (cur !== start && guard++ < count + 1) { loop.push(cur); cur = next.get(cur); if (cur === undefined) { bad = true; break; } }
    if (bad || loop.length !== count) { fs.forEach(keep); continue; }
    nf.push(loop); if (nuv) nuv.push(loop.every(v => uvAt.has(v)) ? loop.map(v => uvAt.get(v).slice()) : null); if (nfm) nfm.push(pm.fm[fs[0]]);
  }
  const out = new PolyMesh(pm.v.map(p => p.slice()), nf, nuv, nfm); out.softAngle = 30;
  compact(out);
  return out;
}

function csg(A, B, operation) {
  const ev = new Evaluator(); ev.attributes = ['position', 'uv', 'normal']; ev.useGroups = true;
  const ba = brushOf(A, 0), bb = brushOf(B, 1);
  const res = ev.evaluate(ba, bb, CSG_OP[operation] ?? ADDITION);
  const geom = res.geometry;
  const mats = Array.isArray(res.material) ? res.material : [res.material];
  const pm = App.io.threeToPoly(geom);
  const fmat = pm._fmat || []; delete pm._fmat;
  const baseMat = A.inca.material;
  pm.fm = fmat.map(i => { const id = mats[i]?.userData?.matId; return id && id !== baseMat ? id : null; });
  if (!pm.fm.some(Boolean)) pm.fm = null;
  geom.dispose(); ba.geometry.dispose(); bb.geometry.dispose();
  const merged = mergeCoplanar(pm);
  return merged;
}

OPS.polyCBoolOp = {
  base: 'polyCBoolOp',
  channels: [{ k: 'operation', type: 'enum', options: ['Union', 'Difference', 'Intersection'] }],
  apply: (_pm, p, h) => {
    const A = App.nodes.get(p.a), B = App.nodes.get(p.b);
    if (!A || !B || A.inca?.kind !== 'mesh' || B.inca?.kind !== 'mesh') { const lg = p._key && lastGood.get(p._key); return lg ? { mesh: lg.clone() } : { mesh: new PolyMesh(), error: 'boolean input missing' }; }
    const m = csg(A, B, +p.operation || 0);
    if (p._key) lastGood.set(p._key, m);
    return { mesh: m };
  },
};

function runBoolean(operation) {
  const ms = App.sel.filter(o => o.inca.kind === 'mesh');
  if (ms.length < 2) { App.emit('warning', '// Warning: Select two polygon objects (first = A, second = B).'); return; }
  const [A, B] = ms;
  Undo.checkpoint('boolean ' + OPNAMES[operation]);
  if (App.compMode) Sel.setMode(null);
  let o;
  try {
    o = S.createMesh(null, {}, { name: 'polySurface1', mesh: new PolyMesh(), material: A.inca.material });
    o.inca.history = [];
    const key = 'b' + Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
    S.addHistory(o, 'polyCBoolOp', { a: A.inca.id, b: B.inca.id, operation, _key: key });
  } catch (e) { if (o) S.deleteNode(o); throw e; }
  if (!o.inca.mesh.f.length && operation !== 2) App.emit('warning', '// Warning: Boolean produced an empty mesh.');
  S.centerPivot(o);
  for (const x of [A, B]) S.setAttr(x, 'visibility', 0, { silent: true, force: true });
  Sel.select([o], 'replace', { echo: false });
  App.emit('echo', `polyCBoolOp -op ${operation + 1} -ch 1 -preserveColor 0 -classification 1 -name ${o.inca.name} ${A.inca.name} ${B.inca.name};`);
  App.emit('result', `${o.inca.name} ${o.inca.history[0].name}`);
  App.emit('inViewEditor', o.inca.history[0]);
  App.dirty('outliner', 'channels', 'attr');
  return o;
}
App.cmds.register('booleanUnion', 'Union', () => runBoolean(0), { icon: 'boolean', help: 'Booleans: union of two selected meshes' });
App.cmds.register('booleanDifference', 'Difference', () => runBoolean(1), { icon: 'boolean', help: 'Booleans: subtract the second selected mesh from the first' });
App.cmds.register('booleanIntersection', 'Intersection', () => runBoolean(2), { icon: 'boolean', help: 'Booleans: intersection of two selected meshes' });

// deleting an input keeps the result (bake its current shape)
App.on('nodeDeleted', (o) => {
  if (!o?.inca || o.inca.kind !== 'mesh') return;
  for (const n of App.nodes.values()) {
    if (!n.isObject3D || n === o || n.inca.kind !== 'mesh' || !n.inca.history) continue;
    if (n.inca.history.some(h => h.type === 'polyCBoolOp' && (h.params.a === o.inca.id || h.params.b === o.inca.id))) S.bakeMesh(n, n.inca.mesh);
  }
});
// re-evaluate boolean results when an input mesh changes shape or moves (cheap check on refresh)
const sigs = new Map();
App.on('refresh', (d) => {
  if (!(d.has('channels') || d.has('attr') || d.has('outliner'))) return;
  for (const n of App.nodes.values()) {
    if (!n.isObject3D || n.inca.kind !== 'mesh' || !n.inca.history) continue;
    const idx = n.inca.history.findIndex(h => h.type === 'polyCBoolOp'); if (idx < 0) continue;
    const h = n.inca.history[idx]; const A = App.nodes.get(h.params.a), B = App.nodes.get(h.params.b); if (!A || !B) continue;
    A.updateWorldMatrix(true, false); B.updateWorldMatrix(true, false);
    const sig = A.matrixWorld.elements.join(',') + '|' + B.matrixWorld.elements.join(',') + '|' + A.inca.mesh.v.length + ':' + A.inca.mesh.f.length + ':' + (A.inca.history?.length) + '|' + B.inca.mesh.v.length + ':' + B.inca.mesh.f.length + ':' + (B.inca.history?.length) + '|' + h.params.operation;
    const prev = sigs.get(h.id); sigs.set(h.id, sig);
    if (prev !== undefined && prev !== sig) S.evaluate(n, idx);
  }
});
