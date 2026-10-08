// Inca — snapshot-based undo queue
import { App } from './app.js';
import { serializeScene, loadScene } from './scene.js';

const undoStack = []; const redoStack = [];
let suspended = 0;

function snapshot(label) {
  const state = serializeScene();
  state.compMode = App.compMode; state.hilite = App.hilite.map(o => o.inca.id);
  state.compSel = {}; for (const o of App.hilite) if (o.inca.compSel) state.compSel[o.inca.id] = { v: [...o.inca.compSel.v], e: [...o.inca.compSel.e], f: [...o.inca.compSel.f] };
  return { label, json: JSON.stringify(state) };
}
function restore(snap) {
  suspended++;
  try {
    const st = JSON.parse(snap.json);
    const cams = {}; // keep live camera views (camera moves are not undoable)
    for (const n of ['persp', 'top', 'front', 'side']) { const o = [...App.nodes.values()].find(x => x.isObject3D && x.inca.name === n); if (o) cams[n] = { t: o.inca.t.slice(), r: o.inca.r.slice(), cam: { ...o.inca.cam } }; }
    st.nodes.forEach(d => { if (cams[d.name] && d.cam && d.cam.startup) { d.t = cams[d.name].t; d.r = cams[d.name].r; d.cam = cams[d.name].cam; } });
    loadScene(st);
    App.compMode = st.compMode || null;
    App.hilite = (st.hilite || []).map(id => App.nodes.get(id)).filter(Boolean);
    for (const o of App.hilite) { const cs = st.compSel?.[o.inca.id]; if (cs && o.inca.compSel) o.inca.compSel = { v: new Set(cs.v), e: new Set(cs.e), f: new Set(cs.f) }; o.userData.compDirty = true; }
    App.emit('viewportsRebind');
    App.emit('selectionChanged');
  } finally { suspended--; }
}
export const Undo = {
  checkpoint(label = 'edit') {
    if (suspended) return;
    undoStack.push(snapshot(label));
    const max = App.prefs?.undoLevels ?? 50;
    while (undoStack.length > max) undoStack.shift();
    redoStack.length = 0;
    markModified();
  },
  undo() {
    if (!undoStack.length) { App.help('// Warning: There are no more commands to undo.'); return false; }
    const s = undoStack.pop(); redoStack.push(snapshot(s.label)); restore(s);
    App.help('// Undo: ' + s.label); App.emit('echo', '// Undo: ' + s.label); markModified(); return true;
  },
  redo() {
    if (!redoStack.length) { App.help('// Warning: There are no more commands to redo.'); return false; }
    const s = redoStack.pop(); undoStack.push(snapshot(s.label)); restore(s);
    App.help('// Redo: ' + s.label); App.emit('echo', '// Redo: ' + s.label); markModified(); return true;
  },
  clear() { undoStack.length = 0; redoStack.length = 0; },
  get canUndo() { return undoStack.length > 0; },
  get canRedo() { return redoStack.length > 0; },
  suspend(fn) { suspended++; try { return fn(); } finally { suspended--; } },
  get suspended() { return suspended > 0; },
  labels() { return undoStack.map(s => s.label); },
};
function markModified() { if (!App.modified) { App.modified = true; App.dirty('title'); } }
App.undo = Undo;
