// Inca — Render View, Render Settings, batch rendering, sky dome environments and Playblast.
// Two renderers: "Inca Path Tracer" (three-gpu-pathtracer WebGLPathTracer, progressive) and
// "Inca Hardware" (rasterized three.js with shadows, tone mapping and supersampled anti-aliasing).
import * as THREE from 'three';
import { HDRLoader } from 'three/addons/loaders/HDRLoader.js';
import { EXRLoader } from 'three/addons/loaders/EXRLoader.js';
import { App } from '../core/app.js';
import { h, iconBtn, menuBar, FloatWin, numField, toast, fmt, colorPicker, colorToCss } from './dom.js';
import { syncCamera, allDag, findNode } from '../core/scene.js';
import { icon } from './icons.js';
import { fileUrl } from '../core/materials.js';

const N = () => (typeof window !== 'undefined' ? window.incaNative : null) || null;
const echo = (s) => App.emit('echo', s);
const warn = (s) => { App.emit('warning', '// Warning: ' + s); toast(s, 3000); };
const pad4 = (n) => String(Math.round(n)).padStart(4, '0');
const joinPath = (...p) => p.filter(Boolean).map((s, i) => i === 0 ? String(s).replace(/[\\/]+$/, '') : String(s).replace(/^[\\/]+|[\\/]+$/g, '')).join('/');
const fmtTime = (ms) => { const s = Math.floor(ms / 1000); return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`; };
const sleep = (ms) => new Promise(r => setTimeout(r, ms));

export const RENDERERS = [['path', 'Inca Path Tracer'], ['raster', 'Inca Hardware']];
export const SIZE_PRESETS = [['HD 540', 960, 540], ['HD 720', 1280, 720], ['HD 1080', 1920, 1080], ['640x480', 640, 480], ['1024x1024', 1024, 1024], ['2K Square', 2048, 2048], ['4K UHD', 3840, 2160]];
const TM = { aces: THREE.ACESFilmicToneMapping, agx: THREE.AgXToneMapping, neutral: THREE.NeutralToneMapping, linear: THREE.LinearToneMapping, none: THREE.NoToneMapping };
const TM_OPTS = [['aces', 'ACES Filmic'], ['agx', 'AgX'], ['neutral', 'Khronos Neutral'], ['linear', 'Linear'], ['none', 'None']];
const AA = { off: 1, low: 1, medium: 2, high: 3, ultra: 4 };

// ------------------------------------------------------------------ render settings
export function ensureRenderSettings() {
  const d = {
    width: 960, height: 540, camera: 'persp', renderer: 'path', samples: 128, bounces: 5, toneMapping: 'aces', exposure: 0,
    background: [0, 0, 0], transparentBg: false, filename: 'image', format: 'png', start: 1, end: 10, by: 1, animation: false,
    pixelAspect: 1, keepRatio: true, transmissiveBounces: 5, filterGlossy: 0.5, caustics: false,
    hwAA: 'high', hwShadowMap: 2048, hwAllShadows: true, hwToneMapping: 'aces', testRes: 100, autoResize: true, jpgQuality: 0.92,
  };
  App.renderSettings ||= {};
  for (const [k, v] of Object.entries(d)) if (App.renderSettings[k] === undefined) App.renderSettings[k] = Array.isArray(v) ? v.slice() : v;
  return App.renderSettings;
}
ensureRenderSettings();
App.on('sceneLoaded', () => { ensureRenderSettings(); RS_WIN?.rebuild(); });

// ------------------------------------------------------------------ path tracer loading (lazy, with fallback)
let PT = null, ptError = null, ptLoading = null;
function loadPathTracer() {
  if (PT || ptError) return Promise.resolve(PT);
  if (!ptLoading) ptLoading = import('three-gpu-pathtracer').then(m => { if (!m.WebGLPathTracer) throw new Error('WebGLPathTracer export missing'); PT = m; return m; })
    .catch(e => { ptError = e; console.warn('Inca Path Tracer unavailable:', e); return null; });
  return ptLoading;
}

// ------------------------------------------------------------------ sky dome environments
// App.skyEnv(lightNode) -> { env, bg, equirect }. env/bg are equirect DataTextures (linear, half float,
// EquirectangularReflectionMapping): every WebGLRenderer PMREM-filters an equirect scene.environment itself,
// which keeps this valid across the viewports' separate GL contexts; the path tracer needs the raw equirect data.
const skyCache = new Map();
function dataEquirect(w, hh, fn) {
  const data = new Uint16Array(w * hh * 4); const toH = THREE.DataUtils.toHalfFloat;
  for (let y = 0; y < hh; y++) {
    const v = (y + 0.5) / hh; // 0 = bottom (nadir), 1 = top (zenith)
    for (let x = 0; x < w; x++) {
      const c = fn((x + 0.5) / w, v); const k = (y * w + x) * 4;
      data[k] = toH(c[0]); data[k + 1] = toH(c[1]); data[k + 2] = toH(c[2]); data[k + 3] = toH(1);
    }
  }
  return finishEquirect(new THREE.DataTexture(data, w, hh, THREE.RGBAFormat, THREE.HalfFloatType));
}
function finishEquirect(t) {
  t.mapping = THREE.EquirectangularReflectionMapping; t.colorSpace = THREE.LinearSRGBColorSpace;
  t.minFilter = t.magFilter = THREE.LinearFilter; t.generateMipmaps = false;
  t.wrapS = THREE.RepeatWrapping; t.wrapT = THREE.ClampToEdgeWrapping; t.needsUpdate = true;
  return t;
}
function proceduralSky(color) {
  const zen = [0.30, 0.48, 0.85], hor = [0.92, 0.95, 1.0], gnd = [0.32, 0.29, 0.26], gh = [0.55, 0.52, 0.48];
  const lerp = (a, b, t) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
  return dataEquirect(256, 128, (u, v) => {
    const s = Math.sin((v - 0.5) * Math.PI);
    const c = s >= 0 ? lerp(hor, zen, Math.pow(s, 0.55)) : lerp(gh, gnd, Math.pow(-s, 0.35));
    return [c[0] * color[0], c[1] * color[1], c[2] * color[2]];
  });
}
function uniformEnv(c) { return dataEquirect(32, 16, () => c); }
function srgbToLinear(x) { return x <= 0.04045 ? x / 12.92 : Math.pow((x + 0.055) / 1.055, 2.4); }
function absPathOf(p) {
  if (/^file:\/\//.test(p)) { let s = decodeURI(p.replace(/^file:\/\//, '')); if (/^\/[a-zA-Z]:/.test(s)) s = s.slice(1); return s; }
  if (/^([a-zA-Z]:)?[\\/]/.test(p)) return p;
  return App.project ? joinPath(App.project, p) : p;
}
async function readBytes(p) {
  const n = N();
  if (n && n.readBinary && !/^(data:|blob:|https?:)/.test(p)) return new Uint8Array(await n.readBinary(absPathOf(p))).buffer;
  const r = await fetch(fileUrl(p)); if (!r.ok) throw new Error('HTTP ' + r.status); return r.arrayBuffer();
}
async function loadSkyTexture(file, color) {
  const buf = await readBytes(file);
  const ext = (file.split('.').pop() || '').toLowerCase();
  let tex;
  if (ext === 'hdr' || ext === 'exr') {
    const loader = ext === 'hdr' ? new HDRLoader() : new EXRLoader();
    loader.setDataType(THREE.HalfFloatType);
    const d = loader.parse(buf);
    if (!d || !d.data) throw new Error('could not decode ' + file);
    tex = new THREE.DataTexture(d.data, d.width, d.height, d.format ?? THREE.RGBAFormat, d.type ?? THREE.HalfFloatType);
    tex.flipY = !!d.flipY;
    if (color.some(c => Math.abs(c - 1) > 1e-4) && tex.format === THREE.RGBAFormat && tex.type === THREE.HalfFloatType) {
      const D = tex.image.data, fh = THREE.DataUtils.fromHalfFloat, th = THREE.DataUtils.toHalfFloat;
      for (let i = 0; i < D.length; i += 4) for (let c = 0; c < 3; c++) D[i + c] = th(fh(D[i + c]) * color[c]);
    }
  } else {
    const bmp = await createImageBitmap(new Blob([buf]));
    const sc = Math.min(1, 2048 / bmp.width);
    const w = Math.max(2, Math.round(bmp.width * sc)), hh = Math.max(1, Math.round(bmp.height * sc));
    const cv = document.createElement('canvas'); cv.width = w; cv.height = hh;
    const x = cv.getContext('2d'); x.drawImage(bmp, 0, 0, w, hh); bmp.close?.();
    const px = x.getImageData(0, 0, w, hh).data;
    const lut = new Float32Array(256); for (let i = 0; i < 256; i++) lut[i] = srgbToLinear(i / 255);
    const data = new Uint16Array(w * hh * 4); const th = THREE.DataUtils.toHalfFloat;
    for (let y = 0; y < hh; y++) for (let i = 0; i < w; i++) {
      const s = ((hh - 1 - y) * w + i) * 4, d = (y * w + i) * 4; // flip rows: row 0 = bottom
      data[d] = th(lut[px[s]] * color[0]); data[d + 1] = th(lut[px[s + 1]] * color[1]); data[d + 2] = th(lut[px[s + 2]] * color[2]); data[d + 3] = th(1);
    }
    tex = new THREE.DataTexture(data, w, hh, THREE.RGBAFormat, THREE.HalfFloatType);
  }
  return finishEquirect(tex);
}
export function skyEnv(node) {
  const a = node?.inca?.light; if (!a) return null;
  const color = (a.color || [1, 1, 1]).map(c => Math.max(0, c));
  const file = (a.textureFile || '').trim();
  const key = (file ? 'f:' + fileUrl(file) : 'p') + '|' + color.map(c => c.toFixed(4)).join(',');
  let e = skyCache.get(key);
  if (!e) {
    e = { tex: null, fallback: null, error: null };
    skyCache.set(key, e);
    if (file) {
      e.fallback = proceduralSky(color);
      loadSkyTexture(file, color).then(t => { e.tex = t; e.fallback.dispose(); e.fallback = null; App.requestRender(); App.emit('skyChanged', node); })
        .catch(err => { e.error = err; console.warn('sky dome texture failed', file, err); App.emit('warning', `// Warning: Sky dome texture could not be loaded: ${file}`); App.requestRender(); });
    } else e.tex = proceduralSky(color);
    // keep the cache small
    while (skyCache.size > 8) { const [k0, e0] = skyCache.entries().next().value; skyCache.delete(k0); e0.tex?.dispose(); e0.fallback?.dispose(); }
  }
  const t = e.tex || e.fallback;
  return t ? { env: t, bg: t, equirect: t, loading: !e.tex } : null;
}
App.skyEnv = skyEnv;
const uniformCache = new Map();
function ambientEnv(c) {
  const key = c.map(x => x.toFixed(3)).join(',');
  let t = uniformCache.get(key);
  if (!t) { t = uniformEnv(c); uniformCache.set(key, t); if (uniformCache.size > 4) { const [k0, t0] = uniformCache.entries().next().value; uniformCache.delete(k0); t0.dispose(); } }
  return t;
}

