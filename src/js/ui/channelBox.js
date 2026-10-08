// Inca — Channel Box / Layer Editor
import { App } from '../core/app.js';
import * as S from '../core/scene.js';
import { Undo } from '../core/undo.js';
import { Sel } from '../core/selection.js';
import { OPS, niceName } from '../core/ops.js';
import { h, menuBar, showMenu, fmt, iconBtn, colorToCss, promptDialog } from './dom.js';
import { registerPanel } from './docks.js';
import { placeProxy } from '../core/manip.js';
import '../core/rigging.js'; // rigging: loads the rigging/deformation modules (joints, IK, skin, deformers, constraints)

const XF_LABEL = { translateX: 'Translate X', translateY: 'Translate Y', translateZ: 'Translate Z', rotateX: 'Rotate X', rotateY: 'Rotate Y', rotateZ: 'Rotate Z', scaleX: 'Scale X', scaleY: 'Scale Y', scaleZ: 'Scale Z', visibility: 'Visibility' };
const LIGHT_CH = { intensity: 'Intensity', colorR: 'Color R', colorG: 'Color G', colorB: 'Color B', coneAngle: 'Cone Angle', penumbraAngle: 'Penumbra Angle', dropoff: 'Dropoff', decayRate: 'Decay Rate', shadows: 'Use Shadows' };
const CAM_CH = { focalLength: 'Focal Length', near: 'Near Clip Plane', far: 'Far Clip Plane', orthoWidth: 'Orthographic Width', coi: 'Center Of Interest' };
const LAYER_COLORS = [null, [0, 0, 0], [0.25, 0.25, 0.25], [0.6, 0.6, 0.6], [0.6, 0, 0.16], [0, 0, 0.38], [0, 0, 1], [0, 0.27, 0.1], [0.15, 0, 0.26], [0.78, 0, 0.78], [0.54, 0.28, 0.2], [0.25, 0.14, 0.12], [0.6, 0.15, 0], [1, 0, 0], [0, 1, 0], [0, 0.25, 0.6], [1, 1, 1], [1, 1, 0], [0.39, 0.86, 1], [0.26, 1, 0.64], [1, 0.69, 0.69], [0.89, 0.67, 0.47], [1, 1, 0.39], [0, 0.6, 0.33]];

const st = { selChannels: new Set(), openHistory: new Set(), sig: '', fields: [] };
App.channelSlider = {
  active: () => st.selChannels.size > 0 && (App.sel.length > 0),
  drag(e) { // MMB virtual slider in a viewport
    const chans = [...st.selChannels]; const targets = targetsFor(chans);
    if (!targets.length) return;
    Undo.checkpoint('channel drag');
    const x0 = e.clientX; const start = targets.map(([rec, a]) => +S.getAttr(rec, a) || 0);
    const step = App.prefs.opt.cbSliderStep || 0.1;
    const mm = (ev) => { const d = (ev.clientX - x0) * step * (ev.ctrlKey ? 0.1 : ev.shiftKey ? 10 : 1); targets.forEach(([rec, a], i) => { S.setAttr(rec, a, start[i] + d, { silent: true }); }); App.dirty('channels'); placeProxy(); App.requestRender(); };
    const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); for (const [rec, a] of targets) if (rec.isObject3D) App.anim.autoKey(rec, [a]); App.dirty('channels', 'attr'); };
    addEventListener('mousemove', mm); addEventListener('mouseup', mu);
  },
};
// channel id format: "xf:translateX" | "shape:intensity" | "h:<historyId>:<param>"
function targetsFor(chans) {
  const out = [];
  for (const c of chans) {
    const [kind, a, b] = c.split(':');
    if (kind === 'xf' || kind === 'shape') for (const o of App.sel) { if (kind === 'shape' && S.getAttr(o, a) === undefined) continue; out.push([o, a]); }
    else if (kind === 'h') { const rec = App.nodes.get(a); if (rec) out.push([rec, b]); }
  }
  return out;
}
const lead = () => Sel.lead();

