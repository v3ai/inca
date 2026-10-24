// Inca viewport shaders: gradient background, shaded meshes, lines.

struct Light {
    pos: vec4<f32>,    // xyz position, w = type (0 ambient, 1 directional, 2 point, 3 spot, 4 area)
    dir: vec4<f32>,    // xyz direction the light points to
    color: vec4<f32>,  // rgb * intensity
    params: vec4<f32>, // x cos(outer), y cos(inner), z decay rate
};

struct Frame {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    cam_pos: vec4<f32>,  // w = 1 for ortho
    cam_dir: vec4<f32>,
    bg_top: vec4<f32>,
    bg_bottom: vec4<f32>,
    mode: vec4<f32>,     // x lighting (0 default, 1 all lights, 2 flat, 3 none), y light count, z xray, w backface culling
    post: vec4<f32>,     // x exposure (stops), y gamma
    lights: array<Light, 8>,
};

struct Draw {
    model: mat4x4<f32>,
    normal: mat4x4<f32>,
    tint: vec4<f32>,     // rgb, a = mix amount
    params: vec4<f32>,   // x unlit, y alpha, z depth bias, w specular
    params2: vec4<f32>,  // x roughness
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> draw: Draw;

// ---------------------------------------------------------------- background
struct BgOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) t: f32,
};

@vertex
fn vs_bg(@builtin(vertex_index) i: u32) -> BgOut {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    var o: BgOut;
    o.pos = vec4(p[i], 0.0, 1.0);
    o.t = p[i].y * 0.5 + 0.5;
    return o;
}

@fragment
fn fs_bg(i: BgOut) -> @location(0) vec4<f32> {
    return vec4(mix(frame.bg_bottom.rgb, frame.bg_top.rgb, clamp(i.t, 0.0, 1.0)), 1.0);
}

// ---------------------------------------------------------------- meshes
struct MeshIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
};
struct MeshOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
};

@vertex
fn vs_mesh(v: MeshIn) -> MeshOut {
    var o: MeshOut;
    let w = draw.model * vec4(v.pos, 1.0);
    o.world = w.xyz;
    o.clip = frame.view_proj * w;
    o.clip.z = o.clip.z - draw.params.z * o.clip.w;
    o.normal = (draw.normal * vec4(v.normal, 0.0)).xyz;
    o.color = v.color;
    return o;
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3(0.0031308));
}

fn shade(n_in: vec3<f32>, world: vec3<f32>, base: vec3<f32>, front: bool) -> vec3<f32> {
    var n = normalize(n_in);
    var v = normalize(frame.cam_pos.xyz - world);
    if (frame.cam_pos.w > 0.5) {
        v = -frame.cam_dir.xyz;
    }
    // two-sided: light the side that faces the camera (independent of winding / front_facing)
    if (dot(n, v) < 0.0) {
        n = -n;
    }
    let spec_k = draw.params.w;
    let rough = max(draw.params2.x, 0.03);
    let shin = 2.0 / (rough * rough) - 2.0;
    if (frame.mode.x < 0.5) {
        // default lighting: a headlight slightly above the camera
        let l = normalize(v + vec3(0.0, 0.15, 0.0));
        let d = max(dot(n, l), 0.0);
        let h = normalize(l + v);
        let s = spec_k * pow(max(dot(n, h), 0.0), max(shin, 1.0)) * 0.5;
        return base * d + vec3(s);
    }
    if (frame.mode.x > 2.5) {
        return base * 0.06;
    }
    if (frame.mode.x > 1.5) {
        return base;
    }
    var c = vec3(0.0);
    let count = i32(frame.mode.y);
    for (var i = 0; i < count; i = i + 1) {
        let L = frame.lights[i];
        let ty = L.pos.w;
        if (ty < 0.5) {
            c = c + base * L.color.rgb;
            continue;
        }
        var l: vec3<f32>;
        var att = 1.0;
        if (ty < 1.5) {
            l = -normalize(L.dir.xyz);
        } else {
            let d = L.pos.xyz - world;
            let dist = length(d);
            l = d / max(dist, 1e-5);
            if (L.params.z > 0.5) {
                att = 1.0 / pow(max(dist, 0.01), L.params.z);
            }
            if (ty > 2.5 && ty < 3.5) {
                let cd = dot(-l, normalize(L.dir.xyz));
                att = att * smoothstep(L.params.x, L.params.y, cd);
            }
            if (ty > 3.5) {
                att = att * max(dot(-l, normalize(L.dir.xyz)), 0.0);
            }
        }
        let d = max(dot(n, l), 0.0);
        let h = normalize(l + v);
        let s = spec_k * pow(max(dot(n, h), 0.0), max(shin, 1.0)) * 0.5;
        c = c + (base * d + vec3(s) * step(0.0001, d)) * L.color.rgb * att;
    }
    return c;
}

@fragment
fn fs_mesh(i: MeshOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    if (frame.mode.w > 0.5 && !front) {
        discard;
    }
    let base = mix(i.color, draw.tint.rgb, draw.tint.a);
    var c = base;
    if (draw.params.x < 0.5) {
        // lit surfaces: material colours are linear and the result is gamma-encoded once, as in the
        // JS (three.js sRGB output); the egui texture path shows these values as they are
        c = shade(i.normal, i.world, base, front);
        c = pow(max(c * exp2(frame.post.x), vec3(0.0)), vec3(1.0 / frame.post.y));
        c = linear_to_srgb(c);
    } else {
        c = pow(max(c * exp2(frame.post.x), vec3(0.0)), vec3(1.0 / frame.post.y));
    }
    return vec4(c, draw.params.y);
}

// ---------------------------------------------------------------- lines
struct LineIn {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
};
struct LineOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_line(v: LineIn) -> LineOut {
    var o: LineOut;
    o.clip = frame.view_proj * draw.model * vec4(v.pos, 1.0);
    o.clip.z = o.clip.z - draw.params.z * o.clip.w;
    o.color = mix(v.color, vec4(draw.tint.rgb, v.color.a), draw.tint.a);
    return o;
}

@fragment
fn fs_line(i: LineOut) -> @location(0) vec4<f32> {
    return i.color;
}
