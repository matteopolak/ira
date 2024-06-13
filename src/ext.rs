use glam::Vec3;
use ira_drum::{Drum, DrumBuilder, Handle};
use wgpu::util::DeviceExt;

use crate::{GpuMaterial, GpuMesh, GpuModel, Instance};

#[must_use]
#[derive(Debug)]
pub struct GpuTexture {
	pub texture: wgpu::Texture,
	pub view: wgpu::TextureView,
	pub sampler: wgpu::Sampler,

	view_dimension: wgpu::TextureViewDimension,
}

impl GpuTexture {
	pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

	pub fn create_multisampled(
		device: &wgpu::Device,
		config: &wgpu::SurfaceConfiguration,
		sample_count: u32,
	) -> Self {
		let size = wgpu::Extent3d {
			width: config.width,
			height: config.height,
			depth_or_array_layers: 1,
		};
		let desc = wgpu::TextureDescriptor {
			label: None,
			size,
			mip_level_count: 1,
			sample_count,
			dimension: wgpu::TextureDimension::D2,
			format: config.format,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT
				| wgpu::TextureUsages::TEXTURE_BINDING
				| wgpu::TextureUsages::COPY_SRC,
			view_formats: &[config.format],
		};
		let texture = device.create_texture(&desc);

		let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Linear,
			mipmap_filter: wgpu::FilterMode::Nearest,
			compare: None,
			lod_min_clamp: 0.0,
			lod_max_clamp: 100.0,
			..Default::default()
		});

		Self {
			texture,
			view,
			sampler,
			view_dimension: wgpu::TextureViewDimension::D2,
		}
	}

	pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
		let size = wgpu::Extent3d {
			width,
			height,
			depth_or_array_layers: 1,
		};

		self.texture = device.create_texture(&wgpu::TextureDescriptor {
			label: None,
			size,
			mip_level_count: self.texture.mip_level_count(),
			sample_count: self.texture.sample_count(),
			dimension: self.texture.dimension(),
			format: self.texture.format(),
			usage: self.texture.usage(),
			view_formats: &[],
		});

		self.view = self
			.texture
			.create_view(&wgpu::TextureViewDescriptor::default());
	}

	pub fn create_depth_texture(
		device: &wgpu::Device,
		config: &wgpu::SurfaceConfiguration,
		sample_count: u32,
		label: &str,
	) -> Self {
		let size = wgpu::Extent3d {
			width: config.width,
			height: config.height,
			depth_or_array_layers: 1,
		};
		let desc = wgpu::TextureDescriptor {
			label: Some(label),
			size,
			mip_level_count: 1,
			sample_count,
			dimension: wgpu::TextureDimension::D2,
			format: Self::DEPTH_FORMAT,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
			view_formats: &[],
		};
		let texture = device.create_texture(&desc);

		let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Linear,
			mipmap_filter: wgpu::FilterMode::Nearest,
			compare: Some(wgpu::CompareFunction::LessEqual),
			lod_min_clamp: 0.0,
			lod_max_clamp: 100.0,
			..Default::default()
		});

		Self {
			texture,
			view,
			sampler,
			view_dimension: wgpu::TextureViewDimension::D2,
		}
	}

	fn view_layout(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
		wgpu::BindGroupLayoutEntry {
			binding,
			visibility: wgpu::ShaderStages::FRAGMENT,
			ty: wgpu::BindingType::Texture {
				sample_type: wgpu::TextureSampleType::Float { filterable: true },
				multisampled: false,
				view_dimension: self.view_dimension,
			},
			count: None,
		}
	}

	fn sampler_layout(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
		wgpu::BindGroupLayoutEntry {
			binding,
			visibility: wgpu::ShaderStages::FRAGMENT,
			ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
			count: None,
		}
	}
}

