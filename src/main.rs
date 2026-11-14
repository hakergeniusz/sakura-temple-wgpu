mod builders;
mod terrain;
mod voxel;
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

use voxel::{CatmullRomCurve3, ModelInstance, Rng, VoxelVertex};
use world::{
    make_tour_curves, DrawBatch, TimeOfDay, WorldScene, NIGHT_LIGHTS, PRESETS,
};

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
struct CubeVertex {
    pos: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct PetalGpu {
    pos_spin: [f32; 4],
    vel_col: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    light_vp: [[f32; 4]; 4],
    cam_pos: [f32; 4],
    sun_dir: [f32; 4],
    sky_top: [f32; 4],
    sky_mid: [f32; 4],
    sky_bot: [f32; 4],
    fog_col: [f32; 4],
    sun_col: [f32; 4],
    hemi_sky: [f32; 4],
    hemi_gnd: [f32; 4],
    point_lights: [[f32; 4]; 12],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct SimParams {
    dt: f32,
    time: f32,
    count: u32,
    paused: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct PetalCam {
    view_proj: [[f32; 4]; 4],
    cam_pos: [f32; 4],
    fog_col: [f32; 4],
}

fn cube_mesh() -> ([CubeVertex; 24], [u16; 36]) {
    let f = |p: [[f32; 3]; 4]| p.map(|pos| CubeVertex { pos });
    let mut v = Vec::with_capacity(24);
    v.extend_from_slice(&f([[-0.5,-0.5, 0.5],[ 0.5,-0.5, 0.5],[ 0.5, 0.5, 0.5],[-0.5, 0.5, 0.5]]));
    v.extend_from_slice(&f([[ 0.5,-0.5,-0.5],[-0.5,-0.5,-0.5],[-0.5, 0.5,-0.5],[ 0.5, 0.5,-0.5]]));
    v.extend_from_slice(&f([[ 0.5,-0.5, 0.5],[ 0.5,-0.5,-0.5],[ 0.5, 0.5,-0.5],[ 0.5, 0.5, 0.5]]));
    v.extend_from_slice(&f([[-0.5,-0.5,-0.5],[-0.5,-0.5, 0.5],[-0.5, 0.5, 0.5],[-0.5, 0.5,-0.5]]));
    v.extend_from_slice(&f([[-0.5, 0.5, 0.5],[ 0.5, 0.5, 0.5],[ 0.5, 0.5,-0.5],[-0.5, 0.5,-0.5]]));
    v.extend_from_slice(&f([[-0.5,-0.5,-0.5],[ 0.5,-0.5,-0.5],[ 0.5,-0.5, 0.5],[-0.5,-0.5, 0.5]]));
    let mut idx = [0u16; 36];
    for face in 0..6u16 {
        let b = face * 4;
        let o = face as usize * 6;
        idx[o..o + 6].copy_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    (v.try_into().unwrap(), idx)
}

fn make_petals() -> Vec<PetalGpu> {
    let mut rng = Rng(991177);
    let count = 2800;
    let mut petals = Vec::with_capacity(count);
    for i in 0..count {
        petals.push(PetalGpu {
            pos_spin: [
                (rng.next_f32() - 0.5) * 400.0,
                10.0 + rng.next_f32() * 65.0,
                (rng.next_f32() - 0.5) * 400.0,
                rng.next_f32() * std::f32::consts::TAU,
            ],
            vel_col: [
                1.4 + rng.next_f32() * 1.4,
                0.7 + rng.next_f32() * 1.1,
                rng.next_f32() * 10.0,
                (i % 4) as f32,
            ],
        });
    }
    petals
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
    preset_name: &'static str,
    tour_pos_curve: CatmullRomCurve3,
    tour_look_curve: CatmullRomCurve3,
}

impl OrbitCam {
    fn new() -> Self {
        let (tour_pos_curve, tour_look_curve) = make_tour_curves();
        let mut c = Self {
            target: PRESETS[0].target,
            yaw: 0.0,
            pitch: 0.0,
            dist: 200.0,
            tour: false,
            tour_t: 0.0,
            tour_pos_actual: PRESETS[0].pos,
            tour_tgt_actual: PRESETS[0].target,
            preset_name: PRESETS[0].name,
            tour_pos_curve,
            tour_look_curve,
        };
        c.set_preset(0);
        c
    }

    fn eye(&self) -> Vec3 {
        if self.tour {
            return self.tour_pos_actual;
        }
        let cp = self.pitch.cos();
        self.target + Vec3::new(self.yaw.sin() * cp, self.pitch.sin(), self.yaw.cos() * cp) * self.dist
    }

    fn set_preset(&mut self, idx: usize) {
        let p = &PRESETS[idx.min(PRESETS.len() - 1)];
        self.tour = false;
        self.preset_name = p.name;
        self.target = p.target;
        let d = p.pos - p.target;
        self.dist = d.length().clamp(6.0, 460.0);
        self.pitch = (d.y / self.dist).asin().clamp(0.02, 1.48);
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

    world_vb: wgpu::Buffer,
    world_ib: wgpu::Buffer,
    instance_buf: wgpu::Buffer,
    cube_vb: wgpu::Buffer,
    cube_ib: wgpu::Buffer,
    petal_buf: wgpu::Buffer,
    petal_count: u32,

    scene: WorldScene,
    dyn_instances: Vec<ModelInstance>,

    cam: OrbitCam,
    time_of_day: TimeOfDay,
    paused: bool,
    sim_time: f32,

    left_drag: bool,
    right_drag: bool,
    last_cursor: Option<(f64, f64)>,
    last_frame: Instant,
    fps_accum: f32,
    fps_frames: u32,
}

fn make_depth(device: &wgpu::Device, w: u32, h: u32, label: &str) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
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
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: true,
            })
            .await
            .unwrap();

        let adapter_limits = adapter.limits();
        let mut limits = wgpu::Limits::default();
        limits.max_buffer_size = adapter_limits.max_buffer_size;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .unwrap();

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

        let scene = WorldScene::build();
        println!(
            "Built 桜の帝都村: {} micro-voxels, {} tris, {} batches ({} verts, {} indices)",
            scene.total_voxels,
            scene.total_tris,
            scene.total_draws,
            scene.vertices.len(),
            scene.indices.len()
        );

        let world_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("world_vb"),
            contents: bytemuck::cast_slice(&scene.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let world_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("world_ib"),
            contents: bytemuck::cast_slice(&scene.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let mut all_instances = scene.static_instances.clone();
        let mut dyn_instances = Vec::with_capacity(scene.dynamic_inst_count as usize);
        scene.update_dynamic_instances(0.0, &mut dyn_instances);
        all_instances.extend_from_slice(&dyn_instances);

        let instance_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("instance_buf"),
            contents: bytemuck::cast_slice(&all_instances),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let (cverts, cindices) = cube_mesh();
        let cube_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube_vb"),
            contents: bytemuck::cast_slice(&cverts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let cube_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube_ib"),
            contents: bytemuck::cast_slice(&cindices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let petals = make_petals();
        let petal_count = petals.len() as u32;
        let petal_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("petal_buf"),
            contents: bytemuck::cast_slice(&petals),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
        });

        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sim_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sim_params"),
            size: std::mem::size_of::<SimParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let petal_cam_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("petal_cam"),
            size: std::mem::size_of::<PetalCam>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let main_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("main_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let main_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("main_bg"),
            layout: &main_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_samp) },
            ],
        });

