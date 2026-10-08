// Inca — Quad Draw tool (Mesh Tools > Quad Draw): retopology on a live surface.
//  LMB click         place a dot on the live surface (App.liveSurface, or the mesh under the cursor)
//  LMB drag on dot/vertex   slide it along the surface
//  Shift (hover)     preview the quad (or triangle) bounded by the nearest dots / vertices — green
//  Shift+click       create that face in the target mesh (polySurface)
//  Ctrl+Shift+click  delete the dot or the face under the cursor
//  Backspace         delete the last dot;  Enter: complete (select the result)
import * as THREE from 'three';
import { App } from '../core/app.js';
import * as S from '../core/scene.js';
import { Sel } from '../core/selection.js';
import { Undo } from '../core/undo.js';
import { PolyMesh, compact } from '../core/polymesh.js';

const dotMat = new THREE.PointsMaterial({ size: 8, sizeAttenuation: false, color: 0x3fa9ff, depthTest: false });
const vertMat = new THREE.PointsMaterial({ size: 6, sizeAttenuation: false, color: 0xffe14a, depthTest: false });
const hotMat = new THREE.PointsMaterial({ size: 11, sizeAttenuation: false, color: 0xff4040, depthTest: false });
const greenLine = new THREE.LineBasicMaterial({ color: 0x5cff5c, depthTest: false });
const greenFill = new THREE.MeshBasicMaterial({ color: 0x5cff5c, transparent: true, opacity: 0.3, depthTest: false, side: THREE.DoubleSide });
const Q = { dots: [], targetId: null, preview: null, hot: null, lastPx: null, lastVp: null, shift: false };

