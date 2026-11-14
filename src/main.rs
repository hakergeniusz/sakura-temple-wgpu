mod world;

use std::{sync::Arc, time::Instant};
use bytemuck::{Pod, Zeroable};
use glam::{
    camera::rh::{
        proj::directx::{orthographic, perspective},
        view::look_at_mat4,
    },
    Vec3,
};
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
use world::{build_dynamic_instances, generate_world, VoxelInstance, WorldData};

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
struct CubeVertex {
    pos: [f32; 3],
    norm: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    light_vp: [[f32; 4]; 4],
    cam_pos: [f32; 4],
    sun_dir: [f32; 4],
    point_lights: [[f32; 4]; 6],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct SimParams {
    dt: f32,
    time: f32,
    count: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct PetalCam {
    view_proj: [[f32; 4]; 4],
    cam_pos: [f32; 4],
}

fn cube_mesh() -> ([CubeVertex; 24], [u16; 36]) {
    let f = |p: [[f32; 3]; 4], n: [f32; 3]| p.map(|pos| CubeVertex { pos, norm: n });
    let mut v = Vec::with_capacity(24);
    v.extend_from_slice(&f([[-0.5,-0.5, 0.5],[ 0.5,-0.5, 0.5],[ 0.5, 0.5, 0.5],[-0.5, 0.5, 0.5]], [ 0.0, 0.0, 1.0]));
    v.extend_from_slice(&f([[ 0.5,-0.5,-0.5],[-0.5,-0.5,-0.5],[-0.5, 0.5,-0.5],[ 0.5, 0.5,-0.5]], [ 0.0, 0.0,-1.0]));
    v.extend_from_slice(&f([[ 0.5,-0.5, 0.5],[ 0.5,-0.5,-0.5],[ 0.5, 0.5,-0.5],[ 0.5, 0.5, 0.5]], [ 1.0, 0.0, 0.0]));
    v.extend_from_slice(&f([[-0.5,-0.5,-0.5],[-0.5,-0.5, 0.5],[-0.5, 0.5, 0.5],[-0.5, 0.5,-0.5]], [-1.0, 0.0, 0.0]));
    v.extend_from_slice(&f([[-0.5, 0.5, 0.5],[ 0.5, 0.5, 0.5],[ 0.5, 0.5,-0.5],[-0.5, 0.5,-0.5]], [ 0.0, 1.0, 0.0]));
    v.extend_from_slice(&f([[-0.5,-0.5,-0.5],[ 0.5,-0.5,-0.5],[ 0.5,-0.5, 0.5],[-0.5,-0.5, 0.5]], [ 0.0,-1.0, 0.0]));
    let mut idx = [0u16; 36];
    for face in 0..6u16 {
        let b = face * 4;
        let o = face as usize * 6;
        idx[o..o + 6].copy_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    (v.try_into().unwrap(), idx)
}

fn catmull_rom_closed(pts: &[Vec3], t: f32) -> Vec3 {
    let n = pts.len();
    let scaled = t.rem_euclid(1.0) * n as f32;
    let i = scaled.floor() as usize % n;
    let u = scaled - scaled.floor();
    let p0 = pts[(i + n - 1) % n];
    let p1 = pts[i];
    let p2 = pts[(i + 1) % n];
    let p3 = pts[(i + 2) % n];
    let u2 = u * u;
    let u3 = u2 * u;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * u
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * u3)
}

struct OrbitCam {
    target: Vec3,
    yaw: f32,
    pitch: f32,
    dist: f32,
    tour: bool,
    tour_t: f32,
    tour_pos_actual: Vec3,
    tour_tgt_actual: Vec3,
}

impl OrbitCam {
    fn new() -> Self {
        let mut c = Self {
            target: Vec3::new(0.0, 22.0, -10.0),
            yaw: 0.74, pitch: 0.34, dist: 198.0,
            tour: false, tour_t: 0.0,
            tour_pos_actual: Vec3::new(128.0, 86.0, 138.0),
            tour_tgt_actual: Vec3::new(0.0, 22.0, -10.0),
        };
        c.tour_pos_actual = c.eye();
        c
    }
    fn eye(&self) -> Vec3 {
        if self.tour { return self.tour_pos_actual; }
        let cp = self.pitch.cos();
        self.target + Vec3::new(self.yaw.sin() * cp, self.pitch.sin(), self.yaw.cos() * cp) * self.dist
    }
    fn set_preset(&mut self, eye: Vec3, tgt: Vec3) {
        self.tour = false;
        self.target = tgt;
        let d = eye - tgt;
        self.dist = d.length().clamp(14.0, 360.0);
        self.pitch = (d.y / self.dist).asin().clamp(0.03, 1.48);
        self.yaw = d.x.atan2(d.z);
    }
}

struct GpuState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_view: wgpu::TextureView,
    shadow_view: wgpu::TextureView,

    sky_pipe: wgpu::RenderPipeline,
    shadow_pipe: wgpu::RenderPipeline,
    opaque_pipe: wgpu::RenderPipeline,
    trans_pipe: wgpu::RenderPipeline,
    petal_compute_pipe: wgpu::ComputePipeline,
    petal_render_pipe: wgpu::RenderPipeline,

    main_bg: wgpu::BindGroup,
    shadow_bg: wgpu::BindGroup,
    petal_comp_bg: wgpu::BindGroup,
    petal_rend_bg: wgpu::BindGroup,

    uniform_buf: wgpu::Buffer,
    sim_buf: wgpu::Buffer,
    petal_cam_buf: wgpu::Buffer,

    cube_vb: wgpu::Buffer,
    cube_ib: wgpu::Buffer,
    static_opaque_buf: wgpu::Buffer,
    static_opaque_len: u32,
    static_shadow_len: u32,
    static_trans_buf: wgpu::Buffer,
    static_trans_len: u32,
    dyn_opaque_buf: wgpu::Buffer,
    dyn_trans_buf: wgpu::Buffer,
    petal_buf: wgpu::Buffer,
    petal_count: u32,

    world: WorldData,
    dyn_opaque_vec: Vec<VoxelInstance>,
    dyn_trans_vec: Vec<VoxelInstance>,
    cam: OrbitCam,
    left_drag: bool,
    right_drag: bool,
    last_cursor: Option<(f64, f64)>,
    start_time: Instant,
    last_frame: Instant,
    fps_accum: f32,
    fps_frames: u32,
}

fn make_depth(device: &wgpu::Device, w: u32, h: u32, label: &str) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    tex.create_view(&Default::default())
}