// ------------------------------------------------------------------ scene preparation for final renders
export function renderCameraNode(name) {
  const n = findNode(name);
  if (n && n.isObject3D && n.inca.kind === 'camera') return n;
  return findNode('persp');
}
export function cameraList() { return allDag(true).filter(o => o.inca.kind === 'camera'); }
const _p = new THREE.Vector3(), _q = new THREE.Quaternion(), _s = new THREE.Vector3();
// an independent copy of the node's camera so the viewports re-syncing their aspect can't disturb a render
export function makeRenderCamera(camNode, aspect, reuse = null) {
  syncCamera(camNode, aspect); camNode.updateWorldMatrix(true, true);
  const src = camNode.userData.camera;
  let cam = reuse && reuse.isOrthographicCamera === !!src.isOrthographicCamera ? reuse : (src.isOrthographicCamera ? new THREE.OrthographicCamera() : new THREE.PerspectiveCamera());
  if (cam.isPerspectiveCamera) { cam.fov = src.fov; cam.aspect = src.aspect; cam.zoom = src.zoom; cam.filmGauge = src.filmGauge; cam.filmOffset = src.filmOffset; cam.view = null; }
  else { cam.left = src.left; cam.right = src.right; cam.top = src.top; cam.bottom = src.bottom; cam.zoom = src.zoom; cam.view = null; }
  cam.near = src.near; cam.far = src.far;
  cam.matrixAutoUpdate = false;
  cam.matrix.copy(src.matrixWorld); cam.matrix.decompose(_p, _q, _s); cam.position.copy(_p); cam.quaternion.copy(_q); cam.scale.copy(_s);
  cam.matrixWorld.copy(src.matrixWorld); cam.matrixWorldInverse.copy(src.matrixWorld).invert();
  cam.updateProjectionMatrix();
  App.requestRender(); // viewports re-sync the shared camera to their own aspect
  return cam;
}
// Hide helpers, use real light intensities, set environment. Returns a restore function.
export function prepareRenderScene({ pt = false, cam = null } = {}) {
  const rs = ensureRenderSettings();
  const saved = []; const set = (o, k, v) => { saved.push([o, k, o[k]]); o[k] = v; };
  const shadowSaved = []; const added = [];
  const sc = App.scene;
  for (const c of sc.children) if (c !== App.world) set(c, 'visible', false);
  const lights = [];
  App.world.traverse((o) => {
    if (o.inca) { if (o.inca.template && o.visible) set(o, 'visible', false); return; }
    const ud = o.userData; if (!ud.helper) return;
    const owner = ud.owner;
    if (o.isLight) {
      const a = owner?.inca?.light; if (!a) { set(o, 'visible', false); return; }
      set(o, 'visible', true);
      if (a.type === 'skyDomeLight') { set(o, 'intensity', 0); lights.push({ owner, a, sky: true }); return; }
      if (a.type === 'ambientLight') { set(o, 'intensity', pt ? 0 : (a.intensity || 0) * Math.PI); lights.push({ owner, a, amb: true }); return; }
      set(o, 'intensity', (a.intensity || 0) * Math.PI);
      if (o.shadow && 'castShadow' in o) {
        const cs = !pt && !!(rs.hwAllShadows || a.shadows);
        set(o, 'castShadow', cs);
        const sz = +rs.hwShadowMap || 2048;
        if (cs && o.shadow.mapSize.x !== sz) { shadowSaved.push([o, o.shadow.mapSize.x, o.shadow.mapSize.y]); o.shadow.mapSize.set(sz, sz); if (o.shadow.map) { o.shadow.map.dispose(); o.shadow.map = null; } }
      }
      lights.push({ owner, a });
      return;
    }
    if (o.isMesh && owner?.inca && !ud.isPick && owner.userData.mesh === o) {
      set(o, 'visible', true);
      if (ud.matsT) set(o, 'material', ud.matsT);
      return;
    }
    if (o.visible) set(o, 'visible', false);
  });
  const shown = (o) => { for (let p = o; p; p = p.parent) if (!p.visible) return false; return true; };
  let direct = 0, sky = null; const amb = [0, 0, 0];
  for (const L of lights) {
    if (!shown(L.owner)) continue;
    if (L.sky) { if (!sky) sky = L.owner; }
    else if (L.amb) { const I = L.a.intensity || 0; for (let i = 0; i < 3; i++) amb[i] += (L.a.color?.[i] ?? 1) * I; }
    else direct++;
  }
  const hasAmb = amb[0] + amb[1] + amb[2] > 0;
  // environment + background
  set(sc, 'environment', null); set(sc, 'background', null);
  set(sc, 'environmentIntensity', 1); set(sc, 'backgroundIntensity', 1);
  if (sky && App.skyEnv) {
    const s = App.skyEnv(sky);
    if (s) {
      sc.environment = pt ? (s.equirect || s.env) : s.env; sc.environmentIntensity = sky.inca.light.intensity ?? 1;
      if (sky.inca.light.showBackground !== false && !rs.transparentBg) { sc.background = s.bg; sc.backgroundIntensity = sky.inca.light.intensity ?? 1; }
    }
  } else if (pt && hasAmb) { sc.environment = ambientEnv(amb); sc.environmentIntensity = 1; }
  if (!sc.background && !rs.transparentBg) { const b = rs.background || [0, 0, 0]; sc.background = new THREE.Color(b[0], b[1], b[2]); }
  // Maya's default light: a directional light riding with the camera when the scene has no lights
  if (!direct && !sky && !hasAmb && cam) {
    const dl = new THREE.DirectionalLight(0xffffff, Math.PI); dl.name = 'incaRenderDefaultLight';
    cam.getWorldPosition(_p); cam.getWorldQuaternion(_q);
    dl.position.copy(_p).add(new THREE.Vector3(-0.15, 0.2, 0).applyQuaternion(_q));
    dl.target.position.copy(_p).add(new THREE.Vector3(0, 0, -1).applyQuaternion(_q));
    sc.add(dl, dl.target); added.push(dl, dl.target);
  }
  sc.updateMatrixWorld(true);
  return () => {
    for (let i = saved.length - 1; i >= 0; i--) { const [o, k, v] = saved[i]; o[k] = v; }
    for (const [o, x, y] of shadowSaved) { o.shadow.mapSize.set(x, y); if (o.shadow.map) { o.shadow.map.dispose(); o.shadow.map = null; } }
    for (const a of added) { sc.remove(a); a.dispose?.(); }
    App.requestRender();
  };
}

