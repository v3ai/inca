// Inca — Attribute Editor, embeddable attribute panels, Tool Settings and Modeling Toolkit
import { App } from '../core/app.js';
import * as S from '../core/scene.js';
import { Undo } from '../core/undo.js';
import { Sel } from '../core/selection.js';
import { OPS, niceName } from '../core/ops.js';
import { MAT_TYPES, TEX_TYPES, MAPPABLE, updateMaterial, swatch, createTexture, getThreeTexture } from '../core/materials.js';
import { Manip, placeProxy } from '../core/manip.js';
import { h, menuBar, showMenu, fmt, iconBtn, colorToCss, colorPicker, numField } from './dom.js';
import { icon } from './icons.js';
import { registerPanel } from './docks.js';

const ck = (l) => Undo.checkpoint(l);
const D2R = Math.PI / 180;

// ------------------------------------------------------------------ attribute setters
function setVal(rec, key, v, { silent = false } = {}) {
  if (rec.isObject3D || rec.kind === 'history') { S.setAttr(rec, key, v, { silent: true, force: false }); if (rec.isObject3D) { placeProxy(); App.anim.autoKey(rec, [key]); } }
  else if (rec.kind === 'material') { rec.attrs[key] = Array.isArray(v) ? v.slice() : v; updateMaterial(rec); }
  else if (rec.kind === 'texture') { rec.attrs[key] = Array.isArray(v) ? v.slice() : v; touchTexture(rec); }
  else if (rec.attrs) rec.attrs[key] = v;
  App.modified = true;
  if (!silent) App.dirty('channels', 'hypershade', 'title');
  App.requestRender();
}
export function touchTexture(t) { t._swatch = null; getThreeTexture(t.id); for (const m of App.mats.values()) if (Object.values(m.maps || {}).includes(t.id)) updateMaterial(m); App.emit('materialChanged', t); }
function getVal(rec, key) { if (rec.isObject3D || rec.kind === 'history') return S.getAttr(rec, key); return rec.attrs ? rec.attrs[key] : undefined; }

