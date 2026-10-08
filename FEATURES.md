# Inca — Master Feature List

Inca is a free, open desktop 3D content-creation app that recreates the workflow and layout of
Autodesk Maya (2020-era UI). The list was derived from the supplied course videos (Maya 2018/2020 UI,
project setup, shelves/prefs/hotkeys, box modeling a sofa, modeling a rocket, lighting, shading,
keyframes and timing) and the *Maya Tutorials Collection* book (Maya 2–4 era: Hypergraph, Hypershade,
NURBS, deformers, skeletons, MEL, dynamics, Artisan sculpting, rendering). The book's screenshots are
old, so the look follows the videos; the book was used for capabilities.

Legend: ✅ built · 🟡 partial / simplified · ⏭ planned

---

## 1. Application shell & interface
- ✅ Maya-style dark theme, title `scene* - Inca - project`, splash screen, app icon
- ✅ Main menu bar: File, Edit, Create, Select, Modify, Display, Windows + menu-set menus + Help
- ✅ Menu sets (Modeling F2, Rigging F3, Animation F4, FX F5, Rendering F6) swapping the right-hand menus
- ✅ Status line: menu set, new/open/save, undo/redo, hierarchy/object/component modes, object & component masks, lock selection, snap toggles (grid/curve/point/live), construction history, render buttons, numeric input line (absolute/relative/rename/select by name), symmetry, panel toggles
- ✅ Shelves: Curves/Surfaces, Poly Modeling, Sculpting, Rigging, Animation, Rendering, FX, Custom; Ctrl+Shift+click a menu item to add it; right-click/MMB-drag to edit; Shelf Editor; save/load shelves
- ✅ Tool box (Select, Lasso, Paint, Move, Rotate, Scale, Last tool) + Quick Layout buttons
- ✅ Workspaces (Maya Classic, Modeling Standard/Expert, Sculpting, Animation, Rigging, Rendering, UV Editing)
- ✅ Channel Box / Layer Editor: channels, SHAPES, INPUTS (construction history), virtual sliders (MMB in viewport, Ctrl/MMB-drag in fields), expressions in fields (`+=2`), right-click Key/Lock/Hide/Break, keyed/locked colouring
- ✅ Display layers: create (empty/from selected), V / T / R, colour, select, rename, delete
- ✅ Attribute Editor (Ctrl+A) with tabs for transform, shape, history nodes, material and textures; texture map buttons; Copy Tab; lock/focus
- ✅ Outliner (dock), hierarchy, drag to parent/reorder/unparent, inline rename, search, shapes/sets/materials display
- ✅ Time Slider, Range Slider, playback controls, auto key, fps menu, looping, key ticks, shift-drag range edits
- ✅ Command line (MEL / JS), result line, help line, Script Editor
- ✅ Floating editors; side tabs; dock resizing
- ✅ Hotbox (hold Space), RMB marking menus (object, component, Shift+RMB tool menus, Ctrl+RMB conversion)
- ✅ In-View Editor for history nodes (drag labels to scrub values); option boxes remember settings
- ✅ Preferences (interface, display, manipulators, animation, time slider, units, undo, files, autosave); Hotkey Editor with keyboard map
- ⏭ Arbitrary panel docking / saving custom workspaces, tear-off menus

## 2. Projects & files
- ✅ Project Window / Set Project (standard folder tree + workspace file), recent projects
- ✅ New / Open / Save / Save As / Increment & Save / Revert; native `.inca` scene format (JSON); recent files; autosave
- ✅ Import OBJ (keeps n-gons, MTL colours/textures), FBX, glTF/GLB, STL, Inca scenes; Export All / Selection to OBJ+MTL, glTF/GLB, STL, Inca
- ⏭ References / proxies, archive scene

## 3. Viewports & navigation
- ✅ Persp/Top/Front/Side, single/four/two/three layouts, tap Space to toggle
- ✅ Alt+LMB tumble, Alt+MMB track, Alt+RMB/wheel dolly; F/A frame, Shift+F/A all views; bookmarks; default view
- ✅ Shading 4/5/6/7, wireframe on shaded, X-ray, backface culling, shadows, flat/none lighting, exposure, background toggle (Alt+B)
- ✅ Smooth mesh preview 1/2/3; resolution & film gates; HUD (poly count, fps, frame, camera names, view axis)
- ✅ Panel menus & toolbar, Isolate Select, Show filters, look through selected, new cameras
- ✅ Face normals / border edge display

## 4. Selection & transforms
- ✅ Click/Shift/Ctrl/Ctrl+Shift, marquee, lasso, paint selection, select all/inverse/by type/hierarchy/similar/by name
- ✅ Move/Rotate/Scale manipulators (world/object space), MMB drag, step snap, grid snap (X), point snap (V), discrete rotate (J), manipulator size +/-
- ✅ Pivot edit (D/Insert), Center Pivot, Freeze, Reset, Match transforms, Snap to ground
- ✅ Duplicate, Duplicate Special, Shift+D with transform, Group/Ungroup, Parent/Unparent, Copy/Paste, Delete
- ✅ Soft selection (B) with falloff display; 🟡 symmetry setting stored (not yet applied to component moves)

