# JS → Rust parity checklist

Everything the Electron app (0.1.12, `src/`) has that the Rust app (`crates/`) doesn't yet, or does
differently. Items are worked top to bottom and ticked when done. Compiled from the JS command
registry, menus, UI modules and a 0.1.12 screenshot.

## A. Look & layout (what you see at start-up)

- [x] A1. Icon set: the JS SVG icons (shelf, status line, tool box, outliner, panel toolbar, menus) instead of hand-drawn vector icons / text buttons
- [x] A2. Shelves: same shelf tabs and buttons as `shelf.js` (Curves/Surfaces, Poly Modeling, Sculpting, Rigging, Animation, Rendering, FX, Custom), icon buttons with labels on hover
- [x] A3. Status line: same groups and icons — menu set, new/open/save, undo/redo, hierarchy/object/component mode, selection masks, snap toggles (grid/curve/point/live), history, render/IPR/render settings, input line (Absolute / Relative / Rename / Select) with "x y z or name", Symmetry dropdown, panel toggles on the right
- [x] A4. Tool box: Select, Lasso, Paint, Move, Rotate, Scale, Last Tool; Quick Layout buttons at the bottom (single, four, side by side, outliner + persp, …)
- [x] A5. Workspaces: dock layout remembered between sessions (0.1.12 restores Outliner left + Attribute Editor right from prefs), Workspace selector on the right of the menu bar, workspaces (Maya Classic, Modeling Standard/Expert, Sculpting, Animation, Rigging, Rendering, UV Editing)
- [x] A6. Viewport defaults: flat grey background (Alt+B cycles grey / black / dark / light / gradient, saved), JS grid colours; UI body text already 12px like the JS CSS
- [x] A7. Viewport panel (shadow maps still planned, toggle disabled): menus View / Shading / Lighting / Show / Renderer / Panels with the JS items; icon toolbar row (camera attrs, bookmarks, grid, film/resolution gate, wireframe/shaded/textured/lights/shadows/x-ray/isolate, exposure + gamma fields, colour management dropdown)
- [x] A8. Outliner: its own menu bar (Edit / Display / Show / Help), type icons, defaultLightSet / defaultObjectSet, shapes / materials / sets display options
- [x] A9. Attribute Editor: List / Selected / Focus / Attributes / Show / Help menus, node field + Focus + lock, bottom buttons Select / Load Attributes / Copy Tab / Close, collapsible sections
- [x] A10. Time / Range Slider: play backwards, step key buttons, frame field, "film (24 fps)" style fps names, character set / anim layer menus (placeholders), auto key + playback prefs buttons, shift-drag range edits
- [x] A11. Side tabs (vertical, toggle the right dock), dock widths remembered, Hide/Show UI elements (Ctrl+Space)

## B. Commands & menus missing in Rust

- [x] B1. File: Increment & Save, Revert to Saved, Project Window / Set Project, Recent Projects, autosave
- [x] B2. Edit: Cut, Copy/Paste Keys, Delete Keys (current frame), Bake Simulation, Delete All by Type (lights/cameras/curves), Duplicate Special (with options), Duplicate with Transform (Shift+D), Search and Replace Names, Prefix Hierarchy Names, Select Similar, Select by Name, All by Type (geometry/lights/cameras/curves)
- [x] B3. Modify: Match Translation / Rotation / All, Add Attribute, Prefix Hierarchy
- [x] B4. Display: HUD toggles (poly count, fps, frame, camera names, view axis), Hide Unselected / Show Selection / Show Last Hidden, Hide/Show lights/cameras/geometry, Template / Untemplate / Untemplate All, Face Normals, Border Edges, Toggle Background, Frame Selection/All in all views
- [x] B5. Mesh: Cleanup, Collapse, Connect, Chamfer Vertices, Polygon Edges to Curve, Soften/Harden (30°) (Reduce / Conform / Conform Normals are menu-only placeholders in the JS too)
- [x] B6. Mesh Tools: Insert Edge Loop tool (interactive), Multi-Cut, Target Weld, Quad Draw, Sculpt tool (brushes), Make Live
- [x] B7. Curves / Surfaces: Pencil Curve tool, Open/Close (Attach / Detach / Birail / Boundary are menu-only placeholders in the JS too)
- [x] B8. UV: UV Editor (select / move / rotate / scale UVs, flip, rotate 90, normalize, layout, unfold/relax, shells, checker / shading)
- [ ] B8b. UV component mode in the viewport (F12) and the texture image behind the UV Editor
- [x] B9. Selection: Lasso and Paint tools, Soft Select (B, B+drag radius, falloff colours, smooth / linear), Symmetry (object X, world X/Y/Z), cycle component type (Shift+Q), pre-selection highlight
- [x] B10. Manipulators: hold V point snap, hold X grid snap, hold J discrete rotate, W/E/R + LMB marking menus, manipulator size (+/-) readout + reset
- [ ] B10b. Ctrl+click axis lock and MMB drag along the active handle (needs a custom manipulator instead of transform-gizmo)
- [x] B11. Marking menus: RMB object/component menus, Shift+RMB tool menus, Ctrl+RMB conversion; In-View Editor for new history nodes; option boxes (smooth, mirror, add divisions, extrude, bridge, bevel, merge, planar map, revolve, group, chamfer, duplicate special)
- [x] B12. Rigging: IK handles (RP / SC, pole vector, twist), Insert / Mirror / Orient Joint, Unbind Skin, Paint Skin Weights, Cluster, Lattice (+ point tool), Soft Modification, Blend Shape, Delete Deformers, Pole Vector constraint, Remove Constraints, Set Driven Key window; load the JS rig nodes (joints, IK, lattices, constraints) instead of keeping them as locators
- [x] B13. Animation: (Play Backwards, Stop, Copy/Paste Keys done) Bake, Ghosting, Motion Trails, Playblast (video / PNG sequence)
- [x] B14. FX: Create Particles (particle tool), Rewind Simulation, Remove Rigid Body, per-field connections
- [ ] B15. Rendering: IPR, Render Sequence (batch to the project images folder), keep/compare images, exposure / gamma / alpha display in Render View, Hardware renderer option, Assign New Material dialog, Material Attributes
- [ ] B16. Editors: Hypergraph (hierarchy + input/output connections), Connection Editor, Shelf Editor, Hotkey Editor (editable keymap), Modeling Toolkit panel, Component Editor, Preferences window with all JS sections (interface, display, manipulators, animation, time slider, units, undo, files, autosave)
- [ ] B17. Hypershade node graph work area (drag to connect), swatch renders
- [ ] B18. Help: Getting Started, Mouse & Navigation
- [ ] B19. Create: Type (3D text), Distance Tool, Camera and Aim
- [ ] B20. MEL: `connectionInfo`, `distanceDimension`, `instance`, `inViewMessage`, `python`/`echo` stubs; JavaScript `cmds.*` tab equivalent stays Rhai

## C. Behaviour differences to check

- [ ] C1. Hotkeys: compare every JS hotkey (F2–F6 menu sets, Alt+B, Ctrl+Space, B, V, J, Shift+D, Ctrl+Shift+click to shelf, …)
- [ ] C2. Preferences persisted to a prefs file (JS stores menu set, workspace, HUD, shelves, hotkeys)
- [ ] C3. Viewport visuals: pre-selection highlight, component colours, wire colours, selected-face colour as in JS
- [ ] C4. Channel Box: MMB virtual slider in viewport, Ctrl/MMB drag in fields, expressions (`+=2`), int/enum stepping
