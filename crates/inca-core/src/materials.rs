//! Shading nodes: materials and textures (attribute maps, same layout as the JS app).

use crate::params::Params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub struct MatType {
    pub ty: &'static str,
    pub label: &'static str,
    pub defaults: fn() -> Value,
    /// attributes that accept a texture connection
    pub mappable: &'static [&'static str],
}

pub static MAT_TYPES: &[MatType] = &[
    MatType { ty: "lambert", label: "Lambert", defaults: || json!({ "color": [0.5, 0.5, 0.5], "transparency": [0, 0, 0], "ambientColor": [0, 0, 0], "incandescence": [0, 0, 0], "diffuse": 0.8, "bump": 0 }), mappable: &["color", "transparency", "incandescence", "bump"] },
    MatType { ty: "blinn", label: "Blinn", defaults: || json!({ "color": [0.5, 0.5, 0.5], "transparency": [0, 0, 0], "ambientColor": [0, 0, 0], "incandescence": [0, 0, 0], "diffuse": 0.8, "eccentricity": 0.3, "specularRollOff": 0.7, "specularColor": [0.5, 0.5, 0.5], "reflectivity": 0.5, "bump": 0 }), mappable: &["color", "transparency", "incandescence", "specularColor", "bump"] },
    MatType { ty: "phong", label: "Phong", defaults: || json!({ "color": [0.5, 0.5, 0.5], "transparency": [0, 0, 0], "ambientColor": [0, 0, 0], "incandescence": [0, 0, 0], "diffuse": 0.8, "cosinePower": 20, "specularColor": [0.5, 0.5, 0.5], "reflectivity": 0.5, "bump": 0 }), mappable: &["color", "transparency", "incandescence", "specularColor", "bump"] },
    MatType { ty: "standardSurface", label: "Standard Surface", defaults: || json!({ "base": 0.8, "baseColor": [0.8, 0.8, 0.8], "diffuseRoughness": 0, "metalness": 0, "specular": 1, "specularColor": [1, 1, 1], "specularRoughness": 0.2, "specularIOR": 1.5, "transmission": 0, "transmissionColor": [1, 1, 1], "coat": 0, "coatRoughness": 0.1, "sheen": 0, "sheenColor": [1, 1, 1], "emission": 0, "emissionColor": [1, 1, 1], "opacity": [1, 1, 1], "thinWalled": false }), mappable: &["baseColor", "specularRoughness", "metalness", "emissionColor", "opacity", "bump"] },
    MatType { ty: "surfaceShader", label: "Surface Shader", defaults: || json!({ "outColor": [0.5, 0.5, 0.5], "outTransparency": [0, 0, 0] }), mappable: &["outColor"] },
];

pub struct TexType {
    pub ty: &'static str,
    pub label: &'static str,
    pub defaults: fn() -> Value,
}

pub static TEX_TYPES: &[TexType] = &[
    TexType { ty: "file", label: "File", defaults: || json!({ "fileTextureName": "", "repeatU": 1, "repeatV": 1, "offsetU": 0, "offsetV": 0, "rotateUV": 0, "colorSpace": "sRGB" }) },
    TexType { ty: "checker", label: "Checker", defaults: || json!({ "color1": [1, 1, 1], "color2": [0, 0, 0], "repeatU": 4, "repeatV": 4, "contrast": 1 }) },
    TexType { ty: "ramp", label: "Ramp", defaults: || json!({ "type": "V Ramp", "colors": [[0, [1, 0, 0]], [0.5, [0, 1, 0]], [1, [0, 0, 1]]], "repeatU": 1, "repeatV": 1 }) },
    TexType { ty: "noise", label: "Noise", defaults: || json!({ "color1": [1, 1, 1], "color2": [0, 0, 0], "frequency": 8, "amplitude": 1, "repeatU": 1, "repeatV": 1 }) },
    TexType { ty: "grid", label: "Grid", defaults: || json!({ "lineColor": [1, 1, 1], "fillerColor": [0, 0, 0], "uWidth": 0.1, "vWidth": 0.1, "repeatU": 4, "repeatV": 4 }) },
];

