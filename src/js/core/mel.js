// Inca — MEL-style scripting language and JavaScript "cmds" API
import * as THREE from 'three';
import { App } from './app.js';
import * as S from './scene.js';
import { Sel } from './selection.js';
import { Undo } from './undo.js';
import { GEN_INFO } from './primitives.js';
import { createPrimitive, assignMaterial } from './commands.js';
import { createMaterial, MAT_TYPES } from './materials.js';
import { Curve } from './curves.js';

// ------------------------------------------------------------------ tokenizer
function tokenize(src) {
  const t = []; let i = 0;
  const isId = (c) => /[A-Za-z0-9_$.:|\[\]]/.test(c);
  while (i < src.length) {
    const c = src[i];
    if (c === '/' && src[i + 1] === '/') { while (i < src.length && src[i] !== '\n') i++; continue; }
    if (c === '/' && src[i + 1] === '*') { i = src.indexOf('*/', i + 2); i = i < 0 ? src.length : i + 2; continue; }
    if (/\s/.test(c)) { i++; continue; }
    if (c === '"') { let s = ''; i++; while (i < src.length && src[i] !== '"') { if (src[i] === '\\') { i++; s += { n: '\n', t: '\t', '"': '"', '\\': '\\' }[src[i]] ?? src[i]; } else s += src[i]; i++; } i++; t.push({ k: 'str', v: s }); continue; }
    if (c === '`') { let s = ''; i++; while (i < src.length && src[i] !== '`') s += src[i++]; i++; t.push({ k: 'tick', v: s }); continue; }
    if (/[0-9]/.test(c) || (c === '.' && /[0-9]/.test(src[i + 1]))) { let s = ''; while (i < src.length && /[0-9.eE]/.test(src[i])) { if ((src[i] === 'e' || src[i] === 'E') && /[-+]/.test(src[i + 1])) { s += src[i++]; } s += src[i++]; } t.push({ k: 'num', v: parseFloat(s) }); continue; }
    const two = src.substr(i, 2);
    if (['==', '!=', '<=', '>=', '&&', '||', '++', '--', '+=', '-=', '*=', '/='].includes(two)) { t.push({ k: 'op', v: two }); i += 2; continue; }
    if ('+-*/%<>=!(){}[];,?:<<>>'.includes(c)) { t.push({ k: 'op', v: c }); i++; continue; }
    if (isId(c)) { let s = ''; while (i < src.length && isId(src[i])) s += src[i++]; t.push({ k: s[0] === '$' ? 'var' : 'id', v: s }); continue; }
    t.push({ k: 'op', v: c }); i++;
  }
  return t;
}

