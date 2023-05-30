use crate::canvas::{
    Canvas, GifUploader, ScreenshotUploader, DEFAULT_CANVAS_HEIGHT, DEFAULT_CANVAS_WIDTH,
};
use wgpu::{util::DeviceExt, Adapter, Device, PresentMode, Surface, SurfaceConfiguration};
use winit::window::Window;

pub struct RenderContext {
    pub(crate) canvas: Canvas,
    pub(crate) screenshot_uploader: ScreenshotUploader,
    pub(crate) gif_uploader: GifUploader,
    pub(crate) surface: wgpu::Surface,
    pub(crate) device: wgpu::Device,
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) queue: wgpu::Queue,
    pub(crate) surface_config: wgpu::SurfaceConfiguration,
    pub(crate) window_size: winit::dpi::PhysicalSize<u32>,
    pub(crate) window: Window,
    pub(crate) render_pipeline: wgpu::RenderPipeline,
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) index_buffer: wgpu::Buffer,
    pub(crate) num_indices: u32,
    pub(crate) diffuse_bind_group: wgpu::BindGroup,
    pub(crate) texture: wgpu::Texture,
    // pub(crate) texture_size: wgpu::Extent3d,
}

impl RenderContext {
    // Creating some of the wgpu types requires async code
    pub(crate) async fn new(window: Window) -> Self {
        // Create surface
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });
        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        // Create adapter. device and queue
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    label: None,
                },
                None, // Trace path
            )
            .await
            .unwrap();

        // Configure surface
        let size = window.inner_size();
        let surface_config =
            create_surface_config(&window, &surface, &adapter, PresentMode::AutoVsync);
        surface.configure(&device, &surface_config);
        // let surface_caps = surface.get_capabilities(&adapter);
        // let surface_format: wgpu::TextureFormat = surface_caps
        //     .formats
        //     .iter()
        //     .copied()
        //     .find(|f| f.describe().srgb)
        //     .unwrap_or(surface_caps.formats[0]);
        // let surface_config = wgpu::SurfaceConfiguration {
        //     usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        //     format: surface_format,
        //     width: size.width,
        //     height: size.height,
        //     // present_mode: surface_caps.present_modes[0],
        //     present_mode: PresentMode::AutoVsync,
        //     alpha_mode: surface_caps.alpha_modes[0],
        //     view_formats: vec![],
        // };

        let (render_pipeline, texture, diffuse_bind_group) = create_pipeline(
            &device,
            &surface_config,
            DEFAULT_CANVAS_WIDTH,
            DEFAULT_CANVAS_HEIGHT,
        );

        // Vertex and index buffer
        #[rustfmt::skip]
        const VERTICES: &[Vertex] = &[
            Vertex { position: [-1.0, -1.0, 0.0], uv: [0.0, 1.0]},
            Vertex { position: [1.0,  -1.0, 0.0], uv: [1.0, 1.0]},
            Vertex { position: [-1.0, 1.0,  0.0], uv: [0.0, 0.0]},
            Vertex { position: [1.0,  1.0,  0.0], uv: [1.0, 0.0]},
        ];
        const INDICES: &[u16] = &[0, 1, 2, 3, 2, 1];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let num_indices = INDICES.len() as u32;

        let canvas = Canvas::new(DEFAULT_CANVAS_WIDTH, DEFAULT_CANVAS_HEIGHT);
        let screenshot_uploader =
            ScreenshotUploader::new(DEFAULT_CANVAS_WIDTH, DEFAULT_CANVAS_HEIGHT);
        let gif_uploader = GifUploader::new(DEFAULT_CANVAS_WIDTH, DEFAULT_CANVAS_HEIGHT);

        Self {
            window,
            surface,
            device,
            adapter,
            queue,
            surface_config,
            window_size: size,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            num_indices,
            diffuse_bind_group,
            texture,
            canvas,
            screenshot_uploader,
            gif_uploader,
        }
    }

    pub(crate) fn resize_canvas_texture(&mut self, width: u32, height: u32) {
        let (pipeline, texture, bind_group) =
            create_pipeline(&self.device, &self.surface_config, width, height);
        self.render_pipeline = pipeline;
        self.texture = texture;
        self.diffuse_bind_group = bind_group;
    }

    pub(crate) fn reconfigure_present_mode(&mut self, present_mode: PresentMode) {
        self.surface_config.present_mode = present_mode;
        self.surface.configure(&self.device, &self.surface_config);
    }

    pub(crate) fn resize_window(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.window_size = new_size;
            self.surface_config.width = new_size.width;
            self.surface_config.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    pub(crate) fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // Update texture
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            self.canvas.pixels.as_slice(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: std::num::NonZeroU32::new(4 * self.canvas.width),
                rows_per_image: std::num::NonZeroU32::new(self.canvas.height),
            },
            self.texture.size(),
        );

        // Render texture
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

fn create_surface_config(
    window: &Window,
    surface: &Surface,
    adapter: &Adapter,
    present_mode: PresentMode,
) -> SurfaceConfiguration {
    let size = window.inner_size();
    let surface_caps = surface.get_capabilities(adapter);
    let surface_format: wgpu::TextureFormat = surface_caps
        .formats
        .iter()
        .copied()
        .find(|f| f.describe().srgb)
        .unwrap_or(surface_caps.formats[0]);
    wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: size.width,
        height: size.height,
        // present_mode: surface_caps.present_modes[0],
        // present_mode: PresentMode::AutoVsync,
        present_mode,
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
    }
}

fn create_pipeline(
    device: &Device,
    surface_config: &SurfaceConfiguration,
    width: u32,
    height: u32,
) -> (wgpu::RenderPipeline, wgpu::Texture, wgpu::BindGroup) {
    let texture_size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let diffuse_texture = device.create_texture(&wgpu::TextureDescriptor {
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        label: Some("diffuse_texture"),
        view_formats: &[],
    });

    let diffuse_texture_view = diffuse_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let texture_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("texture_bind_group_layout"),
        });
    let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &texture_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&diffuse_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
            },
        ],
        label: Some("diffuse_bind_group"),
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/shader.wgsl").into()),
    });

    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[&texture_bind_group_layout],
        push_constant_ranges: &[],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[Vertex::desc()],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_config.format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
    });

    (render_pipeline, diffuse_texture, diffuse_bind_group)
}

/// Vertex representation
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    uv: [f32; 2],
}

impl Vertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

// #[derive(Clone, Copy, Debug, Default, PartialEq)]
// pub struct Color {
//     pub r: f32,
//     pub g: f32,
//     pub b: f32,
//     pub a: f32,
// }
//
// #[allow(missing_docs)]
// impl Color {
//     pub const TRANSPARENT: Self = Self {
//         r: 0.0,
//         g: 0.0,
//         b: 0.0,
//         a: 0.0,
//     };
//     pub const BLACK: Self = Self {
//         r: 0.0,
//         g: 0.0,
//         b: 0.0,
//         a: 1.0,
//     };
//     pub const WHITE: Self = Self {
//         r: 1.0,
//         g: 1.0,
//         b: 1.0,
//         a: 1.0,
//     };
//     pub const RED: Self = Self {
//         r: 1.0,
//         g: 0.0,
//         b: 0.0,
//         a: 1.0,
//     };
//     pub const GREEN: Self = Self {
//         r: 0.0,
//         g: 1.0,
//         b: 0.0,
//         a: 1.0,
//     };
//     pub const BLUE: Self = Self {
//         r: 0.0,
//         g: 0.0,
//         b: 1.0,
//         a: 1.0,
//     };
// }