        let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let shadow_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_bg"),
            layout: &shadow_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() }],
        });

        let shadow_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow_pl"),
            bind_group_layouts: &[Some(&shadow_bgl)],
            immediate_size: 0,
        });
        let main_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("main_pl"),
            bind_group_layouts: &[Some(&main_bgl)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let v_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<VoxelVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 24, shader_location: 2, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 36, shader_location: 3, format: wgpu::VertexFormat::Uint32 },
            ],
        };
        let i_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ModelInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 4, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: 16, shader_location: 5, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: 32, shader_location: 6, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: 48, shader_location: 7, format: wgpu::VertexFormat::Float32x4 },
            ],
        };

        let sky_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&main_pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_sky"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_sky"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: fmt,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let shadow_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&shadow_pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_shadow"),
                buffers: &[Some(v_layout.clone()), Some(i_layout.clone())],
                compilation_options: Default::default(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let opaque_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("opaque"),
            layout: Some(&main_pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(v_layout.clone()), Some(i_layout.clone())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_opaque"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: fmt,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let trans_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("translucent"),
            layout: Some(&main_pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(v_layout), Some(i_layout)],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_translucent"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: fmt,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let p_shader = device.create_shader_module(wgpu::include_wgsl!("particles.wgsl"));
        let petal_comp_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("petal_comp_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let petal_comp_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("petal_comp_bg"),
            layout: &petal_comp_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: sim_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: petal_buf.as_entire_binding() },
            ],
        });
        let petal_comp_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("petal_comp_pl"),
            bind_group_layouts: &[Some(&petal_comp_bgl)],
            immediate_size: 0,
        });
        let petal_compute_pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("petal_compute"),
            layout: Some(&petal_comp_pl),
            module: &p_shader,
            entry_point: Some("cs_petals"),
            compilation_options: Default::default(),
            cache: None,
        });

        let petal_rend_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("petal_rend_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let petal_rend_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("petal_rend_bg"),
            layout: &petal_rend_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: petal_cam_buf.as_entire_binding() }],
        });
        let petal_rend_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("petal_rend_pl"),
            bind_group_layouts: &[Some(&petal_rend_bgl)],
            immediate_size: 0,
        });
        let pv_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CubeVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        };
        let pi_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PetalGpu>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: 16, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
            ],
        };
        let petal_render_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("petal_render"),
            layout: Some(&petal_rend_pl),
            vertex: wgpu::VertexState {
                module: &p_shader,
                entry_point: Some("vs_petal"),
                buffers: &[Some(pv_layout), Some(pi_layout)],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &p_shader,
                entry_point: Some("fs_petal"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: fmt,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let cam = OrbitCam::new();
        window.set_title(&format!(
            "桜の帝都村 · Micro-Voxel Sakura Empire ({} voxels · {} tris) — [1-9] Views [T] Tour [N] Time [P] Pause",
            scene.total_voxels, scene.total_tris
        ));

        Self {
            window,
            surface,
            device,
            queue,
            config,
            depth_view,
            shadow_view,
            sky_pipe,
            shadow_pipe,
            opaque_pipe,
            trans_pipe,
            petal_compute_pipe,
            petal_render_pipe,
            main_bg,
            shadow_bg,
            petal_comp_bg,
            petal_rend_bg,
            uniform_buf,
            sim_buf,
            petal_cam_buf,
            world_vb,
            world_ib,
            instance_buf,
            cube_vb,
            cube_ib,
            petal_buf,
            petal_count,
            scene,
            dyn_instances,
            cam,
            time_of_day: TimeOfDay::Day,
            paused: false,
            sim_time: 0.0,
            left_drag: false,
            right_drag: false,
            last_cursor: None,
            last_frame: Instant::now(),
            fps_accum: 0.0,
            fps_frames: 0,
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

    fn draw_batch<'a>(rpass: &mut wgpu::RenderPass<'a>, b: &DrawBatch) {
        if b.idx_count > 0 && b.inst_count > 0 {
            rpass.draw_indexed(
                b.idx_start..(b.idx_start + b.idx_count),
                b.base_vertex,
                b.inst_start..(b.inst_start + b.inst_count),
            );
        }
    }

    fn render(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;
        if !self.paused {
            self.sim_time += dt;
        }
        let t = self.sim_time;

        self.fps_frames += 1;
        self.fps_accum += dt;
        if self.fps_accum >= 0.5 {
            let fps = (self.fps_frames as f32 / self.fps_accum).round() as u32;
            let mode_str = self.time_of_day.name();
            let view_str = if self.cam.tour { "Tour (75s)" } else { self.cam.preset_name };
            let pause_str = if self.paused { " | PAUSED" } else { "" };
            self.window.set_title(&format!(
                "桜の帝都村 | {mode_str} | {view_str}{pause_str} | {} tris · {} draws · {fps} FPS | [1-9] [T] Tour [N] Time [P] Pause",
                self.scene.total_tris, self.scene.total_draws
            ));
            self.fps_frames = 0;
            self.fps_accum = 0.0;
        }

        if self.cam.tour {
            self.cam.tour_t = (self.cam.tour_t + dt / 75.0).fract();
            let gp = self.cam.tour_pos_curve.get_point_at(self.cam.tour_t);
            let gl = self.cam.tour_look_curve.get_point_at(self.cam.tour_t);
            let alpha = 1.0 - (-4.5 * dt).exp();
            self.cam.tour_pos_actual = self.cam.tour_pos_actual.lerp(gp, alpha);
            self.cam.tour_tgt_actual = self.cam.tour_tgt_actual.lerp(gl, alpha);
        }

        if !self.paused {
            self.scene.update_dynamic_instances(t, &mut self.dyn_instances);
            let byte_offset =
                (self.scene.dynamic_inst_start as usize * std::mem::size_of::<ModelInstance>()) as u64;
            self.queue.write_buffer(
                &self.instance_buf,
                byte_offset,
                bytemuck::cast_slice(&self.dyn_instances),
            );
        }

        let eye = self.cam.eye();
        let tgt = if self.cam.tour { self.cam.tour_tgt_actual } else { self.cam.target };
        let aspect = self.config.width as f32 / self.config.height.max(1) as f32;
        let proj = perspective(44.0f32.to_radians(), aspect, 0.8, 1300.0);
        let view = look_at_mat4(eye, tgt, Vec3::Y);
        let vp = proj * view;

        let tod = self.time_of_day.params();
        let light_target = Vec3::new(0.0, 18.0, -10.0);
        let light_proj = orthographic(-240.0, 240.0, -240.0, 240.0, 20.0, 650.0);
        let light_view = look_at_mat4(tod.sun_pos, light_target, Vec3::Y);
        let light_vp = light_proj * light_view;
        let sun_dir = (tod.sun_pos - light_target).normalize();

        let uniforms = Uniforms {
            view_proj: vp.to_cols_array_2d(),
            inv_view_proj: vp.inverse().to_cols_array_2d(),
            light_vp: light_vp.to_cols_array_2d(),
            cam_pos: [eye.x, eye.y, eye.z, t],
            sun_dir: [sun_dir.x, sun_dir.y, sun_dir.z, tod.fog_d],
            sky_top: [tod.sky_top[0], tod.sky_top[1], tod.sky_top[2], tod.exposure],
            sky_mid: [tod.sky_mid[0], tod.sky_mid[1], tod.sky_mid[2], tod.sun_int],
            sky_bot: [tod.sky_bot[0], tod.sky_bot[1], tod.sky_bot[2], tod.hemi_int],
            fog_col: [tod.fog_col[0], tod.fog_col[1], tod.fog_col[2], tod.point_int],
            sun_col: [tod.sun_col[0], tod.sun_col[1], tod.sun_col[2], tod.win_emissive],
            hemi_sky: [tod.hemi_sky[0], tod.hemi_sky[1], tod.hemi_sky[2], tod.lamp_emissive],
            hemi_gnd: [tod.hemi_gnd[0], tod.hemi_gnd[1], tod.hemi_gnd[2], 0.0],
            point_lights: NIGHT_LIGHTS,
        };
        self.queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&uniforms));
        self.queue.write_buffer(
            &self.sim_buf,
            0,
            bytemuck::bytes_of(&SimParams {
                dt,
                time: t,
                count: self.petal_count,
                paused: u32::from(self.paused),
            }),
        );
        self.queue.write_buffer(
            &self.petal_cam_buf,
            0,
            bytemuck::bytes_of(&PetalCam {
                view_proj: vp.to_cols_array_2d(),
                cam_pos: [eye.x, eye.y, eye.z, tod.fog_d],
                fog_col: [tod.fog_col[0], tod.fog_col[1], tod.fog_col[2], tod.exposure],
            }),
        );

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
            let mut cpass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("petal_cs"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.petal_compute_pipe);
            cpass.set_bind_group(0, &self.petal_comp_bg, &[]);
            cpass.dispatch_workgroups(self.petal_count.div_ceil(64), 1, 1);
        }

        // 2. Shadow Map Pass (2048x2048)
        {
            let mut spass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow_pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            spass.set_pipeline(&self.shadow_pipe);
            spass.set_bind_group(0, &self.shadow_bg, &[]);
            spass.set_vertex_buffer(0, self.world_vb.slice(..));
            spass.set_vertex_buffer(1, self.instance_buf.slice(..));
            spass.set_index_buffer(self.world_ib.slice(..), wgpu::IndexFormat::Uint32);
            for b in &self.scene.opaque_shadow_batches {
                Self::draw_batch(&mut spass, b);
            }
        }

        // 3. Main Render Pass
        {
            let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            // Sky Dome
            rpass.set_pipeline(&self.sky_pipe);
            rpass.set_bind_group(0, &self.main_bg, &[]);
            rpass.draw(0..3, 0..1);

            // Opaque Greedy-Meshed World & Entities
            rpass.set_pipeline(&self.opaque_pipe);
            rpass.set_bind_group(0, &self.main_bg, &[]);
            rpass.set_vertex_buffer(0, self.world_vb.slice(..));
            rpass.set_vertex_buffer(1, self.instance_buf.slice(..));
            rpass.set_index_buffer(self.world_ib.slice(..), wgpu::IndexFormat::Uint32);
            for b in &self.scene.opaque_shadow_batches {
                Self::draw_batch(&mut rpass, b);
            }

            // GPU-Simulated Sakura Petals
            rpass.set_pipeline(&self.petal_render_pipe);
            rpass.set_bind_group(0, &self.petal_rend_bg, &[]);
            rpass.set_vertex_buffer(0, self.cube_vb.slice(..));
            rpass.set_index_buffer(self.cube_ib.slice(..), wgpu::IndexFormat::Uint16);
            rpass.set_vertex_buffer(1, self.petal_buf.slice(..));
            rpass.draw_indexed(0..36, 0, 0..self.petal_count);

            // Translucent Water, Volumetric Clouds & Waterfall Mist
            rpass.set_pipeline(&self.trans_pipe);
            rpass.set_bind_group(0, &self.main_bg, &[]);
            rpass.set_vertex_buffer(0, self.world_vb.slice(..));
            rpass.set_vertex_buffer(1, self.instance_buf.slice(..));
            rpass.set_index_buffer(self.world_ib.slice(..), wgpu::IndexFormat::Uint32);
            Self::draw_batch(&mut rpass, &self.scene.water_batch);
            Self::draw_batch(&mut rpass, &self.scene.cloud_batch);
            Self::draw_batch(&mut rpass, &self.scene.mist_batch);
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
                .with_title("桜の帝都村 · Massive Micro-Voxel Sakura Empire — Generating World...")
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
                if !pressed {
                    s.last_cursor = None;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some((lx, ly)) = s.last_cursor {
                    let (dx, dy) = ((position.x - lx) as f32, (position.y - ly) as f32);
                    if s.left_drag {
                        s.cam.tour = false;
                        s.cam.yaw -= dx * 0.0055;
                        s.cam.pitch = (s.cam.pitch + dy * 0.0055).clamp(0.02, 1.48);
                    } else if s.right_drag {
                        s.cam.tour = false;
                        let right = Vec3::new(s.cam.yaw.cos(), 0.0, -s.cam.yaw.sin());
                        let fwd = Vec3::new(s.cam.yaw.sin(), 0.0, s.cam.yaw.cos());
                        let pan = s.cam.dist * 0.0014;
                        s.cam.target -= right * (dx * pan) + fwd * (dy * pan);
                    }
                }
                if s.left_drag || s.right_drag {
                    s.last_cursor = Some((position.x, position.y));
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let sc = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.02,
                };
                s.cam.tour = false;
                s.cam.dist = (s.cam.dist * (1.0 - sc * 0.1)).clamp(6.0, 460.0);
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => match code {
                KeyCode::KeyT | KeyCode::Space => {
                    s.cam.tour = !s.cam.tour;
                    if s.cam.tour {
                        s.cam.tour_pos_actual = s.cam.eye();
                        s.cam.tour_tgt_actual = s.cam.target;
                    }
                }
                KeyCode::KeyN | KeyCode::KeyL => {
                    s.time_of_day = s.time_of_day.next();
                }
                KeyCode::KeyP => {
                    s.paused = !s.paused;
                }
                KeyCode::KeyR => {
                    s.cam.set_preset(0);
                }
                KeyCode::Digit1 => s.cam.set_preset(0),
                KeyCode::Digit2 => s.cam.set_preset(1),
                KeyCode::Digit3 => s.cam.set_preset(2),
                KeyCode::Digit4 => s.cam.set_preset(3),
                KeyCode::Digit5 => s.cam.set_preset(4),
                KeyCode::Digit6 => s.cam.set_preset(5),
                KeyCode::Digit7 => s.cam.set_preset(6),
                KeyCode::Digit8 => s.cam.set_preset(7),
                KeyCode::Digit9 => s.cam.set_preset(8),
                KeyCode::Escape => event_loop.exit(),
                _ => {}
            },
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(s) = &self.state {
            s.window.request_redraw();
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = App { state: None };
    event_loop.run_app(&mut app).unwrap();
}
