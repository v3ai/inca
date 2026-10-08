// Inca — files & projects: scenes (.inca), import/export (OBJ, glTF/GLB, FBX, STL), Project Window, autosave.
import * as THREE from 'three';
import { App } from './app.js';
import * as S from './scene.js';
import { Undo } from './undo.js';
import { Sel } from './selection.js';
import { Cmds } from './commands.js';
import { PolyMesh } from './polymesh.js';
import { initDefaultMaterials, clearShading, createMaterial, createTexture, updateMaterial } from './materials.js';
import { h, FloatWin, confirmDialog, toast } from '../ui/dom.js';

const N = () => window.incaNative;
const C = Cmds.register;
const echo = (s) => App.emit('echo', s);
const SEP = () => (App._paths?.sep || '/');
export const PROJECT_DIRS = ['scenes', 'assets', 'images', 'sourceimages', 'renderData', 'renderData/depth', 'renderData/iprImages', 'renderData/shaders', 'clips', 'sound', 'scripts', 'data', 'movies', 'timeEditor', 'autosave', 'sceneAssembly', 'cache', 'cache/particles', 'cache/nCache', 'cache/bifrost', 'Time Editor/Clip Exports'];

const basename = (p) => String(p).split(/[\\/]/).pop();
const dirname = (p) => { const s = String(p).replace(/[\\/][^\\/]*$/, ''); return s || '/'; };
const join = (...a) => a.filter(Boolean).join('/').replace(/[\\/]+/g, '/').replace(/^([a-zA-Z]):\//, '$1:/');
const stem = (p) => basename(p).replace(/\.[^.]+$/, '');
const ext = (p) => (String(p).match(/\.([^.\\/]+)$/) || [, ''])[1].toLowerCase();

async function paths() { if (!App._paths && N()) App._paths = await N().paths(); return App._paths || { home: '', documents: '', sep: '/' }; }
async function defaultDir(sub) { if (App.project) return join(App.project, sub); const p = await paths(); return join(p.documents, 'inca', 'projects', 'default', sub); }

function addRecent(p) { const r = App.prefs.recent || []; App.prefs.recent = [p, ...r.filter(x => x !== p)].slice(0, App.prefs.recentCount || 10); App.savePrefs(); App.dirty('menus'); }

// ------------------------------------------------------------------ scenes
async function askSave() {
  if (!App.modified) return true;
  const r = await confirmDialog('Warning: Scene Not Saved', `Save changes to ${App.sceneFile || 'untitled scene'}?`, ['Save', "Don't Save", 'Cancel']);
  if (r === 2) return false;
  if (r === 0) return !!(await saveScene());
  return true;
}
export function resetScene() {
  S.clearScene(); clearShading(); initDefaultMaterials(); S.createStartupCameras();
  Object.assign(App.time, { current: 1, start: 1, end: 120, animStart: 1, animEnd: 200, playing: false });
  App.sceneFile = null; App.sceneName = 'untitled'; App.modified = false; App._lastHidden = null;
  Undo.clear(); App.emit('sceneLoaded'); App.emit('selectionChanged');
  App.dirty('outliner', 'channels', 'attr', 'layers', 'timeline', 'title', 'hypershade');
}
async function newScene({ force = false } = {}) {
  if (!force && !(await askSave())) return;
  resetScene(); echo('file -f -new;');
}
async function openScene() {
  if (!(await askSave())) return;
  const p = await N()?.openDialog({ title: 'Open', defaultPath: await defaultDir('scenes'), filters: [{ name: 'Inca Scenes', extensions: ['inca'] }, { name: 'All Files', extensions: ['*'] }] });
  if (p) return openPath(p, { skipAsk: true });
}
export async function openPath(p, { skipAsk = false } = {}) {
  if (!p) return;
  const e = ext(p);
  if (e !== 'inca') return importPath(p);
  if (!skipAsk && !(await askSave())) return;
  try {
    const txt = await N().readText(p); const data = JSON.parse(txt);
    if (data.format !== 'inca-scene') throw new Error('Not an Inca scene file');
    Undo.suspend(() => S.loadScene(data));
    App.sceneFile = p; App.sceneName = stem(p); App.modified = false; Undo.clear();
    if (data.project && !App.project) { /* keep current project */ }
    addRecent(p); App.setTime(App.time.current);
    App.viewports.forEach(v => v.needsRender = true);
    echo(`file -f -options "v=0;" -ignoreVersion -o "${p}";`); App.emit('result', p);
    App.dirty('title');
  } catch (err) { App.emit('error', `// Error: Could not open ${p}: ${err.message}`); }
}
async function saveScene() {
  if (!App.sceneFile) return saveSceneAs();
  return writeScene(App.sceneFile);
}
async function saveSceneAs() {
  const p = await N()?.saveDialog({ title: 'Save As', defaultPath: join(await defaultDir('scenes'), (App.sceneName === 'untitled' ? 'untitled' : App.sceneName) + '.inca'), filters: [{ name: 'Inca Scene', extensions: ['inca'] }] });
  if (!p) return null;
  return writeScene(/\.inca$/i.test(p) ? p : p + '.inca');
}
async function writeScene(p) {
  if (!N()) { const blob = new Blob([JSON.stringify(S.serializeScene())], { type: 'application/json' }); const a = h('a', { href: URL.createObjectURL(blob), download: stem(p) + '.inca' }); a.click(); return p; }
  try {
    const data = S.serializeScene();
    if (App.prefs.incrementalSave && await N().exists(p)) { const bdir = join(dirname(p), 'incrementalSave', basename(p)); const files = await N().readdir(bdir); const n = files.length + 1; await N().writeText(join(bdir, `${stem(p)}.${String(n).padStart(4, '0')}.inca`), await N().readText(p)); }
    await N().writeText(p, JSON.stringify(data));
    App.sceneFile = p; App.sceneName = stem(p); App.modified = false; addRecent(p);
    echo(`file -save; // ${p}`); App.dirty('title'); App.help('Saved ' + p);
    return p;
  } catch (err) { App.emit('error', '// Error: Save failed: ' + err.message); return null; }
}
C('newScene', 'New Scene', (a) => newScene(a), { icon: 'newScene', noRepeat: true });
C('openScene', 'Open Scene...', () => openScene(), { icon: 'openScene', noRepeat: true });
C('saveScene', 'Save Scene', () => saveScene(), { icon: 'saveScene', noRepeat: true });
C('saveSceneAs', 'Save Scene As...', () => saveSceneAs(), { noRepeat: true });
C('incrementAndSave', 'Increment & Save', async () => {
  if (!App.sceneFile) return saveSceneAs();
  const m = App.sceneFile.match(/^(.*?)(\d+)?\.inca$/i); const base = m[1].replace(/[._]$/, ''); const n = m[2] ? parseInt(m[2], 10) + 1 : 1;
  return writeScene(`${base}.${String(n).padStart(4, '0')}.inca`);
}, { noRepeat: true });
C('quit', 'Exit', async () => { if (!(await askSave())) return; if (N()) N().quit(); else window.close(); }, { noRepeat: true });
C('openRecent', 'Recent File', (a) => openPath(a.path), { noRepeat: true });
C('revertScene', 'Revert to Saved', async () => { if (!App.sceneFile) return; const r = await confirmDialog('Revert', 'Discard changes and reload ' + App.sceneFile + '?', ['Revert', 'Cancel']); if (r === 0) openPath(App.sceneFile, { skipAsk: true }); }, { noRepeat: true });

// ------------------------------------------------------------------ projects
export async function setProject(p, { quiet = false } = {}) {
  if (!p) return;
  if (N() && !(await N().exists(p))) { if (!quiet) App.emit('warning', '// Warning: Project folder does not exist: ' + p); return; }
  App.project = p.replace(/[\\/]+$/, '');
  App.prefs.lastProject = App.project;
  App.prefs.recentProjects = [App.project, ...(App.prefs.recentProjects || []).filter(x => x !== App.project)].slice(0, 8);
  App.savePrefs();
  echo(`setProject "${App.project}";`);
  App.dirty('title', 'menus');
  if (!quiet) toast('Project set to ' + App.project);
}
async function createProject(root, name, dirs = PROJECT_DIRS) {
  const p = join(root, name);
  for (const d of dirs) await N().mkdir(join(p, d));
  const ws = { format: 'inca-workspace', version: App.version, name, fileRules: Object.fromEntries(dirs.map(d => [d.replace(/\W+/g, '_'), d])) };
  await N().writeText(join(p, 'workspace.inca'), JSON.stringify(ws, null, 2));
  // Maya-compatible workspace definition
  await N().writeText(join(p, 'workspace.mel'), '//Inca workspace definition\n' + dirs.map(d => `workspace -fr "${d.replace(/\W+/g, '')}" "${d}";`).join('\n') + '\n');
  return p;
}
function projectWindow() {
  const win = new FloatWin('projectWindow', 'Project Window', { w: 560, h: 520 });
  if (win.reused) return;
  (async () => {
    const pp = await paths();
    let root = App.project ? dirname(App.project) : join(pp.documents, 'inca', 'projects');
    const name = h('input', { class: 'wide', value: App.project ? basename(App.project) : 'new_project' });
    const loc = h('input', { class: 'wide', value: root });
    const rows = PROJECT_DIRS.map(d => { const cb = h('input', { type: 'checkbox', checked: true }); return { d, cb, row: h('div', { class: 'dlg-row' }, h('label', { text: d.split('/').pop() }), cb, h('span', { class: 'dim', text: d })) }; });
    win.body.append(h('div', { class: 'dlg-body' },
      h('div', { class: 'dlg-row' }, h('label', { text: 'Current Project:' }), name),
      h('div', { class: 'dlg-row' }, h('label', { text: 'Location:' }), loc, h('button', { text: '📁', title: 'Browse', onclick: async () => { const d = await N()?.openDialog({ directory: true, defaultPath: loc.value }); if (d) loc.value = d; } })),
      h('div', { class: 'dlg-sect', text: 'Primary Project Locations' }), ...rows.map(r => r.row)),
      h('div', { class: 'dlg-buttons' },
        h('button', { text: 'Accept', onclick: async () => { if (!N()) return toast('Projects need the desktop app'); const p = await createProject(loc.value, name.value.trim() || 'new_project', rows.filter(r => r.cb.checked).map(r => r.d)); await setProject(p); win.close(); } }),
        h('button', { text: 'Cancel', onclick: () => win.close() })));
  })();
}
C('projectWindow', 'Project Window', () => projectWindow(), { noRepeat: true });
C('setProject', 'Set Project...', async () => { const p = await N()?.openDialog({ title: 'Set Project', directory: true, defaultPath: App.project || (await paths()).documents }); if (p) setProject(p); }, { noRepeat: true });

// ------------------------------------------------------------------ import
function threeToPoly(geom) {
  const g = geom.index ? geom : geom; const pos = g.attributes.position; const uv = g.attributes.uv; const idx = g.index;
  const map = new Map(); const v = []; const remap = [];
  for (let i = 0; i < pos.count; i++) {
    const x = pos.getX(i), y = pos.getY(i), z = pos.getZ(i);
    const k = `${x.toFixed(5)},${y.toFixed(5)},${z.toFixed(5)}`;
    let j = map.get(k); if (j === undefined) { j = v.length; v.push([x, y, z]); map.set(k, j); }
    remap.push(j);
  }
  const f = [], fuv = [];
  const tri = (a, b, c) => { const A = remap[a], B = remap[b], Cc = remap[c]; if (A === B || B === Cc || A === Cc) return; f.push([A, B, Cc]); fuv.push(uv ? [[uv.getX(a), uv.getY(a)], [uv.getX(b), uv.getY(b)], [uv.getX(c), uv.getY(c)]] : null); };
  const groups = g.groups && g.groups.length ? g.groups : [{ start: 0, count: idx ? idx.count : pos.count, materialIndex: 0 }];
  const fmat = [];
  for (const gr of groups) for (let i = gr.start; i + 2 < gr.start + gr.count; i += 3) { const n0 = f.length; if (idx) tri(idx.getX(i), idx.getX(i + 1), idx.getX(i + 2)); else tri(i, i + 1, i + 2); if (f.length > n0) fmat.push(gr.materialIndex || 0); }
  const pm = new PolyMesh(v, f, uv ? fuv : null);
  pm._fmat = fmat;
  return pm;
}
async function imageToDataURL(img) {
  try { const cv = document.createElement('canvas'); cv.width = img.width; cv.height = img.height; cv.getContext('2d').drawImage(img, 0, 0); return cv.toDataURL('image/png'); } catch { return ''; }
}
async function importMaterial(m, cache) {
  if (!m) return App.defaultMatId;
  if (cache.has(m)) return cache.get(m);
  const attrs = { baseColor: m.color ? [m.color.r, m.color.g, m.color.b] : [0.8, 0.8, 0.8], metalness: m.metalness ?? 0, specularRoughness: m.roughness ?? 0.5, base: 1 };
  if (m.emissive) { attrs.emissionColor = [m.emissive.r, m.emissive.g, m.emissive.b]; attrs.emission = (m.emissive.r + m.emissive.g + m.emissive.b) > 0 ? (m.emissiveIntensity ?? 1) : 0; }
  if (m.opacity !== undefined && m.opacity < 1) attrs.opacity = [m.opacity, m.opacity, m.opacity];
  const rec = createMaterial('standardSurface', (m.name || 'importedMaterial').replace(/[^A-Za-z0-9_]/g, '_') || 'importedMaterial', { attrs });
  if (m.map && m.map.image) { const url = m.map.userData?.path || await imageToDataURL(m.map.image); if (url) { const t = createTexture('file', null, { attrs: { fileTextureName: url } }); rec.maps.baseColor = t.id; } }
  updateMaterial(rec);
  cache.set(m, rec.id); return rec.id;
}
async function importObject3D(root, parent, cache) {
  const made = [];
  const visit = async (obj, par) => {
    let node = null;
    if (obj.isMesh && obj.geometry) {
      const pm = threeToPoly(obj.geometry);
      const mats = Array.isArray(obj.material) ? obj.material : [obj.material];
      const ids = []; for (const m of mats) ids.push(await importMaterial(m, cache));
      if (ids.length > 1) pm.fm = pm._fmat.map(i => (ids[i] && ids[i] !== ids[0]) ? ids[i] : null);
      delete pm._fmat;
      node = S.createMesh(null, {}, { name: (obj.name || 'polySurface1').replace(/[^A-Za-z0-9_]/g, '_') || 'polySurface1', mesh: pm, material: ids[0], parent: par });
      S.bakeMesh(node, pm);
    } else if (obj.isLight) { return; }
    else if (obj.children.length || obj === root) node = S.createGroup((obj.name || 'group1').replace(/[^A-Za-z0-9_]/g, '_') || 'group1', { parent: par });
    if (!node) return;
    obj.updateMatrix(); S.setLocalMatrix(node, obj.matrix.clone());
    made.push(node);
    for (const c of obj.children) await visit(c, node);
  };
  await visit(root, parent);
  return made;
}
function parseOBJ(text, name) {
  const v = [], vt = []; const objs = []; let cur = null; let curMat = null; const mats = new Set();
  const start = (n) => { cur = { name: n || name, f: [], fuv: [], fm: [] }; objs.push(cur); };
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim(); if (!line || line[0] === '#') continue;
    const p = line.split(/\s+/);
    if (p[0] === 'v') v.push([+p[1], +p[2], +p[3]]);
    else if (p[0] === 'vt') vt.push([+p[1], +p[2]]);
    else if (p[0] === 'o' || p[0] === 'g') { if (!cur || cur.f.length) start(p.slice(1).join('_')); else cur.name = p.slice(1).join('_') || cur.name; }
    else if (p[0] === 'usemtl') { curMat = p[1]; mats.add(curMat); }
    else if (p[0] === 'f') {
      if (!cur) start(name);
      const vs = [], us = []; let hasUV = true;
      for (const t of p.slice(1)) { const [a, b] = t.split('/'); let i = parseInt(a, 10); i = i < 0 ? v.length + i : i - 1; vs.push(i); if (b) { let j = parseInt(b, 10); j = j < 0 ? vt.length + j : j - 1; us.push(vt[j] ? vt[j].slice() : [0, 0]); } else hasUV = false; }
      cur.f.push(vs); cur.fuv.push(hasUV ? us : null); cur.fm.push(curMat);
    }
  }
  return { objs: objs.filter(o => o.f.length).map(o => {
    const used = new Map(); const nv = [];
    const f = o.f.map(F => F.map(i => { if (!used.has(i)) { used.set(i, nv.length); nv.push(v[i] ? v[i].slice() : [0, 0, 0]); } return used.get(i); }));
    return { name: o.name, pm: new PolyMesh(nv, f, o.fuv.some(Boolean) ? o.fuv : null), fm: o.fm };
  }), mats: [...mats] };
}
export async function importPath(p) {
  const e = ext(p); Undo.checkpoint('import');
  const made = [];
  try {
    if (e === 'obj') {
      const text = await N().readText(p); const { objs, mats } = parseOBJ(text, stem(p));
      const matIds = {}; for (const m of mats) { const r = createMaterial('blinn', m.replace(/[^A-Za-z0-9_]/g, '_')); matIds[m] = r.id; }
      // read .mtl colours if present
      try { const mtlLine = text.match(/^mtllib\s+(.+)$/m); if (mtlLine) { const mtl = await N().readText(join(dirname(p), mtlLine[1].trim())); let curM = null; for (const l of mtl.split(/\r?\n/)) { const q = l.trim().split(/\s+/); if (q[0] === 'newmtl') curM = q[1]; else if (q[0] === 'Kd' && curM && matIds[curM]) { const r = App.mats.get(matIds[curM]); r.attrs.color = [+q[1], +q[2], +q[3]]; r.attrs.diffuse = 1; updateMaterial(r); } else if (q[0] === 'map_Kd' && curM && matIds[curM]) { const t = createTexture('file', null, { attrs: { fileTextureName: join(dirname(p), q.slice(1).join(' ')) } }); App.mats.get(matIds[curM]).maps.color = t.id; updateMaterial(App.mats.get(matIds[curM])); } } } } catch { }
      for (const o of objs) {
        const first = o.fm.find(Boolean); const base = first ? matIds[first] : App.defaultMatId;
        if (o.fm.some(m => m && matIds[m] !== base)) o.pm.fm = o.fm.map(m => m && matIds[m] !== base ? matIds[m] : null);
        const n = S.createMesh(null, {}, { name: o.name.replace(/[^A-Za-z0-9_]/g, '_') || 'polySurface1', mesh: o.pm, material: base }); S.bakeMesh(n, o.pm); made.push(n);
      }
    } else if (e === 'gltf' || e === 'glb' || e === 'fbx' || e === 'stl') {
      const buf = await N().readBinary(p); let root;
      if (e === 'stl') { const { STLLoader } = await import('three/addons/loaders/STLLoader.js'); const g = new STLLoader().parse(buf); root = new THREE.Mesh(g, new THREE.MeshStandardMaterial()); root.name = stem(p); }
      else if (e === 'fbx') { const { FBXLoader } = await import('three/addons/loaders/FBXLoader.js'); root = new FBXLoader().parse(buf, dirname(p) + '/'); root.name = stem(p); root.scale.multiplyScalar(1); }
      else {
        const { GLTFLoader } = await import('three/addons/loaders/GLTFLoader.js');
        const data = e === 'gltf' ? new TextDecoder().decode(buf) : buf;
        const gltf = await new Promise((res, rej) => new GLTFLoader().parse(data, 'file://' + dirname(p).replace(/\\/g, '/') + '/', res, rej));
        root = gltf.scene; root.name = stem(p);
        if (gltf.animations?.length) App.emit('warning', '// Warning: glTF animations are not imported yet');
      }
      made.push(...await importObject3D(root, null, new Map()));
    } else if (e === 'inca') {
      const data = JSON.parse(await N().readText(p)); const map = {};
      for (const d of data.nodes) { if (d.cam?.startup) continue; const par = d.parent ? map[d.parent] : null; if (d.parent && !par) continue; const n = S.deserializeNode(d, par, { fresh: true }); map[d.id] = n; if (!d.parent) made.push(n); }
    } else throw new Error('Unsupported file type .' + e);
    Sel.select(made.filter(m => !S.dagParent(m) || !made.includes(S.dagParent(m))), 'replace', { echo: false });
    echo(`file -import -type "${e.toUpperCase()}" -ignoreVersion -ra true -mergeNamespacesOnClash false -pr "${p}";`);
    App.dirty('outliner', 'hypershade'); App.viewports.forEach(v => v.needsRender = true);
    toast(`Imported ${made.length} node(s) from ${basename(p)}`);
  } catch (err) { console.error(err); App.emit('error', `// Error: Import failed: ${err.message}`); }
}
C('importFile', 'Import...', async () => { const p = await N()?.openDialog({ title: 'Import', defaultPath: await defaultDir('assets'), filters: [{ name: 'All Supported', extensions: ['obj', 'fbx', 'gltf', 'glb', 'stl', 'inca'] }, { name: 'OBJ', extensions: ['obj'] }, { name: 'FBX', extensions: ['fbx'] }, { name: 'glTF', extensions: ['gltf', 'glb'] }, { name: 'STL', extensions: ['stl'] }, { name: 'Inca Scene', extensions: ['inca'] }] }); if (p) importPath(p); }, { noRepeat: true });