// ------------------------------------------------------------------ field widgets
// spec: {key,label,type:'float'|'int'|'bool'|'enum'|'color'|'vec3'|'text'|'file'|'info'|'ramp', min,max, smin,smax, options, values, get, set, map}
function row(label, ...kids) { return h('div', { class: 'ae-row' }, h('div', { class: 'lbl', text: label, title: label }), ...kids); }
function fieldFor(rec, spec, ctx) {
  const get = spec.get || (() => getVal(rec, spec.key));
  const set = spec.set || ((v, o) => setVal(rec, spec.key, v, o));
  const label = spec.label || niceName(spec.key);
  let el, upd;
  const startEdit = () => ck('set ' + label);
  if (spec.type === 'info') { const s = h('span', { class: 'dim' }); upd = () => { s.textContent = String(get() ?? ''); }; el = row(label, s); }
  else if (spec.type === 'bool') {
    const cb = h('input', { type: 'checkbox' }); cb.addEventListener('change', () => { startEdit(); set(cb.checked); ctx.refresh(); });
    upd = () => { cb.checked = !!get(); }; el = h('div', { class: 'ae-row' }, h('div', { class: 'lbl' }), h('label', { style: { display: 'flex', alignItems: 'center' } }, cb, label));
  } else if (spec.type === 'enum') {
    const sel = h('select', {}, (spec.options || []).map((o, i) => h('option', { value: i, text: o })));
    sel.addEventListener('change', () => { startEdit(); set(spec.values ? spec.values[sel.selectedIndex] : sel.selectedIndex); ctx.refresh(true); });
    upd = () => { const v = get(); sel.selectedIndex = spec.values ? Math.max(0, spec.values.indexOf(v)) : (+v || 0); }; el = row(label, sel);
  } else if (spec.type === 'text' || spec.type === 'file') {
    const inp = h('input', { class: 'wide' }); inp.addEventListener('keydown', e => e.stopPropagation());
    inp.addEventListener('change', () => { startEdit(); set(inp.value); ctx.refresh(); });
    const kids = [inp];
    if (spec.type === 'file') kids.push(h('div', { class: 'mapbtn', title: 'Browse', html: icon('openScene'), onclick: async () => {
      const N = window.incaNative; if (!N) return;
      const dir = App.project ? App.project + '/sourceimages' : undefined;
      const p = await N.openDialog({ title: 'Open', defaultPath: dir, filters: spec.filters || [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'bmp', 'tga', 'tif', 'tiff', 'exr', 'hdr', 'webp', 'gif'] }, { name: 'All Files', extensions: ['*'] }] });
      if (p) { let v = p; if (App.project && p.replace(/\\/g, '/').startsWith(App.project.replace(/\\/g, '/') + '/')) v = p.replace(/\\/g, '/').slice(App.project.length + 1); startEdit(); set(v); ctx.refresh(); }
    } }));
    upd = () => { if (document.activeElement !== inp) inp.value = get() ?? ''; }; el = row(label, ...kids);
  } else if (spec.type === 'vec3') {
    const fs = [0, 1, 2].map(i => numField(0, (n, live) => { const v = (get() || [0, 0, 0]).slice(); v[i] = n; set(v, { silent: live }); if (!live) ctx.refresh(); }, { step: spec.step || 0.1, onStart: startEdit }));
    fs.forEach(f => { f.addEventListener('focus', startEdit, { once: false }); });
    upd = () => { const v = get() || [0, 0, 0]; fs.forEach((f, i) => f.set(v[i])); }; el = row(label, ...fs);
  } else if (spec.type === 'color') {
    const sw = h('div', { class: 'colorsw' });
    const sl = h('input', { type: 'range', min: 0, max: 1000 });
    const mb = mapButton(rec, spec, ctx);
    sw.addEventListener('click', (e) => { startEdit(); const r = sw.getBoundingClientRect(); colorPicker(r.left, r.bottom + 2, get() || [0, 0, 0], (c) => { set(c, { silent: true }); upd(); }, () => ctx.refresh()); });
    sl.addEventListener('pointerdown', startEdit);
    sl.addEventListener('input', () => { const v = get() || [0, 0, 0]; const mx = Math.max(...v, 1e-6); const t = sl.value / 1000; const nv = mx > 1e-5 ? v.map(c => c / mx * t) : [t, t, t]; set(nv, { silent: true }); sw.style.background = colorToCss(nv); });
    sl.addEventListener('change', () => ctx.refresh());
    upd = () => { const v = get() || [0, 0, 0]; sw.style.background = colorToCss(v); sl.value = Math.max(...v) * 1000; mb?._upd(); };
    el = row(label, sw, sl, mb);
  } else { // float / int with slider
    const isInt = spec.type === 'int';
    const nf = numField(0, (n, live) => { if (spec.min !== undefined) n = Math.max(spec.min, n); if (spec.max !== undefined) n = Math.min(spec.max, n); set(n, { silent: live }); sl.value = n; if (!live) ctx.refresh(); }, { step: spec.step || (isInt ? 1 : 0.01), int: isInt, onStart: startEdit });
    nf.addEventListener('focus', startEdit);
    const smin = spec.smin ?? spec.min ?? 0, smax = spec.smax ?? spec.max ?? (isInt ? 20 : 1);
    const sl = h('input', { type: 'range', min: smin, max: smax, step: isInt ? 1 : (smax - smin) / 1000 });
    sl.addEventListener('pointerdown', startEdit);
    sl.addEventListener('input', () => { const n = isInt ? Math.round(+sl.value) : +sl.value; nf.set(n); set(n, { silent: true }); });
    sl.addEventListener('change', () => ctx.refresh());
    const mb = mapButton(rec, spec, ctx);
    upd = () => { const v = +get() || 0; nf.set(v); sl.value = v; mb?._upd(); };
    el = row(label, nf, spec.noSlider ? null : sl, mb);
  }
  upd(); el._upd = upd;
  return el;
}
// texture map button for material attributes
function mapButton(rec, spec, ctx) {
  if (rec.kind !== 'material' || !(MAPPABLE[rec.type] || []).includes(spec.key)) return null;
  const b = h('div', { class: 'mapbtn', title: 'Map a texture', html: icon('mapBtn') });
  b._upd = () => { const t = rec.maps[spec.key]; b.classList.toggle('on', !!t); b.innerHTML = t ? icon('inputLink') : icon('mapBtn'); b.title = t ? `Connected to ${App.texs.get(t)?.name} (click to edit, right-click to break)` : 'Map a texture'; };
  b.addEventListener('click', (e) => {
    const t = rec.maps[spec.key];
    if (t && App.texs.get(t)) { ctx.openRec?.(App.texs.get(t)); return; }
    showMenu([{ section: '2D Textures' }, ...Object.entries(TEX_TYPES).map(([k, d]) => ({ label: d.label, icon: k === 'file' ? 'fileTex' : k === 'grid' ? 'gridTex' : k, fn: () => {
      const tex = App.ui.createTextureNode ? App.ui.createTextureNode(k) : (ck('create texture'), createTexture(k));
      rec.maps[spec.key] = tex.id; updateMaterial(rec); App.emit('echo', `connectAttr -force ${tex.name}.outColor ${rec.name}.${spec.key};`); App.dirty('hypershade'); ctx.refresh(true); if (k === 'file') ctx.openRec?.(tex);
    } })), ...(App.texs.size ? [{ section: 'Existing' }, ...[...App.texs.values()].map(t => ({ label: t.name, fn: () => { ck('connect'); rec.maps[spec.key] = t.id; updateMaterial(rec); App.dirty('hypershade'); ctx.refresh(true); } }))] : [])], e.clientX, e.clientY);
  });
  b.addEventListener('contextmenu', (e) => { e.preventDefault(); if (!rec.maps[spec.key]) return; showMenu([{ label: 'Break Connection', fn: () => { ck('break connection'); delete rec.maps[spec.key]; updateMaterial(rec); App.dirty('hypershade'); ctx.refresh(true); } }], e.clientX, e.clientY); });
  return b;
}
function frame(title, children, { closed = false, key = null } = {}) {
  const k = key || title; const st = App.prefs.opt.aeFrames || (App.prefs.opt.aeFrames = {});
  const isClosed = st[k] ?? closed;
  const f = h('div', { class: 'frame' + (isClosed ? ' closed' : '') });
  const head = h('div', { class: 'frame-head' }, h('span', { class: 'tri', text: isClosed ? '▶' : '▼' }), title);
  head.addEventListener('click', () => { f.classList.toggle('closed'); const c = f.classList.contains('closed'); head.firstChild.textContent = c ? '▶' : '▼'; st[k] = c; App.savePrefs(); });
  f.append(head, h('div', { class: 'frame-body' }, children));
  return f;
}

