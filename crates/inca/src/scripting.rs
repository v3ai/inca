//! Rhai scripting (the Rust counterpart of the JS app's `cmds.*` JavaScript tab).
//!
//! Scripts call `mel("polyCube -w 2")`, `cmd("move", [1, 2, 3])`, `getAttr("pCube1.tx")`,
//! `setAttr("pCube1.ty", 2.0)`, `ls()` and `select("pCube1")`.

use crate::app::IncaApp;
use inca_core::commands::NoHost;
use inca_core::mel::{Interp, Val};
use inca_core::scene::{MsgKind, Scene};
use rhai::{Array, Dynamic, Engine};
use std::sync::{Arc, Mutex};

type Shared = Arc<Mutex<(Scene, Interp, Vec<(MsgKind, String)>)>>;

pub fn engine() -> Engine {
    Engine::new()
}

fn to_dyn(v: Val) -> Dynamic {
    match v {
        Val::Int(i) => Dynamic::from(i),
        Val::Float(f) => Dynamic::from(f),
        Val::Str(s) => Dynamic::from(s),
        Val::Vec3(a) => Dynamic::from(a.iter().map(|x| Dynamic::from(*x)).collect::<Array>()),
        Val::Array(a) => Dynamic::from(a.into_iter().map(to_dyn).collect::<Array>()),
        Val::None => Dynamic::UNIT,
    }
}

fn from_dyn(d: &Dynamic) -> String {
    if let Some(a) = d.clone().try_cast::<Array>() {
        return a.iter().map(from_dyn).collect::<Vec<_>>().join(" ");
    }
    if d.is_string() {
        let s = d.clone().into_string().unwrap_or_default();
        if s.starts_with('-') || s.parse::<f64>().is_ok() {
            return s;
        }
        return format!("\"{s}\"");
    }
    d.to_string()
}

fn run(sh: &Shared, src: &str) -> Dynamic {
    let mut g = sh.lock().unwrap();
    let (s, i, log) = &mut *g;
    log.push((MsgKind::Echo, src.to_string()));
    let r = i.run(s, &mut NoHost, src);
    log.extend(s.msgs.drain(..));
    match r {
        Ok(v) => to_dyn(v),
        Err(e) => {
            log.push((MsgKind::Error, format!("// Error: {e}")));
            Dynamic::UNIT
        }
    }
}

pub fn run_rhai(app: &mut IncaApp, src: &str) -> Result<String, String> {
    let shared: Shared = Arc::new(Mutex::new((std::mem::take(&mut app.s), std::mem::take(&mut app.mel), vec![])));
    let mut e = engine();
    let sh = shared.clone();
    e.register_fn("mel", move |src: &str| run(&sh, src));
    let sh = shared.clone();
    e.register_fn("cmd", move |name: &str| run(&sh, &format!("{name};")));
    let sh = shared.clone();
    e.register_fn("cmd", move |name: &str, args: Array| run(&sh, &format!("{name} {};", args.iter().map(from_dyn).collect::<Vec<_>>().join(" "))));
    let sh = shared.clone();
    e.register_fn("getAttr", move |a: &str| run(&sh, &format!("getAttr {a};")));
    let sh = shared.clone();
    e.register_fn("setAttr", move |a: &str, v: Dynamic| run(&sh, &format!("setAttr {a} {};", from_dyn(&v))));
    let sh = shared.clone();
    e.register_fn("ls", move || run(&sh, "ls -dag;"));
    let sh = shared.clone();
    e.register_fn("selected", move || run(&sh, "ls -sl;"));
    let sh = shared.clone();
    e.register_fn("select", move |n: &str| run(&sh, &format!("select -r {n};")));
    let sh = shared.clone();
    e.on_print(move |t| sh.lock().unwrap().2.push((MsgKind::Info, t.to_string())));
    let r = e.eval::<Dynamic>(src);
    drop(e);
    let (s, i, log) = Arc::try_unwrap(shared).map_err(|_| "script still running".to_string())?.into_inner().unwrap();
    app.s = s;
    app.mel = i;
    app.script_log.extend(log);
    match r {
        Ok(v) if v.is_unit() => Ok(String::new()),
        Ok(v) => Ok(v.to_string()),
        Err(e) => Err(e.to_string()),
    }
}
