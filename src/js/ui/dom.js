// Inca — small DOM toolkit: elements, popup menus, floating windows, dialogs, color picker
import { icon } from './icons.js';
import { App } from '../core/app.js';

export function h(tag, props = {}, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(props || {})) {
    if (v === undefined || v === null || v === false) continue;
    if (k === 'class') el.className = v;
    else if (k === 'style' && typeof v === 'object') Object.assign(el.style, v);
    else if (k === 'html') el.innerHTML = v;
    else if (k === 'text') el.textContent = v;
    else if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2).toLowerCase(), v);
    else if (k === 'value') el.value = v;
    else if (k === 'checked') el.checked = !!v;
    else el.setAttribute(k, v === true ? '' : v);
  }
  for (const c of children.flat(Infinity)) { if (c === null || c === undefined || c === false) continue; el.append(c.nodeType ? c : document.createTextNode(String(c))); }
  return el;
}
export const $ = (s, r = document) => r.querySelector(s);
export const $$ = (s, r = document) => [...r.querySelectorAll(s)];
export function iconBtn(name, title, onclick, cls = 'ib') {
  const b = h('div', { class: cls, title, html: icon(name) });
  if (onclick) b.addEventListener('click', onclick);
  b.addEventListener('mouseenter', () => App.help(title));
  return b;
}
export function fmt(n, d = 3) {
  if (typeof n !== 'number' || !isFinite(n)) return String(n);
  const r = Math.round(n * 1000) / 1000;
  return Object.is(r, -0) ? '0' : String(+r.toFixed(d));
}

// ---------------------------------------------------------------- popup menus
let openMenus = [];
let menuCloseCb = null;
export function closeMenus() {
  for (const m of openMenus) m.remove(); openMenus = [];
  document.querySelectorAll('.menu-title.open').forEach(e => e.classList.remove('open'));
  if (menuCloseCb) { const cb = menuCloseCb; menuCloseCb = null; cb(); }
}
export const menuIsOpen = () => openMenus.length > 0;
function resolveItems(items) { return typeof items === 'function' ? items() : items; }
export function showMenu(items, x, y, opts = {}) {
  if (!opts.sub) closeMenus();
  if (opts.onClose) menuCloseCb = opts.onClose;
  const list = resolveItems(items) || [];
  const el = h('div', { class: 'popup' });
  el.addEventListener('mousedown', e => e.stopPropagation());
  el.addEventListener('contextmenu', e => e.preventDefault());
  let subTimer = 0; let openSub = null;
  const level = opts.level || 0;
  const closeSub = () => { if (openSub) { const idx = openMenus.indexOf(openSub); if (idx >= 0) for (const m of openMenus.splice(idx)) m.remove(); openSub = null; } };
  for (const it of list) {
    if (!it) continue;
    if (it === '-') { el.append(h('div', { class: 'sep' })); continue; }
    if (it.section) { el.append(h('div', { class: 'sect', text: it.section })); continue; }
    const info = it.cmd && App.cmds ? App.cmds.info(it.cmd) : null;
    const label = it.label ?? (info ? info.label : '?');
    const hk = it.hk ?? (info ? info.hotkey : '');
    const en = it.enabled ? it.enabled() : true;
    const checked = it.check ? it.check() : null;
    const row = h('div', { class: 'mi' + (en ? '' : ' disabled') });
    row.append(h('span', { class: 'chk', text: checked === null ? '' : checked ? '✓' : '' }));
    if (it.icon || (info && info.icon && opts.icons !== false && it.showIcon)) row.append(h('span', { class: 'ico', html: icon(it.icon || info.icon) }));
    row.append(h('span', { class: 'lbl', text: label }));
    if (hk) row.append(h('span', { class: 'hk', text: hk }));
    const optFn = it.opt || (info && info.opt);
    if (optFn) {
      const ob = h('span', { class: 'opt', title: label + ' Options', html: icon('optionBox') });
      ob.addEventListener('mouseup', e => { e.stopPropagation(); closeMenus(); if (typeof optFn === 'function') optFn(); else App.cmds.option(it.cmd); });
      row.append(ob);
    } else if (!it.sub) row.append(h('span', { style: { width: '22px' } }));
    if (it.sub) row.append(h('span', { class: 'arrow', text: '▶' }));
    row.addEventListener('mouseenter', () => {
      clearTimeout(subTimer);
      subTimer = setTimeout(() => {
        closeSub();
        if (it.sub && en) {
          const r = row.getBoundingClientRect();
          openSub = showMenu(it.sub, r.right - 2, r.top - 4, { sub: true, level: level + 1 });
        }
      }, it.sub ? 120 : 200);
    });
    if (it.title || info?.help) row.addEventListener('mouseenter', () => App.help(it.title || info.help));
    row.addEventListener('mouseup', (e) => {
      if (!en || it.sub) return;
      if (e.button === 2 && !opts.allowRight) return;
      if (e.ctrlKey && e.shiftKey && (it.cmd) && App.shelf) { App.shelf.addToCurrent(it.cmd, label); closeMenus(); return; }
      closeMenus();
      setTimeout(() => {
        try {
          if (it.fn) it.fn(e);
          else if (it.cmd) App.cmds.run(it.cmd, it.args);
        } catch (err) { console.error(err); App.emit('error', String(err.message || err)); }
      }, 0);
    });
    el.append(row);
  }
  document.body.append(el);
  // position within window
  const r = el.getBoundingClientRect();
  let px = x, py = y;
  if (px + r.width > innerWidth) px = opts.sub ? Math.max(0, x - r.width - (opts.parentWidth || 180)) : innerWidth - r.width - 2;
  if (py + r.height > innerHeight) py = Math.max(0, innerHeight - r.height - 2);
  el.style.left = px + 'px'; el.style.top = py + 'px';
  openMenus.push(el);
  return el;
}
document.addEventListener('mousedown', (e) => { if (openMenus.length && !e.target.closest('.popup') && !e.target.closest('.menu-title')) closeMenus(); }, true);
document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && openMenus.length) { closeMenus(); e.stopPropagation(); } }, true);

