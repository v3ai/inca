// Inca — unique node naming (Maya style: pCube1, pCube2, ...)
import { App } from './app.js';

export function nameExists(n) { return App.names.has(n); }
export function uniqueName(want) {
  let n = String(want || 'node').replace(/[^A-Za-z0-9_:|]/g, '_');
  if (/^[0-9]/.test(n)) n = '_' + n;
  if (!App.names.has(n)) { App.names.set(n, true); return n; }
  const m = n.match(/^(.*?)(\d+)$/);
  let stem = m ? m[1] : n, i = m ? parseInt(m[2], 10) : 1;
  while (App.names.has(stem + i)) i++;
  const r = stem + i; App.names.set(r, true); return r;
}
// claim an exact name if free, otherwise the next unique one
export function registerName(n) { return uniqueName(n); }
export function releaseName(n) { App.names.delete(n); }
export function renameTo(oldName, want) { releaseName(oldName); return uniqueName(want); }
