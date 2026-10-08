// Inca — status line and tool box
import { App } from '../core/app.js';
import * as S from '../core/scene.js';
import { Sel } from '../core/selection.js';
import { Undo } from '../core/undo.js';
import { Manip, placeProxy } from '../core/manip.js';
import { h, iconBtn, showMenu } from './dom.js';
import { icon } from './icons.js';

export const MENU_SETS = [['modeling', 'Modeling'], ['rigging', 'Rigging'], ['animation', 'Animation'], ['fx', 'FX'], ['rendering', 'Rendering']];
App.selectMode = 'object';

function buildStatusline() {
  const sl = document.getElementById('statusline');
  const ms = h('select', { title: 'Menu set: changes the menus to the right of Windows (F2 Modeling, F3 Rigging, F4 Animation, F5 FX, F6 Rendering)' }, MENU_SETS.map(([k, l]) => h('option', { value: k, text: l })));
  ms.value = App.prefs.menuSet || 'modeling';
  ms.addEventListener('change', () => App.ui.setMenuSet(ms.value));
  const sep = () => h('div', { class: 'sl-sep' });
  const grp = (...k) => h('div', { class: 'sl-group' }, ...k);
  const B = (ic, title, fn) => iconBtn(ic, title, fn);
  const tog = (ic, title, get, set) => { const b = B(ic, title, () => { set(); App.dirty('statusline'); }); b._get = get; return b; };
  const toggles = [];
  const T = (...a) => { const b = tog(...a); toggles.push(b); return b; };
  const modeH = T('selHierarchy', 'Select by hierarchy and combinations', () => App.selectMode === 'hierarchy', () => { App.selectMode = 'hierarchy'; if (App.compMode) Sel.setMode(null); });
  const modeO = T('selObject', 'Select by object type (F8)', () => App.selectMode === 'object' && !App.compMode, () => { App.selectMode = 'object'; if (App.compMode) Sel.setMode(null); placeProxy(); });
  const modeC = T('selComponent', 'Select by component type (F8)', () => !!App.compMode, () => { if (!App.compMode) Sel.setMode(App.lastCompMode || 'vertex'); placeProxy(); });
  // selection masks (object types)
  App.selMask = App.selMask || { mesh: true, curve: true, light: true, camera: true, locator: true, group: true };
  const mask = (ic, title, k) => T(ic, title, () => App.selMask[k] !== false, () => { App.selMask[k] = App.selMask[k] === false; });
  const masks = grp(mask('olCurve', 'Select curves', 'curve'), mask('olMesh', 'Select surfaces (polygons & NURBS)', 'mesh'), mask('olLight', 'Select lights', 'light'), mask('olCamera', 'Select cameras', 'camera'), mask('olLocator', 'Select locators', 'locator'));
  // component masks shown in component mode
  const compMasks = grp(...[['vertexMode', 'polySphere', 'Vertex (F9)', 'vertex'], ['edgeMode', 'multiCut', 'Edge (F10)', 'edge'], ['faceMode', 'polyPlane', 'Face (F11)', 'face'], ['uvMode', 'uvEditor', 'UV (F12)', 'uv']].map(([cmd, ic, t, m]) => T(ic, t, () => App.compMode === m, () => App.cmds.run(cmd))));
  const lockSel = T('isolate', 'Lock / unlock current selection', () => !!App.lockSelection, () => { App.lockSelection = !App.lockSelection; });
  // snapping
  const snaps = grp(
    T('snapGrid', 'Snap to grids (hold X)', () => Manip.snapGrid || Manip._keyGrid, () => { Manip.snapGrid = !Manip.snapGrid; }),
    T('snapCurve', 'Snap to curves (hold C)', () => Manip.snapCurve || Manip._keyCurve, () => { Manip.snapCurve = !Manip.snapCurve; }),
    T('snapPoint', 'Snap to points (hold V)', () => Manip.snapPoint || Manip._keyPoint, () => { Manip.snapPoint = !Manip.snapPoint; }),
    T('snapSurface', 'Make the selected object live (draw curves on its surface)', () => !!App.liveSurface, () => { const o = Sel.lead(); App.liveSurface = App.liveSurface ? null : (o && o.inca.kind === 'mesh' ? o : null); if (!App.liveSurface && !o) App.help('Select a polygon object to make live'); App.requestRender(); }),
  );
  const hist = T('deleteHistory', 'Construction history on/off', () => App.prefs.constructionHistory !== false, () => { App.prefs.constructionHistory = App.prefs.constructionHistory === false; App.savePrefs(); App.help('Construction history ' + (App.prefs.constructionHistory === false ? 'off' : 'on')); });
  const render = grp(B('renderView', 'Open Render View', () => App.cmds.run('renderView')), B('renderFrame', 'Render the current frame', () => App.cmds.run('renderCurrent')), B('ipr', 'IPR render the current frame', () => App.cmds.run('ipr')), B('renderSettings', 'Display render settings', () => App.cmds.run('renderSettings')), B('hypershade', 'Display Hypershade window', () => App.cmds.run('hypershade')));
  // numeric input field
  const inMode = h('select', { style: { width: '74px' }, title: 'Input line mode' }, ['Absolute', 'Relative', 'Rename', 'Select'].map(m => h('option', { text: m })));
  const inField = h('input', { class: 'sl-field', style: { width: '150px' }, placeholder: 'x y z  or name', title: 'Absolute/relative transform for the selection, rename the selection or select by name (wildcards *)' });
  inField.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key !== 'Enter') return; doInput(inMode.value, inField.value.trim()); inField.value = ''; inField.blur(); });
  const sym = h('select', { style: { width: '108px' }, title: 'Symmetry' }, ['Symmetry: Off', 'Object X', 'World X', 'World Y', 'World Z'].map(s => h('option', { text: s })));
  sym.addEventListener('change', () => { Manip.symmetry = ['off', 'objectX', 'worldX', 'worldY', 'worldZ'][sym.selectedIndex]; App.help('Symmetry: ' + sym.value); });
  const panels = grp(
    T('modelingToolkit', 'Show or hide the Modeling Toolkit', () => App.ui.rightPanel?.() === 'mtk', () => App.ui.toggleRightPanel('mtk')),
    T('attrEditor', 'Show or hide the Attribute Editor (Ctrl+A)', () => App.ui.rightPanel?.() === 'attr', () => App.ui.toggleRightPanel('attr')),
    T('toolSettings', 'Show or hide the Tool Settings', () => App.ui.leftPanelOpen?.('toolSettings'), () => App.ui.toggleToolSettings()),
    T('channelBox', 'Show or hide the Channel Box / Layer Editor', () => App.ui.rightPanel?.() === 'channels', () => App.ui.toggleRightPanel('channels')),
  );
  sl.append(ms, sep(), grp(B('newScene', 'Create a new scene (Ctrl+N)', () => App.cmds.run('newScene')), B('openScene', 'Open a scene (Ctrl+O)', () => App.cmds.run('openScene')), B('saveScene', 'Save the current scene (Ctrl+S)', () => App.cmds.run('saveScene'))), sep(),
    grp(B('undo', 'Undo the last action (Z / Ctrl+Z)', () => App.cmds.run('undo')), B('redo', 'Redo the last undone action (Shift+Z / Ctrl+Y)', () => App.cmds.run('redo'))), sep(),
    grp(modeH, modeO, modeC), sep(), masks, compMasks, sep(), lockSel, sep(), snaps, sep(), hist, sep(), render, sep(), inMode, inField, sep(), sym, h('div', { class: 'spacer' }), panels);
  App.on('refresh', (d) => { if (d.has('statusline') || d.has('menus') || d.has('panelUI') || d.has('outliner')) sync(); });
  App.on('selectionChanged', sync); App.on('toolChanged', sync);
  App.ui.syncStatusline = sync;
  function sync() {
    for (const b of toggles) b.classList.toggle('on', !!b._get());
    masks.classList.toggle('hidden', !!App.compMode); compMasks.classList.toggle('hidden', !App.compMode);
    if (ms.value !== App.prefs.menuSet) ms.value = App.prefs.menuSet;
    sym.selectedIndex = Math.max(0, ['off', 'objectX', 'worldX', 'worldY', 'worldZ'].indexOf(Manip.symmetry || 'off'));
  }
  sync();
}
function doInput(mode, txt) {
  if (!txt) return;
  if (mode === 'Rename') { const o = Sel.lead(); if (!o) return; Undo.checkpoint('rename'); S.rename(o, txt); App.emit('echo', `rename "${txt}";`); return; }
  if (mode === 'Select') { const re = new RegExp('^' + txt.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*').replace(/\?/g, '.') + '$'); Sel.select(S.allDag().filter(o => re.test(o.inca.name))); return; }
  const v = txt.split(/[\s,]+/).map(Number); if (v.some(isNaN) || !v.length) return App.emit('warning', '// Warning: Enter numbers: x y z');
  const k = Manip.mode === 'rotate' ? 'r' : Manip.mode === 'scale' ? 's' : 't';
  const vec = v.length === 1 ? [v[0], v[0], v[0]] : [v[0], v[1] ?? 0, v[2] ?? 0];
  Undo.checkpoint('input line');
  for (const o of App.sel) { o.inca[k] = mode === 'Relative' ? o.inca[k].map((x, i) => k === 's' ? x * vec[i] : x + vec[i]) : vec.slice(); S.updateXform(o); }
  App.emit('echo', `${k === 't' ? 'move' : k === 'r' ? 'rotate' : 'scale'} ${mode === 'Relative' ? '-r ' : ''}${vec.join(' ')};`);
  placeProxy(); App.dirty('channels'); App.requestRender();
}

// ------------------------------------------------------------------ tool box
function buildToolbox() {
  const tb = document.getElementById('toolbox');
  const tools = [['select', 'select', 'Select Tool (Q)'], ['lasso', 'lasso', 'Lasso Select Tool'], ['paint', 'paintSelect', 'Paint Selection Tool'], ['move', 'move', 'Move Tool (W)'], ['rotate', 'rotate', 'Rotate Tool (E)'], ['scale', 'scale', 'Scale Tool (R)']];
  const btns = tools.map(([t, ic, title]) => { const b = iconBtn(ic, title, () => App.setTool(t)); b._t = t; b.addEventListener('dblclick', () => App.ui.toggleToolSettings(true)); return b; });
  const last = iconBtn('lastTool', 'Last tool used (Y)', () => App.cmds.run('lastToolUsed'));
  last.addEventListener('dblclick', () => App.ui.toggleToolSettings(true));
  const L = (ic, title, fn) => { const b = iconBtn(ic, title, fn, 'ib layout-btn'); return b; };
  const layouts = [
    L('layoutSingle', 'Single Perspective View', () => { App.ui.toggleOutlinerDock(false); App.setLayout('single', 'persp'); }),
    L('layoutFour', 'Four View', () => App.setLayout('four')),
    L('layoutOutliner', 'Perspective / Outliner', () => { App.setLayout('single', 'persp'); App.ui.toggleOutlinerDock(true); }),
    L('layoutTwo', 'Perspective / Graph Editor', () => { App.setLayout('single', 'persp'); App.cmds.run('graphEditor'); }),
    L('outliner', 'Outliner', () => App.ui.toggleOutlinerDock()),
  ];
  tb.append(...btns, last, h('div', { class: 'tb-sep' }), h('div', { class: 'spacer' }), ...layouts, h('div', { style: { height: '6px' } }));
  const sync = () => { for (const b of btns) b.classList.toggle('on', App.tool === b._t); last.classList.toggle('on', !!App.tools?.isInteractive?.(App.tool)); last.innerHTML = icon(App.tools?.isInteractive?.(App.tool) ? (App.tools.iconOf?.(App.tool) || 'lastTool') : (App.lastTool || 'lastTool')); };
  App.on('toolChanged', sync); sync();
}
App.on('beforeUI', () => { buildStatusline(); buildToolbox(); });
