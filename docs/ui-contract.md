# Inca UI module contract

Inca is an Electron + three.js (0.186) desktop app that recreates the Autodesk Maya 2020 workflow.
Renderer code is plain ES modules loaded from `src/index.html` with an import map:
`three`, `three/addons/…`, `three-mesh-bvh`, `three-gpu-pathtracer` (0.0.26). No bundler.

## Global state
`src/js/core/app.js` exports `App` (also `window.App`). Important fields:
- `App.scene` THREE.Scene, `App.world` root Group holding DAG nodes (each DAG node is a `THREE.Group`
  with an `inca` record: `{id,name,kind:'mesh'|'group'|'light'|'camera'|'curve'|'locator', t,r,s, ...}`).
- `App.nodes` Map id → DAG object or DG record (materials `{id,name,type,attrs,maps,kind:'material'}`,
  textures `{kind:'texture'}`, history `{kind:'history'}`).
- `App.sel` selected DAG objects (last = lead). `App.compMode`, `App.hilite`.
- `App.mats`, `App.texs`, `App.defaultMatId`, `App.layers`.
- `App.time = {current,start,end,animStart,animEnd,fps,playing,autoKey,...}`
- `App.anim` (core/anim.js `Anim`), `App.cmds` (core/commands.js `Cmds`), `App.undo` (`Undo`).
- `App.prefs` persisted preferences (see below), `App.savePrefs()`, `App.applyPrefs()`.
- `App.renderSettings = { width, height, camera:'persp', renderer:'path'|'raster', samples, bounces,
   toneMapping:'aces'|'linear'|'none', exposure, background:[r,g,b], transparentBg:false, filename:'image', format:'png' }`
- `App.viewports` (ui/viewport.js `Viewport` instances), `App.activeViewport`.
- Events: `App.on(ev, fn)` / `App.emit(ev, data)`. Main ones:
  - `'refresh'` (data = Set of dirty flags: 'outliner','channels','attr','timeline','graph','hypershade','layers','title', ...) – batched once per frame after `App.dirty(...)`.
  - `'selectionChanged'`, `'sceneLoaded'`, `'timeChanged'` (after `App.setTime`), `'materialChanged'`(rec), `'nodeDeleted'`(o)
  - `'echo'`/`'result'`/`'warning'`/`'error'` (string) – script output. Every entry is also appended to
    `App.scriptLog` (array of `{type:'echo'|'result'|'warning'|'error'|'input', text}`) and announced with `'log'`(entry).
- `App.setTime(frame)` sets current time, evaluates animation, emits 'timeChanged'.
- `App.requestRender()` redraws all viewports.
- `App.help(text)` writes the help line at the bottom.
- `App.ui` — object of UI entry points. Each module **assigns its own functions** onto `App.ui`
  (e.g. `App.ui.graphEditor = openGraphEditor`). Shell-provided ones you can call:
  `App.ui.showAttr(rec)` (show a node in the Attribute Editor dock),
  `App.ui.attrPanel(container, rec) -> { refresh(), destroy() }` (embed an attribute editor for a node/material/texture in your own container),
  `App.ui.prompt(title,label,value)` → Promise<string|null>, `App.ui.toggleRightPanel(name)`.
- Undo: call `Undo.checkpoint('label')` (core/undo.js) **before** mutating scene state.
- Native (Electron) API: `window.incaNative` (see preload.js): openDialog, saveDialog, readText, readBinary,
  writeText, writeBinary, exists, mkdir, readdir, paths, showItem. It may be undefined in a browser — guard it.
- `App.project` = current project root folder path (string) or null.

## Prefs (`App.prefs`)
```
{ undoLevels, manipSize, grid:{size,divisions,major,spacing}, hud:{polyCount,fps,frame,camNames,viewAxis},
  bgMode, smoothDivisions, defaultInTangent, defaultOutTangent, playbackSpeed (0=every frame,1=real-time,0.5,2),
  linearUnit:'cm', angularUnit:'deg', uiScale:1, opt:{}, hotkeys:{combo:cmdId}, shelves, recent:[], workspace,
  orthoTumble, menuSet, autosave:{on,interval}, primitivesInteractive:false }
```
After changing, call `App.applyPrefs()` then `App.savePrefs()`.

## Hotkeys (`App.hotkeys`)
`{ map: Map<combo,cmdId>, defaults: Map, bind(combo, cmdId), unbind(combo), comboOf(cmdId), reset(), save() }`.
Combos look like `"Ctrl+Shift+D"`, `"Alt+V"`, `"F8"`, `"W"`, `"Delete"`. All commands: `App.cmds.list()` → `[{id,label,...}]`.

## Shelf (`App.shelf`)
`{ shelves:[{name, items:[{cmd, label, icon, mel?, js?}|{sep:true}]}], current (index), addToCurrent(cmdId,label), render(), save(), runItem(item) }`

## DOM toolkit (`src/js/ui/dom.js`)
`h(tag, props, ...children)`, `iconBtn(name,title,fn)`, `menuBar(container, [{label, items}])`,
`showMenu(items, x, y)`, `FloatWin(id, title, {w,h,onClose})` (`.body`, `.close()`, `.onResize`, `.reused`),
`numField(value, onChange, {step,int})`, `colorPicker(x,y,rgb,onChange,onDone)`, `colorToCss`,
`optionsDialog(title, fields, onApply)`, `promptDialog`, `confirmDialog`, `toast`, `drag(el,onMove,getStart,onEnd)`, `fmt`.
Menu item shape: `{label, cmd?, fn?, hk?, check?:()=>bool, enabled?:()=>bool, sub?:items|()=>items, opt?}` or `'-'` or `{section:'Title'}`.
Icons: `src/js/ui/icons.js` `icon(name)` returns SVG markup (see its keys).

## Styling
`src/css/inca.css` already defines Maya-dark styles: `.fwin`, `.pmenubar`, `.ptoolbar`, `.pbody`,
`.hs*` (hypershade), `.ge*` (graph editor), `.rv-*` (render view), `.se-*` (script editor), `.list-box`, `.dlg-*`, `.frame*` (collapsible frames).
If you need more CSS, inject one `<style>` element from your module (id it to avoid duplicates). Do not edit inca.css.
