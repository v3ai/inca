# Inca — Master Feature List

Inca is a free, open desktop 3D content-creation app that recreates the workflow and layout of
Autodesk Maya (2020-era UI). It is a native Rust application (egui + wgpu); see
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). The list was derived from the supplied course videos (Maya 2018/2020 UI,
project setup, shelves/prefs/hotkeys, box modeling a sofa, modeling a rocket, lighting, shading,
keyframes and timing) and the *Maya Tutorials Collection* book (Maya 2–4 era: Hypergraph, Hypershade,
NURBS, deformers, skeletons, MEL, dynamics, Artisan sculpting, rendering). The book's screenshots are
old, so the look follows the videos; the book was used for capabilities.

Legend: ✅ built · 🟡 partial / simplified · ⏭ planned

---

## 1. Application shell & interface
- ✅ Maya-style dark theme, title `scene* - Inca - project`, app icon
- ✅ Main menu bar: File, Edit, Create, Select, Modify, Display, Windows + menu-set menus + Help
- ✅ Menu sets (Modeling F2, Rigging F3, Animation F4, FX F5, Rendering F6) swapping the right-hand menus
- ✅ Status line: menu set, new/open/save, undo/redo, hierarchy/object/component modes, object & component masks, lock selection, snap toggles (grid/curve/point/live), construction history, render buttons, numeric input line (absolute/relative/rename/select by name), symmetry, panel toggles
- ✅ Shelves: Curves/Surfaces, Poly Modeling, Sculpting, Rigging, Animation, Rendering, FX, Custom; Ctrl+Shift+click a menu item to add it; right-click to delete or edit; Shelf Editor (new / rename / reorder shelves, MEL buttons, icons); shelves are saved between sessions
- ✅ Tool box (Select, Lasso, Paint, Move, Rotate, Scale, Last tool) + Quick Layout buttons
- ✅ Workspaces (Maya Classic, Modeling Standard/Expert, Sculpting, Animation, Rigging, Rendering, UV Editing)
- ✅ Channel Box / Layer Editor: channels, SHAPES, INPUTS (construction history), virtual sliders (MMB in viewport, Ctrl/MMB-drag in fields; int/enum channels step in whole numbers and respect min/max), expressions in fields (`+=2`); a value typed into a selected channel goes to all selected channels, right-click Key/Lock/Hide/Break, keyed/locked colouring
- ✅ Display layers: create (empty/from selected), V / T / R, colour, select, rename, delete; templated objects (and Template layers) draw as grey wireframes, pink when selected from the Outliner, and can't be picked in viewports
- ✅ Attribute Editor (Ctrl+A) with tabs for transform, shape, history nodes, material and textures; texture map buttons; Copy Tab; lock/focus
- ✅ Outliner (dock), hierarchy, drag to parent/reorder/unparent, inline rename, search, shapes/sets/materials display
- ✅ Time Slider, Range Slider, playback controls, auto key, fps menu, looping, key ticks, shift-drag range edits
- ✅ Command line (MEL), result line, help line, Script Editor
- ✅ Floating editors; side tabs; dock resizing
- ✅ Hotbox (hold Space, click a menu name to open it), RMB marking menus (object, component, Shift+RMB tool menus, Ctrl+RMB conversion)
- ✅ In-View Editor for history nodes (drag the title to move it); option boxes remember settings
- ✅ Preferences (interface, display, manipulators, animation, time slider, units, undo, files, autosave); Hotkey Editor with keyboard map
- ⏭ Arbitrary panel docking / saving custom workspaces, tear-off menus

