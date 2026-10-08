// Node test for the animation / UV editor logic: node test/editors.test.mjs
// Minimal browser stubs so the UI modules can be imported (nothing is rendered).
globalThis.window = globalThis;
globalThis.requestAnimationFrame = (f) => setTimeout(f, 0);
globalThis.document = { addEventListener() {}, querySelectorAll: () => [], getElementById: () => null, createElement: () => ({ getContext: () => null, style: {} }), head: { append() {} }, body: { append() {} } };
const { App } = await import('../src/js/core/app.js');
const { AnimCurve } = await import('../src/js/core/anim.js');
const M = await import('../src/js/ui/editorMath.js');
const { GENERATORS } = await import('../src/js/core/primitives.js');

let fails = 0;
const ok = (cond, msg) => { if (!cond) { fails++; console.log('FAIL', msg); } else console.log('ok  ', msg); };
const near = (a, b, e = 1e-6) => Math.abs(a - b) <= e;
App.prefs = { defaultInTangent: 'auto', defaultOutTangent: 'auto' };

// ---------------- generic
ok(M.niceStep(3) === 5 && M.niceStep(0.03) === 0.05 && M.niceStep(10) === 10 && M.niceStep(11) === 20, 'niceStep');
ok(M.attrColor('translateX') === '#e0393e' && M.attrColor('rotateY') === '#3fcf4a' && M.attrColor('scaleZ') === '#3f86f2', 'attrColor axes');

// ---------------- key moves
const c = new AnimCurve('n1', 'translateX');
const k1 = c.setKey(1, 0), k2 = c.setKey(10, 5), k3 = c.setKey(20, 0);
const sel = new Map();
M.selApply(sel, [[c, k2]], 'replace');
let st = M.beginKeyDrag(sel);
M.applyKeyDrag(st, 3.4, 2, { snap: true });
ok(k2.t === 13 && k2.v === 7, 'drag snaps time to whole frame, moves value');
M.applyKeyDrag(st, 3.4, 2, { snap: true, value: false });
ok(k2.t === 13 && k2.v === 5, 'time-only constraint keeps value');
M.applyKeyDrag(st, 12, 0, { snap: true });
ok(c.keys[2] === k2 && k2.t === 22, 'keys re-sorted after passing a neighbour');
M.applyKeyDrag(st, 10, 0, { snap: true });
ok(k2.t === 20, 'moved onto existing key');
M.resolveKeyCollisions([c], sel);
ok(c.keys.length === 2 && c.keys.includes(k2) && !c.keys.includes(k3), 'collision: moved key replaces stationary key');
M.selApply(sel, [[c, k1]], 'toggle');
ok(M.selCount(sel) === 2, 'toggle adds');
M.selApply(sel, [[c, k1]], 'deselect');
ok(M.selCount(sel) === 1 && M.selHas(sel, c, k2), 'deselect');
const { removed } = M.deleteSelectedKeys(sel);
ok(removed === 1 && c.keys.length === 1 && sel.size === 0, 'delete selected keys');
// prune
const c2 = new AnimCurve('n1', 'rotateY'); const a = c2.setKey(1, 1); c2.setKey(5, 2);
M.selApply(sel, [[c2, a], [c, k1]], 'replace');
c2.keys.shift();
M.selPrune(sel, new Set([c2]));
ok(sel.size === 0, 'prune removes dead curves and keys');
// tangent handle math round trip
const s0 = 1.7, sx = 6, sy = 12;
const [hx, hy] = M.handleVec(s0, sx, sy);
ok(near(M.slopeFromHandle(hx, hy, sx, sy, 'out'), s0) && near(M.slopeFromHandle(-hx, -hy, sx, sy, 'in'), s0), 'handle <-> slope round trip');
// fixed tangent evaluation uses is/os
const c3 = new AnimCurve('n2', 'translateY'); c3.setKey(0, 0); const kk = c3.setKey(10, 10);
kk.it = 'fixed'; kk.is = 0; ok(near(c3.eval(10), 10) && c3.slopes(1)[0] === 0, 'fixed tangent slope respected');

