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
	pub image: Option<Image>,

	pub tex_coord: Option<u32>,
}

impl Texture {
	pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

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
			image: None,
			tex_coord: None,
		}
	}

	/// Creates a new texture from a solid colour.
	pub fn new_solid(device: &wgpu::Device, queue: &wgpu::Queue, rgba: [u8; 4]) -> Self {
		println!("using default");
		let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
			32,
			32,
			image::Rgba(rgba),
		));

		Self::from_image(device, queue, image.into(), Some("solid colour"))
	}

	pub fn new_solid_f32(device: &wgpu::Device, queue: &wgpu::Queue, rgba: [f32; 4]) -> Self {
		let rgba8 = [
			(rgba[0] * 255.0) as u8,
			(rgba[1] * 255.0) as u8,
			(rgba[2] * 255.0) as u8,
			(rgba[3] * 255.0) as u8,
		];

		Self::new_solid(device, queue, rgba8)
	}

	pub fn try_from_path<P: AsRef<Path>>(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		path: P,
		label: &str,
	) -> Result<Self> {
		let image = image::open(path)?;

		Ok(Self::from_image(device, queue, image.into(), Some(label)))
	}

	pub fn try_from_bytes<B>(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		bytes: B,
		label: Option<&str>,
	) -> Result<Self>
	where
		B: AsRef<[u8]>,
	{
		let image = image::load_from_memory(bytes.as_ref())?;

		Ok(Self::from_image(device, queue, image.into(), label))
	}

	pub fn from_image(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		image: Image,
		label: Option<&str>,
	) -> Self {
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

		Self {
			texture,
			view,
			sampler,
			image: Some(image),
			tex_coord: None,
		}
	}
}

/// RGBA8 image data.
#[derive(Debug)]
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

	pub fn is_transparent(&self) -> bool {
		self.data.chunks_exact(4).any(|pixel| pixel[3] < 255)
	}
}

impl From<image::DynamicImage> for Image {
	fn from(img: image::DynamicImage) -> Self {
		let rgba = img.into_rgba8();
		let dimensions = rgba.dimensions();

		Self {
			data: rgba.into_raw(),
			dimensions,
		}
	}
}

impl TryFrom<gltf::image::Data> for Image {
	type Error = anyhow::Error;

	fn try_from(data: gltf::image::Data) -> Result<Self, Self::Error> {
		let dimensions = (data.width, data.height);

		println!("format: {:?}", (data.width, data.height));

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
			data: image.into_rgba8().into_raw(),
			dimensions,
		})
	}
}

#[derive(Debug)]
pub struct Material {
	pub diffuse: Texture,
	pub normal: Texture,
	pub metallic_roughness: Texture,
	pub ao: Texture,

	pub transparent: bool,
}

impl Material {
	pub fn default(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
		let diffuse = Texture::new_solid(device, queue, [u8::MAX; 4]);
		let normal = Texture::new_solid(device, queue, [128, 128, 255, 255]);
		let metallic_roughness = Texture::new_solid(device, queue, [0, 0, 0, 255]);
		let ao = Texture::new_solid(device, queue, [255, 255, 255, 255]);

		Self {
			diffuse,
			normal,
			metallic_roughness,
			ao,
			transparent: false,
		}
	}