// ------------------------------------------------------------------ render engine (own WebGL context)
let engine = null;
function getEngine() { if (!engine) engine = new Engine(); return engine; }
class Engine {
  constructor() {
    this.glCanvas = document.createElement('canvas');
    this.renderer = new THREE.WebGLRenderer({ canvas: this.glCanvas, antialias: true, alpha: true, preserveDrawingBuffer: true, powerPreference: 'high-performance' });
    const r = this.renderer;
    r.setPixelRatio(1); r.outputColorSpace = THREE.SRGBColorSpace;
    r.shadowMap.enabled = true; r.shadowMap.type = THREE.PCFShadowMap;
    this.out = document.createElement('canvas'); this.out.width = 960; this.out.height = 540;
    this.octx = this.out.getContext('2d');
    this.pt = null; this.cam = null; this.job = null; this.raf = 0;
    this.onUpdate = null;
    this.offs = [
      App.on('refresh', (f) => { if (this.job?.ipr && (f.has('channels') || f.has('attr') || f.has('outliner') || f.has('hypershade') || f.has('layers'))) this.job.check = true; }),
      App.on('materialChanged', () => { if (this.job?.ipr) this.job.needsScene = true; }),
      App.on('timeChanged', () => { if (this.job?.ipr) this.job.check = true; }),
      App.on('skyChanged', () => { if (this.job?.ipr) this.job.needsScene = true; }),
      App.on('sceneLoaded', () => { if (this.job?.ipr) { this.job.camNode = renderCameraNode(ensureRenderSettings().camera); this.job.needsScene = true; } }),
    ];
  }
  // cheap requestRender hook while IPR runs
  hook() {
    if (this._wrap) return;
    const orig = App.requestRender; const self = this;
    this._orig = orig;
    this._wrap = function () { orig.apply(this, arguments); if (self.job && self.job.ipr) self.job.check = true; };
    App.requestRender = this._wrap;
  }
  unhook() { if (this._wrap && App.requestRender === this._wrap) App.requestRender = this._orig; this._wrap = null; }
  dispose() {
    this.stop(); this.unhook();
    for (const off of this.offs) off();
    try { this.pt?.dispose(); } catch (e) { console.warn(e); }
    this.renderer.dispose(); this.renderer.forceContextLoss?.();
    this.pt = null;
  }
  get size() { const rs = ensureRenderSettings(); const k = (rs.testRes || 100) / 100; return { W: Math.max(1, Math.round(rs.width * k)), H: Math.max(1, Math.round(rs.height * k)) }; }
  stop(cancelled = true) {
    cancelAnimationFrame(this.raf); this.raf = 0;
    const j = this.job;
    if (j && !j.finished) { j.finished = true; j.cancelled = cancelled; j.elapsed = performance.now() - j.t0; j.resolve?.(j); }
    if (j) j.ipr = false;
    this.unhook();
    this.onUpdate?.(j);
  }
  // returns a promise resolving with the job when the final image is done
  async start({ ipr = false } = {}) {
    this.stop();
    const rs = ensureRenderSettings();
    const { W, H } = this.size;
    const camNode = renderCameraNode(rs.camera);
    if (!camNode) { warn('No camera to render from.'); return null; }
    const job = { ipr, W, H, camNode, mode: rs.renderer === 'path' ? 'path' : 'raster', t0: performance.now(), frame: App.time.current, samples: 0, target: Math.max(1, rs.samples | 0), perFrame: 2, finished: false, done: false, check: false, needsScene: false, status: '' };
    const p = new Promise(res => { job.resolve = res; });
    this.job = job;
    if (job.mode === 'path') {
      job.status = 'Loading path tracer…'; this.onUpdate?.(job);
      await loadPathTracer();
      if (this.job !== job) return p;
      const ok = PT && this.renderer.extensions.has('EXT_color_buffer_float');
      if (!ok) { job.mode = 'raster'; job.fallback = true; warn(`Inca Path Tracer is unavailable (${ptError ? ptError.message : 'float render targets not supported'}); rendering with Inca Hardware.`); }
    }
    echo(`render -camera ${camNode.inca.name}${ipr ? ' -ipr' : ''};`);
    if (job.mode === 'raster') this.renderRaster(job);
    else this.setupPath(job);
    if (ipr) { this.hook(); job.sceneSig = this.sceneSig(); job.camSig = this.camSig(job.camNode); }
    if (job.mode === 'raster' && !ipr) { this.finish(job); return p; }
    this._last = performance.now();
    const tick = () => { this.raf = requestAnimationFrame(tick); this.tick(); };
    this.raf = requestAnimationFrame(tick);
    return p;
  }
  finish(job) {
    job.done = true;
    job.elapsed = performance.now() - job.t0;
    if (!job.ipr && !job.finished) { job.finished = true; cancelAnimationFrame(this.raf); this.raf = 0; job.resolve?.(job); }
    this.onUpdate?.(job);
  }
  sceneSig() {
    let s = JSON.stringify(App.renderSettings) + '|';
    App.world.traverse((o) => {
      if (!o.inca) return;
      const inc = o.inca; const ud = o.userData;
      s += inc.id + (o.visible ? 'v' : 'h') + (inc.template ? 't' : '') + (o.parent?.inca?.id || '') + o.matrix.elements.map(x => x.toFixed(5)).join(',');
      if (inc.kind === 'mesh' && ud.mesh) s += ud.mesh.geometry.uuid + (ud.mesh.userData.matIds || []).join(',');
      if (inc.kind === 'light') s += JSON.stringify(inc.light);
      s += ';';
    });
    return s;
  }
  camSig(camNode) { camNode.updateWorldMatrix(true, false); return camNode.matrixWorld.elements.map(x => x.toFixed(5)).join(',') + JSON.stringify(camNode.inca.cam); }
  tick() {
    const job = this.job; if (!job || job.finished) return;
    const now = performance.now(); const dt = now - this._last; this._last = now;
    if (job.ipr) {
      if (job.check) {
        job.check = false;
        const sig = this.sceneSig();
        if (sig !== job.sceneSig) { job.sceneSig = sig; job.needsScene = true; }
      }
      const cs = this.camSig(job.camNode);
      if (cs !== job.camSig) { job.camSig = cs; job.needsCamera = true; }
      const sz = this.size;
      if (sz.W !== job.W || sz.H !== job.H) { job.W = sz.W; job.H = sz.H; job.needsScene = true; }
    }
    if (job.mode === 'raster') {
      if (job.needsScene || job.needsCamera) { job.needsScene = job.needsCamera = false; job.t0 = performance.now(); this.renderRaster(job); this.finish(job); }
      return;
    }
    if (job.needsScene) { job.needsScene = job.needsCamera = false; this.setupPath(job); }
    else if (job.needsCamera) {
      job.needsCamera = false;
      this.cam = makeRenderCamera(job.camNode, job.W * (App.renderSettings.pixelAspect || 1) / job.H, this.cam);
      this.pt.setCamera(this.cam); job.t0 = performance.now(); job.done = false;
    }
    if (job.done) return;
    const pt = this.pt;
    if (pt.isCompiling) { job.status = 'Compiling shaders…'; this.onUpdate?.(job); return; }
    // adapt the number of tiles per frame to keep the UI responsive
    if (dt < 28 && job.perFrame < 32) job.perFrame++; else if (dt > 45 && job.perFrame > 1) job.perFrame--;
    for (let i = 0; i < job.perFrame; i++) { pt.renderSample(); if (pt.samples >= job.target) break; }
    job.samples = Math.floor(pt.samples);
    job.status = '';
    this.copyOut(job.W, job.H);
    if (job.samples === 0 && performance.now() - job.t0 > 8000 && !job.warnedStall) { job.warnedStall = true; warn('The path tracer is not producing samples on this GPU; try Inca Hardware.'); }
    if (pt.samples >= job.target) this.finish(job);
    else this.onUpdate?.(job);
  }
  applyToneMapping(path) {
    const rs = ensureRenderSettings(); const r = this.renderer;
    r.toneMapping = TM[path ? rs.toneMapping : (rs.hwToneMapping || rs.toneMapping)] ?? THREE.ACESFilmicToneMapping;
    r.toneMappingExposure = Math.pow(2, rs.exposure || 0);
  }
  setupPath(job) {
    const rs = ensureRenderSettings(); const r = this.renderer;
    r.setRenderTarget(null);
    r.setSize(job.W, job.H, false);
    this.applyToneMapping(true);
    if (!this.pt) {
      this.pt = new PT.WebGLPathTracer(r);
      Object.assign(this.pt, { renderDelay: 0, minSamples: 1, fadeDuration: 0, rasterizeScene: false, dynamicLowRes: false, synchronizeRenderSize: true, renderScale: 1, renderToCanvas: true });
    }
    const pt = this.pt;
    const t = job.W * job.H > 2.2e6 ? 3 : 2; pt.tiles.set(t, t);
    pt.bounces = Math.max(1, rs.bounces | 0);
    pt.transmissiveBounces = Math.max(1, (rs.transmissiveBounces ?? rs.bounces) | 0);
    pt.filterGlossyFactor = rs.caustics ? 0 : (+rs.filterGlossy || 0);
    pt.multipleImportanceSampling = true;
    this.cam = makeRenderCamera(job.camNode, job.W * (rs.pixelAspect || 1) / job.H, this.cam);
    const restore = prepareRenderScene({ pt: true, cam: this.cam });
    try { pt.setScene(App.scene, this.cam); }
    catch (e) { console.error(e); job.mode = 'raster'; job.fallback = true; warn('Path tracer failed (' + (e.message || e) + '); using Inca Hardware.'); }
    finally { restore(); }
    if (job.mode === 'raster') { this.renderRaster(job); this.finish(job); return; }
    pt.reset();
    job.t0 = performance.now(); job.done = false; job.samples = 0; job.target = Math.max(1, rs.samples | 0);
    if (job.ipr) { job.sceneSig = this.sceneSig(); job.camSig = this.camSig(job.camNode); }
  }
  renderRaster(job) {
    const rs = ensureRenderSettings(); const r = this.renderer;
    const maxDim = Math.min(8192, r.capabilities.maxTextureSize || 8192);
    const ss = Math.max(1, Math.min(AA[rs.hwAA] ?? 2, Math.floor(maxDim / Math.max(job.W, job.H))));
    r.setRenderTarget(null);
    r.setSize(job.W * ss, job.H * ss, false);
    this.applyToneMapping(false);
    r.shadowMap.enabled = true; r.shadowMap.needsUpdate = true;
    this.cam = makeRenderCamera(job.camNode, job.W * (rs.pixelAspect || 1) / job.H, this.cam);
    const restore = prepareRenderScene({ pt: false, cam: this.cam });
    try {
      const b = rs.background || [0, 0, 0];
      r.setClearColor(new THREE.Color(b[0], b[1], b[2]), rs.transparentBg ? 0 : 1);
      r.clear();
      r.render(App.scene, this.cam);
    } catch (e) { console.error(e); warn('Render failed: ' + (e.message || e)); }
    finally { restore(); }
    job.samples = ss; job.ss = ss;
    this.copyOut(job.W, job.H);
  }
  copyOut(W, H) {
    if (this.out.width !== W || this.out.height !== H) { this.out.width = W; this.out.height = H; }
    const c = this.octx;
    c.clearRect(0, 0, W, H);
    c.imageSmoothingEnabled = true; c.imageSmoothingQuality = 'high';
    const sw = this.glCanvas.width, sh = this.glCanvas.height;
    if (sw >= W * 3) {
      // two-step downsample for heavy supersampling
      const mid = document.createElement('canvas'); mid.width = W * 2; mid.height = H * 2;
      const m = mid.getContext('2d'); m.imageSmoothingQuality = 'high'; m.drawImage(this.glCanvas, 0, 0, sw, sh, 0, 0, W * 2, H * 2);
      c.drawImage(mid, 0, 0, W * 2, H * 2, 0, 0, W, H);
    } else c.drawImage(this.glCanvas, 0, 0, sw, sh, 0, 0, W, H);
  }
}

// ------------------------------------------------------------------ image saving helpers
function canvasBlob(canvas, type = 'image/png', quality = 0.92) {
  let src = canvas;
  if (type === 'image/jpeg') { src = document.createElement('canvas'); src.width = canvas.width; src.height = canvas.height; const x = src.getContext('2d'); x.fillStyle = '#000'; x.fillRect(0, 0, src.width, src.height); x.drawImage(canvas, 0, 0); }
  return new Promise((res, rej) => src.toBlob(b => b ? res(b) : rej(new Error('encode failed')), type, quality));
}
export async function writeCanvas(canvas, path, type) {
  const blob = await canvasBlob(canvas, type, ensureRenderSettings().jpgQuality || 0.92);
  const n = N();
  if (n && n.writeBinary) { await n.writeBinary(path, new Uint8Array(await blob.arrayBuffer())); return path; }
  downloadBlob(blob, path.split(/[\\/]/).pop());
  return null;
}
function downloadBlob(blob, name) {
  const a = document.createElement('a'); a.href = URL.createObjectURL(blob); a.download = name; document.body.append(a); a.click();
  setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 1000);
}
async function defaultDir(sub) {
  if (App.project) return joinPath(App.project, sub);
  const n = N();
  if (n && n.paths) { try { const p = await n.paths(); return joinPath(p.documents || p.home, 'inca', sub); } catch { /* ignore */ } }
  return sub;
}
const extOf = (fmt0) => fmt0 === 'jpg' || fmt0 === 'jpeg' ? 'jpg' : 'png';
const mimeOf = (ext) => ext === 'jpg' ? 'image/jpeg' : 'image/png';

