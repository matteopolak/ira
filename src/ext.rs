use wgpu::util::DeviceExt;

#[must_use]
#[derive(Debug)]
pub struct GpuTexture {
	pub texture: wgpu::Texture,
	pub view: wgpu::TextureView,
	pub sampler: wgpu::Sampler,
}

trait TextureExt {
	fn into_gpu(self, device: &wgpu::Device, queue: &wgpu::Queue) -> GpuTexture;

	fn view_dimension(&self) -> wgpu::TextureViewDimension;
	fn view_layout(&self, binding: u32) -> wgpu::BindGroupLayoutEntry;
	fn sampler_layout(&self, binding: u32) -> wgpu::BindGroupLayoutEntry;
}

impl TextureExt for ira_drum::Texture {
	fn into_gpu(self, device: &wgpu::Device, queue: &wgpu::Queue) -> GpuTexture {
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
