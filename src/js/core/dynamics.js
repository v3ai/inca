// Inca — dynamics / FX: emitters, particle systems, fields, rigid bodies and fire/smoke presets.
// Nodes: DAG kinds 'emitter', 'particles', 'field' (registered through scene.js DAG_FACTORIES) plus rigid-body
// flags on mesh nodes. User-editable attributes live in inca.extraAttrs (flat values); internal links and the rest
// pose live in inca.extra.dyn (both serialized by scene.js).
// Simulation: App.dynamics.step(t) is called by App.setTime after animation evaluation. Frames are integrated with
// sub-steps, cached per frame (scrubbing back shows the cache) and reset at the start frame.
import * as THREE from 'three';
import { App } from './app.js';
import * as S from './scene.js';
import { Undo } from './undo.js';
import { Sel } from './selection.js';
import { uniqueName } from './names.js';

const C = (...a) => App.cmds.register(...a);
const KINDS = new Set(['emitter', 'particles', 'field']);
const D2R = Math.PI / 180;
const SIM_LIMIT = 1000;       // max frames simulated in one step() call when jumping forward
const CACHE_MAX = 4000;       // max cached frames
const COL = { plain: new THREE.Color(0x10106e), sel: new THREE.Color(0xffffff), lead: new THREE.Color(0x5cf5a2) };

// ------------------------------------------------------------------ attribute defaults
const EMITTER_DEFAULTS = { emitterType: 'omni', rate: 100, speed: 1, speedRandom: 0, spread: 0, directionX: 1, directionY: 0, directionZ: 0, lifespan: 5, lifespanRandom: 0, volumeShape: 'cube', isFull: true, enabled: true };
const PARTICLE_DEFAULTS = { renderType: 'points', pointSize: 4, radius: 0.15, radiusEnd: 0.15, color: [0.25, 0.6, 1], colorEnd: [0.25, 0.6, 1], opacity: 1, opacityEnd: 1, additive: false, maxCount: 20000, conserve: 1, collide: true, enabled: true };
const FIELD_DEFAULTS = {
  gravity: { magnitude: 9.8, directionX: 0, directionY: -1, directionZ: 0, attenuation: 0, useMaxDistance: false, maxDistance: 20 },
  turbulence: { magnitude: 5, frequency: 1, phaseX: 0, phaseY: 0, phaseZ: 0, attenuation: 0, useMaxDistance: false, maxDistance: 20 },
  radial: { magnitude: 5, attenuation: 1, useMaxDistance: true, maxDistance: 20 },
  drag: { magnitude: 0.5, attenuation: 0, useMaxDistance: false, maxDistance: 20 },
  air: { magnitude: 4, speed: 5, directionX: 0, directionY: 1, directionZ: 0, attenuation: 0, useMaxDistance: false, maxDistance: 20 },
  vortex: { magnitude: 5, axisX: 0, axisY: 1, axisZ: 0, attenuation: 1, useMaxDistance: false, maxDistance: 20 },
};
const RIGID_DEFAULTS = { rigidBody: 'active', mass: 1, bounciness: 0.6, staticFriction: 0.2, dynamicFriction: 0.2, damping: 0, standIn: 'cube', collisions: true, initialVelocityX: 0, initialVelocityY: 0, initialVelocityZ: 0, initialSpinX: 0, initialSpinY: 0, initialSpinZ: 0 };
const RIGID_KEYS = Object.keys(RIGID_DEFAULTS);

const ea = (o) => o.inca.extraAttrs || (o.inca.extraAttrs = {});
const dyn = (o) => { const x = o.inca.extra || (o.inca.extra = {}); return x.dyn || (x.dyn = {}); };
const num = (v, d = 0) => (v === undefined || v === null || isNaN(+v)) ? d : +v;
const vec = (a, k) => new THREE.Vector3(num(a[k + 'X']), num(a[k + 'Y']), num(a[k + 'Z']));
const isRigid = (o) => o && o.inca && o.inca.kind === 'mesh' && o.inca.extraAttrs && (o.inca.extraAttrs.rigidBody === 'active' || o.inca.extraAttrs.rigidBody === 'passive');
const shapeNameFor = (n) => n.match(/\d+$/) ? n.replace(/(\d+)$/, 'Shape$1') : n + 'Shape';

// ------------------------------------------------------------------ icons
function iconGeom(kind, type, a = {}) {
  const p = []; const seg = (x, y) => p.push(...x, ...y);
  const circle = (r, axis = 'y', off = [0, 0, 0], n = 24) => { for (let i = 0; i < n; i++) { const t0 = i / n * Math.PI * 2, t1 = (i + 1) / n * Math.PI * 2; const P = (t) => { const c = Math.cos(t) * r, s = Math.sin(t) * r; const q = axis === 'y' ? [c, 0, s] : axis === 'z' ? [c, s, 0] : [0, c, s]; return [q[0] + off[0], q[1] + off[1], q[2] + off[2]]; }; seg(P(t0), P(t1)); } };
  const arrow = (from, to, hs = 0.12) => { seg(from, to); const d = new THREE.Vector3(...to).sub(new THREE.Vector3(...from)).normalize(); const side = Math.abs(d.y) > 0.9 ? new THREE.Vector3(1, 0, 0) : new THREE.Vector3(0, 1, 0); const s = side.cross(d).normalize().multiplyScalar(hs); const b = new THREE.Vector3(...to).addScaledVector(d, -hs * 2); seg(to, b.clone().add(s).toArray()); seg(to, b.clone().sub(s).toArray()); };
  const box = (h) => { const c = [[-h, -h, -h], [h, -h, -h], [h, h, -h], [-h, h, -h], [-h, -h, h], [h, -h, h], [h, h, h], [-h, h, h]]; [[0, 1], [1, 2], [2, 3], [3, 0], [4, 5], [5, 6], [6, 7], [7, 4], [0, 4], [1, 5], [2, 6], [3, 7]].forEach(([i, j]) => seg(c[i], c[j])); };
  if (kind === 'emitter') {
    if (type === 'volume') { if (a.volumeShape === 'sphere') { circle(1, 'y'); circle(1, 'z'); circle(1, 'x'); } else box(1); circle(0.15, 'y'); }
    else if (type === 'directional') { circle(0.2, 'y'); circle(0.2, 'z'); circle(0.2, 'x'); arrow([0, 0, 0], [num(a.directionX, 1) * 0.8, num(a.directionY) * 0.8, num(a.directionZ) * 0.8]); }
    else if (type === 'surface' || type === 'vertex') { circle(0.15, 'y'); circle(0.15, 'z'); circle(0.15, 'x'); for (const d of [[1, 0, 0], [0, 1, 0], [0, 0, 1]]) { seg(d.map(x => x * 0.2), d.map(x => x * 0.4)); seg(d.map(x => -x * 0.2), d.map(x => -x * 0.4)); } }
    else { circle(0.25, 'y'); circle(0.25, 'z'); circle(0.25, 'x'); for (const d of [[1, 0, 0], [0, 1, 0], [0, 0, 1], [0.7, 0.7, 0], [-0.7, 0.7, 0], [0, 0.7, 0.7], [0, 0.7, -0.7]]) { seg(d.map(x => x * 0.3), d.map(x => x * 0.55)); seg(d.map(x => -x * 0.3), d.map(x => -x * 0.55)); } }
  } else if (kind === 'field') {
    if (type === 'gravity') { for (const x of [-0.3, 0, 0.3]) arrow([x, 0.5, 0], [x, -0.5, 0]); circle(0.45, 'y', [0, 0.5, 0], 16); }
    else if (type === 'turbulence') { for (const z of [-0.3, 0, 0.3]) { let prev = null; for (let i = 0; i <= 20; i++) { const x = -0.6 + i * 0.06; const q = [x, Math.sin(i * 0.9 + z * 6) * 0.15 + z * 0.6, z]; if (prev) seg(prev, q); prev = q; } } }
    else if (type === 'radial') { for (const d of [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1], [0.7, 0.7, 0], [-0.7, -0.7, 0], [0.7, -0.7, 0], [-0.7, 0.7, 0]]) arrow(d.map(x => x * 0.15), d.map(x => x * 0.6), 0.07); }
    else if (type === 'drag') { circle(0.5, 'y', [0, 0.3, 0], 16); for (let i = 0; i < 8; i++) { const t = i / 8 * Math.PI * 2; seg([Math.cos(t) * 0.5, 0.3, Math.sin(t) * 0.5], [0, -0.4, 0]); } }
    else if (type === 'air') { circle(0.5, 'z', [0, 0, 0], 20); for (let i = 0; i < 3; i++) { const t = i / 3 * Math.PI * 2; seg([0, 0, 0], [Math.cos(t) * 0.5, Math.sin(t) * 0.5, 0]); } arrow([0, 0, 0], [num(a.directionX) * 0.9, num(a.directionY, 1) * 0.9, num(a.directionZ) * 0.9]); }
    else if (type === 'vortex') { let prev = null; for (let i = 0; i <= 60; i++) { const t = i / 60 * Math.PI * 5; const r = 0.08 + i / 60 * 0.5; const q = [Math.cos(t) * r, (i / 60 - 0.5) * 0.5, Math.sin(t) * r]; if (prev) seg(prev, q); prev = q; } arrow([0, -0.4, 0], [0, 0.6, 0], 0.08); }
  }
  const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(p, 3)); return g;
}
function selColor(o) { const i = App.sel.indexOf(o); if (i < 0) return COL.plain; return i === App.sel.length - 1 ? COL.lead : COL.sel; }
function attachIcon(o) {
  const ud = o.userData; const inc = o.inca; const a = ea(o);
  const type = inc.kind === 'emitter' ? a.emitterType : dyn(o).fieldType;
  const key = inc.kind + ':' + type + ':' + (a.volumeShape || '') + ':' + [a.directionX, a.directionY, a.directionZ].join(',');
  if (ud.icon && ud._iconKey === key) return;
  ud._iconKey = key;
  if (!ud.icon) {
    const mat = new THREE.LineBasicMaterial({ color: COL.plain.clone() });
    const l = S.helper(new THREE.LineSegments(iconGeom(inc.kind, type, a), mat)); l.userData.owner = o;
    l.onBeforeRender = () => { mat.color.copy(selColor(o)); };
    o.add(l); ud.icon = l; ud.wire = l;
    S.addPick(o, new THREE.BoxGeometry(1.2, 1.2, 1.2));
  } else { ud.icon.geometry.dispose(); ud.icon.geometry = iconGeom(inc.kind, type, a); }
  App.requestRender();
}

