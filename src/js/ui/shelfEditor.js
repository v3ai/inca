// Inca — Shelf Editor: manage shelves and their buttons
import { App } from '../core/app.js';
import { h, FloatWin, confirmDialog, toast, promptDialog, showMenu } from './dom.js';
import { icon, ICONS } from './icons.js';
import { categoryOf } from './hotkeyEditor.js';

function injectStyle() {
  if (document.getElementById('inca-she-style')) return;
  document.head.append(h('style', { id: 'inca-she-style', text: `
.shewin .she-main { flex: 1; display: flex; gap: 6px; padding: 6px; min-height: 0; }
.shewin .she-col { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.shewin .she-col > .ttl { font-weight: 700; color: #ddd; padding: 2px 0; }
.shewin .she-shelves { width: 170px; flex: none; }
.shewin .she-items { width: 230px; flex: none; }
.shewin .she-props { flex: 1; }
.shewin .list-box { flex: 1; min-height: 0; }
.shewin .list-box .li { display: flex; align-items: center; gap: 6px; height: 24px; }
.shewin .list-box .li .ico { width: 20px; height: 20px; flex: none; } .shewin .list-box .li .ico svg { width: 20px; height: 20px; }
.shewin .list-box .li.sepi { color: #888; font-style: italic; }
.shewin .she-btns { display: flex; gap: 2px; flex-wrap: wrap; flex: none; }
.shewin .she-btns button { padding: 0 6px; height: 20px; font-size: 11px; flex: 1; }
.shewin .she-form { flex: 1; overflow: auto; display: flex; flex-direction: column; gap: 5px; background: #3c3c3c; border: 1px solid #2a2a2a; padding: 8px; }
.shewin .she-form .dlg-row > label:first-child { width: 80px; }
.shewin .she-form input.wide, .shewin .she-form select { flex: 1; min-width: 0; }
.shewin .she-form textarea { width: 100%; flex: 1; min-height: 60px; background: #262626; }
.shewin .she-preview { width: 34px; height: 34px; border: 1px solid #222; background: #4a4a4a; display: flex; align-items: center; justify-content: center; border-radius: 3px; }
.shewin .she-preview svg { width: 28px; height: 28px; }
.she-iconpick { position: fixed; z-index: 99999; background: #3a3a3a; border: 1px solid #1f1f1f; box-shadow: 0 4px 14px rgba(0,0,0,.6); padding: 6px; display: grid; grid-template-columns: repeat(12, 26px); gap: 2px; max-height: 320px; overflow: auto; }
.she-iconpick div { width: 26px; height: 26px; display: flex; align-items: center; justify-content: center; border-radius: 2px; }
.she-iconpick div:hover { background: #5d7a8f; } .she-iconpick svg { width: 20px; height: 20px; }
` }));
}

