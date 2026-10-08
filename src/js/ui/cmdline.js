// Inca — command line, result line, script log and help line
import { App } from '../core/app.js';
import { h, iconBtn } from './dom.js';
import { runMEL, runJS, fmtVal } from '../core/mel.js';

App.scriptLog = [];
App.logPush = (entry) => {
  App.scriptLog.push(entry);
  if (App.scriptLog.length > 5000) App.scriptLog.splice(0, 1000);
  App.emit('log', entry);
};
let resultEl = null;
function showResult(text, cls = '') {
  if (!resultEl) return;
  resultEl.value = text; resultEl.className = cls;
}
const fmtLine = (type, s) => {
  s = String(s ?? '');
  if (type === 'result' && !s.startsWith('//')) return '// Result: ' + s + ' //';
  if (type === 'warning' && !s.startsWith('//')) return '// Warning: ' + s;
  if (type === 'error' && !s.startsWith('//')) return '// Error: ' + s;
  return s;
};
App.on('echo', (s) => { App.logPush({ type: 'echo', text: s }); });
App.on('print', (s) => { App.logPush({ type: 'echo', text: String(s).replace(/\n$/, '') }); });
App.on('result', (s) => { const t = fmtLine('result', s); App.logPush({ type: 'result', text: t }); showResult(t); });
App.on('warning', (s) => { const t = fmtLine('warning', s); App.logPush({ type: 'warning', text: t }); showResult(t, 'warn'); App.help(t); });
App.on('error', (s) => { const t = fmtLine('error', s); App.logPush({ type: 'error', text: t }); showResult(t, 'err'); App.help(t); console.warn(t); });

const history = []; let hi = -1;
function buildCmdline() {
  const bar = document.getElementById('cmdline');
  let lang = App.prefs.opt.cmdLang || 'MEL';
  const langBtn = h('button', { id: 'cmd-lang', text: lang, title: 'Click to switch between MEL and JavaScript' });
  langBtn.addEventListener('click', () => { lang = lang === 'MEL' ? 'JS' : 'MEL'; langBtn.textContent = lang; App.prefs.opt.cmdLang = lang; App.savePrefs(); });
  const inp = h('input', { id: 'cmd-input', spellcheck: 'false', placeholder: '' });
  resultEl = h('input', { id: 'cmd-result', readonly: true });
  inp.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.key === 'Enter') {
      const src = inp.value.trim(); if (!src) return;
      history.push(src); hi = history.length;
      App.logPush({ type: 'input', text: src });
      showResult('');
      try {
        const r = lang === 'MEL' ? runMEL(src) : runJS(src);
        if (r instanceof Promise) r.then((v) => { if (v !== undefined && v !== null && v !== '') App.emit('result', fmtVal(v)); }).catch(err => App.emit('error', err.message || String(err)));
      } catch (err) { App.emit('error', err.message || String(err)); }
      inp.value = '';
    } else if (e.key === 'ArrowUp') { if (hi > 0) { hi--; inp.value = history[hi]; } e.preventDefault(); }
    else if (e.key === 'ArrowDown') { if (hi < history.length - 1) { hi++; inp.value = history[hi]; } else { hi = history.length; inp.value = ''; } e.preventDefault(); }
    else if (e.key === 'Escape') inp.blur();
  });
  bar.append(langBtn, inp, resultEl, iconBtn('scriptEditor', 'Script Editor', () => App.cmds.run('scriptEditor')));
}
App.on('beforeUI', buildCmdline);
App.ui = App.ui || {};
App.ui.focusCommandLine = () => document.getElementById('cmd-input')?.focus();