// ------------------------------------------------------------------ particle rendering
const PVERT = `attribute vec3 pcolor; attribute float palpha; attribute float psize;
uniform float uScale; uniform float uPersp; uniform float uWorld; uniform float uPx;
varying vec3 vColor; varying float vAlpha;
void main() { vec4 mv = modelViewMatrix * vec4(position, 1.0); gl_Position = projectionMatrix * mv;
  vColor = pcolor; vAlpha = palpha;
  float s = uWorld > 0.5 ? psize * uScale / (uPersp > 0.5 ? max(1e-3, -mv.z) : 1.0) : psize * uPx;
  gl_PointSize = clamp(s, 1.0, 512.0); }`;
const PFRAG = `uniform float uWorld; uniform vec3 uSel; uniform float uSelOn;
varying vec3 vColor; varying float vAlpha;
void main() { vec2 c = gl_PointCoord - 0.5; float d = length(c); if (d > 0.5) discard;
  vec3 col = mix(vColor, uSel, uSelOn * 0.6);
  float a = vAlpha;
  if (uWorld > 0.5) a *= smoothstep(0.5, 0.15, d); else col *= 1.0 - d * 0.6;
  if (a < 0.003) discard;
  gl_FragColor = vec4(col, a); }`;
const _vp = new THREE.Vector4();
function attachPoints(o) {
  const ud = o.userData; if (ud.points) return;
  const g = new THREE.BufferGeometry();
  const mat = new THREE.ShaderMaterial({ vertexShader: PVERT, fragmentShader: PFRAG, transparent: true, depthWrite: false,
    uniforms: { uScale: { value: 500 }, uPersp: { value: 1 }, uWorld: { value: 0 }, uPx: { value: 1 }, uSel: { value: new THREE.Color(1, 1, 1) }, uSelOn: { value: 0 } } });
  const pts = S.helper(new THREE.Points(g, mat));
  pts.frustumCulled = false; pts.matrixAutoUpdate = false; pts.matrixWorldAutoUpdate = false; pts.matrixWorld.identity();
  pts.userData.owner = o; pts.userData.isPick = true;
  pts.onBeforeRender = (renderer, scene, camera) => {
    pts.matrixWorld.identity();
    renderer.getCurrentViewport(_vp); const vh = _vp.w || 500; const u = mat.uniforms; const a = ea(o);
    if (camera.isPerspectiveCamera) { u.uPersp.value = 1; u.uScale.value = vh / (2 * Math.tan(camera.fov * D2R / 2)); }
    else { u.uPersp.value = 0; u.uScale.value = vh * camera.zoom / Math.max(1e-6, camera.top - camera.bottom); }
    u.uPx.value = renderer.getPixelRatio();
    u.uWorld.value = a.renderType === 'points' ? 0 : 1;
    const i = App.sel.indexOf(o); u.uSelOn.value = i < 0 ? 0 : 1; u.uSel.value.copy(i === App.sel.length - 1 ? COL.lead : COL.sel);
    const add = !!a.additive; if (mat.blending !== (add ? THREE.AdditiveBlending : THREE.NormalBlending)) { mat.blending = add ? THREE.AdditiveBlending : THREE.NormalBlending; mat.needsUpdate = true; }
  };
  o.add(pts); ud.points = pts; ud.pick = pts; ud.wire = null;
}
function writePoints(o, P) {
  const pts = o.userData.points; if (!pts) return;
  const a = ea(o); const n = P ? P.n : 0;
  let g = pts.geometry;
  const cap = g.userData.cap || 0;
  if (n > cap) {
    const nc = Math.max(64, 1 << Math.ceil(Math.log2(n)));
    g.dispose(); g = new THREE.BufferGeometry(); g.userData.cap = nc;
    g.setAttribute('position', new THREE.BufferAttribute(new Float32Array(nc * 3), 3));
    g.setAttribute('pcolor', new THREE.BufferAttribute(new Float32Array(nc * 3), 3));
    g.setAttribute('palpha', new THREE.BufferAttribute(new Float32Array(nc), 1));
    g.setAttribute('psize', new THREE.BufferAttribute(new Float32Array(nc), 1));
    pts.geometry = g;
  }
  if (!n) { g.setDrawRange(0, 0); App.requestRender(); return; }
  const pos = g.attributes.position.array, col = g.attributes.pcolor.array, al = g.attributes.palpha.array, sz = g.attributes.psize.array;
  const c0 = a.color || [1, 1, 1], c1 = a.colorEnd || c0; const o0 = num(a.opacity, 1), o1 = num(a.opacityEnd, o0);
  const world = a.renderType !== 'points'; const r0 = num(a.radius, 0.15), r1 = num(a.radiusEnd, r0), ps = num(a.pointSize, 4);
  pos.set(P.pos.subarray(0, n * 3));
  for (let i = 0; i < n; i++) {
    const life = P.life[i]; const t = life > 0 ? Math.min(1, P.age[i] / life) : 0;
    col[i * 3] = c0[0] + (c1[0] - c0[0]) * t; col[i * 3 + 1] = c0[1] + (c1[1] - c0[1]) * t; col[i * 3 + 2] = c0[2] + (c1[2] - c0[2]) * t;
    al[i] = Math.max(0, Math.min(1, o0 + (o1 - o0) * t));
    sz[i] = world ? 2 * (r0 + (r1 - r0) * t) : ps;
  }
  for (const k of ['position', 'pcolor', 'palpha', 'psize']) g.attributes[k].needsUpdate = true;
  g.setDrawRange(0, n);
  g.boundingSphere = null; g.computeBoundingSphere();
  App.requestRender();
}

// ------------------------------------------------------------------ node factories (also used by deserializeNode)
function createDynNode(kind, name, opts = {}, d = null) {
  const o = S.makeDag(kind, name, opts);
  o.inca.shapeName = (d && d.shapeName && opts.id && !App.names.has(d.shapeName)) ? uniqueName(d.shapeName) : uniqueName(shapeNameFor(o.inca.name));
  if (d) {
    if (d.extraAttrs) o.inca.extraAttrs = JSON.parse(JSON.stringify(d.extraAttrs));
    if (d.extra) o.inca.extra = JSON.parse(JSON.stringify(d.extra));
  }
  return o;
}
function buildVisuals(o) {
  const k = o.inca.kind;
  if (k === 'particles') { attachPoints(o); writePoints(o, rt.lastParts?.get(o.inca.id) || initialParticles(o)); }
  else if (k === 'emitter' || k === 'field') attachIcon(o);
}
for (const k of KINDS) S.DAG_FACTORIES[k] = (name, opts, d) => {
  const o = createDynNode(k, name, opts, d);
  // deserializeNode overwrites inca.extra / extraAttrs after the factory: rebuild visuals on the next tick
  queueMicrotask(() => { if (App.nodes.get(o.inca.id) === o) buildVisuals(o); });
  return o;
};

export function createEmitterNode(type = 'omni', { name = 'emitter1', parent = null, attrs = {}, particles = null } = {}) {
  const o = createDynNode('emitter', name, { parent });
  Object.assign(ea(o), JSON.parse(JSON.stringify(EMITTER_DEFAULTS)), { emitterType: type }, attrs);
  const p = particles || createParticleNode({});
  dyn(o).target = p.inca.id;
  buildVisuals(o);
  return { emitter: o, particles: p };
}
export function createParticleNode({ name = 'particle1', attrs = {}, initial = null } = {}) {
  const o = createDynNode('particles', name, {});
  Object.assign(ea(o), JSON.parse(JSON.stringify(PARTICLE_DEFAULTS)), attrs);
  if (initial) dyn(o).initial = initial.map(x => +x.toFixed(5));
  buildVisuals(o);
  return o;
}
export function createFieldNode(type, { name = null, attrs = {}, affects = null, parent = null } = {}) {
  const o = createDynNode('field', name || type + 'Field1', { parent });
  dyn(o).fieldType = type;
  Object.assign(ea(o), JSON.parse(JSON.stringify(FIELD_DEFAULTS[type])), attrs);
  if (affects) dyn(o).affects = affects.slice();
  buildVisuals(o);
  return o;
}