## 2. Projects & files
- ✅ Project Window / Set Project (standard folder tree + workspace file), recent projects
- ✅ New / Open / Save / Save As / Increment & Save / Revert; native `.inca` scene format (JSON); recent files; autosave
- ✅ Import OBJ (keeps n-gons, MTL colours/textures), glTF/GLB, STL, FBX, Maya ASCII, Inca scenes (including scenes saved by the old Electron app); Export All / Selection to OBJ+MTL, glTF/GLB, STL, FBX, Maya ASCII, Inca
- ✅ Maya ASCII (`.ma`) open, import and export [#29](https://github.com/v3ai/inca/issues/29): transforms and hierarchy, joints with joint orients, meshes (primitive creation nodes stay live construction history; stored polyFaces with UVs and vertex tweaks otherwise), cameras, lights, locators, Lambert / Blinn / Phong / Standard Surface / Surface Shader materials with file textures, shading-group assignments, skinCluster weights, keyframe animation
- ✅ FBX open, import and export [#31](https://github.com/v3ai/inca/issues/31): reads binary and ASCII FBX 7.x (2011 and later), writes binary FBX 7.4; transform hierarchy (pre-rotations become joint orients), meshes with UVs, normals and per-face materials, Lambert / Phong materials with file textures, skeletons, skin clusters with their weights and bind pose, cameras, lights, translate / rotate / scale / visibility animation; unit scale and Z-up files are converted
- ⏭ Maya Binary (`.mb`: Autodesk's undocumented format; Inca says how to resave as `.ma`), FBX blend shapes and constraints, references / proxies, archive scene

## 3. Viewports & navigation
- ✅ Persp/Top/Front/Side, single/four/two/three layouts, tap Space to toggle
- ✅ Alt+LMB tumble, Alt+MMB track, Alt+RMB/wheel dolly; F/A frame, Shift+F/A all views; bookmarks; default view
- ✅ Shading 4/5/6/7, wireframe on shaded, X-ray, backface culling (also hides the wires of back faces and makes them unpickable), flat/none lighting, exposure, background toggle (Alt+B)
- ✅ Smooth mesh preview 1/2/3; resolution & film gates; HUD (poly count, fps, frame, camera names, view axis)
- ✅ Panel menus & toolbar, Isolate Select, Show filters, look through selected, new cameras
- ✅ Face normals / border edge display
- ⏭ Shadows in the viewport (the path tracer renders them)

## 4. Selection & transforms
- ✅ Click/Shift/Ctrl/Ctrl+Shift, marquee, lasso, paint selection, select all/inverse/by type/hierarchy/similar/by name
- ✅ Move/Rotate/Scale manipulators (world/object space), MMB drag along the active handle (dragging a handle makes it active; Ctrl+click an axis to lock it and work in the other two, Ctrl+click the centre for the view plane), Discrete Move/Rotate/Scale with step size, grid snap (X; the centre handle snaps onto the ground grid in persp), point snap (V), curve snap (C), discrete rotate (J), manipulator size +/- (with a size readout and Reset)
- ✅ Hold W / E / R + LMB: tool marking menus (axis orientation, edit pivot, soft select, discrete step, symmetry, tool settings)
- ✅ Edit Pivot shows its blue ring (and an inner ring while point snapping); manipulator guide lines along the dragged axis (Preferences ▸ Manipulators ▸ Show manipulator guides)
- ✅ Selecting translate/rotate/scale channels in the Channel Box shows that manipulator with those axes active; an LMB click in a viewport ends the MMB channel drag and restores the tool
- ✅ Pivot edit (hold D, Insert toggles; snaps to grid and to vertices), Center Pivot, Freeze, Reset, Match transforms, Snap to ground
- ✅ Duplicate, Duplicate Special, Shift+D with transform, Group/Ungroup, Parent/Unparent, Copy/Paste, Delete
- ✅ Soft selection (B) with falloff display; symmetry (Object X, World X/Y/Z) mirrors component moves/rotates/scales

## 5. Polygon modeling
- ✅ Primitives with construction history: Sphere, Cube, Cylinder, Cone, Plane, Torus, Pyramid, Prism, Pipe, Helix, Disc, Platonic solids
- ✅ Component modes (F8–F12, UV mode synced with the UV Editor), grow/shrink, edge loop/ring/border (double-click an edge; loops follow open borders), convert selection, pre-selection highlight
- ✅ Extrude (faces/edges; the move manipulator after a face extrude drives Translate / Local Translate Z, with divisions spread along it), Bevel, Bridge, Insert Edge Loop tool, Multi-Cut, Target Weld, Merge / Merge to Center / Collapse, Connect, Chamfer vertex, Poke, Delete Edge/Vertex, Extract, Duplicate faces, Fill Hole, Triangulate, Quadrangulate, Add Divisions, Reverse normals
- ✅ Combine, Separate, Smooth, Mirror, Cleanup, Booleans (union/difference/intersection, with history), Create Polygon tool, Quad Draw (retopology)
- ✅ Soften/Harden edges, Delete History, Modeling Toolkit panel
- ✅ UV: planar/cylindrical/spherical/automatic projection; UV Editor (select/move/rotate/scale, flip, rotate 90, normalize, layout, unfold/relax, texture image or checker behind the UVs)
- ✅ Sculpting: Sculpt, Smooth, Relax, Grab, Pinch, Flatten, Inflate brushes with mirror

## 6. NURBS & curves
- ✅ NURBS primitives (tessellated, with isoparm display); circle & square curves
- ✅ CV / EP / Pencil curve tools; Revolve, Loft, Extrude along path, Planar — all with live history from the curves
- ✅ Three Point Circular Arc tool (Create ▸ Curve Tools, Curves/Surfaces shelf): click the start, a point on the arc and the end; then drag any of the three points or the centre, click the arc to flip it to the other side of the chord (radius readout), Enter / RMB to finish
- ✅ Rebuild, Reverse, Open/Close curve; polygon edges → curve; NURBS → polygons
- ⏭ Birail, Boundary, Attach/Detach, true NURBS evaluation with trims

## 7. Materials, shading & textures
- ✅ Lambert, Blinn, Phong, Standard Surface, Surface Shader
- ✅ Assign New / Existing material (menus & marking menus), per-face assignment
- ✅ Hypershade: material / texture tabs with rendered swatches, create bar, node graph work area (drag a texture's output onto a material slot, a material's output onto an object; right-click a slot to disconnect), property editor
- ✅ File (PNG / JPEG / HDR), Checker, Ramp, Noise, Grid textures; bump, roughness, metalness, emission maps
- ⏭ Layered shader, Paint Effects, 3D paint

## 8. Lighting
- ✅ Ambient, Directional, Point, Spot, Area, Sky Dome
- ✅ Intensity, colour, decay, cone/penumbra/dropoff, shadows & softness; viewport default / all lights / shadows
- ⏭ Light linking, fog, glow

## 9. Animation
- ✅ Set Key (S), Shift+W/E/R, Key Selected channels, Auto Key, delete/copy/paste/bake keys
- ✅ Scrubbing, ranges, playback (real-time / every frame / speeds), step frames/keys, frame rates
- ✅ Graph Editor: curves, key select/move, tangent types, tangent handles, infinity modes, framing, stats fields
- ✅ Dope Sheet; Playblast (PNG sequence)
- ✅ Set Driven Key, Attach to Motion Path, ghosting, motion trails
- ⏭ Animation layers, Time Editor/Trax, character sets, sound

## 10. Rigging & deformation
- ✅ Joint tool, insert/mirror/orient joints; IK handles (rotate-plane & single-chain solvers, pole vector, twist, IK blend)
- ✅ Smooth bind (linear blend skinning), unbind, paint skin weights
- ✅ Deformers: Bend, Twist, Flare, Sine, Squash, Wave, Lattice (with lattice point tool), Cluster, Soft Modification, Blend Shape
- ✅ Constraints: Parent, Point, Orient, Scale, Aim, Pole Vector (with maintain offset & weights)
- ✅ HumanIK (Skeleton ▸ HumanIK, Character Controls window) [#31](https://github.com/v3ai/inca/issues/31): standard humanoid skeleton in a T-pose (height, spine joints, fingers); character definitions mapped automatically from HumanIK, Mixamo, Unreal, Blender, 3ds Max Biped and BVH joint names (and the hierarchy), editable slot by slot; control rig with FK hips / spine / neck / head / shoulders and IK hand and foot effectors with pole vectors; live retargeting from another character (world rotations relative to each T-pose, hips motion scaled by height) and Bake to Skeleton; MEL `hikCreateSkeleton`, `hikCreateCharacter`, `hikCreateControlRig`, `hikSetSource`, `hikSetSlot`, `hikBake`
- ⏭ Flexors, HumanIK full-body solver (reach / pinning, finger effectors)

## 11. Rendering
- ✅ Render View: render current frame from any camera, IPR, keep/compare images, exposure/gamma/alpha display, save image
- ✅ Inca Path Tracer (physically based, progressive) and Inca Hardware renderer
- ✅ Render Settings (image size presets, file naming, frame range, samples, bounces, tone mapping, background)
- ✅ Render Sequence (batch frames to the project's images folder)
- ⏭ Render layers, AOVs

## 12. Scripting & editors
- ✅ Script Editor (MEL & Rhai tabs, history, save to shelf, load/save scripts)
- ✅ MEL-style language: variables, procs, loops, and commands (polyCube, move, rotate, scale, select, setAttr, getAttr, setKeyframe, currentTime, ls, xform, parent, group, duplicate, file, …); Rhai scripting as a second language
- ✅ Every UI action echoes its command; MEL command reference window
- ✅ Hypergraph (hierarchy, MMB-drag a node onto another to parent; input/output connections of every selected object with Maya's row layout, shared materials drawn once, connection lines coloured by type: green for vector attributes, blue for single values, one line per connection between the same two nodes, hover a line for its attributes)
- ✅ Connection Editor (Windows > General Editors): outputs / inputs side by side, click an output then an input to connect, click a connected input to break it, Reload Left / Right, Swap
- ✅ Direct attribute connections: connectAttr / disconnectAttr / listConnections / isConnected / connectionInfo, live evaluation, saved with the scene, yellow in the Channel Box (Break Connections removes them), arrows in the Hypergraph, driven objects drawn purple when the driver is selected (also constraints); texture outColor → material attribute = texture map
- ⏭ Plug-in API, custom UI from scripts

## 13. Dynamics & FX
- ✅ Emitters (omni/directional/volume/surface), emit from object, Create Particles (grids), particles with colour and opacity over lifetime
- ✅ Fields: gravity, turbulence, radial, drag, air, vortex; per-field connections (Affect Selected Object(s))
- ✅ Active / passive rigid bodies with gravity, bounciness, friction
- ✅ Fire and smoke presets; cached simulation that resets at the start frame
- ⏭ nCloth, fluids, hair

## Packaging
- ✅ Windows (portable `.exe` and `.zip`), Linux (`.AppImage` and `.tar.gz`), macOS (`.dmg` for Apple Silicon and Intel, ad-hoc signed, not notarized); every release builds all of them in GitHub Actions