pub fn mat_type(ty: &str) -> &'static MatType {
    MAT_TYPES.iter().find(|m| m.ty == ty).unwrap_or(&MAT_TYPES[0])
}
pub fn tex_type(ty: &str) -> Option<&'static TexType> {
    TEX_TYPES.iter().find(|m| m.ty == ty)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Material {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub attrs: Params,
    /// attribute -> texture id
    #[serde(default)]
    pub maps: Params,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Texture {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub attrs: Params,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShadingData {
    pub mats: Vec<Material>,
    pub texs: Vec<Texture>,
    pub default_mat_id: Option<String>,
}

pub fn defaults_for(ty: &str) -> Params {
    match (mat_type(ty).defaults)() {
        Value::Object(m) => m,
        _ => Params::new(),
    }
}
pub fn tex_defaults_for(ty: &str) -> Params {
    match tex_type(ty).map(|t| (t.defaults)()) {
        Some(Value::Object(m)) => m,
        _ => Params::new(),
    }
}

fn rgb(v: Option<&Value>) -> Option<[f32; 3]> {
    let a = v?.as_array()?;
    Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32])
}
fn f(v: Option<&Value>) -> Option<f32> {
    v?.as_f64().map(|x| x as f32)
}

/// Simplified physically based view of any material, used by the viewport and the path tracer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pbr {
    pub base: [f32; 3],
    pub roughness: f32,
    pub metalness: f32,
    pub specular: f32,
    pub emission: [f32; 3],
    pub opacity: f32,
    pub transmission: f32,
    pub ior: f32,
    pub unlit: bool,
}

impl Default for Pbr {
    fn default() -> Self {
        Pbr { base: [0.4, 0.4, 0.4], roughness: 0.6, metalness: 0.0, specular: 0.0, emission: [0.0; 3], opacity: 1.0, transmission: 0.0, ior: 1.5, unlit: false }
    }
}

impl Material {
    pub fn new(id: String, name: String, ty: &str) -> Self {
        Material { id, name, ty: ty.into(), attrs: defaults_for(ty), maps: Params::new() }
    }

    pub fn pbr(&self) -> Pbr {
        let a = &self.attrs;
        let mut p = Pbr::default();
        match self.ty.as_str() {
            "standardSurface" => {
                let b = f(a.get("base")).unwrap_or(0.8);
                let c = rgb(a.get("baseColor")).unwrap_or([0.8; 3]);
                p.base = [c[0] * b, c[1] * b, c[2] * b];
                p.roughness = f(a.get("specularRoughness")).unwrap_or(0.2);
                p.metalness = f(a.get("metalness")).unwrap_or(0.0);
                p.specular = f(a.get("specular")).unwrap_or(1.0);
                let e = f(a.get("emission")).unwrap_or(0.0);
                let ec = rgb(a.get("emissionColor")).unwrap_or([1.0; 3]);
                p.emission = [ec[0] * e, ec[1] * e, ec[2] * e];
                p.opacity = rgb(a.get("opacity")).map(|o| (o[0] + o[1] + o[2]) / 3.0).unwrap_or(1.0);
                p.transmission = f(a.get("transmission")).unwrap_or(0.0);
                p.ior = f(a.get("specularIOR")).unwrap_or(1.5);
            }
            "surfaceShader" => {
                p.base = rgb(a.get("outColor")).unwrap_or([0.5; 3]);
                p.emission = p.base;
                p.unlit = true;
                p.opacity = 1.0 - rgb(a.get("outTransparency")).map(|o| (o[0] + o[1] + o[2]) / 3.0).unwrap_or(0.0);
            }
            ty => {
                let c = rgb(a.get("color")).unwrap_or([0.5; 3]);
                let d = f(a.get("diffuse")).unwrap_or(0.8);
                p.base = [c[0] * d, c[1] * d, c[2] * d];
                p.emission = rgb(a.get("incandescence")).unwrap_or([0.0; 3]);
                p.opacity = 1.0 - rgb(a.get("transparency")).map(|o| (o[0] + o[1] + o[2]) / 3.0).unwrap_or(0.0);
                match ty {
                    "blinn" => {
                        p.roughness = f(a.get("eccentricity")).unwrap_or(0.3).clamp(0.02, 1.0);
                        p.specular = rgb(a.get("specularColor")).map(|s| (s[0] + s[1] + s[2]) / 3.0).unwrap_or(0.5);
                    }
                    "phong" => {
                        let cp = f(a.get("cosinePower")).unwrap_or(20.0).max(2.0);
                        p.roughness = (2.0 / (cp + 2.0)).sqrt();
                        p.specular = rgb(a.get("specularColor")).map(|s| (s[0] + s[1] + s[2]) / 3.0).unwrap_or(0.5);
                    }
                    _ => {
                        p.roughness = 1.0;
                        p.specular = 0.0;
                    }
                }
            }
        }
        p
    }

