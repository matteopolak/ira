use std::sync::Arc;

use glam::Vec3;
use ira_drum::{Drum, Material, Vertex};
use winit::window::Window;

use crate::{
	camera, light, Camera, CameraUniform, DrumExt, GpuDrum, GpuTexture, Instance, MaterialExt,
	VertexExt,
};

pub struct RenderState {
	pub window: Arc<Window>,
	pub camera: Camera,

	pub(crate) sample_count: u32,
	pub(crate) multisampled_texture: GpuTexture,

	pub(crate) device: wgpu::Device,
	pub(crate) queue: wgpu::Queue,
	pub(crate) surface: wgpu::Surface<'static>,
	pub(crate) config: wgpu::SurfaceConfiguration,

	pub(crate) brdf_bind_group: wgpu::BindGroup,

	pub(crate) opaque_render_pipeline: wgpu::RenderPipeline,
	pub(crate) transparent_render_pipeline: wgpu::RenderPipeline,

	pub(crate) depth_texture: GpuTexture,

	pub(crate) lights: light::Lights,
}

impl RenderState {
	pub async fn new(window: Window, drum: &Drum) -> Self {
		let window = Arc::new(window);
		let size = window.inner_size();
		let instance = wgpu::Instance::default();

		let surface = instance.create_surface(window.clone()).unwrap();
		let adapter = request_adapter(&instance, &surface).await;

		let (device, queue) = request_device(&adapter).await;

		let config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			format: wgpu::TextureFormat::Rgba8UnormSrgb,
			width: size.width,
			height: size.height,
			desired_maximum_frame_latency: 2,
			present_mode: wgpu::PresentMode::Mailbox,
			alpha_mode: wgpu::CompositeAlphaMode::Auto,
			view_formats: vec![],
		};

		surface.configure(&device, &config);

		let sample_flags = adapter.get_texture_format_features(config.format).flags;

		let sample_count = {
			if sample_flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X16) {
				16
			} else if sample_flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X8) {
				8
			} else if sample_flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4) {
				4
			} else if sample_flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X2) {
				2
			} else {
				1
			}
		};

		let depth_texture =
			GpuTexture::create_depth_texture(&device, &config, sample_count, "depth_texture");

		let (brdf_bind_group, brdf_bind_group_layout) =
			drum.create_brdf_bind_group(&device, &queue);

		let material_bind_group_layout = Material::create_bind_group_layout(&device);
		let camera_settings = camera::Settings::new(config.width as f32, config.height as f32);
		let camera = CameraUniform::from_settings(&camera_settings).into_gpu(&device);
		let camera = Camera::new(camera, camera_settings);

		let lights = light::Lights::from_lights(
			&[
				light::GpuLight::from_position(Vec3::new(0.0, 2.0, 0.0)),
				light::GpuLight::from_position(Vec3::new(2.0, 0.0, 0.0)),
			],
			&device,
		);

		let (opaque_render_pipeline, transparent_render_pipeline) = create_pbr_render_pipelines(
			&device,
			&material_bind_group_layout,
			&brdf_bind_group_layout,
			&camera,
			&lights,
			sample_count,
		);

		let multisampled_texture = GpuTexture::create_multisampled(&device, &config, sample_count);

		Self {
			window,
			camera,
			sample_count,
			multisampled_texture,
			device,
			queue,
			surface,
			config,
			brdf_bind_group,
			opaque_render_pipeline,
			transparent_render_pipeline,
			depth_texture,
			lights,
		}
	}

	/// Resizes the surface and its associated textures.
	///
	/// This should be used when the window is resized.
	pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
		self.config.width = size.width.max(1);
		self.config.height = size.height.max(1);

		self.camera
			.settings
			.projection
			.resize(self.config.width as f32, self.config.height as f32);
		self.depth_texture
			.resize(&self.device, self.config.width, self.config.height);
		self.multisampled_texture
			.resize(&self.device, self.config.width, self.config.height);

		self.surface.configure(&self.device, &self.config);
		self.window.request_redraw();
	}

	/// Renders a frame to the surface.
	///
	/// # Errors
	///
	/// See [`wgpu::Surface::get_current_texture`].
	pub fn render_frame(&self, drum: &GpuDrum) -> Result<(), wgpu::SurfaceError> {
		let frame = self.surface.get_current_texture()?;
		let mut encoder = self
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
		let view = if self.sample_count > 1 {
			&self.multisampled_texture.view
		} else {
			&frame
				.texture
				.create_view(&wgpu::TextureViewDescriptor::default())
		};

		let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: None,
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
					store: wgpu::StoreOp::Store,
				},
			})],
			depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
				view: &self.depth_texture.view,
				depth_ops: Some(wgpu::Operations {
					load: wgpu::LoadOp::Clear(1.0),
					store: wgpu::StoreOp::Store,
				}),
				stencil_ops: None,
			}),
			timestamp_writes: None,
			occlusion_query_set: None,
		});

		rpass.set_pipeline(&self.opaque_render_pipeline);

		for model in &drum.models {
			model.draw_instanced(
				&mut rpass,
				drum,
				&self.camera.gpu.bind_group,
				&self.lights.bind_group,
				&self.brdf_bind_group,
				false,
			);
		}

		drop(rpass);

		let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: None,
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Load,
					store: wgpu::StoreOp::Store,
				},
			})],
			depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
				view: &self.depth_texture.view,
				depth_ops: Some(wgpu::Operations {
					load: wgpu::LoadOp::Load,
					store: wgpu::StoreOp::Store,
				}),
				stencil_ops: None,
			}),
			timestamp_writes: None,
			occlusion_query_set: None,
		});

		rpass.set_pipeline(&self.transparent_render_pipeline);

		for model in &drum.models {
			model.draw_instanced(
				&mut rpass,
				drum,
				&self.camera.gpu.bind_group,
				&self.lights.bind_group,
				&self.brdf_bind_group,
				true,
			);
		}

		drop(rpass);

		if self.sample_count > 1 {
			let frame_view = frame
				.texture
				.create_view(&wgpu::TextureViewDescriptor::default());

			// copy MSAA texture to frame view
			encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: None,
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view,
					resolve_target: Some(&frame_view),
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Load,
						store: wgpu::StoreOp::Store,
					},
				})],
				depth_stencil_attachment: None,
				timestamp_writes: None,
				occlusion_query_set: None,
			});
		}

		self.queue.submit(Some(encoder.finish()));
		frame.present();

		Ok(())
	}
}

