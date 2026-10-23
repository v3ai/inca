//! Keyframe animation: anim curves with Maya-style tangents and infinity modes.

use serde::{Deserialize, Serialize};

pub const TANGENTS: &[&str] = &["auto", "spline", "clamped", "linear", "flat", "step", "plateau", "fixed"];
pub const INFINITY: &[&str] = &["constant", "linear", "cycle", "cycleRelative", "oscillate"];

fn auto() -> String {
    "auto".into()
}
fn constant() -> String {
    "constant".into()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Key {
    pub t: f64,
    pub v: f64,
    #[serde(default = "auto")]
    pub it: String,
    #[serde(default = "auto")]
    pub ot: String,
    /// fixed in/out slopes (value per frame)
    #[serde(default)]
    pub is: f64,
    #[serde(default)]
    pub os: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AnimCurve {
    pub node: String,
    pub attr: String,
    pub keys: Vec<Key>,
    #[serde(default = "constant")]
    pub pre: String,
    #[serde(default = "constant")]
    pub post: String,
}

impl AnimCurve {
    pub fn new(node: &str, attr: &str) -> Self {
        AnimCurve { node: node.into(), attr: attr.into(), keys: vec![], pre: constant(), post: constant() }
    }
    pub fn find(&self, t: f64) -> Option<usize> {
        self.keys.iter().position(|k| (k.t - t).abs() < 1e-4)
    }
    pub fn set_key(&mut self, t: f64, v: f64, tangent: &str) -> usize {
        if let Some(i) = self.find(t) {
            self.keys[i].v = v;
            return i;
        }
        let ot = if tangent == "step" { "step" } else { tangent };
        self.keys.push(Key { t, v, it: tangent.into(), ot: ot.into(), is: 0.0, os: 0.0 });
        self.sort();
        self.find(t).unwrap()
    }
    pub fn sort(&mut self) {
        self.keys.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    }
    pub fn remove_at(&mut self, t: f64) -> bool {
        match self.find(t) {
            Some(i) => {
                self.keys.remove(i);
                true
            }
            None => false,
        }
    }

    /// In/out slopes (value per frame) of key i.
    pub fn slopes(&self, i: usize) -> (f64, f64) {
        let k = &self.keys[i];
        let p = if i > 0 { self.keys.get(i - 1) } else { None };
        let n = self.keys.get(i + 1);
        let lin = |a: &Key, b: &Key| (b.v - a.v) / if b.t - a.t == 0.0 { 1.0 } else { b.t - a.t };
        let one = |ty: &str, inside: bool| -> f64 {
            match ty {
                "fixed" => {
                    if inside {
                        k.is
                    } else {
                        k.os
                    }
                }
                "flat" | "step" | "stepnext" => 0.0,
                "linear" => {
                    if inside {
                        p.map(|p| lin(p, k)).or_else(|| n.map(|n| lin(k, n))).unwrap_or(0.0)
                    } else {
                        n.map(|n| lin(k, n)).or_else(|| p.map(|p| lin(p, k))).unwrap_or(0.0)
                    }
                }
                _ => {
                    if p.is_none() && n.is_none() {
                        return 0.0;
                    }
                    let s = match (p, n) {
                        (None, Some(n)) => lin(k, n),
                        (Some(p), None) => lin(p, k),
                        (Some(p), Some(n)) => (n.v - p.v) / if n.t - p.t == 0.0 { 1.0 } else { n.t - p.t },
                        _ => 0.0,
                    };
                    if ty == "spline" {
                        return s;
                    }
                    let (Some(p), Some(n)) = (p, n) else {
                        return if ty == "clamped" { s } else { 0.0 };
                    };
                    let d0 = k.v - p.v;
                    let d1 = n.v - k.v;
                    if ty == "clamped" {
                        return if d0.abs() < 1e-6 || d1.abs() < 1e-6 || d0 * d1 < 0.0 { 0.0 } else { s };
                    }
                    // auto / plateau: flatten at extremes and prevent overshoot
                    if d0 * d1 <= 0.0 {
                        return 0.0;
                    }
                    let m0 = d0 / (k.t - p.t);
                    let m1 = d1 / (n.t - k.t);
                    let lim = 3.0 * m0.abs().min(m1.abs());
                    if s.abs() > lim {
                        s.signum() * lim
                    } else {
                        s
                    }
                }
            }
        };
        (one(&k.it, true), one(&k.ot, false))
    }

    fn eval_raw(&self, t: f64) -> f64 {
        let k = &self.keys;
        let n = k.len();
        if n == 0 {
            return 0.0;
        }
        if n == 1 || t <= k[0].t {
            return k[0].v;
        }
        if t >= k[n - 1].t {
            return k[n - 1].v;
        }
        let hi = k.partition_point(|x| x.t <= t).min(n - 1);
        let lo = hi - 1;
        let (a, b) = (&k[lo], &k[hi]);
        if a.ot == "step" {
            return a.v;
        }
        if a.ot == "stepnext" {
            return b.v;
        }
        let dt = b.t - a.t;
        let s = (t - a.t) / dt;
        let m0 = self.slopes(lo).1 * dt;
        let m1 = self.slopes(hi).0 * dt;
        let s2 = s * s;
        let s3 = s2 * s;
        (2.0 * s3 - 3.0 * s2 + 1.0) * a.v + (s3 - 2.0 * s2 + s) * m0 + (-2.0 * s3 + 3.0 * s2) * b.v + (s3 - s2) * m1
    }

    pub fn eval(&self, t: f64) -> f64 {
        let n = self.keys.len();
        if n == 0 {
            return 0.0;
        }
        if n == 1 {
            return self.keys[0].v;
        }
        let (t0, t1) = (self.keys[0].t, self.keys[n - 1].t);
        if t < t0 {
            return self.inf(t, &self.pre, true);
        }
        if t > t1 {
            return self.inf(t, &self.post, false);
        }
        self.eval_raw(t)
    }

    fn inf(&self, t: f64, mode: &str, before: bool) -> f64 {
        let n = self.keys.len();
        let (t0, t1) = (self.keys[0].t, self.keys[n - 1].t);
        let span = if t1 - t0 == 0.0 { 1.0 } else { t1 - t0 };
        let (v0, v1) = (self.keys[0].v, self.keys[n - 1].v);
        match mode {
            "linear" => {
                if before {
                    v0 + (t - t0) * self.slopes(0).0
                } else {
                    v1 + (t - t1) * self.slopes(n - 1).1
                }
            }
            "cycle" | "cycleRelative" | "oscillate" => {
                let c = ((t - t0) / span).floor();
                let mut lt = t - c * span;
                if mode == "oscillate" && (c as i64) % 2 != 0 {
                    lt = t1 - (lt - t0);
                }
                let off = if mode == "cycleRelative" { c * (v1 - v0) } else { 0.0 };
                self.eval_raw(lt) + off
            }
            _ => {
                if before {
                    v0
                } else {
                    v1
                }
            }
        }
    }
}

/// All anim curves of a scene, keyed by "nodeId.attr".
#[derive(Clone, Debug, Default)]
pub struct Anim {
    pub curves: indexmap::IndexMap<String, AnimCurve>,
}

impl Anim {
    pub fn key(node: &str, attr: &str) -> String {
        format!("{node}.{attr}")
    }
    pub fn curve(&self, node: &str, attr: &str) -> Option<&AnimCurve> {
        self.curves.get(&Self::key(node, attr))
    }
    pub fn curve_mut(&mut self, node: &str, attr: &str, create: bool) -> Option<&mut AnimCurve> {
        let k = Self::key(node, attr);
        if create && !self.curves.contains_key(&k) {
            self.curves.insert(k.clone(), AnimCurve::new(node, attr));
        }
        self.curves.get_mut(&k)
    }
    pub fn curves_of(&self, node: &str) -> impl Iterator<Item = &AnimCurve> {
        let node = node.to_string();
        self.curves.values().filter(move |c| c.node == node)
    }
    pub fn is_keyed(&self, node: &str, attr: &str) -> bool {
        self.curve(node, attr).is_some_and(|c| !c.keys.is_empty())
    }
    pub fn on_key(&self, node: &str, attr: &str, t: f64) -> bool {
        self.curve(node, attr).is_some_and(|c| c.find(t).is_some())
    }
    pub fn remove_node(&mut self, node: &str) {
        self.curves.retain(|_, c| c.node != node);
    }
    /// Key times of a node (or all nodes), sorted and unique.
    pub fn key_times(&self, nodes: &[String]) -> Vec<f64> {
        let mut ts: Vec<f64> = self.curves.values().filter(|c| nodes.is_empty() || nodes.contains(&c.node)).flat_map(|c| c.keys.iter().map(|k| k.t)).collect();
        ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ts.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        ts
    }
    pub fn serialize(&self) -> Vec<AnimCurve> {
        self.curves.values().cloned().collect()
    }
    pub fn load(&mut self, list: Vec<AnimCurve>) {
        self.curves.clear();
        for c in list {
            self.curves.insert(Self::key(&c.node, &c.attr), c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_and_auto() {
        let mut c = AnimCurve::new("n", "translateX");
        c.set_key(1.0, 0.0, "linear");
        c.set_key(11.0, 10.0, "linear");
        assert!((c.eval(6.0) - 5.0).abs() < 1e-9);
        assert_eq!(c.eval(-5.0), 0.0);
        let mut a = AnimCurve::new("n", "translateY");
        a.set_key(0.0, 0.0, "auto");
        a.set_key(10.0, 5.0, "auto");
        a.set_key(20.0, 0.0, "auto");
        // auto tangents flatten at the peak: no overshoot
        for t in 0..=20 {
            assert!(a.eval(t as f64) <= 5.0 + 1e-9);
        }
        a.post = "cycle".into();
        assert!((a.eval(30.0) - a.eval(10.0)).abs() < 1e-9);
    }
}
