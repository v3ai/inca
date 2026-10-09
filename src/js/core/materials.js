// Inca — shading nodes (materials + textures) mapped onto three.js physical materials.
import * as THREE from 'three';
import { App } from './app.js';
import { uniqueName, registerName, releaseName } from './names.js';

export const MAT_TYPES = {
  lambert: { label: 'Lambert', defaults: { color: [0.5, 0.5, 0.5], transparency: [0, 0, 0], ambientColor: [0, 0, 0], incandescence: [0, 0, 0], diffuse: 0.8, bump: 0 } },
  blinn: { label: 'Blinn', defaults: { color: [0.5, 0.5, 0.5], transparency: [0, 0, 0], ambientColor: [0, 0, 0], incandescence: [0, 0, 0], diffuse: 0.8, eccentricity: 0.3, specularRollOff: 0.7, specularColor: [0.5, 0.5, 0.5], reflectivity: 0.5, bump: 0 } },
  phong: { label: 'Phong', defaults: { color: [0.5, 0.5, 0.5], transparency: [0, 0, 0], ambientColor: [0, 0, 0], incandescence: [0, 0, 0], diffuse: 0.8, cosinePower: 20, specularColor: [0.5, 0.5, 0.5], reflectivity: 0.5, bump: 0 } },
  standardSurface: { label: 'Standard Surface', defaults: { base: 0.8, baseColor: [0.8, 0.8, 0.8], diffuseRoughness: 0, metalness: 0, specular: 1, specularColor: [1, 1, 1], specularRoughness: 0.2, specularIOR: 1.5, transmission: 0, transmissionColor: [1, 1, 1], coat: 0, coatRoughness: 0.1, sheen: 0, sheenColor: [1, 1, 1], emission: 0, emissionColor: [1, 1, 1], opacity: [1, 1, 1], thinWalled: false, bump: 0 } },
  surfaceShader: { label: 'Surface Shader', defaults: { outColor: [0.5, 0.5, 0.5], outTransparency: [0, 0, 0] } },
};
export const TEX_TYPES = {
  file: { label: 'File', defaults: { fileTextureName: '', repeatU: 1, repeatV: 1, offsetU: 0, offsetV: 0, rotateUV: 0, colorSpace: 'sRGB' } },
  checker: { label: 'Checker', defaults: { color1: [1, 1, 1], color2: [0, 0, 0], repeatU: 4, repeatV: 4, contrast: 1 } },
  ramp: { label: 'Ramp', defaults: { type: 'V Ramp', colors: [[0, [1, 0, 0]], [0.5, [0, 1, 0]], [1, [0, 0, 1]]], repeatU: 1, repeatV: 1 } },
  noise: { label: 'Noise', defaults: { color1: [1, 1, 1], color2: [0, 0, 0], frequency: 8, amplitude: 1, repeatU: 1, repeatV: 1 } },
  grid: { label: 'Grid', defaults: { lineColor: [1, 1, 1], fillerColor: [0, 0, 0], uWidth: 0.1, vWidth: 0.1, repeatU: 4, repeatV: 4 } },
};
// which material attributes accept a texture connection
export const MAPPABLE = {
  lambert: ['color', 'transparency', 'incandescence', 'bump'], blinn: ['color', 'transparency', 'incandescence', 'specularColor', 'bump'], phong: ['color', 'transparency', 'incandescence', 'specularColor', 'bump'],
  standardSurface: ['baseColor', 'specularRoughness', 'metalness', 'emissionColor', 'opacity', 'bump'], surfaceShader: ['outColor'],
};

const deep = (o) => JSON.parse(JSON.stringify(o));

