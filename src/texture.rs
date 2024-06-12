use std::{borrow::Cow, path::Path};

use anyhow::{anyhow, Ok, Result};
use glam::{Vec2, Vec3};
use image::GenericImageView;
use wgpu::util::DeviceExt;

use crate::Vertex;

#[must_use]
#[derive(Debug)]
pub struct Texture {
	pub texture: wgpu::Texture,
	pub view: wgpu::TextureView,
	pub sampler: wgpu::Sampler,
}

impl Texture {
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
		}
	}

	fn write_image_texture(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		image: &Image,
	) -> wgpu::Texture {
		let is_cube = image.array_layers == 6;
		let dimensions = if is_cube {
			let side = image.dimensions.0 / if image.mip_levels.is_some() { 8 } else { 4 };

			(side, side)
		} else {
			(image.dimensions.0, image.dimensions.1)
		};

		let size = wgpu::Extent3d {
			width: dimensions.0,
			height: dimensions.1,
			depth_or_array_layers: image.array_layers,
		};

		let buf = image.create_image_buffer();

		device.create_texture_with_data(
			queue,
			&wgpu::TextureDescriptor {
				label: None,
				size,
				mip_level_count: image.mip_levels.unwrap_or(1),
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: match image.data {
					ImageData::Rgba8(..) => wgpu::TextureFormat::Rgba8UnormSrgb,
					ImageData::Rgba32F(..) => wgpu::TextureFormat::Rgba32Float,
				},
				usage: wgpu::TextureUsages::TEXTURE_BINDING
					| wgpu::TextureUsages::COPY_DST
					| wgpu::TextureUsages::RENDER_ATTACHMENT,
				view_formats: &[],
			},
			wgpu::util::TextureDataOrder::MipMajor,
			&buf,
		)
	}

	/// Creates a new texture from an image.
	///
	/// # Panics
	/// Panics if the image format is not `Rgba8`, `Rgba16`, or `Rgba32F`.
	pub fn from_image(device: &wgpu::Device, queue: &wgpu::Queue, image: &Image) -> Self {
		let texture = Self::write_image_texture(device, queue, image);

		let view = texture.create_view(&wgpu::TextureViewDescriptor {
			label: None,
			dimension: Some(image.view_dimension),
			mip_level_count: image.mip_levels,
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
			lod_max_clamp: image.mip_levels.unwrap_or(1) as f32,
			anisotropy_clamp: 16,
			..Default::default()
		});

		Self {
			texture,
			view,
			sampler,
		}
	}

	/// Creates a new texture from an HDR image file.
	///
	/// # Errors
	/// See [`Texture::try_from_path`] for more information.
	pub fn try_from_hdr_path<P: AsRef<Path>>(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		path: P,
	) -> Result<Self> {
		let image = image::open(path)?.into_rgba32f();
		let dimensions = image.dimensions();
		let image = Image {
			data: ImageData::Rgba32F(image),
			dimensions,
			..Default::default()
		};

		Ok(Self::from_image(device, queue, &image))
	}
}

#[derive(Debug)]
pub struct Image {
	pub data: ImageData,
	pub dimensions: (u32, u32),
	pub view_dimension: wgpu::TextureViewDimension,
	pub mip_levels: Option<u32>,
	pub array_layers: u32,
}

impl Default for Image {
	fn default() -> Self {
		Self {
			data: ImageData::Rgba8(image::RgbaImage::new(0, 0)),
			dimensions: (0, 0),
			view_dimension: wgpu::TextureViewDimension::D2,
			mip_levels: None,
			array_layers: 1,
		}
	}
}

#[derive(Debug)]
pub enum ImageData {
	Rgba8(image::RgbaImage),
	Rgba32F(image::Rgba32FImage),
}

impl AsRef<[u8]> for ImageData {
	fn as_ref(&self) -> &[u8] {
		match self {
			ImageData::Rgba8(data) => data,
			ImageData::Rgba32F(data) => bytemuck::cast_slice(data),
		}
	}
}

impl Image {
	pub fn blend(&mut self, base: [f32; 4]) {
		let ImageData::Rgba8(data) = &mut self.data else {
			return;
		};

		for pixel in data.chunks_exact_mut(4) {
			for (p, b) in pixel.iter_mut().zip(base.iter()) {
				*p = (f32::from(*p) * b) as u8;
			}
		}
	}

	#[must_use]
	pub fn mip_size(&self, mip: u32) -> (u32, u32) {
		let (width, height) = self.dimensions;

		((width >> mip).max(1), (height >> mip).max(1))
	}

