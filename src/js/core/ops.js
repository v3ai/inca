// Inca — construction-history node types. Each history entry: { id, name, type, params }.
import * as P from './polymesh.js';
import { GENERATORS, GEN_INFO } from './primitives.js';
import { Curve, revolve, loft, extrudeAlong, planar } from './curves.js';
import { App } from './app.js';

// channel description: [param, type, {min,max,options,label}]
const num = (k, o = {}) => ({ k, type: 'float', ...o });
const int = (k, o = {}) => ({ k, type: 'int', ...o });
const bool = (k, o = {}) => ({ k, type: 'bool', ...o });
const en = (k, options, o = {}) => ({ k, type: 'enum', options, ...o });

const genChannels = {
  polyCube: [num('width', { min: 0.0001 }), num('height', { min: 0.0001 }), num('depth', { min: 0.0001 }), int('subdivisionsWidth', { min: 1, max: 200 }), int('subdivisionsHeight', { min: 1, max: 200 }), int('subdivisionsDepth', { min: 1, max: 200 })],
  polySphere: [num('radius', { min: 0.0001 }), int('subdivisionsAxis', { min: 3, max: 200 }), int('subdivisionsHeight', { min: 2, max: 200 })],
  polyCylinder: [num('radius', { min: 0.0001 }), num('height', { min: 0.0001 }), int('subdivisionsAxis', { min: 3, max: 200 }), int('subdivisionsHeight', { min: 1, max: 200 }), int('subdivisionsCaps', { min: 0, max: 50 })],
  polyCone: [num('radius', { min: 0.0001 }), num('height', { min: 0.0001 }), int('subdivisionsAxis', { min: 3, max: 200 }), int('subdivisionsHeight', { min: 1, max: 200 }), int('subdivisionsCap', { min: 0, max: 50 })],
  polyPlane: [num('width', { min: 0.0001 }), num('height', { min: 0.0001 }), int('subdivisionsWidth', { min: 1, max: 300 }), int('subdivisionsHeight', { min: 1, max: 300 })],
  polyTorus: [num('radius', { min: 0.0001 }), num('sectionRadius', { min: 0.0001 }), num('twist'), int('subdivisionsAxis', { min: 3, max: 200 }), int('subdivisionsHeight', { min: 3, max: 200 })],
  polyPyramid: [num('sideLength', { min: 0.0001 }), int('numberOfSides', { min: 3, max: 5 })],
  polyPrism: [num('length', { min: 0.0001 }), num('sideLength', { min: 0.0001 }), int('numberOfSides', { min: 3, max: 100 })],
  polyPipe: [num('radius', { min: 0.0001 }), num('height', { min: 0.0001 }), num('thickness', { min: 0.0001 }), int('subdivisionsAxis', { min: 3, max: 200 }), int('subdivisionsHeight', { min: 1, max: 200 })],
  polyHelix: [num('coils', { min: 0.1 }), num('height', { min: 0.0001 }), num('width', { min: 0.0001 }), num('radius', { min: 0.0001 }), int('subdivisionsAxis', { min: 3, max: 100 }), int('subdivisionsCoil', { min: 3, max: 200 }), en('direction', ['Clockwise', 'Counterclockwise'], { values: [-1, 1] })],
  polyDisc: [num('radius', { min: 0.0001 }), int('sides', { min: 3, max: 200 }), int('subdivisions', { min: 1, max: 50 })],
  polyPlatonic: [en('primitive', ['Tetrahedron', 'Octahedron', 'Icosahedron', 'Dodecahedron']), num('radius', { min: 0.0001 })],
  nurbsSphere: [num('radius', { min: 0.0001 }), int('sections', { min: 4, max: 50 }), int('spans', { min: 2, max: 50 })],
  nurbsCylinder: [num('radius', { min: 0.0001 }), num('height', { min: 0.0001 }), int('sections', { min: 4, max: 50 }), int('spans', { min: 1, max: 50 })],
  nurbsCone: [num('radius', { min: 0.0001 }), num('height', { min: 0.0001 }), int('sections', { min: 4, max: 50 }), int('spans', { min: 1, max: 50 })],
  nurbsTorus: [num('radius', { min: 0.0001 }), num('sectionRadius', { min: 0.0001 }), int('sections', { min: 4, max: 50 }), int('spans', { min: 3, max: 50 })],
  nurbsPlane: [num('width', { min: 0.0001 }), num('lengthRatio', { min: 0.0001 }), int('patchesU', { min: 1, max: 50 }), int('patchesV', { min: 1, max: 50 })],
  nurbsCube: [num('width', { min: 0.0001 }), num('lengthRatio', { min: 0.0001 }), num('heightRatio', { min: 0.0001 }), int('patchesU', { min: 1, max: 50 }), int('patchesV', { min: 1, max: 50 })],
};

