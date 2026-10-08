// Inca — interactive viewport tools: CV/EP/Pencil curves, Create Polygon, Insert Edge Loop, Multi-Cut,
// Target Weld, Lasso / Paint selection and Sculpt brushes.
import * as THREE from 'three';
import { App } from '../core/app.js';
import * as S from '../core/scene.js';
import { Sel } from '../core/selection.js';
import { Undo } from '../core/undo.js';
import { Curve } from '../core/curves.js';
import { PolyMesh, edgeRing, ekey } from '../core/polymesh.js';
import { Manip, placeProxy } from '../core/manip.js';
import { toolLineMat, toolPtMat } from './viewport.js';

const C = App.cmds.register;
const INTERACTIVE = new Set(['cvCurve', 'epCurve', 'pencilCurve', 'createPoly', 'edgeLoop', 'multiCut', 'targetWeld', 'lasso', 'paint', 'sculpt', 'jointTool']);
const ICONS = { cvCurve: 'cvCurve', epCurve: 'epCurve', pencilCurve: 'pencilCurve', createPoly: 'planar', edgeLoop: 'edgeLoop', multiCut: 'multiCut', targetWeld: 'targetWeld', lasso: 'lasso', paint: 'paintSelect', sculpt: 'softSelect', jointTool: 'joint' };
const opt = (k, d) => (App.prefs.opt.tools?.[k] ?? d);
const setOpt = (k, v) => { (App.prefs.opt.tools ||= {})[k] = v; App.savePrefs(); };
let st = null; // active tool state

// ---- overlay drawing into every viewport's tool scene
function overlay(build) {
  for (const vp of App.viewports) {
    for (const c of [...vp.toolScene.children]) { vp.toolScene.remove(c); c.geometry?.dispose(); }
    if (build) { const objs = build(vp); for (const o of objs.filter(Boolean)) vp.toolScene.add(o); }
    vp.needsRender = true;
  }
}
const lineObj = (pts, mat = toolLineMat, closed = false) => { if (pts.length < 2) return null; const arr = closed ? [...pts, pts[0]] : pts; const g = new THREE.BufferGeometry().setFromPoints(arr.map(p => p.isVector3 ? p : new THREE.Vector3(...p))); const l = new THREE.Line(g, mat); l.computeLineDistances(); return l; };
const ptsObj = (pts, mat = toolPtMat) => { if (!pts.length) return null; const g = new THREE.BufferGeometry().setFromPoints(pts.map(p => p.isVector3 ? p : new THREE.Vector3(...p))); return new THREE.Points(g, mat); };
const solidLine = new THREE.LineBasicMaterial({ color: 0xffe14a, depthTest: false });
const greenLine = new THREE.LineBasicMaterial({ color: 0x5cff5c, depthTest: false });
const bluePts = new THREE.PointsMaterial({ size: 7, sizeAttenuation: false, color: 0x4ab8ff, depthTest: false });
const redPts = new THREE.PointsMaterial({ size: 9, sizeAttenuation: false, color: 0xff3a3a, depthTest: false });

function worldVerts(o) { o.updateWorldMatrix(true, false); return o.inca.mesh.v.map(p => new THREE.Vector3(...p).applyMatrix4(o.matrixWorld)); }
function meshTargets() { const s = new Set([...App.hilite, ...App.sel].filter(o => o.inca.kind === 'mesh')); if (!s.size) for (const o of S.allDag()) if (o.inca.kind === 'mesh' && o.visible) s.add(o); return [...s]; }
// nearest edge under the cursor among candidate meshes: {o, a, b, t, d}
function pickEdge(vp, px, objs = meshTargets()) {
  let best = null;
  for (const o of objs) {
    if (!o.visible) continue;
    const W = worldVerts(o); const Sc = W.map(w => vp.project(w));
    o.inca.mesh.topo.edges.forEach(([a, b]) => {
      const A = Sc[a], B = Sc[b]; if (A.z > 1 || B.z > 1) return;
      const dx = B.x - A.x, dy = B.y - A.y; const L2 = dx * dx + dy * dy || 1;
      let t = ((px.x - A.x) * dx + (px.y - A.y) * dy) / L2; t = Math.max(0, Math.min(1, t));
      const d = Math.hypot(A.x + dx * t - px.x, A.y + dy * t - px.y);
      if (d < 10 && (!best || d < best.d)) { const wp = W[a].clone().lerp(W[b], t); if (!vp.occluded(wp, o)) best = { o, a, b, t, d, W }; }
    });
  }
  return best;
}
function pickVertex(vp, px, objs = meshTargets(), radius = 10) {
  let best = null;
  for (const o of objs) { if (!o.visible) continue; const W = worldVerts(o); W.forEach((w, i) => { const s = vp.project(w); if (s.z > 1) return; const d = Math.hypot(s.x - px.x, s.y - px.y); if (d < radius && (!best || d < best.d) && !vp.occluded(w, o)) best = { o, i, d, w }; }); }
  return best;
}

