use std::{fmt, fs, io, path::Path};

use bincode::{Decode, Encode};
use image::{buffer::ConvertBuffer, DynamicImage};
use image_dds::{Mipmaps, Surface};

use crate::{handle::Handle, DrumBuilder};

#[derive(Clone, Copy, Debug, Encode, Decode)]
pub struct Extent3d {
	pub width: u32,
	pub height: u32,
	pub depth: u32,
}

impl Extent3d {
	#[must_use]
	pub fn from_wh(width: u32, height: u32) -> Self {
		Self {
			width,
			height,
			depth: 1,
		}
	}

	#[must_use]
	pub fn from_whd(width: u32, height: u32, depth: u32) -> Self {
		Self {
			width,
			height,
			depth,
		}
	}

	#[must_use]
	pub fn len(&self) -> usize {
		self.width as usize * self.height as usize * self.depth as usize
	}

	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.width == 0 || self.height == 0 || self.depth == 0
	}
}

#[cfg(feature = "wgpu")]
impl From<Extent3d> for wgpu::Extent3d {
	fn from(value: Extent3d) -> Self {
		Self {
			width: value.width,
			height: value.height,
			depth_or_array_layers: value.depth,
		}
	}
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Encode, Decode)]
pub enum Format {
	R8Unorm,
	Rg8Unorm,
	/// Stored as RGBA, alpha channel is treated as binary.
	Rgb8A8Unorm,
	Rgba8Unorm,
	Rgba8UnormSrgb,
	Rgba32Float,
	Depth32Float,
	Bc1RgbaUnorm,
	Bc1RgbaUnormSrgb,
	Bc3RgbaUnorm,
	Bc3RgbaUnormSrgb,
	Bc4RUnorm,
	Bc5RgUnorm,
	Bc7RgbaUnorm,
	Bc7RgbaUnormSrgb,
}

impl Format {
	/// Returns the size of a single pixel in bytes.
	#[must_use]
	pub fn bytes(&self) -> usize {
		use Format::*;

		match self {
			R8Unorm => 1,
			Rg8Unorm => 2,
			Rgb8A8Unorm | Rgba8Unorm | Rgba8UnormSrgb | Depth32Float => 4,
			Bc1RgbaUnorm | Bc1RgbaUnormSrgb | Bc4RUnorm => 8,
			Rgba32Float | Bc5RgUnorm | Bc3RgbaUnorm | Bc3RgbaUnormSrgb | Bc7RgbaUnorm
			| Bc7RgbaUnormSrgb => 16,
		}
	}

	#[must_use]
	pub fn to_compressed(&self) -> Option<Self> {
		use Format::*;

		Some(match self {
			R8Unorm => Bc4RUnorm,
			Rg8Unorm => Bc5RgUnorm,
			Rgb8A8Unorm => Bc1RgbaUnorm,
			Rgba8Unorm => Bc7RgbaUnorm,
			Rgba8UnormSrgb => Bc7RgbaUnormSrgb,
			_ => return None,
		})
	}

	#[must_use]
	pub fn to_srgb(&self) -> Self {
		use Format::*;

		match self {
			Rgba8Unorm => Rgba8UnormSrgb,
			Bc1RgbaUnorm => Bc1RgbaUnormSrgb,
			Bc3RgbaUnorm => Bc3RgbaUnormSrgb,
			Bc7RgbaUnorm => Bc7RgbaUnormSrgb,
			_ => *self,
		}
	}
}

impl From<Format> for image::ColorType {
	fn from(value: Format) -> Self {
		use image::ColorType as C;
		use Format as F;

		match value {
			F::R8Unorm => C::L8,
			F::Rg8Unorm => C::La8,
			F::Rgb8A8Unorm | F::Rgba8Unorm | F::Rgba8UnormSrgb => C::Rgba8,
			F::Rgba32Float => C::Rgba32F,
			_ => unreachable!(),
		}
	}
}

impl TryFrom<image::ColorType> for Format {
	type Error = ();

	fn try_from(value: image::ColorType) -> Result<Self, Self::Error> {
		use image::ColorType as C;
		use Format as F;

		match value {
			C::L8 => Ok(F::R8Unorm),
			C::La8 => Ok(F::Rg8Unorm),
			C::Rgba8 => Ok(F::Rgba8Unorm),
			C::Rgba32F => Ok(F::Rgba32Float),
			_ => Err(()),
		}
	}
}