// ------------------------------------------------------------------ deterministic random / noise
function rng(seed) { let s = seed >>> 0; return () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }; }
function hashStr(s) { let h = 2166136261; for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619); } return h >>> 0; }
function hash3(x, y, z) { let h = Math.imul(x, 374761393) + Math.imul(y, 668265263) + Math.imul(z, 2147483647); h = Math.imul(h ^ (h >>> 13), 1274126177); return ((h ^ (h >>> 16)) >>> 0) / 4294967296; }
function vnoise(x, y, z) {
  const xi = Math.floor(x), yi = Math.floor(y), zi = Math.floor(z); const xf = x - xi, yf = y - yi, zf = z - zi;
  const u = xf * xf * (3 - 2 * xf), v = yf * yf * (3 - 2 * yf), w = zf * zf * (3 - 2 * zf);
  const L = (a, b, t) => a + (b - a) * t;
  const c = (i, j, k) => hash3(xi + i, yi + j, zi + k);
  return L(L(L(c(0, 0, 0), c(1, 0, 0), u), L(c(0, 1, 0), c(1, 1, 0), u), v), L(L(c(0, 0, 1), c(1, 0, 1), u), L(c(0, 1, 1), c(1, 1, 1), u), v), w) * 2 - 1;
}

// ------------------------------------------------------------------ fields
function gatherFields() {
  const out = [];
  for (const o of S.allDag()) {
    if (o.inca.kind !== 'field' || !o.visible) continue;
    const a = ea(o); const type = dyn(o).fieldType; o.updateWorldMatrix(true, false);
    const pos = new THREE.Vector3().setFromMatrixPosition(o.matrixWorld);
    const rot = new THREE.Matrix3().setFromMatrix4(o.matrixWorld);
    const f = { id: o.inca.id, type, a, pos, mag: num(a.magnitude), att: num(a.attenuation), maxD: a.useMaxDistance ? num(a.maxDistance, 20) : -1, affects: dyn(o).affects ? new Set(dyn(o).affects) : null };
    if (type === 'gravity' || type === 'air') { f.dir = vec(a, 'direction').applyMatrix3(rot); if (f.dir.lengthSq() < 1e-12) f.dir.set(0, -1, 0); f.dir.normalize(); }
    if (type === 'vortex') { f.dir = vec(a, 'axis').applyMatrix3(rot); if (f.dir.lengthSq() < 1e-12) f.dir.set(0, 1, 0); f.dir.normalize(); }
    if (type === 'air') f.speed = num(a.speed, 5);
    if (type === 'turbulence') { f.freq = num(a.frequency, 1); f.phase = [num(a.phaseX), num(a.phaseY), num(a.phaseZ)]; }
    out.push(f);
  }
  return out;
}
// acceleration on a point (x,y,z) with velocity (vx,vy,vz) added into acc[0..2]
function fieldAccel(fields, targetId, x, y, z, vx, vy, vz, time, acc) {
  for (const f of fields) {
    if (f.affects && !f.affects.has(targetId)) continue;
    const dx = x - f.pos.x, dy = y - f.pos.y, dz = z - f.pos.z; const d = Math.hypot(dx, dy, dz);
    let fall = 1;
    if (f.maxD >= 0) { if (d > f.maxD) continue; if (f.att) fall = Math.pow(Math.max(0, 1 - d / Math.max(1e-6, f.maxD)), f.att); }
    else if (f.att && (f.type === 'radial' || f.type === 'vortex' || f.type === 'turbulence')) fall = 1 / Math.pow(1 + d, f.att);
    const m = f.mag * fall;
    switch (f.type) {
      case 'gravity': acc[0] += f.dir.x * m; acc[1] += f.dir.y * m; acc[2] += f.dir.z * m; break;
      case 'radial': if (d > 1e-6) { acc[0] += dx / d * m; acc[1] += dy / d * m; acc[2] += dz / d * m; } break;
      case 'drag': acc[0] -= vx * m; acc[1] -= vy * m; acc[2] -= vz * m; break;
      case 'air': { const along = vx * f.dir.x + vy * f.dir.y + vz * f.dir.z; const k = Math.max(0, f.speed - along) * m * 0.25; acc[0] += f.dir.x * k; acc[1] += f.dir.y * k; acc[2] += f.dir.z * k; break; }
      case 'vortex': if (d > 1e-6) { const t = [f.dir.y * dz - f.dir.z * dy, f.dir.z * dx - f.dir.x * dz, f.dir.x * dy - f.dir.y * dx]; const tl = Math.hypot(...t); if (tl > 1e-6) { acc[0] += t[0] / tl * m; acc[1] += t[1] / tl * m; acc[2] += t[2] / tl * m; } } break;
      case 'turbulence': { const q = f.freq; const px = x * q + f.phase[0], py = y * q + f.phase[1], pz = z * q + f.phase[2] + time * 0.5;
        acc[0] += vnoise(px, py, pz) * m; acc[1] += vnoise(px + 31.7, py + 17.3, pz + 5.1) * m; acc[2] += vnoise(px + 11.1, py + 53.9, pz + 27.4) * m; break; }
    }
  }
}

// ------------------------------------------------------------------ particle state
function emptyParts(cap = 64) { return { n: 0, pos: new Float32Array(cap * 3), vel: new Float32Array(cap * 3), age: new Float32Array(cap), life: new Float32Array(cap) }; }
function growParts(P, need) {
  if (need * 3 <= P.pos.length) return;
  const cap = Math.max(need, P.pos.length / 3 * 2 | 0);
  for (const k of ['pos', 'vel']) { const a = new Float32Array(cap * 3); a.set(P[k]); P[k] = a; }
  for (const k of ['age', 'life']) { const a = new Float32Array(cap); a.set(P[k]); P[k] = a; }
}
function cloneParts(P) { return { n: P.n, pos: P.pos.slice(0, P.n * 3), vel: P.vel.slice(0, P.n * 3), age: P.age.slice(0, P.n), life: P.life.slice(0, P.n) }; }
function initialParticles(o) {
  const init = dyn(o).initial || []; const n = Math.floor(init.length / 3); const P = emptyParts(Math.max(64, n));
  for (let i = 0; i < n * 3; i++) P.pos[i] = init[i];
  P.n = n; for (let i = 0; i < n; i++) P.life[i] = -1;
  return P;
}

