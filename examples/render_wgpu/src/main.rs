use baseview::{MouseEvent, Size, Window, WindowHandler, WindowOpenOptions};
use wgpu::{util::DeviceExt, Buffer, Device, Queue, RenderPipeline, Surface};

struct WgpuExample {
    pipeline: RenderPipeline,
    device: Device,
    surface: Surface,
    queue: Queue,
    vertex_buffer: Buffer,
}

impl<'a> WgpuExample {
    pub async fn new(window: &mut Window<'a>) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let surface = unsafe { instance.create_surface(window) }.unwrap();
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
                None,
            )
            .await
            .unwrap();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES_START),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            // TODO this needs to be twice the window size for some reason
            width: 1024,
            height: 1024,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                    format: config.format,
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

        surface.configure(&device, &config);

        Self { pipeline, device, surface, queue, vertex_buffer }
    }
}

impl WindowHandler for WgpuExample {
    fn on_frame(&mut self, _window: &mut baseview::Window) {
        let output = self.surface.get_current_texture().unwrap();
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.2, b: 0.3, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
    fn on_event(
        &mut self, _window: &mut baseview::Window, event: baseview::Event,
    ) -> baseview::EventStatus {
        if let baseview::Event::Mouse(MouseEvent::CursorMoved { position, modifiers: _ }) = event {
            let center_x: f32 = (position.x as f32 - 256.0) / 256.0;
            let center_y: f32 = (256.0 - position.y as f32) / 256.0;
            let vertices = &[
                Vertex { position: [center_x, center_y + 0.25, 0.0], color: [1.0, 0.0, 0.0] },
                Vertex {
                    position: [center_x - 0.25, center_y - 0.25, 0.0],
                    color: [0.0, 1.0, 0.0],
                },
                Vertex {
                    position: [center_x + 0.25, center_y - 0.25, 0.0],
                    color: [0.0, 0.0, 1.0],
                },
            ];
            let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            self.vertex_buffer = vertex_buffer;
        }
        baseview::EventStatus::Captured
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

const VERTICES_START: &[Vertex] = &[
    Vertex { position: [0.0, 0.25, 0.0], color: [1.0, 0.0, 0.0] },
    Vertex { position: [-0.25, -0.25, 0.0], color: [0.0, 1.0, 0.0] },
    Vertex { position: [0.25, -0.25, 0.0], color: [0.0, 0.0, 1.0] },
];

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

fn main() {
    let window_open_options = WindowOpenOptions {
        title: "wgpu on baseview".into(),
        size: Size::new(512.0, 512.0),
        scale: baseview::WindowScalePolicy::SystemScaleFactor,
        gl_config: None,
    };

    Window::open_blocking(window_open_options, |window| {
        pollster::block_on(WgpuExample::new(window))
    })
}