impl TryFrom<Format> for image_dds::ImageFormat {
	type Error = ();

	fn try_from(value: Format) -> Result<Self, Self::Error> {
		use image_dds::ImageFormat as I;
		use Format as F;

		Ok(match value {
			F::R8Unorm => I::R8Unorm,
			F::Rgba8Unorm => I::Rgba8Unorm,
			F::Rgba8UnormSrgb => I::Rgba8UnormSrgb,
			F::Rgba32Float => I::Rgba32Float,
			F::Bc1RgbaUnorm => I::BC1RgbaUnorm,
			F::Bc1RgbaUnormSrgb => I::BC1RgbaUnormSrgb,
			F::Bc3RgbaUnorm => I::BC3RgbaUnorm,
			F::Bc3RgbaUnormSrgb => I::BC3RgbaUnormSrgb,
			F::Bc4RUnorm => I::BC4RUnorm,
			F::Bc5RgUnorm => I::BC5RgUnorm,
			F::Bc7RgbaUnorm => I::BC7RgbaUnorm,
			F::Bc7RgbaUnormSrgb => I::BC7RgbaUnormSrgb,
			_ => return Err(()),
		})
	}
}

#[cfg(feature = "gltf")]
impl TryFrom<gltf::image::Format> for Format {
	type Error = ();

	fn try_from(value: gltf::image::Format) -> Result<Self, Self::Error> {
		use gltf::image::Format as G;
		use Format as F;

		match value {
			G::R8 => Ok(F::R8Unorm),
			G::R8G8 => Ok(F::Rg8Unorm),
			G::R8G8B8A8 => Ok(F::Rgba8Unorm),
			G::R32G32B32A32FLOAT => Ok(F::Rgba32Float),
			_ => Err(()),
		}
	}
}

#[cfg(feature = "wgpu")]
impl From<Format> for wgpu::TextureFormat {
	fn from(value: Format) -> Self {
		use wgpu::TextureFormat as W;
		use Format as F;

		match value {
			F::R8Unorm => W::R8Unorm,
			F::Rg8Unorm => W::Rg8Unorm,
			F::Rgba8Unorm | F::Rgb8A8Unorm => W::Rgba8Unorm,
			F::Rgba8UnormSrgb => W::Rgba8UnormSrgb,
			F::Rgba32Float => W::Rgba32Float,
			F::Depth32Float => W::Depth32Float,
			F::Bc1RgbaUnorm => W::Bc1RgbaUnorm,
			F::Bc1RgbaUnormSrgb => W::Bc1RgbaUnormSrgb,
			F::Bc3RgbaUnorm => W::Bc3RgbaUnorm,
			F::Bc3RgbaUnormSrgb => W::Bc3RgbaUnormSrgb,
			F::Bc4RUnorm => W::Bc4RUnorm,
			F::Bc5RgUnorm => W::Bc5RgUnorm,
			F::Bc7RgbaUnorm => W::Bc7RgbaUnorm,
			F::Bc7RgbaUnormSrgb => W::Bc7RgbaUnormSrgb,
		}
	}
}

#[derive(Encode, Decode)]
pub struct Texture {
	pub extent: Extent3d,
	pub format: Format,
	pub mipmaps: u32,
	pub data: Box<[u8]>,
}

impl fmt::Debug for Texture {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Texture")
			.field("extent", &self.extent)
			.field("format", &self.format)
			.field("mipmaps", &self.mipmaps)
			.field("data", &format_args!("Box<[u8; {}]>", self.data.len()))
			.finish()
	}
}

impl Texture {
	pub const DEFAULT_EXTENT: Extent3d = Extent3d {
		width: 32,
		height: 32,
		depth: 1,
	};

	pub fn into_handle(self, drum: &mut DrumBuilder) -> Handle<Texture> {
		Handle::from_vec(&mut drum.textures, self)
	}

	#[must_use]
	#[inline]
	pub fn default_normal() -> Self {
		Self::from_rgba_pixel([128, 128, 255, 255])
	}

