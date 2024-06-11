use std::path::Path;

use bincode::{Decode, Encode};

use crate::handle::Handle;

#[derive(Clone, Copy, Debug, Encode, Decode)]
pub struct Extent3d {
	pub width: u16,
	pub height: u16,
	pub depth: u8,
}

impl Extent3d {
	pub fn from_wh(width: u16, height: u16) -> Self {
		Self {
			width,
			height,
			depth: 1,
		}
	}

	pub fn len(&self) -> usize {
		self.width as usize * self.height as usize * self.depth as usize
	}
}

#[cfg(feature = "wgpu")]
impl From<Extent3d> for wgpu::Extent3d {
	fn from(value: Extent3d) -> Self {
		Self {
			width: u32::from(value.width),
			height: u32::from(value.height),
			depth_or_array_layers: u32::from(value.depth),
		}
	}
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Encode, Decode)]
pub enum Format {
	R8Unorm,
	Rg8Unorm,
	Rgba8Unorm,
	Rgba8UnormSrgb,
	Rgba32Float,
	Depth32Float,
	Bc1RgbaUnorm,
	Bc1RgbaUnormSrgb,
	Bc2RgbaUnorm,
	Bc2RgbaUnormSrgb,
	Bc3RgbaUnorm,
	Bc3RgbaUnormSrgb,
	Bc4RUnorm,
	Bc5RgUnorm,
	Bc7RgbaUnorm,
	Bc7RgbaUnormSrgb,
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
			F::Rgba8Unorm => W::Rgba8Unorm,
			F::Rgba8UnormSrgb => W::Rgba8UnormSrgb,
			F::Rgba32Float => W::Rgba32Float,
			F::Depth32Float => W::Depth32Float,
			F::Bc1RgbaUnorm => W::Bc1RgbaUnorm,
			F::Bc1RgbaUnormSrgb => W::Bc1RgbaUnormSrgb,
			F::Bc2RgbaUnorm => W::Bc2RgbaUnorm,
			F::Bc2RgbaUnormSrgb => W::Bc2RgbaUnormSrgb,
			F::Bc3RgbaUnorm => W::Bc3RgbaUnorm,
			F::Bc3RgbaUnormSrgb => W::Bc3RgbaUnormSrgb,
			F::Bc4RUnorm => W::Bc4RUnorm,
			F::Bc5RgUnorm => W::Bc5RgUnorm,
			F::Bc7RgbaUnorm => W::Bc7RgbaUnorm,
			F::Bc7RgbaUnormSrgb => W::Bc7RgbaUnormSrgb,
		}
	}
}

#[derive(Debug, Encode, Decode)]
pub struct Texture {
	pub extent: Extent3d,
	pub format: Format,
	pub data: Box<[u8]>,
}

impl Texture {
	pub const DEFAULT_EXTENT: Extent3d = Extent3d {
		width: 32,
		height: 32,
		depth: 1,
	};

	pub fn from_rgba_pixel(pixel: [u8; 4]) -> Self {
		let extent = Self::DEFAULT_EXTENT;
		let data = vec![pixel; extent.len()]
			.into_flattened()
			.into_boxed_slice();

		Self {
			extent,
			format: Format::Rgba8Unorm,
			data,
		}
	}

	pub fn from_rgba_pixel_f32(pixel: [f32; 4]) -> Self {
		let pixel = pixel.map(|p| (p * 255.0) as u8);

		Self::from_rgba_pixel(pixel)
	}

	pub fn from_rg_pixel(pixel: [u8; 2]) -> Self {
		let extent = Self::DEFAULT_EXTENT;
		let data = vec![pixel; extent.len()]
			.into_flattened()
			.into_boxed_slice();

		Self {
			extent,
			format: Format::Rg8Unorm,
			data,
		}
	}

	pub fn from_rg_pixel_f32(pixel: [f32; 2]) -> Self {
		let pixel = pixel.map(|p| (p * 255.0) as u8);

		Self::from_rg_pixel(pixel)
	}

	pub fn from_r_pixel(pixel: u8) -> Self {
		let extent = Self::DEFAULT_EXTENT;
		let data = vec![pixel; extent.len()].into_boxed_slice();

		Self {
			extent,
			format: Format::R8Unorm,
			data,
		}
	}

	pub fn from_r_pixel_f32(pixel: f32) -> Self {
		let pixel = (pixel * 255.0) as u8;

		Self::from_r_pixel(pixel)
	}
}

#[cfg(feature = "gltf")]
impl Texture {
	pub fn from_gltf_data(data: gltf::image::Data) -> Self {
		use image::buffer::ConvertBuffer;

		let extent = Extent3d::from_wh(data.width as u16, data.height as u16);

		let (data, format) = match data.format.try_into() {
			Ok(format) => (data.pixels, format),
			Err(..) if data.format == gltf::image::Format::R8G8B8 => {
				let image: image::RgbaImage =
					image::RgbImage::from_vec(data.width, data.height, data.pixels)
						.expect("invalid image data")
						.convert();

				(image.into_raw(), Format::Rgba8Unorm)
			}
			Err(..) => panic!("unsupported image format"),
		};

		Self {
			extent,
			format,
			data: data.into_boxed_slice(),
		}
	}
}

#[derive(Debug, Encode, Decode)]
pub struct Material {
	pub albedo: Handle<Texture>,
	pub normal: Option<Handle<Texture>>,
	pub metallic_roughness: Option<Handle<Texture>>,
	pub ao: Option<Handle<Texture>>,

	pub transparent: bool,
}

#[cfg(feature = "gltf")]
impl Material {
	pub fn from_gltf_material(material: &gltf::Material<'_>, root: &Path) -> Result<Self> {
		let diffuse_texture = material
			.pbr_metallic_roughness()
			.base_color_texture()
			.map(|t| t.texture().source())
			.map(|s| gltf::image::Data::from_source(s.source(), Some(root), &[]))
			.transpose()?;

		let diffuse_colour = material.pbr_metallic_roughness().base_color_factor();
		let diffuse_texture = diffuse_texture.map_or_else(
			|| Texture::from_rgba_pixel_f32(diffuse_colour),
			Texture::from_gltf_data,
		);

		let normal_texture = material
			.normal_texture()
			.map(|t| t.texture().source())
			.map(|s| gltf::image::Data::from_source(s.source(), Some(root), &[]))
			.transpose()?
			.map(Texture::from_gltf_data);

		let metallic_roughness = material.pbr_metallic_roughness();
		let metallic_factor = metallic_roughness.metallic_factor();
		let roughness_factor = metallic_roughness.roughness_factor();

		let metallic_roughness_texture = metallic_roughness
			.metallic_roughness_texture()
			.map(|t| t.texture().source())
			.map(|s| gltf::image::Data::from_source(s.source(), Some(root), &[]))
			.transpose()?;

		// roughness = G
		// metallic = B (will be written to R)
		let metallic_roughness_texture = metallic_roughness_texture.map_or_else(
			|| Texture::from_rg_pixel_f32([metallic_factor, roughness_factor]),
			Texture::from_gltf_data,
		);

		let ao_texture = material
			.occlusion_texture()
			.map(|t| t.texture().source())
			.map(|s| gltf::image::Data::from_source(s.source(), Some(root), &[]))
			.transpose()?;
		let ao_texture = ao_texture.map(Texture::from_gltf_data)?;

		Ok(Self {
			albedo: diffuse_texture,
			normal: normal_texture,
			metallic_roughness: metallic_roughness_texture,
			ao: ao_texture,
			transparent: material.alpha_mode() == gltf::material::AlphaMode::Blend,
		})
	}
}