## 5. Polygon modeling
- ✅ Primitives with construction history: Sphere, Cube, Cylinder, Cone, Plane, Torus, Pyramid, Prism, Pipe, Helix, Disc, Platonic solids
- ✅ Component modes (F8–F12), grow/shrink, edge loop/ring/border, convert selection, pre-selection highlight
- ✅ Extrude (faces/edges), Bevel, Bridge, Insert Edge Loop tool, Multi-Cut, Target Weld, Merge / Merge to Center / Collapse, Connect, Chamfer vertex, Poke, Delete Edge/Vertex, Extract, Duplicate faces, Fill Hole, Triangulate, Quadrangulate, Add Divisions, Reverse normals
- ✅ Combine, Separate, Smooth, Mirror, Cleanup, Booleans (union/difference/intersection, with history), Create Polygon tool, Quad Draw (retopology)
- ✅ Soften/Harden edges, Delete History, Modeling Toolkit panel
- ✅ UV: planar/cylindrical/spherical/automatic projection; UV Editor (select/move/rotate/scale, flip, rotate 90, normalize, layout, unfold/relax)
- ✅ Sculpting: Sculpt, Smooth, Relax, Grab, Pinch, Flatten, Inflate brushes with mirror

## 6. NURBS & curves
- ✅ NURBS primitives (tessellated, with isoparm display); circle & square curves
- ✅ CV / EP / Pencil curve tools; Revolve, Loft, Extrude along path, Planar — all with live history from the curves
- ✅ Rebuild, Reverse, Open/Close curve; polygon edges → curve; NURBS → polygons
- ⏭ Birail, Boundary, Attach/Detach, true NURBS evaluation with trims

## 7. Materials, shading & textures
- ✅ Lambert, Blinn, Phong, Standard Surface, Surface Shader
- ✅ Assign New / Existing material (menus & marking menus), per-face assignment
- ✅ Hypershade: browser tabs with swatches, Create panel, node graph with drag-to-connect, property editor, drag a material onto objects
- ✅ File (incl. HDR/EXR), Checker, Ramp, Noise, Grid textures; bump, roughness, metalness, emission maps
- ⏭ Layered shader, Paint Effects, 3D paint

## 8. Lighting
- ✅ Ambient, Directional, Point, Spot, Area, Sky Dome (HDRI image or procedural sky)
- ✅ Intensity, colour, decay, cone/penumbra/dropoff, shadows & softness; viewport default / all lights / shadows
- ⏭ Light linking, fog, glow

## 9. Animation
- ✅ Set Key (S), Shift+W/E/R, Key Selected channels, Auto Key, delete/copy/paste/bake keys
- ✅ Scrubbing, ranges, playback (real-time / every frame / speeds), step frames/keys, frame rates
- ✅ Graph Editor: curves, key select/move, tangent types, tangent handles, infinity modes, framing, stats fields
- ✅ Dope Sheet; Playblast (WebM video or PNG sequence)
- ✅ Set Driven Key, Attach to Motion Path, ghosting, motion trails
- ⏭ Animation layers, Time Editor/Trax, character sets, sound

## 10. Rigging & deformation
- ✅ Joint tool, insert/mirror/orient joints; IK handles (rotate-plane & single-chain solvers, pole vector, twist, IK blend)
- ✅ Smooth bind (linear blend skinning), unbind, paint skin weights
- ✅ Deformers: Bend, Twist, Flare, Sine, Squash, Wave, Lattice (with lattice point tool), Cluster, Soft Modification, Blend Shape
- ✅ Constraints: Parent, Point, Orient, Scale, Aim, Pole Vector (with maintain offset & weights)
- ⏭ HumanIK-style auto rig, flexors

## 11. Rendering
- ✅ Render View: render current frame from any camera, IPR, keep/compare images, exposure/gamma/alpha display, save image
- ✅ Inca Path Tracer (physically based, progressive) and Inca Hardware renderer
- ✅ Render Settings (image size presets, file naming, frame range, samples, bounces, tone mapping, background)
- ✅ Render Sequence (batch frames to the project's images folder)
- ⏭ Render layers, AOVs

## 12. Scripting & editors
- ✅ Script Editor (MEL & JavaScript tabs, history, save to shelf, load/save scripts)
- ✅ MEL-style language: variables, procs, loops, and commands (polyCube, move, rotate, scale, select, setAttr, getAttr, setKeyframe, currentTime, ls, xform, parent, group, duplicate, file, …); `cmds.*` JavaScript API
- ✅ Every UI action echoes its command; MEL command reference window
- ✅ Hypergraph (hierarchy and input/output connections)
- ⏭ Plug-in API, custom UI from scripts

## 13. Dynamics & FX
- ✅ Emitters (omni/directional/volume/surface/vertex), emit from object, particles (points / soft sprites with colour, size and opacity over lifetime)
- ✅ Fields: gravity, turbulence, radial, drag, air, vortex
- ✅ Active / passive rigid bodies with gravity, bounciness, friction
- ✅ Fire and smoke presets; cached simulation that resets at the start frame
- ⏭ nCloth, fluids, hair

## Packaging
- ✅ Windows (portable `.exe` and `.zip`), Linux (`.AppImage`); macOS build script (`npm run dist:mac`, build on a Mac)