    /// Main colour attribute name (for swatches and the texture "color" slot).
    pub fn color_attr(&self) -> &'static str {
        match self.ty.as_str() {
            "standardSurface" => "baseColor",
            "surfaceShader" => "outColor",
            _ => "color",
        }
    }
}

impl Texture {
    /// Evaluate procedural textures at uv; file textures return None (handled by the caller with the image).
    pub fn sample(&self, u: f64, v: f64) -> Option<[f32; 3]> {
        let a = &self.attrs;
        let ru = a.get("repeatU").and_then(|x| x.as_f64()).unwrap_or(1.0);
        let rv = a.get("repeatV").and_then(|x| x.as_f64()).unwrap_or(1.0);
        let (u, v) = (u * ru, v * rv);
        let mix = |c1: [f32; 3], c2: [f32; 3], t: f32| [c1[0] + (c2[0] - c1[0]) * t, c1[1] + (c2[1] - c1[1]) * t, c1[2] + (c2[2] - c1[2]) * t];
        match self.ty.as_str() {
            "checker" => {
                let c1 = rgb(a.get("color1")).unwrap_or([1.0; 3]);
                let c2 = rgb(a.get("color2")).unwrap_or([0.0; 3]);
                Some(if ((u.floor() + v.floor()) as i64).rem_euclid(2) == 0 { c1 } else { c2 })
            }
            "grid" => {
                let lc = rgb(a.get("lineColor")).unwrap_or([1.0; 3]);
                let fc = rgb(a.get("fillerColor")).unwrap_or([0.0; 3]);
                let uw = a.get("uWidth").and_then(|x| x.as_f64()).unwrap_or(0.1);
                let vw = a.get("vWidth").and_then(|x| x.as_f64()).unwrap_or(0.1);
                Some(if u.fract().abs() < uw || v.fract().abs() < vw { lc } else { fc })
            }
            "noise" => {
                let c1 = rgb(a.get("color1")).unwrap_or([1.0; 3]);
                let c2 = rgb(a.get("color2")).unwrap_or([0.0; 3]);
                let fq = a.get("frequency").and_then(|x| x.as_f64()).unwrap_or(8.0);
                Some(mix(c2, c1, value_noise(u * fq, v * fq) as f32))
            }
            "ramp" => {
                let stops: Vec<(f64, [f32; 3])> = a
                    .get("colors")
                    .and_then(|c| c.as_array())
                    .map(|l| l.iter().filter_map(|s| Some((s.get(0)?.as_f64()?, rgb(s.get(1))?))).collect())
                    .unwrap_or_default();
                let horizontal = a.get("type").and_then(|t| t.as_str()) == Some("U Ramp");
                let t = if horizontal { u.fract() } else { v.fract() };
                if stops.is_empty() {
                    return Some([t as f32; 3]);
                }
                let mut prev = stops[0];
                for s in &stops {
                    if t <= s.0 {
                        let k = if s.0 > prev.0 { ((t - prev.0) / (s.0 - prev.0)) as f32 } else { 0.0 };
                        return Some(mix(prev.1, s.1, k));
                    }
                    prev = *s;
                }
                Some(prev.1)
            }
            _ => None,
        }
    }
}

fn hash2(x: i64, y: i64) -> f64 {
    let mut h = (x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263)) as u64;
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) & 0xffff) as f64 / 65535.0
}
fn value_noise(x: f64, y: f64) -> f64 {
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let (xf, yf) = (x - x.floor(), y - y.floor());
    let s = |t: f64| t * t * (3.0 - 2.0 * t);
    let a = hash2(xi, yi);
    let b = hash2(xi + 1, yi);
    let c = hash2(xi, yi + 1);
    let d = hash2(xi + 1, yi + 1);
    let u = s(xf);
    let v = s(yf);
    (a + (b - a) * u) * (1.0 - v) + (c + (d - c) * u) * v
}