	#[must_use]
	#[inline]
	pub fn default_metallic_roughness() -> Self {
		Self::from_rgba_pixel([128, 128, 255, 255])
	}

	#[must_use]
	#[inline]
	pub fn default_ao() -> Self {
		Self::from_r_pixel(255)
	}

	#[must_use]
	#[inline]
	pub fn default_emissive() -> Self {
		Self::from_rgb_pixel([255, 255, 255])
	}

	#[must_use]
	pub fn from_rgba_pixel(pixel: [u8; 4]) -> Self {
		let extent = Self::DEFAULT_EXTENT;
		let data = vec![pixel; extent.len()]
			.into_flattened()
			.into_boxed_slice();

		Self {
			extent,
			format: Format::Rgba8Unorm,
			mipmaps: 1,
			data,
		}
	}

	#[must_use]
	pub fn from_rgba_pixel_f32(pixel: [f32; 4]) -> Self {
		let pixel = pixel.map(|p| (p * 255.0) as u8);

		Self::from_rgba_pixel(pixel)
	}

	#[must_use]
	pub fn from_rgb_pixel(pixel: [u8; 3]) -> Self {
		let pixel = [pixel[0], pixel[1], pixel[2], 255];

		let extent = Self::DEFAULT_EXTENT;
		let data = vec![pixel; extent.len()]
			.into_flattened()
			.into_boxed_slice();

		Self {
			extent,
			format: Format::Rgba8Unorm,
			mipmaps: 1,
			data,
		}
	}

	#[must_use]
	pub fn from_rgb_pixel_f32(pixel: [f32; 3]) -> Self {
		let pixel = pixel.map(|p| (p * 255.0) as u8);

		Self::from_rgb_pixel(pixel)
	}

	#[must_use]
	pub fn from_rg_pixel(pixel: [u8; 2]) -> Self {
		let extent = Self::DEFAULT_EXTENT;
		let data = vec![pixel; extent.len()]
			.into_flattened()
			.into_boxed_slice();

		Self {
			extent,
			format: Format::Rg8Unorm,
			mipmaps: 1,
			data,
		}
	}

	#[must_use]
	pub fn from_rg_pixel_f32(pixel: [f32; 2]) -> Self {
		let pixel = pixel.map(|p| (p * 255.0) as u8);

		Self::from_rg_pixel(pixel)
	}

	#[must_use]
	pub fn from_r_pixel(pixel: u8) -> Self {
		let extent = Self::DEFAULT_EXTENT;
		let data = vec![pixel; extent.len()].into_boxed_slice();

		Self {
			extent,
			format: Format::R8Unorm,
			mipmaps: 1,
			data,
		}
	}

	#[must_use]
	pub fn from_r_pixel_f32(pixel: f32) -> Self {
		let pixel = (pixel * 255.0) as u8;

		Self::from_r_pixel(pixel)
	}

	/// Compresses the texture using `BCn` if possible.
	///
	/// # Errors
	///
	/// See [`image_dds::SurfaceRgba8::encode`].
	#[tracing::instrument(skip(mipmaps))]
	pub fn compress<F>(&mut self, mipmaps: F) -> Result<(), image_dds::error::SurfaceError>
	where
		F: Fn(Extent3d) -> Mipmaps,
	{
		let Some(format) = self.format.to_compressed() else {
			return Ok(());
		};

		let Ok(surface_format) = self.format.try_into() else {
			return Ok(());
		};

		let Ok(compressed_format) = format.try_into() else {
			return Ok(());
		};

		let surface = Surface {
			width: self.extent.width,
			height: self.extent.height,
			data: self.data.as_ref(),
			depth: 1,
			layers: self.extent.depth,
			mipmaps: 1,
			image_format: surface_format,
		};

		let surface = surface.encode(
			compressed_format,
			image_dds::Quality::Normal,
			mipmaps(self.extent),
		)?;

		self.format = format;
		self.data = surface.data.into_boxed_slice();
		self.mipmaps = surface.mipmaps;

		Ok(())
	}
}