// ------------------------------------------------------------------ Render View window
let RV = null;
let cssDone = false;
function injectCss() {
  if (cssDone || document.getElementById('rv-extra-css')) { cssDone = true; return; }
  cssDone = true;
  document.head.append(h('style', { id: 'rv-extra-css', text: `
.rv { display: flex; flex-direction: column; flex: 1; min-height: 0; font-size: 11.5px; }
.rv .ptoolbar { border-bottom: 1px solid var(--line); gap: 2px; height: 28px; }
.rv .ptoolbar .ib { width: 24px; height: 24px; }
.rv .ptoolbar .tsep { width: 1px; height: 18px; background: #5a5a5a; margin: 0 4px; flex: none; }
.rv .ptoolbar select { height: 20px; font-size: 11px; }
.rv .ptoolbar input { width: 42px; height: 18px; font-size: 11px; }
.rv .ptoolbar .lbl { color: #aaa; font-size: 11px; margin: 0 2px 0 4px; }
.rv .tbtn { height: 20px; min-width: 26px; padding: 0 6px; display: inline-flex; align-items: center; justify-content: center; border-radius: 3px; background: #3a3a3a; color: #ddd; font-size: 11px; font-weight: 600; cursor: default; }
.rv .tbtn:hover { background: #555; } .rv .tbtn.on { background: #5d7a8f; box-shadow: inset 0 0 0 1px #81a9c4; color: #fff; }
.rv .ib.busy { background: #6a5a2a; }
.rv-body.rv-scroll { display: flex; align-items: flex-start; justify-content: flex-start; background: #2a2a2a; }
.rv-stage { margin: auto; flex: none; position: relative; background-color: #000; }
.rv-stage.checker { background-image: linear-gradient(45deg,#333 25%,transparent 25%),linear-gradient(-45deg,#333 25%,transparent 25%),linear-gradient(45deg,transparent 75%,#333 75%),linear-gradient(-45deg,transparent 75%,#333 75%); background-size: 16px 16px; background-position: 0 0,0 8px,8px -8px,-8px 0; background-color: #222; }
.rv-stage canvas { display: block; width: 100%; height: 100%; }
.rv-empty { color: #888; position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; text-align: center; pointer-events: none; }
.rv-kept { height: 58px; flex: none; display: flex; gap: 4px; padding: 3px 6px; overflow-x: auto; overflow-y: hidden; border-top: 1px solid var(--line); background: #333; }
.rv-kept .kt { flex: none; height: 50px; border: 1px solid #222; position: relative; cursor: default; }
.rv-kept .kt.on { border-color: #9cd0f5; box-shadow: 0 0 0 1px #9cd0f5; }
.rv-kept .kt img { height: 100%; display: block; }
.rv-kept .kt span { position: absolute; left: 2px; bottom: 1px; font-size: 9.5px; color: #fff; text-shadow: 0 0 2px #000; }
.rv-status .progress { max-width: 160px; }
.rs-win { display: flex; flex-direction: column; flex: 1; min-height: 0; font-size: 11.5px; }
.rs-top { display: flex; align-items: center; gap: 8px; padding: 6px 10px; border-bottom: 1px solid var(--line); flex: none; }
.rs-top label { color: #ccc; }
.rs-tabs { display: flex; gap: 1px; padding: 4px 8px 0; flex: none; border-bottom: 1px solid #5a5a5a; }
.rs-tab { padding: 3px 10px; background: #383838; border-radius: 3px 3px 0 0; color: #b5b5b5; cursor: default; }
.rs-tab.on { background: #5a5a5a; color: #fff; }
.rs-body { flex: 1; overflow: auto; padding: 2px 0 8px; min-height: 0; }
.rs-row { display: flex; align-items: center; gap: 6px; min-height: 22px; padding: 1px 8px; }
.rs-row > label:first-child { width: 150px; text-align: right; color: #c8c8c8; flex: none; }
.rs-row input:not([type]) { width: 70px; }
.rs-row input.wide { flex: 1; width: auto; }
.rs-row .csw { width: 48px; height: 16px; border: 1px solid #222; }
.rs-row .note { color: #8f8f8f; font-size: 11px; }
.pb-prev video { width: 100%; flex: 1; min-height: 0; background: #000; }
` }));
  if (!document.getElementById('rv-svg-filters')) {
    const d = document.createElement('div'); d.id = 'rv-svg-filters'; d.style.cssText = 'position:absolute;width:0;height:0;overflow:hidden';
    d.innerHTML = `<svg width="0" height="0"><filter id="rv-adjust" color-interpolation-filters="sRGB"><feComponentTransfer><feFuncR type="gamma" amplitude="1" exponent="1" offset="0"/><feFuncG type="gamma" amplitude="1" exponent="1" offset="0"/><feFuncB type="gamma" amplitude="1" exponent="1" offset="0"/></feComponentTransfer></filter><filter id="rv-alpha" color-interpolation-filters="sRGB"><feColorMatrix type="matrix" values="0 0 0 1 0  0 0 0 1 0  0 0 0 1 0  0 0 0 0 1"/></filter></svg>`;
    document.body.append(d);
  }
}

export function openRenderView(renderNow = false, { ipr = false } = {}) {
  injectCss();
  ensureRenderSettings();
  const win = new FloatWin('renderView', 'Render View', { w: 980, h: 700, minW: 480, minH: 300 });
  if (!(win.reused && RV)) {
    RV = new RenderView(win);
    win.onClose = () => { if (RV) RV.destroy(); RV = null; };
  }
  if (renderNow) RV.render({ ipr });
  return RV;
}

