//! Drive the real app from a script and capture a screenshot (the JS app's INCA_TEST/INCA_SHOTS):
//!
//! `INCA_TEST=script.mel INCA_SHOT=out.png inca` runs the MEL script once the window is up,
//! saves a screenshot of the whole window and exits. With `INCA_SHOT_WAIT=<file>` it stays open
//! (for driving it with xdotool) and takes the screenshot once that file appears.

use crate::app::IncaApp;

pub fn step(app: &mut IncaApp, ctx: &egui::Context) {
    let Some(t) = app.test.as_mut() else { return };
    t.frame += 1;
    ctx.request_repaint();
    if t.frame == 3 {
        if let Some(src) = t.script.take() {
            // lines starting with "//!ui " are app actions (e.g. //!ui layoutFour)
            let (ui_lines, mel): (Vec<&str>, Vec<&str>) = src.lines().partition(|l| l.starts_with("//!ui "));
            let mel = mel.join("\n");
            if let Err(e) = app.run_mel("test script", &mel) {
                eprintln!("INCA_TEST error: {e}");
            }
            for l in ui_lines {
                let a = l.trim_start_matches("//!ui ").trim();
                match a.strip_prefix("workspace ") {
                    Some(w) => crate::workspace::apply(app, w),
                    None => app.action(a),
                }
            }
        }
    }
    let t = app.test.as_mut().unwrap();
    let wait = std::env::var("INCA_SHOT_WAIT").ok();
    let shot_frame: u32 = match &wait {
        Some(f) if std::path::Path::new(f).exists() => {
            let _ = std::fs::remove_file(f);
            t.frame
        }
        Some(_) => u32::MAX,
        None => std::env::var("INCA_SHOT_FRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(12),
    };
    if t.frame == shot_frame {
        match &t.shot {
            Some(_) => ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default())),
            None => {}
        }
        t.requested = true;
    }
    let shot = ctx.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
    });
    if let (Some(img), Some(path)) = (shot, t.shot.clone()) {
        let [w, h] = img.size;
        let px: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
        match image::save_buffer(&path, &px, w as u32, h as u32, image::ColorType::Rgba8) {
            Ok(_) => eprintln!("INCA_SHOT saved {}", path.display()),
            Err(e) => eprintln!("INCA_SHOT failed: {e}"),
        }
        for (k, l) in &app.script_log {
            eprintln!("{k:?}: {l}");
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    if wait.is_none() && t.frame > shot_frame + 400 {
        eprintln!("INCA_SHOT: no screenshot event, giving up");
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}