function build(container) {
  const menus = h('div', { class: 'pmenubar' });
  const tabs = h('div', { class: 'le-tabs', style: { paddingTop: '2px' } }, h('div', { class: 'le-tab active', text: 'Channel Box / Layer Editor' }));
  const cbBody = h('div', { class: 'pbody cb', style: { flex: '1 1 60%' } });
  const split = h('div', { class: 'split-h' });
  const le = h('div', { class: 'le', style: { flex: '0 0 190px', display: 'flex', flexDirection: 'column', minHeight: '60px' } });
  container.append(tabs, menus, cbBody, split, le);
  menuBar(menus, [
    { label: 'Channels', items: () => [
      { label: 'Key Selected', fn: () => keySelected() }, { label: 'Key All Keyable', fn: () => App.cmds.run('setKey') }, { label: 'Breakdown Selected', fn: () => keySelected() },
      '-', { label: 'Cut Selected', fn: () => deleteKeysSel() }, { label: 'Delete Selected', fn: () => deleteKeysSel() },
      '-', { label: 'Lock Selected', fn: () => lockSel(true) }, { label: 'Unlock Selected', fn: () => lockSel(false) }, { label: 'Hide Selected', fn: () => hideSel() },
      '-', { label: 'Break Connections', fn: () => deleteKeysSel() }, { label: 'Select All Channels', fn: () => { for (const k of Object.keys(XF_LABEL)) st.selChannels.add('xf:' + k); refreshValues(); } },
      '-', { label: 'Slider step: fine (0.01)', check: () => App.prefs.opt.cbSliderStep === 0.01, fn: () => { App.prefs.opt.cbSliderStep = 0.01; App.savePrefs(); } }, { label: 'Slider step: medium (0.1)', check: () => (App.prefs.opt.cbSliderStep || 0.1) === 0.1, fn: () => { App.prefs.opt.cbSliderStep = 0.1; App.savePrefs(); } }, { label: 'Slider step: coarse (1)', check: () => App.prefs.opt.cbSliderStep === 1, fn: () => { App.prefs.opt.cbSliderStep = 1; App.savePrefs(); } },
    ] },
    { label: 'Edit', items: [{ label: 'Expressions...', fn: () => App.cmds.run('scriptEditor') }, { label: 'Set Driven Key...', fn: () => App.cmds.run('setDrivenKeyWindow') }, '-', { label: 'Delete Attribute', enabled: () => false }] },
    { label: 'Object', items: () => [{ label: lead() ? lead().inca.name : '(none)', enabled: () => false }, { label: 'Rename...', cmd: 'renameSel' }, { label: 'Attribute Editor', cmd: 'attributeEditor' }] },
    { label: 'Show', items: () => [{ label: 'Show Shapes', check: () => App.prefs.opt.cbShapes !== false, fn: () => { App.prefs.opt.cbShapes = App.prefs.opt.cbShapes === false; App.savePrefs(); st.sig = ''; refreshAll(); } }, { label: 'Show Inputs', check: () => App.prefs.opt.cbInputs !== false, fn: () => { App.prefs.opt.cbInputs = App.prefs.opt.cbInputs === false; App.savePrefs(); st.sig = ''; refreshAll(); } }, { label: 'Show Hidden Channels', fn: () => { for (const o of App.sel) o.inca.hiddenCh = {}; st.sig = ''; refreshAll(); } }] },
  ]);
  // splitter between channel box and layer editor
  split.addEventListener('mousedown', (e) => { const y0 = e.clientY, h0 = le.offsetHeight; const mm = (ev) => { le.style.flexBasis = Math.max(60, Math.min(container.offsetHeight - 80, h0 - (ev.clientY - y0))) + 'px'; }; const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); }; addEventListener('mousemove', mm); addEventListener('mouseup', mu); });

  // ------------------------------------------------------------ channel box body
  function row(chanId, label, rec, attr, opts = {}) {
    const r = h('div', { class: 'cb-row' + (st.selChannels.has(chanId) ? ' sel' : '') });
    const name = h('div', { class: 'cb-name', text: label, title: attr });
    let inp;
    const val = S.getAttr(rec, attr);
    if (opts.type === 'enum') {
      inp = h('select', {}, (opts.options || []).map((o, i) => h('option', { value: i, text: o })));
      const vals = opts.values; inp.selectedIndex = vals ? Math.max(0, vals.indexOf(val)) : (+val || 0);
      inp.addEventListener('change', () => commit(chanId, rec, attr, vals ? vals[inp.selectedIndex] : inp.selectedIndex));
    } else if (opts.type === 'bool' || attr === 'visibility') {
      inp = h('select', {}, h('option', { text: 'off', value: 0 }), h('option', { text: 'on', value: 1 }));
      inp.selectedIndex = val ? 1 : 0;
      inp.addEventListener('change', () => commit(chanId, rec, attr, attr === 'visibility' ? inp.selectedIndex : !!inp.selectedIndex));
    } else {
      inp = h('input', { value: fmt(+val || 0) });
      inp.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') { inp.blur(); } if (e.key === 'Escape') { inp.value = fmt(+S.getAttr(rec, attr) || 0); inp.blur(); } if (e.key === 'Tab') { e.preventDefault(); const all = [...cbBody.querySelectorAll('.cb-row input')]; const i = all.indexOf(inp); inp.blur(); const nx = all[(i + (e.shiftKey ? -1 : 1) + all.length) % all.length]; nx?.focus(); nx?.select(); } });
      inp.addEventListener('focus', () => { inp.select(); });
      inp.addEventListener('change', () => {
        const s = inp.value.trim(); const cur = +S.getAttr(rec, attr) || 0; let n;
        const m = s.match(/^([+\-*/])=\s*(-?[\d.eE+-]+)$/);
        if (m) { const a = parseFloat(m[2]); n = m[1] === '+' ? cur + a : m[1] === '-' ? cur - a : m[1] === '*' ? cur * a : cur / a; }
        else { n = Number(s); if (isNaN(n)) { try { n = Function('"use strict";return (' + s.replace(/[^-+*/().\d\s eE]/g, '') + ')')(); } catch { n = NaN; } } }
        if (typeof n !== 'number' || isNaN(n)) { inp.value = fmt(cur); return; }
        if (opts.type === 'int') n = Math.round(n);
        if (opts.min !== undefined) n = Math.max(opts.min, n); if (opts.max !== undefined) n = Math.min(opts.max, n);
        commit(chanId, rec, attr, n, opts.relative ? n - cur : null);
      });
      // ctrl+LMB / MMB drag directly in the field = virtual slider
      inp.addEventListener('mousedown', (e) => {
        if (!(e.button === 1 || (e.button === 0 && e.ctrlKey))) return;
        e.preventDefault(); if (!st.selChannels.has(chanId)) { st.selChannels = new Set([chanId]); }
        App.channelSlider.drag(e);
      });
    }
    r._upd = () => {
      const v = S.getAttr(rec, attr);
      if (inp.tagName === 'SELECT') { if (document.activeElement !== inp) { if (opts.type === 'enum') { const vals = opts.values; inp.selectedIndex = vals ? Math.max(0, vals.indexOf(v)) : (+v || 0); } else inp.selectedIndex = v ? 1 : 0; } }
      else if (document.activeElement !== inp) inp.value = fmt(+v || 0);
      const keyed = rec.isObject3D && App.anim.isKeyed(rec, attr); const onKey = keyed && App.anim.onKey(rec, attr);
      const locked = S.isLocked(rec, attr);
      inp.classList.toggle('keyed', keyed && !onKey); inp.classList.toggle('onkey', onKey); inp.classList.toggle('locked', locked);
      r.classList.toggle('sel', st.selChannels.has(chanId));
    };
    r._upd();
    name.addEventListener('mousedown', (e) => {
      if (e.button === 2) return;
      if (e.ctrlKey) st.selChannels.has(chanId) ? st.selChannels.delete(chanId) : st.selChannels.add(chanId);
      else if (e.shiftKey && st.lastClicked) { const ids = st.order; const a = ids.indexOf(st.lastClicked), b = ids.indexOf(chanId); if (a >= 0 && b >= 0) for (let i = Math.min(a, b); i <= Math.max(a, b); i++) st.selChannels.add(ids[i]); }
      else { st.selChannels = new Set([chanId]); }
      st.lastClicked = chanId;
      // drag across names to select a range
      const mm = (ev) => { const el = document.elementFromPoint(ev.clientX, ev.clientY)?.closest('.cb-row'); if (el && el._chan) { st.selChannels.add(el._chan); refreshValues(); } };
      const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu);
      refreshValues(); App.help('Middle-drag in a viewport to change the selected channels (virtual slider)');
    });
    name.addEventListener('contextmenu', (e) => { e.preventDefault(); if (!st.selChannels.has(chanId)) { st.selChannels = new Set([chanId]); refreshValues(); } channelMenu(e); });
    r._chan = chanId;
    r.append(name, inp);
    st.fields.push(r); st.order.push(chanId);
    return r;
  }
  function commit(chanId, rec, attr, value) {
    const list = st.selChannels.has(chanId) && st.selChannels.size > 0 ? [...new Set([chanId, ...st.selChannels])] : [chanId];
    const sameAttrTargets = [];
    for (const c of list) { const [kind, a, b] = c.split(':'); const at = kind === 'h' ? b : a; if (at !== attr) continue; sameAttrTargets.push(...targetsFor([c])); }
    const targets = sameAttrTargets.length ? sameAttrTargets : [[rec, attr]];
    Undo.checkpoint('setAttr ' + attr);
    let n = 0;
    for (const [r, a] of targets) { if (S.isLocked(r, a)) { App.emit('warning', `// Warning: ${S.nodeName(r)}.${a} is locked`); continue; } if (S.setAttr(r, a, value)) { n++; if (r.isObject3D) App.anim.autoKey(r, [a]); } }
    App.emit('echo', `setAttr "${S.nodeName(rec)}.${attr}" ${typeof value === 'boolean' ? +value : fmt(+value, 4)};`);
    placeProxy(); App.dirty('channels', 'attr', 'outliner'); App.requestRender();
  }
  function channelMenu(e) {
    showMenu([
      { label: 'Channels', enabled: () => false }, '-',
      { label: 'Key Selected', fn: keySelected }, { label: 'Key All', fn: () => App.cmds.run('setKey') }, { label: 'Breakdown Selected', fn: keySelected },
      '-', { label: 'Cut Selected', fn: deleteKeysSel }, { label: 'Copy Selected', fn: () => App.anim.copyKeys(App.sel) }, { label: 'Paste Selected', fn: () => { Undo.checkpoint('paste keys'); App.anim.pasteKeys(App.sel); App.anim.evaluate(); } }, { label: 'Delete Selected', fn: deleteKeysSel },
      '-', { label: 'Break Connections', fn: deleteKeysSel },
      '-', { label: 'Select All', fn: () => { for (const k of st.order) st.selChannels.add(k); refreshValues(); } },
      '-', { label: 'Lock Selected', fn: () => lockSel(true) }, { label: 'Unlock Selected', fn: () => lockSel(false) }, { label: 'Hide Selected', fn: hideSel }, { label: 'Lock and Hide Selected', fn: () => { lockSel(true); hideSel(); } },
      '-', { label: 'Expressions...', fn: () => App.cmds.run('scriptEditor') }, { label: 'Set Driven Key...', fn: () => App.cmds.run('setDrivenKeyWindow') },
    ], e.clientX, e.clientY);
  }

  function signature() { const o = App.sel.map(x => x.inca.id + ':' + (x.inca.history ? x.inca.history.map(hh => hh.id).join(',') : '') + ':' + Object.keys(x.inca.hiddenCh || {}).join(',') + ':' + Object.keys(x.inca.extraAttrs || {}).join(',')).join('|'); /* rigging: + extraAttrs */ return o + '#' + [...st.openHistory].join(',') + '#' + (App.prefs.opt.cbShapes !== false) + (App.prefs.opt.cbInputs !== false); }
  function rebuild() {
    cbBody.innerHTML = ''; st.fields = []; st.order = [];
    const o = lead();
    if (!o) { cbBody.append(h('div', { class: 'cb-empty', text: '' })); return; }
    const head = h('div', { class: 'cb-head', text: o.inca.name + (App.sel.length > 1 ? '  ...' : ''), title: 'Double-click to rename' });
    head.addEventListener('dblclick', () => { const inp = h('input', { value: o.inca.name, style: { width: '100%' } }); head.replaceWith(inp); inp.focus(); inp.select(); const done = (ok) => { if (ok && inp.value.trim() && inp.value !== o.inca.name) { Undo.checkpoint('rename'); S.rename(o, inp.value.trim()); App.emit('echo', `rename "${inp.value.trim()}";`); } st.sig = ''; refreshAll(); }; inp.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') done(true); if (e.key === 'Escape') done(false); }); inp.addEventListener('blur', () => done(true)); });
    cbBody.append(head);
    const hidden = o.inca.hiddenCh || {};
    if (!o.inca.startup || true) for (const [a, l] of Object.entries(XF_LABEL)) if (!hidden[a]) cbBody.append(row('xf:' + a, l, o, a));
    for (const c of App.rig?.channelRows?.(o) || []) { if (c.section) cbBody.append(h('div', { class: 'cb-node', text: c.section })); else if (!hidden[c.attr]) cbBody.append(row('xf:' + c.attr, c.label, o, c.attr, c.opts || {})); } // rigging: extra (deformer / IK / constraint / blend shape) attributes
    if (App.prefs.opt.cbShapes !== false && o.inca.shapeName) {
      cbBody.append(h('div', { class: 'cb-sect', text: 'SHAPES' }), h('div', { class: 'cb-node', text: o.inca.shapeName }));
      if (o.inca.light) {
        const L = o.inca.light;
        for (const [a, l] of Object.entries(LIGHT_CH)) {
          if (a === 'coneAngle' || a === 'penumbraAngle' || a === 'dropoff') { if (L.type !== 'spotLight') continue; }
          if (a === 'decayRate' && !(L.type === 'pointLight' || L.type === 'spotLight' || L.type === 'areaLight')) continue;
          if (a === 'shadows' && L.type === 'ambientLight') continue;
          cbBody.append(row('shape:' + a, l, o, a, a === 'decayRate' ? { type: 'enum', options: ['No Decay', 'Linear', 'Quadratic', 'Cubic'] } : a === 'shadows' ? { type: 'bool' } : {}));
        }
      } else if (o.inca.cam) { for (const [a, l] of Object.entries(CAM_CH)) cbBody.append(row('shape:' + a, l, o, a)); }
      else if (o.inca.kind === 'mesh') cbBody.append(row('shape:smoothLevel', 'Smooth Level', o, 'smoothLevel', { type: 'enum', options: ['Rough (1)', 'Cage+Smooth (2)', 'Smooth (3)'] }));
    }
    if (App.prefs.opt.cbInputs !== false && o.inca.history && o.inca.history.length) {
      const vis = [...o.inca.history].reverse().filter(hh => OPS[hh.type] && !OPS[hh.type].hidden);
      if (vis.length) {
        cbBody.append(h('div', { class: 'cb-sect', text: 'INPUTS' }));
        for (const hh of vis) {
          const open = st.openHistory.has(hh.id);
          const nd = h('div', { class: 'cb-node' + (open ? ' open' : ''), title: hh._error ? 'Error: ' + hh._error : '' }, h('span', { class: 'tri', text: open ? '▼' : '▶' }), hh.name, hh._error ? h('span', { style: { color: '#f77', marginLeft: '6px' }, text: '⚠' }) : null);
          nd.addEventListener('click', () => { if (open) st.openHistory.delete(hh.id); else st.openHistory.add(hh.id); st.sig = ''; refreshAll(); App.emit('inViewEditor', open ? null : hh); });
          cbBody.append(nd);
          if (open) for (const ch of OPS[hh.type].channels || []) cbBody.append(row('h:' + hh.id + ':' + ch.k, ch.label || niceName(ch.k), hh, ch.k, ch));
        }
      }
    }
  }
  function refreshValues() { for (const r of st.fields) r._upd(); }
  function refreshAll() { const sig = signature(); if (sig !== st.sig) { st.sig = sig; rebuild(); } else refreshValues(); }
  App.on('selectionChanged', () => { st.selChannels = new Set([...st.selChannels].filter(c => !c.startsWith('h:'))); });

  // ------------------------------------------------------------ layer editor
  const leTabs = h('div', { class: 'le-tabs' }, h('div', { class: 'le-tab active', text: 'Display' }), h('div', { class: 'le-tab', text: 'Anim', title: 'Animation layers are not available yet' }));
  const leMenu = h('div', { class: 'pmenubar' });
  const leTool = h('div', { class: 'ptoolbar' });
  const leList = h('div', { class: 'pbody' });
  le.append(leTabs, leMenu, leTool, leList);
  const selLayers = new Set();
  menuBar(leMenu, [
    { label: 'Layers', items: () => [
      { label: 'Create Empty Layer', fn: () => newLayer(false) }, { label: 'Create Layer from Selected', fn: () => newLayer(true) },
      { label: 'Select Objects in Selected Layers', fn: selectInLayers }, { label: 'Remove Selected Objects from Layers', fn: removeFromLayers },
      '-', { label: 'Delete Selected Layers', fn: deleteLayers }, { label: 'Delete Unused Layers', fn: () => { Undo.checkpoint('delete layers'); const used = new Set(S.allDag().map(o => o.inca.layer)); App.layers = App.layers.filter(l => used.has(l.id)); App.dirty('layers'); } },
      '-', { label: 'Set All Layers', sub: [{ label: 'Visible', fn: () => setAll('visible', true) }, { label: 'Invisible', fn: () => setAll('visible', false) }, { label: 'Normal', fn: () => setAll('mode', 'normal') }, { label: 'Template', fn: () => setAll('mode', 'template') }, { label: 'Reference', fn: () => setAll('mode', 'reference') }] },
    ] },
    { label: 'Options', items: [{ label: 'Make New Layers Current', check: () => !!App.prefs.opt.layerCurrent, fn: () => { App.prefs.opt.layerCurrent = !App.prefs.opt.layerCurrent; App.savePrefs(); } }, { label: 'Show Namespace', enabled: () => false }] },
    { label: 'Help', items: [{ label: 'Help on Display Layers', fn: () => App.help('Display layers group objects to toggle visibility (V) and display type: T template, R reference') }] },
  ]);
  leTool.append(h('div', { class: 'spacer' }), iconBtn('layerNew', 'Create a new layer', () => newLayer(false)), iconBtn('layerFromSel', 'Create a new layer and assign selected objects', () => newLayer(true)));
  function newLayer(fromSel) {
    Undo.checkpoint('create layer');
    let i = 1; while (App.layers.some(l => l.name === 'layer' + i)) i++;
    const L = { id: App.newId('L'), name: 'layer' + i, visible: true, mode: 'normal', color: 0, playback: true };
    App.layers.unshift(L);
    if (fromSel) for (const o of App.sel) { o.inca.layer = L.id; S.applyVisibility(o); }
    App.emit('echo', fromSel ? `createDisplayLayer -name "${L.name}" -number 1 -nr;` : `createDisplayLayer -name "${L.name}" -number 1 -empty;`);
    App.dirty('layers');
  }
  const membersOf = (L) => S.allDag().filter(o => o.inca.layer === L.id);
  function selectInLayers() { const out = []; for (const L of App.layers) if (selLayers.has(L.id)) out.push(...membersOf(L)); Sel.select(out); }
  function removeFromLayers() { Undo.checkpoint('remove from layer'); for (const o of App.sel) { o.inca.layer = null; S.applyVisibility(o); } App.dirty('layers'); App.requestRender(); }
  function deleteLayers() { Undo.checkpoint('delete layer'); for (const L of App.layers.filter(l => selLayers.has(l.id))) { for (const o of membersOf(L)) { o.inca.layer = null; S.applyVisibility(o); } } App.layers = App.layers.filter(l => !selLayers.has(l.id)); selLayers.clear(); App.dirty('layers'); App.requestRender(); }
  function setAll(k, v) { Undo.checkpoint('layers'); for (const L of App.layers) { L[k] = v; for (const o of membersOf(L)) S.applyVisibility(o); } App.dirty('layers'); App.requestRender(); }
  function renderLayers() {
    leList.innerHTML = '';
    for (const L of App.layers) {
      const vis = h('div', { class: 'box', text: L.visible ? 'V' : '', title: 'Toggle visibility' });
      vis.addEventListener('click', () => { Undo.checkpoint('layer visibility'); L.visible = !L.visible; for (const o of membersOf(L)) S.applyVisibility(o); App.dirty('layers'); App.requestRender(); });
      const mode = h('div', { class: 'box', text: L.mode === 'template' ? 'T' : L.mode === 'reference' ? 'R' : '', title: 'Display type: normal / template / reference' });
      mode.addEventListener('click', () => { Undo.checkpoint('layer type'); L.mode = L.mode === 'normal' ? 'template' : L.mode === 'template' ? 'reference' : 'normal'; if (L.mode !== 'normal') Sel.select(App.sel.filter(o => o.inca.layer !== L.id), 'replace', { echo: false }); App.dirty('layers'); App.requestRender(); });
      const c = LAYER_COLORS[L.color || 0];
      const sw = h('div', { class: 'swatch', style: { background: c ? colorToCss(c) : 'transparent' }, title: 'Layer color' });
      sw.addEventListener('click', (e) => showMenu(LAYER_COLORS.map((cc, i) => ({ label: i === 0 ? 'None' : 'Color ' + i, icon: null, fn: () => { L.color = i; App.dirty('layers'); App.requestRender(); } })), e.clientX, e.clientY));
      const nm = h('div', { class: 'lname', text: L.name });
      const r = h('div', { class: 'le-row' + (selLayers.has(L.id) ? ' sel' : '') }, vis, mode, sw, nm);
      r.addEventListener('mousedown', (e) => { if (e.target === vis || e.target === mode || e.target === sw) return; if (e.ctrlKey || e.shiftKey) { selLayers.has(L.id) ? selLayers.delete(L.id) : selLayers.add(L.id); } else { selLayers.clear(); selLayers.add(L.id); } renderLayers(); });
      r.addEventListener('dblclick', async (e) => { if (e.target !== nm) return; const n = await promptDialog('Edit Layer', 'Name:', L.name); if (n) { Undo.checkpoint('rename layer'); L.name = n.replace(/[^A-Za-z0-9_]/g, '_'); App.dirty('layers'); } });
      r.addEventListener('contextmenu', (e) => { e.preventDefault(); if (!selLayers.has(L.id)) { selLayers.clear(); selLayers.add(L.id); renderLayers(); }
        showMenu([{ label: 'Add Selected Objects', fn: () => { Undo.checkpoint('add to layer'); for (const o of App.sel) { o.inca.layer = L.id; S.applyVisibility(o); } App.dirty('layers'); App.requestRender(); } }, { label: 'Remove Selected Objects', fn: removeFromLayers }, '-', { label: 'Select Objects', fn: selectInLayers }, '-', { label: 'Template', check: () => L.mode === 'template', fn: () => { L.mode = L.mode === 'template' ? 'normal' : 'template'; App.dirty('layers'); App.requestRender(); } }, { label: 'Reference', check: () => L.mode === 'reference', fn: () => { L.mode = L.mode === 'reference' ? 'normal' : 'reference'; App.dirty('layers'); App.requestRender(); } }, '-', { label: 'Empty the Layer', fn: () => { Undo.checkpoint('empty layer'); for (const o of membersOf(L)) { o.inca.layer = null; S.applyVisibility(o); } App.dirty('layers'); } }, { label: 'Delete Layer', fn: deleteLayers }, '-', { label: 'Rename...', fn: async () => { const n = await promptDialog('Edit Layer', 'Name:', L.name); if (n) { L.name = n; App.dirty('layers'); } } }], e.clientX, e.clientY); });
      leList.append(r);
    }
  }
  return {
    refresh(d) {
      if (!d || d.has('all') || d.has('channels') || d.has('attr') || d.has('outliner') || d.has('timeline')) refreshAll();
      if (!d || d.has('all') || d.has('layers')) renderLayers();
    },
    show() { st.sig = ''; },
  };
}
function keySelected() {
  const chans = [...st.selChannels]; if (!chans.length) return App.cmds.run('setKey');
  Undo.checkpoint('key selected');
  for (const [rec, a] of targetsFor(chans)) if (rec.isObject3D) App.anim.setKey(rec, a);
  App.emit('echo', 'setKeyframe -breakdown 0 -hierarchy none -controlPoints 0 -shape 0 -at ' + chans.map(c => c.split(':')[1]).join(' -at ') + ';');
  App.dirty('channels', 'timeline', 'graph');
}
function deleteKeysSel() { Undo.checkpoint('delete keys'); for (const [rec, a] of targetsFor([...st.selChannels])) if (rec.isObject3D) App.anim.deleteCurve(rec, a); App.dirty('channels', 'timeline', 'graph'); }
function lockSel(on) { Undo.checkpoint(on ? 'lock' : 'unlock'); for (const [rec, a] of targetsFor([...st.selChannels])) if (rec.isObject3D) rec.inca.locks[a] = on; App.dirty('channels'); }
function hideSel() { Undo.checkpoint('hide channels'); for (const [rec, a] of targetsFor([...st.selChannels])) if (rec.isObject3D) { rec.inca.hiddenCh = rec.inca.hiddenCh || {}; rec.inca.hiddenCh[a] = true; } st.selChannels.clear(); App.dirty('channels'); }

registerPanel('channels', 'Channel Box / Layer Editor', build, { short: 'Channel Box / Layer Editor' });