// ------------------------------------------------------------------ tool implementations
const TOOLS = {
  // ---------------- CV / EP curves
  cvCurve: curveTool('cv'), epCurve: curveTool('ep'),
  pencilCurve: {
    help: 'Pencil Curve Tool: drag to draw a curve freehand',
    down(vp, e, px) {
      const pts = []; const add = (p) => { const g = vp.groundPoint(p); if (g && (!pts.length || g.distanceTo(pts[pts.length - 1]) > vp.worldPerPixel() * 6)) pts.push(g); overlay(() => [lineObj(pts, solidLine)]); };
      add(px);
      const mm = (ev) => add(vp.localPx(ev));
      const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); overlay(null); if (pts.length < 2) return; Undo.checkpoint('pencil curve'); const keep = simplify(pts, vp.worldPerPixel() * 4); const o = S.createCurve(new Curve(keep.map(p => p.toArray()), 3, 'open', 'ep'), { name: 'curve1' }); Sel.select([o]); App.emit('echo', `curve -d 3 -ep ${keep.length} points;`); };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu); return true;
    },
  },
  // ---------------- create polygon
  createPoly: {
    help: 'Create Polygon Tool: click to place vertices. Enter to complete, Backspace to remove the last point',
    start() { st.pts = []; },
    down(vp, e, px) { const g = vp.groundPoint(px); if (g) { st.pts.push(g); this.draw(); } return true; },
    hover(vp, e, px) { st.hoverPt = vp.groundPoint(px); this.draw(); return true; },
    draw() { overlay(() => [lineObj(st.hoverPt ? [...st.pts, st.hoverPt] : st.pts, solidLine, st.pts.length > 1), ptsObj(st.pts)]); },
    back() { st.pts.pop(); this.draw(); },
    complete() {
      if (st.pts.length < 3) { App.help('Place at least 3 points'); return; }
      Undo.checkpoint('create polygon');
      const c = new THREE.Vector3(); st.pts.forEach(p => c.add(p)); c.multiplyScalar(1 / st.pts.length);
      const v = st.pts.map(p => p.clone().sub(c).toArray());
      // orient the face towards the camera / +Y
      const n = new THREE.Vector3(); for (let i = 0; i < v.length; i++) { const a = v[i], b = v[(i + 1) % v.length]; n.x += (a[1] - b[1]) * (a[2] + b[2]); n.y += (a[2] - b[2]) * (a[0] + b[0]); n.z += (a[0] - b[0]) * (a[1] + b[1]); }
      const vp = App.activeViewport; const view = vp.camBasis().back;
      let f = v.map((_, i) => i); if (n.dot(view) < 0 && !(Math.abs(n.y) > 0.5 && n.y > 0)) f = f.reverse(); else if (Math.abs(view.y) > 0.3 && n.y < 0) f = f.reverse();
      const pm = new PolyMesh(v, [f]);
      const o = S.createMesh(null, {}, { name: 'polySurface1', mesh: pm }); S.bakeMesh(o, pm); o.inca.t = c.toArray(); S.updateXform(o);
      Sel.select([o]); App.emit('echo', `polyCreateFacet -ch on -tx 1 -s 1 -p ${st.pts.map(p => p.toArray().map(x => +x.toFixed(3)).join(' ')).join(' -p ')};`);
      st.pts = []; this.draw();
    },
  },
  // ---------------- insert edge loop
  edgeLoop: {
    help: 'Insert Edge Loop Tool: click and drag on an edge to insert an edge loop (Ctrl: drag to slide). Set multiple loops in Tool Settings',
    settings: [{ key: 'loops', label: 'Number of edge loops', type: 'int', min: 1, max: 20, smin: 1, smax: 10, get: () => opt('edgeLoops', 1), set: (v) => setOpt('edgeLoops', v) }, { key: 'mode', label: 'Maintain position', type: 'enum', options: ['Relative distance from edge', 'Equal distance (multiple)'], get: () => opt('edgeLoopMode', 0), set: (v) => setOpt('edgeLoopMode', v) }],
    hover(vp, e, px) { const p = pickEdge(vp, px); st.hover = p; this.preview(p); return true; },
    preview(p) {
      if (!p) return overlay(null);
      const pm = p.o.inca.mesh; const ring = edgeRing(pm, p.a, p.b); const W = p.W || worldVerts(p.o);
      const n = opt('edgeLoops', 1); const ts = n === 1 ? [p.t] : Array.from({ length: n }, (_, i) => (i + 1) / (n + 1));
      overlay(() => ts.map(t => lineObj(ring.edges.map(e => W[e.a].clone().lerp(W[e.b], t)), greenLine, ring.closed)));
    },
    down(vp, e, px) {
      const p = pickEdge(vp, px); if (!p) return true;
      Undo.checkpoint('insert edge loop');
      const n = opt('edgeLoops', 1);
      if (!App.hilite.includes(p.o)) { if (!App.compMode) { Sel.select([p.o], 'replace', { echo: false }); } }
      const { h } = S.applyOp(p.o, 'polySplitRing', { edge: [p.a, p.b], t: p.t, multiple: n });
      const W0 = p.W; const Sa = vp.project(W0[p.a]), Sb = vp.project(W0[p.b]);
      overlay(null);
      const mm = (ev) => { if (n !== 1) return; const q = vp.localPx(ev); const dx = Sb.x - Sa.x, dy = Sb.y - Sa.y; let t = ((q.x - Sa.x) * dx + (q.y - Sa.y) * dy) / (dx * dx + dy * dy || 1); t = Math.max(0.01, Math.min(0.99, t)); h.params.t = t; S.evaluate(p.o, p.o.inca.history.indexOf(h)); App.requestRender(); };
      const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); App.emit('echo', `polySplitRing -ch on -splitType 1 -weight ${h.params.t.toFixed(4)} -smoothingAngle 30 -fixQuads 1 -insertWithEdgeFlow 0;`); App.emit('result', h.name); App.emit('inViewEditor', h); Sel.pruneInvalid(p.o); p.o.userData.compDirty = true; App.dirty('channels'); };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu);
      return true;
    },
  },
  // ---------------- multi-cut
  multiCut: {
    help: 'Multi-Cut Tool: click on edges/vertices to cut. Shift snaps to 10% steps. Ctrl+click on an edge inserts an edge loop. Enter or right-click to complete, Backspace undoes the last point',
    start() { st.pts = []; st.obj = null; },
    hover(vp, e, px) { st.hoverPt = this.pick(vp, px, e); this.draw(); return true; },
    pick(vp, px, e) {
      const objs = st.obj ? [st.obj] : meshTargets();
      const v = pickVertex(vp, px, objs, 8); if (v) return { o: v.o, vertex: v.i, w: v.w };
      const ed = pickEdge(vp, px, objs); if (!ed) return null;
      let t = ed.t; if (e.shiftKey) t = Math.round(t * 10) / 10; t = Math.max(0.001, Math.min(0.999, t));
      return { o: ed.o, edge: [ed.a, ed.b], t, w: ed.W[ed.a].clone().lerp(ed.W[ed.b], t) };
    },
    draw() { overlay(() => [lineObj([...st.pts.map(p => p.w), ...(st.hoverPt ? [st.hoverPt.w] : [])], solidLine), ptsObj(st.pts.map(p => p.w), bluePts), st.hoverPt ? ptsObj([st.hoverPt.w], redPts) : null]); },
    down(vp, e, px) {
      if (e.button === 2) { this.complete(); return true; }
      const p = this.pick(vp, px, e); if (!p) { if (st.pts.length) this.complete(); return true; }
      if (e.ctrlKey && p.edge) { Undo.checkpoint('insert edge loop'); const { h } = S.applyOp(p.o, 'polySplitRing', { edge: p.edge, t: p.t, multiple: 1 }); App.emit('inViewEditor', h); App.requestRender(); return true; }
      if (st.obj && p.o !== st.obj) return true;
      st.obj = p.o; st.pts.push(p); this.draw(); return true;
    },
    back() { st.pts.pop(); if (!st.pts.length) st.obj = null; this.draw(); },
    complete() {
      if (st.pts.length >= 2 && st.obj) {
        Undo.checkpoint('multi-cut');
        const { select } = S.applyOp(st.obj, 'polySplit', { points: st.pts.map(p => p.vertex !== undefined ? { vertex: p.vertex } : { edge: p.edge, t: p.t }) });
        Sel.pruneInvalid(st.obj); st.obj.userData.compDirty = true;
        App.emit('echo', `polySplit -ch 1 -sma 180 -ep ...;`);
      }
      st.pts = []; st.obj = null; this.draw(); App.dirty('channels');
    },
  },
  // ---------------- target weld
  targetWeld: {
    help: 'Target Weld Tool: drag from a vertex onto another vertex to weld them together',
    hover(vp, e, px) { const v = pickVertex(vp, px); overlay(() => [v ? ptsObj([v.w], redPts) : null]); return true; },
    down(vp, e, px) {
      const src = pickVertex(vp, px); if (!src) return true;
      const mm = (ev) => { const q = vp.localPx(ev); const tgt = pickVertex(vp, q, [src.o]); const end = tgt ? tgt.w : (vp.groundPoint(q) || src.w); overlay(() => [lineObj([src.w, end], solidLine), ptsObj([src.w], bluePts), tgt ? ptsObj([tgt.w], redPts) : null]); };
      const mu = (ev) => {
        removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); overlay(null);
        const tgt = pickVertex(vp, vp.localPx(ev), [src.o]); if (!tgt || tgt.i === src.i) return;
        Undo.checkpoint('target weld');
        const o = src.o; const pm = o.inca.mesh; const a = pm.v[src.i], b = pm.v[tgt.i];
        const d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        S.setMeshPositions(o, [src.i], new Float32Array(b)); S.commitTweak(o, [src.i], d);
        S.applyOp(o, 'polyMergeVert', { verts: [src.i, tgt.i], threshold: 1e-4 });
        Sel.pruneInvalid(o); o.userData.compDirty = true;
        App.emit('echo', `polyMergeVertex -d 0.0001 -am 1 -ch 1 ${o.inca.name}.vtx[${src.i}] ${o.inca.name}.vtx[${tgt.i}];`);
      };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu); return true;
    },
  },
  // ---------------- lasso
  lasso: {
    help: 'Lasso Tool: drag to draw a lasso around objects or components (Shift toggles, Ctrl deselects)',
    down(vp, e, px) {
      const mode = e.shiftKey && e.ctrlKey ? 'add' : e.shiftKey ? 'toggle' : e.ctrlKey ? 'deselect' : 'replace';
      const poly = [px];
      const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg'); Object.assign(svg.style, { position: 'absolute', inset: 0, width: '100%', height: '100%', pointerEvents: 'none', zIndex: 4 });
      const path = document.createElementNS('http://www.w3.org/2000/svg', 'polyline'); path.setAttribute('fill', 'rgba(255,255,255,0.05)'); path.setAttribute('stroke', '#fff'); path.setAttribute('stroke-dasharray', '4 3'); svg.append(path); vp.view.append(svg);
      const mm = (ev) => { poly.push(vp.localPx(ev)); path.setAttribute('points', poly.map(p => p.x + ',' + p.y).join(' ')); };
      const mu = () => {
        removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); svg.remove();
        if (poly.length < 3) { vp.clickSelect(px, mode); return; }
        const inside = (s) => { let c = false; for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) { const a = poly[i], b = poly[j]; if (((a.y > s.y) !== (b.y > s.y)) && (s.x < (b.x - a.x) * (s.y - a.y) / (b.y - a.y) + a.x)) c = !c; } return c; };
        const xs = poly.map(p => p.x), ys = poly.map(p => p.y); const box = { x0: Math.min(...xs), y0: Math.min(...ys), x1: Math.max(...xs), y1: Math.max(...ys) };
        if (App.compMode && App.hilite.length) {
          const ents = vp.marqueeComponents(box).map(({ o, ids }) => {
            o.updateWorldMatrix(true, false); const M = o.matrixWorld; const pm = o.inca.mesh;
            const ptOf = (i) => { if (App.compMode === 'face') return new THREE.Vector3(...pm.faceCenter(i)); if (App.compMode === 'edge') { const [a, b] = pm.topo.edges[i]; return new THREE.Vector3(...pm.v[a]).lerp(new THREE.Vector3(...pm.v[b]), 0.5); } return new THREE.Vector3(...(o.inca.kind === 'curve' ? o.inca.curve.cvs[i] : pm.v[i])); };
            return { o, ids: ids.filter(i => inside(vp.project(ptOf(i).applyMatrix4(M)))) };
          });
          Sel.selectComponents(ents, mode);
        } else {
          const objs = vp.marqueeObjects(box).filter(n => { const pts = n.inca.kind === 'mesh' ? n.inca.mesh.v : n.inca.kind === 'curve' ? n.inca.curve.sample(4) : [[0, 0, 0]]; const step = Math.max(1, Math.floor(pts.length / 2000)); for (let i = 0; i < pts.length; i += step) { if (inside(vp.project(new THREE.Vector3(...pts[i]).applyMatrix4(n.matrixWorld)))) return true; } return false; });
          Sel.select(vp.selectTarget(objs), mode);
        }
        placeProxy(); App.requestRender();
      };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu); return true;
    },
  },
  // ---------------- paint selection
  paint: {
    help: 'Paint Selection Tool: drag over components to select them (Ctrl to unselect). Brush size in Tool Settings',
    settings: [{ key: 'r', label: 'Brush radius (px)', type: 'int', min: 2, max: 200, smin: 2, smax: 100, get: () => opt('paintRadius', 20), set: (v) => setOpt('paintRadius', v) }],
    down(vp, e, px) {
      if (!App.compMode || !App.hilite.length) { App.help('Paint selection works on components: press F9/F10/F11 first'); return false; }
      const mode = e.ctrlKey ? 'deselect' : 'add';
      if (!e.shiftKey && !e.ctrlKey) Sel.selectComponents([], 'replace');
      const R = opt('paintRadius', 20);
      const paint = (p) => { const r = { x0: p.x - R, y0: p.y - R, x1: p.x + R, y1: p.y + R }; Sel.selectComponents(vp.marqueeComponents(r), mode); App.requestRender(); };
      paint(px);
      let last = 0; const mm = (ev) => { const now = performance.now(); if (now - last < 40) return; last = now; paint(vp.localPx(ev)); };
      const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); placeProxy(); };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu); return true;
    },
  },
  // ---------------- sculpt brushes
  sculpt: {
    help: 'Sculpt: drag on a mesh to sculpt. Ctrl inverts, Shift smooths. B+drag (or Tool Settings) changes the brush size',
    settings: [
      { key: 'brush', label: 'Brush', type: 'enum', options: ['Sculpt', 'Smooth', 'Relax', 'Grab', 'Pinch', 'Flatten', 'Inflate'], values: ['sculpt', 'smooth', 'relax', 'grab', 'pinch', 'flatten', 'inflate'], get: () => opt('sculptBrush', 'sculpt'), set: (v) => setOpt('sculptBrush', v) },
      { key: 'size', label: 'Size (world)', type: 'float', min: 0.01, smin: 0.05, smax: 10, get: () => opt('sculptSize', 1), set: (v) => setOpt('sculptSize', v) },
      { key: 'strength', label: 'Strength', type: 'float', min: 0, max: 1, smin: 0, smax: 1, get: () => opt('sculptStrength', 0.3), set: (v) => setOpt('sculptStrength', v) },
      { key: 'mirror', label: 'Mirror X', type: 'bool', get: () => opt('sculptMirror', false), set: (v) => setOpt('sculptMirror', v) },
    ],
    hover(vp, e, px) { const hit = this.hit(vp, px); overlay(() => hit ? [ring(hit.point, hit.normal, opt('sculptSize', 1))] : []); return true; },
    hit(vp, px) { const rc = vp.raycaster(px); let best = null; for (const o of vp.visibleNodes()) { if (o.inca.kind !== 'mesh') continue; const hh = vp.rayMesh(rc, o); if (hh && (!best || hh.distance < best.distance)) { best = hh; best.o = o; } } if (best) { best.normal = best.face ? best.face.normal.clone().transformDirection(best.o.userData.mesh.matrixWorld) : new THREE.Vector3(0, 1, 0); } return best; },
    down(vp, e, px) {
      const hit = this.hit(vp, px); if (!hit) return true;
      const o = hit.o; if (o.inca.smoothLevel) { o.inca.smoothLevel = 0; S.rebuildShape(o); }
      Undo.checkpoint('sculpt');
      const pm = o.inca.mesh; const orig = pm.v.map(p => p.slice()); const touched = new Set();
      o.updateWorldMatrix(true, false); const M = o.matrixWorld, inv = M.clone().invert();
      const brushName = e.shiftKey ? 'smooth' : opt('sculptBrush', 'sculpt'); const sign = e.ctrlKey ? -1 : 1;
      const R = opt('sculptSize', 1), K = opt('sculptStrength', 0.3);
      const nbr = pm.topo.vertEdges.map((es, i) => es.map(ei => { const [a, b] = pm.topo.edges[ei]; return a === i ? b : a; }));
      const vnormal = (i) => { const n = new THREE.Vector3(); for (const f of pm.topo.vertFaces[i]) n.add(new THREE.Vector3(...pm.faceNormal(f))); return n.normalize(); };
      const grabStart = hit.point.clone(); let grabSet = null;
      const stroke = (h0, dragDelta) => {
        const c = h0.point.clone().applyMatrix4(inv); const rl = R / Math.max(1e-6, new THREE.Vector3().setFromMatrixScale(M).x);
        const nrm = h0.normal.clone().transformDirection(inv).normalize();
        const ids = []; for (let i = 0; i < pm.v.length; i++) { const p = pm.v[i]; const d = Math.hypot(p[0] - c.x, p[1] - c.y, p[2] - c.z); if (d < rl) ids.push([i, d]); }
        const fall = (d) => { const t = 1 - d / rl; return t * t * (3 - 2 * t); };
        let planeP = null; if (brushName === 'flatten' && ids.length) { planeP = new THREE.Vector3(); for (const [i] of ids) planeP.add(new THREE.Vector3(...pm.v[i])); planeP.multiplyScalar(1 / ids.length); }
        const apply = (i, d) => {
          const p = pm.v[i]; const w = fall(d) * K; const v = new THREE.Vector3(...p);
          if (brushName === 'sculpt') v.addScaledVector(nrm, sign * w * rl * 0.15);
          else if (brushName === 'inflate') v.addScaledVector(vnormal(i), sign * w * rl * 0.12);
          else if (brushName === 'smooth' || brushName === 'relax') { const ns = nbr[i]; if (!ns.length) return; const avg = new THREE.Vector3(); for (const j of ns) avg.add(new THREE.Vector3(...pm.v[j])); avg.multiplyScalar(1 / ns.length); if (brushName === 'relax') { const n = vnormal(i); avg.sub(n.clone().multiplyScalar(avg.clone().sub(v).dot(n))); } v.lerp(avg, Math.min(1, w * 2)); }
          else if (brushName === 'pinch') v.lerp(c, sign * w * 0.1);
          else if (brushName === 'flatten') { const off = v.clone().sub(planeP).dot(nrm); v.addScaledVector(nrm, -off * w); }
          pm.v[i] = [v.x, v.y, v.z]; touched.add(i);
        };
        if (brushName === 'grab') {
          if (!grabSet) grabSet = ids.map(([i, d]) => [i, fall(d), orig[i].slice()]);
          const dl = dragDelta.clone().transformDirection ? dragDelta.clone() : dragDelta; const ld = dl.clone().applyMatrix4(inv.clone().setPosition(0, 0, 0));
          for (const [i, w, p0] of grabSet) { pm.v[i] = [p0[0] + ld.x * w, p0[1] + ld.y * w, p0[2] + ld.z * w]; touched.add(i); }
        } else for (const [i, d] of ids) apply(i, d);
        if (opt('sculptMirror', false)) for (const i of [...touched]) { const p = pm.v[i]; let best = -1, bd = 1e-3; for (let j = 0; j < pm.v.length; j++) { const q = orig[j]; const dd = Math.hypot(q[0] + orig[i][0], q[1] - orig[i][1], q[2] - orig[i][2]); if (dd < bd) { bd = dd; best = j; } } if (best >= 0 && best !== i) { pm.v[best] = [-p[0], p[1], p[2]]; touched.add(best); } }
        pm.invalidate(); S.rebuildShape(o);
      };
      stroke(hit, new THREE.Vector3());
      let last = 0;
      const mm = (ev) => {
        const now = performance.now(); if (now - last < 16) return; last = now;
        const q = vp.localPx(ev);
        if (brushName === 'grab') { const plane = new THREE.Plane().setFromNormalAndCoplanarPoint(vp.camBasis().back, grabStart); const p = new THREE.Vector3(); vp.raycaster(q).ray.intersectPlane(plane, p); stroke(hit, p.sub(grabStart)); }
        else { const hh = this.hit(vp, q); if (hh && hh.o === o) { stroke(hh, null); overlay(() => [ring(hh.point, hh.normal, R)]); } }
        App.requestRender();
      };
      const mu = () => {
        removeEventListener('mousemove', mm); removeEventListener('mouseup', mu);
        const verts = [...touched]; const deltas = []; for (const i of verts) { const a = orig[i], b = pm.v[i]; deltas.push(b[0] - a[0], b[1] - a[1], b[2] - a[2]); }
        if (verts.length) S.commitTweak(o, verts, deltas);
        o.userData.compDirty = true; App.dirty('channels');
      };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu); return true;
    },
  },
};
function ring(p, n, r) { const q = new THREE.Quaternion().setFromUnitVectors(new THREE.Vector3(0, 0, 1), n.clone().normalize()); const pts = []; for (let i = 0; i <= 32; i++) { const a = i / 32 * Math.PI * 2; pts.push(new THREE.Vector3(Math.cos(a) * r, Math.sin(a) * r, 0).applyQuaternion(q).add(p)); } pts.push(p.clone(), p.clone().addScaledVector(n, r * 0.5)); return lineObj(pts, greenLine); }
function simplify(pts, tol) { if (pts.length < 3) return pts; const out = [pts[0]]; for (let i = 1; i < pts.length - 1; i++) if (pts[i].distanceTo(out[out.length - 1]) > tol * 3) out.push(pts[i]); out.push(pts[pts.length - 1]); return out; }
function curveTool(kind) {
  return {
    help: (kind === 'cv' ? 'CV Curve Tool' : 'EP Curve Tool') + ': click to place points (hold X to snap to grid, V to points). Enter to complete, Backspace deletes the last point',
    settings: [{ key: 'degree', label: 'Curve degree', type: 'enum', options: ['1 Linear', '2', '3 Cubic', '5', '7'], values: [1, 2, 3, 5, 7], get: () => opt(kind + 'Degree', 3), set: (v) => setOpt(kind + 'Degree', v) }, { key: 'closed', label: 'Close curve on complete', type: 'bool', get: () => opt(kind + 'Closed', false), set: (v) => setOpt(kind + 'Closed', v) }],
    start() { st.pts = []; },
    down(vp, e, px) { const g = vp.groundPoint(px); if (g) { st.pts.push(g); this.draw(); } return true; },
    hover(vp, e, px) { st.hoverPt = vp.groundPoint(px); this.draw(); return true; },
    draw() {
      const pts = st.hoverPt ? [...st.pts, st.hoverPt] : st.pts;
      overlay(() => {
        const out = [ptsObj(st.pts)];
        if (pts.length >= 2) { const c = new Curve(pts.map(p => p.toArray()), Math.min(opt(kind + 'Degree', 3), pts.length - 1), 'open', kind); out.push(lineObj(c.sample(12), solidLine)); if (kind === 'cv') out.push(lineObj(pts, toolLineMat)); }
        return out;
      });
    },
    back() { st.pts.pop(); this.draw(); },
    complete() {
      if (st.pts.length < 2) { App.help('Place at least 2 points'); return; }
      Undo.checkpoint(kind + ' curve');
      const deg = Math.min(opt(kind + 'Degree', 3), st.pts.length - 1);
      const c = new Curve(st.pts.map(p => p.toArray()), deg, opt(kind + 'Closed', false) ? 'periodic' : 'open', kind);
      const o = S.createCurve(c, { name: 'curve1' }); Sel.select([o]);
      App.emit('echo', `curve -d ${deg} ${kind === 'ep' ? '-ep' : '-p'} ${st.pts.map(p => p.toArray().map(x => +x.toFixed(3)).join(' ')).join(kind === 'ep' ? ' -ep ' : ' -p ')} ;`); App.emit('result', o.inca.name);
      st.pts = []; st.hoverPt = null; this.draw();
    },
  };
}