// menu bar: titles that open menus, with hover-switching
export function menuBar(container, menus, { cls = '' } = {}) {
  container.innerHTML = '';
  let active = false;
  const titles = [];
  for (const m of menus) {
    const t = h('div', { class: 'menu-title ' + (m.cls || cls), text: m.label });
    const open = () => {
      closeMenus(); active = true; t.classList.add('open');
      const r = t.getBoundingClientRect();
      showMenu(m.items, r.left, r.bottom, { onClose: () => { active = false; } });
      t.classList.add('open');
    };
    t.addEventListener('mousedown', (e) => { e.preventDefault(); if (t.classList.contains('open')) { closeMenus(); return; } open(); });
    t.addEventListener('mouseenter', () => { if (openMenus.length && active && !t.classList.contains('open')) { const keep = menuCloseCb; menuCloseCb = null; closeMenus(); open(); } });
    container.append(t); titles.push(t);
  }
  return titles;
}

// ---------------------------------------------------------------- floating windows
let zTop = 1000;
const wins = new Map();
export class FloatWin {
  constructor(id, title, { w = 500, h: hh = 400, x = null, y = null, onClose = null, minW = 220, minH = 120 } = {}) {
    if (wins.has(id)) { const w0 = wins.get(id); w0.focus(); w0.reused = true; return w0; }
    this.id = id; this.onClose = onClose; this.reused = false;
    this.el = h('div', { class: 'fwin' });
    this.titleEl = h('span', { class: 't', text: title });
    const bar = h('div', { class: 'fwin-title' }, this.titleEl,
      h('span', { class: 'wb', text: '—', title: 'Minimize', onclick: () => this.toggleMin() }),
      h('span', { class: 'wb', text: '☐', title: 'Maximize', onclick: () => this.toggleMax() }),
      h('span', { class: 'wb close', text: '✕', title: 'Close', onclick: () => this.close() }));
    this.body = h('div', { class: 'fwin-body' });
    const rz = h('div', { class: 'fwin-resize' });
    this.el.append(bar, this.body, rz);
    const sx = x ?? Math.max(20, (innerWidth - w) / 2 + (wins.size % 6) * 24), sy = y ?? Math.max(20, (innerHeight - hh) / 2 + (wins.size % 6) * 24);
    Object.assign(this.el.style, { left: sx + 'px', top: sy + 'px', width: w + 'px', height: hh + 'px' });
    document.body.append(this.el);
    this.el.addEventListener('mousedown', () => this.focus(), true);
    drag(bar, (dx, dy, st) => { this.el.style.left = (st.l + dx) + 'px'; this.el.style.top = Math.max(0, st.t + dy) + 'px'; }, () => ({ l: this.el.offsetLeft, t: this.el.offsetTop }));
    drag(rz, (dx, dy, st) => { this.el.style.width = Math.max(minW, st.w + dx) + 'px'; this.el.style.height = Math.max(minH, st.h + dy) + 'px'; this.resized(); }, () => ({ w: this.el.offsetWidth, h: this.el.offsetHeight }));
    wins.set(id, this); this.focus();
    this._ro = new ResizeObserver(() => this.resized()); this._ro.observe(this.body);
  }
  resized() { if (this.onResize) this.onResize(); }
  focus() { this.el.style.zIndex = ++zTop; document.querySelectorAll('.fwin.focused').forEach(e => e.classList.remove('focused')); this.el.classList.add('focused'); }
  setTitle(t) { this.titleEl.textContent = t; }
  toggleMin() { this.body.classList.toggle('hidden'); this.el.style.height = this.body.classList.contains('hidden') ? '27px' : (this._h || '400px'); if (!this.body.classList.contains('hidden')) this._h = null; else this._h = this._h || this.el.style.height; }
  toggleMax() {
    if (this._max) { Object.assign(this.el.style, this._max); this._max = null; }
    else { this._max = { left: this.el.style.left, top: this.el.style.top, width: this.el.style.width, height: this.el.style.height }; Object.assign(this.el.style, { left: '0px', top: '0px', width: innerWidth + 'px', height: innerHeight + 'px' }); }
    this.resized();
  }
  close() { if (this._ro) this._ro.disconnect(); this.el.remove(); wins.delete(this.id); if (this.onClose) this.onClose(); }
  static get(id) { return wins.get(id); }
  static isOpen(id) { return wins.has(id); }
}
export function drag(el, onMove, getStart, onEnd, { button = 0 } = {}) {
  el.addEventListener('mousedown', (e) => {
    if (e.button !== button || e.target.closest('.wb')) return;
    e.preventDefault();
    const st = getStart ? getStart(e) : {}; const x0 = e.clientX, y0 = e.clientY;
    const mm = (ev) => onMove(ev.clientX - x0, ev.clientY - y0, st, ev);
    const mu = (ev) => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); if (onEnd) onEnd(ev, st); };
    addEventListener('mousemove', mm); addEventListener('mouseup', mu);
  });
}