// ------------------------------------------------------------------ attribute layouts per node type
const MAT_LAYOUT = {
  lambert: [['Common Material Attributes', ['color:color', 'transparency:color', 'ambientColor:color', 'incandescence:color', 'bump:float:0:2', 'diffuse:float:0:1']]],
  blinn: [['Common Material Attributes', ['color:color', 'transparency:color', 'ambientColor:color', 'incandescence:color', 'bump:float:0:2', 'diffuse:float:0:1']], ['Specular Shading', ['eccentricity:float:0:1', 'specularRollOff:float:0:1', 'specularColor:color', 'reflectivity:float:0:1']]],
  phong: [['Common Material Attributes', ['color:color', 'transparency:color', 'ambientColor:color', 'incandescence:color', 'bump:float:0:2', 'diffuse:float:0:1']], ['Specular Shading', ['cosinePower:float:2:100', 'specularColor:color', 'reflectivity:float:0:1']]],
  standardSurface: [['Base', ['base:float:0:1', 'baseColor:color', 'diffuseRoughness:float:0:1', 'metalness:float:0:1']], ['Specular', ['specular:float:0:1', 'specularColor:color', 'specularRoughness:float:0:1', 'specularIOR:float:1:3']], ['Transmission', ['transmission:float:0:1', 'transmissionColor:color']], ['Coat', ['coat:float:0:1', 'coatRoughness:float:0:1']], ['Sheen', ['sheen:float:0:1', 'sheenColor:color']], ['Emission', ['emission:float:0:10', 'emissionColor:color']], ['Geometry', ['opacity:color', 'thinWalled:bool', 'bump:float:0:2']]],
  surfaceShader: [['Surface Shader Attributes', ['outColor:color', 'outTransparency:color']]],
};
const parseSpec = (s) => { const [key, type, a, b] = s.split(':'); const sp = { key, type }; if (a !== undefined) { sp.smin = +a; sp.smax = +b; sp.min = key === 'specularIOR' ? 1 : (type === 'float' ? Math.min(0, +a) : undefined); } return sp; };
function layoutFor(rec, ctx) {
  const F = (spec) => { const el = fieldFor(rec, spec, ctx); ctx.fields.push(el); return el; };
  const out = [];
  if (rec.kind === 'material') {
    out.push(row('Type', h('span', { text: MAT_TYPES[rec.type]?.label || rec.type, class: 'dim' })));
    const sw = h('img', { src: swatch(rec, 96), style: { width: '96px', height: '96px', margin: '4px 0 4px 124px', border: '1px solid #222' } }); ctx.fields.push({ _upd: () => { sw.src = swatch(rec, 96); } });
    out.push(sw);
    for (const [t, keys] of MAT_LAYOUT[rec.type] || []) out.push(frame(t, keys.map(k => F(parseSpec(k))), { key: rec.type + t }));
    const users = S.allDag().filter(o => o.inca.kind === 'mesh' && (o.inca.material === rec.id || (o.inca.mesh.fm || []).includes(rec.id)));
    out.push(frame('Shading Group', [row('Objects', h('span', { class: 'dim', text: users.map(o => o.inca.name).join(', ') || '(none)' })), h('div', { class: 'ae-row' }, h('div', { class: 'lbl' }), h('button', { text: 'Select Objects', onclick: () => Sel.select(users) }), h('button', { text: 'Assign to Selection', onclick: () => App.cmds.run('assignExisting', { id: rec.id }) }))], { closed: true, key: 'sg' }));
  } else if (rec.kind === 'texture') {
    const a = rec.attrs;
    const sw = h('img', { src: swatch(rec, 96), style: { width: '96px', height: '96px', margin: '4px 0 4px 124px', border: '1px solid #222' } }); ctx.fields.push({ _upd: () => { sw.src = swatch(rec, 96); } }); out.push(sw);
    const fields = [];
    if (rec.type === 'file') fields.push(F({ key: 'fileTextureName', label: 'Image Name', type: 'file' }), F({ key: 'colorSpace', label: 'Color Space', type: 'enum', options: ['sRGB', 'Raw'], values: ['sRGB', 'Raw'] }));
    for (const [k, v] of Object.entries(a)) {
      if (k === 'fileTextureName' || k === 'colorSpace') continue;
      if (k === 'colors') { fields.push(rampEditor(rec, ctx)); continue; }
      if (k === 'type' && rec.type === 'ramp') { fields.push(F({ key: 'type', label: 'Type', type: 'enum', options: ['V Ramp', 'U Ramp', 'Circular Ramp'], values: ['V Ramp', 'U Ramp', 'Circular Ramp'] })); continue; }
      if (Array.isArray(v)) fields.push(F({ key: k, type: 'color' }));
      else if (typeof v === 'number') fields.push(F({ key: k, type: /repeat|frequency/i.test(k) ? 'float' : 'float', smin: /rotate/i.test(k) ? 0 : 0, smax: /repeat/i.test(k) ? 16 : /rotate/i.test(k) ? 360 : /frequency/.test(k) ? 32 : 1 }));
      else if (typeof v === 'boolean') fields.push(F({ key: k, type: 'bool' }));
    }
    out.push(frame((TEX_TYPES[rec.type]?.label || rec.type) + ' Attributes', fields, { key: 'tex' + rec.type }));
    const users = [...App.mats.values()].filter(m => Object.values(m.maps || {}).includes(rec.id));
    out.push(frame('Connections', users.map(m => row('outColor →', h('a', { text: m.name + '.' + Object.keys(m.maps).find(k => m.maps[k] === rec.id), style: { color: '#9cd0f5', cursor: 'pointer' }, onclick: () => ctx.openRec?.(m) }))), { key: 'texConn' }));
  } else if (rec.kind === 'history') {
    const op = OPS[rec.type];
    out.push(frame(niceName(rec.type.replace(/^poly/, 'Poly ')) + ' History', (op?.channels || []).map(ch => F({ ...ch, key: ch.k, label: ch.label || niceName(ch.k), smin: ch.min ?? 0, smax: ch.max ?? (ch.type === 'int' ? 20 : 10) })), { key: 'hist' }));
    if (rec._error) out.push(h('div', { style: { color: '#f88', padding: '6px 10px' }, text: 'Error: ' + rec._error }));
  } else if (rec.isObject3D) {
    const inc = rec.inca;
    if (ctx.tab === 'transform') {
      out.push(frame('Transform Attributes', [
        F({ key: 'translate', type: 'vec3', get: () => inc.t, set: (v, o) => { inc.t = v.slice(); S.updateXform(rec); placeProxy(); App.anim.autoKey(rec, ['translateX', 'translateY', 'translateZ']); if (!o?.silent) App.dirty('channels'); App.requestRender(); } }),
        F({ key: 'rotate', type: 'vec3', step: 1, get: () => inc.r, set: (v, o) => { inc.r = v.slice(); S.updateXform(rec); placeProxy(); App.anim.autoKey(rec, ['rotateX', 'rotateY', 'rotateZ']); if (!o?.silent) App.dirty('channels'); App.requestRender(); } }),
        F({ key: 'scale', type: 'vec3', get: () => inc.s, set: (v, o) => { inc.s = v.slice(); S.updateXform(rec); placeProxy(); App.anim.autoKey(rec, ['scaleX', 'scaleY', 'scaleZ']); if (!o?.silent) App.dirty('channels'); App.requestRender(); } }),
        F({ key: 'rotateOrder', label: 'Rotate Order', type: 'enum', options: ['xyz', 'yzx', 'zxy', 'xzy', 'yxz', 'zyx'], values: ['xyz', 'yzx', 'zxy', 'xzy', 'yxz', 'zyx'] }),
      ], { key: 'xf' }));
      out.push(frame('Pivots', [F({ key: 'rotatePivot', label: 'Local Rotate Pivot', type: 'vec3', get: () => inc.p, set: (v) => { S.setPivotLocal(rec, v); placeProxy(); App.requestRender(); } }), F({ key: 'worldPivot', label: 'World Pivot', type: 'info', get: () => S.worldPivot(rec).toArray().map(x => fmt(x)).join('  ') })], { key: 'piv' }));
      out.push(frame('Display', [F({ key: 'visibility', label: 'Visibility', type: 'bool' }), F({ key: 'template', label: 'Template', type: 'bool', get: () => inc.template, set: (v) => { inc.template = v; if (v) Sel.select(App.sel.filter(o => o !== rec), 'replace', { echo: false }); App.requestRender(); } }), F({ key: 'layer', label: 'Display Layer', type: 'info', get: () => App.layers.find(l => l.id === inc.layer)?.name || 'defaultLayer' })], { key: 'disp' }));
      out.push(frame('Node Behavior', [F({ key: 'selectable', label: 'Selectable', type: 'bool', get: () => inc.selectable !== false, set: (v) => { inc.selectable = v; } })], { closed: true, key: 'nb' }));
      out.push(frame('Extra Attributes', [h('div', { class: 'dim', style: { padding: '2px 8px' }, text: 'Notes:' }), notesBox(rec)], { closed: true, key: 'extra' }));
    } else if (ctx.tab === 'shape') {
      if (inc.kind === 'mesh') {
        const st = inc.mesh.stats();
        out.push(frame('Mesh Info', [F({ key: 'i1', label: 'Vertices', type: 'info', get: () => inc.mesh.stats().verts }), F({ key: 'i2', label: 'Edges', type: 'info', get: () => inc.mesh.stats().edges }), F({ key: 'i3', label: 'Faces', type: 'info', get: () => inc.mesh.stats().faces }), F({ key: 'i4', label: 'Triangles', type: 'info', get: () => inc.mesh.stats().tris }), F({ key: 'i5', label: 'UVs', type: 'info', get: () => inc.mesh.stats().uvs })], { key: 'minfo' }));
        out.push(frame('Smooth Mesh', [F({ key: 'smoothLevel', label: 'Display', type: 'enum', options: ['Base mesh (1)', 'Cage + smooth (2)', 'Smooth mesh (3)'] }), F({ key: 'smoothDivisions', label: 'Preview Division Levels', type: 'int', min: 1, max: 4, smin: 1, smax: 4, get: () => App.prefs.smoothDivisions || 2, set: (v) => { App.prefs.smoothDivisions = v; App.savePrefs(); for (const o of S.allDag()) if (o.inca.kind === 'mesh' && o.inca.smoothLevel) { o.inca.mesh._smooth = null; S.rebuildShape(o); } } })], { key: 'smooth' }));
        out.push(frame('Normals Display', [F({ key: 'softAngle', label: 'Soft Edge Angle', type: 'float', min: 0, max: 180, smin: 0, smax: 180, get: () => inc.mesh.softAngle, set: (v) => { inc.mesh.softAngle = v; S.rebuildShape(rec); } })], { key: 'norm' }));
        out.push(frame('Render Stats', [F({ key: 'castShadows', label: 'Casts Shadows', type: 'bool', get: () => rec.userData.mesh.castShadow, set: (v) => { rec.userData.mesh.castShadow = v; inc.extra = { ...(inc.extra || {}), castShadows: v }; } }), F({ key: 'receiveShadows', label: 'Receive Shadows', type: 'bool', get: () => rec.userData.mesh.receiveShadow, set: (v) => { rec.userData.mesh.receiveShadow = v; inc.extra = { ...(inc.extra || {}), receiveShadows: v }; } }), F({ key: 'xray', label: 'X-Ray (object)', type: 'bool', get: () => !!inc.xray, set: (v) => { inc.xray = v; } })], { key: 'rstats' }));
        out.push(frame('Shading', [F({ key: 'mat', label: 'Material', type: 'info', get: () => App.mats.get(inc.material)?.name })], { key: 'shd' }));
      } else if (inc.kind === 'light') {
        const L = inc.light; const t = L.type;
        const fs = [F({ key: 'type', label: 'Type', type: 'info', get: () => niceName(t) }), F({ key: 'color', type: 'color' }), F({ key: 'intensity', type: 'float', min: 0, smin: 0, smax: t === 'skyDomeLight' ? 4 : 10 })];
        if (t === 'pointLight' || t === 'spotLight' || t === 'areaLight') fs.push(F({ key: 'decayRate', label: 'Decay Rate', type: 'enum', options: ['No Decay', 'Linear', 'Quadratic', 'Cubic'] }));
        if (t === 'spotLight') fs.push(F({ key: 'coneAngle', label: 'Cone Angle', type: 'float', min: 0.5, max: 179, smin: 1, smax: 179 }), F({ key: 'penumbraAngle', label: 'Penumbra Angle', type: 'float', min: -10, max: 10, smin: -10, smax: 10 }), F({ key: 'dropoff', label: 'Dropoff', type: 'float', min: 0, smin: 0, smax: 100 }));
        if (t === 'ambientLight') fs.push(F({ key: 'ambientShade', label: 'Ambient Shade', type: 'float', min: 0, max: 1 }));
        if (t === 'skyDomeLight') fs.push(F({ key: 'textureFile', label: 'Color Texture (HDRI)', type: 'file', filters: [{ name: 'HDR / Images', extensions: ['hdr', 'exr', 'png', 'jpg', 'jpeg'] }] }), F({ key: 'showBackground', label: 'Visible in render/viewport', type: 'bool' }));
        out.push(frame(niceName(t) + ' Attributes', fs, { key: 'light' + t }));
        if (t !== 'ambientLight' && t !== 'skyDomeLight') out.push(frame('Shadows', [F({ key: 'shadows', label: 'Use Depth Map / Ray Trace Shadows', type: 'bool' }), F({ key: 'shadowRadius', label: 'Shadow Softness', type: 'float', min: 0, smin: 0, smax: 10 })], { key: 'shadows' }));
        out.push(h('div', { class: 'dim', style: { padding: '6px 10px', fontSize: '11px' }, text: 'Tip: press 7 in a viewport to see lights; turn on Lighting > Shadows for shadows.' }));
      } else if (inc.kind === 'camera') {
        const c = inc.cam;
        out.push(frame('Camera Attributes', [F({ key: 'focalLength', label: 'Focal Length', type: 'float', min: 2.5, smin: 2.5, smax: 200 }), F({ key: 'aov', label: 'Angle of View', type: 'info', get: () => fmt(2 * Math.atan((c.horizontalFilmAperture * 25.4 / 2) / c.focalLength) / D2R, 2) }), F({ key: 'near', label: 'Near Clip Plane', type: 'float', min: 0.0001, smin: 0.001, smax: 10 }), F({ key: 'far', label: 'Far Clip Plane', type: 'float', min: 1, smin: 100, smax: 100000 }), F({ key: 'coi', label: 'Center of Interest', type: 'float', min: 0.001, smin: 0, smax: 100 })], { key: 'cam' }));
        out.push(frame('Film Back', [F({ key: 'horizontalFilmAperture', label: 'Camera Aperture (inch) H', type: 'float', min: 0.01, smin: 0.1, smax: 3 }), F({ key: 'verticalFilmAperture', label: 'Camera Aperture (inch) V', type: 'float', min: 0.01, smin: 0.1, smax: 3 })], { key: 'film', closed: true }));
        out.push(frame('Orthographic Views', [F({ key: 'ortho', label: 'Orthographic', type: 'bool', set: (v) => { c.ortho = v; const cam = rec.userData.camera; App.viewports.forEach(vp => vp.needsRender = true); App.emit('warning', '// Warning: switching projection takes effect after re-opening the scene'); } }), F({ key: 'orthoWidth', label: 'Orthographic Width', type: 'float', min: 0.01, smin: 1, smax: 100 })], { key: 'orth', closed: true }));
      } else if (inc.kind === 'curve') {
        const cv = inc.curve;
        out.push(frame('NURBS Curve History', [F({ key: 'deg', label: 'Degree', type: 'enum', options: ['1 Linear', '2', '3 Cubic'], values: [1, 2, 3], get: () => cv.degree, set: (v) => { cv.degree = v; S.rebuildShape(rec); } }), F({ key: 'form', label: 'Form', type: 'enum', options: ['Open', 'Periodic (closed)'], values: ['open', 'periodic'], get: () => cv.form, set: (v) => { cv.form = v; S.rebuildShape(rec); } }), F({ key: 'kind', label: 'Curve Type', type: 'info', get: () => cv.kind === 'ep' ? 'EP (interpolating)' : 'CV' }), F({ key: 'spans', label: 'Spans', type: 'info', get: () => cv.spans }), F({ key: 'cvs', label: 'CVs', type: 'info', get: () => cv.cvs.length }), F({ key: 'len', label: 'Arc Length', type: 'info', get: () => fmt(cv.length()) })], { key: 'crv' }));
      } else if (inc.kind === 'locator') {
        out.push(frame('Locator Attributes', [F({ key: 'lsc', label: 'Local Scale', type: 'info', get: () => '1 1 1' })], { key: 'loc' }));
      }
      if (App.dynamics?.aeFrames) out.push(...App.dynamics.aeFrames(rec, F, frame)); // dynamics: emitter / particles / field / rigid body attributes
    }
  }
  return out;
}
function notesBox(rec) { const ta = h('textarea', { class: 'ae-notes', style: { width: 'calc(100% - 8px)' } }); ta.value = rec.inca.notes || ''; ta.addEventListener('keydown', e => e.stopPropagation()); ta.addEventListener('change', () => { rec.inca.notes = ta.value; rec.inca.extra = { ...(rec.inca.extra || {}), notes: ta.value }; App.modified = true; }); return ta; }
function rampEditor(rec, ctx) {
  const wrap = h('div', { style: { padding: '2px 0 6px 0' } });
  const draw = () => {
    wrap.innerHTML = '';
    const cols = rec.attrs.colors;
    const bar = h('div', { style: { height: '22px', margin: '2px 8px 6px 124px', border: '1px solid #222', background: `linear-gradient(90deg, ${cols.slice().sort((a, b) => a[0] - b[0]).map(([p, c]) => colorToCss(c) + ' ' + (p * 100) + '%').join(',')})` } });
    wrap.append(bar);
    cols.forEach((e, i) => {
      const pos = numField(e[0], (n) => { ck('ramp'); e[0] = Math.max(0, Math.min(1, n)); touchTexture(rec); draw(); }, { step: 0.01 });
      const sw = h('div', { class: 'colorsw', style: { background: colorToCss(e[1]) } });
      sw.addEventListener('click', () => { ck('ramp color'); const r = sw.getBoundingClientRect(); colorPicker(r.left, r.bottom, e[1], (c) => { e[1] = c; sw.style.background = colorToCss(c); touchTexture(rec); }, () => draw()); });
      wrap.append(row('Entry ' + (i + 1), pos, sw, h('button', { text: '✕', title: 'Remove entry', style: { height: '18px', padding: '0 6px' }, onclick: () => { if (cols.length <= 1) return; ck('ramp'); cols.splice(i, 1); touchTexture(rec); draw(); } })));
    });
    wrap.append(h('div', { class: 'ae-row' }, h('div', { class: 'lbl' }), h('button', { text: 'Add Entry', onclick: () => { ck('ramp'); cols.push([0.75, [1, 1, 1]]); touchTexture(rec); draw(); } })));
  };
  draw(); return wrap;
}