pub trait TextureExt {
	fn to_gpu(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> GpuTexture;

	fn view_dimension(&self) -> wgpu::TextureViewDimension;
	fn view_layout(&self, binding: u32) -> wgpu::BindGroupLayoutEntry;
	fn sampler_layout(&self, binding: u32) -> wgpu::BindGroupLayoutEntry;
}

impl TextureExt for ira_drum::Texture {
	fn to_gpu(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> GpuTexture {
		let texture = device.create_texture_with_data(
			queue,
			&wgpu::TextureDescriptor {
				label: None,
				size: self.extent.into(),
				mip_level_count: self.mipmaps,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: self.format.into(),
				usage: wgpu::TextureUsages::TEXTURE_BINDING
					| wgpu::TextureUsages::COPY_DST
					| wgpu::TextureUsages::RENDER_ATTACHMENT,
				view_formats: &[],
			},
			wgpu::util::TextureDataOrder::MipMajor,
			&self.data,
		);

		let view = texture.create_view(&wgpu::TextureViewDescriptor {
			label: None,
			dimension: Some(self.view_dimension()),
			mip_level_count: Some(self.mipmaps),
			..Default::default()
		});
		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Linear,
			mipmap_filter: wgpu::FilterMode::Linear,
			lod_min_clamp: 0.0,
			lod_max_clamp: self.mipmaps as f32,
			anisotropy_clamp: 16,
			..Default::default()
		});

		GpuTexture {
			texture,
			view,
			sampler,
			view_dimension: self.view_dimension(),
		}
	}

	fn view_dimension(&self) -> wgpu::TextureViewDimension {
		if self.extent.depth == 6 {
			wgpu::TextureViewDimension::Cube
		} else {
			wgpu::TextureViewDimension::D2
		}
	}

	fn view_layout(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
		let view_dimension = self.view_dimension();

		wgpu::BindGroupLayoutEntry {
			binding,
			visibility: wgpu::ShaderStages::FRAGMENT,
			ty: wgpu::BindingType::Texture {
				sample_type: wgpu::TextureSampleType::Float { filterable: true },
				multisampled: false,
				view_dimension,
			},
			count: None,
		}
	}

	fn sampler_layout(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
		wgpu::BindGroupLayoutEntry {
			binding,
			visibility: wgpu::ShaderStages::FRAGMENT,
			ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
			count: None,
		}
	}
}

pub struct GpuTextureCollection {
	pub textures: Vec<GpuTexture>,

	/// Default texture for when a texture is missing.
	pub normal: Option<Handle<GpuTexture>>,
	pub metallic_roughness: Option<Handle<GpuTexture>>,
	pub ao: Option<Handle<GpuTexture>>,
	pub emissive: Option<Handle<GpuTexture>>,
}

#[derive(Debug, Default)]
pub struct GpuDrum {
	pub textures: Vec<GpuTexture>,
	pub materials: Vec<GpuMaterial>,
	pub meshes: Vec<GpuMesh>,
	pub models: Vec<GpuModel>,
}

impl GpuDrum {
	pub fn add_texture(&mut self, texture: GpuTexture) -> Handle<GpuTexture> {
		Handle::from_vec(&mut self.textures, texture)
	}
}

pub trait DrumExt {
	fn create_brdf_bind_group(
		&self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
	) -> (wgpu::BindGroup, wgpu::BindGroupLayout);

	fn into_gpu(self, device: &wgpu::Device, queue: &wgpu::Queue) -> GpuDrum;
}

impl DrumExt for ira_drum::Drum {
	fn into_gpu(self, device: &wgpu::Device, queue: &wgpu::Queue) -> GpuDrum {
		let mut drum = GpuDrum::default();

		let textures = self
			.textures
			.iter()
			.map(|t| t.to_gpu(device, queue))
			.collect::<Vec<_>>();

		let materials = IntoIterator::into_iter(self.materials)
			.map(|m| m.into_gpu(device, queue, &mut drum))
			.collect();

		let meshes = self.meshes.iter().map(|m| m.to_gpu(device)).collect();

		let models = IntoIterator::into_iter(self.models)
			.map(|m| m.into_gpu(device, &[Instance::from_up(Vec3::ZERO, Vec3::Z)]))
			.collect();

		GpuDrum {
			textures,
			materials,
			meshes,
			models,
		}
	}