#[must_use]
pub fn create_render_pipeline(
	device: &wgpu::Device,
	layout: &wgpu::PipelineLayout,
	target: wgpu::ColorTargetState,
	depth_write_enabled: bool,
	shader: &wgpu::ShaderModule,
	sample_count: u32,
) -> wgpu::RenderPipeline {
	device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: None,
		layout: Some(layout),
		vertex: wgpu::VertexState {
			module: shader,
			entry_point: "vs_main",
			buffers: &[Vertex::desc(), Instance::desc()],
			compilation_options: Default::default(),
		},
		fragment: Some(wgpu::FragmentState {
			module: shader,
			entry_point: "fs_main",
			compilation_options: Default::default(),
			targets: &[Some(target)],
		}),
		primitive: wgpu::PrimitiveState {
			conservative: false,
			topology: wgpu::PrimitiveTopology::TriangleList,
			strip_index_format: None,
			front_face: wgpu::FrontFace::Ccw,
			cull_mode: Some(wgpu::Face::Back),
			polygon_mode: wgpu::PolygonMode::Fill,
			unclipped_depth: false,
		},
		depth_stencil: Some(wgpu::DepthStencilState {
			format: GpuTexture::DEPTH_FORMAT,
			depth_write_enabled,
			depth_compare: wgpu::CompareFunction::LessEqual,
			stencil: wgpu::StencilState::default(),
			bias: wgpu::DepthBiasState::default(),
		}),
		multisample: wgpu::MultisampleState {
			count: sample_count,
			..Default::default()
		},
		multiview: None,
	})
}

pub async fn request_device(adapter: &wgpu::Adapter) -> (wgpu::Device, wgpu::Queue) {
	adapter
		.request_device(
			&wgpu::DeviceDescriptor {
				label: None,
				required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
					| wgpu::Features::TEXTURE_COMPRESSION_BC,
				required_limits: wgpu::Limits::downlevel_defaults()
					.using_resolution(adapter.limits()),
			},
			None,
		)
		.await
		.expect("failed to create device")
}

pub async fn request_adapter(
	instance: &wgpu::Instance,
	surface: &wgpu::Surface<'_>,
) -> wgpu::Adapter {
	instance
		.request_adapter(&wgpu::RequestAdapterOptions {
			power_preference: wgpu::PowerPreference::default(),
			force_fallback_adapter: false,
			compatible_surface: Some(surface),
		})
		.await
		.expect("failed to find an appropriate adapter")
}

#[allow(clippy::too_many_arguments)]
pub fn create_pbr_render_pipelines(
	device: &wgpu::Device,
	material_bind_group_layout: &wgpu::BindGroupLayout,
	brdf_bind_group_layout: &wgpu::BindGroupLayout,
	camera: &Camera,
	lights: &light::Lights,
	sample_count: u32,
) -> (wgpu::RenderPipeline, wgpu::RenderPipeline) {
	let shader = device.create_shader_module(wgpu::include_wgsl!(concat!(
		env!("CARGO_MANIFEST_DIR"),
		"/shaders/pbr.wgsl"
	)));

	let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
		label: None,
		bind_group_layouts: &[
			material_bind_group_layout,
			&camera.gpu.bind_group_layout,
			&lights.bind_group_layout,
			brdf_bind_group_layout,
		],
		push_constant_ranges: &[],
	});

	let opaque_render_pipeline = create_render_pipeline(
		device,
		&pipeline_layout,
		wgpu::ColorTargetState {
			format: wgpu::TextureFormat::Rgba8UnormSrgb,
			blend: None,
			write_mask: wgpu::ColorWrites::ALL,
		},
		true,
		&shader,
		sample_count,
	);

	let transparent_render_pipeline = {
		let transparent_target = wgpu::ColorTargetState {
			format: wgpu::TextureFormat::Rgba8UnormSrgb,
			blend: Some(wgpu::BlendState {
				color: wgpu::BlendComponent {
					src_factor: wgpu::BlendFactor::SrcAlpha,
					dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
					operation: wgpu::BlendOperation::Add,
				},
				alpha: wgpu::BlendComponent {
					src_factor: wgpu::BlendFactor::One,
					dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
					operation: wgpu::BlendOperation::Add,
				},
			}),
			write_mask: wgpu::ColorWrites::COLOR,
		};

		create_render_pipeline(
			device,
			&pipeline_layout,
			transparent_target,
			false,
			&shader,
			sample_count,
		)
	};

	(opaque_render_pipeline, transparent_render_pipeline)
}
