// Inca — global application state + event bus
export const App = {
  version: '0.1.0',
  scene: null,        // THREE.Scene
  world: null,        // root group for DAG nodes
  nodes: new Map(),   // id -> Object3D (DAG) or record (DG)
  names: new Map(),   // name -> id
  sel: [],            // selected DAG objects in order (last = lead)
  compMode: null,     // null | 'vertex' | 'edge' | 'face' | 'uv'
  hilite: [],         // objects in component mode
  mats: new Map(),
  texs: new Map(),
  layers: [],
  time: { current: 1, start: 1, end: 120, animStart: 1, animEnd: 200, fps: 24, playing: false, loop: 'continuous', autoKey: false, playEvery: false },
  tool: 'select',
  lastTool: 'move',
  sceneFile: null,
  sceneName: 'untitled',
  modified: false,
  project: null,
  prefs: null,
  viewports: [],
  activeViewport: null,
  idCounter: 1,
  _ev: new Map(),
  on(ev, fn) { if (!this._ev.has(ev)) this._ev.set(ev, new Set()); this._ev.get(ev).add(fn); return () => this._ev.get(ev).delete(fn); },
  emit(ev, data) { const s = this._ev.get(ev); if (s) for (const fn of [...s]) { try { fn(data); } catch (e) { console.error('event', ev, e); } } },
  _dirty: new Set(), _raf: 0,
  dirty(...flags) {
    for (const f of flags) this._dirty.add(f);
    this.requestRender();
    if (!this._raf) this._raf = requestAnimationFrame(() => {
      this._raf = 0; const d = new Set(this._dirty); this._dirty.clear();
      this.emit('refresh', d);
    });
  },
  requestRender() { for (const vp of this.viewports) vp.needsRender = true; },
  newId(prefix = 'n') { return prefix + (this.idCounter++); },
  log: (...a) => console.log(...a),
  help(msg) { const el = document.getElementById('helpline-text'); if (el) el.textContent = msg || ''; },
};
window.App = App;