// ------------------------------------------------------------------ interpreter
const TYPES = new Set(['int', 'float', 'string', 'vector', 'matrix', 'global', 'proc']);
class Interp {
  constructor(out) { this.vars = new Map(); this.out = out; this.procs = new Map(); }
  run(src) { this.t = tokenize(src); this.p = 0; let last; while (this.p < this.t.length) last = this.stmt(); return last; }
  peek(o = 0) { return this.t[this.p + o]; }
  eat(v) { const k = this.t[this.p]; if (v !== undefined && (!k || k.v !== v)) throw new Error(`Syntax error: expected "${v}" near "${k ? k.v : 'end'}"`); this.p++; return k; }
  is(v, o = 0) { const k = this.t[this.p + o]; return k && k.k === 'op' && k.v === v; }
  stmt() {
    const k = this.peek(); if (!k) return;
    if (this.is(';')) { this.p++; return; }
    if (this.is('{')) { this.p++; let r; while (!this.is('}')) { if (!this.peek()) throw new Error('Missing }'); r = this.stmt(); } this.p++; return r; }
    if (k.k === 'id' && k.v === 'proc') return this.procDef();
    if (k.k === 'id' && TYPES.has(k.v)) { this.p++; while (this.peek() && this.peek().k === 'id' && TYPES.has(this.peek().v)) this.p++; return this.decl(); }
    if (k.k === 'id' && k.v === 'if') return this.ifStmt();
    if (k.k === 'id' && k.v === 'for') return this.forStmt();
    if (k.k === 'id' && k.v === 'while') return this.whileStmt();
    if (k.k === 'id' && k.v === 'return') { this.p++; const v = this.is(';') ? undefined : this.expr(); this.semi(); throw { ret: v }; }
    if (k.k === 'id' && (k.v === 'break' || k.v === 'continue')) { this.p++; this.semi(); throw { [k.v]: true }; }
    if (k.k === 'var') { const r = this.expr(); this.semi(); return r; }
    if (k.k === 'id' && (k.v === 'print')) { this.p++; const v = this.is('(') ? this.expr() : this.expr(); this.out(fmtVal(v)); this.semi(); return; }
    if (k.k === 'id') { const r = this.command(); this.semi(); return r; }
    const r = this.expr(); this.semi(); return r;
  }
  semi() { if (this.is(';')) this.p++; }
  decl() {
    let r;
    do {
      const v = this.eat(); if (v.k !== 'var') throw new Error('Expected variable name');
      let arr = false; if (this.is('[')) { this.p++; if (!this.is(']')) this.expr(); this.eat(']'); arr = true; }
      if (this.is('=')) { this.p++; r = this.expr(); } else r = arr ? [] : 0;
      this.vars.set(v.v, r);
    } while (this.is(',') && this.eat(','));
    this.semi(); return r;
  }
  procDef() {
    this.eat('proc'); if (this.peek().k === 'id' && TYPES.has(this.peek().v)) { this.p++; if (this.is('[')) { this.p++; this.eat(']'); } }
    const name = this.eat().v; this.eat('('); const params = [];
    while (!this.is(')')) { while (this.peek().k === 'id' && TYPES.has(this.peek().v)) this.p++; params.push(this.eat().v); if (this.is('[')) { this.p++; this.eat(']'); } if (this.is(',')) this.p++; }
    this.eat(')'); const start = this.p; this.skipBlock();
    this.procs.set(name, { params, body: this.t.slice(start, this.p) });
  }
  skipBlock() { this.eat('{'); let d = 1; while (d > 0) { const k = this.eat(); if (!k) throw new Error('Missing }'); if (k.k === 'op' && k.v === '{') d++; if (k.k === 'op' && k.v === '}') d--; } }
  skipStmt() { if (this.is('{')) return this.skipBlock(); let d = 0; while (this.peek()) { const k = this.eat(); if (k.k === 'op' && (k.v === '(' || k.v === '{')) d++; if (k.k === 'op' && (k.v === ')' || k.v === '}')) d--; if (d <= 0 && k.k === 'op' && k.v === ';') break; } }
  ifStmt() {
    this.eat('if'); this.eat('('); const c = this.expr(); this.eat(')');
    if (truthy(c)) { this.stmt(); while (this.peek() && this.peek().v === 'else') { this.p++; if (this.peek().v === 'if') { this.eat('if'); this.eat('('); this.expr(); this.eat(')'); } this.skipStmt(); } }
    else { this.skipStmt(); if (this.peek() && this.peek().v === 'else') { this.p++; this.stmt(); } }
  }
  forStmt() {
    this.eat('for'); this.eat('(');
    // for ($x in $arr)
    if (this.peek().k === 'var' && this.peek(1) && this.peek(1).v === 'in') {
      const v = this.eat().v; this.p++; const arr = this.expr(); this.eat(')'); const body = this.p;
      let end = body; this.skipStmt(); end = this.p;
      for (const x of (Array.isArray(arr) ? arr : [arr])) { this.vars.set(v, x); this.p = body; try { this.stmt(); } catch (e) { if (e.break) break; if (e.continue) continue; throw e; } }
      this.p = end; return;
    }
    if (!this.is(';')) { if (this.peek().k === 'id' && TYPES.has(this.peek().v)) { this.p++; this.declNoSemi(); } else this.expr(); } this.eat(';');
    const condP = this.p; let d = 0; while (!(this.is(';') && d === 0)) { if (this.is('(')) d++; if (this.is(')')) d--; this.p++; } this.p++;
    const stepP = this.p; d = 0; while (!(this.is(')') && d === 0)) { if (this.is('(')) d++; if (this.is(')')) d--; this.p++; } this.p++;
    const body = this.p; this.skipStmt(); const end = this.p;
    let guard = 0;
    while (guard++ < 1e6) {
      this.p = condP; const c = this.is(';') ? 1 : this.expr(); if (!truthy(c)) break;
      this.p = body; try { this.stmt(); } catch (e) { if (e.break) break; if (!e.continue) throw e; }
      this.p = stepP; if (!this.is(')')) this.expr();
    }
    this.p = end;
  }
  declNoSemi() { const v = this.eat().v; let r = 0; if (this.is('=')) { this.p++; r = this.expr(); } this.vars.set(v, r); }
  whileStmt() {
    this.eat('while'); this.eat('('); const condP = this.p; let d = 0; while (!(this.is(')') && d === 0)) { if (this.is('(')) d++; if (this.is(')')) d--; this.p++; } this.p++;
    const body = this.p; this.skipStmt(); const end = this.p; let guard = 0;
    while (guard++ < 1e6) { this.p = condP; if (!truthy(this.expr())) break; this.p = body; try { this.stmt(); } catch (e) { if (e.break) break; if (!e.continue) throw e; } }
    this.p = end;
  }
  // command invocation: name arg arg -flag value ...
  command(stopAtParen = false) {
    const name = this.eat().v;
    if (this.is('(') && !MEL[name] && !this.procs.has(name) && MATH[name]) { return this.callFunc(name); }
    if (this.is('(') && (this.procs.has(name) || MATH[name] || (MEL[name] && !stopAtParen))) {
      // function-call syntax
      return this.callFunc(name);
    }
    const args = [];
    while (this.peek() && !this.is(';') && !this.is('}') && !(stopAtParen && this.is(')'))) {
      const k = this.peek();
      if (k.k === 'op' && k.v === '-' && this.peek(1) && this.peek(1).k === 'id' && !/^[0-9]/.test(this.peek(1).v)) { this.p += 2; args.push({ flag: this.peek(-1).v }); continue; }
      if (k.k === 'op' && k.v === '-' && this.peek(1) && this.peek(1).k === 'num') { this.p += 2; args.push(-this.peek(-1).v); continue; }
      if (k.k === 'id') { this.p++; args.push(k.v); continue; }
      if (k.k === 'str' || k.k === 'num') { this.p++; args.push(k.v); continue; }
      if (k.k === 'var') { this.p++; let v = this.vars.get(k.v); if (this.is('[')) { this.p++; const i = this.expr(); this.eat(']'); v = Array.isArray(v) ? v[i] : v; } args.push(v); continue; }
      if (k.k === 'tick') { this.p++; args.push(new Interp(this.out).withVars(this.vars).run(k.v)); continue; }
      if (k.k === 'op' && k.v === '(') { this.p++; args.push(this.expr()); this.eat(')'); continue; }
      if (k.k === 'op' && k.v === '{') { this.p++; const arr = []; while (!this.is('}')) { arr.push(this.expr()); if (this.is(',')) this.p++; } this.p++; args.push(arr); continue; }
      this.p++;
    }
    return this.invoke(name, args);
  }
  withVars(v) { this.vars = v; return this; }
  callFunc(name) {
    this.eat('('); const args = [];
    while (!this.is(')')) { args.push(this.expr()); if (this.is(',')) this.p++; }
    this.eat(')');
    if (MATH[name]) return MATH[name](...args);
    if (this.procs.has(name)) return this.callProc(name, args);
    return this.invoke(name, args);
  }
  callProc(name, args) {
    const pr = this.procs.get(name); const sub = new Interp(this.out); sub.procs = this.procs;
    sub.vars = new Map(this.vars); pr.params.forEach((p, i) => sub.vars.set(p, args[i]));
    sub.t = pr.body; sub.p = 0;
    try { sub.stmt(); } catch (e) { if (e && 'ret' in e) return e.ret; throw e; }
  }
  invoke(name, args) {
    if (this.procs.has(name)) return this.callProc(name, args);
    const fn = MEL[name];
    if (!fn) {
      if (App.cmds.registry.has(name)) return App.cmds.run(name);
      const alias = Object.keys(MEL_ALIASES).find(a => a.toLowerCase() === name.toLowerCase());
      if (alias) return App.cmds.run(MEL_ALIASES[alias]);
      throw new Error(`Cannot find procedure "${name}".`);
    }
    const flags = {}; const pos = [];
    let cur = null;
    for (const a of args) {
      if (a && typeof a === 'object' && 'flag' in a) { cur = a.flag; flags[cur] = flags[cur] || []; flags[cur].push([]); continue; }
      if (cur) flags[cur][flags[cur].length - 1].push(a); else pos.push(a);
    }
    // flag values: single -> scalar; none -> true
    const F = {}; for (const k in flags) { const vals = flags[k][flags[k].length - 1]; F[k] = vals.length === 0 ? true : vals.length === 1 ? vals[0] : vals; F[k + '__all'] = flags[k]; }
    // positional args that came after a flag belong to it only up to its arity; push extra values back as positional
    for (const k in flags) { const ar = FLAG_ARITY[name]?.[k] ?? FLAG_ARITY._[k]; if (ar !== undefined) { const vals = flags[k][flags[k].length - 1]; if (vals.length > ar) { pos.push(...vals.slice(ar)); F[k] = ar === 0 ? true : ar === 1 ? vals[0] : vals.slice(0, ar); } } }
    return fn(F, pos.flat ? pos.flat() : pos);
  }
  // expressions
  expr() { return this.assign(); }
  assign() {
    const k = this.peek();
    if (k && k.k === 'var' && this.peek(1) && this.peek(1).k === 'op' && ['=', '+=', '-=', '*=', '/='].includes(this.peek(1).v)) {
      this.p += 2; const op = this.peek(-1).v; const r = this.expr(); const cur = this.vars.get(k.v);
      const v = op === '=' ? r : op === '+=' ? (cur + r) : op === '-=' ? cur - r : op === '*=' ? cur * r : cur / r; this.vars.set(k.v, v); return v;
    }
    if (k && k.k === 'var' && this.peek(1) && this.peek(1).v === '[') {
      // $a[i] = v
      const save = this.p; this.p += 2; const idx = this.expr(); this.eat(']');
      if (this.is('=')) { this.p++; const r = this.expr(); let arr = this.vars.get(k.v); if (!Array.isArray(arr)) arr = []; arr[idx] = r; this.vars.set(k.v, arr); return r; }
      this.p = save;
    }
    return this.ternary();
  }
  ternary() { const c = this.or(); if (this.is('?')) { this.p++; const a = this.expr(); this.eat(':'); const b = this.expr(); return truthy(c) ? a : b; } return c; }
  or() { let l = this.and(); while (this.is('||')) { this.p++; const r = this.and(); l = (truthy(l) || truthy(r)) ? 1 : 0; } return l; }
  and() { let l = this.cmp(); while (this.is('&&')) { this.p++; const r = this.cmp(); l = (truthy(l) && truthy(r)) ? 1 : 0; } return l; }
  cmp() { let l = this.add(); while (['==', '!=', '<', '>', '<=', '>='].some(o => this.is(o))) { const o = this.eat().v; const r = this.add(); l = ({ '==': l == r, '!=': l != r, '<': l < r, '>': l > r, '<=': l <= r, '>=': l >= r })[o] ? 1 : 0; } return l; }
  add() { let l = this.mul(); while (this.is('+') || this.is('-')) { const o = this.eat().v; const r = this.mul(); if (o === '+') l = (Array.isArray(l) && Array.isArray(r)) ? l.map((x, i) => x + r[i]) : l + r; else l = Array.isArray(l) ? l.map((x, i) => x - r[i]) : l - r; } return l; }
  mul() { let l = this.unary(); while (this.is('*') || this.is('/') || this.is('%')) { const o = this.eat().v; const r = this.unary(); l = o === '*' ? (Array.isArray(l) ? l.map(x => x * r) : l * r) : o === '/' ? l / r : l % r; } return l; }
  unary() {
    if (this.is('-')) { this.p++; const v = this.unary(); return Array.isArray(v) ? v.map(x => -x) : -v; }
    if (this.is('!')) { this.p++; return truthy(this.unary()) ? 0 : 1; }
    if (this.is('++') || this.is('--')) { const o = this.eat().v; const n = this.eat().v; const v = (this.vars.get(n) || 0) + (o === '++' ? 1 : -1); this.vars.set(n, v); return v; }
    return this.postfix();
  }
  postfix() {
    const k = this.peek();
    if (k && k.k === 'var' && this.peek(1) && (this.peek(1).v === '++' || this.peek(1).v === '--')) { this.p += 2; const v = this.vars.get(k.v) || 0; this.vars.set(k.v, v + (this.peek(-1).v === '++' ? 1 : -1)); return v; }
    let v = this.primary();
    while (this.is('[')) { this.p++; const i = this.expr(); this.eat(']'); v = Array.isArray(v) ? v[i] : (typeof v === 'string' ? v[i] : undefined); }
    return v;
  }
  primary() {
    const k = this.eat(); if (!k) throw new Error('Unexpected end of input');
    if (k.k === 'num' || k.k === 'str') return k.v;
    if (k.k === 'var') { if (!this.vars.has(k.v)) throw new Error(`"${k.v}" is an undeclared variable.`); return this.vars.get(k.v); }
    if (k.k === 'tick') return new Interp(this.out).withVars(this.vars).run(k.v);
    if (k.k === 'op' && k.v === '(') {
      if (this.peek() && this.peek().k === 'id' && (MEL[this.peek().v] || this.procs.has(this.peek().v)) && !(this.peek(1) && this.peek(1).v === '(')) { const r = this.command(true); this.eat(')'); return r; }
      const v = this.expr(); this.eat(')'); return v;
    }
    if (k.k === 'op' && k.v === '<' && this.is('<')) { this.p++; const arr = []; while (!(this.is('>') && this.is('>', 1))) { arr.push(this.expr()); if (this.is(',')) this.p++; } this.p += 2; return arr; }
    if (k.k === 'op' && k.v === '{') { const arr = []; while (!this.is('}')) { arr.push(this.expr()); if (this.is(',')) this.p++; } this.p++; return arr; }
    if (k.k === 'id') { this.p--; if (this.peek(1) && this.peek(1).v === '(') return this.callFunc(this.eat().v); if (k.v === 'true' || k.v === 'on' || k.v === 'yes') { this.p++; return 1; } if (k.v === 'false' || k.v === 'off' || k.v === 'no') { this.p++; return 0; } this.p++; return k.v; }
    throw new Error(`Syntax error near "${k.v}"`);
  }
}
const truthy = (v) => Array.isArray(v) ? v.length > 0 : !!v;
export function fmtVal(v) { if (Array.isArray(v)) return v.map(x => fmtVal(x)).join(' '); if (typeof v === 'number') return String(+v.toFixed(6)); return v === undefined ? '' : String(v); }