	pub fn create_bind_group(
		&self,
		device: &wgpu::Device,
		layout: &wgpu::BindGroupLayout,
	) -> wgpu::BindGroup {
		device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&self.diffuse.view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&self.diffuse.sampler),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::TextureView(&self.normal.view),
				},
				wgpu::BindGroupEntry {
					binding: 3,
					resource: wgpu::BindingResource::Sampler(&self.normal.sampler),
				},
				wgpu::BindGroupEntry {
					binding: 4,
					resource: wgpu::BindingResource::TextureView(&self.metallic_roughness.view),
				},
				wgpu::BindGroupEntry {
					binding: 5,
					resource: wgpu::BindingResource::Sampler(&self.metallic_roughness.sampler),
				},
				wgpu::BindGroupEntry {
					binding: 6,
					resource: wgpu::BindingResource::TextureView(&self.ao.view),
				},
				wgpu::BindGroupEntry {
					binding: 7,
					resource: wgpu::BindingResource::Sampler(&self.ao.sampler),
				},
			],
			label: None,
		})
	}

	/// Creates a new material with a solid colour.
	pub fn from_rgba8(device: &wgpu::Device, queue: &wgpu::Queue, rgba: [u8; 4]) -> Self {
		let image =
			image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(1, 1, image::Rgba(rgba)));

		let diffuse = Texture::from_image(device, queue, image.into(), Some("solid colour"));

		Self {
			diffuse,
			..Self::default(device, queue)
		}
	}

	/// Creates a new material with a solid colour, using floating point values.
	pub fn from_rgba(device: &wgpu::Device, queue: &wgpu::Queue, rgba: [f32; 4]) -> Self {
		let rgba8 = [
			(rgba[0] * 255.0) as u8,
			(rgba[1] * 255.0) as u8,
			(rgba[2] * 255.0) as u8,
			(rgba[3] * 255.0) as u8,
		];

		Self::from_rgba8(device, queue, rgba8)
	}

	pub fn from_gltf_material(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		material: &gltf::Material<'_>,
		root: &Path,
	) -> Result<Self> {
		println!("diffuse");
		let mut source = None;
		let mut diffuse_tex_coord = 0;
		let diffuse_texture = material
			.pbr_metallic_roughness()
			.base_color_texture()
			.map(|t| {
				diffuse_tex_coord = t.tex_coord();

				println!("tex coord: {:?}", t.tex_coord());

				t.texture().source()
			})
			.map(|s| {
				println!("source: {:?}", s.source());
				source = Some(s.source());
				gltf::image::Data::from_source(s.source(), Some(root), &[])
			})
			.transpose()?;

		let diffuse_colour = material.pbr_metallic_roughness().base_color_factor();
		println!("colour: {:?}", diffuse_colour);
		let mut diffuse_texture = if source.map(|s| match s {
			gltf::image::Source::Uri { uri, .. } => uri,
			_ => "",
		}) == Some("textures/GLASS_N_CHROME_baseColor.png____")
		{
			Texture::new_solid(device, queue, [255, 0, 0, 255])
		} else {
			diffuse_texture.map_or_else(
				|| Ok(Texture::new_solid_f32(device, queue, diffuse_colour)),
				|t| {
					Image::try_from(t).map(|mut i| {
						// multiply alpha by colour
						i.blend(diffuse_colour);

						Texture::from_image(device, queue, i.into(), Some("diffuse texture"))
					})
				},
			)?
		};

		diffuse_texture.tex_coord = Some(diffuse_tex_coord);

		println!("normal");

		let mut normal_tex_coord = 0;
		let normal_texture = material
			.normal_texture()
			.map(|t| {
				normal_tex_coord = t.tex_coord();
				t.texture().source()
			})
			.map(|s| {
				println!("source: {:?}", s.source());
				gltf::image::Data::from_source(s.source(), Some(root), &[])
			})
			.transpose()?;
		let mut normal_texture = normal_texture.map_or_else(
			|| Ok(Texture::new_solid(device, queue, [128, 128, 255, 255])),
			|t| {
				Image::try_from(t)
					.map(|i| Texture::from_image(device, queue, i, Some("normal map")))
			},
		)?;

		normal_texture.tex_coord = Some(normal_tex_coord);

		println!("metallic roughness");

		let mut metallic_roughness_tex_coord = 0;
		let metallic_roughness_texture = material
			.pbr_metallic_roughness()
			.metallic_roughness_texture()
			.map(|t| {
				metallic_roughness_tex_coord = t.tex_coord();
				t.texture().source()
			})
			.map(|s| {
				println!("source: {:?}", s.source());
				gltf::image::Data::from_source(s.source(), Some(root), &[])
			})
			.transpose()?;
		let mut metallic_roughness_texture = metallic_roughness_texture.map_or_else(
			|| Ok(Texture::new_solid(device, queue, [0, 0, 0, 255])),
			|t| {
				Image::try_from(t)
					.map(|i| Texture::from_image(device, queue, i, Some("metallic roughness map")))
			},
		)?;

		metallic_roughness_texture.tex_coord = Some(metallic_roughness_tex_coord);

		println!("ambient occlusion");

		let mut ao_tex_coord = 0;
		let ao_texture = material
			.occlusion_texture()
			.map(|t| {
				ao_tex_coord = t.tex_coord();

				t.texture().source()
			})
			.map(|s| {
				println!("source: {:?}", s.source());
				gltf::image::Data::from_source(s.source(), Some(root), &[])
			})
			.transpose()?;
		let mut ao_texture = ao_texture.map_or_else(
			|| Ok(Texture::new_solid(device, queue, [255; 4])),
			|t| {
				Image::try_from(t)
					.map(|i| Texture::from_image(device, queue, i, Some("ambient occlusion map")))
			},
		)?;

		ao_texture.tex_coord = Some(ao_tex_coord);

		println!("done");

		Ok(Self {
			diffuse: diffuse_texture,
			normal: normal_texture,
			metallic_roughness: metallic_roughness_texture,
			ao: ao_texture,
			transparent: material.alpha_mode() == gltf::material::AlphaMode::Blend,
		})
	}

	pub fn from_obj_material(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		material: &tobj::Material,
		root: &Path,
	) -> Result<Self> {
		todo!()
	}
}