// ------------------------------------------------------------------ embeddable panel
export function attrPanel(container, rec, opts = {}) {
  const ctx = { fields: [], tab: opts.tab || 'transform', openRec: opts.openRec || ((r) => App.ui.showAttr(r)), refresh: (full) => { if (full) build(); else for (const f of ctx.fields) f._upd?.(); } };
  const body = h('div', { class: 'ae-body' }); container.append(body);
  function build() { body.innerHTML = ''; ctx.fields = []; if (!rec) return; body.append(...layoutFor(rec, ctx)); }
  build();
  return { refresh: (full) => ctx.refresh(full), destroy: () => body.remove(), setRec: (r, tab) => { rec = r; if (tab) ctx.tab = tab; build(); }, ctx };
}
App.ui.attrPanel = (c, r) => attrPanel(c, r);

// ------------------------------------------------------------------ Attribute Editor dock panel
const AE = { target: null, tab: null, locked: false };
function aeTabs(target) {
  // returns [{id, label, rec, tab}]
  if (!target) return [];
  if (target.isObject3D) {
    const o = target; const t = [{ id: 'xf', label: o.inca.name, rec: o, tab: 'transform' }];
    if (o.inca.shapeName) t.push({ id: 'shape', label: o.inca.shapeName, rec: o, tab: 'shape' });
    for (const hh of [...(o.inca.history || [])].reverse()) if (OPS[hh.type] && !OPS[hh.type].hidden) t.push({ id: hh.id, label: hh.name, rec: hh });
    if (o.inca.kind === 'mesh') { const m = App.mats.get(o.inca.material); if (m) { t.push({ id: m.id, label: m.name, rec: m }); for (const tid of Object.values(m.maps || {})) { const tx = App.texs.get(tid); if (tx) t.push({ id: tx.id, label: tx.name, rec: tx }); } } }
    return t;
  }
  if (target.kind === 'material') { const t = [{ id: target.id, label: target.name, rec: target }]; for (const tid of Object.values(target.maps || {})) { const tx = App.texs.get(tid); if (tx) t.push({ id: tx.id, label: tx.name, rec: tx }); } return t; }
  return [{ id: target.id, label: target.name, rec: target }];
}
function buildAE(container) {
  const menus = h('div', { class: 'pmenubar' });
  const tabs = h('div', { class: 'ae-tabs' });
  const nameIn = h('input', {});
  nameIn.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') nameIn.blur(); });
  nameIn.addEventListener('change', () => { const t = cur(); if (!t) return; ck('rename'); S.rename(t.rec.isObject3D && t.tab === 'shape' ? t.rec : t.rec, nameIn.value.trim()); render(true); });
  const lockBtn = iconBtn('locked', 'Lock the Attribute Editor to the current node', () => { AE.locked = !AE.locked; lockBtn.classList.toggle('on', AE.locked); });
  lockBtn.innerHTML = '🔒'; lockBtn.style.fontSize = '11px';
  const head = h('div', { class: 'ae-head' }, h('span', { class: 'dim', text: 'node:' }), nameIn, h('button', { text: 'Focus', title: 'Show the selected node', onclick: () => { AE.locked = false; lockBtn.classList.remove('on'); follow(); } }), lockBtn);
  const holder = h('div', { style: { flex: '1', display: 'flex', flexDirection: 'column', minHeight: '0' } });
  const empty = h('div', { class: 'cb-empty', text: 'Select an object to see its attributes.' });
  const btns = h('div', { class: 'ae-buttons' }, h('button', { text: 'Select', onclick: () => { const t = cur(); if (t?.rec.isObject3D) Sel.select([t.rec]); else if (t?.rec.kind === 'history') Sel.select([S.historyOwner(t.rec)]); } }), h('button', { text: 'Load Attributes', onclick: () => render(true) }), h('button', { text: 'Copy Tab' }), h('button', { text: 'Close', onclick: () => App.ui.toggleRightPanel('attr') }));
  btns.children[2].onclick = () => { const t = cur(); if (!t) return; import('./dom.js').then(({ FloatWin }) => { const w = new FloatWin('aecopy-' + t.id + Math.random(), 'Attribute Editor: ' + t.label, { w: 380, h: 520 }); const p = attrPanel(w.body, t.rec, { tab: t.tab }); const off = App.on('refresh', () => p.refresh()); w.onClose = off; }); };
  container.append(menus, tabs, head, holder, btns);
  menuBar(menus, [
    { label: 'List', items: [{ label: 'Auto Load Selected Attributes', check: () => !AE.locked, fn: () => { AE.locked = !AE.locked; lockBtn.classList.toggle('on', AE.locked); } }, { label: 'Load Selected Attributes', fn: () => { AE.locked = false; follow(); } }] },
    { label: 'Selected', items: [{ label: 'Select Node', fn: () => btns.children[0].click() }] },
    { label: 'Focus', items: () => App.sel.map(o => ({ label: o.inca.name, fn: () => { AE.target = o; render(true); } })) },
    { label: 'Attributes', items: [{ label: 'Add Attribute...', enabled: () => false }] },
    { label: 'Show', items: [{ label: 'Show Notes', fn: () => App.help('Notes are under Extra Attributes on the transform tab') }] },
    { label: 'Help', items: [{ label: 'Help on Attribute Editor', fn: () => App.help('The Attribute Editor shows every attribute of the selected node; tabs list connected nodes (shape, history, material).') }] },
  ]);
  let panel = null; let sig = '';
  const cur = () => { const list = aeTabs(AE.target); return list.find(t => t.id === AE.tab) || list[0]; };
  function follow() { if (AE.locked) return; const o = App.compMode ? (App.hilite[App.hilite.length - 1] || Sel.lead()) : Sel.lead(); if (o !== AE.target && !(AE.target && !AE.target.isObject3D && AE.explicit)) { AE.target = o; AE.tab = AE.target && AE.lastTabKind === 'shape' && o?.inca.shapeName ? 'shape' : AE.tab; } render(); }
  function render(force = false) {
    if (AE.target && AE.target.isObject3D && !App.nodes.has(AE.target.inca.id)) AE.target = null;
    if (AE.target && !AE.target.isObject3D && !App.nodes.has(AE.target.id)) AE.target = null;
    const list = aeTabs(AE.target);
    const s = list.map(t => t.id + t.label).join('|') + '#' + (cur()?.id || '');
    if (!force && s === sig) { panel?.refresh(); return; }
    sig = s;
    tabs.innerHTML = '';
    const c = cur();
    for (const t of list) { const el = h('div', { class: 'ae-tab' + (c && t.id === c.id ? ' active' : ''), text: t.label }); el.addEventListener('click', () => { AE.tab = t.id; AE.lastTabKind = t.tab || null; render(true); }); tabs.append(el); }
    holder.innerHTML = '';
    if (!c) { holder.append(empty); nameIn.value = ''; panel = null; return; }
    nameIn.value = c.rec.isObject3D ? (c.tab === 'shape' ? c.rec.inca.shapeName : c.rec.inca.name) : c.rec.name;
    panel = attrPanel(holder, c.rec, { tab: c.tab, openRec: (r) => { if (aeTabs(AE.target).some(t => t.rec === r)) { AE.tab = r.id; render(true); } else App.ui.showAttr(r); } });
  }
  App.ui.showAttr = (rec) => {
    if (!rec) return;
    if (rec.kind === 'history') { AE.target = S.historyOwner(rec); AE.tab = rec.id; }
    else if (rec.isObject3D) { AE.target = rec; AE.tab = 'xf'; }
    else { const lead = Sel.lead(); if (lead && aeTabs(lead).some(t => t.rec === rec)) { AE.target = lead; } else AE.target = rec; AE.tab = rec.id; AE.explicit = true; }
    App.ui.showRightPanel('attr'); render(true);
  };
  App.on('selectionChanged', () => { AE.explicit = false; follow(); });
  App.on('sceneLoaded', () => { AE.target = null; AE.explicit = false; follow(); });
  return { refresh(d) { if (d.has('all') || d.has('selection')) follow(); else if (d.has('attr') || d.has('channels') || d.has('hypershade') || d.has('outliner')) render(); }, show() { follow(); render(true); } };
}
registerPanel('attr', 'Attribute Editor', buildAE, { short: 'Attribute Editor' });

