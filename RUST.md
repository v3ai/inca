# Inca in Rust

This branch rebuilds Inca as a native Rust application. The Electron/three.js app in `src/`
stays in the tree as the reference for features, workflow and file formats.

## Approach

The first attempt translated the JavaScript line by line, aiming for a bit-exact clone. It
spent its effort reproducing JS number formatting and three.js maths, and never got as far as
a window you could use. This attempt goes the other way:

1. **Features first, parity later.** Get a working app with most of Inca's features, then
   close the gaps. Behaviour should match Inca and Maya (names, hotkeys, MEL, workflow), but
   the code doesn't have to mirror the JS.
2. **Use good existing crates** for everything that isn't Inca-specific: GUI, GPU, maths,
   file formats, booleans, physics and scripting. Inca-specific code is the scene graph, the
   construction history, the polygon kernel and the Maya-style UI.
3. **Keep the file format.** The `.inca` JSON layout is unchanged, so scenes move between the
   two apps in both directions (meshes, history, materials, lights, cameras, curves,
   animation, layers). Rigging data from the JS app is carried through untouched.
4. **One command path.** Menu items, shelf buttons and hotkeys run MEL through the same
   interpreter as the command line and Script Editor. Every action echoes its command and
   undoes as one step.

## Layout

```
Cargo.toml                 workspace
crates/inca-core/          engine, no GUI (unit-tested)
  math.rs                  glam helpers, Maya rotate orders, rays, boxes
  mesh.rs                  PolyMesh n-gon kernel + modeling ops      (polymesh.js)
  primitives.rs            poly + NURBS primitives, parameter tables (primitives.js)
  curves.rs                B-spline/EP/arc curves, revolve/loft/extrude/planar (curves.js)
  ops.rs                   construction-history node types           (ops.js)
  booleans.rs              mesh booleans via boolmesh                (booleans.js)
  scene.rs                 DAG, history evaluation, attributes, undo, .inca format (scene.js, undo.js, names.js)
  selection.rs             grow/shrink/convert/loops/rings/borders   (selection.js)
  materials.rs             Lambert/Blinn/Phong/Standard Surface/Surface Shader + textures (materials.js)
  anim.rs                  anim curves, Maya tangents and infinity   (anim.js)
  mel.rs                   MEL lexer, parser and interpreter         (mel.js)
  commands.rs              Maya command set used by MEL and the UI   (commands.js)
  io.rs                    .inca, OBJ/MTL, STL, glTF/GLB             (io.js)
  render.rs                Inca Path Tracer (CPU, BVH, rayon)        (renderView.js)
crates/inca/               the application (eframe/egui + wgpu)
  app.rs                   state, MEL execution, playback, frame loop (main.js, app.js)
  actions.rs               action table, menu MEL, hotkeys           (commands.js, hotkeys)
  menus.rs                 menu bar + menu sets                      (menus.js)
  toolbar.rs               status line, shelves, tool box            (statusline.js, shelf.js)
  panels.rs                Outliner, Channel Box, Layer Editor, Attribute Editor, Tool Settings
  timeline.rs              Time/Range Slider, playback, command line, help line
  viewport/                cameras, navigation, picking, draw lists + wgpu renderer (viewport.js)
  vp_ui.rs                 viewport panes, panel menus, selection, manipulators, HUD
  windows.rs               Script Editor, Hypershade, Graph Editor, Render View/Settings, dialogs
  scripting.rs             Rhai scripting tab (counterpart of the JS `cmds.*` tab)
  icons.rs, theme.rs       vector icons, Maya dark theme
  test_hook.rs             INCA_TEST / INCA_SHOT automation
```

## Crates we build on

| Need | Crate | Notes |
|---|---|---|
| Window, UI widgets | `eframe` / `egui` 0.36 | immediate-mode UI with menus, panels, windows and drag-and-drop |
| GPU | `wgpu` (through `egui-wgpu`) | viewports render offscreen (MSAA + depth) and are shown as egui images |
| Manipulators | `transform-gizmo-egui` | move/rotate/scale handles; Inca applies their deltas around the pivot |
| Maths | `glam` (f64) | matrices, quaternions, Euler orders |
| Plots | `egui_plot` | Graph Editor curves |
| Booleans | `boolmesh` | pure-Rust Manifold port; union/difference/intersection with live history |
| Triangulation | `earcutr` | concave n-gons |
| OBJ / STL / glTF | `tobj`, `stl_io`, `gltf` | import; OBJ/STL/GLB exporters are written in-house |
| Images | `image` | textures, screenshots, render output |
| Parallelism | `rayon` | path tracer |
| Scripting | `rhai` | second scripting language next to MEL |
| File dialogs | `rfd` | native open/save dialogs |
| Physics (planned) | `rapier3d` | rigid bodies for the FX menu set |

## Status against FEATURES.md

✅ working · 🟡 partial · ⏭ not started

