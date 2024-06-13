use ira_drum::Handle;
use wgpu::util::DeviceExt;

use crate::{GpuDrum, GpuTexture};

#[derive(Debug)]
pub struct GpuMesh {
	pub vertex_buffer: wgpu::Buffer,
	pub index_buffer: wgpu::Buffer,
	pub material: Handle<GpuMaterial>,

	pub num_indices: u32,
}

#[derive(Debug)]
pub struct GpuMeshes {
	pub opaque: Box<[GpuMesh]>,
	pub transparent: Box<[GpuMesh]>,
}

#[derive(Debug)]
pub struct GpuMaterial {
	pub albedo: Handle<GpuTexture>,
	pub normal: Handle<GpuTexture>,
	pub metallic_roughness: Handle<GpuTexture>,
	pub ao: Handle<GpuTexture>,
	pub emissive: Handle<GpuTexture>,

	pub transparent: bool,
	pub bind_group: wgpu::BindGroup,
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
			|| drum.textures.default_normal(device, queue),
			|h| Handle::new(h.raw()),
		);
		let metallic_roughness_handle = self.metallic_roughness.map_or_else(
			|| drum.textures.default_metallic_roughness(device, queue),
			|h| Handle::new(h.raw()),
		);
		let ao_handle = self.ao.map_or_else(
			|| drum.textures.default_ao(device, queue),
			|h| Handle::new(h.raw()),
		);
		let emissive_handle = self.emissive.map_or_else(
			|| drum.textures.default_emissive(device, queue),
			|h| Handle::new(h.raw()),
		);

		let albedo = albedo_handle.resolve(&drum.textures.bank);
		let normal = normal_handle.resolve(&drum.textures.bank);
		let metallic_roughness = metallic_roughness_handle.resolve(&drum.textures.bank);
		let ao = ao_handle.resolve(&drum.textures.bank);
		let emissive = emissive_handle.resolve(&drum.textures.bank);

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
