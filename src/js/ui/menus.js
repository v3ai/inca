// Inca — main menu bar with Maya-style menu sets
import { App } from '../core/app.js';
import * as S from '../core/scene.js';
import { Sel } from '../core/selection.js';
import { GEN_INFO } from '../core/primitives.js';
import { MAT_TYPES } from '../core/materials.js';
import { h, menuBar } from './dom.js';
import { MENU_SETS } from './statusline.js';

// menu item from a command id; disabled automatically if the command isn't available
const m = (cmd, label, extra = {}) => ({ cmd, label: label ?? App.cmds.registry.get(cmd)?.label ?? cmd, enabled: () => App.cmds.registry.has(cmd) && (!extra.when || extra.when()), ...extra });
const sub = (label, items) => ({ label, sub: items });
const sect = (s) => ({ section: s });
const polyPrims = () => Object.keys(GEN_INFO).filter(g => !GEN_INFO[g].nurbs).map(g => m(g, null, { showIcon: true }));
const nurbsPrims = () => [...Object.keys(GEN_INFO).filter(g => GEN_INFO[g].nurbs).map(g => m(g, null, { showIcon: true })), m('nurbsCircle', 'Circle', { showIcon: true }), m('nurbsSquare', 'Square', { showIcon: true })];
const lights = () => ['ambientLight', 'directionalLight', 'pointLight', 'spotLight', 'areaLight', 'skyDomeLight'].map(l => m(l, null, { showIcon: true }));