class RenderView {
  constructor(win) {
    this.win = win;
    this.engine = getEngine();
    this.engine.onUpdate = (job) => this.onEngine(job);
    this.kept = []; this.viewing = -1; // -1 = live image
    this.zoom = 1; this.alpha = false; this.dispExposure = 0; this.dispGamma = 1;
    this.lastOpts = { ipr: false };
    this.offs = [App.on('refresh', (f) => { if (f.has('outliner')) this.fillCameras(); })];
    this.build();
    win.onResize = () => { if (ensureRenderSettings().autoResize) this.fit(); };
  }
  destroy() {
    for (const off of this.offs) off();
    if (this.batch) this.batch.cancel = true;
    if (engine) { engine.dispose(); engine = null; }
    this.kept = [];
  }
  build() {
    const rs = ensureRenderSettings();
    this.root = h('div', { class: 'rv' });
    const mb = h('div', { class: 'pmenubar' });
    menuBar(mb, this.menus());
    this.renderBtn = iconBtn('renderFrame', 'Render the current frame', () => this.render({ ipr: false }));
    this.iprBtn = iconBtn('ipr', 'IPR render the current frame (updates as the scene changes)', () => this.toggleIpr());
    this.camSel = h('select', { title: 'Render camera' });
    this.camSel.addEventListener('change', () => { ensureRenderSettings().camera = this.camSel.value; this.settingsChanged('camera'); RS_WIN?.rebuild(); });
    this.rendSel = h('select', { title: 'Renderer' }, RENDERERS.map(([v, l]) => h('option', { value: v, text: l })));
    this.rendSel.value = rs.renderer;
    this.rendSel.addEventListener('change', () => { ensureRenderSettings().renderer = this.rendSel.value; this.settingsChanged('renderer'); RS_WIN?.rebuild(); });
    this.rgbBtn = h('span', { class: 'tbtn on', text: 'RGB', title: 'Display RGB channels', onclick: () => this.setAlpha(false) });
    this.aBtn = h('span', { class: 'tbtn', text: 'A', title: 'Display alpha channel', onclick: () => this.setAlpha(true) });
    this.expField = numField(0, (v) => { this.dispExposure = v; this.applyDisplay(); }, { step: 0.05 });
    this.gamField = numField(1, (v) => { this.dispGamma = Math.max(0.05, v); this.applyDisplay(); }, { step: 0.02 });
    this.expField.title = 'Display exposure (stops)'; this.gamField.title = 'Display gamma';
    const tb = h('div', { class: 'ptoolbar' },
      this.renderBtn,
      iconBtn('redo', 'Redo previous render', () => this.render(this.lastOpts)),
      this.iprBtn,
      iconBtn('stop', 'Stop rendering (Esc)', () => this.stop()),
      h('div', { class: 'tsep' }),
      iconBtn('saveScene', 'Save image…', () => this.saveImage()),
      iconBtn('plus', 'Keep image (snapshot)', () => this.keepImage()),
      iconBtn('trash', 'Remove the displayed kept image', () => this.removeImage()),
      h('div', { class: 'tsep' }),
      iconBtn('renderSettings', 'Render Settings', () => openRenderSettings()),
      this.rendSel,
      h('span', { class: 'lbl', text: 'Camera' }), this.camSel,
      h('div', { class: 'tsep' }),
      this.rgbBtn, this.aBtn,
      h('div', { class: 'tsep' }),
      h('span', { class: 'lbl', title: 'Display exposure' }, iconEl('exposure')), this.expField,
      h('span', { class: 'lbl', title: 'Display gamma' }, iconEl('gamma')), this.gamField,
      h('div', { class: 'tsep' }),
      iconBtn('frameAll', 'Frame image (fit to window)', () => this.fit()),
      h('span', { class: 'tbtn', text: '1:1', title: 'Real size', onclick: () => this.setZoom(1) }));
    this.fillCameras();
    this.stage = h('div', { class: 'rv-stage' });
    this.empty = h('div', { class: 'rv-empty', html: 'Click the Render button (or Render &gt; Render Current Frame)<br>to render the scene.' });
    this.body = h('div', { class: 'rv-body rv-scroll', tabindex: 0 }, this.stage, this.empty);
    this.keptEl = h('div', { class: 'rv-kept', style: { display: 'none' } });
    this.statusText = h('span', { text: '' });
    this.progress = h('div', { class: 'progress', style: { display: 'none' } }, h('div'));
    this.status = h('div', { class: 'rv-status' }, this.statusText, h('span', { style: { flex: 1 } }), this.progress);
    this.root.append(mb, tb, this.body, this.keptEl, this.status);
    this.win.body.append(this.root);
    this.showLive();
    this.bindView();
    this.win.el.addEventListener('keydown', (e) => {
      const tag = e.target.tagName;
      if (tag === 'INPUT' || tag === 'SELECT' || tag === 'TEXTAREA') { e.stopPropagation(); return; }
      let handled = true;
      if (e.key === 'Escape') this.stop();
      else if (e.key === 'f' || e.key === 'F') this.fit();
      else if (e.key === '+' || e.key === '=') this.setZoom(this.zoom * 1.25);
      else if (e.key === '-') this.setZoom(this.zoom / 1.25);
      else if (e.key === 'a' || e.key === 'A') this.setAlpha(!this.alpha);
      else handled = false;
      if (handled) { e.preventDefault(); e.stopPropagation(); }
    });
    this.updateStatus(null);
  }
  menus() {
    const rs = () => ensureRenderSettings();
    return [
      { label: 'File', items: [
        { label: 'Save Image...', fn: () => this.saveImage() },
        '-',
        { label: 'Keep Image', fn: () => this.keepImage() },
        { label: 'Remove Image', enabled: () => this.viewing >= 0, fn: () => this.removeImage() },
        { label: 'Remove All Kept Images', enabled: () => this.kept.length > 0, fn: () => { this.kept = []; this.showLive(); this.drawKept(); } },
        '-',
        { label: 'Close', fn: () => this.win.close() },
      ] },
      { label: 'View', items: [
        { label: 'Frame Image', hk: 'F', fn: () => this.fit() },
        { label: 'Real Size', fn: () => this.setZoom(1) },
        { label: 'Zoom In', hk: '+', fn: () => this.setZoom(this.zoom * 1.25) },
        { label: 'Zoom Out', hk: '-', fn: () => this.setZoom(this.zoom / 1.25) },
        '-',
        { label: 'Display RGB Channels', check: () => !this.alpha, fn: () => this.setAlpha(false) },
        { label: 'Display Alpha Channel', check: () => this.alpha, fn: () => this.setAlpha(true) },
        { label: 'Reset Exposure / Gamma', fn: () => { this.dispExposure = 0; this.dispGamma = 1; this.expField.set(0); this.gamField.set(1); this.applyDisplay(); } },
        '-',
        { label: 'Show Live Image', enabled: () => this.viewing >= 0, fn: () => this.showLive() },
      ] },
      { label: 'Render', items: [
        { label: 'Render Current Frame', icon: 'renderFrame', fn: () => this.render({ ipr: false }) },
        { label: 'Redo Previous Render', fn: () => this.render(this.lastOpts) },
        { label: 'IPR', icon: 'ipr', check: () => !!engine?.job?.ipr, fn: () => this.toggleIpr() },
        { label: 'Stop Rendering', hk: 'Esc', fn: () => this.stop() },
        '-',
        { label: 'Render Sequence', fn: () => renderSequence() },
        '-',
        { label: 'Render Using', sub: () => RENDERERS.map(([v, l]) => ({ label: l, check: () => rs().renderer === v, fn: () => { rs().renderer = v; this.rendSel.value = v; this.settingsChanged('renderer'); RS_WIN?.rebuild(); } })) },
        { label: 'Render Settings...', icon: 'renderSettings', fn: () => openRenderSettings() },
      ] },
      { label: 'Options', items: [
        { section: 'Test Resolution' },
        ...[10, 25, 50, 100].map(p => ({ label: `${p}% of Render Settings (${Math.round(rs().width * p / 100)}x${Math.round(rs().height * p / 100)})`, check: () => (rs().testRes || 100) === p, fn: () => { rs().testRes = p; this.settingsChanged('testRes'); } })),
        '-',
        { label: 'Auto Resize', check: () => !!rs().autoResize, fn: () => { rs().autoResize = !rs().autoResize; if (rs().autoResize) this.fit(); } },
        { label: 'Checkerboard Behind Transparency', check: () => this.stage.classList.contains('checker'), fn: () => this.stage.classList.toggle('checker') },
      ] },
      { label: 'Help', items: [{ label: 'Render View Help', fn: () => toast('Wheel zooms, middle-drag pans. Keep Image stores snapshots for comparison.', 3500) }] },
    ];
  }
  fillCameras() {
    const rs = ensureRenderSettings();
    const cams = cameraList();
    const cur = this.camSel.value || rs.camera;
    this.camSel.innerHTML = '';
    for (const c of cams) this.camSel.append(h('option', { value: c.inca.name, text: c.inca.name }));
    this.camSel.value = cams.some(c => c.inca.name === rs.camera) ? rs.camera : cur;
  }
  bindView() {
    const b = this.body;
    b.addEventListener('wheel', (e) => { e.preventDefault(); this.setZoom(this.zoom * Math.exp(-e.deltaY * 0.0015), e); }, { passive: false });
    b.addEventListener('mousedown', (e) => {
      if (!(e.button === 1 || (e.altKey && e.button === 0))) return;
      e.preventDefault(); const x0 = e.clientX, y0 = e.clientY, sl = b.scrollLeft, st = b.scrollTop;
      const mm = (ev) => { b.scrollLeft = sl - (ev.clientX - x0); b.scrollTop = st - (ev.clientY - y0); };
      const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu);
    });
    b.addEventListener('contextmenu', (e) => e.preventDefault());
  }
  currentCanvas() { return this.viewing >= 0 ? this.kept[this.viewing]?.canvas : this.engine.out; }
  showLive() {
    this.viewing = -1;
    this.stage.replaceChildren(this.engine.out);
    this.layoutStage(); this.drawKept(); this.updateStatus(engine?.job);
  }
  showKept(i) {
    const k = this.kept[i]; if (!k) return this.showLive();
    this.viewing = i;
    this.stage.replaceChildren(k.canvas);
    this.layoutStage(); this.drawKept();
    this.statusText.textContent = 'Kept image ' + (i + 1) + ':  ' + k.info;
  }
  layoutStage() {
    const c = this.currentCanvas(); if (!c) return;
    const has = this.viewing >= 0 || !!engine?.job;
    this.empty.style.display = has ? 'none' : '';
    this.stage.style.display = has ? '' : 'none';
    if (ensureRenderSettings().autoResize && !this._userZoom) this.zoom = this.fitZoom();
    this.stage.style.width = Math.round(c.width * this.zoom) + 'px';
    this.stage.style.height = Math.round(c.height * this.zoom) + 'px';
    c.style.imageRendering = this.zoom >= 2 ? 'pixelated' : 'auto';
    this.applyDisplay();
  }
  fitZoom() {
    const c = this.currentCanvas(); const W = this.body.clientWidth - 16, H = this.body.clientHeight - 16;
    if (!c || W <= 0 || H <= 0) return 1;
    return Math.min(1, W / c.width, H / c.height);
  }
  fit() { this._userZoom = false; this.zoom = this.fitZoom(); this.layoutStage(); }
  setZoom(z, e = null) {
    this._userZoom = true;
    const b = this.body; const old = this.zoom;
    this.zoom = Math.max(0.05, Math.min(16, z));
    let fx = 0.5, fy = 0.5;
    if (e) { const r = b.getBoundingClientRect(); fx = (e.clientX - r.left + b.scrollLeft) / Math.max(1, b.scrollWidth); fy = (e.clientY - r.top + b.scrollTop) / Math.max(1, b.scrollHeight); }
    this.layoutStage();
    if (old !== this.zoom) { b.scrollLeft = fx * b.scrollWidth - (e ? e.clientX - b.getBoundingClientRect().left : b.clientWidth / 2); b.scrollTop = fy * b.scrollHeight - (e ? e.clientY - b.getBoundingClientRect().top : b.clientHeight / 2); }
  }
  setAlpha(on) { this.alpha = on; this.rgbBtn.classList.toggle('on', !on); this.aBtn.classList.toggle('on', on); this.applyDisplay(); }
  applyDisplay() {
    const amp = Math.pow(2, this.dispExposure), ex = 1 / this.dispGamma;
    const f = document.getElementById('rv-adjust');
    if (f) for (const fn of f.querySelectorAll('feFuncR,feFuncG,feFuncB')) { fn.setAttribute('amplitude', String(amp)); fn.setAttribute('exponent', String(ex)); }
    const parts = [];
    if (this.alpha) parts.push('url(#rv-alpha)');
    if (Math.abs(this.dispExposure) > 1e-4 || Math.abs(this.dispGamma - 1) > 1e-4) parts.push('url(#rv-adjust)');
    const c = this.currentCanvas(); if (c) c.style.filter = parts.join(' ');
  }

  // ---------------------------------------------------------------- rendering
  render({ ipr = false } = {}) {
    if (this.batch) { warn('A batch render is in progress.'); return null; }
    this.lastOpts = { ipr };
    if (this.viewing >= 0) this.showLive();
    this._userZoom = false;
    const p = this.engine.start({ ipr });
    this.layoutStage();
    return p;
  }
  toggleIpr() { if (engine?.job?.ipr && !engine.job.finished) this.stop(); else this.render({ ipr: true }); }
  stop() { if (this.batch) this.batch.cancel = true; this.engine.stop(true); }
  settingsChanged(key) {
    App.requestRender();
    const j = engine?.job;
    if (key === 'renderer' && j && j.ipr && !j.finished) { this.render({ ipr: true }); return; }
    if (j && j.ipr && !j.finished) { j.camNode = renderCameraNode(ensureRenderSettings().camera); j.needsScene = true; }
    if (key === 'camera' && this.camSel.value !== ensureRenderSettings().camera) this.fillCameras();
    if (key === 'renderer') this.rendSel.value = ensureRenderSettings().renderer;
  }
  onEngine(job) {
    if (this.viewing < 0) this.layoutStage();
    this.updateStatus(job);
    const running = !!job && !job.finished && !job.done;
    this.renderBtn.classList.toggle('busy', running && !job.ipr);
    this.iprBtn.classList.toggle('on', !!job && !!job.ipr && !job.finished);
  }
  statusLine(job) {
    if (!job) return 'Ready';
    const rs = ensureRenderSettings();
    const t = job.done || job.finished ? (job.elapsed ?? performance.now() - job.t0) : performance.now() - job.t0;
    const mode = job.mode === 'path' ? `Samples: ${job.samples}/${job.target}` : `Inca Hardware  AA: ${job.ss || AA[rs.hwAA] || 1}x`;
    let s = `Frame: ${fmt(job.frame, 2)}   Render Time: ${fmtTime(t)}   Camera: ${job.camNode?.inca?.name || rs.camera}   ${mode}   ${job.W}x${job.H}`;
    if (job.ipr) s = 'IPR   ' + s;
    if (job.status) s += '   ' + job.status;
    if (job.cancelled && !job.done) s += '   (stopped)';
    if (this.batch) s = this.batch.label + '   ' + s;
    return s;
  }
  updateStatus(job) {
    if (this.viewing >= 0) return;
    this.statusText.textContent = this.statusLine(job);
    const prog = job && job.mode === 'path' && !job.done ? Math.min(1, job.samples / job.target) : 0;
    this.progress.style.display = prog > 0 ? '' : 'none';
    this.progress.firstChild.style.width = (prog * 100).toFixed(1) + '%';
  }

  // ---------------------------------------------------------------- kept images / saving
  keepImage() {
    const src = this.engine.out;
    if (!engine?.job) { warn('Render an image first.'); return; }
    const c = document.createElement('canvas'); c.width = src.width; c.height = src.height; c.getContext('2d').drawImage(src, 0, 0);
    const d = new Date();
    this.kept.push({ canvas: c, info: this.statusLine(engine.job), label: `${d.getHours()}:${String(d.getMinutes()).padStart(2, '0')}:${String(d.getSeconds()).padStart(2, '0')}` });
    if (this.kept.length > 24) this.kept.shift();
    this.drawKept();
    App.help('Image kept. Click a thumbnail in the strip to compare.');
  }
  removeImage() {
    if (this.viewing < 0) { if (this.kept.length) { this.kept.pop(); this.drawKept(); } return; }
    this.kept.splice(this.viewing, 1);
    this.showLive();
  }
  drawKept() {
    const K = this.keptEl;
    K.style.display = this.kept.length ? '' : 'none';
    K.innerHTML = '';
    if (!this.kept.length) return;
    const live = h('div', { class: 'kt' + (this.viewing < 0 ? ' on' : ''), title: 'Live render', style: { width: '50px', display: 'flex', alignItems: 'center', justifyContent: 'center', background: '#2a2a2a' }, onclick: () => this.showLive() }, h('span', { text: 'Live', style: { position: 'static', fontSize: '11px' } }));
    K.append(live);
    this.kept.forEach((k, i) => {
      if (!k.thumb) { const t = document.createElement('canvas'); const sc = 50 / k.canvas.height; t.width = Math.max(1, Math.round(k.canvas.width * sc)); t.height = 50; t.getContext('2d').drawImage(k.canvas, 0, 0, t.width, t.height); k.thumb = t.toDataURL('image/jpeg', 0.7); }
      K.append(h('div', { class: 'kt' + (i === this.viewing ? ' on' : ''), title: k.info, onclick: () => this.showKept(i) }, h('img', { src: k.thumb }), h('span', { text: k.label })));
    });
  }
  async saveImage() {
    const canvas = this.currentCanvas();
    if (!canvas || (this.viewing < 0 && !engine?.job)) { warn('There is no image to save.'); return; }
    const rs = ensureRenderSettings(); const ext = extOf(rs.format);
    const n = N();
    if (n && n.saveDialog) {
      const dir = await defaultDir('images');
      try { await n.mkdir?.(dir); } catch { /* ignore */ }
      let p = await n.saveDialog({ title: 'Save Image', defaultPath: joinPath(dir, `${rs.filename || 'image'}.${ext}`), filters: [{ name: 'PNG Image', extensions: ['png'] }, { name: 'JPEG Image', extensions: ['jpg', 'jpeg'] }] });
      if (!p) return;
      if (!/\.(png|jpe?g)$/i.test(p)) p += '.' + ext;
      const type = /\.jpe?g$/i.test(p) ? 'image/jpeg' : 'image/png';
      try { await writeCanvas(canvas, p, type); App.emit('result', `// Result: ${p}`); toast('Saved ' + p.split(/[\\/]/).pop()); }
      catch (e) { App.emit('error', '// Error: Could not save image: ' + (e.message || e)); }
    } else {
      await writeCanvas(canvas, `${rs.filename || 'image'}.${ext}`, mimeOf(ext));
    }
  }
}
function iconEl(name) { const s = document.createElement('span'); s.style.cssText = 'display:inline-flex;width:18px;height:18px;vertical-align:middle'; s.innerHTML = icon(name); const svg = s.querySelector('svg'); if (svg) { svg.style.width = '18px'; svg.style.height = '18px'; } return s; }

