use ira_drum::Handle;

use crate::{GpuMesh, GpuModel, ModelExt};

#[cfg(feature = "client")]
use crate::render::texture::{GpuTexture, GpuTextureCollection, TextureExt};
#[cfg(feature = "client")]
use crate::{GpuMaterial, MaterialExt, MeshExt};

#[derive(Debug, Default)]
pub struct GpuDrum {
	#[cfg(feature = "client")]
	pub textures: GpuTextureCollection,
	#[cfg(feature = "client")]
	pub materials: Vec<GpuMaterial>,
	pub meshes: Vec<GpuMesh>,
	pub models: Vec<GpuModel>,
}

impl GpuDrum {
	/// Adds a texture to the drum, returning a handle to it.
	#[cfg(feature = "client")]
	pub fn add_texture(&mut self, texture: GpuTexture) -> Handle<GpuTexture> {
		self.textures.add_texture(texture)
	}

	/// Finds a model by name, returning its index.
	#[must_use]
	pub fn model_id(&self, name: &str) -> Option<u32> {
		self.models
			.iter()
			.position(|m| &*m.name == name)
			.map(|i| i as u32)
	}

	/// Finds a model by name, returning a reference to it.
	#[must_use]
	pub fn model_by_name(&self, name: &str) -> Option<&GpuModel> {
		self.models.iter().find(|m| &*m.name == name)
	}
}

pub trait DrumExt {
	#[cfg(feature = "client")]
	fn create_brdf_bind_group(
		&self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
	) -> (wgpu::BindGroup, wgpu::BindGroupLayout);

	fn into_gpu(
		self,
		#[cfg(feature = "client")] device: &wgpu::Device,
		#[cfg(feature = "client")] queue: &wgpu::Queue,
	) -> GpuDrum;
}

impl DrumExt for ira_drum::Drum {
	fn into_gpu(
		self,
		#[cfg(feature = "client")] device: &wgpu::Device,
		#[cfg(feature = "client")] queue: &wgpu::Queue,
	) -> GpuDrum {
		let mut drum = GpuDrum::default();

		#[cfg(feature = "client")]
		{
			drum.textures.bank = self
				.textures
				.iter()
				.map(|t| t.to_gpu(device, queue))
				.collect::<Vec<_>>();

			drum.materials = IntoIterator::into_iter(self.materials)
				.map(|m| m.into_gpu(device, queue, &mut drum))
				.collect();

			drum.meshes = self.meshes.iter().map(|m| m.to_gpu(device)).collect();
		}

		drum.models = IntoIterator::into_iter(self.models)
			.map(|m| {
				m.into_gpu(
					&drum,
					#[cfg(feature = "client")]
					device,
				)
			})
			.collect();

		drum
	}

	#[cfg(feature = "client")]
	fn create_brdf_bind_group(
		&self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
	) -> (wgpu::BindGroup, wgpu::BindGroupLayout) {
		use ira_drum::Texture;

		let brdf_lut = self.brdf_lut.as_ref().unwrap();
		let irradiance_map = self.irradiance_map.as_ref().unwrap();
		let prefiltered_map = self.prefiltered_map.as_ref().unwrap();

		let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: None,
			entries: &[
				brdf_lut.view_layout(0),
				Texture::sampler_layout(1),
				prefiltered_map.view_layout(2),
				Texture::sampler_layout(3),
				irradiance_map.view_layout(4),
				Texture::sampler_layout(5),
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