// ------------------------------------------------------------------ Tool Settings
function buildToolSettings(container) {
  const head = h('div', { class: 'cb-head' });
  const body = h('div', { class: 'pbody tool-settings' });
  container.append(head, body);
  const ctx = { fields: [], refresh: () => ctx.fields.forEach(f => f._upd?.()) };
  const P = App.prefs.opt.tools || (App.prefs.opt.tools = {});
  const F = (spec) => { const el = fieldFor({ attrs: {} }, spec, ctx); ctx.fields.push(el); return el; };
  const softFields = () => frame('Soft Selection', [
    F({ key: 'soft', label: 'Soft Select', type: 'bool', get: () => Manip.soft.on, set: (v) => { Manip.soft.on = v; for (const o of App.hilite) o.userData.compDirty = true; App.requestRender(); } }),
    F({ key: 'falloffMode', label: 'Falloff mode', type: 'enum', options: ['Volume'], get: () => 0, set: () => {} }),
    F({ key: 'radius', label: 'Falloff radius', type: 'float', min: 0.001, smin: 0, smax: 10, get: () => Manip.soft.radius, set: (v) => { Manip.soft.radius = v; for (const o of App.hilite) o.userData.compDirty = true; App.requestRender(); } }),
    F({ key: 'curve', label: 'Falloff curve', type: 'enum', options: ['Smooth', 'Linear'], values: ['smooth', 'linear'], get: () => Manip.soft.falloff, set: (v) => { Manip.soft.falloff = v; for (const o of App.hilite) o.userData.compDirty = true; App.requestRender(); } }),
  ], { key: 'ts-soft' });
  function render() {
    body.innerHTML = ''; ctx.fields = [];
    const t = App.tool;
    head.textContent = { select: 'Select Tool', lasso: 'Lasso Tool', paint: 'Paint Selection Tool', move: 'Move Tool', rotate: 'Rotate Tool', scale: 'Scale Tool', cvCurve: 'CV Curve Tool', epCurve: 'EP Curve Tool', edgeLoop: 'Insert Edge Loop Tool', multiCut: 'Multi-Cut Tool', targetWeld: 'Target Weld Tool', createPoly: 'Create Polygon Tool' }[t] || t;
    body.append(h('div', { class: 'ae-row', style: { justifyContent: 'flex-end', padding: '2px 4px' } }, h('button', { text: 'Reset Tool', onclick: () => { P[t] = {}; if (TRANSFORM_SPACE[t]) Manip.space[TRANSFORM_SPACE[t]] = t === 'move' ? 'world' : 'local'; Manip.soft = { on: false, radius: 2, falloff: 'smooth' }; render(); placeProxy(); App.requestRender(); } })));
    const mode = TRANSFORM_SPACE[t];
    if (mode) {
      const spaces = t === 'scale' ? [['Object', 'local'], ['World', 'world']] : [['World', 'world'], ['Object', 'local']];
      body.append(frame(head.textContent.replace(' Tool', '') + ' Settings', [
        F({ key: 'space', label: 'Axis Orientation', type: 'enum', options: spaces.map(s => s[0]), values: spaces.map(s => s[1]), get: () => Manip.space[mode], set: (v) => { Manip.space[mode] = v; placeProxy(); App.requestRender(); } }),
        F({ key: 'step', label: t === 'rotate' ? 'Discrete rotate (deg)' : 'Step snap', type: 'float', min: 0, smin: 0, smax: t === 'rotate' ? 90 : 5, get: () => (Manip.step || {})[mode] || 0, set: (v) => { Manip.step = { ...(Manip.step || {}), [mode]: v }; App.requestRender(); } }),
        F({ key: 'pivot', label: 'Edit pivot (D)', type: 'bool', get: () => Manip.pivotMode, set: (v) => { Manip.pivotMode = v; placeProxy(); App.requestRender(); } }),
      ], { key: 'ts-' + t }));
      body.append(softFields());
      body.append(frame('Symmetry Settings', [F({ key: 'sym', label: 'Symmetry', type: 'enum', options: ['Off', 'Object X', 'World X', 'World Y', 'World Z'], values: ['off', 'objectX', 'worldX', 'worldY', 'worldZ'], get: () => Manip.symmetry || 'off', set: (v) => { Manip.symmetry = v; App.dirty('statusline'); } })], { key: 'ts-sym', closed: true }));
    } else if (t === 'select' || t === 'lasso' || t === 'paint') {
      body.append(frame('Common Selection Options', [F({ key: 'cam', label: 'Camera-based selection', type: 'bool', get: () => App.prefs.opt.cameraBasedSel !== false, set: (v) => { App.prefs.opt.cameraBasedSel = v; App.savePrefs(); } }), F({ key: 'pre', label: 'Pre-selection highlight', type: 'bool', get: () => App.prefs.opt.preselect !== false, set: (v) => { App.prefs.opt.preselect = v; App.savePrefs(); } })], { key: 'ts-sel' }));
      body.append(softFields());
    } else if (App.tools?.settings?.[t]) {
      body.append(frame('Tool Settings', App.tools.settings[t].map(s => F(s)), { key: 'ts-' + t }));
    } else body.append(h('div', { class: 'cb-empty', text: 'No settings for this tool.' }));
  }
  App.on('toolChanged', () => { if (container.isConnected) render(); });
  return { refresh(d) { if (d.has('all') || d.has('toolSettings')) render(); else ctx.refresh(); }, show: render };
}
const TRANSFORM_SPACE = { move: 'translate', rotate: 'rotate', scale: 'scale' };
registerPanel('toolSettings', 'Tool Settings', buildToolSettings, { short: 'Tool Settings' });