export function createMaterial(type = 'lambert', name = null, { id = null, attrs = null, maps = null } = {}) {
  const def = MAT_TYPES[type] || MAT_TYPES.lambert;
  const rec = { id: id || App.newId('m'), name: name ? registerName(name) : uniqueName(type + '1'), type, attrs: { ...deep(def.defaults), ...(attrs ? deep(attrs) : {}) }, maps: maps ? { ...maps } : {}, kind: 'material' };
  App.mats.set(rec.id, rec);
  App.nodes.set(rec.id, rec);
  return rec;
}
export function createTexture(type = 'file', name = null, { id = null, attrs = null } = {}) {
  const def = TEX_TYPES[type] || TEX_TYPES.file;
  const rec = { id: id || App.newId('t'), name: name ? registerName(name) : uniqueName(type + '1'), type, attrs: { ...deep(def.defaults), ...(attrs ? deep(attrs) : {}) }, kind: 'texture' };
  App.texs.set(rec.id, rec);
  App.nodes.set(rec.id, rec);
  return rec;
}
export function deleteShadingNode(rec) {
  if (!rec || rec.id === App.defaultMatId) return;
  if (rec.kind === 'material') {
    App.mats.delete(rec.id);
    // reassign objects to default
    for (const o of App.nodes.values()) if (o.isObject3D && o.inca) {
      if (o.inca.material === rec.id) o.inca.material = App.defaultMatId;
      const pm = o.inca.mesh; if (pm && pm.fm) pm.fm = pm.fm.map(m => m === rec.id ? null : m);
    }
    disposeMat(rec);
  } else {
    App.texs.delete(rec.id);
    for (const m of App.mats.values()) for (const k in m.maps) if (m.maps[k] === rec.id) { delete m.maps[k]; updateMaterial(m); }
    if (rec._tex) rec._tex.dispose();
  }
  App.nodes.delete(rec.id); releaseName(rec.name);
}
function disposeMat(rec) { if (rec._m) rec._m.dispose(); if (rec._mu) rec._mu.dispose(); rec._m = rec._mu = null; }

const col = (c, s = 1) => new THREE.Color(c[0] * s, c[1] * s, c[2] * s);
const lum = (c) => (c[0] + c[1] + c[2]) / 3;

