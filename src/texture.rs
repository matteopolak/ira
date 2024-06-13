use ira_drum::Handle;
use wgpu::util::DeviceExt;

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
				usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
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

#[derive(Debug, Default)]
pub struct GpuTextureCollection {
	pub bank: Vec<GpuTexture>,

	/// Default texture for when a texture is missing.
	pub normal: Option<Handle<GpuTexture>>,
	pub metallic_roughness: Option<Handle<GpuTexture>>,
	pub ao: Option<Handle<GpuTexture>>,
	pub emissive: Option<Handle<GpuTexture>>,
}

impl GpuTextureCollection {
	pub fn add_texture(&mut self, texture: GpuTexture) -> Handle<GpuTexture> {
		Handle::from_vec(&mut self.bank, texture)
	}

	pub fn default_normal(
		&mut self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
	) -> Handle<GpuTexture> {
		self.normal.unwrap_or_else(|| {
			let texture = ira_drum::Texture::default_normal().to_gpu(device, queue);
			self.add_texture(texture)
		})
	}

	pub fn default_metallic_roughness(
		&mut self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
	) -> Handle<GpuTexture> {
		self.metallic_roughness.unwrap_or_else(|| {
			let texture = ira_drum::Texture::default_metallic_roughness().to_gpu(device, queue);
			self.add_texture(texture)
		})
	}

	pub fn default_ao(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Handle<GpuTexture> {
		self.ao.unwrap_or_else(|| {
			let texture = ira_drum::Texture::default_ao().to_gpu(device, queue);
			self.add_texture(texture)
		})
	}

	pub fn default_emissive(
		&mut self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
	) -> Handle<GpuTexture> {
		self.emissive.unwrap_or_else(|| {
			let texture = ira_drum::Texture::default_emissive().to_gpu(device, queue);
			self.add_texture(texture)
		})
	}
}

#[must_use]
#[derive(Debug)]
pub struct GpuTexture {
	pub texture: wgpu::Texture,
	pub view: wgpu::TextureView,
	pub sampler: wgpu::Sampler,

	pub view_dimension: wgpu::TextureViewDimension,
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

	pub fn view_layout(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
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

	pub fn sampler_layout(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
		wgpu::BindGroupLayoutEntry {
			binding,
			visibility: wgpu::ShaderStages::FRAGMENT,
			ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
			count: None,
		}
	}
}