impl GpuState {
    async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: true,
        }).await.unwrap();

        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: Default::default(),
            trace: wgpu::Trace::Off,
        }).await.unwrap();

        let caps = surface.get_capabilities(&adapter);
        let fmt = caps.formats.iter().find(|f| f.is_srgb()).copied().unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: fmt,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);

        let depth_view = make_depth(&device, config.width, config.height, "depth");
        let shadow_view = make_depth(&device, 2048, 2048, "shadow");
        let shadow_samp = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow_samp"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let world = generate_world();
        let (verts, indices) = cube_mesh();
        let cube_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube_vb"), contents: bytemuck::cast_slice(&verts), usage: wgpu::BufferUsages::VERTEX,
        });
        let cube_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube_ib"), contents: bytemuck::cast_slice(&indices), usage: wgpu::BufferUsages::INDEX,
        });

        let static_opaque_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("static_opaque"), contents: bytemuck::cast_slice(&world.opaque), usage: wgpu::BufferUsages::VERTEX,
        });
        let static_trans_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("static_trans"), contents: bytemuck::cast_slice(&world.translucent_base), usage: wgpu::BufferUsages::VERTEX,
        });
        let dyn_opaque_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dyn_opaque"), size: 4096 * std::mem::size_of::<VoxelInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
        });
        let dyn_trans_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dyn_trans"), size: 4096 * std::mem::size_of::<VoxelInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
        });

        let petal_count = world.petals.len() as u32;
        let petal_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("petal_buf"), contents: bytemuck::cast_slice(&world.petals),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
        });

        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"), size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
        });
        let sim_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sim_params"), size: std::mem::size_of::<SimParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
        });
        let petal_cam_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("petal_cam"), size: std::mem::size_of::<PetalCam>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
        });

        let main_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("main_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0, visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let main_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("main_bg"), layout: &main_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_samp) },
            ],
        });
        // The shadow pass writes the shadow texture (exclusive DEPTH_STENCIL_WRITE),
        // so it must not also bind it for sampling — uniform-only bind group instead.
        let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0, visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let shadow_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_bg"), layout: &shadow_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() }],
        });
        let shadow_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow_pl"), bind_group_layouts: &[Some(&shadow_bgl)], immediate_size: 0,
        });
        let main_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("main_pl"), bind_group_layouts: &[Some(&main_bgl)], immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let v_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CubeVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x3 },
            ],
        };
        let i_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<VoxelInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: 16, shader_location: 3, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 28, shader_location: 4, format: wgpu::VertexFormat::Uint32 },
            ],
        };

        let sky_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"), layout: Some(&main_pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_sky"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader, entry_point: Some("fs_sky"),
                targets: &[Some(wgpu::ColorTargetState { format: fmt, blend: Some(wgpu::BlendState::REPLACE), write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float, depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual), stencil: Default::default(), bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(), multiview_mask: None, cache: None,
        });

        let shadow_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"), layout: Some(&shadow_pl),
            vertex: wgpu::VertexState {
                module: &shader, entry_point: Some("vs_shadow"),
                buffers: &[Some(v_layout.clone()), Some(i_layout.clone())], compilation_options: Default::default(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float, depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less), stencil: Default::default(),
                bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 },
            }),
            multisample: wgpu::MultisampleState::default(), multiview_mask: None, cache: None,
        });

        let opaque_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("opaque"), layout: Some(&main_pl),
            vertex: wgpu::VertexState {
                module: &shader, entry_point: Some("vs_main"),
                buffers: &[Some(v_layout.clone()), Some(i_layout.clone())], compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader, entry_point: Some("fs_opaque"),
                targets: &[Some(wgpu::ColorTargetState { format: fmt, blend: Some(wgpu::BlendState::REPLACE), write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float, depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less), stencil: Default::default(), bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(), multiview_mask: None, cache: None,
        });

        let trans_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("translucent"), layout: Some(&main_pl),
            vertex: wgpu::VertexState {
                module: &shader, entry_point: Some("vs_main"),
                buffers: &[Some(v_layout), Some(i_layout)], compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader, entry_point: Some("fs_translucent"),
                targets: &[Some(wgpu::ColorTargetState { format: fmt, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float, depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Less), stencil: Default::default(), bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(), multiview_mask: None, cache: None,
        });

        let p_shader = device.create_shader_module(wgpu::include_wgsl!("particles.wgsl"));
        let petal_comp_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("petal_comp_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });
        let petal_comp_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("petal_comp_bg"), layout: &petal_comp_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: sim_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: petal_buf.as_entire_binding() },
            ],
        });
        let petal_comp_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("petal_comp_pl"), bind_group_layouts: &[Some(&petal_comp_bgl)], immediate_size: 0,
        });
        let petal_compute_pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("petal_compute"), layout: Some(&petal_comp_pl),
            module: &p_shader, entry_point: Some("cs_petals"), compilation_options: Default::default(), cache: None,
        });

        let petal_rend_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("petal_rend_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0, visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let petal_rend_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("petal_rend_bg"), layout: &petal_rend_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: petal_cam_buf.as_entire_binding() }],
        });
        let petal_rend_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("petal_rend_pl"), bind_group_layouts: &[Some(&petal_rend_bgl)], immediate_size: 0,
        });
        let pv_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CubeVertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 }],
        };
        let pi_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<world::PetalGpu>() as u64, step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: 16, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
            ],
        };
        let petal_render_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("petal_render"), layout: Some(&petal_rend_pl),
            vertex: wgpu::VertexState {
                module: &p_shader, entry_point: Some("vs_petal"),
                buffers: &[Some(pv_layout), Some(pi_layout)], compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &p_shader, entry_point: Some("fs_petal"),
                targets: &[Some(wgpu::ColorTargetState { format: fmt, blend: Some(wgpu::BlendState::REPLACE), write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float, depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less), stencil: Default::default(), bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(), multiview_mask: None, cache: None,
        });

        let static_opaque_len = world.opaque.len() as u32;
        let static_shadow_len = world.shadow_opaque_count;
        let static_trans_len = world.translucent_base.len() as u32;
        window.set_title(&format!("桜の寺村 Grand Sakura Valley ({static_opaque_len} voxels) — [T] Tour · [1-7] Views"));

        Self {
            window, surface, device, queue, config, depth_view, shadow_view,
            sky_pipe, shadow_pipe, opaque_pipe, trans_pipe, petal_compute_pipe, petal_render_pipe,
            main_bg, shadow_bg, petal_comp_bg, petal_rend_bg, uniform_buf, sim_buf, petal_cam_buf,
            cube_vb, cube_ib, static_opaque_buf, static_opaque_len, static_shadow_len,
            static_trans_buf, static_trans_len, dyn_opaque_buf, dyn_trans_buf, petal_buf, petal_count,
            world, dyn_opaque_vec: Vec::with_capacity(512), dyn_trans_vec: Vec::with_capacity(1024),
            cam: OrbitCam::new(), left_drag: false, right_drag: false, last_cursor: None,
            start_time: Instant::now(), last_frame: Instant::now(), fps_accum: 0.0, fps_frames: 0,
        }
    }

    fn resize(&mut self, w: u32, h: u32) {
        if w > 0 && h > 0 {
            self.config.width = w;
            self.config.height = h;
            self.surface.configure(&self.device, &self.config);
            self.depth_view = make_depth(&self.device, w, h, "depth");
        }
    }

    fn render(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;
        let elapsed = (now - self.start_time).as_secs_f32();

        self.fps_frames += 1;
        self.fps_accum += dt;
        if self.fps_accum >= 0.5 {
            let fps = (self.fps_frames as f32 / self.fps_accum).round() as u32;
            self.window.set_title(&format!(
                "桜の寺村 Grand Sakura Valley | {} voxels | {fps} FPS | [T] Tour [1-7] Presets",
                self.static_opaque_len + self.static_trans_len
            ));
            self.fps_frames = 0;
            self.fps_accum = 0.0;
        }

        if self.cam.tour {
            self.cam.tour_t = (self.cam.tour_t + dt / 58.0).fract();
            let pos_pts = [
                Vec3::new(118.0,86.0,130.0), Vec3::new(18.0,34.0,118.0), Vec3::new(0.0,15.0,92.0),
                Vec3::new(0.0,18.5,68.0),    Vec3::new(0.0,14.8,54.0),  Vec3::new(0.0,19.5,28.0),
                Vec3::new(0.0,14.8,9.0),     Vec3::new(0.0,24.5,-5.0),  Vec3::new(20.0,30.0,-20.0),
                Vec3::new(58.0,36.0,-10.0),  Vec3::new(32.0,38.0,-22.0),Vec3::new(-4.0,40.0,-22.0),
                Vec3::new(-16.0,48.0,-22.0), Vec3::new(-58.0,58.0,-26.0),Vec3::new(-66.0,66.0,-54.0),
                Vec3::new(-82.0,68.0,-6.0),  Vec3::new(68.0,86.0,98.0),
            ];
            let look_pts = [
                Vec3::new(0.0,22.0,-10.0), Vec3::new(0.0,15.0,76.0),  Vec3::new(0.0,15.0,58.0),
                Vec3::new(0.0,14.0,42.0),  Vec3::new(0.0,15.0,28.0),  Vec3::new(-10.0,13.0,16.0),
                Vec3::new(0.0,22.0,-18.0), Vec3::new(16.0,26.0,-34.0),Vec3::new(46.0,28.0,-32.0),
                Vec3::new(78.0,18.0,-14.0),Vec3::new(24.0,30.0,-45.0),Vec3::new(-24.0,38.0,-42.0),
                Vec3::new(-36.0,45.0,-45.0),Vec3::new(-36.0,48.0,-45.0),Vec3::new(-48.0,34.0,-36.0),
                Vec3::new(-68.0,22.0,14.0), Vec3::new(0.0,22.0,-10.0),
            ];
            let gp = catmull_rom_closed(&pos_pts, self.cam.tour_t);
            let gl = catmull_rom_closed(&look_pts, self.cam.tour_t);
            let alpha = 1.0 - (-4.5 * dt).exp();
            self.cam.tour_pos_actual = self.cam.tour_pos_actual.lerp(gp, alpha);
            self.cam.tour_tgt_actual = self.cam.tour_tgt_actual.lerp(gl, alpha);
        }

        build_dynamic_instances(&mut self.world, elapsed, dt, &mut self.dyn_opaque_vec, &mut self.dyn_trans_vec);
        if !self.dyn_opaque_vec.is_empty() {
            self.queue.write_buffer(&self.dyn_opaque_buf, 0, bytemuck::cast_slice(&self.dyn_opaque_vec));
        }
        if !self.dyn_trans_vec.is_empty() {
            self.queue.write_buffer(&self.dyn_trans_buf, 0, bytemuck::cast_slice(&self.dyn_trans_vec));
        }

        let eye = self.cam.eye();
        let tgt = if self.cam.tour { self.cam.tour_tgt_actual } else { self.cam.target };
        let aspect = self.config.width as f32 / self.config.height.max(1) as f32;
        let proj = perspective(46.0f32.to_radians(), aspect, 0.8, 850.0);
        let view = look_at_mat4(eye, tgt, Vec3::Y);
        let vp = proj * view;

        let sun_pos = Vec3::new(145.0, 205.0, 125.0);
        let light_proj = orthographic(-165.0, 165.0, -165.0, 165.0, 30.0, 460.0);
        let light_view = look_at_mat4(sun_pos, Vec3::new(0.0, 20.0, -10.0), Vec3::Y);
        let light_vp = light_proj * light_view;
        let sun_dir = (sun_pos - Vec3::new(0.0, 20.0, -10.0)).normalize();

        let uniforms = Uniforms {
            view_proj: vp.to_cols_array_2d(),
            inv_view_proj: vp.inverse().to_cols_array_2d(),
            light_vp: light_vp.to_cols_array_2d(),
            cam_pos: [eye.x, eye.y, eye.z, elapsed],
            sun_dir: [sun_dir.x, sun_dir.y, sun_dir.z, 0.0021],
            point_lights: self.world.point_lights,
        };
        self.queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&uniforms));
        self.queue.write_buffer(&self.sim_buf, 0, bytemuck::bytes_of(&SimParams { dt, time: elapsed, count: self.petal_count, _pad: 0 }));
        self.queue.write_buffer(&self.petal_cam_buf, 0, bytemuck::bytes_of(&PetalCam { view_proj: vp.to_cols_array_2d(), cam_pos: [eye.x, eye.y, eye.z, 0.0] }));

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) => f,
            wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
                drop(f);
                self.resize(self.config.width, self.config.height);
                return;
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.resize(self.config.width, self.config.height);
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return,
        };
        let color_view = frame.texture.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&Default::default());

        // 1. GPU Compute Pass: Falling Sakura Petals
        {
            let mut cpass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("petal_cs"), timestamp_writes: None });
            cpass.set_pipeline(&self.petal_compute_pipe);
            cpass.set_bind_group(0, &self.petal_comp_bg, &[]);
            cpass.dispatch_workgroups(self.petal_count.div_ceil(64), 1, 1);
        }

        // 2. Shadow Map Pass
        {
            let mut spass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow_pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            spass.set_pipeline(&self.shadow_pipe);
            spass.set_bind_group(0, &self.shadow_bg, &[]);
            spass.set_vertex_buffer(0, self.cube_vb.slice(..));
            spass.set_index_buffer(self.cube_ib.slice(..), wgpu::IndexFormat::Uint16);
            spass.set_vertex_buffer(1, self.static_opaque_buf.slice(..));
            spass.draw_indexed(0..36, 0, 0..self.static_shadow_len);
            if !self.dyn_opaque_vec.is_empty() {
                spass.set_vertex_buffer(1, self.dyn_opaque_buf.slice(..));
                spass.draw_indexed(0..36, 0, 0..self.dyn_opaque_vec.len() as u32);
            }
        }

        // 3. Main Render Pass
        {
            let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view, resolve_target: None, depth_slice: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            rpass.set_pipeline(&self.sky_pipe);
            rpass.set_bind_group(0, &self.main_bg, &[]);
            rpass.draw(0..3, 0..1);

            rpass.set_pipeline(&self.opaque_pipe);
            rpass.set_bind_group(0, &self.main_bg, &[]);
            rpass.set_vertex_buffer(0, self.cube_vb.slice(..));
            rpass.set_index_buffer(self.cube_ib.slice(..), wgpu::IndexFormat::Uint16);
            rpass.set_vertex_buffer(1, self.static_opaque_buf.slice(..));
            rpass.draw_indexed(0..36, 0, 0..self.static_opaque_len);
            if !self.dyn_opaque_vec.is_empty() {
                rpass.set_vertex_buffer(1, self.dyn_opaque_buf.slice(..));
                rpass.draw_indexed(0..36, 0, 0..self.dyn_opaque_vec.len() as u32);
            }

            rpass.set_pipeline(&self.petal_render_pipe);
            rpass.set_bind_group(0, &self.petal_rend_bg, &[]);
            rpass.set_vertex_buffer(0, self.cube_vb.slice(..));
            rpass.set_index_buffer(self.cube_ib.slice(..), wgpu::IndexFormat::Uint16);
            rpass.set_vertex_buffer(1, self.petal_buf.slice(..));
            rpass.draw_indexed(0..36, 0, 0..self.petal_count);

            rpass.set_pipeline(&self.trans_pipe);
            rpass.set_bind_group(0, &self.main_bg, &[]);
            rpass.set_vertex_buffer(0, self.cube_vb.slice(..));
            rpass.set_index_buffer(self.cube_ib.slice(..), wgpu::IndexFormat::Uint16);
            rpass.set_vertex_buffer(1, self.static_trans_buf.slice(..));
            rpass.draw_indexed(0..36, 0, 0..self.static_trans_len);
            if !self.dyn_trans_vec.is_empty() {
                rpass.set_vertex_buffer(1, self.dyn_trans_buf.slice(..));
                rpass.draw_indexed(0..36, 0, 0..self.dyn_trans_vec.len() as u32);
            }
        }

        self.queue.submit(Some(enc.finish()));
        self.queue.present(frame);
    }
}

