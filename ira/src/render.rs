use ira_drum::Vertex;

use crate::{light, Camera, GpuTexture, Instance, VertexExt};

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