export function getThreeTexture(tid) {
  const t = App.texs.get(tid); if (!t) return null;
  if (t._tex && t._texKey === JSON.stringify(t.attrs)) return t._tex;
  if (t._tex) t._tex.dispose();
  let tex;
  const a = t.attrs;
  if (t.type === 'file') {
    if (!a.fileTextureName) { tex = new THREE.CanvasTexture(solidCanvas([0.5, 0.5, 0.5])); }
    else {
      const url = fileUrl(a.fileTextureName);
      tex = new THREE.TextureLoader().load(url, () => { App.requestRender(); App.emit('textureLoaded', t); }, undefined, () => console.warn('texture load failed', url));
    }
    tex.colorSpace = a.colorSpace === 'Raw' ? THREE.NoColorSpace : THREE.SRGBColorSpace;
  } else {
    tex = new THREE.CanvasTexture(proceduralCanvas(t));
    tex.colorSpace = THREE.SRGBColorSpace;
  }
  tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
  if (t.type === 'file') { tex.repeat.set(a.repeatU || 1, a.repeatV || 1); tex.offset.set(a.offsetU || 0, a.offsetV || 0); tex.rotation = (a.rotateUV || 0) * Math.PI / 180; }
  else { tex.repeat.set(1, 1); }
  tex.anisotropy = 8;
  t._tex = tex; t._texKey = JSON.stringify(t.attrs);
  return tex;
}
export function fileUrl(p) {
  if (/^(data:|blob:|https?:|file:)/.test(p)) return p;
  let path = p;
  if (App.project && !/^([a-zA-Z]:)?[\\/]/.test(p)) path = App.project.replace(/[\\/]$/, '') + '/' + p;
  return 'file://' + (path.startsWith('/') ? '' : '/') + path.replace(/\\/g, '/');
}
function solidCanvas(c) { const cv = document.createElement('canvas'); cv.width = cv.height = 4; const x = cv.getContext('2d'); x.fillStyle = rgb(c); x.fillRect(0, 0, 4, 4); return cv; }
const rgb = (c) => `rgb(${Math.round(Math.min(1, c[0]) * 255)},${Math.round(Math.min(1, c[1]) * 255)},${Math.round(Math.min(1, c[2]) * 255)})`;
export function proceduralCanvas(t, size = 512) {
  const cv = document.createElement('canvas'); cv.width = cv.height = size; const x = cv.getContext('2d'); const a = t.attrs;
  if (t.type === 'checker') {
    const nu = Math.max(1, a.repeatU), nv = Math.max(1, a.repeatV);
    for (let j = 0; j < nv * 2; j++) for (let i = 0; i < nu * 2; i++) { x.fillStyle = rgb((i + j) % 2 ? a.color2 : a.color1); x.fillRect(i * size / (nu * 2), j * size / (nv * 2), size / (nu * 2) + 1, size / (nv * 2) + 1); }
  } else if (t.type === 'grid') {
    x.fillStyle = rgb(a.fillerColor); x.fillRect(0, 0, size, size); x.fillStyle = rgb(a.lineColor);
    const nu = Math.max(1, a.repeatU), nv = Math.max(1, a.repeatV);
    for (let i = 0; i < nu; i++) x.fillRect(i * size / nu, 0, a.uWidth * size / nu, size);
    for (let j = 0; j < nv; j++) x.fillRect(0, j * size / nv, size, a.vWidth * size / nv);
  } else if (t.type === 'ramp') {
    const g = a.type === 'U Ramp' ? x.createLinearGradient(0, 0, size, 0) : a.type === 'Circular Ramp' ? x.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2) : x.createLinearGradient(0, size, 0, 0);
    for (const [p, c] of a.colors) g.addColorStop(Math.max(0, Math.min(1, p)), rgb(c));
    x.fillStyle = g; x.fillRect(0, 0, size, size);
  } else if (t.type === 'noise') {
    const img = x.createImageData(size, size); const f = Math.max(1, a.frequency);
    const rnd = (i, j) => { const s = Math.sin(i * 127.1 + j * 311.7) * 43758.5453; return s - Math.floor(s); };
    const sm = (t) => t * t * (3 - 2 * t);
    const vn = (u, v) => { const i = Math.floor(u), j = Math.floor(v); const fu = sm(u - i), fv = sm(v - j); const P = (a, b) => rnd(((a % f) + f) % f, ((b % f) + f) % f); return (P(i, j) * (1 - fu) + P(i + 1, j) * fu) * (1 - fv) + (P(i, j + 1) * (1 - fu) + P(i + 1, j + 1) * fu) * fv; };
    for (let y = 0; y < size; y++) for (let xx = 0; xx < size; xx++) {
      let n = 0, amp = 0.5, fr = 1; for (let o = 0; o < 4; o++) { n += vn(xx / size * f * fr, y / size * f * fr) * amp; amp *= 0.5; fr *= 2; }
      n = Math.max(0, Math.min(1, 0.5 + (n - 0.5) * 2 * a.amplitude));
      const k = (y * size + xx) * 4; for (let c = 0; c < 3; c++) img.data[k + c] = 255 * (a.color2[c] * (1 - n) + a.color1[c] * n); img.data[k + 3] = 255;
    }
    x.putImageData(img, 0, 0);
  } else { x.fillStyle = '#888'; x.fillRect(0, 0, size, size); }
  return cv;
}

export function getThreeMaterial(id, textured = true) {
  const rec = App.mats.get(id) || App.mats.get(App.defaultMatId);
  if (!rec) return new THREE.MeshStandardMaterial();
  if (!rec._m) { rec._m = new THREE.MeshPhysicalMaterial(); rec._mu = new THREE.MeshPhysicalMaterial(); rec._m.userData.rec = rec; rec._mu.userData.rec = rec; applyAttrs(rec); }
  return textured ? rec._m : rec._mu;
}
export function updateMaterial(rec) { if (rec._m) applyAttrs(rec); rec._swatch = null; App.emit('materialChanged', rec); App.requestRender(); }

