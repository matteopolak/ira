use std::{borrow::Cow, sync::Arc};

use winit::{
	application::ApplicationHandler,
	event::WindowEvent,
	event_loop::{ActiveEventLoop, EventLoop},
	window::{Window, WindowAttributes, WindowId},
};

#[derive(Default)]
struct App {
	state: Option<State>,
}

struct State {
	window: Arc<Window>,
	device: wgpu::Device,
	queue: wgpu::Queue,
	surface: wgpu::Surface<'static>,
	render_pipeline: wgpu::RenderPipeline,
	config: wgpu::SurfaceConfiguration,
}

impl State {
	async fn new(window: Window) -> Self {
		let window = Arc::new(window);

		let mut size = window.inner_size();
		size.width = size.width.max(1);
		size.height = size.height.max(1);

		let instance = wgpu::Instance::default();

		let surface = instance.create_surface(window.clone()).unwrap();
		let adapter = instance
			.request_adapter(&wgpu::RequestAdapterOptions {
				power_preference: wgpu::PowerPreference::default(),
				force_fallback_adapter: false,
				// Request an adapter which can render to our surface
				compatible_surface: Some(&surface),
			})
			.await
			.expect("Failed to find an appropriate adapter");

		// Create the logical device and command queue
		let (device, queue) = adapter
			.request_device(
				&wgpu::DeviceDescriptor {
					label: None,
					required_features: wgpu::Features::empty(),
					// Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
					required_limits: wgpu::Limits::downlevel_webgl2_defaults()
						.using_resolution(adapter.limits()),
				},
				None,
			)
			.await
			.expect("Failed to create device");

		// Load the shaders from disk
		let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: None,
			source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
				"../shaders/triangle.wgsl"
			))),
		});

		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: None,
			bind_group_layouts: &[],
			push_constant_ranges: &[],
		});

		let swapchain_capabilities = surface.get_capabilities(&adapter);
		let swapchain_format = swapchain_capabilities.formats[0];

		let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: None,
			layout: Some(&pipeline_layout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: "vs_main",
				buffers: &[],
				compilation_options: Default::default(),
			},
			fragment: Some(wgpu::FragmentState {
				module: &shader,
				entry_point: "fs_main",
				compilation_options: Default::default(),
				targets: &[Some(swapchain_format.into())],
			}),
			primitive: wgpu::PrimitiveState::default(),
			depth_stencil: None,
			multisample: wgpu::MultisampleState::default(),
			multiview: None,
		});

		let config = surface
			.get_default_config(&adapter, size.width, size.height)
			.unwrap();
		surface.configure(&device, &config);

		Self {
			window,
			device,
			queue,
			surface,
			render_pipeline,
			config,
		}
	}

	fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
		self.config.width = size.width.max(1);
		self.config.height = size.height.max(1);

		self.surface.configure(&self.device, &self.config);
		self.window.request_redraw();
	}

	fn render(&self) {
		let frame = self
			.surface
			.get_current_texture()
			.expect("Failed to acquire next swap chain texture");
		let view = frame
			.texture
			.create_view(&wgpu::TextureViewDescriptor::default());
		let mut encoder = self
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
		{
			let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: None,
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &view,
					resolve_target: None,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
						store: wgpu::StoreOp::Store,
					},
				})],
				depth_stencil_attachment: None,
				timestamp_writes: None,
				occlusion_query_set: None,
			});
			rpass.set_pipeline(&self.render_pipeline);
			rpass.draw(0..3, 0..1);
		}

		self.queue.submit(Some(encoder.finish()));
		frame.present();
	}
}

impl ApplicationHandler for App {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		let window = event_loop
			.create_window(WindowAttributes::default().with_title("Triangle"))
			.unwrap();
		let state = pollster::block_on(State::new(window));

		self.state = Some(state);
	}

	fn window_event(
		&mut self,
		event_loop: &ActiveEventLoop,
		window_id: WindowId,
		event: WindowEvent,
	) {
		let Some(state) = self.state.as_mut() else {
			return;
		};

		if window_id != state.window.id() {
			return;
		}

		match event {
			WindowEvent::Resized(size) => {
				state.resize(size);
			}
			WindowEvent::RedrawRequested => {
				state.render();
			}
			WindowEvent::CloseRequested => {
				event_loop.exit();
			}
			_ => {}
		}
	}
}

fn main() {
	let event_loop = EventLoop::new().unwrap();
	let mut app = App::default();

	let _ = event_loop.run_app(&mut app);
}
