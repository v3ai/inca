// Inca — Electron main process
const { app, BrowserWindow, ipcMain, dialog, Menu, shell } = require('electron');
const path = require('path');
const fs = require('fs');

let win = null;
let pendingOpenFile = null;

const prefsPath = () => path.join(app.getPath('userData'), 'inca-prefs.json');

// three.js add-ons are served from vendor/ (electron-builder never packages node_modules/*/examples).
// When running from source, (re)create vendor/ if it is missing so a skipped `npm install` hook can't blank the app.
function ensureVendor() {
  const dst = path.join(__dirname, 'vendor', 'three-addons');
  const src = path.join(__dirname, 'node_modules', 'three', 'examples', 'jsm');
  try {
    if (fs.existsSync(path.join(dst, 'controls', 'TransformControls.js'))) return;
    if (!fs.existsSync(src)) { console.error('Inca: node_modules/three is missing - run "npm install" in', __dirname); return; }
    fs.cpSync(src, dst, { recursive: true, filter: (p) => !p.endsWith('.d.ts') });
    console.log('Inca: copied three.js add-ons to', dst);
  } catch (e) { console.error('Inca: could not prepare vendor/three-addons:', e.message); }
}

function createWindow() {
  ensureVendor();
  win = new BrowserWindow({
    width: 1600,
    height: 960,
    minWidth: 1000,
    minHeight: 640,
    backgroundColor: '#444444',
    title: 'Inca',
    icon: path.join(__dirname, 'build', 'icon.png'),
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
    },
  });

  // Inca draws its own Maya-style menu bar inside the window.
  if (process.platform === 'darwin') {
    Menu.setApplicationMenu(Menu.buildFromTemplate([
      { role: 'appMenu' },
      { role: 'editMenu' },
      { role: 'windowMenu' },
    ]));
  } else {
    Menu.setApplicationMenu(null);
  }

  win.loadFile(path.join(__dirname, 'src', 'index.html'));
  win.webContents.on('before-input-event', (e, input) => {
    if (input.type === 'keyDown' && input.key === 'F12' && input.control && input.shift) {
      win.webContents.toggleDevTools();
    }
  });
  win.on('close', (e) => {
    if (win.__forceClose) return;
    e.preventDefault();
    win.webContents.send('app:request-close');
  });
  win.webContents.setWindowOpenHandler(({ url }) => { shell.openExternal(url); return { action: 'deny' }; });
}

// ---------- IPC: file system & dialogs ----------
ipcMain.handle('dialog:open', async (_e, opts = {}) => {
  const r = await dialog.showOpenDialog(win, {
    title: opts.title || 'Open',
    defaultPath: opts.defaultPath,
    filters: opts.filters,
    properties: opts.directory ? ['openDirectory', 'createDirectory'] : ['openFile'],
  });
  return r.canceled ? null : r.filePaths[0];
});
ipcMain.handle('dialog:save', async (_e, opts = {}) => {
  const r = await dialog.showSaveDialog(win, {
    title: opts.title || 'Save',
    defaultPath: opts.defaultPath,
    filters: opts.filters,
  });
  return r.canceled ? null : r.filePath;
});
ipcMain.handle('dialog:confirm', async (_e, opts = {}) => {
  const r = await dialog.showMessageBox(win, {
    type: 'question',
    buttons: opts.buttons || ['Save', "Don't Save", 'Cancel'],
    defaultId: 0, cancelId: (opts.buttons || [1, 2, 3]).length - 1,
    title: opts.title || 'Inca', message: opts.message || '',
  });
  return r.response;
});
ipcMain.handle('fs:readText', (_e, p) => fs.readFileSync(p, 'utf8'));
ipcMain.handle('fs:readBinary', (_e, p) => { const b = fs.readFileSync(p); return b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength); });
ipcMain.handle('fs:writeText', (_e, p, data) => { fs.mkdirSync(path.dirname(p), { recursive: true }); fs.writeFileSync(p, data, 'utf8'); return true; });
ipcMain.handle('fs:writeBinary', (_e, p, data) => { fs.mkdirSync(path.dirname(p), { recursive: true }); fs.writeFileSync(p, Buffer.from(data)); return true; });
ipcMain.handle('fs:exists', (_e, p) => fs.existsSync(p));
ipcMain.handle('fs:mkdir', (_e, p) => { fs.mkdirSync(p, { recursive: true }); return true; });
ipcMain.handle('fs:readdir', (_e, p) => {
  try { return fs.readdirSync(p, { withFileTypes: true }).map(d => ({ name: d.name, dir: d.isDirectory() })); } catch { return []; }
});
ipcMain.handle('app:paths', () => ({
  home: app.getPath('home'), documents: app.getPath('documents'), userData: app.getPath('userData'), sep: path.sep,
}));
ipcMain.handle('prefs:load', () => { try { return JSON.parse(fs.readFileSync(prefsPath(), 'utf8')); } catch { return null; } });
ipcMain.handle('prefs:save', (_e, data) => { fs.mkdirSync(path.dirname(prefsPath()), { recursive: true }); fs.writeFileSync(prefsPath(), JSON.stringify(data, null, 2)); return true; });
ipcMain.handle('app:quit', () => { if (win) { win.__forceClose = true; win.close(); } });
ipcMain.handle('app:setTitle', (_e, t) => { if (win) win.setTitle(t); });
ipcMain.handle('app:openExternal', (_e, url) => shell.openExternal(url));
ipcMain.handle('app:pendingFile', () => { const f = pendingOpenFile; pendingOpenFile = null; return f; });
ipcMain.handle('app:showItem', (_e, p) => shell.showItemInFolder(p));

// file passed on the command line / double-clicked .inca
const argFile = process.argv.find(a => a.toLowerCase().endsWith('.inca'));
if (argFile) pendingOpenFile = path.resolve(argFile);
app.on('open-file', (e, p) => { e.preventDefault(); pendingOpenFile = p; if (win) win.webContents.send('app:open-file', p); });

// ---------- automated UI testing (INCA_TEST=script.js INCA_SHOTS=dir)
ipcMain.handle('test:shot', async (_e, name) => {
  const dir = process.env.INCA_SHOTS || path.join(app.getPath('temp'), 'inca-shots');
  fs.mkdirSync(dir, { recursive: true });
  const img = await win.webContents.capturePage();
  const f = path.join(dir, name + '.png'); fs.writeFileSync(f, img.toPNG()); return f;
});
ipcMain.handle('test:input', (_e, ev) => { win.webContents.sendInputEvent(ev); });
ipcMain.handle('test:log', (_e, s) => { process.stdout.write('[renderer] ' + s + '\n'); });
ipcMain.handle('test:done', () => { if (process.env.INCA_TEST) { win.__forceClose = true; app.quit(); } });
function runTestScript() {
  const f = process.env.INCA_TEST; if (!f) return;
  win.webContents.on('console-message', (_e, level, message) => { if (level >= (+process.env.INCA_LOGLEVEL || 2)) process.stdout.write('[console ' + level + '] ' + message + '\n'); });
  win.webContents.on('did-finish-load', () => {
    const code = fs.readFileSync(f, 'utf8');
    setTimeout(() => win.webContents.executeJavaScript(`(async () => { try { ${code}\n } catch (e) { await incaNative.testLog('TEST ERROR ' + (e.stack || e)); } await incaNative.testDone(); })()`).catch(e => { console.error(e); app.quit(); }), 1500);
  });
}
app.whenReady().then(() => { createWindow(); runTestScript(); });
app.on('window-all-closed', () => app.quit());
app.on('activate', () => { if (BrowserWindow.getAllWindows().length === 0) createWindow(); });