	fn create_brdf_bind_group(
		&self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
	) -> (wgpu::BindGroup, wgpu::BindGroupLayout) {
		let brdf_lut = self.brdf_lut.as_ref().unwrap();
		let irradiance_map = self.irradiance_map.as_ref().unwrap();
		let prefiltered_map = self.prefiltered_map.as_ref().unwrap();

		let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: None,
			entries: &[
				brdf_lut.view_layout(0),
				brdf_lut.sampler_layout(1),
				prefiltered_map.view_layout(2),
				prefiltered_map.sampler_layout(3),
				irradiance_map.view_layout(4),
				irradiance_map.sampler_layout(5),
			],
		});

		let brdf_lut = brdf_lut.to_gpu(device, queue);
		let prefiltered_map = prefiltered_map.to_gpu(device, queue);
		let irradiance_map = irradiance_map.to_gpu(device, queue);

		let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: None,
			layout: &layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&brdf_lut.view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&brdf_lut.sampler),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::TextureView(&prefiltered_map.view),
				},
				wgpu::BindGroupEntry {
					binding: 3,
					resource: wgpu::BindingResource::Sampler(&prefiltered_map.sampler),
				},
				wgpu::BindGroupEntry {
					binding: 4,
					resource: wgpu::BindingResource::TextureView(&irradiance_map.view),
				},
				wgpu::BindGroupEntry {
					binding: 5,
					resource: wgpu::BindingResource::Sampler(&irradiance_map.sampler),
				},
			],
		});

		(bind_group, layout)
	}
}

pub trait ModelExt {
	fn into_gpu(self, device: &wgpu::Device, instances: &[Instance]) -> GpuModel;
}

impl ModelExt for ira_drum::Model {
	fn into_gpu(self, device: &wgpu::Device, instances: &[Instance]) -> GpuModel {
		let instances = instances.iter().map(Instance::to_gpu).collect::<Vec<_>>();

		let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Instance Buffer"),
			contents: bytemuck::cast_slice(&instances),
			usage: wgpu::BufferUsages::VERTEX,
		});

		GpuModel {
			meshes: self.meshes.into(),
			instance_buffer,
			instances,
		}
	}
}

pub trait MaterialExt {
	fn into_gpu(
		self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		drum: &mut GpuDrum,
	) -> GpuMaterial;

	fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout;
}

