//! Inca engine: scene graph with construction history, polygon kernel, curves, materials,
//! animation, undo, MEL, file IO and a CPU path tracer. No GUI code lives here.

pub mod booleans;
pub mod curves;
pub mod math;
pub mod text;
pub mod mesh;
pub mod ops;
pub mod params;
pub mod primitives;
pub mod anim;
pub mod materials;
pub mod scene;
pub mod io;
pub mod commands;
pub mod mel;
pub mod render;
pub mod selection;
pub mod uv;
pub mod rig;
pub mod deformers;
pub mod skin;
pub mod dynamics;
pub mod maya;
pub mod fbx;
