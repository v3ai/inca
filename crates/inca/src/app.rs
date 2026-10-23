//! Application state, actions (menu items / shelf buttons / hotkeys) and the frame loop.

use crate::viewport::{gpu::Gpu, Viewport};
use crate::{panels, theme, timeline, toolbar, vp_ui, windows};
use inca_core::commands::Host;
use inca_core::mel::{Interp, Val};
use inca_core::scene::{CompMode, MsgKind, Scene, SelMode};
use std::collections::HashMap;

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
        const UI: &[&str] = &["setToolTo", "viewFit", "FrameSelected", "FrameAll", "refresh", "play", "ScriptEditor", "HypershadeWindow", "GraphEditor", "RenderViewWindow", "OutlinerWindow", "render", "modelEditor", "workspace", "inViewMessage", "headsUpMessage", "confirmDialog", "promptDialog"];
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
            hotbox: None,
        };
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
                if !modifiers.ctrl && !modifiers.command && !modifiers.alt {
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

    fn tick_playback(&mut self, ctx: &egui::Context) {
        if !self.playing {
            return;
        }
        let now = ctx.input(|i| i.time);
        let fps = self.s.time.fps.max(1.0);
        if self.play_clock == 0.0 {
            self.play_clock = now;
        }
        let frames = ((now - self.play_clock) * fps).floor();
        if frames >= 1.0 {
            self.play_clock += frames / fps;
            let mut t = self.s.time.current + frames;
            if t > self.s.time.end {
                t = self.s.time.start + (t - self.s.time.end - 1.0).rem_euclid((self.s.time.end - self.s.time.start + 1.0).max(1.0));
            }
            self.s.set_time(t);
        }
        ctx.request_repaint();
    }

}

impl eframe::App for IncaApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "recent", &self.recent);
        eframe::set_value(storage, "customShelf", &self.custom_shelf);
        eframe::set_value(storage, "workspace", &self.workspace);
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
        if self.show_right {
            egui::Panel::right("rightpanel").default_size(300.0).min_size(220.0).frame(theme::panel_frame()).show(ui, |ui| panels::right_panel(self, ui));
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(theme::BG_DARK)).show(ui, |ui| vp_ui::viewports(self, ui, frame));
        windows::show(self, &ctx, frame);
        crate::menus::hotbox(self, &ctx);
        crate::test_hook::step(self, &ctx);
        if self.panels.quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
