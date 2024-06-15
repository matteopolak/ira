use gltf::{image::Data, Gltf};
use std::{
	fmt,
	path::{Path, PathBuf},
};

use crate::{
	DrumBuilder, Extent3d, Format, Material, MeshHandles, Model, Source, Texture, Vec2, Vec3,
	Vertex,
};

#[must_use]
#[derive(Debug)]
pub struct GltfSource {
	gltf: Gltf,
	root: PathBuf,
}

impl GltfSource {
	/// Creates a new glTF source.
	pub fn new(gltf: Gltf, root: PathBuf) -> Self {
		Self { gltf, root }
	}

	/// Opens a glTF file from the given path.
	///
	/// The root directory of the glTF file is used to resolve relative paths.
	///
	/// # Errors
	///
	/// See [`Gltf::open`].
	///
	/// # Panics
	///
	/// Panics if the given path does not have a parent directory.
	pub fn from_path<P: AsRef<Path>>(path: P) -> gltf::Result<Self> {
		let root = path.as_ref().parent().unwrap().to_path_buf();
		let gltf = Gltf::open(path)?;

		Ok(Self::new(gltf, root))
	}
}

impl Source for GltfSource {
	type Error = gltf::Error;

	/// Adds the glTF source to the drum.
	///
	/// # Errors
	///
	/// See [`gltf::Error`] for more information.
	fn add_to_drum(self, drum: &mut DrumBuilder) -> Result<(), Self::Error> {
		let mut opaque_meshes = Vec::new();
		let mut transparent_meshes = Vec::new();

		let mut material_handles = Vec::new();

		for material in self.gltf.materials() {
			let material = Material::from_gltf_material(drum, &material, &self.root)?;

			material_handles.push(drum.add_material(material));
		}

		let buffers = self
			.gltf
			.buffers()
			.map(|b| {
				let data = gltf::buffer::Data::from_source(b.source(), Some(&self.root))?;

				Ok(data.0)
			})
			.collect::<gltf::Result<Vec<_>>>()?;

		let mut centroid = Vec3::ZERO;
		let mut num_vertices = 0;

		for mesh in self.gltf.meshes() {
			for primitive in mesh.primitives() {
				let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

				let material_index = primitive.material().index();

				let positions = reader.read_positions().ok_or(gltf::Error::MissingBlob)?;
				let normals = reader.read_normals().ok_or(gltf::Error::MissingBlob)?;
				let tex_coords = reader.read_tex_coords(0);

				let vertices = positions.zip(normals);
				let vertices: Box<[Vertex]> = if let Some(tex_coords) = tex_coords {
					vertices
						.zip(tex_coords.into_f32())
						.map(|((position, normal), tex_coord)| {
							centroid += Vec3::from(position);

							Vertex::new(
								position.into(),
								normal.into(),
								tex_coord.map(f32::fract).into(),
							)
						})
						.collect()
				} else {
					vertices
						.map(|(position, normal)| {
							centroid += Vec3::from(position);

							Vertex::new(position.into(), normal.into(), Vec2::ZERO)
						})
						.collect()
				};

				num_vertices += vertices.len();

				let indices = reader
					.read_indices()
					.ok_or(gltf::Error::MissingBlob)?
					.into_u32()
					.collect();

				let mesh = crate::Mesh::new(
					vertices,
					indices,
					material_handles[material_index.unwrap_or_default()],
				);

				let material = mesh.material.resolve(&drum.materials);

				if material.transparent {
					transparent_meshes.push(drum.add_mesh(mesh));
				} else {
					opaque_meshes.push(drum.add_mesh(mesh));
				}
			}
		}

		centroid /= num_vertices as f32;

		let model = Model {
			meshes: MeshHandles {
				opaque: opaque_meshes.into_boxed_slice(),
				transparent: transparent_meshes.into_boxed_slice(),
			},
			center: centroid,
		};

		drum.add_model(model);

		Ok(())
	}
}