// ------------------------------------------------------------------ batch render (Render Sequence)
export async function renderSequence() {
  const rs = ensureRenderSettings();
  const n = N();
  let dir;
  if (App.project) dir = joinPath(App.project, 'images');
  else if (n && n.openDialog) { dir = await n.openDialog({ title: 'Choose a folder for the rendered images', directory: true }); if (!dir) return; }
  else { dir = ''; warn('No project set: images will be downloaded.'); }
  const rv = openRenderView(false);
  if (rv.batch) { warn('A batch render is already running.'); return; }
  const ext = extOf(rs.format); const type = mimeOf(ext);
  const start = Math.round(rs.start ?? App.time.start), end = Math.round(rs.end ?? App.time.end), by = Math.max(1, Math.round(rs.by || 1));
  const frames = []; for (let f = start; f <= end; f += by) frames.push(f);
  if (!frames.length) { warn('The frame range is empty.'); return; }
  if (dir && n?.mkdir) { try { await n.mkdir(dir); } catch { /* ignore */ } }
  const t0 = App.time.current; const tStart = performance.now();
  const batch = rv.batch = { cancel: false, label: '' };
  echo(`// Batch render: ${frames.length} frames → ${dir || 'downloads'}`);
  let done = 0, last = null;
  try {
    for (const f of frames) {
      if (batch.cancel) break;
      batch.label = `Batch: frame ${f} (${done + 1}/${frames.length})`;
      App.setTime(f);
      await sleep(0);
      rv.lastOpts = { ipr: false };
      if (rv.viewing >= 0) rv.showLive();
      const job = await rv.engine.start({ ipr: false });
      if (!job || job.cancelled || batch.cancel) break;
      const name = `${rs.filename || 'image'}.${pad4(f)}.${ext}`;
      last = dir ? joinPath(dir, name) : name;
      await writeCanvas(rv.engine.out, last, type);
      done++;
    }
  } catch (e) { console.error(e); App.emit('error', '// Error: Batch render failed: ' + (e.message || e)); }
  finally {
    rv.batch = null;
    App.setTime(t0);
    rv.updateStatus(engine?.job);
  }
  const msg = `Rendered ${done} of ${frames.length} frames in ${fmtTime(performance.now() - tStart)}${dir ? ' to ' + dir : ''}`;
  App.emit('result', '// ' + msg);
  toast(msg, 3500);
  if (done && n?.showItem && last && dir) rv.statusText.append(' ', h('a', { href: '#', text: 'Show in folder', style: { color: '#8fc3ea' }, onclick: (e) => { e.preventDefault(); n.showItem(last); } }));
}

