# Inca architecture

Inca is a native Rust application. It started as a rewrite of the Electron/three.js app (versions up
to 0.1.12, still in the git history) and replaced it from 0.2.0.

## Principles

1. **Behave like Maya.** Names, hotkeys, MEL, menus and workflow follow Maya; the Electron app's
   behaviour is the reference where Maya leaves room.
2. **Use good existing crates** for everything that isn't Inca-specific: GUI, GPU, maths, file
   formats, booleans, physics and scripting. Inca-specific code is the scene graph, the construction
   history, the polygon kernel and the Maya-style UI.
3. **Keep the file format.** The `.inca` JSON layout is the one the Electron app used, so old scenes
   open (meshes, history, materials, lights, cameras, curves, animation, layers; rigging records are
   converted to construction history on load).
4. **One command path.** Menu items, shelf buttons and hotkeys run MEL through the same interpreter as
   the command line and Script Editor. Every action echoes its command and undoes as one step.

## Layout

```
Cargo.toml                 workspace
crates/inca-core/          engine, no GUI (unit-tested)
  math.rs                  glam helpers, Maya rotate orders, rays, boxes
  mesh.rs                  PolyMesh n-gon kernel + modeling ops
  primitives.rs            poly + NURBS primitives, parameter tables
  curves.rs                B-spline/EP/arc curves, revolve/loft/extrude/planar
  ops.rs                   construction-history node types
  booleans.rs              mesh booleans via boolmesh
  scene.rs                 DAG, history evaluation, attributes, undo, .inca format
  selection.rs             grow/shrink/convert/loops/rings/borders
  materials.rs             Lambert/Blinn/Phong/Standard Surface/Surface Shader + textures
  anim.rs                  anim curves, Maya tangents and infinity
  mel.rs                   MEL lexer, parser and interpreter
  commands.rs              Maya command set used by MEL and the UI
  io.rs                    .inca, OBJ/MTL, STL, glTF/GLB
  render.rs                Inca Path Tracer (CPU, BVH, rayon)
  rig.rs, skin.rs          joints, IK, driven keys, motion paths, skinning
  deformers.rs             nonlinear deformers, cluster, soft mod, lattice, blend shapes
  dynamics.rs              particles, fields, rigid bodies (rapier3d)
  text.rs                  3D type from font outlines
crates/inca/               the application (eframe/egui + wgpu)
  app.rs                   state, MEL execution, playback, frame loop
  actions.rs               action table, menu MEL, hotkeys
  menus.rs                 menu bar + menu sets
  toolbar.rs               status line, shelves, tool box
  panels.rs                Outliner, Channel Box, Layer Editor, Attribute Editor, Tool Settings
  timeline.rs              Time/Range Slider, playback, command line, help line
  viewport/                cameras, navigation, picking, draw lists + wgpu renderer
  vp_ui.rs                 viewport panes, panel menus, selection, manipulators, HUD
  windows.rs               Script Editor, Graph Editor, Dope Sheet, Render Settings, dialogs
  hypershade.rs            Hypershade: swatches, node graph, property editor
  render_view.rs           Render View, IPR, Render Sequence, hardware renderer
  hypergraph.rs            Hypergraph and Connection Editor
  rig_tools.rs             IK Handle, Lattice Point, Paint Skin Weights tools, Set Driven Key
  mesh_tools.rs            Insert Edge Loop, Multi-Cut, Target Weld, Sculpt, Quad Draw
  uv_editor.rs             UV Editor
  prefs.rs, hotkey_editor.rs, component_editor.rs, anim_extras.rs (ghosts, trails, playblast)
  scripting.rs             Rhai scripting tab
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
| Physics | `rapier3d` | active/passive rigid bodies (convex hulls / triangle meshes) |

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
| Physics | `rapier3d` | active/passive rigid bodies (convex hulls / triangle meshes) |

## Status

[FEATURES.md](../FEATURES.md) lists what works; [PARITY.md](PARITY.md) is the checklist that took the
Rust app to feature parity with Electron 0.1.12, including the re-check of every closed GitHub issue.

## Build, run, test

```
cargo run --release -p inca           # the app (optionally: cargo run -p inca -- scene.inca)
cargo test                             # kernel, history, MEL, IO, booleans, path tracer, picking
```

Linux needs the usual windowing libraries (X11/Wayland, `libxkbcommon-x11`) and a Vulkan or
GL driver. Mesa's lavapipe works for headless runs.

Drive the real app from a script and capture the window:

```
INCA_TEST=script.mel INCA_SHOT=out.png [INCA_SHOT_FRAME=12] cargo run -p inca
```

Lines starting with `//!ui <action>` in the script run UI actions (e.g. `//!ui layoutFour`,
`//!ui renderCurrent`).