// ---------------------------------------------------------------- dialogs
export function toast(msg, ms = 1800) {
  const t = h('div', { class: 'toast', text: msg }); document.body.append(t);
  setTimeout(() => { t.style.opacity = '0'; setTimeout(() => t.remove(), 400); }, ms);
}
export function promptDialog(title, label, value = '', { okLabel = 'OK' } = {}) {
  return new Promise((res) => {
    const win = new FloatWin('prompt-' + Math.random(), title, { w: 360, h: 150 });
    const inp = h('input', { class: 'wide', value });
    const done = (v) => { win.onClose = null; win.close(); res(v); };
    win.onClose = () => res(null);
    win.body.append(h('div', { class: 'dlg-body' }, h('div', { class: 'dlg-row' }, h('label', { text: label, style: { width: '90px' } }), inp)),
      h('div', { class: 'dlg-buttons' }, h('button', { text: okLabel, onclick: () => done(inp.value) }), h('button', { text: 'Cancel', onclick: () => done(null) })));
    inp.addEventListener('keydown', (e) => { if (e.key === 'Enter') done(inp.value); if (e.key === 'Escape') done(null); e.stopPropagation(); });
    setTimeout(() => { inp.focus(); inp.select(); }, 10);
  });
}
export function confirmDialog(title, message, buttons = ['OK', 'Cancel']) {
  return new Promise((res) => {
    const win = new FloatWin('confirm-' + Math.random(), title, { w: 420, h: 150 });
    win.onClose = () => res(buttons.length - 1);
    win.body.append(h('div', { class: 'dlg-body' }, h('div', { text: message, style: { padding: '6px 0', lineHeight: '1.4' } })),
      h('div', { class: 'dlg-buttons' }, buttons.map((b, i) => h('button', { text: b, onclick: () => { win.onClose = null; win.close(); res(i); } }))));
  });
}
// Maya-style option box: fields [{key,label,type:'float'|'int'|'bool'|'enum'|'radio'|'text'|'vec3',options,value}]
export function optionsDialog(title, fields, onApply, { applyLabel = 'Apply', w = 480, values = null, onReset = null } = {}) {
  const id = 'opt-' + title;
  const win = new FloatWin(id, title, { w, h: Math.min(560, 120 + fields.length * 26) });
  if (win.reused) win.body.innerHTML = '';
  const body = h('div', { class: 'dlg-body' });
  const vals = {};
  const inputs = {};
  const build = (fs) => {
    body.innerHTML = '';
    for (const f of fs) {
      if (f.section) { body.append(h('div', { class: 'dlg-sect', text: f.section })); continue; }
      const v = values && f.key in values ? values[f.key] : f.value;
      vals[f.key] = v;
      let inp;
      if (f.type === 'bool') { inp = h('input', { type: 'checkbox', checked: !!v, onchange: (e) => vals[f.key] = e.target.checked }); }
      else if (f.type === 'enum') { inp = h('select', { onchange: (e) => vals[f.key] = f.values ? f.values[e.target.selectedIndex] : e.target.selectedIndex }, (f.options || []).map((o, i) => h('option', { value: i, text: o }))); inp.selectedIndex = f.values ? Math.max(0, f.values.indexOf(v)) : (v || 0); }
      else if (f.type === 'radio') {
        inp = h('div', { class: 'radio' }, (f.options || []).map((o, i) => h('label', {}, h('input', { type: 'radio', name: id + f.key, checked: (f.values ? f.values[i] : i) === v, onchange: () => vals[f.key] = f.values ? f.values[i] : i }), o)));
      }
      else if (f.type === 'vec3') { inp = h('span', {}, [0, 1, 2].map(i => h('input', { value: v[i], style: { width: '64px', marginRight: '2px' }, onchange: (e) => { vals[f.key] = vals[f.key].slice(); vals[f.key][i] = parseFloat(e.target.value) || 0; } }))); vals[f.key] = v.slice(); }
      else if (f.type === 'text') { inp = h('input', { class: 'wide', value: v, onchange: (e) => vals[f.key] = e.target.value }); }
      else { inp = h('input', { value: v, onchange: (e) => { const n = f.type === 'int' ? parseInt(e.target.value, 10) : parseFloat(e.target.value); vals[f.key] = isNaN(n) ? f.value : n; } }); }
      inputs[f.key] = inp;
      body.append(h('div', { class: 'dlg-row' }, h('label', { text: f.label || f.key }), inp, f.suffix ? h('span', { class: 'dim', text: f.suffix }) : null));
    }
  };
  build(fields);
  const menuRow = h('div', { class: 'pmenubar' });
  menuBar(menuRow, [{ label: 'Edit', items: [{ label: 'Reset Settings', fn: () => { if (onReset) onReset(); values = null; build(fields); } }, { label: 'Save Settings', fn: () => toast('Settings saved') }] }, { label: 'Help', items: [{ label: 'Help on ' + title, fn: () => toast(title) }] }]);
  win.body.append(menuRow, body, h('div', { class: 'dlg-buttons' },
    h('button', { text: applyLabel + ' and Close', onclick: () => { onApply({ ...vals }); win.close(); } }),
    h('button', { text: applyLabel, onclick: () => onApply({ ...vals }) }),
    h('button', { text: 'Close', onclick: () => win.close() })));
  return win;
}