function applyAttrs(rec) {
  for (const [m, textured] of [[rec._m, true], [rec._mu, false]]) {
    const a = rec.attrs; const maps = rec.maps || {};
    const mapOf = (k) => (textured && maps[k]) ? getThreeTexture(maps[k]) : null;
    // Maya shows both sides of polygons unless Backface Culling is turned on
    m.side = App.backfaceCulling ? THREE.FrontSide : THREE.DoubleSide;
    m.polygonOffset = true; m.polygonOffsetFactor = 1; m.polygonOffsetUnits = 1;
    let colorMap = null, opacity = 1;
    if (rec.type === 'standardSurface') {
      m.color = col(a.baseColor, a.base);
      m.metalness = a.metalness; m.roughness = Math.max(0.02, a.specularRoughness);
      m.ior = a.specularIOR; m.specularIntensity = a.specular; m.specularColor = col(a.specularColor);
      m.transmission = a.transmission; m.thickness = a.thinWalled ? 0 : 0.5;
      m.clearcoat = a.coat; m.clearcoatRoughness = a.coatRoughness;
      m.sheen = a.sheen; m.sheenColor = col(a.sheenColor);
      m.emissive = col(a.emissionColor, a.emission); m.emissiveIntensity = 1;
      opacity = lum(a.opacity);
      colorMap = mapOf('baseColor');
      m.roughnessMap = mapOf('specularRoughness'); m.metalnessMap = mapOf('metalness'); m.emissiveMap = mapOf('emissionColor');
      if (m.emissiveMap) m.emissive = new THREE.Color(1, 1, 1).multiplyScalar(Math.max(a.emission, 0.0001));
    } else if (rec.type === 'surfaceShader') {
      m.color = new THREE.Color(0, 0, 0); m.emissive = col(a.outColor); m.roughness = 1; m.metalness = 0; m.specularIntensity = 0;
      m.emissiveMap = mapOf('outColor'); if (m.emissiveMap) m.emissive = new THREE.Color(1, 1, 1);
      opacity = 1 - lum(a.outTransparency);
      m.clearcoat = 0; m.transmission = 0; m.sheen = 0;
    } else {
      // Lambert/Blinn/Phong: approximate classic shading with the physical model
      m.color = col(a.color, a.diffuse);
      m.metalness = 0;
      if (rec.type === 'lambert') { m.roughness = 1; m.specularIntensity = 0; }
      else if (rec.type === 'blinn') { m.roughness = Math.min(1, Math.max(0.05, Math.sqrt(a.eccentricity))); m.specularIntensity = Math.min(1, lum(a.specularColor) * 2 * a.specularRollOff); m.specularColor = col(a.specularColor, 1 / Math.max(0.01, lum(a.specularColor))); }
      else { m.roughness = Math.min(1, Math.max(0.04, Math.sqrt(2 / (a.cosinePower + 2)) * 1.2)); m.specularIntensity = Math.min(1, lum(a.specularColor) * 2); m.specularColor = col(a.specularColor, 1 / Math.max(0.01, lum(a.specularColor))); }
      m.emissive = col(a.incandescence); m.emissiveMap = mapOf('incandescence');
      if (m.emissiveMap) m.emissive = new THREE.Color(1, 1, 1);
      opacity = 1 - lum(a.transparency);
      colorMap = mapOf('color');
      m.ior = 1.5; m.transmission = 0; m.clearcoat = 0; m.sheen = 0;
      if (rec.type !== 'lambert') m.specularColorMap = mapOf('specularColor');
      m.roughnessMap = null; m.metalnessMap = null;
    }
    m.map = colorMap;
    if (colorMap) m.color = new THREE.Color(1, 1, 1).multiplyScalar(rec.type === 'standardSurface' ? a.base : (a.diffuse ?? 1));
    const bumpTex = mapOf('bump');
    m.bumpMap = bumpTex; m.bumpScale = (a.bump || 1) * 1;
    const transMap = rec.type === 'standardSurface' ? mapOf('opacity') : rec.type === 'surfaceShader' ? null : mapOf('transparency');
    m.alphaMap = transMap;
    m.opacity = Math.max(0, Math.min(1, opacity));
    m.transparent = m.opacity < 0.999 || !!transMap;
    m.depthWrite = !m.transparent;
    m.needsUpdate = true;
  }
}