// ------------------------------------------------------------------ Render Settings window
let RS_WIN = null;
export function openRenderSettings() {
  injectCss(); ensureRenderSettings();
  const win = new FloatWin('renderSettings', 'Render Settings', { w: 540, h: 640, minW: 420, minH: 300 });
  if (win.reused && RS_WIN) { RS_WIN.rebuild(); return win; }
  RS_WIN = new RenderSettingsWin(win);
  win.onClose = () => { RS_WIN = null; };
  return win;
}
class RenderSettingsWin {
  constructor(win) {
    this.win = win; this.tab = 'common';
    this.root = h('div', { class: 'rs-win' });
    win.body.append(this.root);
    win.el.addEventListener('keydown', (e) => { const tag = e.target.tagName; if (tag === 'INPUT' || tag === 'SELECT' || tag === 'TEXTAREA') e.stopPropagation(); });
    this.rebuild();
  }
  changed(key) {
    App.requestRender();
    if (RV) RV.settingsChanged(key);
    if (!App.modified) { App.modified = true; App.dirty('title'); }
  }
  rebuild() {
    const rs = ensureRenderSettings();
    const scroll = this.body ? this.body.scrollTop : 0;
    this.root.innerHTML = '';
    const rendSel = h('select', {}, RENDERERS.map(([v, l]) => h('option', { value: v, text: l })));
    rendSel.value = rs.renderer;
    rendSel.addEventListener('change', () => { rs.renderer = rendSel.value; this.changed('renderer'); this.rebuild(); });
    const top = h('div', { class: 'rs-top' }, h('label', { text: 'Render Using' }), rendSel,
      rs.renderer === 'path' && ptError ? h('span', { class: 'dim', text: '(unavailable — falls back to Inca Hardware)', title: String(ptError.message || ptError) }) : null);
    const rendLabel = RENDERERS.find(r => r[0] === rs.renderer)?.[1] || 'Renderer';
    const tabs = h('div', { class: 'rs-tabs' },
      h('div', { class: 'rs-tab' + (this.tab === 'common' ? ' on' : ''), text: 'Common', onmousedown: () => { this.tab = 'common'; this.rebuild(); } }),
      h('div', { class: 'rs-tab' + (this.tab === 'renderer' ? ' on' : ''), text: rendLabel, onmousedown: () => { this.tab = 'renderer'; this.rebuild(); } }));
    this.body = h('div', { class: 'rs-body' });
    if (this.tab === 'common') this.common(rs); else this.rendererTab(rs);
    const buttons = h('div', { class: 'dlg-buttons' },
      h('button', { text: 'Render', onclick: () => openRenderView(true) }),
      h('button', { text: 'Render Sequence', onclick: () => renderSequence() }),
      h('button', { text: 'Close', onclick: () => this.win.close() }));
    this.root.append(top, tabs, this.body, buttons);
    this.body.scrollTop = scroll;
  }
  // ---- field builders
  frame(title, rows) {
    const f = h('div', { class: 'frame' }); const head = h('div', { class: 'frame-head' }, h('span', { class: 'tri', text: '▼' }), title);
    this._closed ||= new Set();
    if (this._closed.has(title)) { f.classList.add('closed'); head.firstChild.textContent = '▶'; }
    head.addEventListener('click', () => { f.classList.toggle('closed'); const c = f.classList.contains('closed'); head.firstChild.textContent = c ? '▶' : '▼'; if (c) this._closed.add(title); else this._closed.delete(title); });
    f.append(head, h('div', { class: 'frame-body' }, rows));
    this.body.append(f);
  }
  row(label, ...els) { return h('div', { class: 'rs-row' }, h('label', { text: label }), ...els); }
  num(rs, key, { int = false, min = -Infinity, max = Infinity, step = 0.1, after = null } = {}) {
    const f = numField(rs[key], (v, live) => { v = Math.max(min, Math.min(max, int ? Math.round(v) : v)); rs[key] = v; if (!live) { f.set(v); this.changed(key); after && after(v); } }, { step, int });
    return f;
  }
  check(rs, key, after = null) { const c = h('input', { type: 'checkbox', checked: !!rs[key] }); c.addEventListener('change', () => { rs[key] = c.checked; this.changed(key); after && after(); }); return c; }
  select(rs, key, opts, after = null) {
    const s = h('select', {}, opts.map(([v, l]) => h('option', { value: String(v), text: l })));
    s.value = String(rs[key]);
    s.addEventListener('change', () => { const o = opts.find(x => String(x[0]) === s.value); rs[key] = o ? o[0] : s.value; this.changed(key); after && after(); });
    return s;
  }
  text(rs, key, after = null) { const t = h('input', { class: 'wide', value: rs[key] ?? '' }); t.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') t.blur(); }); t.addEventListener('change', () => { rs[key] = t.value.trim(); this.changed(key); after && after(); }); return t; }
  color(rs, key) {
    const sw = h('div', { class: 'csw', style: { background: colorToCss(rs[key] || [0, 0, 0]) } });
    sw.addEventListener('click', (e) => colorPicker(e.clientX, e.clientY, (rs[key] || [0, 0, 0]).slice(), (c) => { rs[key] = c.slice(); sw.style.background = colorToCss(c); }, () => this.changed(key)));
    return sw;
  }
  common(rs) {
    const ext = extOf(rs.format);
    const pathPreview = h('span', { class: 'note' });
    const updPreview = () => {
      const e2 = extOf(rs.format);
      const name = rs.animation ? `${rs.filename || 'image'}.${pad4(rs.start)}.${e2}` : `${rs.filename || 'image'}.${e2}`;
      pathPreview.textContent = (App.project ? joinPath(App.project, 'images') : '<project>/images') + '/' + name;
    };
    updPreview();
    this.frame('File Output', [
      this.row('File name prefix', this.text(rs, 'filename', updPreview)),
      this.row('Image format', this.select(rs, 'format', [['png', 'PNG (png)'], ['jpg', 'JPEG (jpg)']], updPreview)),
      ext === 'jpg' ? this.row('JPEG quality', this.num(rs, 'jpgQuality', { min: 0.1, max: 1, step: 0.01 })) : null,
      this.row('Path', pathPreview),
    ]);
    const animRow = h('div', { class: 'radio', style: { display: 'flex', gap: '12px' } },
      h('label', {}, h('input', { type: 'radio', name: 'rs-anim', checked: !rs.animation, onchange: () => { rs.animation = false; this.changed('animation'); this.rebuild(); } }), 'Single frame'),
      h('label', {}, h('input', { type: 'radio', name: 'rs-anim', checked: !!rs.animation, onchange: () => { rs.animation = true; this.changed('animation'); this.rebuild(); } }), 'Animation (name.#.ext)'));
    this.frame('Frame Range', [
      this.row('Frame/Animation ext', animRow),
      this.row('Start frame', this.num(rs, 'start', { int: true, step: 1, after: updPreview })),
      this.row('End frame', this.num(rs, 'end', { int: true, step: 1 })),
      this.row('By frame', this.num(rs, 'by', { int: true, min: 1, step: 1 })),
      this.row('', h('button', { text: 'Use Time Slider Range', onclick: () => { rs.start = App.time.start; rs.end = App.time.end; this.changed('start'); this.rebuild(); } })),
      this.row('', h('span', { class: 'note', text: 'Render > Render Sequence renders Start..End into the images folder.' })),
    ]);
    const cams = cameraList();
    const camSel = this.select(rs, 'camera', cams.map(c => [c.inca.name, c.inca.name]), () => RV?.fillCameras());
    this.frame('Renderable Cameras', [this.row('Renderable camera', camSel)]);
    // image size
    const presetSel = h('select', {}, [...SIZE_PRESETS.map(([n0, w, hh]) => h('option', { value: n0, text: `${n0}  (${w}x${hh})` })), h('option', { value: 'Custom', text: 'Custom' })]);
    const match = SIZE_PRESETS.find(p => p[1] === rs.width && p[2] === rs.height);
    presetSel.value = match ? match[0] : 'Custom';
    presetSel.addEventListener('change', () => { const p = SIZE_PRESETS.find(x => x[0] === presetSel.value); if (p) { rs.width = p[1]; rs.height = p[2]; this.changed('width'); this.rebuild(); } });
    const ratio = rs.width / rs.height;
    const wF = this.num(rs, 'width', { int: true, min: 1, max: 16384, step: 1, after: (v) => { if (rs.keepRatio) { rs.height = Math.max(1, Math.round(v / ratio)); } this.changed('height'); this.rebuild(); } });
    const hF = this.num(rs, 'height', { int: true, min: 1, max: 16384, step: 1, after: (v) => { if (rs.keepRatio) { rs.width = Math.max(1, Math.round(v * ratio)); } this.changed('width'); this.rebuild(); } });
    this.frame('Image Size', [
      this.row('Presets', presetSel),
      this.row('Maintain width/height ratio', this.check(rs, 'keepRatio')),
      this.row('Width', wF, h('span', { class: 'note', text: 'pixels' })),
      this.row('Height', hF, h('span', { class: 'note', text: 'pixels' })),
      this.row('Device aspect ratio', h('span', { text: fmt(rs.width * (rs.pixelAspect || 1) / rs.height, 3) })),
      this.row('Pixel aspect ratio', this.num(rs, 'pixelAspect', { min: 0.1, max: 10, step: 0.01, after: () => this.rebuild() })),
      this.row('Test resolution', this.select(rs, 'testRes', [[100, '100%'], [50, '50%'], [25, '25%'], [10, '10%']])),
    ]);
  }
  rendererTab(rs) {
    if (rs.renderer === 'path') {
      this.frame('Sampling', [
        this.row('Samples', this.num(rs, 'samples', { int: true, min: 1, max: 100000, step: 1 }), h('span', { class: 'note', text: 'per pixel (progressive)' })),
        this.row('Max bounces', this.num(rs, 'bounces', { int: true, min: 1, max: 64, step: 1 })),
        this.row('Transmission bounces', this.num(rs, 'transmissiveBounces', { int: true, min: 1, max: 64, step: 1 })),
        this.row('Filter glossy', this.num(rs, 'filterGlossy', { min: 0, max: 1, step: 0.01 }), h('span', { class: 'note', text: 'reduces fireflies' })),
        this.row('Caustics', this.check(rs, 'caustics'), h('span', { class: 'note', text: 'unfiltered glossy paths (noisier)' })),
      ]);
      this.frame('Color Management', [
        this.row('Tone mapping', this.select(rs, 'toneMapping', TM_OPTS)),
        this.row('Exposure', this.num(rs, 'exposure', { min: -20, max: 20, step: 0.05 }), h('span', { class: 'note', text: 'stops' })),
      ]);
    } else {
      this.frame('Anti-aliasing', [
        this.row('Quality', this.select(rs, 'hwAA', [['off', 'Off (MSAA only)'], ['low', 'Low (1x + MSAA)'], ['medium', 'Medium (2x2 supersampling)'], ['high', 'High (3x3 supersampling)'], ['ultra', 'Ultra (4x4 supersampling)']])),
      ]);
      this.frame('Shadows', [
        this.row('Shadows on all lights', this.check(rs, 'hwAllShadows'), h('span', { class: 'note', text: 'off: only lights with shadows enabled' })),
        this.row('Shadow map size', this.select(rs, 'hwShadowMap', [[512, '512'], [1024, '1024'], [2048, '2048'], [4096, '4096'], [8192, '8192']])),
      ]);
      this.frame('Color Management', [
        this.row('Tone mapping', this.select(rs, 'hwToneMapping', TM_OPTS)),
        this.row('Exposure', this.num(rs, 'exposure', { min: -20, max: 20, step: 0.05 }), h('span', { class: 'note', text: 'stops' })),
      ]);
    }
    this.frame('Environment', [
      this.row('Background color', this.color(rs, 'background')),
      this.row('Transparent background', this.check(rs, 'transparentBg'), h('span', { class: 'note', text: 'alpha = 0 behind objects' })),
      this.row('', h('span', { class: 'note', text: 'A visible Sky Dome Light lights the scene with its texture or a procedural sky.' })),
    ]);
  }
}

// ------------------------------------------------------------------ Playblast
export function playblast() {
  injectCss();
  const win = new FloatWin('playblast', 'Playblast Options', { w: 460, h: 330, minW: 360, minH: 240 });
  if (win.reused) return win;
  const o = playblast.opts ||= { range: 'slider', start: App.time.start, end: App.time.end, scale: 100, format: 'webm', path: '' };
  const body = h('div', { class: 'dlg-body' });
  const pathIn = h('input', { class: 'wide', value: o.path, placeholder: '<project>/movies/playblast.webm' });
  pathIn.addEventListener('keydown', (e) => e.stopPropagation());
  pathIn.addEventListener('change', () => { o.path = pathIn.value.trim(); });
  const startF = numField(o.start, (v) => { o.start = Math.round(v); }, { int: true, step: 1 });
  const endF = numField(o.end, (v) => { o.end = Math.round(v); }, { int: true, step: 1 });
  const setRangeEnabled = () => { startF.disabled = endF.disabled = o.range === 'slider'; if (o.range === 'slider') { startF.set(App.time.start); endF.set(App.time.end); } };
  const fmtSel = h('select', {}, h('option', { value: 'webm', text: 'WebM video (.webm)' }), h('option', { value: 'png', text: 'PNG image sequence' }));
  fmtSel.value = o.format;
  fmtSel.addEventListener('change', () => { o.format = fmtSel.value; if (!o.path) pathIn.placeholder = o.format === 'webm' ? '<project>/movies/playblast.webm' : '<project>/movies/playblast.####.png'; else pathIn.value = o.path = o.path.replace(/\.(webm|png)$/i, '') + (o.format === 'webm' ? '.webm' : '.png'); });
  const scaleSel = h('select', {}, [25, 50, 75, 100].map(s => h('option', { value: s, text: s + '%' })));
  scaleSel.value = String(o.scale); scaleSel.addEventListener('change', () => { o.scale = +scaleSel.value; });
  const browse = h('button', { text: 'Browse…', onclick: async () => {
    const n = N(); if (!n || !n.saveDialog) { toast('Browsing requires the desktop app.'); return; }
    const dir = await defaultDir('movies');
    const p = await n.saveDialog({ title: 'Playblast Output', defaultPath: joinPath(dir, o.format === 'webm' ? 'playblast.webm' : 'playblast.png'), filters: o.format === 'webm' ? [{ name: 'WebM Video', extensions: ['webm'] }] : [{ name: 'PNG Image', extensions: ['png'] }] });
    if (p) { o.path = p; pathIn.value = p; }
  } });
  body.append(
    h('div', { class: 'dlg-sect', text: 'Time Range' }),
    h('div', { class: 'dlg-row' }, h('label', { text: 'Time range' }), h('div', { class: 'radio' },
      h('label', {}, h('input', { type: 'radio', name: 'pb-range', checked: o.range === 'slider', onchange: () => { o.range = 'slider'; setRangeEnabled(); } }), 'Time Slider'),
      h('label', {}, h('input', { type: 'radio', name: 'pb-range', checked: o.range === 'custom', onchange: () => { o.range = 'custom'; setRangeEnabled(); } }), 'Start/End'))),
    h('div', { class: 'dlg-row' }, h('label', { text: 'Start time' }), startF),
    h('div', { class: 'dlg-row' }, h('label', { text: 'End time' }), endF),
    h('div', { class: 'dlg-sect', text: 'Output' }),
    h('div', { class: 'dlg-row' }, h('label', { text: 'Format' }), fmtSel),
    h('div', { class: 'dlg-row' }, h('label', { text: 'Scale' }), scaleSel),
    h('div', { class: 'dlg-row' }, h('label', { text: 'File' }), pathIn, browse));
  setRangeEnabled();
  const status = h('span', { class: 'dim', style: { padding: '0 8px', alignSelf: 'center' } });
  const goBtn = h('button', { text: 'Playblast' });
  goBtn.addEventListener('click', async () => {
    if (playblast.running) { playblast.running.cancel = true; return; }
    goBtn.textContent = 'Cancel';
    try { await runPlayblast({ ...o, start: o.range === 'slider' ? App.time.start : o.start, end: o.range === 'slider' ? App.time.end : o.end }, (s) => { status.textContent = s; }); }
    finally { goBtn.textContent = 'Playblast'; }
  });
  win.body.append(body, h('div', { class: 'dlg-buttons' }, status, goBtn, h('button', { text: 'Close', onclick: () => { if (playblast.running) playblast.running.cancel = true; win.close(); } })));
  win.el.addEventListener('keydown', (e) => { if (e.key === 'Escape' && playblast.running) { playblast.running.cancel = true; e.stopPropagation(); } });
  return win;
}
async function runPlayblast(o, onStatus = () => {}) {
  const vp = App.activeViewport || App.viewports?.[0];
  if (!vp || !vp.canvas) { warn('No active viewport to playblast.'); return; }
  const n = N();
  const isVideo = o.format === 'webm';
  // output path
  let out = o.path;
  if (!out) { const dir = await defaultDir('movies'); out = joinPath(dir, isVideo ? 'playblast.webm' : 'playblast.png'); }
  else if (!/[\\/]/.test(out) && App.project) out = joinPath(App.project, 'movies', out);
  if (isVideo && !/\.webm$/i.test(out)) out += '.webm';
  if (n?.mkdir) { try { await n.mkdir(out.replace(/[\\/][^\\/]*$/, '')); } catch { /* ignore */ } }
  const start = Math.round(Math.min(o.start, o.end)), end = Math.round(Math.max(o.start, o.end));
  const fps = App.time.fps || 24;
  const run = playblast.running = { cancel: false };
  const t0 = App.time.current;
  vp.render();
  const sw = vp.canvas.width, sh = vp.canvas.height;
  const k = (o.scale || 100) / 100;
  const W = Math.max(2, Math.round(sw * k / 2) * 2), H = Math.max(2, Math.round(sh * k / 2) * 2);
  const cv = document.createElement('canvas'); cv.width = W; cv.height = H;
  const ctx = cv.getContext('2d');
  const draw = () => { ctx.imageSmoothingQuality = 'high'; ctx.drawImage(vp.canvas, 0, 0, vp.canvas.width, vp.canvas.height, 0, 0, W, H); };
  let rec = null, chunks = [], track = null, stopped = null;
  if (isVideo) {
    if (typeof MediaRecorder === 'undefined' || !cv.captureStream) { warn('Video recording is not supported here; use a PNG image sequence.'); playblast.running = null; return; }
    let stream = cv.captureStream(0); track = stream.getVideoTracks()[0];
    if (!track || !track.requestFrame) { stream = cv.captureStream(fps); track = stream.getVideoTracks()[0]; }
    const mime = ['video/webm;codecs=vp9', 'video/webm;codecs=vp8', 'video/webm'].find(m => MediaRecorder.isTypeSupported(m)) || '';
    rec = new MediaRecorder(stream, mime ? { mimeType: mime, videoBitsPerSecond: Math.round(Math.max(2e6, W * H * fps * 0.25)) } : undefined);
    rec.ondataavailable = (e) => { if (e.data && e.data.size) chunks.push(e.data); };
    stopped = new Promise(r => { rec.onstop = r; });
    rec.start();
  }
  echo(`playblast -startTime ${start} -endTime ${end} -format ${isVideo ? 'webm' : 'image'} -percent ${o.scale} -filename "${out}";`);
  let frames = 0; let lastPng = null; let blob = null;
  try {
    for (let f = start; f <= end; f++) {
      if (run.cancel) break;
      const tf = performance.now();
      App.setTime(f);
      vp.render();
      draw();
      frames++;
      onStatus(`Frame ${f} (${frames}/${end - start + 1})`);
      App.help(`Playblast: frame ${f}`);
      if (isVideo) {
        if (track && track.requestFrame) track.requestFrame();
        const wait = 1000 / fps - (performance.now() - tf);
        await sleep(Math.max(0, wait));
      } else {
        const base = out.replace(/\.png$/i, '');
        lastPng = `${base}.${pad4(f)}.png`;
        if (n?.writeBinary) await writeCanvas(cv, lastPng, 'image/png');
        else { const b = await canvasBlob(cv, 'image/png'); downloadBlob(b, lastPng.split(/[\\/]/).pop()); await sleep(120); }
        await sleep(0);
      }
    }
    if (isVideo) {
      await sleep(80); rec.stop(); await stopped;
      blob = new Blob(chunks, { type: 'video/webm' });
      if (n?.writeBinary) await n.writeBinary(out, new Uint8Array(await blob.arrayBuffer()));
    }
  } catch (e) {
    console.error(e); App.emit('error', '// Error: Playblast failed: ' + (e.message || e));
    if (rec && rec.state !== 'inactive') rec.stop();
    return;
  } finally {
    playblast.running = null;
    App.setTime(t0); App.requestRender();
    App.help('');
  }
  if (!frames) return;
  onStatus(run.cancel ? `Cancelled after ${frames} frames` : `Done: ${frames} frames`);
  App.emit('result', `// Result: ${isVideo ? out : lastPng}`);
  showPlayblastResult({ blob, path: n?.writeBinary ? (isVideo ? out : lastPng) : null, isVideo, png: !isVideo ? cv : null });
}
function showPlayblastResult({ blob, path, isVideo, png }) {
  injectCss();
  const title = 'Playblast' + (path ? ' — ' + path.split(/[\\/]/).pop() : '');
  const win = new FloatWin('playblastPreview', title, { w: 680, h: 460, minW: 300, minH: 200 });
  win.setTitle(title);
  win.body.innerHTML = '';
  if (previewUrl) { URL.revokeObjectURL(previewUrl); previewUrl = null; }
  let url = null;
  let media;
  if (isVideo && blob) { url = previewUrl = URL.createObjectURL(blob); media = h('video', { src: url, controls: true, autoplay: true, loop: true }); }
  else { media = h('img', { src: png ? png.toDataURL('image/png') : '', style: { maxWidth: '100%', maxHeight: '100%', objectFit: 'contain', flex: 1, minHeight: 0, background: '#000' } }); }
  const n = N();
  const btns = h('div', { class: 'dlg-buttons' },
    path && n?.showItem ? h('button', { text: 'Show in Folder', onclick: () => n.showItem(path) }) : null,
    isVideo && blob && !path ? h('button', { text: 'Download', onclick: () => downloadBlob(blob, 'playblast.webm') }) : null,
    h('button', { text: 'Close', onclick: () => win.close() }));
  win.body.append(h('div', { class: 'pb-prev', style: { flex: 1, display: 'flex', flexDirection: 'column', minHeight: 0, background: '#000' } }, media), path ? h('div', { class: 'dim', style: { padding: '2px 8px', fontSize: '11px' }, text: path }) : null, btns);
  win.onClose = () => { if (url) URL.revokeObjectURL(url); if (previewUrl === url) previewUrl = null; };
}
let previewUrl = null;

// ------------------------------------------------------------------ registration
App.ui = App.ui || {};
App.ui.renderView = (renderNow = false, { ipr = false } = {}) => openRenderView(renderNow, { ipr });
App.ui.renderSettings = openRenderSettings;
App.ui.playblast = playblast;
App.ui.renderSequence = renderSequence;