// ---------------------------------------------------------------- color picker
export function colorToCss(c) { return `rgb(${Math.round(Math.min(1, Math.max(0, c[0])) * 255)},${Math.round(Math.min(1, Math.max(0, c[1])) * 255)},${Math.round(Math.min(1, Math.max(0, c[2])) * 255)})`; }
function rgb2hsv([r, g, b]) { const mx = Math.max(r, g, b), mn = Math.min(r, g, b), d = mx - mn; let hh = 0; if (d) { if (mx === r) hh = ((g - b) / d) % 6; else if (mx === g) hh = (b - r) / d + 2; else hh = (r - g) / d + 4; hh *= 60; if (hh < 0) hh += 360; } return [hh, mx ? d / mx : 0, mx]; }
function hsv2rgb([hh, s, v]) { const c = v * s, x = c * (1 - Math.abs((hh / 60) % 2 - 1)), m = v - c; let r = 0, g = 0, b = 0; if (hh < 60) [r, g, b] = [c, x, 0]; else if (hh < 120) [r, g, b] = [x, c, 0]; else if (hh < 180) [r, g, b] = [0, c, x]; else if (hh < 240) [r, g, b] = [0, x, c]; else if (hh < 300) [r, g, b] = [x, 0, c]; else [r, g, b] = [c, 0, x]; return [r + m, g + m, b + m]; }
export function colorPicker(x, y, value, onChange, onDone) {
  document.querySelectorAll('.color-pop').forEach(e => e.remove());
  let hsv = rgb2hsv(value.map(c => Math.min(1, c)));
  const pop = h('div', { class: 'color-pop' });
  const wheel = h('canvas', { width: 232, height: 150 });
  const prev = h('div', { style: { height: '22px', border: '1px solid #222' } });
  const fields = ['H', 'S', 'V'].map((l, i) => h('input', { type: 'range', min: 0, max: i === 0 ? 360 : 1000, value: i === 0 ? hsv[0] : hsv[i] * 1000 }));
  const rgbIn = ['R', 'G', 'B'].map(() => h('input', { type: 'text' }));
  const draw = () => {
    const ctx = wheel.getContext('2d'); const W = wheel.width, H = wheel.height;
    const img = ctx.createImageData(W, H);
    for (let j = 0; j < H; j++) for (let i = 0; i < W; i++) { const c = hsv2rgb([i / W * 360, 1 - j / H, hsv[2]]); const k = (j * W + i) * 4; img.data[k] = c[0] * 255; img.data[k + 1] = c[1] * 255; img.data[k + 2] = c[2] * 255; img.data[k + 3] = 255; }
    ctx.putImageData(img, 0, 0);
    ctx.strokeStyle = '#fff'; ctx.beginPath(); ctx.arc(hsv[0] / 360 * W, (1 - hsv[1]) * H, 4, 0, 7); ctx.stroke();
    const c = hsv2rgb(hsv); prev.style.background = colorToCss(c);
    rgbIn.forEach((inp, i) => { if (document.activeElement !== inp) inp.value = c[i].toFixed(3); });
    fields[0].value = hsv[0]; fields[1].value = hsv[1] * 1000; fields[2].value = hsv[2] * 1000;
  };
  const emit = () => { draw(); onChange(hsv2rgb(hsv)); };
  const pick = (e) => { const r = wheel.getBoundingClientRect(); hsv[0] = Math.max(0, Math.min(359.9, (e.clientX - r.left) / r.width * 360)); hsv[1] = Math.max(0, Math.min(1, 1 - (e.clientY - r.top) / r.height)); emit(); };
  wheel.addEventListener('mousedown', (e) => { pick(e); const mm = (ev) => pick(ev); const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); }; addEventListener('mousemove', mm); addEventListener('mouseup', mu); });
  fields.forEach((f, i) => f.addEventListener('input', () => { hsv[i] = i === 0 ? +f.value : f.value / 1000; emit(); }));
  rgbIn.forEach((inp, i) => inp.addEventListener('change', () => { const c = hsv2rgb(hsv); c[i] = Math.max(0, parseFloat(inp.value) || 0); hsv = rgb2hsv(c.map(v => Math.min(1, v))); emit(); }));
  pop.append(wheel, prev, ...['H', 'S', 'V'].map((l, i) => h('div', { class: 'cp-row' }, h('label', { text: l }), fields[i])),
    h('div', { class: 'cp-row' }, ...['R', 'G', 'B'].map((l, i) => [h('label', { text: l }), rgbIn[i]])),
    h('div', { class: 'cp-row' }, h('button', { text: 'Accept', style: { flex: 1 }, onclick: () => { pop.remove(); onDone && onDone(hsv2rgb(hsv)); } }), h('button', { text: 'Cancel', style: { flex: 1 }, onclick: () => { onChange(value); pop.remove(); onDone && onDone(null); } })));
  pop.addEventListener('mousedown', e => e.stopPropagation());
  document.body.append(pop);
  const r = pop.getBoundingClientRect();
  pop.style.left = Math.min(x, innerWidth - r.width - 4) + 'px'; pop.style.top = Math.min(y, innerHeight - r.height - 4) + 'px';
  draw();
  const off = (e) => { if (!pop.contains(e.target)) { pop.remove(); removeEventListener('mousedown', off, true); onDone && onDone(hsv2rgb(hsv)); } };
  setTimeout(() => addEventListener('mousedown', off, true), 0);
  return pop;
}