	#[must_use]
	pub fn mip_data(&self, mip: u32, layer: u32, layers: u32) -> Cow<[u8]> {
		let ImageData::Rgba8(data) = &self.data else {
			return Cow::Borrowed(&[]);
		};

		// width and height of the texture
		let (_width, height) = self.mip_size(mip);

		// for no layers, return it all
		if layer == 0 && layers == 1 {
			Cow::Borrowed(data)
		} else {
			// for more than 1 layer, assume cubemap
			// the faces are layed out like
			//
			//    +Y
			// -X +Z +X -Z
			//    -Y
			//
			// order:
			// +X, -X, +Y, -Y, +Z, -Z
			const TOP_LEFT: [(u32, u32); 6] = [
				(2, 1), // +X
				(0, 1), // -X
				(1, 0), // +Y
				(1, 2), // -Y
				(1, 1), // +Z
				(3, 1), // -Z
			];

			// we need to offset the X by the previous mip levels
			// we know the aspect ratio is 4:3 (4 squares across, 3 up)
			// so we can simply get mip_0 width with height / 3 * 4, then
			// keep adding that divided by 2 until we reach this mip level
			// using the geometric sum of mip * (1 + 1/2 + 1/4 + ...)

			let a = (self.mip_size(0).1 / 3 * 4) as f32;
			let r: f32 = 1.0 / 2.0;
			#[allow(clippy::cast_possible_wrap)]
			let offset_x = (a * (1.0 - r.powi(mip as i32)) / (1.0 - r)) as u32;

			let side = height / 3;
			let (width, height) = (side, side);

			let (x, y) = TOP_LEFT[layer as usize];
			let data = data
				.view(offset_x + x * width, y * height, width, height)
				.to_image()
				.to_vec();

			Cow::Owned(data)
		}
	}

	/// Creates a packed image buffer of all mip levels and cube faces.
	#[must_use]
	pub fn create_image_buffer(&self) -> Vec<u8> {
		let mip_levels = self.mip_levels.unwrap_or(1);

		// 4 options:
		// - 1 mip level, 1 face
		// - 1 mip level, 6 faces
		// - n mip levels, 1 face
		// - n mip levels, 6 faces

		let mut buf = Vec::new();

		for mip in 0..mip_levels {
			for layer in 0..self.array_layers {
				let data = self.mip_data(mip, layer, self.array_layers);

				buf.extend_from_slice(data.as_ref());
			}
		}

		buf
	}

	pub fn to_texture(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Texture {
		Texture::from_image(device, queue, self)
	}

	#[must_use]
	pub fn is_transparent(&self) -> bool {
		let ImageData::Rgba8(data) = &self.data else {
			return false;
		};

		data.chunks_exact(4).any(|pixel| pixel[3] < 255)
	}

	/// Creates a new texture from a solid colour.
	#[must_use]
	pub fn new_solid(rgba: [u8; 4]) -> Self {
		image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(32, 32, image::Rgba(rgba)))
			.into()
	}

	#[must_use]
	pub fn new_solid_f32(rgba: [f32; 4]) -> Self {
		let rgba8 = [
			(rgba[0] * 255.0) as u8,
			(rgba[1] * 255.0) as u8,
			(rgba[2] * 255.0) as u8,
			(rgba[3] * 255.0) as u8,
		];

		Self::new_solid(rgba8)
	}