// surface sampling data for an emitting mesh (world space triangles)
function surfaceData(mesh) {
  const pm = mesh.inca.mesh; mesh.updateWorldMatrix(true, false); const M = mesh.matrixWorld; const nm = new THREE.Matrix3().getNormalMatrix(M);
  const W = pm.v.map(p => new THREE.Vector3(p[0], p[1], p[2]).applyMatrix4(M));
  const tris = []; const cum = []; let total = 0;
  const vn = W.map(() => new THREE.Vector3());
  pm.f.forEach((f) => {
    for (let i = 1; i + 1 < f.length; i++) {
      const a = W[f[0]], b = W[f[i]], c = W[f[i + 1]];
      const n = new THREE.Vector3().subVectors(b, a).cross(new THREE.Vector3().subVectors(c, a)); const ar = n.length() / 2;
      if (ar < 1e-12) continue; vn[f[0]].add(n); vn[f[i]].add(n); vn[f[i + 1]].add(n);
      tris.push([a, b, c, n.normalize()]); total += ar; cum.push(total);
    }
  });
  vn.forEach(n => n.lengthSq() > 0 ? n.normalize() : n.set(0, 1, 0));
  return { W, vn, tris, cum, total, nm };
}
function emit(e, P, partsNode, frame, dt, R) {
  const a = ea(e); if (a.enabled === false) return;
  const type = a.emitterType || 'omni';
  const st = R.emitAcc;
  const want = num(a.rate, 100) * dt + (st.get(e.inca.id) || 0);
  let count = Math.floor(want); st.set(e.inca.id, want - count);
  const maxCount = num(ea(partsNode).maxCount, 20000);
  count = Math.min(count, Math.max(0, maxCount - P.n));
  if (count <= 0) return;
  const rnd = rng(hashStr(e.inca.id) ^ Math.imul(frame + 1, 2654435761) ^ (R.sub * 7919));
  e.updateWorldMatrix(true, false); const M = e.matrixWorld;
  const origin = new THREE.Vector3().setFromMatrixPosition(M);
  const rot = new THREE.Matrix3().setFromMatrix4(M);
  const speed = num(a.speed, 1), sr = num(a.speedRandom), life = num(a.lifespan, 5), lr = num(a.lifespanRandom);
  let dir = vec(a, 'direction').applyMatrix3(rot); if (dir.lengthSq() < 1e-12) dir.set(1, 0, 0); dir.normalize();
  const spread = Math.max(0, Math.min(1, num(a.spread))) * Math.PI;
  let surf = null;
  if (type === 'surface' || type === 'vertex') {
    const src = App.nodes.get(dyn(e).source) || (e.parent && e.parent.inca && e.parent.inca.kind === 'mesh' ? e.parent : null);
    if (!src || !src.inca.mesh || !src.inca.mesh.v.length) return;
    const key = src.inca.id; surf = R.surf.get(key); if (!surf) { surf = surfaceData(src); R.surf.set(key, surf); }
    if (type === 'surface' && !surf.tris.length) return;
  }
  const randDir = (out) => { const u = rnd() * 2 - 1, t = rnd() * Math.PI * 2, s = Math.sqrt(1 - u * u); return out.set(s * Math.cos(t), u, s * Math.sin(t)); };
  const cone = (axis, ang, out) => { if (ang <= 1e-6) return out.copy(axis); const cosA = 1 - rnd() * (1 - Math.cos(ang)); const sinA = Math.sqrt(1 - cosA * cosA); const phi = rnd() * Math.PI * 2; const t1 = Math.abs(axis.y) < 0.9 ? new THREE.Vector3(0, 1, 0) : new THREE.Vector3(1, 0, 0); const b1 = t1.cross(axis).normalize(); const b2 = axis.clone().cross(b1); return out.copy(axis).multiplyScalar(cosA).addScaledVector(b1, sinA * Math.cos(phi)).addScaledVector(b2, sinA * Math.sin(phi)); };
  growParts(P, P.n + count);
  const p = new THREE.Vector3(), v = new THREE.Vector3(), tmp = new THREE.Vector3();
  for (let k = 0; k < count; k++) {
    const sp = speed * (1 + (rnd() * 2 - 1) * sr);
    if (type === 'directional') { p.copy(origin); cone(dir, spread, v).multiplyScalar(sp); }
    else if (type === 'volume') {
      if (a.volumeShape === 'sphere') { randDir(tmp).multiplyScalar(Math.cbrt(rnd())); } else tmp.set(rnd() * 2 - 1, rnd() * 2 - 1, rnd() * 2 - 1);
      p.copy(tmp).applyMatrix4(M); v.copy(p).sub(origin); if (v.lengthSq() < 1e-12) randDir(v); v.normalize().multiplyScalar(sp);
    } else if (type === 'surface') {
      const x = rnd() * surf.total; let lo = 0, hi = surf.cum.length - 1; while (lo < hi) { const m = (lo + hi) >> 1; if (surf.cum[m] < x) lo = m + 1; else hi = m; }
      const [A, B, Cc, n] = surf.tris[lo]; let r1 = rnd(), r2 = rnd(); if (r1 + r2 > 1) { r1 = 1 - r1; r2 = 1 - r2; }
      p.copy(A).addScaledVector(tmp.subVectors(B, A), r1).addScaledVector(new THREE.Vector3().subVectors(Cc, A), r2);
      cone(n, spread, v).multiplyScalar(sp);
    } else if (type === 'vertex') { const i = Math.floor(rnd() * surf.W.length); p.copy(surf.W[i]); cone(surf.vn[i], spread, v).multiplyScalar(sp); }
    else { p.copy(origin); randDir(v).multiplyScalar(sp); }
    // spread emission over the sub-step to avoid banding
    const off = rnd() * dt; p.addScaledVector(v, off);
    const i = P.n++;
    P.pos[i * 3] = p.x; P.pos[i * 3 + 1] = p.y; P.pos[i * 3 + 2] = p.z;
    P.vel[i * 3] = v.x; P.vel[i * 3 + 1] = v.y; P.vel[i * 3 + 2] = v.z;
    P.age[i] = off; P.life[i] = life > 0 ? Math.max(0.01, life * (1 + (rnd() * 2 - 1) * lr)) : -1;
  }
}
function stepParticles(o, P, fields, colliders, dt, time) {
  const a = ea(o); const id = o.inca.id; const acc = [0, 0, 0]; const cons = num(a.conserve, 1);
  let w = 0;
  for (let i = 0; i < P.n; i++) {
    const age = P.age[i] + dt; const life = P.life[i];
    if (life > 0 && age >= life) continue; // dies
    let x = P.pos[i * 3], y = P.pos[i * 3 + 1], z = P.pos[i * 3 + 2];
    let vx = P.vel[i * 3] * cons, vy = P.vel[i * 3 + 1] * cons, vz = P.vel[i * 3 + 2] * cons;
    acc[0] = acc[1] = acc[2] = 0;
    fieldAccel(fields, id, x, y, z, vx, vy, vz, time, acc);
    vx += acc[0] * dt; vy += acc[1] * dt; vz += acc[2] * dt;
    x += vx * dt; y += vy * dt; z += vz * dt;
    if (a.collide !== false) for (const c of colliders) {
      // particles collide with passive rigid bodies (oriented boxes)
      const lx = x - c.c.x, ly = y - c.c.y, lz = z - c.c.z;
      const q = [lx * c.ax[0].x + ly * c.ax[0].y + lz * c.ax[0].z, lx * c.ax[1].x + ly * c.ax[1].y + lz * c.ax[1].z, lx * c.ax[2].x + ly * c.ax[2].y + lz * c.ax[2].z];
      const m = 0.02; if (Math.abs(q[0]) > c.h[0] + m || Math.abs(q[1]) > c.h[1] + m || Math.abs(q[2]) > c.h[2] + m) continue;
      // previous position decides the face that was crossed
      const px = P.pos[i * 3] - c.c.x, py = P.pos[i * 3 + 1] - c.c.y, pz = P.pos[i * 3 + 2] - c.c.z;
      const pq = [px * c.ax[0].x + py * c.ax[0].y + pz * c.ax[0].z, px * c.ax[1].x + py * c.ax[1].y + pz * c.ax[1].z, px * c.ax[2].x + py * c.ax[2].y + pz * c.ax[2].z];
      let k = 0, best = -Infinity; for (let j = 0; j < 3; j++) { const s = Math.abs(pq[j]) - c.h[j]; if (s > best) { best = s; k = j; } }
      const s = pq[k] >= 0 ? 1 : -1; const n = c.ax[k];
      const depth = c.h[k] + m - s * q[k]; x += n.x * s * depth; y += n.y * s * depth; z += n.z * s * depth;
      const vn = (vx * n.x + vy * n.y + vz * n.z) * s;
      if (vn < 0) { const e = c.bounce; vx -= n.x * s * vn * (1 + e); vy -= n.y * s * vn * (1 + e); vz -= n.z * s * vn * (1 + e); const fr = 1 - Math.min(1, c.friction); const vn2 = (vx * n.x + vy * n.y + vz * n.z); vx = (vx - n.x * vn2) * fr + n.x * vn2; vy = (vy - n.y * vn2) * fr + n.y * vn2; vz = (vz - n.z * vn2) * fr + n.z * vn2; }
    }
    P.pos[w * 3] = x; P.pos[w * 3 + 1] = y; P.pos[w * 3 + 2] = z;
    P.vel[w * 3] = vx; P.vel[w * 3 + 1] = vy; P.vel[w * 3 + 2] = vz;
    P.age[w] = age; P.life[w] = life; w++;
  }
  P.n = w;
}

