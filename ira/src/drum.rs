use ira_drum::Handle;

use crate::{
	GpuMaterial, GpuMesh, GpuModel, GpuTexture, GpuTextureCollection, MaterialExt, MeshExt,
	ModelExt, TextureExt,
};

#[derive(Debug, Default)]
pub struct GpuDrum {
	pub textures: GpuTextureCollection,
	pub materials: Vec<GpuMaterial>,
	pub meshes: Vec<GpuMesh>,
	pub models: Vec<GpuModel>,
}

impl GpuDrum {
	pub fn add_texture(&mut self, texture: GpuTexture) -> Handle<GpuTexture> {
		self.textures.add_texture(texture)
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

		drum.textures.bank = self
			.textures
			.iter()
			.map(|t| t.to_gpu(device, queue))
			.collect::<Vec<_>>();

		drum.materials = IntoIterator::into_iter(self.materials)
			.map(|m| m.into_gpu(device, queue, &mut drum))
			.collect();

		drum.meshes = self.meshes.iter().map(|m| m.to_gpu(device)).collect();

		drum.models = IntoIterator::into_iter(self.models)
			.map(|m| m.into_gpu(device, Vec::new()))
			.collect();

		drum
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