const MATH = {
  sin: Math.sin, cos: Math.cos, tan: Math.tan, abs: Math.abs, sqrt: Math.sqrt, pow: Math.pow, floor: Math.floor, ceil: Math.ceil, exp: Math.exp, log: Math.log,
  rand: (a, b) => b === undefined ? Math.random() * a : a + Math.random() * (b - a), min: Math.min, max: Math.max, deg_to_rad: (d) => d * Math.PI / 180, rad_to_deg: (r) => r * 180 / Math.PI,
  size: (a) => Array.isArray(a) ? a.length : String(a).length, clamp: (a, b, v) => Math.max(a, Math.min(b, v)), trunc: Math.trunc,
  strip: (s) => String(s).trim(), toupper: (s) => String(s).toUpperCase(), tolower: (s) => String(s).toLowerCase(), substring: (s, a, b) => String(s).substring(a - 1, b),
  gmatch: (s, p) => new RegExp('^' + String(p).replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*').replace(/\?/g, '.') + '$').test(s) ? 1 : 0,
  stringArrayCount: (a) => a.length, mag: (v) => Math.hypot(...v), noise: (x) => Math.sin(x * 12.9898) * 0.5, linstep: (a, b, v) => Math.max(0, Math.min(1, (v - a) / (b - a))), smoothstep: (a, b, v) => { const t = Math.max(0, Math.min(1, (v - a) / (b - a))); return t * t * (3 - 2 * t); },
};

// default arity of common flags (so `move -r 1 2 3 pCube1` parses correctly)
const FLAG_ARITY = {
  _: { r: 0, a: 0, ws: 0, os: 0, q: 0, query: 0, add: 0, tgl: 0, d: 0, cl: 0, sl: 0, ch: 1, n: 1, name: 1, e: 0, edit: 0, w: 1, h: 1, em: 0, rr: 0, abs: 0, relative: 0, absolute: 0, world: 0, rpr: 0 },
  move: { r: 0, a: 0, ws: 0, os: 0, x: 1, y: 1, z: 1 }, rotate: { r: 0, a: 0, ws: 0, os: 0, fo: 0 }, scale: { r: 0, a: 0 },
  xform: { t: 3, translation: 3, ro: 3, rotation: 3, s: 3, scale: 3, piv: 3, pivots: 3, rp: 3, sp: 3 },
  select: { r: 0, add: 0, tgl: 0, d: 0, cl: 0, all: 0, hi: 0 },
  setKeyframe: { t: 1, time: 1, at: 1, attribute: 1, v: 1, value: 1 },
  currentTime: { e: 0, q: 0 }, delete: { ch: 0, channels: 0, all: 0 }, duplicate: { rr: 0, n: 1 },
  ls: { sl: 0, selection: 0, type: 1, dag: 0, tr: 0, transforms: 0, l: 0, long: 0, lights: 0, cameras: 0, geometry: 0, materials: 0 },
  parent: { w: 0, r: 0, add: 0 }, group: { em: 0, n: 1, w: 0 }, setAttr: { type: 1, lock: 1, l: 1, k: 1 }, playbackOptions: { min: 1, max: 1, ast: 1, aet: 1, minTime: 1, maxTime: 1, q: 0 },
  polyEvaluate: { v: 0, vertex: 0, e: 0, edge: 0, f: 0, face: 0, t: 0, triangle: 0 }, objExists: {}, hide: {}, showHidden: { all: 0 }, makeIdentity: { apply: 1, t: 1, r: 1, s: 1 },
  spaceLocator: { p: 3, n: 1 }, circle: { c: 3, nr: 3, r: 1, s: 1, d: 1, n: 1, ch: 1 }, shadingNode: { asShader: 0, asTexture: 0, n: 1, name: 1 }, hyperShade: { assign: 1 },
  file: { new: 0, f: 0, force: 0, open: 0, o: 0, save: 0, s: 0, rename: 1, type: 1, i: 0, import: 0, sn: 0, q: 0 },
  directionalLight: { i: 1, intensity: 1, rgb: 3, rotation: 3, n: 1 }, pointLight: { i: 1, intensity: 1, rgb: 3, n: 1, position: 3 }, spotLight: { i: 1, intensity: 1, rgb: 3, ca: 1, coneAngle: 1, n: 1 }, ambientLight: { i: 1, intensity: 1, n: 1 }, areaLight: { i: 1, n: 1 },
  camera: { fl: 1, focalLength: 1, n: 1 }, keyframe: { q: 0, kc: 0, tc: 0, vc: 0, at: 1 }, cutKey: { t: 1, at: 1, cl: 0 }, curve: { d: 1, p: 3, n: 1 },
};

// primitive creation flags all take one value (polySphere -r 2 -sx 12 ...)
for (const [g, info] of Object.entries(GEN_INFO)) FLAG_ARITY[g] = Object.fromEntries([...Object.keys(info.short), ...Object.keys(info.params), 'n', 'name', 'ch', 'cuv', 'ax', 'o'].map(k => [k, k === 'ax' ? 3 : 1]));
FLAG_ARITY.sphere = FLAG_ARITY.nurbsSphere; FLAG_ARITY.cylinder = FLAG_ARITY.nurbsCylinder; FLAG_ARITY.cone = FLAG_ARITY.nurbsCone; FLAG_ARITY.torus = FLAG_ARITY.nurbsTorus;

// ------------------------------------------------------------------ helpers for commands
const flat = (a) => (Array.isArray(a) ? a.flat(Infinity) : [a]);
const nodesFrom = (pos, fallbackSel = true) => {
  const names = pos.filter(x => typeof x === 'string');
  if (!names.length) return fallbackSel ? App.sel.slice() : [];
  const out = [];
  for (const n of names) { const node = S.findNode(n.split('.')[0].replace(/^\|/, '').split('|').pop()); if (!node) throw new Error(`No object matches name: ${n}`); out.push(node); }
  return out;
};
const nm = (n) => S.nodeName(n);
const nums = (pos) => pos.filter(x => typeof x === 'number');
const ck = (l) => Undo.checkpoint(l);
function splitAttr(s) {
  const i = String(s).indexOf('.'); if (i < 0) throw new Error(`Invalid attribute "${s}"`);
  const node = S.findNode(s.slice(0, i)); if (!node) throw new Error(`No object matches name: ${s.slice(0, i)}`);
  return { node, attr: s.slice(i + 1) };
}
function genCmd(gen) {
  return (F) => {
    const info = GEN_INFO[gen]; const p = {};
    for (const [k, v] of Object.entries(F)) { if (k.endsWith('__all')) continue; const full = info.short[k] || (k in info.params ? k : null); if (full) { const n = typeof v === 'number' ? v : parseFloat(v); if (!isNaN(n)) p[full] = n; } }
    const o = createPrimitive(gen, p);
    if (F.n || F.name) S.rename(o, F.n || F.name);
    return [o.inca.name, o.inca.history[0].name];
  };
}

// ------------------------------------------------------------------ MEL command table
export const MEL = {
  // creation
  ...Object.fromEntries(Object.keys(GEN_INFO).map(g => [g, genCmd(g)])),
  sphere: (F) => MEL.nurbsSphere(F), cylinder: (F) => MEL.nurbsCylinder(F), cone: (F) => MEL.nurbsCone(F), torus: (F) => MEL.nurbsTorus(F), nurbsPlane: genCmd('nurbsPlane'), nurbsCube: genCmd('nurbsCube'),
  circle: (F) => { const o = App.cmds.run('nurbsCircle', { radius: F.r ?? 1, sections: F.s ?? 8 }); if (F.n) S.rename(o, F.n); if (F.c) { o.inca.t = flat(F.c).map(Number); S.updateXform(o); } return [o.inca.name]; },
  curve: (F) => { ck('curve'); const pts = (F.p__all || []).map(v => flat(v).map(Number)); const c = new Curve(pts, F.d ?? 3, 'open'); const o = S.createCurve(c, { name: F.n || 'curve1' }); Sel.select([o]); return o.inca.name; },
  spaceLocator: (F) => { const o = App.cmds.run('createLocator'); if (F.p) { o.inca.t = flat(F.p).map(Number); S.updateXform(o); } if (F.n) S.rename(o, F.n); return [o.inca.name]; },
  camera: (F) => { const o = App.cmds.run('createCamera'); if (F.fl || F.focalLength) o.inca.cam.focalLength = +(F.fl || F.focalLength); if (F.n) S.rename(o, F.n); return [o.inca.name, o.inca.shapeName]; },
  ...Object.fromEntries(['directionalLight', 'pointLight', 'spotLight', 'ambientLight', 'areaLight'].map(t => [t, (F) => { const o = App.cmds.run(t); const i = F.i ?? F.intensity; if (i !== undefined) o.inca.light.intensity = +i; if (F.rgb) o.inca.light.color = flat(F.rgb).map(Number); if (F.ca || F.coneAngle) o.inca.light.coneAngle = +(F.ca || F.coneAngle); if (F.n) S.rename(o, F.n); S.syncLight(o); return o.inca.shapeName; }])),
  group: (F, pos) => { if (F.em) { const g = App.cmds.run('createEmptyGroup'); if (F.n) S.rename(g, F.n); return g.inca.name; } const ns = nodesFrom(pos); Sel.select(ns, 'replace', { echo: false }); const g = App.cmds.run('group'); if (F.n) S.rename(g, F.n); return g.inca.name; },
  duplicate: (F, pos) => { const ns = nodesFrom(pos); ck('duplicate'); const out = S.duplicateNodes(ns); if (F.n) S.rename(out[0], F.n); Sel.select(out, 'replace', { echo: false }); return out.map(nm); },
  instance: (F, pos) => MEL.duplicate(F, pos),
  // transforms
  move: (F, pos) => xformCmd('t', F, pos), rotate: (F, pos) => xformCmd('r', F, pos), scale: (F, pos) => xformCmd('s', F, pos),
  xform: (F, pos) => {
    const ns = nodesFrom(pos); if (!ns.length) throw new Error('No object specified');
    if (F.q || F.query) {
      const o = ns[0];
      if (F.t || F.translation) { if (F.ws || F.worldSpace) { const p = new THREE.Vector3().setFromMatrixPosition(S.worldMatrix(o)); return [p.x, p.y, p.z]; } return o.inca.t.slice(); }
      if (F.ro || F.rotation) return o.inca.r.slice(); if (F.s || F.scale) return o.inca.s.slice();
      if (F.piv || F.rp || F.pivots) { const p = S.worldPivot(o); return [p.x, p.y, p.z]; }
      if (F.bb || F.boundingBox) { const b = S.worldBBox(o); return [...b.min.toArray(), ...b.max.toArray()]; }
      return o.inca.t.slice();
    }
    ck('xform');
    for (const o of ns) {
      const rel = F.r || F.relative;
      for (const [f, k] of [['t', 't'], ['translation', 't'], ['ro', 'r'], ['rotation', 'r'], ['s', 's'], ['scale', 's']]) if (F[f]) { const v = flat(F[f]).map(Number); o.inca[k] = rel ? o.inca[k].map((x, i) => k === 's' ? x * v[i] : x + v[i]) : v; }
      if (F.piv || F.rp) { const v = flat(F.piv || F.rp).map(Number); S.setPivotWorld(o, new THREE.Vector3(...v)); }
      if (F.cp || F.centerPivots) S.centerPivot(o);
      S.updateXform(o);
    }
    App.manipRefresh?.();
  },
  makeIdentity: (F, pos) => { const ns = nodesFrom(pos); ck('makeIdentity'); for (const o of ns) { if (F.apply && +F.apply) S.freezeTransforms(o); else S.resetTransforms(o); } },
  CenterPivot: (F, pos) => { Sel.select(nodesFrom(pos), 'replace', { echo: false }); App.cmds.run('centerPivot'); },
  // selection
  select: (F, pos) => {
    if (F.cl || F.clear) { Sel.select([], 'replace', { echo: false }); return; }
    if (F.all) { Sel.selectAll(); return; }
    const mode = F.add ? 'add' : F.tgl ? 'toggle' : F.d || F.deselect ? 'deselect' : 'replace';
    const names = pos.flat().filter(x => typeof x === 'string');
    const comp = names.filter(n => /\.(vtx|e|f|cv)\[/.test(n));
    if (comp.length) {
      const by = new Map();
      let kind = null;
      for (const c of comp) {
        const m = c.match(/^(.*?)\.(vtx|e|f|cv)\[(.*)\]$/); if (!m) continue;
        const o = S.findNode(m[1]); if (!o) throw new Error('No object matches name: ' + m[1]);
        kind = m[2] === 'vtx' || m[2] === 'cv' ? 'vertex' : m[2] === 'e' ? 'edge' : 'face';
        const ids = []; for (const part of m[3].split(',')) { if (part === '*') { const n = kind === 'vertex' ? (o.inca.mesh?.v.length ?? o.inca.curve.cvs.length) : kind === 'edge' ? o.inca.mesh.topo.edges.length : o.inca.mesh.f.length; for (let i = 0; i < n; i++) ids.push(i); continue; } const [a, b] = part.split(':').map(Number); for (let i = a; i <= (b ?? a); i++) ids.push(i); }
        if (!by.has(o)) by.set(o, []); by.get(o).push(...ids);
      }
      if (!App.compMode || App.compMode !== kind) { App.hilite = [...by.keys()]; App.sel = [...new Set([...App.sel, ...by.keys()])]; Sel.setMode(kind); }
      Sel.selectComponents([...by].map(([o, ids]) => ({ o, ids })), mode);
      return;
    }
    Sel.select(names.map(n => { const o = S.findNode(n); if (!o) throw new Error('No object matches name: ' + n); return o; }), mode, { echo: false });
  },
  ls: (F, pos) => {
    let list = (F.sl || F.selection) ? App.sel.slice() : S.allDag(true);
    if (F.type) { const t = String(F.type); list = list.filter(o => o.inca.kind === t || (t === 'transform') || (t === 'mesh' && o.inca.kind === 'mesh') || (t.endsWith('Light') && o.inca.light?.type === t) || (t === 'light' && o.inca.kind === 'light') || (t === 'camera' && o.inca.kind === 'camera') || (t === 'nurbsCurve' && o.inca.kind === 'curve') || (t === 'joint' && false)); }
    if (F.lights) list = list.filter(o => o.inca.kind === 'light'); if (F.cameras) list = list.filter(o => o.inca.kind === 'camera'); if (F.geometry) list = list.filter(o => o.inca.kind === 'mesh' || o.inca.kind === 'curve');
    if (F.materials || F.mat) return [...App.mats.values()].map(m => m.name);
    const pat = pos.find(x => typeof x === 'string');
    if (pat) { const re = new RegExp('^' + pat.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*') + '$'); list = list.filter(o => re.test(o.inca.name)); }
    return list.map(o => (F.l || F.long) ? S.fullPath(o) : o.inca.name);
  },
  objExists: (F, pos) => S.findNode(String(pos[0]).split('.')[0]) ? 1 : 0,
  nodeType: (F, pos) => { const o = S.findNode(pos[0]); if (!o) throw new Error('No object matches name: ' + pos[0]); return o.isObject3D ? (o.inca.kind === 'mesh' ? 'transform' : o.inca.kind === 'light' ? 'transform' : 'transform') : o.type; },
  listRelatives: (F, pos) => { const o = nodesFrom(pos)[0]; if (!o) return []; if (F.p || F.parent) { const p = S.dagParent(o); return p ? [p.inca.name] : []; } if (F.s || F.shapes) return o.inca.shapeName ? [o.inca.shapeName] : []; const out = []; if (F.ad || F.allDescendents) o.traverse(c => { if (c !== o && c.inca) out.push(c.inca.name); }); else for (const c of S.dagChildren(o)) out.push(c.inca.name); return out; },
  listHistory: (F, pos) => { const o = nodesFrom(pos)[0]; return o?.inca.history ? o.inca.history.filter(h => !h.name.includes('#')).map(h => h.name).reverse() : []; },
  // attributes
  setAttr: (F, pos) => {
    const target = pos[0]; const { node, attr } = splitAttr(target); const vals = pos.slice(1);
    if (F.lock !== undefined || F.l !== undefined) { if (node.isObject3D) { node.inca.locks[S.resolveAttr(node, attr)] = !!+(F.lock ?? F.l); App.dirty('channels'); } return; }
    ck('setAttr');
    const v = vals.length === 1 ? vals[0] : vals.flat();
    const ok = S.setAttr(node, attr, (typeof v === 'string' && !isNaN(+v)) ? +v : v, { force: true });
    if (!ok) throw new Error(`No attribute '${attr}' on ${nm(node)}`);
    App.anim.autoKey(node, [attr]);
  },
  getAttr: (F, pos) => { const { node, attr } = splitAttr(pos[0]); const v = S.getAttr(node, attr); if (v === undefined) throw new Error(`No attribute '${attr}' on ${nm(node)}`); return Array.isArray(v) ? v.slice() : (typeof v === 'boolean' ? +v : v); },
  rename: (F, pos) => { const [a, b] = pos.length === 1 ? [nm(Sel.lead()), pos[0]] : pos; const o = S.findNode(a); if (!o) throw new Error('No object matches name: ' + a); ck('rename'); S.rename(o, b); return nm(o); },
  parent: (F, pos) => { const ns = nodesFrom(pos); ck('parent'); if (F.w || F.world) { for (const o of ns) S.parentTo(o, null, true); return; } const p = ns[ns.length - 1]; for (const c of ns.slice(0, -1)) S.parentTo(c, p, true); },
  delete: (F, pos) => { const ns = nodesFrom(pos); if (F.ch) { ck('delete history'); for (const o of ns) if (o.inca.kind === 'mesh') S.deleteHistory(o); return; } if (F.channels) { for (const o of ns) App.anim.removeNode(o.inca.id); return; } ck('delete'); for (const o of ns) { App.anim.removeNode(o.inca.id); S.deleteNode(o); } App.emit('selectionChanged'); },
  hide: (F, pos) => { for (const o of nodesFrom(pos)) S.setAttr(o, 'visibility', 0); },
  showHidden: (F, pos) => { const ns = F.all ? S.allDag() : nodesFrom(pos); for (const o of ns) S.setAttr(o, 'visibility', 1); },
  // animation
  currentTime: (F, pos) => { if (F.q || F.query || !pos.length) return App.time.current; App.setTime(+pos[0]); return App.time.current; },
  playbackOptions: (F) => { if (F.q) { if (F.min || F.minTime) return App.time.start; if (F.max || F.maxTime) return App.time.end; return; } if (F.min ?? F.minTime) App.time.start = +(F.min ?? F.minTime); if (F.max ?? F.maxTime) App.time.end = +(F.max ?? F.maxTime); if (F.ast) App.time.animStart = +F.ast; if (F.aet) App.time.animEnd = +F.aet; App.dirty('timeline'); },
  play: (F) => { if (F.st === 0 || F.state === 0) App.timeline?.stop(); else App.timeline?.togglePlay(F.f === 0 ? -1 : 1); },
  setKeyframe: (F, pos) => {
    const names = pos.flat().filter(x => typeof x === 'string');
    let targets = [];
    for (const n of names) { if (n.includes('.')) { const { node, attr } = splitAttr(n); targets.push([node, [attr]]); } else { const o = S.findNode(n); if (!o) throw new Error('No object matches name: ' + n); targets.push([o, null]); } }
    if (!names.length) targets = App.sel.map(o => [o, null]);
    const t = F.t ?? F.time; const at = F.at ?? F.attribute; const v = F.v ?? F.value;
    ck('setKeyframe'); let n = 0;
    for (const [o, attrs] of targets) { const list = attrs || (at ? flat(at) : null); for (const a of (list || App.anim.keyableAttrs(o))) { App.anim.setKey(o, a, { t: t !== undefined ? +t : undefined, v: v !== undefined ? +v : undefined }); n++; } }
    App.anim.evaluate(); return n;
  },
  cutKey: (F, pos) => { ck('cutKey'); for (const o of nodesFrom(pos)) { if (F.at || F.attribute) App.anim.deleteCurve(o, F.at || F.attribute); else App.anim.removeNode(o.inca.id); } App.dirty('timeline', 'graph'); },
  keyframe: (F, pos) => { const o = nodesFrom(pos)[0]; if (!o) return 0; if (F.kc || F.keyframeCount) return App.anim.curvesOf(o).reduce((s, c) => s + c.keys.length, 0); if (F.tc || F.timeChange) return App.anim.keyTimes([o]); return App.anim.keyTimes([o]); },
  // shading
  shadingNode: (F, pos) => { const type = pos[0]; if (!MAT_TYPES[type]) { if (['file', 'checker', 'ramp', 'noise', 'grid'].includes(type)) { const t = App.ui?.createTextureNode?.(type); if (F.n) t.name = F.n; return t?.name; } throw new Error(`Unknown node type ${type}`); } ck('shadingNode'); const m = createMaterial(type, F.n || F.name || null); App.dirty('hypershade'); return m.name; },
  hyperShade: (F, pos) => { if (F.assign) { const m = [...App.mats.values()].find(x => x.name === F.assign); if (!m) throw new Error('No material ' + F.assign); assignMaterial(m); } },
  sets: (F, pos) => { if (F.e && F.fe) { const m = [...App.mats.values()].find(x => x.name === String(F.fe).replace(/SG$/, '')); if (m) assignMaterial(m, nodesFrom(pos)); } },
  // polygon queries
  polyEvaluate: (F, pos) => { const o = nodesFrom(pos)[0]; if (!o || o.inca.kind !== 'mesh') throw new Error('polyEvaluate: no mesh'); const s = o.inca.mesh.stats(); if (F.v || F.vertex) return s.verts; if (F.e || F.edge) return s.edges; if (F.f || F.face) return s.faces; if (F.t || F.triangle) return s.tris; return `vertex: ${s.verts} edge: ${s.edges} face: ${s.faces} triangle: ${s.tris}`; },
  // files & misc
  file: async (F, pos) => { if (F.new) return App.cmds.run('newScene', { force: !!(F.f || F.force) }); if (F.open || F.o) return App.io.openPath(pos[0]); if (F.save || F.s) return App.cmds.run('saveScene'); if (F.i || F.import) return App.io.importPath(pos[0]); if (F.q && (F.sn || F.sceneName)) return App.sceneFile || ''; },
  undo: () => { App.cmds.run('undo'); }, redo: () => { App.cmds.run('redo'); },
  refresh: () => App.requestRender(),
  setToolTo: (F, pos) => { const m = { moveSuperContext: 'move', RotateSuperContext: 'rotate', scaleSuperContext: 'scale', selectSuperContext: 'select' }; App.setTool(m[pos[0]] || 'select'); },
  selectMode: (F) => { if (F.co || F.component) Sel.setMode(App.lastCompMode || 'vertex'); else Sel.setMode(null); },
  selectType: (F) => { if (F.v || F.vertex) Sel.setMode('vertex'); else if (F.e || F.edge) Sel.setMode('edge'); else if (F.f || F.facet) Sel.setMode('face'); },
  inViewMessage: (F) => App.activeViewport?.flash(F.amg || F.msg || ''),
  warning: (F, pos) => { App.emit('warning', '// Warning: ' + pos.join(' ')); },
  error: (F, pos) => { throw new Error(pos.join(' ')); },
  eval: (F, pos) => runMEL(String(pos[0])),
  python: () => { throw new Error('Python is not available in Inca. Switch the command line to JS and use cmds.*'); },
  about: (F) => { if (F.v || F.version) return App.version; return 'Inca ' + App.version; },
  sphereCount: () => 0,
  distanceDimension: () => { throw new Error('distanceDimension is not supported yet'); },
};
const MEL_ALIASES = { DeleteHistory: 'deleteHistory', DeleteAllHistory: 'deleteAllHistory', FreezeTransformations: 'freezeTransformations', ResetTransformations: 'resetTransformations', HideSelectedObjects: 'hideSelection', ShowSelectedObjects: 'showSelection', polyUnite: 'combine', polySeparate: 'separate', polySmooth: 'smooth', polyExtrudeFacet: 'extrude', polyBevel3: 'bevel', polyBevel: 'bevel', polyBridgeEdge: 'bridge', polyMergeVertex: 'merge', polyTriangulate: 'triangulate', polyQuad: 'quadrangulate', polyMirrorFace: 'mirror', polyCloseBorder: 'fillHole', polyPoke: 'poke', polyNormal: 'reverseNormals', polySubdivideFacet: 'addDivisions', polyChipOff: 'extract', revolve: 'revolve', loft: 'loft', planarSrf: 'planarSurface', GraphEditor: 'graphEditor', HypershadeWindow: 'hypershade', ScriptEditor: 'scriptEditor', RenderViewWindow: 'renderView', RenderIntoNewWindow: 'renderCurrent', OutlinerWindow: 'outlinerWindow', SaveScene: 'saveScene', NewScene: 'newScene', OpenScene: 'openScene', Undo: 'undo', Redo: 'redo', Group: 'group', Ungroup: 'ungroup', Parent: 'parent', Unparent: 'unparent', Duplicate: 'duplicate', Delete: 'deleteSel', SelectAll: 'selectAll', InvertSelection: 'invertSelection', FrameSelected: 'frameSelected', FrameAll: 'frameAll', SetKey: 'setKey', PlaybackToggle: 'playToggle', ToggleAutoKey: 'toggleAutoKey', Playblast: 'playblast', playblast: 'playblast' };
function xformCmd(k, F, pos) {
  const v = nums(pos); const ns = nodesFrom(pos.filter(x => typeof x === 'string'));
  if (!ns.length && !App.compMode) throw new Error('Nothing selected');
  ck(k === 't' ? 'move' : k === 'r' ? 'rotate' : 'scale');
  const rel = F.r || F.relative; const axisOnly = F.x || F.y || F.z;
  let vec = v.length >= 3 ? v.slice(0, 3) : v.length === 1 ? [v[0], v[0], v[0]] : [k === 's' ? 1 : 0, k === 's' ? 1 : 0, k === 's' ? 1 : 0];
  if (App.compMode && !pos.some(x => typeof x === 'string')) {
    // move components (world-space, relative)
    for (const o of App.hilite) {
      const verts = Sel.affectedVerts(o); if (!verts.length) continue;
      o.updateWorldMatrix(true, false); const inv = o.matrixWorld.clone().invert(); const pm = o.inca.mesh;
      const c = new THREE.Vector3(); for (const i of verts) c.add(new THREE.Vector3(...pm.v[i]).applyMatrix4(o.matrixWorld)); c.multiplyScalar(1 / verts.length);
      const out = []; const deltas = [];
      for (const i of verts) {
        const w = new THREE.Vector3(...pm.v[i]).applyMatrix4(o.matrixWorld);
        if (k === 't') w.add(new THREE.Vector3(...vec)); else if (k === 's') w.sub(c).multiply(new THREE.Vector3(...vec)).add(c); else w.sub(c).applyEuler(new THREE.Euler(...vec.map(x => x * Math.PI / 180), 'XYZ')).add(c);
        w.applyMatrix4(inv); out.push(w.x, w.y, w.z); deltas.push(w.x - pm.v[i][0], w.y - pm.v[i][1], w.z - pm.v[i][2]);
      }
      S.setMeshPositions(o, verts, out); S.commitTweak(o, verts, deltas); o.userData.compDirty = true;
    }
    App.manipRefresh?.(); return;
  }
  for (const o of ns) {
    const cur = o.inca[k];
    if (rel) o.inca[k] = cur.map((x, i) => k === 's' ? x * vec[i] : x + vec[i]);
    else if (axisOnly) { if (F.x) o.inca[k][0] = vec[0]; if (F.y) o.inca[k][1] = vec[0]; if (F.z) o.inca[k][2] = vec[0]; }
    else o.inca[k] = vec.slice();
    S.updateXform(o);
    App.anim.autoKey(o, k === 't' ? ['translateX', 'translateY', 'translateZ'] : k === 'r' ? ['rotateX', 'rotateY', 'rotateZ'] : ['scaleX', 'scaleY', 'scaleZ']);
  }
  App.manipRefresh?.();
}

// ------------------------------------------------------------------ entry points
const interp = new Interp((s) => App.emit('print', s));
export function runMEL(src, { echoInput = false } = {}) {
  if (echoInput) App.emit('echoInput', src);
  try {
    const r = interp.run(src);
    App.dirty('outliner', 'channels', 'attr', 'timeline');
    if (r !== undefined && r !== null && !(r instanceof Promise)) App.emit('result', fmtVal(r));
    return r;
  } catch (e) {
    if (e && e.ret !== undefined) return e.ret;
    App.emit('error', '// Error: ' + (e.message || e));
    return undefined;
  }
}

// JavaScript API that mirrors maya.cmds:  cmds.polyCube({w:2}); cmds.move(1,2,3,'pCube1',{r:true})
export const cmdsApi = new Proxy({}, {
  get(_, name) {
    if (name === 'help') return () => Object.keys(MEL).sort().join(', ');
    return (...args) => {
      const flags = {}; const pos = [];
      for (const a of args) { if (a && typeof a === 'object' && !Array.isArray(a)) Object.assign(flags, a); else pos.push(a); }
      const F = {}; for (const [k, v] of Object.entries(flags)) F[k] = v === true ? true : v;
      if (MEL[name]) return MEL[name](F, pos.flat());
      if (App.cmds.registry.has(name)) return App.cmds.run(name, flags);
      throw new Error(`cmds.${String(name)} is not a known command`);
    };
  },
});
export function runJS(src) {
  try {
    const print = (...a) => App.emit('print', a.map(x => typeof x === 'object' ? JSON.stringify(x) : String(x)).join(' '));
    const fn = new Function('cmds', 'inca', 'THREE', 'print', 'mel', `"use strict";\n${src}`);
    const r = fn(cmdsApi, App, THREE, print, (s) => runMEL(s));
    App.dirty('outliner', 'channels', 'attr', 'timeline');
    if (r !== undefined) App.emit('result', typeof r === 'object' ? JSON.stringify(r) : String(r));
    return r;
  } catch (e) { App.emit('error', '// Error: ' + (e.message || e)); }
}
App.mel = { run: runMEL, js: runJS, MEL };