const COMMON = {
  File: () => [
    m('newScene'), m('openScene'), m('saveScene'), m('saveSceneAs'), m('incrementAndSave'), m('revertScene'), '-',
    m('importFile'), m('exportAll'), m('exportSelection'), '-',
    m('projectWindow'), m('setProject'), '-',
    sub('Recent Files', () => (App.prefs.recent || []).length ? App.prefs.recent.map(p => ({ label: p, fn: () => App.io.openPath(p) })) : [{ label: '(none)', enabled: () => false }]),
    sub('Recent Projects', () => (App.prefs.recentProjects || []).length ? App.prefs.recentProjects.map(p => ({ label: p, fn: () => App.io.setProject(p) })) : [{ label: '(none)', enabled: () => false }]),
    '-', m('quit', 'Exit', { hk: 'Ctrl+Q' }),
  ],
  Edit: () => [
    m('undo'), m('redo'), m('repeatLast', 'Repeat Last'), sub('Recent Commands', () => { const l = App.undo.labels().slice(-15).reverse(); return l.length ? l.map(s => ({ label: s, enabled: () => false })) : [{ label: '(none)', enabled: () => false }]; }), '-',
    m('cut'), m('copy'), m('paste'), '-',
    sub('Keys', [m('copyKeys'), m('pasteKeys'), m('deleteKeysCurrent'), m('deleteChannels', 'Delete Channels (all keys)'), m('bakeKeys')]), '-',
    m('deleteSel', 'Delete'),
    sub('Delete by Type', [m('deleteHistory'), m('deleteChannels'), m('deleteAllLights'), m('deleteAllCameras'), m('deleteAllCurves')]),
    sub('Delete All by Type', [m('deleteAllHistory'), m('deleteAllLights'), m('deleteAllCameras'), m('deleteAllCurves')]), '-',
    m('duplicate'), m('duplicateSpecial', null, { opt: true }), m('duplicateWithTransform'), '-',
    m('group', null, { opt: true }), m('ungroup'), '-',
    m('parent'), m('unparent'), '-',
    m('searchReplace'), m('prefixHierarchy'), m('renameSel'),
  ],
  Create: () => [
    sub('Polygon Primitives', polyPrims), sub('NURBS Primitives', nurbsPrims), m('createPolygonTool', 'Polygon Tool...'), '-',
    sub('Lights', lights), sub('Cameras', [m('createCamera', 'Camera', { showIcon: true }), m('createCameraAim', 'Camera and Aim')]), '-',
    sub('Curve Tools', [m('cvCurveTool'), m('epCurveTool'), m('pencilCurveTool', 'Pencil Curve Tool'), m('arcTool', 'Three Point Circular Arc')]), '-',
    m('createType', 'Type'), m('createEmptyGroup'), m('createLocator'), '-',
    sub('Measure Tools', [m('distanceTool', 'Distance Tool')]),
  ],
  Select: () => [
    m('selectAll', 'All'), sub('All by Type', [m('selectAllGeometry', 'Geometry'), m('selectAllLights', 'Lights'), m('selectAllCameras', 'Cameras'), m('selectAllCurves', 'NURBS Curves')]), m('deselectAll'), m('selectHierarchy'), m('invertSelection'), m('selectSimilar', 'Similar'), '-',
    m('growSelection'), m('shrinkSelection'), '-',
    m('objectModeOnly', 'Object/Component', { hk: 'F8' }), m('vertexMode', 'Vertex'), m('edgeMode', 'Edge'), m('faceMode', 'Face'), m('uvMode', 'UV'), '-',
    m('selectEdgeLoop', 'Select Edge Loop'), m('selectEdgeRing', 'Select Edge Ring'), m('selectBorder', 'Select Border Edges'), '-',
    sub('Convert Selection', [m('toVertices'), m('toEdges'), m('toFaces')]),
    m('selectByName', 'Select by Name...'),
  ],
  Modify: () => [
    sub('Transformation Tools', [m('moveTool'), m('rotateTool'), m('scaleTool'), m('softSelect'), m('pivotEdit', 'Edit Pivot')]),
    m('resetTransformations'), m('freezeTransformations'), sub('Match Transformations', [m('matchTranslation'), m('matchRotation'), m('matchAll')]), '-',
    sub('Pivot', [m('centerPivot'), m('pivotEdit', 'Edit Pivot')]), m('centerPivot'), m('snapToGround', 'Snap to Ground'), m('snapAlign', 'Snap Align Objects'), '-',
    m('prefixHierarchy'), m('searchReplace'), '-',
    sub('Convert', [m('convertNurbsToPoly', 'NURBS to Polygons')]), m('renameSel'),
    '-', m('addAttribute', 'Add Attribute...'),
  ],
  Display: () => [
    m('toggleGrid', 'Grid'), sub('Heads Up Display', [
      { label: 'Poly Count', check: () => !!App.prefs.hud.polyCount, cmd: 'hudPolyCount' }, { label: 'Frame Rate', check: () => !!App.prefs.hud.fps, cmd: 'hudFrameRate' },
      { label: 'Current Frame', check: () => !!App.prefs.hud.frame, cmd: 'hudCurrentFrame' }, { label: 'Camera Names', check: () => App.prefs.hud.camNames !== false, cmd: 'hudCameraNames' }, { label: 'View Axis', check: () => App.prefs.hud.viewAxis !== false, cmd: 'hudViewAxis' }]),
    sub('UI Elements', () => [...[['statusline', 'Status Line'], ['shelf', 'Shelf'], ['timeslider', 'Time Slider'], ['rangeslider', 'Range Slider'], ['cmdline', 'Command Line'], ['helpline', 'Help Line'], ['toolbox', 'Tool Box'], ['sidetabs', 'Side Tabs']].map(([k, l]) => ({ label: l, check: () => App.ui.elementVisible(k), fn: () => App.ui.setElementVisible(k, !App.ui.elementVisible(k)) })), '-', { label: 'Hide UI Elements', hk: 'Ctrl+Space', fn: () => App.ui.toggleAllElements() }, { label: 'Restore UI Elements', fn: () => { for (const k of App.ui.elements) App.ui.setElementVisible(k, true); } }]),
    '-', sub('Hide', [m('hideSelection'), m('hideUnselected', 'Hide Unselected Objects'), '-', m('hideAllLights', 'Lights'), m('hideAllCameras', 'Cameras'), m('hideAllGeometry', 'All Geometry')]),
    sub('Show', [m('showSelection'), m('showLastHidden'), m('showAll', 'All'), '-', m('showAllLights', 'Lights'), m('showAllCameras', 'Cameras'), m('showAllGeometry', 'All Geometry')]),
    sub('Object Display', [m('templateSel'), m('untemplateSel'), m('untemplateAll')]),
    m('toggleWireOnShaded', 'Wireframe on Shaded'), m('toggleXray', 'X-Ray'), m('toggleBackface', 'Backface Culling'), m('cycleBackground', 'Toggle Background'), '-',
    sub('Polygons', [m('smoothOff', 'Smooth Mesh: Off (1)'), m('smoothCage', 'Smooth Mesh: Cage + Smooth (2)'), m('smoothOn', 'Smooth Mesh: Preview (3)'), '-', m('toggleNormals', 'Face Normals'), m('toggleBorderEdges', 'Border Edges'), m('softenEdge'), m('hardenEdge')]),
    m('frameAll'), m('frameSelected'), m('isolateSelect'),
  ],
  Windows: () => [
    sub('Workspaces', () => [...Object.keys(App.ui.workspaces).map(w => ({ label: w, check: () => App.prefs.workspace === w, fn: () => App.ui.applyWorkspace(w) })), '-', { label: 'Reset Current Workspace', fn: () => App.ui.applyWorkspace(App.prefs.workspace) }]),
    sub('General Editors', [m('scriptEditor'), m('shelfEditor'), m('hypergraph'), m('hypergraphConnections', 'Hypergraph: Connections'), m('componentEditor', 'Component Editor'), m('projectWindow')]),
    sub('Modeling Editors', [m('modelingToolkit'), m('uvEditor')]),
    sub('Animation Editors', [m('graphEditor'), m('dopeSheet'), m('setDrivenKeyWindow', 'Set Driven Key...')]),
    sub('Rendering Editors', [m('hypershade'), m('renderView'), m('renderSettings'), m('playblast')]),
    sub('Settings/Preferences', [m('preferences'), m('hotkeyEditor'), m('shelfEditor')]), '-',
    m('outlinerWindow', 'Outliner'), m('attributeEditor'), m('channelBox'), m('toolSettings'), m('modelingToolkit'), '-',
    sub('UI Elements', [m('toggleUIElements', 'Hide/Show UI Elements')]),
  ],
};
const SETS = {
  modeling: {
    Mesh: () => [sect('Combine'), m('booleanUnion', 'Booleans: Union'), m('booleanDifference', 'Booleans: Difference'), m('booleanIntersection', 'Booleans: Intersection'), m('combine'), m('separate'), sect('Remesh'), m('conformMesh', 'Conform'), m('fillHole'), m('reduce', 'Reduce'), m('smooth', null, { opt: true }), m('triangulate'), m('quadrangulate'), sect('Mirror'), m('mirror', null, { opt: true }), sect('Transfer'), m('cleanup'), sect('Optimize'), m('cleanup', 'Cleanup...')],
    'Edit Mesh': () => [sect('Components'), m('addDivisions', null, { opt: true }), m('bevel', null, { opt: true }), m('bridge', null, { opt: true }), m('collapse'), m('connect'), m('chamferVertex'), m('extrude', null, { opt: true }), m('merge', null, { opt: true }), m('mergeToCenter'), m('poke'), sect('Vertex'), m('deleteEdgeVertex', 'Delete Edge/Vertex'), sect('Face'), m('extract'), m('duplicateFaces', 'Duplicate'), sect('Curve'), m('curveFromEdges', 'Polygon Edges to Curve')],
    'Mesh Tools': () => [sect('Tools'), m('createPolygonTool'), m('insertEdgeLoopTool'), m('multiCutTool'), m('quadDrawTool', 'Quad Draw'), m('targetWeldTool'), m('sculptTool', 'Sculpt Tool'), m('makeLive', 'Make Live')],
    'Mesh Display': () => [m('softenEdge'), m('hardenEdge'), m('softHardAngle'), '-', m('reverseNormals', 'Reverse'), m('conformNormals', 'Conform'), '-', m('toggleNormals', 'Toggle Face Normals Display'), m('toggleBorderEdges', 'Toggle Border Edges'), '-', m('smoothOff', 'Smooth Mesh Off'), m('smoothOn', 'Smooth Mesh Preview')],
    Curves: () => [m('cvCurveTool'), m('epCurveTool'), m('pencilCurveTool'), '-', m('reverseCurve'), m('rebuildCurve'), m('openCloseCurve'), m('attachCurves', 'Attach'), m('detachCurve', 'Detach'), '-', m('curveFromEdges', 'Polygon Edges to Curve')],
    Surfaces: () => [m('loft', null, { opt: true }), m('planarSurface'), m('revolve', null, { opt: true }), m('birail', 'Birail'), m('extrudeCurve', 'Extrude', { opt: true }), m('boundarySurface', 'Boundary'), '-', m('convertNurbsToPoly')],
    Deform: () => DEFORM(),
    UV: () => [m('planarMap', null, { opt: true }), m('cylindricalMap'), m('sphericalMap'), m('automaticMap'), '-', m('uvEditor')],
    Generate: () => [m('createType', 'Type'), m('createParticles', 'Particles')],
  },
  rigging: {
    Skeleton: () => [m('jointTool', 'Create Joints'), m('ikHandleTool', 'Create IK Handle'), m('insertJoint', 'Insert Joints'), m('mirrorJoint', 'Mirror Joints'), m('orientJoint', 'Orient Joint')],
    Skin: () => [m('smoothBind', 'Bind Skin'), m('unbindSkin', 'Unbind Skin'), m('paintWeights', 'Paint Skin Weights')],
    Deform: () => DEFORM(),
    Constrain: () => CONSTRAIN(),
    Control: () => [m('createLocator'), m('group'), m('setDrivenKeyWindow', 'Set Driven Key...')],
  },
  animation: {
    Key: () => [m('setKey'), m('keyTranslate'), m('keyRotate'), m('keyScale'), '-', m('setDrivenKeyWindow', 'Set Driven Key...'), '-', m('copyKeys'), m('pasteKeys'), m('deleteKeysCurrent', 'Delete Keys'), m('bakeKeys', 'Bake Simulation'), '-', m('toggleAutoKey', 'Auto Keyframe', { check: () => App.time.autoKey })],
    Playback: () => [m('playToggle', 'Play/Stop'), m('playBackwards'), m('stopPlayback', 'Stop'), '-', m('nextFrame'), m('prevFrame'), m('nextKey'), m('prevKey'), m('goToStart'), m('goToEnd'), '-', m('playblast')],
    Visualize: () => [m('createGhost', 'Ghost Selected'), m('unghostAll', 'Unghost All'), m('motionTrail', 'Create Motion Trail')],
    Deform: () => DEFORM(),
    Constrain: () => [...CONSTRAIN(), '-', m('attachToMotionPath', 'Motion Paths: Attach to Motion Path')],
  },
  fx: {
    nParticles: () => [m('createEmitter', 'Create Emitter'), m('createParticles', 'Create Particles'), m('emitFromObject', 'Emit from Object')],
    Fields: () => [m('gravityField', 'Gravity'), m('turbulenceField', 'Turbulence'), m('radialField', 'Radial'), m('dragField', 'Drag'), m('airField', 'Air'), m('vortexField', 'Vortex')],
    Rigid: () => [m('createActiveRigidBody', 'Active Rigid Body'), m('createPassiveRigidBody', 'Passive Rigid Body')],
    Effects: () => [m('createFire', 'Create Fire'), m('createSmoke', 'Create Smoke')],
  },
  rendering: {
    Lighting: () => [...lights(), '-', m('hypershade'), m('assignNewMaterial', 'Assign New Material...'), sub('Assign Existing Material', () => [...App.mats.values()].map(mm => ({ label: mm.name, fn: () => App.cmds.run('assignExisting', { id: mm.id }) }))), m('materialAttributes')],
    Texturing: () => [m('planarMap'), m('automaticMap'), m('uvEditor')],
    Render: () => [m('renderCurrent'), m('ipr'), m('renderSequence', 'Render Sequence...'), '-', m('renderView'), m('renderSettings'), m('playblast')],
  },
};
function DEFORM() { return [sect('Create'), m('blendShape', 'Blend Shape'), m('createCluster', 'Cluster'), m('createLattice', 'Lattice'), m('softModDeformer', 'Soft Modification'), sub('Nonlinear', [m('bendDeformer', 'Bend'), m('flareDeformer', 'Flare'), m('sineDeformer', 'Sine'), m('squashDeformer', 'Squash'), m('twistDeformer', 'Twist'), m('waveDeformer', 'Wave')]), sect('Edit'), m('deleteDeformers', 'Delete Deformers on Selected')]; }
function CONSTRAIN() { return [m('parentConstraint', 'Parent'), m('pointConstraint', 'Point'), m('orientConstraint', 'Orient'), m('scaleConstraint', 'Scale'), m('aimConstraint', 'Aim'), m('poleVectorConstraint', 'Pole Vector'), '-', m('removeConstraints', 'Remove Constraints')]; }
const HELP = () => [m('gettingStarted', 'Getting Started with Inca'), m('hotkeyList', 'Keyboard Shortcuts'), m('mouseHelp', 'Mouse & Navigation'), '-', m('melReference', 'MEL Command Reference'), '-', m('about', 'About Inca')];

function build() {
  const host = document.getElementById('menus');
  const set = App.prefs.menuSet || 'modeling';
  const list = [];
  for (const [k, fn] of Object.entries(COMMON)) list.push({ label: k, items: fn });
  for (const [k, fn] of Object.entries(SETS[set] || SETS.modeling)) list.push({ label: k, items: fn, cls: 'menuset' });
  list.push({ label: 'Help', items: HELP });
  menuBar(host, list);
  const ws = document.getElementById('workspace-select');
  if (ws && !ws.options.length) { for (const w of Object.keys(App.ui.workspaces || {})) ws.append(h('option', { text: w, value: w })); ws.addEventListener('change', () => App.ui.applyWorkspace(ws.value)); }
  if (ws) ws.value = App.prefs.workspace;
}
App.ui.setMenuSet = (k) => { if (!SETS[k]) return; App.prefs.menuSet = k; App.savePrefs(); build(); App.ui.syncStatusline?.(); App.help('Menu set: ' + MENU_SETS.find(x => x[0] === k)?.[1]); };
App.ui.menuSets = SETS; App.ui.commonMenus = COMMON; App.ui.helpMenu = HELP;
App.on('beforeUI', build);
