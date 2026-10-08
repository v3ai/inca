// Inca — About dialog
import { App } from '../core/app.js';
import { h, FloatWin } from './dom.js';
import { icon } from './icons.js';

const CREDITS = [
  ['three.js', 'https://threejs.org', 'MIT'],
  ['three-gpu-pathtracer', 'https://github.com/gkjohnson/three-gpu-pathtracer', 'MIT'],
  ['three-mesh-bvh', 'https://github.com/gkjohnson/three-mesh-bvh', 'MIT'],
  ['Electron', 'https://www.electronjs.org', 'MIT'],
];

function injectStyle() {
  if (document.getElementById('inca-about-style')) return;
  document.head.append(h('style', { id: 'inca-about-style', text: `
.aboutwin .ab-body { flex: 1; display: flex; flex-direction: column; align-items: center; padding: 18px 22px 10px; gap: 6px; overflow: auto; text-align: center; background: linear-gradient(#4a4a4a, #3a3a3a); }
.aboutwin .ab-logo { width: 84px; height: 84px; } .aboutwin .ab-logo svg { width: 84px; height: 84px; }
.aboutwin .ab-name { font-size: 26px; font-weight: 300; color: #fff; letter-spacing: 1px; }
.aboutwin .ab-ver { color: #aaa; font-size: 12px; }
.aboutwin .ab-tag { color: #ddd; margin: 4px 0 8px; }
.aboutwin .ab-sect { width: 100%; text-align: left; font-weight: 700; color: #ddd; border-bottom: 1px solid #555; padding: 6px 0 2px; }
.aboutwin .ab-credits { width: 100%; text-align: left; line-height: 1.6; }
.aboutwin .ab-credits a { color: #8fc3e6; text-decoration: none; cursor: pointer; } .aboutwin .ab-credits a:hover { text-decoration: underline; }
.aboutwin .ab-lic { width: 100%; text-align: left; color: #aaa; font-size: 11px; line-height: 1.45; }
.aboutwin .ab-sys { width: 100%; text-align: left; color: #999; font-size: 11px; font-family: Consolas, monospace; user-select: text; -webkit-user-select: text; }
` }));
}

function openLink(url) {
  if (window.incaNative?.openExternal) window.incaNative.openExternal(url);
  else window.open(url, '_blank', 'noopener');
}

export function openAbout() {
  injectStyle();
  const win = new FloatWin('about', 'About Inca', { w: 440, h: 600, minW: 360, minH: 320 });
  if (win.reused) return win;
  win.el.classList.add('aboutwin');
  const n = window.incaNative;
  const sys = [
    `Version ${App.version || '0.0.0'}`,
    n?.platform ? `Platform: ${n.platform} (desktop)` : 'Platform: browser',
    typeof navigator !== 'undefined' ? navigator.userAgent.replace(/^.*?(Chrome\/[\d.]+).*?(Electron\/[\d.]+)?.*$/, (m, c, e) => [c, e].filter(Boolean).join('  ')) : '',
  ].filter(Boolean);
  const body = h('div', { class: 'ab-body' },
    h('div', { class: 'ab-logo', html: icon('logo') }),
    h('div', { class: 'ab-name', text: 'Inca' }),
    h('div', { class: 'ab-ver', text: 'Version ' + (App.version || '0.0.0') }),
    h('div', { class: 'ab-tag', text: 'A free 3D modeling, animation and rendering application' }),
    h('div', { class: 'ab-sect', text: 'Built with' }),
    h('div', { class: 'ab-credits' }, CREDITS.map(([name, url, lic]) => h('div', {}, h('a', { text: name, title: url, onclick: () => openLink(url) }), h('span', { class: 'dim', text: '  — ' + lic + ' License' })))),
    h('div', { class: 'ab-sect', text: 'License' }),
    h('div', { class: 'ab-lic', text: 'Inca is free software released under the MIT License. Permission is hereby granted, free of charge, to any person obtaining a copy of this software to deal in the software without restriction. The software is provided "as is", without warranty of any kind.' }),
    h('div', { class: 'ab-lic', text: 'Inca is an independent project and is not affiliated with or endorsed by Autodesk, Inc.' }),
    h('div', { class: 'ab-sect', text: 'System' }),
    h('div', { class: 'ab-sys' }, sys.map(s => h('div', { text: s }))));
  win.body.append(body, h('div', { class: 'dlg-buttons' }, h('button', { text: 'Close', onclick: () => win.close() })));
  win.el.addEventListener('keydown', (e) => { if (e.key === 'Escape' || e.key === 'Enter') { e.stopPropagation(); win.close(); } });
  return win;
}

App.ui = App.ui || {};
App.ui.about = openAbout;