const target = () => { const o = App.nodes.get(Q.targetId); return o && o.inca && o.inca.kind === 'mesh' ? o : null; };
function surfaces() { if (App.liveSurface && App.nodes.get(App.liveSurface.inca.id) === App.liveSurface) return [App.liveSurface]; const t = target(); return S.allDag().filter(o => o.inca.kind === 'mesh' && o.visible && o !== t); }
function hitSurface(vp, px) {
  const rc = vp.raycaster(px); let best = null;
  for (const o of surfaces()) { const h = vp.rayMesh(rc, o); if (h && (!best || h.distance < best.distance)) { best = h; best.o = o; } }
  if (!best) return null;
  const n = best.face ? best.face.normal.clone().transformDirection(best.o.userData.mesh.matrixWorld) : new THREE.Vector3(0, 1, 0);
  const box = S.worldBBox(best.o); const off = box.getSize(new THREE.Vector3()).length() * 0.003;
  return { p: best.point.clone().addScaledVector(n, off), n, o: best.o };
}
function targetWorld() { const t = target(); if (!t) return []; t.updateWorldMatrix(true, false); return t.inca.mesh.v.map(p => new THREE.Vector3(...p).applyMatrix4(t.matrixWorld)); }
function candidates(vp) {
  const out = Q.dots.map((w, i) => ({ kind: 'dot', i, w })); targetWorld().forEach((w, i) => out.push({ kind: 'vert', i, w }));
  for (const c of out) c.s = vp.project(c.w);
  return out.filter(c => c.s.z <= 1 && c.s.z >= -1);
}
function nearest(vp, px, r = 10) { let best = null; for (const c of candidates(vp)) { const d = Math.hypot(c.s.x - px.x, c.s.y - px.y); if (d < r && (!best || d < best.d)) best = { ...c, d }; } return best; }
const cross2 = (o, a, b) => (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
function inPoly(p, poly) { let c = false; for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) { const a = poly[i], b = poly[j]; if (((a.y > p.y) !== (b.y > p.y)) && (p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x)) c = !c; } return c; }
function faceUnderCursor(vp, px) {
  const t = target(); if (!t) return -1; const W = targetWorld(); const Sc = W.map(w => vp.project(w));
  let best = -1, bz = Infinity;
  t.inca.mesh.f.forEach((f, fi) => { const poly = f.map(i => Sc[i]); if (poly.some(s => s.z > 1)) return; if (inPoly(px, poly)) { const z = poly.reduce((a, s) => a + s.z, 0) / poly.length; if (z < bz) { bz = z; best = fi; } } });
  return best;
}
// best convex quad (or triangle) of candidates containing the cursor
function findRegion(vp, px) {
  const cs = candidates(vp).map(c => ({ ...c, d: Math.hypot(c.s.x - px.x, c.s.y - px.y) })).sort((a, b) => a.d - b.d).slice(0, 9);
  if (cs.length < 3) return null;
  if (faceUnderCursor(vp, px) >= 0) return null;
  const t = target(); const faceKeys = new Set(t ? t.inca.mesh.f.map(f => [...f].sort((a, b) => a - b).join(',')) : []);
  let best = null;
  const tryset = (set) => {
    const cx = set.reduce((a, c) => a + c.s.x, 0) / set.length, cy = set.reduce((a, c) => a + c.s.y, 0) / set.length;
    const ord = set.slice().sort((a, b) => Math.atan2(a.s.y - cy, a.s.x - cx) - Math.atan2(b.s.y - cy, b.s.x - cx));
    const P = ord.map(c => c.s); let sgn = 0;
    for (let i = 0; i < P.length; i++) { const cr = cross2(P[i], P[(i + 1) % P.length], P[(i + 2) % P.length]); if (Math.abs(cr) < 1e-6) return; const s = Math.sign(cr); if (sgn && s !== sgn) return; sgn = s; }
    if (!inPoly(px, P)) return;
    for (const c of cs) if (!set.includes(c) && inPoly(c.s, P)) return; // another point inside: too big
    if (ord.every(c => c.kind === 'vert') && faceKeys.has(ord.map(c => c.i).sort((a, b) => a - b).join(','))) return;
    // must not overlap an existing face: centroid inside a face
    let per = 0; for (let i = 0; i < P.length; i++) per += Math.hypot(P[i].x - P[(i + 1) % P.length].x, P[i].y - P[(i + 1) % P.length].y);
    if (faceUnderCursor(vp, { x: cx, y: cy }) >= 0) return;
    if (!best || per < best.per) best = { ord, per };
  };
  const n = cs.length;
  for (let a = 0; a < n; a++) for (let b = a + 1; b < n; b++) for (let c = b + 1; c < n; c++) for (let d = c + 1; d < n; d++) tryset([cs[a], cs[b], cs[c], cs[d]]);
  if (!best) for (let a = 0; a < n; a++) for (let b = a + 1; b < n; b++) for (let c = b + 1; c < n; c++) tryset([cs[a], cs[b], cs[c]]);
  return best ? best.ord : null;
}
// ------------------------------------------------------------------ drawing
function draw() {
  for (const vp of App.viewports) {
    for (const c of [...vp.toolScene.children]) { vp.toolScene.remove(c); c.geometry?.dispose(); }
    if (App.tool !== 'quadDraw') { vp.needsRender = true; continue; }
    if (Q.dots.length) vp.toolScene.add(new THREE.Points(new THREE.BufferGeometry().setFromPoints(Q.dots), dotMat));
    const W = targetWorld(); if (W.length) vp.toolScene.add(new THREE.Points(new THREE.BufferGeometry().setFromPoints(W), vertMat));
    if (Q.hot) vp.toolScene.add(new THREE.Points(new THREE.BufferGeometry().setFromPoints([Q.hot.w]), hotMat));
    if (Q.preview) {
      const pts = Q.preview.map(c => c.w);
      vp.toolScene.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints([...pts, pts[0]]), greenLine));
      const g = new THREE.BufferGeometry().setFromPoints(pts); g.setIndex(pts.length === 4 ? [0, 1, 2, 0, 2, 3] : [0, 1, 2]);
      vp.toolScene.add(new THREE.Mesh(g, greenFill));
    }
    vp.needsRender = true;
  }
}
// ------------------------------------------------------------------ edits
function ensureTarget() {
  let t = target(); if (t) return t;
  const pm = new PolyMesh([], []);
  t = S.createMesh(null, {}, { name: 'polySurface1', mesh: pm }); S.bakeMesh(t, pm);
  Q.targetId = t.inca.id; return t;
}
function makeFace(vp, ord) {
  Undo.checkpoint('quad draw');
  const t = ensureTarget(); const pm = t.inca.mesh.clone(); t.updateWorldMatrix(true, false); const inv = t.matrixWorld.clone().invert();
  const used = new Set(); const idx = ord.map(c => { if (c.kind === 'vert') return c.i; used.add(c.i); const l = c.w.clone().applyMatrix4(inv); pm.v.push([l.x, l.y, l.z]); return pm.v.length - 1; });
  // winding: agree with neighbours sharing an edge, otherwise face the camera
  let face = idx.slice(); const dir = new Set(); pm.f.forEach(f => f.forEach((v, k) => dir.add(v + ',' + f[(k + 1) % f.length])));
  let agree = 0; face.forEach((v, k) => { const w = face[(k + 1) % face.length]; if (dir.has(v + ',' + w)) agree--; if (dir.has(w + ',' + v)) agree++; });
  if (agree < 0) face.reverse();
  else if (agree === 0) {
    const W = ord.map(c => c.w); const n = new THREE.Vector3(); for (let i = 0; i < W.length; i++) { const a = W[i], b = W[(i + 1) % W.length]; n.x += (a.y - b.y) * (a.z + b.z); n.y += (a.z - b.z) * (a.x + b.x); n.z += (a.x - b.x) * (a.y + b.y); }
    const c = W.reduce((s, w) => s.add(w), new THREE.Vector3()).multiplyScalar(1 / W.length); const cp = new THREE.Vector3(); vp.camera.getWorldPosition(cp);
    const view = vp.isOrtho ? vp.camBasis().back : cp.sub(c); if (n.dot(view) < 0) face.reverse();
  }
  if (pm.uv) pm.uv.push(null);
  if (pm.fm) pm.fm.push(null);
  pm.f.push(face); pm.invalidate();
  S.bakeMesh(t, pm);
  Q.dots = Q.dots.filter((_, i) => !used.has(i));
  App.emit('echo', `polyCreateFacet -s 1 ...; // Quad Draw: ${t.inca.name} faces ${pm.f.length}`);
  App.dirty('channels', 'outliner');
}
function deleteAt(vp, px) {
  const nd = nearest(vp, px); if (nd && nd.kind === 'dot') { Q.dots.splice(nd.i, 1); return; }
  const fi = faceUnderCursor(vp, px); const t = target(); if (fi < 0 || !t) return;
  Undo.checkpoint('quad draw delete');
  const pm = t.inca.mesh.clone(); pm.f.splice(fi, 1); if (pm.uv) pm.uv.splice(fi, 1); if (pm.fm) pm.fm.splice(fi, 1); compact(pm); pm.invalidate();
  S.bakeMesh(t, pm); App.dirty('channels');
}
// ------------------------------------------------------------------ tool
const tool = {
  help: 'Quad Draw: click to place dots on the live surface, Shift+click inside 4 dots to fill a quad, drag dots/vertices to slide them, Ctrl+Shift+click deletes, Enter completes',
  start() {
    Q.dots = []; Q.preview = null; Q.hot = null;
    const lead = Sel.lead();
    if (!target() || lead !== target()) Q.targetId = null;
    if (lead && lead.inca.kind === 'mesh') {
      if (!App.liveSurface && lead.inca.mesh.f.length > 0) { App.liveSurface = lead; App.dirty('statusline'); App.emit('echo', `makeLive ${lead.inca.name};`); }
      else if (App.liveSurface && lead !== App.liveSurface) Q.targetId = lead.inca.id;
    }
    draw();
  },
  end() { Q.dots = []; Q.preview = null; Q.hot = null; draw(); },
  hover(vp, e, px) {
    Q.lastPx = px; Q.lastVp = vp; Q.shift = e.shiftKey && !e.ctrlKey;
    Q.hot = nearest(vp, px);
    Q.preview = Q.shift ? findRegion(vp, px) : null;
    draw(); return true;
  },
  down(vp, e, px) {
    if (e.shiftKey && e.ctrlKey) { deleteAt(vp, px); Q.preview = null; draw(); return true; }
    if (e.shiftKey) { const r = findRegion(vp, px); if (r) makeFace(vp, r); Q.preview = null; draw(); return true; }
    const hot = nearest(vp, px);
    if (hot) { // slide along the surface
      let ck = false; const t = target();
      const mm = (ev) => {
        const h = hitSurface(vp, vp.localPx(ev)); if (!h) return;
        if (hot.kind === 'dot') Q.dots[hot.i] = h.p;
        else if (t) { if (!ck) { Undo.checkpoint('quad draw move'); ck = true; } t.updateWorldMatrix(true, false); const l = h.p.clone().applyMatrix4(t.matrixWorld.clone().invert()); t.inca.mesh.v[hot.i] = [l.x, l.y, l.z]; t.inca.mesh.invalidate(); S.rebuildShape(t); }
        Q.hot = { ...hot, w: h.p }; draw();
      };
      const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); const tt = target(); if (ck && tt) S.bakeMesh(tt, tt.inca.mesh); };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu); return true;
    }
    const h = hitSurface(vp, px);
    if (!h) { App.help('Quad Draw: click on a live surface (select a mesh and use Make Live, or click on any mesh)'); return true; }
    Q.dots.push(h.p); draw(); return true;
  },
  complete() {
    const t = target();
    Q.dots = []; Q.preview = null;
    App.setTool('select');
    if (t) Sel.select([t], 'replace', { echo: false });
    draw();
  },
};
App.tools.register('quadDraw', tool, 'polyPlane');
App.cmds.register('quadDrawTool', 'Quad Draw', () => App.setTool('quadDraw'), { icon: 'polyPlane', noRepeat: true, help: tool.help });
// Backspace deletes the last dot; Shift press/release refreshes the preview without moving the mouse
addEventListener('keydown', (e) => {
  if (App.tool !== 'quadDraw' || e.defaultPrevented || /^(INPUT|TEXTAREA|SELECT)$/.test(e.target?.tagName || '')) return;
  if ((e.key === 'Backspace' || e.key === 'Delete') && Q.dots.length && !e.ctrlKey) { Q.dots.pop(); draw(); e.preventDefault(); e.stopPropagation(); }
  else if (e.key === 'Shift' && Q.lastVp && Q.lastPx && !e.ctrlKey) { Q.preview = findRegion(Q.lastVp, Q.lastPx); draw(); }
}, true);
addEventListener('keyup', (e) => { if (App.tool === 'quadDraw' && e.key === 'Shift' && Q.preview) { Q.preview = null; draw(); } }, true);
export const QuadDraw = Q;