// ------------------------------------------------------------------ dispatcher
App.tools = {
  settings: Object.fromEntries(Object.entries(TOOLS).filter(([, t]) => t.settings).map(([k, t]) => [k, t.settings])),
  isInteractive: (n) => INTERACTIVE.has(n),
  iconOf: (n) => ICONS[n],
  register(name, impl, icon = null) { TOOLS[name] = impl; INTERACTIVE.add(name); if (icon) ICONS[name] = icon; if (impl.settings) this.settings[name] = impl.settings; },
  activate(name) { const t = TOOLS[name]; st = { name, t }; t.start?.(); if (t.help) App.help(t.help); },
  deactivate() { if (st?.t?.complete && (st.pts?.length)) { /* Maya completes curves when switching tools */ if (st.name === 'cvCurve' || st.name === 'epCurve') st.t.complete(); } st?.t?.end?.(); st = null; overlay(null); },
  handleDown(vp, e, px) { if (!st || App.tool !== st.name) return false; return !!st.t.down?.(vp, e, px); },
  handleHover(vp, e, px) { if (!st || App.tool !== st.name || !st.t.hover) return false; const now = performance.now(); if (now - (st._ht || 0) < 25) return true; st._ht = now; return st.t.hover(vp, e, px); },
  handleKey(e, combo) {
    if (!st) return false;
    if (combo === 'Enter') { st.t.complete?.(); if (st && ['cvCurve', 'epCurve', 'createPoly'].includes(st.name)) {} return !!st?.t.complete; }
    if (combo === 'Backspace' && (st.pts?.length || st.t.canBack?.())) { st.t.back?.(); return true; }
    if (combo === 'Escape') { if (st.pts?.length) { st.pts = []; st.obj = null; st.t.draw?.(); } else App.setTool('select'); return true; }
    if (combo === 'Q' || combo === 'W' || combo === 'E' || combo === 'R') { st.t.complete?.(); return false; }
    return false;
  },
  complete() { st?.t.complete?.(); },
  cancel() { if (st) { st.pts = []; st.t.draw?.(); } },
};
// right-click completes the multi-cut (handled before marking menus)
addEventListener('mousedown', (e) => { if (e.button === 2 && st && (st.name === 'multiCut' || st.name === 'createPoly' || st.name === 'cvCurve' || st.name === 'epCurve') && st.pts?.length && e.target.closest?.('.panel-view')) { e.stopPropagation(); e.preventDefault(); st.t.complete?.(); } }, true);

// ------------------------------------------------------------------ commands
C('pencilCurveTool', 'Pencil Curve Tool', () => App.setTool('pencilCurve'), { icon: 'pencilCurve', noRepeat: true });
C('sculptTool', 'Sculpt Tool', () => { setOpt('sculptBrush', 'sculpt'); App.setTool('sculpt'); App.ui.toggleToolSettings(true); }, { icon: 'softSelect', noRepeat: true });
for (const [b, l] of [['smooth', 'Smooth'], ['relax', 'Relax'], ['grab', 'Grab'], ['pinch', 'Pinch'], ['flatten', 'Flatten'], ['inflate', 'Inflate']]) C(b + 'SculptTool', l + ' Sculpt Tool', () => { setOpt('sculptBrush', b); App.setTool('sculpt'); App.dirty('toolSettings'); }, { icon: 'softSelect', noRepeat: true });
C('makeLive', 'Make Live', () => { const o = Sel.lead(); App.liveSurface = App.liveSurface === o ? null : (o?.inca.kind === 'mesh' ? o : null); App.help(App.liveSurface ? 'Live surface: ' + o.inca.name : 'No live surface'); App.dirty('statusline'); }, { noRepeat: true });