#[cfg(feature = "gltf")]
impl Texture {
	#[must_use]
	#[tracing::instrument]
	pub fn from_gltf_data(data: gltf::image::Data) -> Self {
		use image::buffer::ConvertBuffer;

		let extent = Extent3d::from_wh(data.width, data.height);

		let (data, format) = match data.format.try_into() {
			Ok(format) => (data.pixels, format),
			Err(..) if data.format == gltf::image::Format::R8G8B8 => {
				let image: image::RgbaImage =
					image::RgbImage::from_vec(data.width, data.height, data.pixels)
						.expect("invalid image data")
						.convert();

				(image.into_raw(), Format::Rgb8A8Unorm)
			}
			Err(..) => panic!("unsupported image format"),
		};

		Self {
			extent,
			format,
			mipmaps: 1,
			data: data.into_boxed_slice(),
		}
	}

	/// Loads a texture from a file.
	///
	/// # Errors
	///
	/// See [`image::open`].
	pub fn from_path<P: AsRef<Path>>(path: P) -> image::ImageResult<Self> {
		let image = image::open(path)?;

		Ok(Self::from_image(image))
	}

	/// Converts a dynamic image into a texture.
	///
	/// # Panics
	///
	/// Panics if the image format is not supported.
	#[must_use]
	#[tracing::instrument]
	pub fn from_image(image: DynamicImage) -> Self {
		let extent = Extent3d::from_wh(image.width(), image.height());
		let (data, format) = match image {
			DynamicImage::ImageLuma8(image) => (image.into_raw(), Format::R8Unorm),
			DynamicImage::ImageLumaA8(image) => (image.into_raw(), Format::Rg8Unorm),
			DynamicImage::ImageRgb8(image) => {
				let image: image::RgbaImage = image.convert();

				(image.into_raw(), Format::Rgba8Unorm)
			}
			DynamicImage::ImageRgba8(image) => (image.into_raw(), Format::Rgba8Unorm),
			DynamicImage::ImageRgb32F(image) => {
				(bytemuck::cast_slice(&image).to_vec(), Format::Rgba32Float)
			}
			_ => panic!("unsupported image format"),
		};

		Self {
			extent,
			format,
			mipmaps: 1,
			data: data.into_boxed_slice(),
		}
	}

	/// Converts a texture into a cubemap texture with 6 layers.
	///
	/// Four formats are supported:
	///
	///    +Y          +X
	/// -X +Z +X -Z    -X
	///    -Y          +Y
	///                -Y
	///    +Y          +Z
	/// -X +Z +X       -Z
	///    -Y
	///    -Z
	///
	/// +X -X +Y -Y +Z -Z
	///
	/// The output texture concatenates the layers in the following order:
	/// +X, -X, +Y, -Y, +Z, -Z.
	///
	/// # Errors
	///
	/// Returns an error if the input texture does not have the correct dimensions.
	#[tracing::instrument]
	pub fn into_cubemap(self, n: u32) -> Result<Self, ()> {
		const TOP_LEFT_SIDE_CROSS: [(usize, usize); 6] = [
			(2, 1), // +X
			(0, 1), // -X
			(1, 0), // +Y
			(1, 2), // -Y
			(1, 1), // +Z
			(3, 1), // -Z
		];

		const TOP_LEFT_CROSS: [(usize, usize); 6] = [
			(2, 1), // +X
			(0, 1), // -X
			(1, 0), // +Y
			(1, 2), // -Y
			(1, 1), // +Z
			(1, 3), // -Z
		];

		const TOP_LEFT_LINE: [(usize, usize); 6] = [
			(0, 0), // +X
			(1, 0), // -X
			(2, 0), // +Y
			(3, 0), // -Y
			(4, 0), // +Z
			(5, 0), // -Z
		];

		const TOP_LEFT_POLE: [(usize, usize); 6] = [
			(0, 0), // +X
			(0, 1), // -X
			(0, 2), // +Y
			(0, 3), // -Y
			(0, 4), // +Z
			(0, 5), // -Z
		];

		let w = self.extent.width;
		let h = self.extent.height;

		// check for the first one
		let (top_left, w, h) = if w / 4 == h / 3 {
			(TOP_LEFT_SIDE_CROSS, w / 4, h / 3)
		}
		// check for the second one
		else if w / 3 == h / 4 {
			(TOP_LEFT_CROSS, w / 3, h / 4)
		}
		// check for the third one
		else if w == h / 6 {
			(TOP_LEFT_POLE, w, w)
		}
		// check for the fourth one
		else if w / 6 == h {
			(TOP_LEFT_LINE, h, h)
		} else {
			return Err(());
		};

		let (w, h) = (w as usize, h as usize);
		let bytes_per_pixel = self.format.bytes();
		let mut data = Vec::with_capacity(w * h * 6 * bytes_per_pixel);

		for (x, y) in top_left {
			let x = x * w;
			let y = y * h;

			for j in 0..h {
				let src = (x + (y + j) * self.extent.width as usize) * bytes_per_pixel;

				data.extend_from_slice(&self.data[src..src + w * bytes_per_pixel]);
			}
		}

		Ok(Self {
			extent: Extent3d::from_whd(w as u32, h as u32, 6),
			format: self.format,
			mipmaps: 1,
			data: data.into_boxed_slice(),
		})
	}
}