impl MaterialExt for ira_drum::Material {
	#[must_use]
	fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
		device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			entries: &[
				// Diffuse
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
						view_dimension: wgpu::TextureViewDimension::D2,
						multisampled: false,
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 1,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
					count: None,
				},
				// Normal
				wgpu::BindGroupLayoutEntry {
					binding: 2,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
						view_dimension: wgpu::TextureViewDimension::D2,
						multisampled: false,
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 3,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
					count: None,
				},
				// Metallic + Roughness
				wgpu::BindGroupLayoutEntry {
					binding: 4,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
						view_dimension: wgpu::TextureViewDimension::D2,
						multisampled: false,
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 5,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
					count: None,
				},
				// AO
				wgpu::BindGroupLayoutEntry {
					binding: 6,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
						view_dimension: wgpu::TextureViewDimension::D2,
						multisampled: false,
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 7,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
					count: None,
				},
				// Emissive
				wgpu::BindGroupLayoutEntry {
					binding: 8,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
						view_dimension: wgpu::TextureViewDimension::D2,
						multisampled: false,
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 9,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
					count: None,
				},
			],
			label: Some("texture_bind_group_layout"),
		})
	}

	fn into_gpu(
		self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		drum: &mut GpuDrum,
	) -> GpuMaterial {
		let albedo_handle = Handle::new(self.albedo.raw());
		let normal_handle = self.normal.map_or_else(
			|| drum.add_texture(ira_drum::Texture::default_normal().to_gpu(device, queue)),
			|h| Handle::new(h.raw()),
		);
		let metallic_roughness_handle = self.metallic_roughness.map_or_else(
			|| {
				drum.add_texture(
					ira_drum::Texture::default_metallic_roughness().to_gpu(device, queue),
				)
			},
			|h| Handle::new(h.raw()),
		);
		let ao_handle = self.ao.map_or_else(
			|| drum.add_texture(ira_drum::Texture::default_ao().to_gpu(device, queue)),
			|h| Handle::new(h.raw()),
		);
		let emissive_handle = self.emissive.map_or_else(
			|| drum.add_texture(ira_drum::Texture::default_emissive().to_gpu(device, queue)),
			|h| Handle::new(h.raw()),
		);

		let albedo = albedo_handle.resolve(&drum.textures);
		let normal = normal_handle.resolve(&drum.textures);
		let metallic_roughness = metallic_roughness_handle.resolve(&drum.textures);
		let ao = ao_handle.resolve(&drum.textures);
		let emissive = emissive_handle.resolve(&drum.textures);

		let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			entries: &[
				// albedo
				albedo.view_layout(0),
				albedo.sampler_layout(1),
				// normal
				normal.view_layout(2),
				normal.sampler_layout(3),
				// metallic + roughness
				metallic_roughness.view_layout(4),
				metallic_roughness.sampler_layout(5),
				// ao
				ao.view_layout(6),
				ao.sampler_layout(7),
				// emissive
				emissive.view_layout(8),
				emissive.sampler_layout(9),
			],
			label: None,
		});

		let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&albedo.view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&albedo.sampler),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::TextureView(&normal.view),
				},
				wgpu::BindGroupEntry {
					binding: 3,
					resource: wgpu::BindingResource::Sampler(&normal.sampler),
				},
				wgpu::BindGroupEntry {
					binding: 4,
					resource: wgpu::BindingResource::TextureView(&metallic_roughness.view),
				},
				wgpu::BindGroupEntry {
					binding: 5,
					resource: wgpu::BindingResource::Sampler(&metallic_roughness.sampler),
				},
				wgpu::BindGroupEntry {
					binding: 6,
					resource: wgpu::BindingResource::TextureView(&ao.view),
				},
				wgpu::BindGroupEntry {
					binding: 7,
					resource: wgpu::BindingResource::Sampler(&ao.sampler),
				},
				wgpu::BindGroupEntry {
					binding: 8,
					resource: wgpu::BindingResource::TextureView(&emissive.view),
				},
				wgpu::BindGroupEntry {
					binding: 9,
					resource: wgpu::BindingResource::Sampler(&emissive.sampler),
				},
			],
			label: None,
		});

		GpuMaterial {
			bind_group,
			albedo: albedo_handle,
			normal: normal_handle,
			metallic_roughness: metallic_roughness_handle,
			ao: ao_handle,
			emissive: emissive_handle,

			transparent: self.transparent,
		}
	}
}

pub trait MeshExt {
	fn to_gpu(&self, device: &wgpu::Device) -> GpuMesh;
}

impl MeshExt for ira_drum::Mesh {
	fn to_gpu(&self, device: &wgpu::Device) -> GpuMesh {
		let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: None,
			contents: bytemuck::cast_slice(&self.vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});
		let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: None,
			contents: bytemuck::cast_slice(&self.indices),
			usage: wgpu::BufferUsages::INDEX,
		});

		GpuMesh {
			vertex_buffer,
			index_buffer,
			num_indices: self.indices.len() as u32,
			material: Handle::new(self.material.raw()),
		}
	}
}