struct App {
    state: Option<GpuState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let attrs = Window::default_attributes()
                .with_title("桜の寺村 Grand Sakura Valley — Loading...")
                .with_inner_size(winit::dpi::LogicalSize::new(1440.0, 900.0));
            let win = Arc::new(event_loop.create_window(attrs).unwrap());
            self.state = Some(pollster::block_on(GpuState::new(win)));
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(s) = self.state.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(sz) => s.resize(sz.width, sz.height),
            WindowEvent::RedrawRequested => s.render(),
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                match button {
                    MouseButton::Left => s.left_drag = pressed,
                    MouseButton::Right => s.right_drag = pressed,
                    _ => {}
                }
                if !pressed { s.last_cursor = None; }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some((lx, ly)) = s.last_cursor {
                    let (dx, dy) = ((position.x - lx) as f32, (position.y - ly) as f32);
                    if s.left_drag {
                        s.cam.tour = false;
                        s.cam.yaw -= dx * 0.0055;
                        s.cam.pitch = (s.cam.pitch + dy * 0.0055).clamp(0.03, 1.48);
                    } else if s.right_drag {
                        s.cam.tour = false;
                        let right = Vec3::new(s.cam.yaw.cos(), 0.0, -s.cam.yaw.sin());
                        let fwd = Vec3::new(s.cam.yaw.sin(), 0.0, s.cam.yaw.cos());
                        let pan = s.cam.dist * 0.0014;
                        s.cam.target -= right * (dx * pan) + fwd * (dy * pan);
                    }
                }
                if s.left_drag || s.right_drag { s.last_cursor = Some((position.x, position.y)); }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let sc = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.02,
                };
                s.cam.tour = false;
                s.cam.dist = (s.cam.dist * (1.0 - sc * 0.1)).clamp(14.0, 360.0);
            }
            WindowEvent::KeyboardInput { event: KeyEvent { physical_key: PhysicalKey::Code(code), state: ElementState::Pressed, .. }, .. } => {
                match code {
                    KeyCode::KeyT | KeyCode::Space => {
                        s.cam.tour = !s.cam.tour;
                        if s.cam.tour {
                            s.cam.tour_pos_actual = s.cam.eye();
                            s.cam.tour_tgt_actual = s.cam.target;
                        }
                    }
                    KeyCode::Digit1 => s.cam.set_preset(Vec3::new(128.0, 86.0, 138.0), Vec3::new(0.0, 22.0, -10.0)),
                    KeyCode::Digit2 => s.cam.set_preset(Vec3::new(34.0, 28.0, 50.0), Vec3::new(0.0, 14.0, 28.0)),
                    KeyCode::Digit3 => s.cam.set_preset(Vec3::new(-70.0, 22.0, 34.0), Vec3::new(-48.0, 12.0, 11.0)),
                    KeyCode::Digit4 => s.cam.set_preset(Vec3::new(110.0, 38.0, 38.0), Vec3::new(76.0, 20.0, -14.0)),
                    KeyCode::Digit5 => s.cam.set_preset(Vec3::new(-54.0, 32.0, 6.0), Vec3::new(-80.0, 20.0, -28.0)),
                    KeyCode::Digit6 => s.cam.set_preset(Vec3::new(4.0, 34.0, 4.0), Vec3::new(10.0, 26.0, -36.0)),
                    KeyCode::Digit7 => s.cam.set_preset(Vec3::new(-10.0, 50.0, -14.0), Vec3::new(-36.0, 44.0, -45.0)),
                    KeyCode::Escape => event_loop.exit(),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(s) = &self.state { s.window.request_redraw(); }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = App { state: None };
    event_loop.run_app(&mut app).unwrap();
}