// ------------------------------------------------------------------ rigid bodies
const _m4 = new THREE.Matrix4(), _q = new THREE.Quaternion(), _v = new THREE.Vector3(), _s = new THREE.Vector3();
function restOf(o) { const d = dyn(o); return d.rest || null; }
function captureRest(o) { dyn(o).rest = { t: o.inca.t.slice(), r: o.inca.r.slice(), s: o.inca.s.slice() }; }
function restoreRest(o) { const r = restOf(o); if (!r) { captureRest(o); return; } o.inca.t = r.t.slice(); o.inca.r = r.r.slice(); o.inca.s = r.s.slice(); S.updateXform(o); }
// oriented box of a mesh node in world space: {c, ax:[3 unit vectors], h:[3], q (world rotation)}
function obbOf(o) {
  o.updateWorldMatrix(true, false); const W = o.matrixWorld;
  const pm = o.inca.mesh; const bb = pm.bbox();
  const lc = new THREE.Vector3((bb.min[0] + bb.max[0]) / 2, (bb.min[1] + bb.max[1]) / 2, (bb.min[2] + bb.max[2]) / 2);
  const hl = [(bb.max[0] - bb.min[0]) / 2, (bb.max[1] - bb.min[1]) / 2, (bb.max[2] - bb.min[2]) / 2];
  const ax = [new THREE.Vector3(), new THREE.Vector3(), new THREE.Vector3()]; W.extractBasis(ax[0], ax[1], ax[2]);
  const h = ax.map((a, i) => Math.max(1e-4, hl[i] * a.length()));
  ax.forEach(a => a.lengthSq() > 0 ? a.normalize() : a.set(1, 0, 0));
  const R = new THREE.Matrix4().makeBasis(ax[0], ax[1], ax[2]); const q = new THREE.Quaternion().setFromRotationMatrix(R);
  return { c: lc.applyMatrix4(W), ax, h, q, W: W.clone() };
}
function bodyInfo(o) {
  const a = ea(o); const box = obbOf(o); const m = Math.max(1e-4, num(a.mass, 1));
  const sphere = a.standIn === 'sphere'; const r = Math.max(...box.h);
  let I;
  if (sphere) { const k = 0.4 * m * r * r; I = [k, k, k]; }
  else { const [x, y, z] = box.h.map(v => Math.max(v, 0.01)); I = [m / 3 * (y * y + z * z), m / 3 * (x * x + z * z), m / 3 * (x * x + y * y)]; }
  return { o, id: o.inca.id, active: a.rigidBody === 'active', sphere, r, h: box.h, ax0: box.ax, q0: box.q, c0: box.c.clone(), W0: box.W, invM: a.rigidBody === 'active' ? 1 / m : 0, invI: I.map(x => 1 / Math.max(1e-9, x)), a };
}
// world-space inverse inertia applied to vector
function invIw(B, st, v) {
  if (!B.invM) return new THREE.Vector3();
  const q = st.qw; const l = v.clone().applyQuaternion(q.clone().invert());
  l.set(l.x * B.invI[0], l.y * B.invI[1], l.z * B.invI[2]);
  return l.applyQuaternion(q);
}
// shape at the current state: center, axes, half extents
function shapeOf(B, st) {
  const ax = [new THREE.Vector3(1, 0, 0), new THREE.Vector3(0, 1, 0), new THREE.Vector3(0, 0, 1)].map(a => a.applyQuaternion(st.qw));
  return { c: st.x, ax, h: B.h, r: B.r, sphere: B.sphere };
}
function cornersOf(sh) {
  const out = []; for (const sx of [-1, 1]) for (const sy of [-1, 1]) for (const sz of [-1, 1]) out.push(sh.c.clone().addScaledVector(sh.ax[0], sx * sh.h[0]).addScaledVector(sh.ax[1], sy * sh.h[1]).addScaledVector(sh.ax[2], sz * sh.h[2]));
  return out;
}
const toLocal = (sh, p) => { const d = p.clone().sub(sh.c); return [d.dot(sh.ax[0]), d.dot(sh.ax[1]), d.dot(sh.ax[2])]; };
// contacts of shape A against shape B; normal points from B towards A
function collide(A, B, out, ia, ib) {
  if (A.sphere && B.sphere) { const d = A.c.clone().sub(B.c); const L = d.length(); const pen = A.r + B.r - L; if (pen > 0) { const n = L > 1e-9 ? d.multiplyScalar(1 / L) : new THREE.Vector3(0, 1, 0); out.push({ a: ia, b: ib, p: B.c.clone().addScaledVector(n, B.r - pen / 2), n, depth: pen }); } return; }
  if (A.sphere || B.sphere) {
    const sp = A.sphere ? A : B, bx = A.sphere ? B : A; const q = toLocal(bx, sp.c);
    const cl = q.map((v, i) => Math.max(-bx.h[i], Math.min(bx.h[i], v)));
    const inside = q.every((v, i) => Math.abs(v) <= bx.h[i]);
    let n, pen, p;
    if (!inside) { p = bx.c.clone().addScaledVector(bx.ax[0], cl[0]).addScaledVector(bx.ax[1], cl[1]).addScaledVector(bx.ax[2], cl[2]); const d = sp.c.clone().sub(p); const L = d.length(); pen = sp.r - L; if (pen <= 0) return; n = d.multiplyScalar(1 / Math.max(L, 1e-9)); }
    else { let k = 0, best = -Infinity; for (let i = 0; i < 3; i++) { const s = Math.abs(q[i]) - bx.h[i]; if (s > best) { best = s; k = i; } } const s = q[k] >= 0 ? 1 : -1; n = bx.ax[k].clone().multiplyScalar(s); pen = sp.r + bx.h[k] - Math.abs(q[k]); p = sp.c.clone().addScaledVector(n, -sp.r); }
    if (sp === B) n.negate(); // normal from B to A
    out.push({ a: ia, b: ib, p, n, depth: pen }); return;
  }
  // box vs box: corners of each box inside the other, face chosen from the region of the other's center
  const test = (X, Y, flip) => { // corners of X inside Y; normal from Y to X
    const cq = toLocal(Y, X.c); let k = 0, best = -Infinity; for (let i = 0; i < 3; i++) { const s = Math.abs(cq[i]) - Y.h[i]; if (s > best) { best = s; k = i; } }
    const s = cq[k] >= 0 ? 1 : -1; const n = Y.ax[k].clone().multiplyScalar(s);
    const margin = Math.max(...X.h) * 1.5 + Y.h[k];
    for (const p of cornersOf(X)) {
      const q = toLocal(Y, p); const depth = Y.h[k] - s * q[k];
      if (depth <= 0 || depth > Y.h[k] + margin) continue;
      let ok = true; for (let j = 0; j < 3; j++) if (j !== k && Math.abs(q[j]) > Y.h[j] + 1e-4) ok = false;
      if (!ok) continue;
      out.push(flip ? { a: ib, b: ia, p, n, depth } : { a: ia, b: ib, p, n, depth });
    }
  };
  test(A, B, false); test(B, A, true);
  // flipped contacts have normal from A to B: convert to B->A convention by swapping a/b (done above)
}
function stepBodies(R, fields, dt, time) {
  const bodies = R.bodies; const states = R.state.bodies;
  // forces
  const acc = [0, 0, 0];
  for (const B of bodies) {
    if (!B.invM) continue; const st = states.get(B.id);
    acc[0] = acc[1] = acc[2] = 0; fieldAccel(fields, B.id, st.x.x, st.x.y, st.x.z, st.v.x, st.v.y, st.v.z, time, acc);
    st.v.x += acc[0] * dt; st.v.y += acc[1] * dt; st.v.z += acc[2] * dt;
    const damp = Math.max(0, 1 - num(B.a.damping) * dt); st.v.multiplyScalar(damp); st.w.multiplyScalar(damp * (1 - 0.02 * dt));
  }
  // contacts
  const shapes = bodies.map(B => shapeOf(B, states.get(B.id)));
  const contacts = [];
  for (let i = 0; i < bodies.length; i++) for (let j = i + 1; j < bodies.length; j++) {
    const A = bodies[i], Bb = bodies[j]; if (!A.invM && !Bb.invM) continue; if (A.a.collisions === false || Bb.a.collisions === false) continue;
    // broad phase: bounding spheres
    const ra = Math.hypot(...A.h), rb = Math.hypot(...Bb.h); if (shapes[i].c.distanceTo(shapes[j].c) > ra + rb + 1e-3) continue;
    if (A.invM) collide(shapes[i], shapes[j], contacts, i, j); else collide(shapes[j], shapes[i], contacts, j, i);
  }
  const g = 9.8;
  for (const c of contacts) {
    const A = bodies[c.a], B = bodies[c.b]; const sa = states.get(A.id), sb = states.get(B.id);
    c.ra = c.p.clone().sub(sa.x); c.rb = c.p.clone().sub(sb.x);
    const vrel = velAt(A, sa, c.ra).sub(velAt(B, sb, c.rb)); const vn = vrel.dot(c.n);
    const e = num(A.a.bounciness, 0.6) * num(B.a.bounciness, 0.6);
    c.bias = vn < -Math.sqrt(2 * g * 0.05) ? -e * vn : 0;
    c.mu = (num(A.a.dynamicFriction, 0.2) + num(B.a.dynamicFriction, 0.2)) / 2;
    const k = (n) => A.invM + B.invM + invIw(A, sa, c.ra.clone().cross(n)).cross(c.ra).dot(n) + invIw(B, sb, c.rb.clone().cross(n)).cross(c.rb).dot(n);
    c.kn = k(c.n); c.jn = 0; c.jt = new THREE.Vector3(); c.k = k;
  }
  for (let it = 0; it < 10; it++) for (const c of contacts) {
    const A = bodies[c.a], B = bodies[c.b]; const sa = states.get(A.id), sb = states.get(B.id);
    const vrel = velAt(A, sa, c.ra).sub(velAt(B, sb, c.rb));
    const vn = vrel.dot(c.n);
    let dj = (c.bias - vn) / c.kn; const j0 = c.jn; c.jn = Math.max(0, j0 + dj); dj = c.jn - j0;
    applyImpulse(A, sa, B, sb, c, c.n.clone().multiplyScalar(dj));
    // friction
    const vr2 = velAt(A, sa, c.ra).sub(velAt(B, sb, c.rb)); const vt = vr2.clone().addScaledVector(c.n, -vr2.dot(c.n)); const vtl = vt.length();
    if (vtl > 1e-7) { const t = vt.multiplyScalar(1 / vtl); const kt = c.k(t); let jt = -vtl / kt; const maxF = c.mu * c.jn; const cur = c.jt.dot(t); const nj = Math.max(-maxF, Math.min(maxF, cur + jt)); jt = nj - cur; c.jt.addScaledVector(t, jt); applyImpulse(A, sa, B, sb, c, t.multiplyScalar(jt)); }
  }
  // integrate
  for (const B of bodies) {
    if (!B.invM) continue; const st = states.get(B.id);
    st.x.addScaledVector(st.v, dt);
    const w = st.w; const dq = new THREE.Quaternion(w.x * dt * 0.5, w.y * dt * 0.5, w.z * dt * 0.5, 0).multiply(st.q);
    st.q.set(st.q.x + dq.x, st.q.y + dq.y, st.q.z + dq.z, st.q.w + dq.w).normalize();
    st.qw.copy(st.q).multiply(B.q0);
  }
  // positional correction (per body / collider pair use the deepest contact)
  const corr = new Map();
  for (const c of contacts) { const key = c.a + ':' + c.b; const prev = corr.get(key); if (!prev || c.depth > prev.depth) corr.set(key, c); }
  for (const c of corr.values()) {
    const A = bodies[c.a], B = bodies[c.b]; const tot = A.invM + B.invM; if (!tot) continue;
    const d = Math.max(0, c.depth - 0.002) * 0.6; if (!d) continue;
    if (A.invM) states.get(A.id).x.addScaledVector(c.n, d * A.invM / tot);
    if (B.invM) states.get(B.id).x.addScaledVector(c.n, -d * B.invM / tot);
  }
}
function velAt(B, st, r) { return st.v.clone().add(st.w.clone().cross(r)); }
function applyImpulse(A, sa, B, sb, c, J) {
  if (A.invM) { sa.v.addScaledVector(J, A.invM); sa.w.add(invIw(A, sa, c.ra.clone().cross(J))); }
  if (B.invM) { sb.v.addScaledVector(J, -B.invM); sb.w.sub(invIw(B, sb, c.rb.clone().cross(J))); }
}
function bodyWorldMatrix(B, st) {
  // W = T(x) * R(q) * T(-c0) * W0
  const m = new THREE.Matrix4().makeTranslation(st.x.x, st.x.y, st.x.z).multiply(new THREE.Matrix4().makeRotationFromQuaternion(st.q)).multiply(new THREE.Matrix4().makeTranslation(-B.c0.x, -B.c0.y, -B.c0.z)).multiply(B.W0);
  return m;
}

