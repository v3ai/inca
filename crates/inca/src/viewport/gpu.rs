//! wgpu renderer for the viewports: each viewport renders offscreen (MSAA + depth) into a
//! texture that egui shows as an image.

use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::{self, wgpu};
use std::collections::HashMap;

pub const MSAA: u32 = 4;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const DRAW_SLOT: u64 = 256;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, Default)]
pub struct MeshVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, Default)]
pub struct LineVertex {
    pub pos: [f32; 3],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuLight {
    pub pos: [f32; 4],
    pub dir: [f32; 4],
    pub color: [f32; 4],
    pub params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct FrameUniform {
    pub view_proj: [[f32; 4]; 4],
    pub view: [[f32; 4]; 4],
    pub cam_pos: [f32; 4],
    pub cam_dir: [f32; 4],
    pub bg_top: [f32; 4],
    pub bg_bottom: [f32; 4],
    pub mode: [f32; 4],
    pub post: [f32; 4],
    pub lights: [GpuLight; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct DrawUniform {
    pub model: [[f32; 4]; 4],
    pub normal: [[f32; 4]; 4],
    pub tint: [f32; 4],
    pub params: [f32; 4],
    pub params2: [f32; 4],
}

impl DrawUniform {
    pub fn new(model: glam::Mat4) -> Self {
        let n = model.inverse().transpose();
        DrawUniform { model: model.to_cols_array_2d(), normal: n.to_cols_array_2d(), tint: [0.0; 4], params: [0.0, 1.0, 0.0, 0.0], params2: [0.5, 0.0, 0.0, 0.0] }
    }
    pub fn tint(mut self, c: [f32; 3], a: f32) -> Self {
        self.tint = [c[0], c[1], c[2], a];
        self
    }
    pub fn unlit(mut self) -> Self {
        self.params[0] = 1.0;
        self
    }
    pub fn alpha(mut self, a: f32) -> Self {
        self.params[1] = a;
        self
    }
    pub fn bias(mut self, b: f32) -> Self {
        self.params[2] = b;
        self
    }
    pub fn spec(mut self, s: f32, rough: f32) -> Self {
        self.params[3] = s;
        self.params2[0] = rough;
        self
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pass {
    /// lines drawn before any geometry, without depth writes (the grid)
    LinesUnder,
    /// opaque shaded triangles
    Mesh,
    /// transparent triangles (x-ray, selected-face overlay)
    MeshBlend,
    /// depth-tested lines
    Lines,
    /// lines drawn on top of everything
    LinesTop,
}

pub enum Geo {
    /// cached mesh triangles by key
    MeshTris(String),
    /// cached mesh wireframe by key
    MeshWire(String),
    TransientTris(Vec<MeshVertex>),
    TransientLines(Vec<LineVertex>),
}

pub struct DrawCmd {
    pub pass: Pass,
    pub geo: Geo,
    pub u: DrawUniform,
}

pub struct GpuMesh {
    pub key: u64,
    tris: Option<wgpu::Buffer>,
    tri_count: u32,
    wire: Option<wgpu::Buffer>,
    wire_count: u32,
}

struct Target {
    size: [u32; 2],
    _color: wgpu::Texture,
    color_view: wgpu::TextureView,
    msaa_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    egui_id: egui::TextureId,
    frame_buf: wgpu::Buffer,
    frame_bg: wgpu::BindGroup,
}

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    frame_bgl: wgpu::BindGroupLayout,
    draw_bgl: wgpu::BindGroupLayout,
    bg_pipe: wgpu::RenderPipeline,
    mesh_pipe: wgpu::RenderPipeline,
    blend_pipe: wgpu::RenderPipeline,
    line_pipe: wgpu::RenderPipeline,
    line_top_pipe: wgpu::RenderPipeline,
    draw_buf: wgpu::Buffer,
    draw_cap: u64,
    draw_bg: wgpu::BindGroup,
    meshes: HashMap<String, GpuMesh>,
    targets: HashMap<usize, Target>,
}

fn vb_layout<'a>(stride: u64, attrs: &'a [wgpu::VertexAttribute]) -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout { array_stride: stride, step_mode: wgpu::VertexStepMode::Vertex, attributes: attrs }
}

impl Gpu {
    pub fn new(rs: &egui_wgpu::RenderState) -> Self {
        let device = rs.device.clone();
        let queue = rs.queue.clone();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("inca viewport"), source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()) });
        let frame_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let draw_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("draw"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<DrawUniform>() as u64) },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("inca"), bind_group_layouts: &[Some(&frame_bgl), Some(&draw_bgl)], immediate_size: 0 });
        let mesh_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];
        let line_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];
        let ms = wgpu::MultisampleState { count: MSAA, mask: !0, alpha_to_coverage_enabled: false };
        let depth = |write: bool, compare: wgpu::CompareFunction| {
            Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: Some(write), depth_compare: Some(compare), stencil: Default::default(), bias: Default::default() })
        };
        let target = |blend: Option<wgpu::BlendState>| [Some(wgpu::ColorTargetState { format: COLOR_FORMAT, blend, write_mask: wgpu::ColorWrites::ALL })];
        let alpha = Some(wgpu::BlendState::ALPHA_BLENDING);
        let mk = |label: &str, vs: &str, fs: &str, buffers: &[Option<wgpu::VertexBufferLayout>], topo: wgpu::PrimitiveTopology, ds: Option<wgpu::DepthStencilState>, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some(vs), compilation_options: Default::default(), buffers },
                primitive: wgpu::PrimitiveState { topology: topo, cull_mode: None, ..Default::default() },
                depth_stencil: ds,
                multisample: ms,
                fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some(fs), compilation_options: Default::default(), targets: &target(blend) }),
                multiview_mask: None,
                cache: None,
            })
        };
        let mesh_vb = [Some(vb_layout(std::mem::size_of::<MeshVertex>() as u64, &mesh_attrs))];
        let line_vb = [Some(vb_layout(std::mem::size_of::<LineVertex>() as u64, &line_attrs))];
        let bg_pipe = mk("bg", "vs_bg", "fs_bg", &[], wgpu::PrimitiveTopology::TriangleList, depth(false, wgpu::CompareFunction::Always), None);
        let mesh_pipe = mk("mesh", "vs_mesh", "fs_mesh", &mesh_vb, wgpu::PrimitiveTopology::TriangleList, depth(true, wgpu::CompareFunction::LessEqual), None);
        let blend_pipe = mk("blend", "vs_mesh", "fs_mesh", &mesh_vb, wgpu::PrimitiveTopology::TriangleList, depth(false, wgpu::CompareFunction::LessEqual), alpha);
        let line_pipe = mk("line", "vs_line", "fs_line", &line_vb, wgpu::PrimitiveTopology::LineList, depth(true, wgpu::CompareFunction::LessEqual), alpha);
        let line_top_pipe = mk("line top", "vs_line", "fs_line", &line_vb, wgpu::PrimitiveTopology::LineList, depth(false, wgpu::CompareFunction::Always), alpha);
        let draw_cap = 256;
        let (draw_buf, draw_bg) = Self::make_draw_buf(&device, &draw_bgl, draw_cap);
        Gpu { device, queue, frame_bgl, draw_bgl, bg_pipe, mesh_pipe, blend_pipe, line_pipe, line_top_pipe, draw_buf, draw_cap, draw_bg, meshes: HashMap::new(), targets: HashMap::new() }
    }

    fn make_draw_buf(device: &wgpu::Device, bgl: &wgpu::BindGroupLayout, cap: u64) -> (wgpu::Buffer, wgpu::BindGroup) {
        let buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("draws"), size: cap * DRAW_SLOT, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draws"),
            layout: bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &buf, offset: 0, size: wgpu::BufferSize::new(std::mem::size_of::<DrawUniform>() as u64) }),
            }],
        });
        (buf, bg)
    }

    fn vbuf(&self, data: &[u8]) -> Option<wgpu::Buffer> {
        if data.is_empty() {
            return None;
        }
        use wgpu::util::DeviceExt;
        Some(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: data, usage: wgpu::BufferUsages::VERTEX }))
    }

    pub fn mesh_key(&self, name: &str) -> Option<u64> {
        self.meshes.get(name).map(|m| m.key)
    }

    pub fn upload_mesh(&mut self, name: &str, key: u64, tris: &[MeshVertex], wire: &[LineVertex]) {
        let m = GpuMesh { key, tris: self.vbuf(bytemuck::cast_slice(tris)), tri_count: tris.len() as u32, wire: self.vbuf(bytemuck::cast_slice(wire)), wire_count: wire.len() as u32 };
        self.meshes.insert(name.to_string(), m);
    }

    pub fn retain_meshes(&mut self, keep: &std::collections::HashSet<String>) {
        self.meshes.retain(|k, _| keep.contains(k));
    }

    /// Make sure viewport `idx` has a render target of `size` pixels; returns its egui texture.
    pub fn target(&mut self, renderer: &mut egui_wgpu::Renderer, idx: usize, size: [u32; 2]) -> egui::TextureId {
        let size = [size[0].clamp(1, 8192), size[1].clamp(1, 8192)];
        if let Some(t) = self.targets.get(&idx) {
            if t.size == size {
                return t.egui_id;
            }
        }
        let mut t = self.make_target(size);
        t.egui_id = match self.targets.get(&idx) {
            Some(old) => {
                renderer.update_egui_texture_from_wgpu_texture(&self.device, &t.color_view, wgpu::FilterMode::Linear, old.egui_id);
                old.egui_id
            }
            None => renderer.register_native_texture(&self.device, &t.color_view, wgpu::FilterMode::Linear),
        };
        let id = t.egui_id;
        self.targets.insert(idx, t);
        id
    }

    fn make_target(&self, size: [u32; 2]) -> Target {
        let ext = wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 };
        let tex = |label: &str, format: wgpu::TextureFormat, samples: u32, usage: wgpu::TextureUsages| {
            self.device.create_texture(&wgpu::TextureDescriptor { label: Some(label), size: ext, mip_level_count: 1, sample_count: samples, dimension: wgpu::TextureDimension::D2, format, usage, view_formats: &[] })
        };
        let color = tex("vp color", COLOR_FORMAT, 1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC);
        let msaa = tex("vp msaa", COLOR_FORMAT, MSAA, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let depth = tex("vp depth", DEPTH_FORMAT, MSAA, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let color_view = color.create_view(&Default::default());
        let frame_buf = self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("frame"), size: std::mem::size_of::<FrameUniform>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let frame_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("frame"), layout: &self.frame_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: frame_buf.as_entire_binding() }] });
        Target { size, msaa_view: msaa.create_view(&Default::default()), depth_view: depth.create_view(&Default::default()), color_view, _color: color, egui_id: egui::TextureId::default(), frame_buf, frame_bg }
    }

    /// Render into a throwaway target and read the pixels back (sRGB RGBA, row-major): the
    /// "Inca Hardware" renderer.
    pub fn render_offscreen(&mut self, size: [u32; 2], frame: &FrameUniform, draws: &[DrawCmd]) -> Vec<u8> {
        let size = [size[0].clamp(1, 8192), size[1].clamp(1, 8192)];
        self.reserve_draws(draws.len() as u64);
        let t = self.make_target(size);
        self.draw_pass(&t, frame, draws);
        let row = (size[0] * 4).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("readback"), size: (row * size[1]) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("readback") });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &t._color, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(size[1]) } },
            wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let Ok(data) = slice.get_mapped_range() else { return vec![] };
        // the target stores sRGB-encoded bytes of what the shader wrote, and the shader already
        // gamma-encodes (the viewport shows the decoded values): undo the storage encoding once
        let lut: Vec<u8> = (0..256).map(|i| {
            let x = i as f32 / 255.0;
            let l = if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) };
            (l * 255.0 + 0.5) as u8
        }).collect();
        let mut out = Vec::with_capacity((size[0] * size[1] * 4) as usize);
        for y in 0..size[1] {
            let r = (y * row) as usize;
            for px in data[r..r + (size[0] * 4) as usize].chunks(4) {
                out.extend_from_slice(&[lut[px[0] as usize], lut[px[1] as usize], lut[px[2] as usize], px[3]]);
            }
        }
        drop(data);
        buf.unmap();
        out
    }

    fn reserve_draws(&mut self, n: u64) {
        if n > self.draw_cap {
            self.draw_cap = n.next_power_of_two();
            let (b, g) = Self::make_draw_buf(&self.device, &self.draw_bgl, self.draw_cap);
            self.draw_buf = b;
            self.draw_bg = g;
        }
    }

    /// Render one viewport.
    pub fn render(&mut self, idx: usize, frame: &FrameUniform, draws: &[DrawCmd]) {
        self.reserve_draws(draws.len() as u64);
        let Some(t) = self.targets.get(&idx) else { return };
        self.draw_pass(t, frame, draws);
    }

    fn draw_pass(&self, t: &Target, frame: &FrameUniform, draws: &[DrawCmd]) {
        let n = draws.len() as u64;
        self.queue.write_buffer(&t.frame_buf, 0, bytemuck::bytes_of(frame));
        let mut slots = vec![0u8; (n.max(1) * DRAW_SLOT) as usize];
        for (i, d) in draws.iter().enumerate() {
            let b = bytemuck::bytes_of(&d.u);
            slots[i * DRAW_SLOT as usize..i * DRAW_SLOT as usize + b.len()].copy_from_slice(b);
        }
        self.queue.write_buffer(&self.draw_buf, 0, &slots);
        // transient geometry packed into two buffers
        let mut ttris: Vec<MeshVertex> = vec![];
        let mut tlines: Vec<LineVertex> = vec![];
        let mut ranges = vec![(0u32, 0u32); draws.len()];
        for (i, d) in draws.iter().enumerate() {
            match &d.geo {
                Geo::TransientTris(v) => {
                    ranges[i] = (ttris.len() as u32, v.len() as u32);
                    ttris.extend_from_slice(v);
                }
                Geo::TransientLines(v) => {
                    ranges[i] = (tlines.len() as u32, v.len() as u32);
                    tlines.extend_from_slice(v);
                }
                _ => {}
            }
        }
        let tri_buf = self.vbuf(bytemuck::cast_slice(&ttris));
        let line_buf = self.vbuf(bytemuck::cast_slice(&tlines));
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("viewport") });
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewport"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &t.msaa_view,
                    depth_slice: None,
                    resolve_target: Some(&t.color_view),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Discard },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_bind_group(0, &t.frame_bg, &[]);
            rp.set_bind_group(1, &self.draw_bg, &[0]);
            rp.set_pipeline(&self.bg_pipe);
            rp.draw(0..3, 0..1);
            for pass in [Pass::LinesUnder, Pass::Mesh, Pass::Lines, Pass::MeshBlend, Pass::LinesTop] {
                rp.set_pipeline(match pass {
                    Pass::LinesUnder => &self.line_top_pipe,
                    Pass::Mesh => &self.mesh_pipe,
                    Pass::MeshBlend => &self.blend_pipe,
                    Pass::Lines => &self.line_pipe,
                    Pass::LinesTop => &self.line_top_pipe,
                });
                for (i, d) in draws.iter().enumerate() {
                    if d.pass != pass {
                        continue;
                    }
                    let (buf, start, count) = match &d.geo {
                        Geo::MeshTris(k) => match self.meshes.get(k) {
                            Some(GpuMesh { tris: Some(b), tri_count, .. }) => (b, 0, *tri_count),
                            _ => continue,
                        },
                        Geo::MeshWire(k) => match self.meshes.get(k) {
                            Some(GpuMesh { wire: Some(b), wire_count, .. }) => (b, 0, *wire_count),
                            _ => continue,
                        },
                        Geo::TransientTris(_) => match &tri_buf {
                            Some(b) => (b, ranges[i].0, ranges[i].1),
                            None => continue,
                        },
                        Geo::TransientLines(_) => match &line_buf {
                            Some(b) => (b, ranges[i].0, ranges[i].1),
                            None => continue,
                        },
                    };
                    if count == 0 {
                        continue;
                    }
                    rp.set_bind_group(1, &self.draw_bg, &[(i as u64 * DRAW_SLOT) as u32]);
                    rp.set_vertex_buffer(0, buf.slice(..));
                    rp.draw(start..start + count, 0..1);
                }
            }
        }
        self.queue.submit([enc.finish()]);
    }
}
