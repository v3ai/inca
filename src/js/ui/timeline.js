// Inca — Time Slider, Range Slider and playback controls
import { App } from '../core/app.js';
import { Undo } from '../core/undo.js';
import { h, iconBtn, showMenu, fmt } from './dom.js';
import { TANGENTS } from '../core/anim.js';

const FPS = [['15 fps', 15], ['game (15 fps)', 15], ['film (24 fps)', 24], ['PAL (25 fps)', 25], ['NTSC (30 fps)', 30], ['show (48 fps)', 48], ['PAL Field (50 fps)', 50], ['NTSC Field (60 fps)', 60], ['120 fps', 120]].filter((x, i) => i !== 1);
const T = App.time;
let play = { on: false, dir: 1, last: 0, acc: 0 };
let rangeSel = null; // [a,b] shift-drag time selection

App.timeline = {
  togglePlay(dir = 1) {
    if (play.on && play.dir === dir) return this.stop();
    play = { on: true, dir, last: performance.now(), acc: 0, frames: 0, t0: performance.now() }; T.playing = true;
    App.dirty('timeline'); App.emit('playback', true);
  },
  stop() { if (!play.on) return; play.on = false; T.playing = false; App.setTime(Math.round(T.current)); App.dirty('timeline'); App.emit('playback', false); },
  get playing() { return play.on; },
  tick() {
    if (!play.on) return;
    const now = performance.now(); const dt = (now - play.last) / 1000; play.last = now;
    const speed = App.prefs.playbackSpeed ?? 1;
    let step;
    if (speed === 0 || T.playEvery) step = 1; // play every frame
    else { play.acc += dt * T.fps * speed; step = Math.floor(play.acc); play.acc -= step; if (!step) return; }
    let t = T.current + step * play.dir;
    const loop = T.loop || 'continuous';
    if (t > T.end) { if (loop === 'once') { App.setTime(T.end); return this.stop(); } if (loop === 'oscillate') { play.dir = -1; t = T.end - (t - T.end); } else t = T.start + ((t - T.start) % Math.max(1, T.end - T.start + 1)); }
    if (t < T.start) { if (loop === 'once') { App.setTime(T.start); return this.stop(); } if (loop === 'oscillate') { play.dir = 1; t = T.start + (T.start - t); } else t = T.end - ((T.start - t - 1) % Math.max(1, T.end - T.start + 1)); }
    App.setTime(Math.round(t));
  },
};