export function openShelfEditor() {
  injectStyle();
  const win = new FloatWin('shelfEditor', 'Shelf Editor', { w: 780, h: 480, minW: 560, minH: 300 });
  if (win.reused) { win._refresh?.(); return win; }
  win.el.classList.add('shewin');
  const SH = App.shelf;
  if (!SH || !Array.isArray(SH.shelves)) {
    win.body.append(h('div', { class: 'dlg-body' }, h('div', { class: 'dim', text: 'The shelf is not available yet.' })), h('div', { class: 'dlg-buttons' }, h('button', { text: 'Close', onclick: () => win.close() })));
    return win;
  }
  let si = Math.min(SH.current ?? 0, SH.shelves.length - 1); if (si < 0) si = 0;
  let ii = -1;
  const commit = () => { try { SH.render?.(); SH.save?.(); } catch (e) { console.error(e); } };
  const shelf = () => SH.shelves[si];
  const items = () => (shelf()?.items || (shelf() ? (shelf().items = []) : []));
  const cmdLabel = (id) => App.cmds?.info?.(id)?.label || id;
  const itemLabel = (it) => it.sep ? '— separator —' : (it.label || (it.cmd ? cmdLabel(it.cmd) : (it.mel ? 'MEL script' : it.js ? 'JS script' : 'item')));
  const itemIcon = (it) => it.icon || (it.cmd && App.cmds?.info?.(it.cmd)?.icon) || (it.mel || it.js ? 'scriptEditor' : 'help');

  const shelfList = h('div', { class: 'list-box' });
  const itemList = h('div', { class: 'list-box' });
  const form = h('div', { class: 'she-form' });

  const renderShelves = () => {
    shelfList.innerHTML = '';
    SH.shelves.forEach((s, i) => {
      const row = h('div', { class: 'li' + (i === si ? ' sel' : ''), text: s.name, title: (s.items || []).length + ' items' });
      row.addEventListener('mousedown', () => { si = i; ii = -1; if (SH.current !== undefined) { SH.current = i; SH.render?.(); } renderAll(); });
      row.addEventListener('dblclick', renameShelf);
      shelfList.append(row);
    });
  };
  const renderItems = () => {
    itemList.innerHTML = '';
    items().forEach((it, i) => {
      const row = h('div', { class: 'li' + (i === ii ? ' sel' : '') + (it.sep ? ' sepi' : ''), draggable: 'true' });
      if (!it.sep) row.append(h('span', { class: 'ico', html: icon(itemIcon(it)) }));
      row.append(h('span', { text: itemLabel(it) }));
      row.addEventListener('mousedown', () => { ii = i; renderItems(); renderForm(); });
      // drag to reorder
      row.addEventListener('dragstart', (e) => { e.dataTransfer.setData('text/x-inca-shelf-item', String(i)); e.dataTransfer.effectAllowed = 'move'; });
      row.addEventListener('dragover', (e) => { e.preventDefault(); e.dataTransfer.dropEffect = 'move'; });
      row.addEventListener('drop', (e) => {
        e.preventDefault(); const from = parseInt(e.dataTransfer.getData('text/x-inca-shelf-item'), 10);
        if (isNaN(from) || from === i) return;
        const arr = items(); const [m] = arr.splice(from, 1); arr.splice(i, 0, m); ii = i; commit(); renderItems(); renderForm();
      });
      itemList.append(row);
    });
    if (!items().length) itemList.append(h('div', { class: 'li dim', text: 'Empty shelf' }));
  };

  const pickIcon = (anchor, onPick) => {
    document.querySelectorAll('.she-iconpick').forEach(e => e.remove());
    const pop = h('div', { class: 'she-iconpick' });
    for (const name of Object.keys(ICONS).sort()) { const d = h('div', { title: name, html: ICONS[name] }); d.addEventListener('click', () => { pop.remove(); onPick(name); }); pop.append(d); }
    document.body.append(pop);
    const r = anchor.getBoundingClientRect(); const pr = pop.getBoundingClientRect();
    pop.style.left = Math.max(4, Math.min(r.left, innerWidth - pr.width - 4)) + 'px';
    pop.style.top = Math.max(4, Math.min(r.bottom + 2, innerHeight - pr.height - 4)) + 'px';
    const off = (e) => { if (!pop.contains(e.target)) { pop.remove(); removeEventListener('mousedown', off, true); } };
    setTimeout(() => addEventListener('mousedown', off, true), 0);
  };

  const renderForm = () => {
    form.innerHTML = '';
    const it = items()[ii];
    if (!it) { form.append(h('div', { class: 'dim', text: shelf() ? 'Select a shelf item to edit its properties.' : 'No shelf selected.' })); return; }
    if (it.sep) { form.append(h('div', { class: 'dim', text: 'Separator (no properties).' })); return; }
    const upd = () => { commit(); renderItems(); };
    const preview = h('div', { class: 'she-preview', html: icon(itemIcon(it)) });
    const lbl = h('input', { class: 'wide', value: it.label || '', onchange: (e) => { it.label = e.target.value; upd(); } });
    const icn = h('input', { class: 'wide', value: it.icon || '', placeholder: '(command default)', onchange: (e) => { it.icon = e.target.value.trim() || undefined; if (!it.icon) delete it.icon; preview.innerHTML = icon(itemIcon(it)); upd(); } });
    const pickBtn = h('button', { text: '…', title: 'Choose icon' });
    pickBtn.addEventListener('click', () => pickIcon(pickBtn, (n) => { it.icon = n; icn.value = n; preview.innerHTML = icon(n); upd(); }));
    const listId = 'she-cmds-' + Math.random().toString(36).slice(2);
    const dl = h('datalist', { id: listId }, (App.cmds?.list?.() || []).map(c => h('option', { value: c.id, text: c.label })));
    const cmd = h('input', { class: 'wide', value: it.cmd || '', list: listId, placeholder: 'command id (e.g. polyCube)', onchange: (e) => {
      const v = e.target.value.trim();
      if (v && App.cmds?.registry && !App.cmds.registry.has(v)) toast(`Unknown command "${v}"`);
      if (v) it.cmd = v; else delete it.cmd;
      preview.innerHTML = icon(itemIcon(it)); upd();
    } });
    const lang = it.js !== undefined && it.mel === undefined ? 'js' : 'mel';
    const langSel = h('select', { style: { flex: 'none', width: '110px' } }, h('option', { value: 'mel', text: 'MEL' }), h('option', { value: 'js', text: 'JavaScript' }));
    langSel.value = lang;
    const src = h('textarea', { spellcheck: 'false', value: (lang === 'js' ? it.js : it.mel) || '' });
    const saveSrc = () => {
      const v = src.value;
      delete it.mel; delete it.js;
      if (v.trim()) it[langSel.value] = v;
      upd();
    };
    src.addEventListener('change', saveSrc);
    langSel.addEventListener('change', saveSrc);
    src.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Tab') { e.preventDefault(); src.setRangeText('    ', src.selectionStart, src.selectionEnd, 'end'); } });
    form.append(
      h('div', { class: 'dlg-row' }, h('label', { text: 'Icon' }), preview, icn, pickBtn),
      h('div', { class: 'dlg-row' }, h('label', { text: 'Label' }), lbl),
      h('div', { class: 'dlg-row' }, h('label', { text: 'Command' }), cmd, dl),
      h('div', { class: 'dlg-row' }, h('label', { text: 'Script' }), langSel, h('span', { class: 'dim', text: 'runs instead of the command when set' })),
      src,
      h('div', { class: 'she-btns' },
        h('button', { text: 'Test', title: 'Run this shelf button', onclick: () => { saveSrc(); try { SH.runItem ? SH.runItem(it) : it.cmd && App.cmds.run(it.cmd); } catch (e) { App.emit('error', '// Error: ' + (e.message || e)); } } }),
        h('button', { text: 'Edit in Script Editor', onclick: () => { const se = App.ui?.scriptEditor?.(); if (se?.setText) se.setText(src.value); } })));
  };
  const renderAll = () => { renderShelves(); renderItems(); renderForm(); };
  win._refresh = renderAll;

  // ---- shelf ops
  const uniqueShelfName = (base) => { let n = base, k = 1; while (SH.shelves.some(s => s.name === n)) n = base + (++k); return n; };
  const ask = (t, l, v) => (App.ui?.prompt || promptDialog)(t, l, v);
  const newShelf = async () => {
    const name = await ask('Create New Shelf', 'Shelf name:', uniqueShelfName('Shelf'));
    if (!name) return;
    SH.shelves.push({ name: uniqueShelfName(name.trim().replace(/\s+/g, '_')), items: [] });
    si = SH.shelves.length - 1; ii = -1; if (SH.current !== undefined) SH.current = si; commit(); renderAll();
  };
  async function renameShelf() {
    const s = shelf(); if (!s) return;
    const name = await ask('Rename Shelf', 'Shelf name:', s.name);
    if (!name || name === s.name) return;
    s.name = SH.shelves.some(x => x !== s && x.name === name) ? uniqueShelfName(name) : name; commit(); renderAll();
  }
  const deleteShelf = async () => {
    const s = shelf(); if (!s) return;
    if (await confirmDialog('Delete Shelf', `Delete the shelf "${s.name}" and its ${(s.items || []).length} items?`, ['Delete', 'Cancel']) !== 0) return;
    SH.shelves.splice(si, 1); si = Math.max(0, Math.min(si, SH.shelves.length - 1)); ii = -1;
    if (SH.current !== undefined) SH.current = Math.min(SH.current, Math.max(0, SH.shelves.length - 1));
    commit(); renderAll();
  };
  const moveShelf = (d) => {
    const j = si + d; if (j < 0 || j >= SH.shelves.length) return;
    const a = SH.shelves; [a[si], a[j]] = [a[j], a[si]];
    if (SH.current === si) SH.current = j; else if (SH.current === j) SH.current = si;
    si = j; commit(); renderAll();
  };
  // ---- item ops
  const moveItem = (d) => { const a = items(); const j = ii + d; if (ii < 0 || j < 0 || j >= a.length) return; [a[ii], a[j]] = [a[j], a[ii]]; ii = j; commit(); renderItems(); };
  const deleteItem = () => { const a = items(); if (ii < 0 || ii >= a.length) return; a.splice(ii, 1); ii = Math.min(ii, a.length - 1); commit(); renderItems(); renderForm(); };
  const insertItem = (it) => { if (!shelf()) return; const a = items(); const at = ii >= 0 ? ii + 1 : a.length; a.splice(at, 0, it); ii = at; commit(); renderItems(); renderForm(); };
  const addCommand = (e) => {
    const byCat = new Map();
    for (const c of App.cmds?.list?.() || []) { const k = categoryOf(c); if (!byCat.has(k)) byCat.set(k, []); byCat.get(k).push(c); }
    const sub = (arr) => arr.sort((a, b) => String(a.label).localeCompare(String(b.label))).map(c => ({ label: c.label + '  (' + c.id + ')', icon: c.icon, fn: () => insertItem({ cmd: c.id, label: c.label, icon: c.icon }) }));
    showMenu([...[...byCat].sort((a, b) => a[0].localeCompare(b[0])).map(([k, arr]) => ({ label: k, sub: () => sub(arr) })), '-', { label: 'Empty MEL button', fn: () => insertItem({ label: 'mel', icon: 'scriptEditor', mel: '' }) }, { label: 'Empty JavaScript button', fn: () => insertItem({ label: 'js', icon: 'scriptEditor', js: '' }) }], e.clientX, e.clientY);
  };
  const duplicateItem = () => { const it = items()[ii]; if (it) insertItem(JSON.parse(JSON.stringify(it))); };

  const B = (text, fn, title) => h('button', { text, title: title || text, onclick: fn });
  win.body.append(
    h('div', { class: 'she-main' },
      h('div', { class: 'she-col she-shelves' }, h('div', { class: 'ttl', text: 'Shelves' }), shelfList,
        h('div', { class: 'she-btns' }, B('New', newShelf, 'Create a new shelf'), B('Rename', renameShelf), B('Delete', deleteShelf)),
        h('div', { class: 'she-btns' }, B('▲ Up', () => moveShelf(-1), 'Move shelf up'), B('▼ Down', () => moveShelf(1), 'Move shelf down'))),
      h('div', { class: 'she-col she-items' }, h('div', { class: 'ttl', text: 'Shelf Contents' }), itemList,
        h('div', { class: 'she-btns' }, B('▲', () => moveItem(-1), 'Move item up'), B('▼', () => moveItem(1), 'Move item down'), B('Delete', deleteItem, 'Delete item'), B('Duplicate', duplicateItem)),
        h('div', { class: 'she-btns' }, B('Add Command…', addCommand, 'Insert a command button'), B('Add Separator', () => insertItem({ sep: true })))),
      h('div', { class: 'she-col she-props' }, h('div', { class: 'ttl', text: 'Item Properties' }), form)),
    h('div', { class: 'dlg-buttons' },
      h('button', { text: 'Save All Shelves', onclick: () => { commit(); toast('Shelves saved'); } }),
      h('button', { text: 'Close', onclick: () => win.close() })));

  win.el.addEventListener('keydown', (e) => {
    const t = e.target; e.stopPropagation();
    if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT')) return;
    if (e.key === 'Delete' || e.key === 'Backspace') { deleteItem(); e.preventDefault(); }
    else if (e.key === 'ArrowUp' && ii > 0) { ii--; renderItems(); renderForm(); }
    else if (e.key === 'ArrowDown' && ii < items().length - 1) { ii++; renderItems(); renderForm(); }
  });
  win.onResize = () => {};
  renderAll();
  return win;
}

App.ui = App.ui || {};
App.ui.shelfEditor = openShelfEditor;