// ------------------------------------------------------------------ Modeling Toolkit
function buildMTK(container) {
  const modes = h('div', { class: 'mtk-modes' });
  const md = [['objectModeOnly', 'selObject', 'Object selection'], ['vertexMode', 'polySphere', 'Vertex selection (F9)'], ['edgeMode', 'multiCut', 'Edge selection (F10)'], ['faceMode', 'polyPlane', 'Face selection (F11)'], ['uvMode', 'uvEditor', 'UV selection (F12)']];
  const mbtns = md.map(([cmd, ic, t]) => { const b = iconBtn(ic, t, () => App.cmds.run(cmd)); b._cmd = cmd; return b; });
  modes.append(...mbtns, h('div', { class: 'spacer' }), iconBtn('softSelect', 'Soft Select (B)', () => App.cmds.run('softSelect')));
  const sym = h('select', {}, ['Off', 'Object X', 'World X', 'World Y', 'World Z'].map(s => h('option', { text: s })));
  sym.addEventListener('change', () => { Manip.symmetry = ['off', 'objectX', 'worldX', 'worldY', 'worldZ'][sym.selectedIndex]; App.dirty('statusline'); });
  const body = h('div', { class: 'pbody' });
  container.append(h('div', { class: 'cb-head', text: 'Modeling Toolkit' }), modes, h('div', { class: 'ae-row', style: { padding: '0 6px' } }, h('span', { class: 'dim', text: 'Symmetry: ' }), sym), body);
  const section = (title, items) => frame(title, items.map(([cmd, ic]) => { const c = App.cmds.registry.get(cmd); if (!c) return null; const r = h('div', { class: 'mtk-btn', html: icon(ic || c.icon || 'help') + `<span>${c.label}</span>`, title: c.help || c.label }); r.addEventListener('click', () => App.cmds.run(cmd)); r.addEventListener('contextmenu', (e) => { e.preventDefault(); if (c.options) App.cmds.option(cmd); }); r.addEventListener('mouseenter', () => App.help(c.help || c.label)); return r; }), { key: 'mtk-' + title });
  body.append(
    section('Mesh', [['combine'], ['separate'], ['smooth'], ['mirror'], ['fillHole'], ['triangulate'], ['quadrangulate'], ['cleanup', 'merge']]),
    section('Components', [['extrude'], ['bevel'], ['bridge'], ['addDivisions'], ['poke'], ['chamferVertex', 'chamfer'], ['merge'], ['mergeToCenter', 'mergeCenter'], ['collapse', 'mergeCenter'], ['deleteEdgeVertex', 'deleteEdge'], ['extract'], ['duplicateFaces', 'duplicateFace'], ['connect', 'multiCut'], ['reverseNormals', 'reverse']]),
    section('Tools', [['multiCutTool', 'multiCut'], ['targetWeldTool', 'targetWeld'], ['insertEdgeLoopTool', 'edgeLoop'], ['createPolygonTool', 'planar'], ['quadDrawTool', 'polyPlane']]),
    section('Normals & UVs', [['softenEdge', 'softEdge'], ['hardenEdge', 'hardEdge'], ['planarMap'], ['automaticMap', 'autoMap'], ['uvEditor']]),
  );
  return { refresh() { for (const b of mbtns) b.classList.toggle('on', b._cmd === 'objectModeOnly' ? !App.compMode : b._cmd === App.compMode + 'Mode'); } };
}
registerPanel('mtk', 'Modeling Toolkit', buildMTK, { short: 'Modeling Toolkit' });