fn bytes_format_from_gltf(data: gltf::image::Data) -> (Vec<u8>, Format, Extent3d) {
	use image::buffer::ConvertBuffer;

	let (bytes, format) = match data.format.try_into() {
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

	let extent = Extent3d::from_wh(data.width, data.height);

	(bytes, format, extent)
}

fn image_from_bytes_format(
	bytes: Vec<u8>,
	format: Format,
	extent: Extent3d,
) -> image::DynamicImage {
	match format {
		Format::R8Unorm => image::GrayImage::from_raw(extent.width, extent.height, bytes)
			.expect("invalid image data")
			.into(),
		Format::Rg8Unorm => image::GrayAlphaImage::from_raw(extent.width, extent.height, bytes)
			.expect("invalid image data")
			.into(),
		Format::Rgb8A8Unorm | Format::Rgba8Unorm => {
			image::RgbaImage::from_raw(extent.width, extent.height, bytes)
				.expect("invalid image data")
				.into()
		}
		_ => panic!("unsupported image format"),
	}
}

impl Texture {
	#[must_use]
	#[tracing::instrument]
	pub fn from_gltf_data(data: gltf::image::Data) -> Self {
		let (data, format, extent) = bytes_format_from_gltf(data);

		Self {
			extent,
			format,
			mipmaps: 1,
			data: data.into_boxed_slice(),
		}
	}

	/// Constructs a new [`Texture`] with:
	///
	/// - R: ambient occlusion
	/// - G: roughness
	/// - B: metallic
	///
	/// If `ao` is not present, it is assumed to be 1.0
	/// If `mr` is not present, it is assumed to be `mf` and `rf`
	pub fn from_ao_mr(ao: Option<Data>, mr: Option<Data>, mf: f32, rf: f32) -> Self {
		let ao = ao
			.map(bytes_format_from_gltf)
			.map(|ao| image_from_bytes_format(ao.0, ao.1, ao.2));
		let mr = mr
			.map(bytes_format_from_gltf)
			.map(|mr| image_from_bytes_format(mr.0, mr.1, mr.2));

		let mf = (mf * 255.0) as u8;
		let rf = (rf * 255.0) as u8;

		match (ao, mr) {
			(Some(ao), Some(mr)) => {
				let ao = ao.into_rgba8();
				let mr = mr.into_rgba8();

				let mut data = Vec::with_capacity(ao.len());

				for (a, m) in ao.pixels().zip(mr.pixels()) {
					data.push(a[0]);
					data.push(m[2]);
					data.push(m[1]);
					data.push(255);
				}

				let extent = Extent3d::from_wh(ao.width(), ao.height());

				Self {
					extent,
					format: Format::Rgb8A8Unorm,
					mipmaps: 1,
					data: data.into_boxed_slice(),
				}
			}
			(Some(ao), None) => {
				let ao = ao.into_rgba8();

				let mut data = Vec::with_capacity(ao.len());

				for a in ao.pixels() {
					data.push(a[0]);
					data.push(rf);
					data.push(mf);
					data.push(255);
				}

				let extent = Extent3d::from_wh(ao.width(), ao.height());

				Self {
					extent,
					format: Format::Rgb8A8Unorm,
					mipmaps: 1,
					data: data.into_boxed_slice(),
				}
			}
			(None, Some(mr)) => {
				let mr = mr.into_rgba8();

				let mut data = Vec::with_capacity(mr.len());

				for m in mr.pixels() {
					data.push(255);
					data.push(m[2]);
					data.push(m[1]);
					data.push(255);
				}

				let extent = Extent3d::from_wh(mr.width(), mr.height());

				Self {
					extent,
					format: Format::Rgb8A8Unorm,
					mipmaps: 1,
					data: data.into_boxed_slice(),
				}
			}
			(None, None) => Texture::from_rgb_pixel([255, rf, mf]),
		}
	}
}

impl Material {
	/// Creates a new material from a glTF material.
	///
	/// # Errors
	///
	/// Returns an error if the material's textures cannot be loaded.
	/// See [`gltf::image::Data::from_source`] for more information.
	#[tracing::instrument]
	pub fn from_gltf_material<P: AsRef<Path> + fmt::Debug>(
		drum: &mut DrumBuilder,
		material: &gltf::Material<'_>,
		root: P,
	) -> Result<Self, gltf::Error> {
		let root = root.as_ref();

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
		let ao = material
			.occlusion_texture()
			.map(|t| t.texture().source())
			.map(|s| gltf::image::Data::from_source(s.source(), Some(root), &[]))
			.transpose()?;
		let orm = Texture::from_ao_mr(ao, metallic_roughness, metallic_factor, roughness_factor);

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
			orm: Some(orm.into_handle(drum)),
			emissive: Some(emissive.into_handle(drum)),
			transparent: material.alpha_mode() == gltf::material::AlphaMode::Blend,
		})
	}
}