// ------------------------------------------------------------------ export
function exportMeshes(selOnly) {
  const roots = selOnly ? App.sel : S.dagChildren(App.world).filter(o => !o.inca.startup);
  const out = []; for (const r of roots) r.traverse(c => { if (c.inca && c.inca.kind === 'mesh' && c.visible && !out.includes(c)) out.push(c); });
  return out;
}
function writeOBJ(meshes, mtlName) {
  let s = `# Inca ${App.version} OBJ export\n` + (mtlName ? `mtllib ${mtlName}\n` : ''); let vo = 1, to = 1; const mats = new Set();
  const v = new THREE.Vector3();
  for (const o of meshes) {
    const pm = o.inca.mesh; o.updateWorldMatrix(true, false); const M = o.matrixWorld;
    s += `o ${o.inca.name}\n`;
    for (const p of pm.v) { v.set(p[0], p[1], p[2]).applyMatrix4(M); s += `v ${+v.x.toFixed(6)} ${+v.y.toFixed(6)} ${+v.z.toFixed(6)}\n`; }
    const uvIdx = []; if (pm.uv) pm.f.forEach((f, fi) => { const u = pm.uv[fi]; uvIdx.push(u ? u.map(c => { s += `vt ${+c[0].toFixed(6)} ${+c[1].toFixed(6)}\n`; return to++; }) : null); });
    let lastM = null;
    pm.f.forEach((f, fi) => {
      const mid = (pm.fm && pm.fm[fi]) || o.inca.material; const mn = App.mats.get(mid)?.name || 'lambert1'; mats.add(mid);
      if (mn !== lastM) { s += `usemtl ${mn}\n`; lastM = mn; }
      const u = uvIdx[fi]; s += 'f ' + f.map((vi, k) => (vi + vo) + (u ? '/' + u[k] : '')).join(' ') + '\n';
    });
    vo += pm.v.length;
  }
  let mtl = '# Inca MTL\n';
  for (const id of mats) { const m = App.mats.get(id); if (!m) continue; const c = m.attrs.color || m.attrs.baseColor || [0.5, 0.5, 0.5]; mtl += `newmtl ${m.name}\nKd ${c.map(x => x.toFixed(4)).join(' ')}\nKa 0 0 0\nKs 0.2 0.2 0.2\nNs 20\nd 1\nillum 2\n\n`; }
  return { obj: s, mtl };
}
function buildExportScene(meshes) {
  const scene = new THREE.Scene();
  for (const o of meshes) {
    o.updateWorldMatrix(true, false);
    const src = o.userData.mesh; const g = src.geometry.clone();
    const mats = (src.userData.matsT || [src.material]).map(m => { const c = m.clone(); c.name = m.userData?.rec?.name || 'material'; return c; });
    const mesh = new THREE.Mesh(g, mats.length > 1 ? mats : mats[0]); mesh.name = o.inca.name;
    mesh.matrixAutoUpdate = false; mesh.matrix.copy(o.matrixWorld); mesh.matrix.decompose(mesh.position, mesh.quaternion, mesh.scale); mesh.matrixAutoUpdate = true;
    scene.add(mesh);
  }
  return scene;
}
async function exportTo(selOnly) {
  const meshes = exportMeshes(selOnly);
  if (!meshes.length) return App.emit('warning', '// Warning: Nothing to export');
  const p = await N()?.saveDialog({ title: selOnly ? 'Export Selection' : 'Export All', defaultPath: join(await defaultDir('assets'), (App.sceneName || 'untitled') + '.obj'), filters: [{ name: 'OBJ', extensions: ['obj'] }, { name: 'glTF Binary', extensions: ['glb'] }, { name: 'glTF', extensions: ['gltf'] }, { name: 'STL', extensions: ['stl'] }, { name: 'Inca Scene', extensions: ['inca'] }] });
  if (!p) return;
  const e = ext(p) || 'obj';
  try {
    if (e === 'obj') { const mtlName = stem(p) + '.mtl'; const { obj, mtl } = writeOBJ(meshes, mtlName); await N().writeText(p, obj); await N().writeText(join(dirname(p), mtlName), mtl); }
    else if (e === 'stl') { const { STLExporter } = await import('three/addons/exporters/STLExporter.js'); const r = new STLExporter().parse(buildExportScene(meshes), { binary: true }); await N().writeBinary(p, r.buffer.slice(r.byteOffset, r.byteOffset + r.byteLength)); }
    else if (e === 'gltf' || e === 'glb') { const { GLTFExporter } = await import('three/addons/exporters/GLTFExporter.js'); const r = await new GLTFExporter().parseAsync(buildExportScene(meshes), { binary: e === 'glb' }); if (e === 'glb') await N().writeBinary(p, r); else await N().writeText(p, JSON.stringify(r)); }
    else if (e === 'inca') { const data = S.serializeScene(); if (selOnly) { const keep = new Set(); for (const o of App.sel) o.traverse(c => c.inca && keep.add(c.inca.id)); data.nodes = data.nodes.filter(d => keep.has(d.id) || d.cam?.startup).map(d => keep.has(d.parent) || !d.parent ? d : { ...d, parent: null }); } await N().writeText(p, JSON.stringify(data)); }
    else throw new Error('Unsupported export type .' + e);
    echo(`file -force -options "groups=1;ptgroups=1;materials=1;smoothing=1;normals=1" -typ "${e.toUpperCase()}" -pr ${selOnly ? '-es' : '-ea'} "${p}";`);
    toast('Exported ' + basename(p));
  } catch (err) { console.error(err); App.emit('error', '// Error: Export failed: ' + err.message); }
}
C('exportAll', 'Export All...', () => exportTo(false), { noRepeat: true });
C('exportSelection', 'Export Selection...', () => { if (!App.sel.length) return App.emit('warning', '// Warning: Nothing selected'); exportTo(true); }, { noRepeat: true });

// ------------------------------------------------------------------ autosave
let autoT = 0;
function scheduleAutosave() {
  clearInterval(autoT);
  const a = App.prefs?.autosave; if (!a || !a.on || !N()) return;
  autoT = setInterval(async () => {
    if (!App.modified) return;
    const dir = await defaultDir('autosave'); const p = join(dir, `${App.sceneName}.autosave.inca`);
    try { await N().writeText(p, JSON.stringify(S.serializeScene())); App.help('Autosaved ' + p); } catch { }
  }, Math.max(1, a.interval || 10) * 60000);
}
App.on('uiReady', scheduleAutosave); App.on('prefsChanged', scheduleAutosave);

App.io = { openPath, importPath, setProject, newScene, saveScene, saveSceneAs, resetScene, projectDir: defaultDir, join, basename, parseOBJ, writeOBJ, threeToPoly };
