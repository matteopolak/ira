use ira_drum::{Handle, Texture};
#[cfg(feature = "client")]
use wgpu::util::DeviceExt;

use crate::GpuDrum;

#[cfg(feature = "client")]
use crate::render::texture::{GpuTexture, TextureExt};

#[derive(Debug)]
pub struct GpuMesh {
	pub num_indices: u32,
	/// The bounding box of the mesh.
	pub min: glam::Vec3,
	pub max: glam::Vec3,

	#[cfg(feature = "client")]
	pub vertex_buffer: wgpu::Buffer,
	#[cfg(feature = "client")]
	pub index_buffer: wgpu::Buffer,
	#[cfg(feature = "client")]
	pub material: Handle<GpuMaterial>,
}

#[derive(Debug)]
pub struct GpuMeshes {
	pub opaque: Box<[GpuMesh]>,
	pub transparent: Box<[GpuMesh]>,
}

#[cfg(feature = "client")]
#[derive(Debug)]
pub struct GpuMaterial {
	pub albedo: Handle<GpuTexture>,
	pub normal: Handle<GpuTexture>,
	pub orm: Handle<GpuTexture>,
	pub emissive: Handle<GpuTexture>,

	pub transparent: bool,
	pub bind_group: wgpu::BindGroup,
}

#[cfg(feature = "client")]
pub trait MaterialExt {
	fn into_gpu(
		self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		drum: &mut GpuDrum,
	) -> GpuMaterial;

	fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout;
}

#[cfg(feature = "client")]
impl MaterialExt for ira_drum::Material {
	#[must_use]
	fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
		device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			entries: &[
				// diffuse
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
				// normal
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
				// ao + roughness + metallic
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
				// emissive
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
			|| drum.textures.default_normal(device, queue),
			|h| Handle::new(h.raw()),
		);
		let orm_handle = self.orm.map_or_else(
			|| drum.textures.default_orm(device, queue),
			|h| Handle::new(h.raw()),
		);
		let emissive_handle = self.emissive.map_or_else(
			|| drum.textures.default_emissive(device, queue),
			|h| Handle::new(h.raw()),
		);

		let albedo = albedo_handle.resolve(&drum.textures.bank);
		let normal = normal_handle.resolve(&drum.textures.bank);
		let orm = orm_handle.resolve(&drum.textures.bank);
		let emissive = emissive_handle.resolve(&drum.textures.bank);

		let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			entries: &[
				// albedo
				albedo.view_layout(0),
				Texture::sampler_layout(1),
				// normal
				normal.view_layout(2),
				Texture::sampler_layout(3),
				// ao + roughness + metallic
				orm.view_layout(4),
				Texture::sampler_layout(5),
				// emissive
				emissive.view_layout(6),
				Texture::sampler_layout(7),
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
					resource: wgpu::BindingResource::TextureView(&orm.view),
				},
				wgpu::BindGroupEntry {
					binding: 5,
					resource: wgpu::BindingResource::Sampler(&orm.sampler),
				},
				wgpu::BindGroupEntry {
					binding: 6,
					resource: wgpu::BindingResource::TextureView(&emissive.view),
				},
				wgpu::BindGroupEntry {
					binding: 7,
					resource: wgpu::BindingResource::Sampler(&emissive.sampler),
				},
			],
			label: None,
		});

		GpuMaterial {
			bind_group,
			albedo: albedo_handle,
			normal: normal_handle,
			orm: orm_handle,
			emissive: emissive_handle,

			transparent: self.transparent,
		}
	}
}

pub trait MeshExt {
	fn to_gpu(&self, #[cfg(feature = "client")] device: &wgpu::Device) -> GpuMesh;
}

impl MeshExt for ira_drum::Mesh {
	fn to_gpu(&self, #[cfg(feature = "client")] device: &wgpu::Device) -> GpuMesh {
		#[cfg(feature = "client")]
		let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: None,
			contents: bytemuck::cast_slice(&self.vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});

		#[cfg(feature = "client")]
		let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: None,
			contents: bytemuck::cast_slice(&self.indices),
			usage: wgpu::BufferUsages::INDEX,
		});

		GpuMesh {
			min: self.min.into(),
			max: self.max.into(),
			num_indices: self.indices.len() as u32,
			#[cfg(feature = "client")]
			vertex_buffer,
			#[cfg(feature = "client")]
			index_buffer,
			#[cfg(feature = "client")]
			material: Handle::new(self.material.raw()),
		}
	}
}