function buildTimeline() {
  // ---------------------------------------------------------------- time slider
  const ts = document.getElementById('timeslider');
  const wrap = h('div', { id: 'ts-canvas-wrap' }); const cv = h('canvas', { id: 'ts-canvas' }); wrap.append(cv);
  const cur = h('input', { title: 'Current time' });
  cur.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') cur.blur(); });
  cur.addEventListener('change', () => { const v = parseFloat(cur.value); if (!isNaN(v)) App.setTime(v); });
  const btns = [
    iconBtn('goStart', 'Go to start of playback range (Shift+Alt+V)', () => App.cmds.run('goToStart')),
    iconBtn('stepBack', 'Step back one frame (Alt+,)', () => App.cmds.run('prevFrame')),
    iconBtn('keyBack', 'Step back one key (,)', () => App.cmds.run('prevKey')),
    iconBtn('playBack', 'Play backwards', () => App.timeline.togglePlay(-1)),
    iconBtn('playFwd', 'Play forwards (Alt+V). Press Esc to stop', () => App.timeline.togglePlay(1)),
    iconBtn('keyFwd', 'Step forward one key (.)', () => App.cmds.run('nextKey')),
    iconBtn('stepFwd', 'Step forward one frame (Alt+.)', () => App.cmds.run('nextFrame')),
    iconBtn('goEnd', 'Go to end of playback range', () => App.cmds.run('goToEnd')),
  ];
  btns.forEach(b => b.classList.add('small'));
  ts.append(wrap, h('div', { id: 'ts-right' }, cur, ...btns));

  // ---------------------------------------------------------------- range slider
  const rs = document.getElementById('rangeslider');
  const fld = (get, set, title) => { const i = h('input', { title }); i.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') i.blur(); }); i.addEventListener('change', () => { const v = parseFloat(i.value); if (!isNaN(v)) { set(v); App.modified = true; App.dirty('timeline', 'title'); } }); i._get = get; return i; };
  const fAS = fld(() => T.animStart, (v) => { T.animStart = v; if (T.start < v) T.start = v; }, 'Animation start time');
  const fS = fld(() => T.start, (v) => { T.start = Math.min(v, T.end); if (T.animStart > T.start) T.animStart = T.start; }, 'Playback start time');
  const fE = fld(() => T.end, (v) => { T.end = Math.max(v, T.start); if (T.animEnd < T.end) T.animEnd = T.end; }, 'Playback end time');
  const fAE = fld(() => T.animEnd, (v) => { T.animEnd = v; if (T.end > v) T.end = v; }, 'Animation end time');
  const bar = h('div', { id: 'rs-bar' }); const hnd = h('div', { id: 'rs-handle' }); const gl = h('div', { class: 'grip' }), gr = h('div', { class: 'grip' }); const hl = h('span'), hr = h('span');
  hnd.append(gl, hl, h('span', { class: 'spacer' }), hr, gr); bar.append(hnd);
  const fpsSel = h('select', { title: 'Playback frame rate' }, FPS.map(([l, v]) => h('option', { text: l, value: v })));
  fpsSel.addEventListener('change', () => { Undo.checkpoint('fps'); T.fps = +fpsSel.value; App.modified = true; App.dirty('timeline', 'title'); App.emit('echo', `currentUnit -time ${{ 24: 'film', 25: 'pal', 30: 'ntsc', 48: 'show', 50: 'palf', 60: 'ntscf' }[T.fps] || T.fps + 'fps'};`); });
  const loopBtn = iconBtn('loop', 'Looping: continuous / once / oscillate', () => { T.loop = { continuous: 'once', once: 'oscillate', oscillate: 'continuous' }[T.loop || 'continuous']; App.help('Playback looping: ' + T.loop); syncRS(); });
  const charSel = h('select', { title: 'Current character set', style: { width: '120px' } }, h('option', { text: 'No Character Set' }));
  const layerSel = h('select', { title: 'Current animation layer', style: { width: '110px' } }, h('option', { text: 'No Anim Layer' }));
  const autoKey = iconBtn('autoKey', 'Toggle auto keyframe', () => App.cmds.run('toggleAutoKey'));
  const prefsBtn = iconBtn('playPrefs', 'Animation preferences', () => App.ui.preferences?.('Time Slider'));
  const speaker = iconBtn('speaker', 'Sound (no audio in scene)', () => App.help('Import audio is not supported yet'));
  rs.append(fAS, fS, bar, fE, fAE, fpsSel, loopBtn, charSel, layerSel, autoKey, speaker, prefsBtn);
  for (const b of [loopBtn, autoKey, speaker, prefsBtn]) b.classList.add('small');
  // drag range bar
  const rsDrag = (mode) => (e) => {
    e.preventDefault(); e.stopPropagation();
    const r = bar.getBoundingClientRect(); const span = (T.animEnd - T.animStart) || 1; const x0 = e.clientX; const s0 = T.start, e0 = T.end;
    const mm = (ev) => { const d = Math.round((ev.clientX - x0) / r.width * span);
      if (mode === 'move') { let ns = s0 + d, ne = e0 + d; if (ns < T.animStart) { ne += T.animStart - ns; ns = T.animStart; } if (ne > T.animEnd) { ns -= ne - T.animEnd; ne = T.animEnd; } T.start = ns; T.end = ne; }
      else if (mode === 'l') T.start = Math.max(T.animStart, Math.min(e0 - 1, s0 + d)); else T.end = Math.min(T.animEnd, Math.max(s0 + 1, e0 + d));
      App.dirty('timeline'); };
    const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); };
    addEventListener('mousemove', mm); addEventListener('mouseup', mu);
  };
  hnd.addEventListener('mousedown', rsDrag('move')); gl.addEventListener('mousedown', rsDrag('l')); gr.addEventListener('mousedown', rsDrag('r'));
  hnd.addEventListener('dblclick', () => { if (T.start === T.animStart && T.end === T.animEnd) { [T.start, T.end] = App._prevRange || [T.start, T.end]; } else { App._prevRange = [T.start, T.end]; T.start = T.animStart; T.end = T.animEnd; } App.dirty('timeline'); });
  function syncRS() {
    for (const f of [fAS, fS, fE, fAE]) if (document.activeElement !== f) f.value = fmt(f._get());
    const span = (T.animEnd - T.animStart) || 1;
    hnd.style.left = ((T.start - T.animStart) / span * 100) + '%'; hnd.style.width = Math.max(2, (T.end - T.start) / span * 100) + '%';
    hl.textContent = fmt(T.start); hr.textContent = fmt(T.end);
    fpsSel.value = String(T.fps); if (!FPS.some(f => f[1] === T.fps)) fpsSel.selectedIndex = -1;
    autoKey.classList.toggle('on', !!T.autoKey); autoKey.style.color = T.autoKey ? '#f55' : '';
    loopBtn.title = 'Looping: ' + (T.loop || 'continuous');
    btns[4].classList.toggle('on', play.on && play.dir > 0); btns[3].classList.toggle('on', play.on && play.dir < 0);
  }

  // ---------------------------------------------------------------- slider drawing
  const ctx = cv.getContext('2d');
  function frameToX(f, W) { const pad = 8; return pad + (f - T.start) / Math.max(1, T.end - T.start) * (W - 2 * pad); }
  function xToFrame(x, W) { const pad = 8; return T.start + (x - pad) / (W - 2 * pad) * Math.max(1, T.end - T.start); }
  function draw() {
    const dpr = devicePixelRatio || 1; const W = wrap.clientWidth, H = wrap.clientHeight; if (!W) return;
    if (cv.width !== W * dpr || cv.height !== H * dpr) { cv.width = W * dpr; cv.height = H * dpr; }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.fillStyle = '#444'; ctx.fillRect(0, 0, W, H);
    ctx.fillStyle = '#525252'; ctx.fillRect(0, 0, W, H);
    const span = Math.max(1, T.end - T.start); const pxPer = (W - 16) / span;
    // choose tick step
    const steps = [1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 5000];
    let major = steps.find(s => s * pxPer >= 45) || 10000; let minor = major / 5; if (minor < 1) minor = 1; if (minor * pxPer < 4) minor = major;
    // range selection
    if (rangeSel) { const a = frameToX(Math.min(...rangeSel), W), b = frameToX(Math.max(...rangeSel) + 1, W); ctx.fillStyle = 'rgba(200,50,50,0.55)'; ctx.fillRect(a, 0, Math.max(2, b - a), H); }
    ctx.strokeStyle = '#8a8a8a'; ctx.fillStyle = '#d0d0d0'; ctx.font = '10.5px Segoe UI, sans-serif'; ctx.textAlign = 'left'; ctx.lineWidth = 1;
    ctx.beginPath();
    const first = Math.ceil(T.start / minor) * minor;
    for (let f = first; f <= T.end + 1e-6; f += minor) { const x = Math.round(frameToX(f, W)) + 0.5; const isMaj = Math.abs(f / major - Math.round(f / major)) < 1e-6; ctx.moveTo(x, H); ctx.lineTo(x, H - (isMaj ? 14 : 6)); if (isMaj) ctx.fillText(fmt(f), x + 2, H - 18 + 6); }
    ctx.stroke();
    // keys of selected objects
    const keys = App.anim.keyTimes(App.sel);
    ctx.fillStyle = '#d73b3e';
    for (const k of keys) { if (k < T.start || k > T.end) continue; const x = frameToX(k, W); ctx.fillRect(Math.round(x) - 1, H - 22, Math.max(2, pxPer * 0.18), 22); }
    // current time marker
    const cx = frameToX(T.current, W);
    ctx.fillStyle = 'rgba(150,150,150,0.55)'; ctx.fillRect(cx - Math.max(1, pxPer / 2), 0, Math.max(2, pxPer), H);
    ctx.fillStyle = '#e8e8e8'; ctx.fillRect(Math.round(cx) - 1, 0, 2, H);
    const lab = fmt(T.current); ctx.font = 'bold 11px Segoe UI, sans-serif'; const tw = ctx.measureText(lab).width + 6;
    ctx.fillStyle = '#d9d9d9'; ctx.fillRect(Math.min(W - tw, cx + 2), 1, tw, 13); ctx.fillStyle = '#111'; ctx.fillText(lab, Math.min(W - tw, cx + 2) + 3, 11);
    if (document.activeElement !== cur) cur.value = fmt(T.current);
  }
  // scrubbing
  cv.addEventListener('mousedown', (e) => {
    e.preventDefault();
    const W = wrap.clientWidth; const r = cv.getBoundingClientRect();
    const at = (ev) => Math.round(Math.max(T.start, Math.min(T.end, xToFrame(ev.clientX - r.left, W))));
    if (e.button === 2) { ctxMenu(e, at(e)); return; }
    if (e.shiftKey && e.button === 0) {
      const a = at(e); rangeSel = [a, a]; draw();
      const mm = (ev) => { rangeSel[1] = at(ev); draw(); };
      const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); if (rangeSel[0] === rangeSel[1]) rangeSel = null; draw(); };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu); return;
    }
    if (e.button === 1 && rangeSel) { // MMB drag moves keys in the selected range
      const x0 = at(e); const lo = Math.min(...rangeSel), hi = Math.max(...rangeSel);
      const mm = (ev) => { const d = at(ev) - x0; rangeSel = [lo + d, hi + d]; draw(); };
      const mu = (ev) => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); const d = at(ev) - x0; if (d) moveKeysRange(lo, hi, d); };
      addEventListener('mousemove', mm); addEventListener('mouseup', mu); return;
    }
    if (e.button === 1) { // MMB: change time without updating the scene (for copying poses)
      const mm = (ev) => { T.current = at(ev); draw(); App.dirty('channels'); };
      const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); App.help('Time changed without updating the scene. Set a key to copy the pose.'); };
      mm(e); addEventListener('mousemove', mm); addEventListener('mouseup', mu); return;
    }
    rangeSel = null;
    if (play.on) App.timeline.stop();
    const mm = (ev) => { const f = at(ev); if (f !== T.current) App.setTime(f); };
    const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); };
    mm(e); addEventListener('mousemove', mm); addEventListener('mouseup', mu);
  });
  cv.addEventListener('contextmenu', e => e.preventDefault());
  function moveKeysRange(lo, hi, d) {
    Undo.checkpoint('move keys');
    for (const n of App.sel) for (const c of App.anim.curvesOf(n)) { const moving = c.keys.filter(k => k.t >= lo - 1e-4 && k.t <= hi + 1e-4); c.keys = c.keys.filter(k => !moving.includes(k) && !(k.t >= lo + d - 1e-4 && k.t <= hi + d + 1e-4)); for (const k of moving) { k.t += d; c.keys.push(k); } c.sort(); }
    App.anim.evaluate(); App.dirty('timeline', 'graph', 'channels');
  }
  function ctxMenu(e, f) {
    const range = rangeSel ? [Math.min(...rangeSel), Math.max(...rangeSel)] : [T.current, T.current];
    const inRange = (k) => k.t >= range[0] - 1e-4 && k.t <= range[1] + 1e-4;
    const keysIn = () => { const r = []; for (const n of App.sel) for (const c of App.anim.curvesOf(n)) for (const k of c.keys) if (inRange(k)) r.push(k); return r; };
    showMenu([
      { label: 'Undo', cmd: 'undo' }, { label: 'Redo', cmd: 'redo' }, '-',
      { label: 'Cut', fn: () => { App.anim.copyKeys(App.sel, rangeSel ? null : T.current); delKeys(); } },
      { label: 'Copy', fn: () => App.anim.copyKeys(App.sel, rangeSel ? null : T.current) },
      { label: 'Paste', sub: [{ label: 'Paste', fn: () => { Undo.checkpoint('paste keys'); App.anim.pasteKeys(App.sel, T.current); App.anim.evaluate(); } }] },
      { label: 'Delete', fn: () => delKeys() }, '-',
      { label: 'Set Key', cmd: 'setKey' }, { label: 'Key Translate', cmd: 'keyTranslate' }, { label: 'Key Rotate', cmd: 'keyRotate' }, { label: 'Key Scale', cmd: 'keyScale' }, '-',
      { label: 'Tangents', sub: TANGENTS.filter(t => t !== 'fixed').map(t => ({ label: t[0].toUpperCase() + t.slice(1), fn: () => { Undo.checkpoint('tangents'); for (const k of keysIn()) { k.it = t; k.ot = t; } App.anim.evaluate(); App.dirty('graph'); } })) },
      '-', { label: 'Playback Looping', sub: ['once', 'oscillate', 'continuous'].map(l => ({ label: l[0].toUpperCase() + l.slice(1), check: () => (T.loop || 'continuous') === l, fn: () => { T.loop = l; syncRS(); } })) },
      { label: 'Set Range To', sub: [{ label: 'Selected', enabled: () => !!rangeSel, fn: () => { T.start = range[0]; T.end = range[1]; App.dirty('timeline'); } }, { label: 'Sound Length', enabled: () => false }, { label: 'Keys of selected', fn: () => { const ks = App.anim.keyTimes(App.sel); if (ks.length) { T.start = ks[0]; T.end = ks[ks.length - 1]; App.dirty('timeline'); } } }] },
      { label: 'Playblast...', cmd: 'playblast' },
    ], e.clientX, e.clientY);
    function delKeys() { Undo.checkpoint('delete keys'); for (const n of App.sel) for (const c of App.anim.curvesOf(n)) { c.keys = c.keys.filter(k => !inRange(k)); if (!c.keys.length) App.anim.deleteCurve(n, c.attr); } App.anim.evaluate(); App.dirty('timeline', 'graph', 'channels'); }
  }
  new ResizeObserver(draw).observe(wrap);
  App.on('refresh', (d) => { if (d.has('timeline') || d.has('outliner') || d.has('channels')) { draw(); syncRS(); } });
  App.on('selectionChanged', draw);
  App.on('playback', syncRS);
  draw(); syncRS();
}
App.on('beforeUI', buildTimeline);
