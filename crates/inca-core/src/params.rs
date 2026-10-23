//! Loose JSON parameter maps used by construction-history nodes (same shape as the JS app's op params).

use crate::math::DVec3;
use serde_json::{Map, Value};

pub type Params = Map<String, Value>;

pub fn num(p: &Params, k: &str) -> Option<f64> {
    match p.get(k)? {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}
pub fn num_or(p: &Params, k: &str, d: f64) -> f64 {
    num(p, k).unwrap_or(d)
}
pub fn bool_or(p: &Params, k: &str, d: bool) -> bool {
    match p.get(k) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0) != 0.0,
        _ => d,
    }
}
pub fn str_or<'a>(p: &'a Params, k: &str, d: &'a str) -> &'a str {
    p.get(k).and_then(|v| v.as_str()).unwrap_or(d)
}
pub fn u32_list(p: &Params, k: &str) -> Option<Vec<u32>> {
    Some(p.get(k)?.as_array()?.iter().filter_map(|v| v.as_u64().map(|x| x as u32)).collect())
}
pub fn usize_list(p: &Params, k: &str) -> Option<Vec<usize>> {
    Some(p.get(k)?.as_array()?.iter().filter_map(|v| v.as_u64().map(|x| x as usize)).collect())
}
/// Edges as `[[a,b],...]`.
pub fn edge_list(p: &Params, k: &str) -> Option<Vec<(u32, u32)>> {
    Some(
        p.get(k)?
            .as_array()?
            .iter()
            .filter_map(|e| {
                let a = e.as_array()?;
                Some((a.first()?.as_u64()? as u32, a.get(1)?.as_u64()? as u32))
            })
            .collect(),
    )
}
pub fn vec3_list(p: &Params, k: &str) -> Option<Vec<DVec3>> {
    Some(p.get(k)?.as_array()?.iter().filter_map(value_vec3).collect())
}
pub fn value_vec3(v: &Value) -> Option<DVec3> {
    let a = v.as_array()?;
    Some(DVec3::new(a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?))
}
pub fn vec3_value(v: DVec3) -> Value {
    serde_json::json!([v.x, v.y, v.z])
}
pub fn edges_value(e: &[(u32, u32)]) -> Value {
    Value::Array(e.iter().map(|&(a, b)| serde_json::json!([a, b])).collect())
}