| Area | Status |
|---|---|
| Shell: Maya dark theme, menu bar with menu sets, status line, shelves, tool box, Channel Box / Layer Editor, Attribute Editor, Outliner (drag to parent, rename, filter), Time/Range Slider, command line, help line, Script Editor | ✅ |
| Hotbox, marking menus (RMB menu only), Shelf Editor, workspaces, Hotkey Editor (read-only list), preferences (small subset) | 🟡 |
| Files: New/Open/Save/Save As, `.inca` (JS-compatible), import OBJ/STL/glTF/GLB/.inca, export OBJ+MTL/STL/GLB/.inca | ✅ |
| Projects, recent files, autosave, FBX | ⏭ |
| Viewports: persp/top/front/side, single/four/two layouts, Space toggle, Alt+LMB/MMB/RMB and wheel navigation, F/A framing, wireframe/shaded, default/all/flat lighting, wire-on-shaded, X-ray, backface culling, smooth mesh preview 1/2/3, grid, HUD, isolate select, look through camera | ✅ |
| Bookmarks, film/resolution gates, textured viewport, shadows in the viewport | ⏭ |
| Selection: click/Shift/Ctrl/Ctrl+Shift, marquee, select all/inverse/hierarchy/by name, components (vertex/edge/face) with occlusion, grow/shrink, loops/rings/borders, convert | ✅ |
| Lasso/paint select, soft selection, symmetry | ⏭ |
| Transforms: move/rotate/scale manipulators (world/object, grid snap), pivots, center pivot, freeze, reset, snap to ground, duplicate, group/ungroup, parent/unparent, copy/paste, delete | ✅ |
| Edit Pivot (D) mode, point snapping, discrete manipulators | ⏭ |
| Poly modeling: all 12 primitives with live history; extrude (faces/edges), bridge, insert edge loop, merge/merge to center, poke, delete edge/vertex, extract/duplicate faces, fill hole, triangulate/quadrangulate, add divisions, reverse, soften/harden, combine, separate, smooth, mirror, booleans, UV projections | ✅ |
| Bevel, multi-cut, target weld, chamfer, quad draw, UV editor, sculpting | ⏭ |
| NURBS & curves: tessellated NURBS primitives with isoparms, circle/square/`curve` command, revolve/loft/extrude/planar with live history, rebuild/reverse, NURBS→poly | 🟡 (interactive curve tools ⏭) |
| Materials: the five Inca material types, five texture types, Hypershade (create, edit, assign, map textures), per-face assignment | ✅ (node graph view ⏭) |
| Lights: ambient, directional, point, spot, area, sky dome; viewport "use all lights" | ✅ |
| Animation: set key (S, Shift+W/E/R), auto key, playback, step frames/keys, Graph Editor (move keys, tangents, infinity), key ticks | ✅ |
| Dope Sheet, set driven key, motion paths, playblast | ⏭ |
| Rigging, deformers, constraints | ⏭ (data from JS scenes is kept) |
| Rendering: Inca Path Tracer (progressive CPU, GI, glass, metals, area/spot lights, textures), Render Settings, save image | ✅ |
| Render sequence, hardware renderer | ⏭ |
| Scripting: MEL (variables, arrays, vectors, procs, loops, switch, backticks, about 120 commands), Rhai, command reference window | ✅ |
| Hypergraph, Connection Editor (`connectAttr` etc. work from MEL) | 🟡 |
| Dynamics & FX (particles, fields, rigid bodies via rapier3d) | ⏭ |

## Roadmap

1. **Now:** a working app covering the table above.
2. **Modeling depth:** bevel, multi-cut, target weld, edit pivot, soft selection and symmetry,
   UV editor, interactive CV/EP/arc curve tools, Quad Draw.
3. **Animation & rigging:** Dope Sheet, joints and IK, skinning, deformers, constraints
   (reading the rigging data the JS app already saves).
4. **FX:** particles and fields, rigid bodies via `rapier3d`.
5. **Polish:** textured viewport and viewport shadows, hotbox, Shelf Editor, Hotkey Editor,
   preferences file, projects and autosave, packaging (cargo-dist / AppImage / dmg / msi).

## Build, run, test

```
cargo run --release -p inca           # the app (optionally: cargo run -p inca -- scene.inca)
cargo test                             # kernel, history, MEL, IO, booleans, path tracer, picking
```

Linux needs the usual windowing libraries (X11/Wayland, `libxkbcommon-x11`) and a Vulkan or
GL driver. Mesa's lavapipe works for headless runs.

Drive the real app from a script and capture the window, the way the JS app's `INCA_TEST` does:

```
INCA_TEST=script.mel INCA_SHOT=out.png [INCA_SHOT_FRAME=12] cargo run -p inca
```

Lines starting with `//!ui <action>` in the script run UI actions (e.g. `//!ui layoutFour`,
`//!ui renderFrame`).
