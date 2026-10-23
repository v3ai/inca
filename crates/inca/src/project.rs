//! Projects, Increment & Save, Revert and autosave (src/js/core/io.js).

use crate::app::IncaApp;
use std::path::{Path, PathBuf};

/// Folders a new project gets (the JS PROJECT_DIRS).
pub const PROJECT_DIRS: &[&str] = &[
    "scenes", "assets", "images", "sourceimages", "renderData", "renderData/depth", "renderData/iprImages", "renderData/shaders", "clips", "sound", "scripts", "data", "movies", "timeEditor", "autosave", "sceneAssembly", "cache", "cache/particles", "cache/nCache", "cache/bifrost", "Time Editor/Clip Exports",
];

fn documents() -> PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default();
    let docs = home.join("Documents");
    if docs.is_dir() {
        docs
    } else {
        home
    }
}

/// A folder of the current project, or of the default project when none is set.
pub fn default_dir(app: &IncaApp, sub: &str) -> PathBuf {
    match &app.s.project {
        Some(p) => Path::new(p).join(sub),
        None => documents().join("inca").join("projects").join("default").join(sub),
    }
}

/// `scene.inca` -> `scene.0001.inca`, `scene.0001.inca` -> `scene.0002.inca`.
pub fn increment_path(p: &Path) -> PathBuf {
    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let digits: String = stem.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
    let base = stem[..stem.len() - digits.len()].trim_end_matches(['.', '_']).to_string();
    let n = digits.parse::<u32>().map(|n| n + 1).unwrap_or(1);
    p.with_file_name(format!("{base}.{n:04}.inca"))
}

pub fn save_to(app: &mut IncaApp, p: &Path) {
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    match inca_core::io::save_scene(&mut app.s, p) {
        Ok(_) => {
            app.add_recent(p);
            app.echo(format!("file -rename \"{}\"; file -save;", p.display()));
            app.last_result = format!("// Saved {}", p.display());
        }
        Err(e) => app.last_result = format!("// Error: {e}"),
    }
}

pub fn increment_and_save(app: &mut IncaApp) {
    match app.s.file.clone() {
        Some(f) => save_to(app, &increment_path(&f)),
        None => app.action("saveSceneAs"),
    }
}

pub fn revert(app: &mut IncaApp) {
    let Some(f) = app.s.file.clone() else { return };
    let ok = app.test.is_some()
        || rfd::MessageDialog::new().set_title("Revert").set_description(format!("Discard changes and reload {}?", f.display())).set_buttons(rfd::MessageButtons::OkCancel).show() == rfd::MessageDialogResult::Ok;
    if ok {
        app.open_path(&f);
    }
}

pub fn set_project(app: &mut IncaApp, p: &str, quiet: bool) {
    let p = p.trim_end_matches(['/', '\\']).to_string();
    if !Path::new(&p).is_dir() {
        if !quiet {
            app.last_result = format!("// Warning: Project folder does not exist: {p}");
        }
        return;
    }
    app.s.project = Some(p.clone());
    app.recent_projects.retain(|x| x != &p);
    app.recent_projects.insert(0, p.clone());
    app.recent_projects.truncate(8);
    app.echo(format!("setProject \"{p}\";"));
    if !quiet {
        app.last_result = format!("// Project set to {p}");
    }
}

/// Make the folders plus workspace.inca and a Maya-style workspace.mel.
pub fn create_project(root: &Path, name: &str, dirs: &[&str]) -> std::io::Result<PathBuf> {
    let p = root.join(name);
    for d in dirs {
        std::fs::create_dir_all(p.join(d))?;
    }
    let key = |d: &str| d.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect::<String>();
    let rules: serde_json::Map<String, serde_json::Value> = dirs.iter().map(|d| (key(d), serde_json::Value::from(*d))).collect();
    let ws = serde_json::json!({ "format": "inca-workspace", "version": env!("CARGO_PKG_VERSION"), "name": name, "fileRules": rules });
    std::fs::write(p.join("workspace.inca"), serde_json::to_string_pretty(&ws).unwrap_or_default())?;
    let mel: String = dirs.iter().map(|d| format!("workspace -fr \"{}\" \"{d}\";\n", d.chars().filter(|c| c.is_alphanumeric()).collect::<String>())).collect();
    std::fs::write(p.join("workspace.mel"), format!("//Inca workspace definition\n{mel}"))?;
    Ok(p)
}

/// File > Project Window state.
pub struct ProjectWin {
    pub name: String,
    pub location: String,
    pub dirs: Vec<bool>,
}

pub fn open_project_window(app: &mut IncaApp) {
    let (name, location) = match &app.s.project {
        Some(p) => {
            let p = Path::new(p);
            (p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), p.parent().map(|d| d.display().to_string()).unwrap_or_default())
        }
        None => ("new_project".into(), documents().join("inca").join("projects").display().to_string()),
    };
    app.project_win = Some(ProjectWin { name, location, dirs: vec![true; PROJECT_DIRS.len()] });
}