// ------------------------------------------------------------------ simulation runtime
const rt = { cache: new Map(), atRest: true, R: null, sig: '', lastParts: null, lastApplied: null };
function dynNodes() { const out = { emitters: [], parts: [], fields: [], rigid: [] }; for (const o of S.allDag()) { const k = o.inca.kind; if (k === 'emitter') out.emitters.push(o); else if (k === 'particles') out.parts.push(o); else if (k === 'field') out.fields.push(o); else if (isRigid(o)) out.rigid.push(o); } return out; }
function cloneState(s) {
  const parts = new Map(); for (const [k, P] of s.parts) parts.set(k, cloneParts(P));
  const bodies = new Map(); for (const [k, b] of s.bodies) bodies.set(k, { x: b.x.clone(), q: b.q.clone(), qw: b.qw.clone(), v: b.v.clone(), w: b.w.clone() });
  return { frame: s.frame, parts, bodies, emitAcc: new Map(s.emitAcc) };
}
function reset() {
  const N = dynNodes();
  for (const o of N.rigid) { if (rt.atRest || !restOf(o)) captureRest(o); else restoreRest(o); }
  rt.atRest = true;
  const bodies = N.rigid.map(bodyInfo);
  const state = { frame: startFrame(), parts: new Map(), bodies: new Map(), emitAcc: new Map() };
  for (const o of N.parts) state.parts.set(o.inca.id, initialParticles(o));
  for (const B of bodies) {
    const a = B.a; const v = B.invM ? new THREE.Vector3(num(a.initialVelocityX), num(a.initialVelocityY), num(a.initialVelocityZ)) : new THREE.Vector3();
    const w = B.invM ? new THREE.Vector3(num(a.initialSpinX), num(a.initialSpinY), num(a.initialSpinZ)).multiplyScalar(D2R) : new THREE.Vector3();
    state.bodies.set(B.id, { x: B.c0.clone(), q: new THREE.Quaternion(), qw: B.q0.clone(), v, w });
  }
  rt.R = { bodies, N };
  rt.cache.clear(); rt.cache.set(state.frame, cloneState(state));
  return state;
}
const startFrame = () => Math.round(App.time.start);
function simulateFrame(prev, frame) {
  const s = cloneState(prev); s.frame = frame;
  const fps = App.time.fps || 24; const subs = Math.max(1, Math.round(App.dynamics.substeps || 4));
  const dt = 1 / fps / subs;
  const fields = gatherFields();
  const N = rt.R.N;
  // passive rigid bodies: follow their (possibly animated) transform
  for (const B of rt.R.bodies) if (!B.invM) { const box = obbOf(B.o); const st = s.bodies.get(B.id); st.x.copy(box.c); st.qw.copy(box.q); B.h = box.h; }
  const colliders = rt.R.bodies.filter(B => !B.invM && B.a.collisions !== false).map(B => { const st = s.bodies.get(B.id); const sh = shapeOf(B, st); return { c: sh.c, ax: sh.ax, h: B.h, bounce: num(B.a.bounciness, 0.6), friction: num(B.a.dynamicFriction, 0.2) * 0.5 }; });
  const R = { emitAcc: s.emitAcc, surf: new Map(), sub: 0 };
  for (let k = 0; k < subs; k++) {
    const time = (frame - 1 + (k + 1) / subs) / fps; R.sub = k;
    for (const o of N.parts) {
      if (!App.nodes.has(o.inca.id)) continue; const P = s.parts.get(o.inca.id); if (!P || ea(o).enabled === false) continue;
      stepParticles(o, P, fields, colliders, dt, time);
    }
    for (const e of N.emitters) { const tgt = App.nodes.get(dyn(e).target); if (!tgt || !e.visible) continue; const P = s.parts.get(tgt.inca.id); if (P) emit(e, P, tgt, frame, dt, R); }
    if (rt.R.bodies.some(B => B.invM)) stepBodies({ bodies: rt.R.bodies, state: s }, fields, dt, time);
  }
  return s;
}
function applyState(s) {
  rt.lastApplied = s.frame;
  rt.lastParts = s.parts;
  for (const o of rt.R.N.parts) if (App.nodes.get(o.inca.id) === o) writePoints(o, s.parts.get(o.inca.id));
  for (const B of rt.R.bodies) {
    if (!B.invM || App.nodes.get(B.id) !== B.o) continue;
    const st = s.bodies.get(B.id);
    if (s.frame <= startFrame()) restoreRest(B.o); else S.setWorldMatrix(B.o, bodyWorldMatrix(B, st));
  }
  rt.atRest = s.frame <= startFrame();
}
function invalidate() { rt.cache.clear(); rt.R = null; }
function hasSim() { for (const o of S.allDag()) { const k = o.inca.kind; if (k === 'particles' || (isRigid(o) && o.inca.extraAttrs.rigidBody === 'active')) return true; } return false; }
function signature() { const parts = []; for (const o of S.allDag()) { const k = o.inca.kind; if (KINDS.has(k) || isRigid(o)) parts.push(o.inca.id + JSON.stringify(o.inca.extraAttrs || {}) + JSON.stringify(o.inca.extra?.dyn?.affects || '') + (o.inca.extra?.dyn?.target || '') + o.visible); } return parts.join('|'); }

function step(t) {
  if (!hasSim()) { if (rt.R) invalidate(); return; }
  const sig = signature(); if (sig !== rt.sig) { rt.sig = sig; invalidate(); for (const o of S.allDag()) if (KINDS.has(o.inca.kind)) buildVisuals(o); }
  const f = Math.floor(t + 1e-6); const start = startFrame();
  if (f <= start || !rt.R || !rt.cache.has(start)) { const s0 = reset(); if (f <= start) { applyState(s0); return; } }
  if (rt.cache.has(f)) { applyState(rt.cache.get(f)); return; }
  let k = start; for (const key of rt.cache.keys()) if (key < f && key > k) k = key;
  const target = Math.min(f, k + SIM_LIMIT);
  const animated = App.anim && App.anim.curves && App.anim.curves.size > 0;
  let s = rt.cache.get(k);
  for (let fr = k + 1; fr <= target; fr++) {
    if (animated && fr !== f) App.anim.evaluate(fr);
    s = simulateFrame(s, fr);
    rt.cache.set(fr, cloneState(s));
    if (rt.cache.size > CACHE_MAX) { const first = [...rt.cache.keys()].find(x => x !== start); rt.cache.delete(first); }
  }
  if (animated && target !== t) App.anim.evaluate(t);
  applyState(s);
}

