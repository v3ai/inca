//! Application state, actions (menu items / shelf buttons / hotkeys) and the frame loop.

use crate::viewport::{gpu::Gpu, Viewport};
use crate::{panels, theme, timeline, toolbar, vp_ui, windows};
use inca_core::commands::Host;
use inca_core::mel::{Interp, Val};
use inca_core::scene::{CompMode, MsgKind, Scene, SelMode};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    Lasso,
    Paint,
    Move,
    Rotate,
    Scale,
    CvCurve,
    EpCurve,
    Arc,
    Polygon,
    Joint,
    EdgeLoop,
    MultiCut,
    TargetWeld,
    Sculpt,
    QuadDraw,
}

impl Tool {
    pub fn is_manip(self) -> bool {
        matches!(self, Tool::Move | Tool::Rotate | Tool::Scale)
    }
    /// Icon of an interactive tool (tool box "last tool" button).
    pub fn icon(self) -> &'static str {
        match self {
            Tool::CvCurve => "cvCurve",
            Tool::EpCurve => "epCurve",
            Tool::Arc => "arcCurve",
            Tool::Polygon => "planar",
            Tool::Joint => "joint",
            Tool::Lasso => "lasso",
            Tool::Paint => "paintSelect",
            Tool::Move => "move",
            Tool::Rotate => "rotate",
            Tool::Scale => "scale",
            Tool::Select => "select",
            Tool::EdgeLoop => "edgeLoop",
            Tool::MultiCut => "multiCut",
            Tool::TargetWeld => "targetWeld",
            Tool::Sculpt => "softSelect",
            Tool::QuadDraw => "planar",
        }
    }
    pub fn is_creation(self) -> bool {
        matches!(self, Tool::CvCurve | Tool::EpCurve | Tool::Arc | Tool::Polygon | Tool::Joint)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layout {
    Single,
    Four,
    TwoSide,
    TwoStack,
    /// persp on the left, top and front stacked on the right
    Three,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuSet {
    Modeling,
    Rigging,
    Animation,
    Fx,
    Rendering,
}

impl MenuSet {
    pub const ALL: [MenuSet; 5] = [MenuSet::Modeling, MenuSet::Rigging, MenuSet::Animation, MenuSet::Fx, MenuSet::Rendering];
    pub fn label(self) -> &'static str {
        match self {
            MenuSet::Modeling => "Modeling",
            MenuSet::Rigging => "Rigging",
            MenuSet::Animation => "Animation",
            MenuSet::Fx => "FX",
            MenuSet::Rendering => "Rendering",
        }
    }
}

#[derive(Default)]
pub struct Windows {
    pub script_editor: bool,
    pub hypershade: bool,
    pub graph_editor: bool,
    pub dope_sheet: bool,
    pub render_view: bool,
    pub render_settings: bool,
    pub prefs: bool,
    pub hotkeys: bool,
    pub about: bool,
    pub command_list: bool,
    pub rename: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RightTab {
    ChannelBox,
    AttributeEditor,
    ToolSettings,
}

/// UI-side commands raised while MEL runs (tools, windows, viewports).
#[derive(Default)]
pub struct UiHost {
    pub reqs: Vec<(String, Vec<Val>)>,
}

impl Host for UiHost {
    fn ui(&mut self, _s: &mut Scene, cmd: &str, args: &[Val]) -> Option<Result<Val, String>> {
        const UI: &[&str] = &["setToolTo", "viewFit", "FrameSelected", "FrameAll", "refresh", "play", "ScriptEditor", "HypershadeWindow", "GraphEditor", "RenderViewWindow", "OutlinerWindow", "render", "modelEditor", "workspace", "inViewMessage", "headsUpMessage", "confirmDialog", "promptDialog", "setProject"];
        if UI.contains(&cmd) {
            self.reqs.push((cmd.into(), args.to_vec()));
            return Some(Ok(Val::None));
        }
        None
    }
}

pub struct IncaApp {
    pub s: Scene,
    pub mel: Interp,
    pub tool: Tool,
    pub last_tool: Tool,
    pub layout: Layout,
    pub active_vp: usize,
    pub vps: Vec<Viewport>,
    pub menu_set: MenuSet,
    pub win: Windows,
    pub right_tab: RightTab,
    pub show_right: bool,
    pub show_outliner: bool,
    /// UI elements switched off (workspace::ELEMENTS keys)
    pub hidden: std::collections::BTreeSet<String>,
    /// what Ctrl+Space hid, to bring back
    pub ui_stash: Option<(std::collections::BTreeSet<String>, bool)>,
    pub workspace: String,
    pub cmdline: String,
    pub cmd_history: Vec<String>,
    pub help: String,
    pub last_result: String,
    pub script_text: String,
    pub script_log: Vec<(MsgKind, String)>,
    pub script_lang_rhai: bool,
    pub shelf_tab: usize,
    pub world_space: bool,
    pub snap_grid: bool,
    pub history_on: bool,
    pub manip_size: f32,
    /// Edit Pivot: the move manipulator moves pivots instead of objects
    pub pivot_mode: bool,
    pub gpu: Option<Gpu>,
    pub smooth_cache: HashMap<String, (u64, inca_core::mesh::PolyMesh)>,
    pub vp_state: vp_ui::VpState,
    pub panels: panels::PanelState,
    pub windows_state: windows::WinState,
    pub playing: bool,
    pub play_clock: f64,
    pub last_action: Option<String>,
    pub clipboard: Option<serde_json::Value>,
    pub test: Option<TestRun>,
    pub recent: Vec<String>,
    /// user shelf buttons (label, MEL), saved between sessions
    pub custom_shelf: Vec<(String, String)>,
    /// status line: select whole hierarchies instead of objects
    pub select_hierarchy: bool,
    pub last_comp_mode: CompMode,
    /// object types switched off in the selection masks ("mesh", "curve", ...)
    pub sel_mask_off: std::collections::HashSet<String>,
    pub lock_selection: bool,
    pub snap_curve: bool,
    pub snap_point: bool,
    pub live_surface: Option<String>,
    /// 0 off, 1 object X, 2 world X, 3 world Y, 4 world Z
    pub symmetry: usize,
    pub paint_radius: f32,
    pub play_dir: f64,
    /// Shift-drag time selection on the Time Slider
    pub range_sel: Option<(f64, f64)>,
    /// copied keys: (node, attr, keys with times relative to the copy start)
    pub key_clip: Vec<(String, String, Vec<inca_core::anim::Key>)>,
    pub prev_range: Option<(f64, f64)>,
    pub recent_projects: Vec<String>,
    pub project_win: Option<crate::project::ProjectWin>,
    /// autosave on, interval in minutes
    pub autosave: (bool, f64),
    pub last_autosave: f64,
    pub prompt: Option<crate::edit_cmds::Prompt>,
    pub dup_opts: crate::edit_cmds::DupOpts,
    pub dup_special_open: bool,
    pub add_attr: Option<crate::edit_cmds::AddAttr>,
    pub last_hidden: Vec<String>,
    pub hud: crate::edit_cmds::Hud,
    pub overlay_normals: HashSet<String>,
    pub overlay_border: HashSet<String>,
    /// measured redraw rate for the fps HUD
    pub fps_measured: f32,
    pub mesh_tool: crate::mesh_tools::MeshToolState,
    pub mesh_opts: crate::mesh_tools::MeshToolOpts,
    /// (sources, duplicates) of the last Shift+D, to repeat its offset
    pub last_dup: Option<(Vec<String>, Vec<String>)>,
    /// viewport background (Alt+B cycles viewport::BG_MODES)
    pub bg_mode: usize,
    /// Space: tap toggles the layout, hold shows the hotbox
    pub space_down: Option<f64>,
    pub hotbox: Option<egui::Pos2>,
}

/// INCA_TEST / INCA_SHOT: run a MEL script at startup, save a screenshot and quit.
pub struct TestRun {
    pub script: Option<String>,
    pub shot: Option<std::path::PathBuf>,
    pub frame: u32,
    pub requested: bool,
}

impl IncaApp {
    pub fn shown(&self, element: &str) -> bool {
        !self.hidden.contains(element)
    }
    pub fn set_shown(&mut self, element: &str, on: bool) {
        if on {
            self.hidden.remove(element);
        } else {
            self.hidden.insert(element.to_string());
        }
    }
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);
        let gpu = cc.wgpu_render_state.as_ref().map(Gpu::new);
        let test = match (std::env::var("INCA_TEST").ok(), std::env::var("INCA_SHOT").ok()) {
            (None, None) => None,
            (script, shot) => Some(TestRun { script: script.and_then(|p| std::fs::read_to_string(p).ok()), shot: shot.map(Into::into), frame: 0, requested: false }),
        };
        let mut app = IncaApp {
            s: Scene::new(),
            mel: Interp::new(),
            tool: Tool::Select,
            last_tool: Tool::Move,
            layout: Layout::Single,
            active_vp: 0,
            vps: vec![Viewport::new("persp"), Viewport::new("top"), Viewport::new("front"), Viewport::new("side")],
            menu_set: MenuSet::Modeling,
            win: Windows::default(),
            right_tab: cc.storage.and_then(|s| eframe::get_value(s, "rightTab")).unwrap_or(RightTab::ChannelBox),
            show_right: cc.storage.and_then(|s| eframe::get_value(s, "showRight")).unwrap_or(true),
            show_outliner: cc.storage.and_then(|s| eframe::get_value(s, "showOutliner")).unwrap_or(false),
            hidden: cc.storage.and_then(|s| eframe::get_value(s, "hiddenUi")).unwrap_or_default(),
            ui_stash: None,
            workspace: cc.storage.and_then(|s| eframe::get_value(s, "workspace")).unwrap_or_else(|| "Maya Classic".into()),
            cmdline: String::new(),
            cmd_history: vec![],
            help: String::new(),
            last_result: String::new(),
            script_text: String::from("// MEL\npolySphere -r 1;\nmove -r 0 1 0;\n"),
            script_log: vec![],
            script_lang_rhai: false,
            shelf_tab: 1,
            world_space: true,
            snap_grid: false,
            history_on: true,
            manip_size: 1.0,
            pivot_mode: false,
            gpu,
            smooth_cache: HashMap::new(),
            vp_state: Default::default(),
            panels: Default::default(),
            windows_state: Default::default(),
            playing: false,
            play_clock: 0.0,
            last_action: None,
            clipboard: None,
            test,
            recent: cc.storage.and_then(|s| eframe::get_value(s, "recent")).unwrap_or_default(),
            custom_shelf: cc.storage.and_then(|s| eframe::get_value(s, "customShelf")).unwrap_or_default(),
            space_down: None,
            select_hierarchy: false,
            last_comp_mode: CompMode::Vertex,
            sel_mask_off: Default::default(),
            lock_selection: false,
            snap_curve: false,
            snap_point: false,
            live_surface: None,
            symmetry: 0,
            paint_radius: 24.0,
            play_dir: 1.0,
            range_sel: None,
            key_clip: vec![],
            prev_range: None,
            recent_projects: cc.storage.and_then(|s| eframe::get_value(s, "recentProjects")).unwrap_or_default(),
            project_win: None,
            autosave: cc.storage.and_then(|s| eframe::get_value(s, "autosave")).unwrap_or((false, 10.0)),
            last_autosave: 0.0,
            prompt: None,
            dup_opts: cc.storage.and_then(|s| eframe::get_value(s, "dupOpts")).unwrap_or_default(),
            dup_special_open: false,
            add_attr: None,
            last_hidden: vec![],
            hud: cc.storage.and_then(|s| eframe::get_value(s, "hud")).unwrap_or_default(),
            overlay_normals: HashSet::new(),
            overlay_border: HashSet::new(),
            fps_measured: 0.0,
            mesh_tool: Default::default(),
            mesh_opts: cc.storage.and_then(|s| eframe::get_value(s, "meshTools")).unwrap_or_default(),
            last_dup: None,
            bg_mode: cc.storage.and_then(|s| eframe::get_value(s, "bgMode")).unwrap_or(0),
            hotbox: None,
        };
        app.panels.ol = cc.storage.and_then(|s| eframe::get_value(s, "outliner")).unwrap_or_default();
        if app.test.is_none() {
            if let Some(p) = cc.storage.and_then(|s| eframe::get_value::<String>(s, "lastProject")) {
                crate::project::set_project(&mut app, &p, true);
                app.script_log.clear();
            }
        }
        if let Some(p) = std::env::args().nth(1).filter(|a| !a.starts_with('-')) {
            app.open_path(std::path::Path::new(&p));
        }
        app.s.modified = false;
        app
    }

    // ------------------------------------------------------------ running commands

    /// Run MEL as one undoable step, echoing it like Maya's Script Editor.
    pub fn run_mel(&mut self, label: &str, src: &str) -> Result<Val, String> {
        self.s.checkpoint(label);
        let before = self.s.rev;
        let r = self.exec_mel(src, true);
        if self.s.rev == before {
            // nothing changed (a query or an error): drop the empty undo step
            self.s.undo_stack.pop();
        }
        r
    }

    /// Run MEL without an undo checkpoint (the caller made one).
    pub fn exec_mel(&mut self, src: &str, echo: bool) -> Result<Val, String> {
        if echo {
            for line in src.lines().filter(|l| !l.trim().is_empty()) {
                self.script_log.push((MsgKind::Echo, line.to_string()));
            }
        }
        let mut host = UiHost::default();
        let r = self.mel.run(&mut self.s, &mut host, src);
        for o in self.mel.output.drain(..) {
            let _ = o;
        }
        self.flush_msgs();
        match &r {
            Ok(v) => {
                let t = v.to_string();
                if !t.is_empty() {
                    self.last_result = format!("// Result: {t} //");
                    self.script_log.push((MsgKind::Result, self.last_result.clone()));
                } else {
                    self.last_result.clear();
                }
            }
            Err(e) => {
                self.last_result = format!("// Error: {e} //");
                self.script_log.push((MsgKind::Error, self.last_result.clone()));
            }
        }
        for (cmd, args) in host.reqs {
            self.ui_command(&cmd, &args);
        }
        r
    }

    fn flush_msgs(&mut self) {
        for (k, m) in self.s.msgs.drain(..) {
            if k == MsgKind::Warning || k == MsgKind::Error {
                self.last_result = m.clone();
            }
            self.script_log.push((k, m));
        }
        if self.script_log.len() > 2000 {
            let n = self.script_log.len() - 2000;
            self.script_log.drain(..n);
        }
    }

    /// Echo a line in the Script Editor without running it (direct manipulation).
    pub fn echo(&mut self, line: impl Into<String>) {
        self.script_log.push((MsgKind::Echo, line.into()));
    }

    fn ui_command(&mut self, cmd: &str, args: &[Val]) {
        match cmd {
            "setToolTo" => {
                let t = args.first().map(|a| a.str()).unwrap_or_default();
                self.set_tool(match t.as_str() {
                    "moveSuperContext" | "Move" => Tool::Move,
                    "RotateSuperContext" | "Rotate" => Tool::Rotate,
                    "scaleSuperContext" | "Scale" => Tool::Scale,
                    _ => Tool::Select,
                });
            }
            "viewFit" | "FrameSelected" => self.action("frameSelected"),
            "setProject" => {
                if let Some(p) = args.first().map(|a| a.str()) {
                    crate::project::set_project(self, &p, false);
                }
            }
            "FrameAll" => self.action("frameAll"),
            "ScriptEditor" => self.win.script_editor = true,
            "HypershadeWindow" => self.win.hypershade = true,
            "GraphEditor" => self.win.graph_editor = true,
            "RenderViewWindow" | "render" => {
                self.win.render_view = true;
                if cmd == "render" {
                    self.windows_state.start_render(&self.s, &self.vps[self.active_vp].camera, 0);
                }
            }
            "OutlinerWindow" => self.show_outliner = true,
            "play" => self.playing = !args.iter().any(|a| a.str() == "-st" || a.str() == "-state") || args.iter().any(|a| a.truthy() && a.str() != "-st"),
            _ => {}
        }
    }

    pub fn set_tool(&mut self, t: Tool) {
        if t.is_manip() {
            self.last_tool = t;
        }
        if self.tool != t {
            self.vp_state.tool_pts.clear();
            if self.tool == Tool::MultiCut {
                crate::mesh_tools::multi_cut_complete(self);
            }
            self.mesh_tool.cut.clear();
            self.mesh_tool.cut_obj = None;
        }
        self.tool = t;
        self.help = match t {
            Tool::Select => "Select Tool: select an object",
            Tool::Lasso => "Lasso Tool: drag a freehand outline around objects or components",
            Tool::Paint => "Paint Selection Tool: drag over components (or objects) to select them; Ctrl drag to deselect",
            Tool::Move => "Move Tool: Use manipulator to move object(s). Ctrl+MMB-drag to move components along normals.",
            Tool::Rotate => "Rotate Tool: Use manipulator to rotate object(s).",
            Tool::Scale => "Scale Tool: Use manipulator to scale object(s).",
            Tool::CvCurve => "CV Curve Tool: click to place CVs, Enter to finish, Backspace removes the last one",
            Tool::EpCurve => "EP Curve Tool: click to place edit points, Enter to finish",
            Tool::Arc => "Three Point Circular Arc: click the start, a point on the arc and the end",
            Tool::Polygon => "Create Polygon Tool: click to place vertices, Enter to create the face",
            Tool::Joint => "Joint Tool: click to place joints (each new joint is parented to the last), Enter to finish the chain",
            Tool::EdgeLoop => "Insert Edge Loop Tool: click and drag on an edge to insert an edge loop and slide it. Set multiple loops in Tool Settings",
            Tool::MultiCut => "Multi-Cut Tool: click on edges/vertices to cut. Shift snaps to 10% steps, Ctrl+click inserts an edge loop. Enter or right-click completes, Backspace undoes the last point",
            Tool::TargetWeld => "Target Weld Tool: drag from a vertex onto another vertex to weld them together",
            Tool::Sculpt => "Sculpt: drag on a mesh to sculpt. Ctrl inverts, Shift smooths. Brush and size in Tool Settings",
            Tool::QuadDraw => "Quad Draw: click to place dots on the live surface, Shift+click to fill a quad, drag to slide, Enter to finish",
        }
        .into();
    }

    pub fn set_comp_mode(&mut self, m: Option<CompMode>) {
        if let Some(c) = m {
            self.last_comp_mode = c;
        }
        let mel = match m {
            None => "selectMode -o;",
            Some(CompMode::Vertex) => "selectMode -co; selectType -pv 1;",
            Some(CompMode::Edge) => "selectMode -co; selectType -pe 1;",
            Some(CompMode::Face) => "selectMode -co; selectType -pf 1;",
        };
        let _ = self.exec_mel(mel, true);
    }

    pub fn select_names(&mut self, ids: &[String], mode: SelMode) {
        let names: Vec<String> = ids.iter().map(|i| self.s.name_of(i)).collect();
        let flag = match mode {
            SelMode::Replace => "-r",
            SelMode::Add => "-add",
            SelMode::Toggle => "-tgl",
            SelMode::Remove => "-d",
        };
        if names.is_empty() {
            if mode == SelMode::Replace {
                self.echo("select -cl;");
                self.s.select(&[], SelMode::Replace);
                if self.s.comp_mode.is_some() {
                    self.s.comp.clear();
                }
            }
            return;
        }
        self.echo(format!("select {flag} {};", names.join(" ")));
        self.s.select(ids, mode);
    }

    pub fn open_path(&mut self, p: &std::path::Path) {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let r = if ext == "inca" { inca_core::io::open_scene(&mut self.s, p).map(|_| vec![]) } else { inca_core::io::import(&mut self.s, p) };
        match r {
            Ok(_) => {
                self.echo(format!("file -o \"{}\";", p.display()));
                self.smooth_cache.clear();
                self.add_recent(p);
            }
            Err(e) => self.last_result = format!("// Error: {e} //"),
        }
    }

    pub fn add_recent(&mut self, p: &std::path::Path) {
        let s = p.to_string_lossy().to_string();
        self.recent.retain(|r| r != &s);
        self.recent.insert(0, s);
        self.recent.truncate(10);
    }

    pub fn title(&self) -> String {
        let name = self.s.file.as_ref().and_then(|f| f.file_name()).map(|f| f.to_string_lossy().to_string()).unwrap_or("untitled".into());
        format!("{}{} - Inca{}", name, if self.s.modified { "*" } else { "" }, self.s.project.as_ref().map(|p| format!(" - {p}")).unwrap_or_default())
    }

    // ------------------------------------------------------------ actions

    /// Run a named action (menus, shelves, hotkeys all go through here).
    pub fn action(&mut self, id: &str) {
        self.hotbox = None;
        if !matches!(id, "repeatLast" | "undo" | "redo") {
            self.last_action = Some(id.to_string());
        }
        if let Some(mel) = crate::actions::mel_for(self, id) {
            let label = crate::actions::label(id);
            let _ = self.run_mel(&label, &mel);
            return;
        }
        crate::actions::builtin(self, id);
    }

    // ------------------------------------------------------------ frame

    fn hotkeys(&mut self, ctx: &egui::Context) {
        // clicked buttons and the viewport take keyboard focus in egui; only text fields should keep it
        if let Some(id) = ctx.memory(|m| m.focused()) {
            if egui::text_edit::TextEditState::load(ctx, id).is_none() {
                ctx.memory_mut(|m| m.surrender_focus(id));
            }
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        let now = ctx.input(|i| i.time);
        for e in &events {
            // Space: a tap toggles single/four view, holding it opens the hotbox
            if let egui::Event::Key { key: egui::Key::Space, pressed, repeat: false, modifiers, .. } = e {
                if modifiers.is_none() {
                    if *pressed {
                        self.space_down = Some(now);
                    } else if let Some(t0) = self.space_down.take() {
                        if now - t0 < 0.25 && self.hotbox.is_none() {
                            self.action("toggleLayout");
                        }
                    }
                }
            }
            // holding D edits the pivot
            if let egui::Event::Key { key: egui::Key::D, pressed, repeat: false, modifiers, .. } = e {
                if !modifiers.ctrl && !modifiers.command && !modifiers.alt && !modifiers.shift {
                    self.pivot_mode = *pressed;
                }
            }
        }
        if let Some(t0) = self.space_down {
            if now - t0 >= 0.25 && self.hotbox.is_none() {
                self.hotbox = ctx.input(|i| i.pointer.hover_pos()).or(Some(ctx.content_rect().center()));
            }
            ctx.request_repaint();
        }
        for e in events {
            if let egui::Event::Key { key, pressed: true, modifiers, repeat: false, .. } = e {
                if let Some(a) = crate::actions::hotkey(key, modifiers, self) {
                    self.action(a);
                }
            }
        }
    }

    /// Play forwards (dir 1) or backwards (-1); the same direction again stops.
    pub fn toggle_play(&mut self, dir: f64) {
        if self.playing && self.play_dir == dir {
            self.playing = false;
            let t = self.s.time.current.round();
            self.s.set_time(t);
            return;
        }
        self.playing = true;
        self.play_dir = dir;
        self.play_clock = 0.0;
    }

    fn tick_playback(&mut self, ctx: &egui::Context) {
        if !self.playing {
            return;
        }
        let now = ctx.input(|i| i.time);
        let fps = self.s.time.fps.max(1.0);
        if self.play_clock == 0.0 {
            self.play_clock = now;
        }
        // play every frame steps one frame per redraw; otherwise follow the clock
        let frames = if self.s.time.play_every { 1.0 } else { ((now - self.play_clock) * fps).floor() };
        if frames >= 1.0 {
            self.play_clock = if self.s.time.play_every { now } else { self.play_clock + frames / fps };
            let (start, end) = (self.s.time.start, self.s.time.end);
            let len = (end - start + 1.0).max(1.0);
            let mut t = self.s.time.current + frames * self.play_dir;
            let lp = self.s.time.r#loop.clone();
            if t > end {
                match lp.as_str() {
                    "once" => {
                        self.s.set_time(end);
                        self.playing = false;
                        return;
                    }
                    "oscillate" => {
                        self.play_dir = -1.0;
                        t = end - (t - end);
                    }
                    _ => t = start + (t - start).rem_euclid(len),
                }
            }
            if t < start {
                match lp.as_str() {
                    "once" => {
                        self.s.set_time(start);
                        self.playing = false;
                        return;
                    }
                    "oscillate" => {
                        self.play_dir = 1.0;
                        t = start + (start - t);
                    }
                    _ => t = end - (start - t - 1.0).rem_euclid(len),
                }
            }
            self.s.set_time(t.round());
        }
        ctx.request_repaint();
    }
}

impl eframe::App for IncaApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "recent", &self.recent);
        eframe::set_value(storage, "customShelf", &self.custom_shelf);
        eframe::set_value(storage, "workspace", &self.workspace);
        eframe::set_value(storage, "bgMode", &self.bg_mode);
        eframe::set_value(storage, "outliner", &self.panels.ol);
        eframe::set_value(storage, "recentProjects", &self.recent_projects);
        eframe::set_value(storage, "autosave", &self.autosave);
        eframe::set_value(storage, "dupOpts", &self.dup_opts);
        eframe::set_value(storage, "hud", &self.hud);
        eframe::set_value(storage, "meshTools", &self.mesh_opts);
        if let Some(p) = &self.s.project {
            eframe::set_value(storage, "lastProject", p);
        }
        eframe::set_value(storage, "rightTab", &self.right_tab);
        eframe::set_value(storage, "showRight", &self.show_right);
        eframe::set_value(storage, "showOutliner", &self.show_outliner);
        // a Ctrl+Space hide isn't remembered
        let hidden = self.ui_stash.as_ref().map_or(&self.hidden, |s| &s.0);
        eframe::set_value(storage, "hiddenUi", hidden);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // files dropped on the window open (.inca) or import (everything else)
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        for p in dropped {
            if p.extension().is_some_and(|e| e == "inca") && self.s.nodes.values().all(|n| n.startup()) {
                self.open_path(&p);
            } else {
                let _ = self.run_mel("import", &format!("file -i \"{}\";", p.display().to_string().replace('\\', "/")));
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.title()));
        self.tick_playback(&ctx);
        let dt = ctx.input(|i| i.unstable_dt);
        if dt > 0.0 {
            self.fps_measured = self.fps_measured * 0.9 + 0.1 / dt;
        }
        self.hotkeys(&ctx);
        // constraints follow their targets live (gizmo drags, channel edits)
        let moved = self.s.eval_constraints();
        if !moved.is_empty() {
            self.s.refresh_many(&moved);
        }
        // ---- layout (Maya order: menus, status line, shelf, then the work area)
        egui::Panel::top("menubar").exact_size(22.0).resizable(false).frame(theme::bar_frame()).show(ui, |ui| crate::menus::menu_bar(self, ui));
        if self.shown("statusline") {
            egui::Panel::top("statusline").exact_size(28.0).resizable(false).frame(theme::bar_frame()).show(ui, |ui| toolbar::status_line(self, ui));
        }
        if self.shown("shelf") {
            egui::Panel::top("shelf").exact_size(64.0).resizable(false).frame(theme::bar_frame()).show(ui, |ui| toolbar::shelf(self, ui));
        }
        if self.shown("helpline") {
            egui::Panel::bottom("helpline").exact_size(20.0).resizable(false).frame(theme::bar_frame()).show(ui, |ui| timeline::help_line(self, ui));
        }
        if self.shown("cmdline") {
            egui::Panel::bottom("cmdline").exact_size(24.0).resizable(false).frame(theme::bar_frame()).show(ui, |ui| timeline::command_line(self, ui));
        }
        if self.shown("rangeslider") {
            egui::Panel::bottom("range").exact_size(26.0).resizable(false).frame(theme::bar_frame()).show(ui, |ui| timeline::range_slider(self, ui));
        }
        if self.shown("timeslider") {
            egui::Panel::bottom("timeslider").exact_size(36.0).resizable(false).frame(theme::bar_frame()).show(ui, |ui| timeline::time_slider(self, ui));
        }
        if self.shown("toolbox") {
            egui::Panel::left("toolbox").exact_size(38.0).resizable(false).frame(theme::bar_frame()).show(ui, |ui| toolbar::tool_box(self, ui));
        }
        if self.show_outliner {
            egui::Panel::left("outlinerWindow").default_size(220.0).frame(theme::panel_frame()).show(ui, |ui| panels::outliner(self, ui));
        }
        if self.shown("sidetabs") {
            egui::Panel::right("sidetabs").exact_size(20.0).resizable(false).frame(egui::Frame::NONE.fill(theme::BG2).stroke(egui::Stroke::new(1.0, theme::LINE))).show(ui, |ui| panels::side_tabs(self, ui));
        }
        if self.show_right {
            egui::Panel::right("rightpanel").default_size(300.0).min_size(220.0).frame(theme::panel_frame()).show(ui, |ui| panels::right_panel(self, ui));
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(theme::BG_DARK)).show(ui, |ui| vp_ui::viewports(self, ui, frame));
        windows::show(self, &ctx, frame);
        panels::ae_copies(self, &ctx);
        crate::project::project_window(self, &ctx);
        crate::edit_cmds::prompt_window(self, &ctx);
        crate::edit_cmds::dup_special_window(self, &ctx);
        crate::edit_cmds::add_attr_window(self, &ctx);
        crate::project::autosave_tick(self, ctx.input(|i| i.time));
        crate::menus::hotbox(self, &ctx);
        crate::test_hook::step(self, &ctx);
        if self.panels.quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