const sel2 = (r) => r;
function curveOf(id) {
  const o = App.nodes.get(id);
  if (!o || !o.inca || !o.inca.curve) return null;
  // curve CVs in the curve object's world space
  const c = o.inca.curve.clone(); o.updateWorldMatrix(true, false);
  const m = o.matrixWorld.elements;
  c.cvs = c.cvs.map(p => [m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12], m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13], m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14]]);
  return c;
}

export const OPS = {
  meshData: { hidden: true, apply: (_pm, p) => ({ mesh: P.PolyMesh.fromJSON(p.data) }) },
  polyTweak: { hidden: true, apply: (pm, p) => P.opTweak(pm, p) },
  polyExtrudeFace: { base: 'polyExtrudeFace', channels: [num('thickness'), num('offset', { min: -1, max: 1 }), int('divisions', { min: 1, max: 50 }), bool('keepFacesTogether')], apply: (pm, p) => P.opExtrudeFaces(pm, p) },
  polyExtrudeEdge: { base: 'polyExtrudeEdge', channels: [num('thickness'), num('offset')], apply: (pm, p) => P.opExtrudeEdges(pm, p) },
  polyBevel: { base: 'polyBevel', channels: [num('fraction', { min: 0.001, max: 1 }), int('segments', { min: 1, max: 1 })], apply: (pm, p) => P.opBevel(pm, p) },
  polyChamferVtx: { base: 'polyChamfer', channels: [num('width', { min: 0.001 })], apply: (pm, p) => P.opChamferVertex(pm, p) },
  polySplitRing: { base: 'polySplitRing', channels: [num('t', { label: 'Weight', min: 0.001, max: 0.999 }), int('multiple', { label: 'Number Of Edge Loops', min: 1, max: 50 })], apply: (pm, p) => P.opInsertEdgeLoop(pm, p) },
  polySplit: { base: 'polySplit', channels: [], apply: (pm, p) => P.opSplit(pm, p) },
  deleteComponent: { base: 'deleteComponent', channels: [], apply: (pm, p) => p.faces ? P.opDeleteFaces(pm, p) : p.edges ? P.opDeleteEdges(pm, p) : P.opDeleteVertices(pm, p) },
  polyMergeVert: { base: 'polyMergeVert', channels: [num('threshold', { min: 0 })], apply: (pm, p) => P.opMergeVertices(pm, p) },
  polyMergeCenter: { base: 'polyMergeVert', channels: [], apply: (pm, p) => P.opMergeToCenter(pm, p) },
  polySmoothFace: { base: 'polySmoothFace', channels: [int('divisions', { min: 0, max: 4 })], apply: (pm, p) => P.opSmooth(pm, p) },
  polySubdFace: { base: 'polySubdFace', channels: [int('divisions', { min: 1, max: 4 })], apply: (pm, p) => P.opAddDivisions(pm, p) },
  polyTriangulate: { base: 'polyTriangulate', channels: [], apply: (pm, p) => P.opTriangulate(pm, p) },
  polyQuad: { base: 'polyQuad', channels: [num('angle', { min: 0, max: 180 })], apply: (pm, p) => P.opQuadrangulate(pm, p) },
  polyNormal: { base: 'polyNormal', channels: [], apply: (pm, p) => P.opReverse(pm, p) },
  polyPoke: { base: 'polyPoke', channels: [], apply: (pm, p) => P.opPoke(pm, p) },
  polyCloseBorder: { base: 'polyCloseBorder', channels: [], apply: (pm, p) => P.opFillHole(pm, p) },
  polyMirror: { base: 'polyMirror', channels: [en('axis', ['X', 'Y', 'Z']), num('position'), bool('merge'), num('threshold', { min: 0 })], apply: (pm, p) => P.opMirror(pm, p) },
  polyBridgeEdge: { base: 'polyBridgeEdge', channels: [int('divisions', { min: 0, max: 100 })], apply: (pm, p) => P.opBridge(pm, p) },
  polySoftEdge: { base: 'polySoftEdge', channels: [num('angle', { min: 0, max: 180 })], apply: (pm, p) => P.opSoftEdge(pm, p) },
  polyProj: { base: 'polyProj', channels: [en('mode', ['planar', 'cylindrical', 'spherical', 'automatic'], { values: ['planar', 'cylindrical', 'spherical', 'automatic'] }), en('axis', ['X', 'Y', 'Z'])], apply: (pm, p) => P.opProjectUV(pm, p) },
  revolve: { base: 'revolve', channels: [en('axis', ['X', 'Y', 'Z']), num('startSweep'), num('endSweep'), int('sections', { min: 1, max: 50 }), int('segments', { label: 'Curve Segments', min: 1, max: 50 })],
    apply: (_pm, p) => { const c = curveOf(p.curve) || (p.curveData ? Curve.fromJSON(p.curveData) : null); return { mesh: c ? revolve(c, p) : new P.PolyMesh() }; } },
  loft: { base: 'loft', channels: [int('segments', { min: 1, max: 50 }), bool('close')],
    apply: (_pm, p) => { const cs = (p.curves || []).map(curveOf).filter(Boolean); return { mesh: cs.length >= 2 ? loft(cs, p) : (p.curveData ? loft(p.curveData.map(Curve.fromJSON), p) : new P.PolyMesh()) }; } },
  extrude: { base: 'extrude', channels: [int('segments', { min: 1, max: 50 }), num('scale', { min: 0 }), num('twist')],
    apply: (_pm, p) => { const a = curveOf(p.profile), b = curveOf(p.path); return { mesh: a && b ? extrudeAlong(a, b, p) : new P.PolyMesh() }; } },
  planarTrim: { base: 'planarTrimSurface', channels: [int('segments', { min: 1, max: 50 })],
    apply: (_pm, p) => { const cs = (p.curves || []).map(curveOf).filter(Boolean); return { mesh: planar(cs, p) }; } },
};
for (const [k, g] of Object.entries(GENERATORS)) {
  OPS[k] = { base: k, generator: true, nurbs: !!GEN_INFO[k].nurbs, channels: genChannels[k] || [], apply: (_pm, p) => ({ mesh: g(p) }) };
}

export function niceName(k) {
  if (k === 't') return 'Weight';
  return k.replace(/([A-Z])/g, ' $1').replace(/^./, c => c.toUpperCase());
}

// evaluate a history stack -> { mesh, select }
export function evalHistory(history) {
  let pm = null; let select = null;
  for (const h of history) {
    const op = OPS[h.type];
    if (!op) { console.warn('unknown op', h.type); continue; }
    try {
      const r = op.apply(pm ? pm : new P.PolyMesh(), h.params);
      pm = r.mesh; select = r.select || null;
      if (r.error) h._error = r.error; else delete h._error;
    } catch (e) { console.error('history op failed', h.type, e); h._error = String(e); }
  }
  return { mesh: pm || new P.PolyMesh(), select };
}