pub fn project_window(app: &mut IncaApp, ctx: &egui::Context) {
    let Some(w) = app.project_win.as_mut() else { return };
    let mut open = true;
    let mut accept = false;
    let mut cancel = false;
    egui::Window::new("Project Window").open(&mut open).default_size([560.0, 520.0]).show(ctx, |ui| {
        egui::Grid::new("pw").num_columns(2).show(ui, |ui| {
            ui.label("Current Project:");
            ui.add(egui::TextEdit::singleline(&mut w.name).desired_width(320.0));
            ui.end_row();
            ui.label("Location:");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut w.location).desired_width(290.0));
                if ui.button("Browse").clicked() {
                    if let Some(d) = rfd::FileDialog::new().set_directory(&w.location).pick_folder() {
                        w.location = d.display().to_string();
                    }
                }
            });
            ui.end_row();
        });
        crate::theme::section_header(ui, "Primary Project Locations");
        egui::ScrollArea::vertical().max_height(330.0).show(ui, |ui| {
            egui::Grid::new("pwdirs").num_columns(3).show(ui, |ui| {
                for (i, d) in PROJECT_DIRS.iter().enumerate() {
                    ui.label(d.rsplit('/').next().unwrap_or(d));
                    ui.checkbox(&mut w.dirs[i], "");
                    ui.label(egui::RichText::new(*d).color(crate::theme::TEXT_DIM));
                    ui.end_row();
                }
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            accept = ui.button("Accept").clicked();
            cancel = ui.button("Cancel").clicked();
        });
    });
    if accept {
        let name = if w.name.trim().is_empty() { "new_project".to_string() } else { w.name.trim().to_string() };
        let dirs: Vec<&str> = PROJECT_DIRS.iter().zip(&w.dirs).filter(|(_, on)| **on).map(|(d, _)| *d).collect();
        let root = PathBuf::from(&w.location);
        match create_project(&root, &name, &dirs) {
            Ok(p) => set_project(app, &p.display().to_string(), false),
            Err(e) => app.last_result = format!("// Error: could not create the project: {e}"),
        }
    }
    if accept || cancel || !open {
        app.project_win = None;
    }
}

/// Write `<scene>.autosave.inca` into the project's autosave folder every `interval` minutes while modified.
pub fn autosave_tick(app: &mut IncaApp, now: f64) {
    if !app.autosave.0 {
        return;
    }
    if app.last_autosave == 0.0 {
        app.last_autosave = now;
    }
    if now - app.last_autosave < app.autosave.1.max(1.0) * 60.0 {
        return;
    }
    app.last_autosave = now;
    if !app.s.modified {
        return;
    }
    let name = app.s.file.as_ref().and_then(|f| f.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or("untitled".into());
    let dir = default_dir(app, "autosave");
    // numbered autosaves, keeping the newest `autosave_limit`
    let prefix = format!("{name}.autosave.");
    let mut olds: Vec<(u32, PathBuf)> = std::fs::read_dir(&dir).map(|rd| rd.filter_map(|e| e.ok()).filter_map(|e| {
        let f = e.file_name().to_string_lossy().to_string();
        let n: u32 = f.strip_prefix(&prefix)?.strip_suffix(".inca")?.parse().ok()?;
        Some((n, e.path()))
    }).collect()).unwrap_or_default();
    olds.sort();
    let next = olds.last().map(|o| o.0 + 1).unwrap_or(1);
    let p = dir.join(format!("{prefix}{next:04}.inca"));
    let data = app.s.serialize();
    if std::fs::create_dir_all(&dir).is_ok() && std::fs::write(&p, serde_json::to_string(&data).unwrap_or_default()).is_ok() {
        app.help = format!("Autosaved {}", p.display());
        let keep = app.prefs.autosave_limit.max(1);
        if olds.len() + 1 > keep {
            for (_, o) in &olds[..olds.len() + 1 - keep] {
                let _ = std::fs::remove_file(o);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn increments_like_the_js() {
        assert_eq!(increment_path(Path::new("/a/scene.inca")), PathBuf::from("/a/scene.0001.inca"));
        assert_eq!(increment_path(Path::new("/a/scene.0009.inca")), PathBuf::from("/a/scene.0010.inca"));
        assert_eq!(increment_path(Path::new("/a/shot_12.inca")), PathBuf::from("/a/shot.0013.inca"));
    }

    #[test]
    fn creates_a_project() {
        let root = std::env::temp_dir().join(format!("inca-proj-{}", std::process::id()));
        let p = create_project(&root, "demo", PROJECT_DIRS).unwrap();
        assert!(p.join("scenes").is_dir() && p.join("cache/particles").is_dir());
        assert!(std::fs::read_to_string(p.join("workspace.mel")).unwrap().contains("workspace -fr \"scenes\" \"scenes\";"));
        let _ = std::fs::remove_dir_all(root);
    }
}