	/// Creates a new texture from an image file.
	///
	/// # Errors
	/// Returns an error if the image file cannot be read or is in an unsupported format.
	pub fn try_from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
		Ok(image::open(path)?.into())
	}

	/// Creates a new texture from a byte slice.
	///
	/// The byte slice should contain RGBA8 image data, but other formats will be attempted.
	///
	/// # Errors
	/// Returns an error if the byte slice cannot be converted into an image.
	pub fn try_from_bytes<B>(bytes: B) -> Result<Self>
	where
		B: AsRef<[u8]>,
	{
		Ok(image::load_from_memory(bytes.as_ref())?.into())
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
	#[must_use]
	pub fn default(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
		let diffuse = Image::new_solid([u8::MAX; 4]).to_texture(device, queue);
		let normal = Image::new_solid([128, 128, 255, 255]).to_texture(device, queue);
		let metallic_roughness = Image::new_solid([0, 0, 0, 255]).to_texture(device, queue);
		let ao = Image::new_solid([u8::MAX; 4]).to_texture(device, queue);

		Self {
			diffuse,
			normal,
			metallic_roughness,
			ao,
			transparent: false,
		}
	}

	#[must_use]
	pub fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
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
			],
			label: Some("texture_bind_group_layout"),
		})
	}

	#[must_use]
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
	#[must_use]
	pub fn from_rgba8(device: &wgpu::Device, queue: &wgpu::Queue, rgba: [u8; 4]) -> Self {
		let image =
			image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(1, 1, image::Rgba(rgba)));

		let diffuse = Texture::from_image(device, queue, &image.into());

		Self {
			diffuse,
			..Self::default(device, queue)
		}
	}

	/// Creates a new material with a solid colour, using floating point values.
	#[must_use]
	pub fn from_rgba(device: &wgpu::Device, queue: &wgpu::Queue, rgba: [f32; 4]) -> Self {
		let rgba8 = [
			(rgba[0] * 255.0) as u8,
			(rgba[1] * 255.0) as u8,
			(rgba[2] * 255.0) as u8,
			(rgba[3] * 255.0) as u8,
		];

		Self::from_rgba8(device, queue, rgba8)
	}

	/// Creates a new material from a glTF material.
	///
	/// The material's textures are loaded from the same directory as the glTF file.
	///
	/// The following properties are currently supported:
	/// - Base colour texture
	/// - Normal texture
	/// - Metallic roughness texture
	/// - Ambient occlusion texture
	/// - Alpha mode
	///
	/// # Errors
	/// Returns an error if the material's textures cannot be loaded.
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
			|| Ok(Image::new_solid_f32(diffuse_colour).to_texture(device, queue)),
			|t| {
				Image::try_from(t).map(|mut i| {
					// multiply alpha by colour
					i.blend(diffuse_colour);

					Texture::from_image(device, queue, &i)
				})
			},
		)?;

		let normal_texture = material
			.normal_texture()
			.map(|t| t.texture().source())
			.map(|s| gltf::image::Data::from_source(s.source(), Some(root), &[]))
			.transpose()?;
		let normal_texture = normal_texture.map_or_else(
			|| Ok(Image::new_solid([128, 128, 255, 255]).to_texture(device, queue)),
			|t| Image::try_from(t).map(|i| Texture::from_image(device, queue, &i)),
		)?;

		let metallic_roughness = material.pbr_metallic_roughness();
		let metallic_factor = metallic_roughness.metallic_factor();
		let roughness_factor = metallic_roughness.roughness_factor();

		let metallic_roughness_texture = metallic_roughness
			.metallic_roughness_texture()
			.map(|t| t.texture().source())
			.map(|s| gltf::image::Data::from_source(s.source(), Some(root), &[]))
			.transpose()?;
		let metallic_roughness_texture = metallic_roughness_texture.map_or_else(
			|| {
				Ok(
					Image::new_solid_f32([0.0, roughness_factor, metallic_factor, 1.0])
						.to_texture(device, queue),
				)
			},
			|t| Image::try_from(t).map(|i| Texture::from_image(device, queue, &i)),
		)?;

		let ao_texture = material
			.occlusion_texture()
			.map(|t| t.texture().source())
			.map(|s| gltf::image::Data::from_source(s.source(), Some(root), &[]))
			.transpose()?;
		let ao_texture = ao_texture.map_or_else(
			|| Ok(Image::new_solid([255; 4]).to_texture(device, queue)),
			|t| Image::try_from(t).map(|i| Texture::from_image(device, queue, &i)),
		)?;

		Ok(Self {
			diffuse: diffuse_texture,
			normal: normal_texture,
			metallic_roughness: metallic_roughness_texture,
			ao: ao_texture,
			transparent: material.alpha_mode() == gltf::material::AlphaMode::Blend,
		})
	}
}

pub fn compute_tangents(vertices: &mut [Vertex], indices: &[u32]) {
	// compute tangents and bitangents
	for i in 0..indices.len() / 3 {
		let i0 = indices[i * 3] as usize;
		let i1 = indices[i * 3 + 1] as usize;
		let i2 = indices[i * 3 + 2] as usize;

		let v0 = &vertices[i0];
		let v1 = &vertices[i1];
		let v2 = &vertices[i2];

		let delta_pos1 = v1.position - v0.position;
		let delta_pos2 = v2.position - v0.position;

		let delta_uv1 = v1.tex_coords - v0.tex_coords;
		let delta_uv2 = v2.tex_coords - v0.tex_coords;

		let tangent = if delta_uv1 == Vec2::ZERO || delta_uv2 == Vec2::ZERO {
			// Calculate a default tangent using the normal
			let normal = v0.normal;
			let auxiliary_vector = if normal.y.abs() > 0.99 {
				Vec3::X
			} else {
				Vec3::Y
			};

			normal.cross(auxiliary_vector).normalize()
		} else {
			let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
			(delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r
		};

		vertices[i0].tangent += tangent;
		vertices[i1].tangent += tangent;
		vertices[i2].tangent += tangent;
	}

	for v in vertices {
		v.tangent = v.tangent.normalize();
	}
}