#[derive(Debug, Encode, Decode)]
pub struct Material {
	pub albedo: Handle<Texture>,
	pub normal: Option<Handle<Texture>>,
	pub metallic_roughness: Option<Handle<Texture>>,
	pub ao: Option<Handle<Texture>>,
	pub emissive: Option<Handle<Texture>>,

	pub transparent: bool,
}

#[cfg(feature = "gltf")]
impl Material {
	/// Creates a new material from a glTF material.
	///
	/// # Errors
	///
	/// Returns an error if the material's textures cannot be loaded.
	/// See [`gltf::image::Data::from_source`] for more information.
	#[tracing::instrument]
	pub fn from_gltf_material(
		drum: &mut DrumBuilder,
		material: &gltf::Material<'_>,
		root: &Path,
	) -> Result<Self, gltf::Error> {
		let albedo_texture = material
			.pbr_metallic_roughness()
			.base_color_texture()
			.map(|t| t.texture().source())
			.map(|s| gltf::image::Data::from_source(s.source(), Some(root), &[]))
			.transpose()?;

		let albedo_colour = material.pbr_metallic_roughness().base_color_factor();
		let albedo = albedo_texture.map_or_else(
			|| Texture::from_rgba_pixel_f32(albedo_colour),
			Texture::from_gltf_data,
		);

		let normal = material
			.normal_texture()
			.map(|t| t.texture().source())
			.map(|s| gltf::image::Data::from_source(s.source(), Some(root), &[]))
			.transpose()?
			.map(Texture::from_gltf_data);

		let metallic_roughness = material.pbr_metallic_roughness();
		let metallic_factor = metallic_roughness.metallic_factor();
		let roughness_factor = metallic_roughness.roughness_factor();

		let metallic_roughness = metallic_roughness
			.metallic_roughness_texture()
			.map(|t| t.texture().source())
			.map(|s| gltf::image::Data::from_source(s.source(), Some(root), &[]))
			.transpose()?;

		// roughness = G
		// metallic = B (will be written to R)
		let metallic_roughness = metallic_roughness.map_or_else(
			|| Texture::from_rg_pixel_f32([metallic_factor, roughness_factor]),
			Texture::from_gltf_data,
		);

		let ao = material
			.occlusion_texture()
			.map(|t| t.texture().source())
			.map(|s| gltf::image::Data::from_source(s.source(), Some(root), &[]))
			.transpose()?;
		let ao = ao.map(Texture::from_gltf_data);

		let emissive = material.emissive_factor();
		let emissive_texture = material
			.emissive_texture()
			.map(|t| t.texture().source())
			.map(|s| gltf::image::Data::from_source(s.source(), Some(root), &[]))
			.transpose()?;
		let emissive = emissive_texture.map_or_else(
			|| Texture::from_rgb_pixel_f32(emissive),
			Texture::from_gltf_data,
		);

		Ok(Self {
			albedo: albedo.into_handle(drum),
			normal: normal.map(|t| t.into_handle(drum)),
			metallic_roughness: Some(metallic_roughness.into_handle(drum)),
			ao: ao.map(|t| t.into_handle(drum)),
			emissive: Some(emissive.into_handle(drum)),
			transparent: material.alpha_mode() == gltf::material::AlphaMode::Blend,
		})
	}
}
