use std::{borrow::Cow, path::Path};

use anyhow::*;

pub struct Texture {
	pub texture: wgpu::Texture,
	pub view: wgpu::TextureView,
	pub sampler: wgpu::Sampler,
}

/// RGBA8 image data.
pub struct Image<'a> {
	pub data: Cow<'a, [u8]>,
	pub dimensions: (u32, u32),
}

impl TryFrom<image::DynamicImage> for Image<'static> {
	type Error = anyhow::Error;

	fn try_from(img: image::DynamicImage) -> Result<Self> {
		let rgba = img.into_rgba8();
		let dimensions = rgba.dimensions();

		Ok(Self {
			data: Cow::Owned(rgba.into_raw()),
			dimensions,
		})
	}
}

impl<'a> TryFrom<gltf::Image<'a>> for Image<'a> {
	type Error = anyhow::Error;

	fn try_from(img: gltf::Image) -> Result<Self, Self::Error> {
		let data = gltf::image::Data::from_source(img.source(), None, &[])?;
		let dimensions = (data.width, data.height);

		let image = match data.format {
			gltf::image::Format::R8 => image::DynamicImage::ImageLuma8(
				image::ImageBuffer::from_vec(data.width, data.height, data.pixels)
					.ok_or_else(|| anyhow!("invalid image dimensions"))?,
			),
			gltf::image::Format::R8G8 => image::DynamicImage::ImageLumaA8(
				image::ImageBuffer::from_vec(data.width, data.height, data.pixels)
					.ok_or_else(|| anyhow!("invalid image dimensions"))?,
			),
			gltf::image::Format::R8G8B8 => image::DynamicImage::ImageRgb8(
				image::ImageBuffer::from_vec(data.width, data.height, data.pixels)
					.ok_or_else(|| anyhow!("invalid image dimensions"))?,
			),
			gltf::image::Format::R8G8B8A8 => image::DynamicImage::ImageRgba8(
				image::ImageBuffer::from_vec(data.width, data.height, data.pixels)
					.ok_or_else(|| anyhow!("invalid image dimensions"))?,
			),
			_ => return Err(anyhow!("unsupported image format")),
		};

		Ok(Self {
			data: Cow::Owned(image.into_rgba8().into_raw()),
			dimensions,
		})
	}
}

impl Texture {
	pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

	pub fn from_path<P: AsRef<Path>>(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		path: P,
		label: &str,
	) -> Result<Self> {
		let image = image::open(path)?;

		Self::from_image(device, queue, image, Some(label))
	}

	pub fn from_bytes<B>(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		bytes: B,
		label: Option<&str>,
	) -> Result<Self>
	where
		B: AsRef<[u8]>,
	{
		let image = image::load_from_memory(bytes.as_ref())?;

		Self::from_image(device, queue, image, label)
	}

	pub fn from_image<'a, I>(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		image: I,
		label: Option<&str>,
	) -> Result<Self>
	where
		I: TryInto<Image<'a>, Error = anyhow::Error>,
	{
		let image = image.try_into()?;

		let size = wgpu::Extent3d {
			width: image.dimensions.0,
			height: image.dimensions.1,
			depth_or_array_layers: 1,
		};
		let texture = device.create_texture(&wgpu::TextureDescriptor {
			label,
			size,
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba8UnormSrgb,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			view_formats: &[],
		});

		queue.write_texture(
			wgpu::ImageCopyTexture {
				aspect: wgpu::TextureAspect::All,
				texture: &texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
			},
			image.data.as_ref(),
			wgpu::ImageDataLayout {
				offset: 0,
				bytes_per_row: Some(4 * image.dimensions.0),
				rows_per_image: Some(image.dimensions.1),
			},
			size,
		);

		let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		});

		Ok(Self {
			texture,
			view,
			sampler,
		})
	}

	pub fn create_depth_texture(
		device: &wgpu::Device,
		config: &wgpu::SurfaceConfiguration,
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
			sample_count: 1,
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
		}
	}
}
