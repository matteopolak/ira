use std::{
	borrow::Cow,
	path::{Path, PathBuf},
};

use anyhow::*;

#[derive(Debug)]
pub struct Texture {
	pub texture: wgpu::Texture,
	pub view: wgpu::TextureView,
	pub sampler: wgpu::Sampler,
	pub transparent: bool,
}

/// RGBA8 image data.
pub struct Image {
	pub data: Vec<u8>,
	pub dimensions: (u32, u32),
}

impl Image {
	pub fn blend(&mut self, base: [f32; 4]) {
		for pixel in self.data.chunks_exact_mut(4) {
			for (p, b) in pixel.iter_mut().zip(base.iter()) {
				*p = (*p as f32 * b) as u8;
			}
		}
	}
}

impl TryFrom<image::DynamicImage> for Image {
	type Error = anyhow::Error;

	fn try_from(img: image::DynamicImage) -> Result<Self> {
		let rgba = img.into_rgba8();
		let dimensions = rgba.dimensions();

		Ok(Self {
			data: rgba.into_raw(),
			dimensions,
		})
	}
}

impl TryFrom<gltf::image::Data> for Image {
	type Error = anyhow::Error;

	fn try_from(data: gltf::image::Data) -> Result<Self, Self::Error> {
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

		println!("format: {:?}", data.format);

		let data = image.into_rgba8();

		let data = data.into_raw();

		let transparent = data.as_slice().chunks_exact(4).any(|pixel| pixel[3] < 240);

		println!("transparent: {}", transparent);

		Ok(Self {
			data: data,
			dimensions,
		})
	}
}

impl Texture {
	pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

	pub fn fallback(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Self> {
		Self::from_rgba8(device, queue, [255; 4])
	}

	pub fn from_rgba8(device: &wgpu::Device, queue: &wgpu::Queue, colour: [u8; 4]) -> Result<Self> {
		let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
			1,
			1,
			image::Rgba(colour),
		));

		Self::from_image(device, queue, image, Some("solid colour"))
	}

	pub fn from_rgba(device: &wgpu::Device, queue: &wgpu::Queue, colour: [f32; 4]) -> Result<Self> {
		let rgba8 = [
			(colour[0] * 255.0) as u8,
			(colour[1] * 255.0) as u8,
			(colour[2] * 255.0) as u8,
			(colour[3] * 255.0) as u8,
		];

		Self::from_rgba8(device, queue, rgba8)
	}

	pub fn from_gltf_material(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		material: &gltf::Material<'_>,
		root: &Path,
	) -> Result<Self> {
		let diffuse_texture = material
			.pbr_metallic_roughness()
			.base_color_texture()
			.map(|t| t.texture().source())
			.map(|s| gltf::image::Data::from_source(s.source(), Some(root), &[]))
			.transpose()?;

		let diffuse_colour = material.pbr_metallic_roughness().base_color_factor();
		let diffuse_texture = diffuse_texture.map_or_else(
			|| Self::from_rgba(device, queue, diffuse_colour),
			|t| {
				Image::try_from(t).and_then(|mut i| {
					// multiply alpha by colour
					i.blend(diffuse_colour);

					Self::from_image(device, queue, i, Some("mat texture"))
				})
			},
		)?;

		Ok(diffuse_texture)
	}

	pub fn from_obj_material(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		material: &tobj::Material,
		root: &Path,
	) -> Result<Self> {
		let diffuse_texture = material.diffuse_texture.as_ref();
		let diffuse_colour = material.diffuse.unwrap_or([1.0, 1.0, 1.0]);

		let diffuse_texture = diffuse_texture.map_or_else(
			|| {
				Self::from_rgba(
					device,
					queue,
					[diffuse_colour[0], diffuse_colour[1], diffuse_colour[2], 1.0],
				)
			},
			|t| {
				let path = root.join(PathBuf::from(t));
				Texture::from_path(device, queue, &path, "mat texture")
			},
		)?;

		Ok(diffuse_texture)
	}

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

	pub fn from_image<I, E>(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		image: I,
		label: Option<&str>,
	) -> Result<Self>
	where
		I: TryInto<Image, Error = E>,
		E: Into<anyhow::Error>,
	{
		let image = image.try_into().map_err(|e| e.into())?;

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

		let transparent = image.data.chunks_exact(4).any(|pixel| pixel[3] < 255);

		Ok(Self {
			texture,
			view,
			sampler,
			transparent,
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
			transparent: false,
		}
	}
}