// ---- swatches (rendered spheres) ----
let swR = null, swScene = null, swCam = null, swMesh = null;
export function swatch(rec, size = 64) {
  if (rec._swatch && rec._swatchSize === size) return rec._swatch;
  try {
    if (!swR) {
      swR = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true, alpha: true });
      swR.outputColorSpace = THREE.SRGBColorSpace; swR.toneMapping = THREE.NoToneMapping;
      swScene = new THREE.Scene(); swCam = new THREE.PerspectiveCamera(30, 1, 0.1, 100); swCam.position.set(0, 0, 4.2);
      swScene.add(new THREE.AmbientLight(0xffffff, 0.35 * Math.PI));
      const d = new THREE.DirectionalLight(0xffffff, 1.0 * Math.PI); d.position.set(-2, 2, 3); swScene.add(d);
      const d2 = new THREE.DirectionalLight(0xffffff, 0.35 * Math.PI); d2.position.set(3, -1, 1); swScene.add(d2);
      swMesh = new THREE.Mesh(new THREE.SphereGeometry(1, 48, 32)); swScene.add(swMesh);
    }
    swR.setSize(size, size, false);
    if (rec.kind === 'texture') {
      const tex = getThreeTexture(rec.id); swMesh.material = new THREE.MeshBasicMaterial({ map: tex });
      swMesh.geometry.dispose(); swMesh.geometry = new THREE.PlaneGeometry(2.6, 2.6);
    } else {
      if (!(swMesh.geometry instanceof THREE.SphereGeometry)) { swMesh.geometry.dispose(); swMesh.geometry = new THREE.SphereGeometry(1, 48, 32); }
      swMesh.material = getThreeMaterial(rec.id, true);
    }
    swR.setClearColor(0x2b2b2b, 1);
    swR.render(swScene, swCam);
    rec._swatch = swR.domElement.toDataURL('image/png'); rec._swatchSize = size;
    if (rec.kind === 'texture') { swMesh.material.dispose(); swMesh.geometry.dispose(); swMesh.geometry = new THREE.SphereGeometry(1, 48, 32); }
  } catch (e) { console.warn('swatch failed', e); rec._swatch = ''; }
  return rec._swatch;
}

export function initDefaultMaterials() {
  const lam = createMaterial('lambert', 'lambert1');
  App.defaultMatId = lam.id;
  createMaterial('standardSurface', 'standardSurface1');
  return lam;
}
export function serializeShading() {
  return {
    mats: [...App.mats.values()].map(m => ({ id: m.id, name: m.name, type: m.type, attrs: m.attrs, maps: m.maps })),
    texs: [...App.texs.values()].map(t => ({ id: t.id, name: t.name, type: t.type, attrs: t.attrs })),
    defaultMatId: App.defaultMatId,
  };
}
export function clearShading() {
  for (const m of App.mats.values()) { disposeMat(m); App.nodes.delete(m.id); releaseName(m.name); }
  for (const t of App.texs.values()) { if (t._tex) t._tex.dispose(); App.nodes.delete(t.id); releaseName(t.name); }
  App.mats.clear(); App.texs.clear();
}
export function loadShading(d) {
  clearShading();
  for (const t of d.texs || []) createTexture(t.type, t.name, { id: t.id, attrs: t.attrs });
  for (const m of d.mats || []) createMaterial(m.type, m.name, { id: m.id, attrs: m.attrs, maps: m.maps });
  App.defaultMatId = d.defaultMatId && App.mats.has(d.defaultMatId) ? d.defaultMatId : [...App.mats.keys()][0];
  if (!App.defaultMatId) initDefaultMaterials();
}
