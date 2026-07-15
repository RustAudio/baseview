use baseview::dpi::{LogicalSize, PhysicalSize};
use baseview::{
    Event, EventStatus, HandlerError, WindowContext, WindowHandler, WindowOpenOptions, WindowSize,
};

use log::LevelFilter;
use std::cell::RefCell;

struct WgpuExample {
    window_context: WindowContext,

    instance: wgpu::Instance,
    device: wgpu::Device,
    pipeline: wgpu::RenderPipeline,
    queue: wgpu::Queue,
    surface: RefCell<wgpu::Surface<'static>>,
    surface_config: RefCell<wgpu::SurfaceConfiguration>,
}

impl WgpuExample {
    async fn new(context: WindowContext) -> Result<Self, HandlerError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
            Box::new(context.platform_handle()),
        ));

        let surface = instance.create_surface(context.platform_handle())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                // Request an adapter which can render to our surface
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await?;

        // Create the logical device and command queue
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                ..Default::default()
            })
            .await?;

        const SHADER: &str = "
            const VERTS = array(
                vec2<f32>(0.5, 1.0),
                vec2<f32>(0.0, 0.0),
                vec2<f32>(1.0, 0.0)
            );

            struct VertexOutput {
                @builtin(position) clip_position: vec4<f32>,
                @location(0) position: vec2<f32>,
            };

            @vertex
            fn vs_main(
                @builtin(vertex_index) in_vertex_index: u32,
            ) -> VertexOutput {
                var out: VertexOutput;
                out.position = VERTS[in_vertex_index];
                out.clip_position = vec4<f32>(out.position - 0.5, 0.0, 1.0);
                return out;
            }

            @fragment
            fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                return vec4<f32>(in.position, 0.5, 1.0);
            }
            ";

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(swapchain_format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let PhysicalSize { width, height } = context.size().physical;

        let surface_config = surface.get_default_config(&adapter, width, height).unwrap();
        surface.configure(&device, &surface_config);

        Ok(Self {
            window_context: context,

            instance,
            device,
            pipeline,
            queue,
            surface: surface.into(),
            surface_config: surface_config.into(),
        })
    }
}

impl WindowHandler for WgpuExample {
    fn on_frame(&self) -> Result<(), HandlerError> {
        let mut surface = self.surface.borrow_mut();

        let surface_texture = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Occluded | wgpu::CurrentSurfaceTexture::Timeout => {
                return Ok(())
            }
            wgpu::CurrentSurfaceTexture::Suboptimal(_) | wgpu::CurrentSurfaceTexture::Outdated => {
                surface.configure(&self.device, &self.surface_config.borrow());
                // We'll retry next frame
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                *surface = self.instance.create_surface(self.window_context.platform_handle())?;
                surface.configure(&self.device, &self.surface_config.borrow());

                // We'll retry next frame
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                unreachable!("No error scope registered, so validation errors will panic")
            }
        };

        let view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            rpass.set_pipeline(&self.pipeline);
            rpass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(surface_texture);

        Ok(())
    }

    fn resized(&self, new_size: WindowSize) -> Result<(), HandlerError> {
        let mut surface_config = self.surface_config.borrow_mut();

        surface_config.width = new_size.physical.width;
        surface_config.height = new_size.physical.height;

        let surface = self.surface.borrow();
        surface.configure(&self.device, &surface_config);

        Ok(())
    }

    fn on_event(&self, event: Event) -> EventStatus {
        log_event(&event);

        EventStatus::Ignored
    }
}

fn main() -> Result<(), baseview::Error> {
    env_logger::builder().filter_level(LevelFilter::Debug).init();
    let window_open_options = WindowOpenOptions::new()
        .with_title("WGPU on Baseview")
        .with_size(LogicalSize::new(512, 512));

    baseview::create_window(window_open_options, |c| pollster::block_on(WgpuExample::new(c)))?
        .run_until_closed();

    Ok(())
}

fn log_event(event: &Event) {
    match event {
        Event::Mouse(e) => println!("Mouse event: {:?}", e),
        Event::Keyboard(e) => println!("Keyboard event: {:?}", e),
        Event::Window(e) => println!("Window event: {:?}", e),
        _ => {}
    }
}