App.dynamics = {
  step, invalidate, substeps: 4, kinds: KINDS,
  createEmitterNode, createParticleNode, createFieldNode,
  get cache() { return rt.cache; },
  particleCount(o) { const P = rt.lastParts?.get(o.inca.id); return P ? P.n : 0; },
  aeFrames: null, // set below
  reset() { invalidate(); App.setTime(App.time.current); },
};
// any undoable edit invalidates the cache (transform edits of emitters/fields/colliders are not in the signature)
const _ck = Undo.checkpoint;
Undo.checkpoint = function (label) { const r = _ck.call(this, label); if (rt.R) invalidate(); return r; };
App.on('sceneLoaded', () => {
  invalidate(); rt.sig = ''; rt.lastParts = null;
  rt.atRest = App.time.current <= startFrame();
  for (const o of S.allDag()) if (KINDS.has(o.inca.kind)) buildVisuals(o);
  if (hasSim()) step(App.time.current);
});
App.on('nodeDeleted', (o) => { if (KINDS.has(o.inca.kind) || isRigid(o)) invalidate(); });
App.on('refresh', (d) => {
  if (!(d.has('attr') || d.has('channels'))) return;
  const sig = signature(); if (sig === rt.sig) return;
  for (const o of S.allDag()) if (KINDS.has(o.inca.kind)) buildVisuals(o);
  if (hasSim()) step(App.time.current); else rt.sig = sig;
});

// ------------------------------------------------------------------ commands
const ck = (l) => Undo.checkpoint(l);
const echo = (s) => App.emit('echo', s);
const exitComp = () => { if (App.compMode) Sel.setMode(null); };
function goStartIfNeeded() { /* Maya creates dynamics at the current frame; sim starts at the playback start */ }
C('createEmitter', 'Create Emitter', (a = {}) => {
  ck('create emitter'); exitComp();
  const { emitter, particles } = createEmitterNode(a.type || 'omni');
  Sel.select([emitter], 'replace', { echo: false });
  echo(`emitter -pos 0 0 0 -type ${ea(emitter).emitterType} -r 100 -sro 0 -nuv 0 -cye none -cyi 1 -spd 1 -srn 0 -nsp 1 -tsp 0 -mxd 0 -mnd 0 -dx 1 -dy 0 -dz 0 -sp 0 ;`);
  App.emit('result', `${emitter.inca.name} ${particles.inca.name}`);
  goStartIfNeeded(); App.dynamics.reset(); return emitter;
}, { icon: 'pointLight', help: 'Create an emitter and a particle object (play the timeline to see particles)', options: [{ key: 'type', label: 'Emitter type', type: 'enum', options: ['omni', 'directional', 'volume'], values: ['omni', 'directional', 'volume'], value: 'omni' }] });
C('createParticles', 'Create Particles', (a = {}) => {
  ck('create particles'); exitComp();
  const nx = Math.max(1, Math.round(a.count || 10)), sp = a.spacing || 0.5, y = a.height ?? 5;
  const init = []; for (let i = 0; i < nx; i++) for (let j = 0; j < nx; j++) init.push((i - (nx - 1) / 2) * sp, y, (j - (nx - 1) / 2) * sp);
  const o = createParticleNode({ initial: init });
  Sel.select([o], 'replace', { echo: false });
  echo(`particle -ll ${-sp * nx / 2} ${y} ${-sp * nx / 2} -ur ${sp * nx / 2} ${y} ${sp * nx / 2} -grs ${sp} -c 1;`); App.emit('result', o.inca.name);
  App.dynamics.reset(); return o;
}, { icon: 'polySoccer', help: 'Create a particle grid (add a field such as Gravity to move it)' });
C('emitFromObject', 'Emit from Object', (a = {}) => {
  const src = App.sel.filter(o => o.inca.kind === 'mesh');
  if (!src.length) { App.emit('warning', '// Warning: Select a polygon object to emit from.'); return; }
  ck('emit from object'); exitComp();
  const made = [];
  for (const m of src) {
    const { emitter, particles } = createEmitterNode(a.type || 'surface', { parent: m });
    dyn(emitter).source = m.inca.id; made.push(emitter);
    echo(`emitter -type ${ea(emitter).emitterType} -r 100 -spd 1 ${m.inca.name};`); App.emit('result', `${emitter.inca.name} ${particles.inca.name}`);
  }
  Sel.select(made, 'replace', { echo: false }); App.dynamics.reset(); return made;
}, { help: 'Emit particles from the selected mesh (surface or vertices)', options: [{ key: 'type', label: 'Emitter type', type: 'enum', options: ['surface', 'vertex'], values: ['surface', 'vertex'], value: 'surface' }] });
const FIELD_LABEL = { gravity: 'Gravity', turbulence: 'Turbulence', radial: 'Radial', drag: 'Drag', air: 'Air', vortex: 'Vortex' };
for (const [type, label] of Object.entries(FIELD_LABEL)) {
  C(type + 'Field', label, () => {
    ck('create ' + type + ' field'); exitComp();
    // Maya: with dynamic objects selected the field is connected to them; here fields affect everything by default
    const o = createFieldNode(type);
    Sel.select([o], 'replace', { echo: false });
    echo(`${type} -pos 0 0 0 -m ${ea(o).magnitude};`); App.emit('result', o.inca.name);
    App.dynamics.reset(); return o;
  }, { icon: { gravity: 'move', turbulence: 'polyHelix', radial: 'pointLight', drag: 'stop', air: 'directionalLight', vortex: 'polyHelix' }[type], help: `Create a ${label.toLowerCase()} field (affects all particles and rigid bodies)` });
}
function makeRigid(kind) {
  const ms = App.sel.filter(o => o.inca.kind === 'mesh');
  if (!ms.length) { App.emit('warning', '// Warning: Select one or more polygon objects.'); return; }
  ck('create ' + kind + ' rigid body');
  for (const o of ms) {
    const a = ea(o);
    if (!isRigid(o)) for (const k of RIGID_KEYS) if (a[k] === undefined) a[k] = RIGID_DEFAULTS[k];
    a.rigidBody = kind;
    if (kind === 'passive') { a.bounciness ??= 0.6; }
    captureRest(o);
    echo(`rigidBody -${kind} -m ${a.mass} -dp ${a.damping} -sf ${a.staticFriction} -df ${a.dynamicFriction} -b ${a.bounciness} -l 0 -tf 200 -iv 0 0 0 -iav 0 0 0 -c 0 -pc 0 -i 0 0 0 -imp 0 0 0 -si 0 0 0 -sio none ${o.inca.name};`);
  }
  App.emit('result', ms.map(o => 'rigidBody' + o.inca.id.replace(/\D/g, '')).join(' '));
  if (kind === 'active' && !S.allDag().some(o => o.inca.kind === 'field' && dyn(o).fieldType === 'gravity')) App.emit('warning', '// Warning: Rigid bodies need a field to move: add Fields > Gravity.');
  App.dynamics.reset(); App.dirty('attr', 'channels');
}
C('createActiveRigidBody', 'Create Active Rigid Body', () => makeRigid('active'), { icon: 'polyCube', help: 'Make the selected meshes active (simulated) rigid bodies' });
C('createPassiveRigidBody', 'Create Passive Rigid Body', () => makeRigid('passive'), { icon: 'polyPlane', help: 'Make the selected meshes passive rigid bodies (static colliders)' });
C('removeRigidBody', 'Remove Rigid Body', () => {
  const ms = App.sel.filter(isRigid); if (!ms.length) return; ck('remove rigid body');
  for (const o of ms) { restoreRest(o); for (const k of RIGID_KEYS) delete o.inca.extraAttrs[k]; if (o.inca.extra?.dyn) delete o.inca.extra.dyn.rest; }
  App.dynamics.reset(); App.dirty('attr', 'channels');
});
C('dynamicsReset', 'Rewind Simulation', () => { App.dynamics.reset(); App.setTime(App.time.start); }, { noRepeat: true });

function fxPreset(kind) {
  ck('create ' + kind); exitComp();
  const src = App.sel.find(o => o.inca.kind === 'mesh');
  const fire = kind === 'fire';
  const pAttrs = fire
    ? { renderType: 'sprites', radius: 0.35, radiusEnd: 0.08, color: [1, 0.85, 0.35], colorEnd: [0.85, 0.12, 0.02], opacity: 0.9, opacityEnd: 0, additive: true, maxCount: 20000 }
    : { renderType: 'sprites', radius: 0.25, radiusEnd: 1.1, color: [0.55, 0.55, 0.55], colorEnd: [0.25, 0.25, 0.27], opacity: 0.45, opacityEnd: 0, additive: false, maxCount: 20000 };
  const particles = createParticleNode({ name: fire ? 'fireParticle1' : 'smokeParticle1', attrs: pAttrs });
  const eAttrs = fire ? { rate: 300, speed: 0.6, speedRandom: 0.5, spread: 0.3, lifespan: 1.1, lifespanRandom: 0.4, volumeShape: 'sphere' } : { rate: 80, speed: 0.4, speedRandom: 0.5, spread: 0.3, lifespan: 4, lifespanRandom: 0.3, volumeShape: 'sphere' };
  let emitter;
  if (src) { ({ emitter } = createEmitterNode('surface', { name: fire ? 'fireEmitter1' : 'smokeEmitter1', parent: src, attrs: eAttrs, particles })); dyn(emitter).source = src.inca.id; }
  else { ({ emitter } = createEmitterNode('volume', { name: fire ? 'fireEmitter1' : 'smokeEmitter1', attrs: eAttrs, particles })); emitter.inca.s = fire ? [0.5, 0.15, 0.5] : [0.4, 0.15, 0.4]; S.updateXform(emitter); buildVisuals(emitter); }
  const aff = [particles.inca.id];
  const air = createFieldNode('air', { name: fire ? 'fireAir1' : 'smokeAir1', attrs: fire ? { magnitude: 6, speed: 3.5 } : { magnitude: 2, speed: 1.6 }, affects: aff });
  const turb = createFieldNode('turbulence', { name: fire ? 'fireTurbulence1' : 'smokeTurbulence1', attrs: fire ? { magnitude: 3, frequency: 1.6 } : { magnitude: 1.5, frequency: 0.6 }, affects: aff });
  const drag = createFieldNode('drag', { name: fire ? 'fireDrag1' : 'smokeDrag1', attrs: { magnitude: fire ? 1.2 : 0.6 }, affects: aff });
  for (const f of [air, turb, drag]) { if (src) { const b = S.worldBBox(src); const c = b.getCenter(new THREE.Vector3()); f.inca.t = [c.x, b.min.y, c.z]; S.updateXform(f); } }
  Sel.select([emitter], 'replace', { echo: false });
  echo(`create${fire ? 'Fire' : 'Smoke'}${src ? ' ' + src.inca.name : ''};`); App.emit('result', `${emitter.inca.name} ${particles.inca.name} ${air.inca.name} ${turb.inca.name} ${drag.inca.name}`);
  App.dynamics.reset(); return emitter;
}
C('createFire', 'Create Fire', () => fxPreset('fire'), { icon: 'pointLight', help: 'Create a fire effect (emits from the selected mesh, or from a small volume at the origin)' });
C('createSmoke', 'Create Smoke', () => fxPreset('smoke'), { icon: 'pointLight', help: 'Create a smoke effect (emits from the selected mesh, or from a small volume at the origin)' });