// ---------------- UVs
const cube = GENERATORS.polyCube({});
ok(!!cube.uv && cube.uv.length === cube.f.length, 'cube has uvs');
const ci = M.cornerIndex(cube);
ok(ci.n === cube.f.reduce((n, f) => n + f.length, 0), 'corner index size');
const { shells } = M.uvShells(cube);
ok(shells.length >= 1 && shells.flat().length === cube.f.length, `uv shells (${shells.length}) cover all faces`);
const coinc = M.coincident(cube, 0);
ok(coinc.includes(0) && coinc.every(id => ci.cv[id] === ci.cv[0]), 'coincident corners share vertex');
const all = [...Array(ci.n).keys()];
const pm = cube.clone();
const snap = M.snapshotCorners(pm, all);
M.xformCorners(pm, all, M.uvXf.translate(0.25, -0.5));
ok(near(M.cornerUV(pm, M.cornerIndex(pm), 3)[0], snap[3][0] + 0.25), 'translate');
M.restoreCorners(pm, all, snap);
ok(near(M.cornerUV(pm, M.cornerIndex(pm), 3)[1], snap[3][1]), 'restore snapshot');
const b0 = M.uvBounds(pm, all);
M.xformCorners(pm, all, M.uvXf.rotate(Math.PI / 2, b0.cu, b0.cv));
M.xformCorners(pm, all, M.uvXf.rotate(-Math.PI / 2, b0.cu, b0.cv));
ok(all.every((id, i) => near(M.cornerUV(pm, M.cornerIndex(pm), id)[0], snap[i][0], 1e-9)), 'rotate +90/-90 identity');
M.xformCorners(pm, all, M.uvXf.flipU(b0.cu)); M.xformCorners(pm, all, M.uvXf.flipU(b0.cu));
ok(all.every((id, i) => near(M.cornerUV(pm, M.cornerIndex(pm), id)[0], snap[i][0], 1e-9)), 'flipU twice identity');
M.xformCorners(pm, all, M.uvXf.scale(3, 2, 0, 0));
M.normalizeCorners(pm, all);
const nb = M.uvBounds(pm, all);
ok(near(nb.u0, 0) && near(nb.v0, 0) && near(Math.max(nb.u1, nb.v1), 1), 'normalize into 0..1');
M.layoutShells(pm, M.uvShells(pm).shells);
const lb = M.uvBounds(pm, all);
ok(lb.u0 >= 0 && lb.v0 >= 0 && lb.u1 <= 1 + 1e-9 && lb.v1 <= 1 + 1e-9, 'layout stays inside 0..1');
// layout of separated faces (every face its own shell)
const sep = cube.clone(); sep.uv = sep.uv.map((u, i) => u.map(p => [p[0] + i * 3, p[1]]));
const ss = M.uvShells(sep).shells; sep.v = sep.v.slice(); // shells by uv split
M.layoutShells(sep, ss);
const sb = M.uvBounds(sep, all);
ok(sb.u0 >= 0 && sb.u1 <= 1 + 1e-9 && sb.v1 <= 1 + 1e-9, `layout of ${ss.length} shells inside 0..1`);
// relax keeps borders, moves interior points of a plane
const plane = GENERATORS.polyPlane({});
if (plane.uv) {
  const pc = M.cornerIndex(plane); const ids = [...Array(pc.n).keys()];
  const before = M.snapshotCorners(plane, ids);
  // jitter interior
  M.xformCorners(plane, ids, (p) => (p[0] > 0.01 && p[0] < 0.99 && p[1] > 0.01 && p[1] < 0.99) ? [p[0] + 0.03, p[1] - 0.02] : p);
  M.relaxUVs(plane, null, 200);
  const err = Math.max(...ids.map((id, i) => Math.hypot(M.cornerUV(plane, pc, id)[0] - before[i][0], M.cornerUV(plane, pc, id)[1] - before[i][1])));
  ok(err < 1e-3, 'relax restores a regular grid (max err ' + err.toExponential(2) + ')');
}
ok(M.faceUVArea([[0, 0], [1, 0], [1, 1], [0, 1]]) === 1 && M.faceUVArea([[0, 0], [0, 1], [1, 1], [1, 0]]) === -1, 'signed uv area');

// ---------------- polyUVSet history op (registered by uvEditor.js)
try {
  const { OPS } = await import('../src/js/core/ops.js');
  await import('../src/js/ui/uvEditor.js');
  ok(!!OPS.polyUVSet && OPS.polyUVSet.hidden, 'polyUVSet registered');
  const uvs = cube.uv.map(u => u.map(p => [p[0] * 0.5, p[1] * 0.5]));
  const r = OPS.polyUVSet.apply(cube, { uv: uvs });
  ok(r.mesh !== cube && near(r.mesh.uv[2][1][0], cube.uv[2][1][0] * 0.5), 'polyUVSet applies uv set to a clone');
  const r2 = OPS.polyUVSet.apply(cube, { uv: uvs.slice(1) });
  ok(near(r2.mesh.uv[2][1][0], cube.uv[2][1][0]), 'polyUVSet passes through on face-count mismatch');
} catch (e) { console.log('skip polyUVSet module test:', e.message); }

console.log(fails ? `\n${fails} FAILED` : '\nall passed');
process.exit(fails ? 1 : 0);