// numeric field supporting Ctrl+LMB / MMB drag (Maya virtual slider) and math expressions (+=, *=)
export function numField(value, onChange, { step = 0.1, int = false, cls = '', width = null, onStart = null, onEnd = null } = {}) {
  const inp = h('input', { class: cls, value: fmt(value) });
  if (width) inp.style.width = width;
  const commit = () => {
    const s = inp.value.trim(); let n;
    const cur = parseFloat(inp.dataset.v ?? value);
    const m = s.match(/^([+\-*/])=\s*(-?[\d.]+)$/);
    if (m) { const a = parseFloat(m[2]); n = m[1] === '+' ? cur + a : m[1] === '-' ? cur - a : m[1] === '*' ? cur * a : cur / a; }
    else { try { n = Number(s); if (isNaN(n)) n = Function('"use strict";return (' + s.replace(/[^-+*/().\d\s eE]/g, '') + ')')(); } catch { n = NaN; } }
    if (typeof n !== 'number' || isNaN(n)) { inp.value = fmt(cur); return; }
    if (int) n = Math.round(n);
    inp.dataset.v = n; inp.value = fmt(n); onChange(n, false);
  };
  inp.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') { commit(); inp.blur(); } if (e.key === 'Escape') { inp.value = fmt(parseFloat(inp.dataset.v ?? value)); inp.blur(); } });
  inp.addEventListener('change', commit);
  inp.addEventListener('mousedown', (e) => {
    if (!(e.button === 1 || (e.button === 0 && e.ctrlKey))) return;
    e.preventDefault();
    const x0 = e.clientX; const v0 = parseFloat(inp.dataset.v ?? value) || 0;
    onStart && onStart();
    const mm = (ev) => { let n = v0 + (ev.clientX - x0) * step * (ev.shiftKey ? 10 : 1); if (int) n = Math.round(n); inp.dataset.v = n; inp.value = fmt(n); onChange(n, true); };
    const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); onEnd && onEnd(); onChange(parseFloat(inp.dataset.v), false); };
    addEventListener('mousemove', mm); addEventListener('mouseup', mu);
  });
  inp.set = (v) => { if (document.activeElement !== inp) { inp.dataset.v = v; inp.value = fmt(v); } };
  inp.dataset.v = value;
  return inp;
}