// ------------------------------------------------------------------ Attribute Editor frames (called from attrEditor.js shape tab)
App.dynamics.aeFrames = (rec, F, frame) => {
  if (!rec || !rec.isObject3D) return [];
  const inc = rec.inca; const a = inc.extraAttrs || {};
  const changed = () => { invalidate(); rt.sig = ''; buildVisuals(rec); App.modified = true; App.dirty('channels', 'title'); App.requestRender(); };
  const A = (key, type, o = {}) => F({ key: 'dyn_' + key, label: o.label, type, min: o.min, max: o.max, smin: o.smin, smax: o.smax, options: o.options, values: o.values,
    get: () => type === 'vec3' ? [num(a[key + 'X']), num(a[key + 'Y']), num(a[key + 'Z'])] : a[key],
    set: (v) => { if (type === 'vec3') { a[key + 'X'] = v[0]; a[key + 'Y'] = v[1]; a[key + 'Z'] = v[2]; } else a[key] = Array.isArray(v) ? v.slice() : v; changed(); } });
  const out = [];
  if (inc.kind === 'emitter') {
    out.push(frame('Basic Emitter Attributes', [
      A('emitterType', 'enum', { label: 'Emitter Type', options: ['Omni', 'Directional', 'Volume', 'Surface', 'Vertex'], values: ['omni', 'directional', 'volume', 'surface', 'vertex'] }),
      A('rate', 'float', { label: 'Rate (Particles/Sec)', min: 0, smin: 0, smax: 1000 }), A('enabled', 'bool', { label: 'Enabled' }),
    ], { key: 'dynEmit' }));
    out.push(frame('Emission Speed / Direction', [A('speed', 'float', { label: 'Speed', smin: 0, smax: 20 }), A('speedRandom', 'float', { label: 'Speed Random', min: 0, smin: 0, smax: 1 }), A('direction', 'vec3', { label: 'Direction' }), A('spread', 'float', { label: 'Spread', min: 0, max: 1 })], { key: 'dynEmitDir' }));
    out.push(frame('Lifespan', [A('lifespan', 'float', { label: 'Lifespan (sec, -1 = forever)', min: -1, smin: 0, smax: 20 }), A('lifespanRandom', 'float', { label: 'Lifespan Random', min: 0, smin: 0, smax: 1 })], { key: 'dynEmitLife' }));
    out.push(frame('Volume Emitter Attributes', [A('volumeShape', 'enum', { label: 'Volume Shape', options: ['Cube', 'Sphere'], values: ['cube', 'sphere'] })], { key: 'dynEmitVol', closed: a.emitterType !== 'volume' }));
    const tgt = App.nodes.get(dyn(rec).target);
    out.push(frame('Connections', [F({ key: 'dyn_target', label: 'Particle Object', type: 'info', get: () => tgt ? tgt.inca.name : '(none)' }), F({ key: 'dyn_src', label: 'Emit From', type: 'info', get: () => App.nodes.get(dyn(rec).source)?.inca.name || '-' })], { key: 'dynEmitCon' }));
  } else if (inc.kind === 'particles') {
    out.push(frame('Count', [F({ key: 'dyn_count', label: 'Count', type: 'info', get: () => App.dynamics.particleCount(rec) }), A('maxCount', 'int', { label: 'Max Count', min: 0, smin: 0, smax: 50000 }), A('enabled', 'bool', { label: 'Is Dynamic' }), A('conserve', 'float', { label: 'Conserve', min: 0, max: 1 }), A('collide', 'bool', { label: 'Collide with passive rigid bodies' })], { key: 'dynPCount' }));
    out.push(frame('Shading', [
      A('renderType', 'enum', { label: 'Particle Render Type', options: ['Points', 'Sprites (soft)'], values: ['points', 'sprites'] }),
      A('pointSize', 'float', { label: 'Point Size (px)', min: 1, smin: 1, smax: 20 }),
      A('radius', 'float', { label: 'Radius (birth)', min: 0, smin: 0, smax: 2 }), A('radiusEnd', 'float', { label: 'Radius (death)', min: 0, smin: 0, smax: 2 }),
      A('color', 'color', { label: 'Color (birth)' }), A('colorEnd', 'color', { label: 'Color (death)' }),
      A('opacity', 'float', { label: 'Opacity (birth)', min: 0, max: 1 }), A('opacityEnd', 'float', { label: 'Opacity (death)', min: 0, max: 1 }),
      A('additive', 'bool', { label: 'Additive blending' }),
    ], { key: 'dynPShade' }));
  } else if (inc.kind === 'field') {
    const type = dyn(rec).fieldType; const fs = [A('magnitude', 'float', { label: 'Magnitude', smin: -20, smax: 20 }), A('attenuation', 'float', { label: 'Attenuation', min: 0, smin: 0, smax: 5 })];
    if (type === 'gravity' || type === 'air') fs.push(A('direction', 'vec3', { label: 'Direction' }));
    if (type === 'air') fs.push(A('speed', 'float', { label: 'Speed', smin: 0, smax: 20 }));
    if (type === 'vortex') fs.push(A('axis', 'vec3', { label: 'Axis' }));
    if (type === 'turbulence') fs.push(A('frequency', 'float', { label: 'Frequency', min: 0, smin: 0, smax: 5 }), A('phaseX', 'float', { label: 'Phase X', smin: -10, smax: 10 }), A('phaseY', 'float', { label: 'Phase Y', smin: -10, smax: 10 }), A('phaseZ', 'float', { label: 'Phase Z', smin: -10, smax: 10 }));
    out.push(frame(FIELD_LABEL[type] + ' Field Attributes', fs, { key: 'dynField' }));
    out.push(frame('Distance', [A('useMaxDistance', 'bool', { label: 'Use Max Distance' }), A('maxDistance', 'float', { label: 'Max Distance', min: 0, smin: 0, smax: 100 })], { key: 'dynFieldDist' }));
    const aff = dyn(rec).affects;
    out.push(frame('Affects', [F({ key: 'dyn_aff', label: 'Affects', type: 'info', get: () => aff ? aff.map(id => App.nodes.get(id)?.inca.name).filter(Boolean).join(', ') || '(nothing)' : 'All dynamic objects' })], { key: 'dynFieldAff', closed: true }));
  } else if (isRigid(rec)) {
    out.push(frame('Rigid Body Attributes', [
      A('rigidBody', 'enum', { label: 'Type', options: ['Active', 'Passive'], values: ['active', 'passive'] }),
      A('mass', 'float', { label: 'Mass', min: 0.0001, smin: 0.01, smax: 10 }), A('bounciness', 'float', { label: 'Bounciness', min: 0, smin: 0, smax: 1 }),
      A('staticFriction', 'float', { label: 'Static Friction', min: 0, smin: 0, smax: 1 }), A('dynamicFriction', 'float', { label: 'Dynamic Friction', min: 0, smin: 0, smax: 1 }),
      A('damping', 'float', { label: 'Damping', min: 0, smin: 0, smax: 2 }), A('collisions', 'bool', { label: 'Collisions' }),
      A('standIn', 'enum', { label: 'Stand In', options: ['Cube', 'Sphere'], values: ['cube', 'sphere'] }),
    ], { key: 'dynRigid' }));
    out.push(frame('Initial Settings', [A('initialVelocity', 'vec3', { label: 'Initial Velocity' }), A('initialSpin', 'vec3', { label: 'Initial Spin (deg/s)' })], { key: 'dynRigidInit' }));
  }
  return out;
};
export { step as dynamicsStep };
