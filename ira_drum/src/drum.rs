use std::{fs::File, io, path::Path};

use bincode::{
	error::{DecodeError, EncodeError},
	Decode, Encode,
};
use flate2::{bufread::DeflateDecoder, write::DeflateEncoder, Compression};

use crate::{Handle, Material, Mesh, Vec2, Vec3, Vertex};

pub const CONFIG: bincode::config::Configuration = bincode::config::standard();

#[derive(Debug, Encode, Decode)]
pub struct Meshes {
	pub opaque: Box<[Mesh]>,
	pub transparent: Box<[Mesh]>,
}

#[derive(Debug, Encode, Decode)]
pub struct Drum {
	pub textures: Box<[super::Texture]>,
	pub materials: Box<[super::Material]>,
	pub meshes: Meshes,
	pub models: Box<[super::Model]>,

	pub brdf_lut: Option<super::Texture>,
	pub irradiance_map: Option<super::Texture>,
	pub prefiltered_map: Option<super::Texture>,
}

impl Drum {
	/// Reads a drum from a reader.
	///
	/// # Errors
	///
	/// See [`bincode::decode_from_std_read`] for more information.
	pub fn from_reader<R: io::BufRead>(reader: R) -> Result<Self, DecodeError> {
		let mut decoder = DeflateDecoder::new(reader);

		bincode::decode_from_std_read(&mut decoder, CONFIG)
	}

	/// Reads a drum from a file.
	///
	/// # Errors
	///
	/// See [`Self::from_reader`] for more information.
	pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, DecodeError> {
		let file = io::BufReader::new(File::open(path).map_err(|e| DecodeError::Io {
			inner: e,
			additional: 0,
		})?);

		Self::from_reader(file)
	}

	/// Writes the drum to a writer.
	///
	/// # Errors
	///
	/// See [`bincode::encode_into_std_write`] for more information.
	pub fn write<W: io::Write>(&self, writer: &mut W) -> Result<usize, EncodeError> {
		let mut encoder = DeflateEncoder::new(writer, Compression::default());

		bincode::encode_into_std_write(self, &mut encoder, CONFIG)
	}

	/// Encodes the drum into a [`Vec<u8>`].
	///
	/// # Errors
	///
	/// See [`Self::write`] for more information.
	pub fn to_vec(&self) -> Result<Vec<u8>, EncodeError> {
		let mut buffer = Vec::new();

		self.write(&mut buffer)?;

		Ok(buffer)
	}

	/// Writes the drum to a file.
	///
	/// # Errors
	///
	/// See [`bincode::encode_into_writer`] for more information.
	/// See [`std::fs::File::open`] for more information.
	pub fn write_to_path<P: AsRef<Path>>(&self, path: P) -> Result<usize, EncodeError> {
		let mut file = io::BufWriter::new(
			File::options()
				.create(true)
				.truncate(true)
				.write(true)
				.open(path)
				.map_err(|e| EncodeError::Io { inner: e, index: 0 })?,
		);

		self.write(&mut file)
	}

	/// Compressed all textures in the drum (where applicable).
	///
	/// # Errors
	///
	/// See [`super::material::Texture::compress`] for more information.
	#[tracing::instrument]
	pub fn compress_textures(&mut self) -> Result<(), image_dds::error::SurfaceError> {
		for texture in &mut self.textures {
			texture.compress()?;
		}

		if let Some(brdf_lut) = &mut self.brdf_lut {
			brdf_lut.compress()?;
		}

		if let Some(irradiance_map) = &mut self.irradiance_map {
			irradiance_map.compress()?;
		}

		if let Some(prefiltered_map) = &mut self.prefiltered_map {
			prefiltered_map.compress()?;
		}

		Ok(())
	}
}

#[derive(Debug, Default)]
pub struct MeshesBuilder {
	pub opaque: Vec<super::Mesh>,
	pub transparent: Vec<super::Mesh>,
}

impl MeshesBuilder {
	#[must_use]
	pub fn build(self) -> Meshes {
		Meshes {
			opaque: self.opaque.into_boxed_slice(),
			transparent: self.transparent.into_boxed_slice(),
		}
	}
}

#[derive(Debug, Default)]
pub struct DrumBuilder {
	pub textures: Vec<super::Texture>,
	pub materials: Vec<super::Material>,
	pub meshes: MeshesBuilder,
	pub models: Vec<super::Model>,

	pub brdf_lut: Option<super::Texture>,
	pub irradiance_map: Option<super::Texture>,
	pub prefiltered_map: Option<super::Texture>,
}

impl DrumBuilder {
	#[must_use]
	pub fn build(self) -> Drum {
		Drum {
			textures: self.textures.into_boxed_slice(),
			materials: self.materials.into_boxed_slice(),
			meshes: self.meshes.build(),
			models: self.models.into_boxed_slice(),

			brdf_lut: self.brdf_lut,
			irradiance_map: self.irradiance_map,
			prefiltered_map: self.prefiltered_map,
		}
	}

	pub fn set_brdf_lut(&mut self, texture: super::Texture) {
		self.brdf_lut = Some(texture);
	}

	pub fn set_irradiance_map(&mut self, texture: super::Texture) {
		self.irradiance_map = Some(texture);
	}

	pub fn set_prefiltered_map(&mut self, texture: super::Texture) {
		self.prefiltered_map = Some(texture);
	}

	pub fn add_material(&mut self, material: super::Material) -> Handle<super::Material> {
		Handle::from_vec(&mut self.materials, material)
	}

	pub fn add_texture(&mut self, texture: super::Texture) -> Handle<super::Texture> {
		Handle::from_vec(&mut self.textures, texture)
	}

	pub fn add_mesh(&mut self, mesh: super::Mesh) -> Handle<super::Mesh> {
		let material = mesh.material.resolve(&self.materials);

		Handle::from_vec(
			if material.transparent {
				&mut self.meshes.transparent
			} else {
				&mut self.meshes.opaque
			},
			mesh,
		)
	}

	pub fn add_model(&mut self, model: super::Model) -> Handle<super::Model> {
		Handle::from_vec(&mut self.models, model)
	}
}

#[cfg(feature = "gltf")]
impl DrumBuilder {
	/// Creates a new [`DrumBuilder`] from a glTF file.
	///
	/// # Errors
	///
	/// See [`Self::extend_from_gltf`].
	pub fn from_gltf(gltf: &gltf::Gltf, root: &Path) -> gltf::Result<Self> {
		let mut drum = DrumBuilder::default();

		drum.add_gltf(gltf, root)?;
		Ok(drum)
	}

	/// Extends the [`DrumBuilder`] with the contents of a glTF file.
	///
	/// # Errors
	///
	/// See [`gltf::Error`] for more information.
	///
	/// If a mesh does not contain normals, tex coords, or indices, a [`gltf::Error::MissingBlob`] will be returned.
	pub fn add_gltf(&mut self, gltf: &gltf::Gltf, root: &Path) -> gltf::Result<()> {
		let mut meshes = Vec::new();

		for material in gltf.materials() {
			let material = Material::from_gltf_material(self, &material, root)?;

			self.add_material(material);
		}

		let buffers = gltf
			.buffers()
			.map(|b| {
				let data = gltf::buffer::Data::from_source(b.source(), Some(root))?;

				Ok(data.0)
			})
			.collect::<gltf::Result<Vec<_>>>()?;

		let mut centroid = Vec3::ZERO;
		let mut num_vertices = 0;

		for mesh in gltf.meshes() {
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

				let mesh = Mesh {
					vertices,
					indices,
					material: Handle::new(material_index.unwrap_or_default() as u32),
				};

				meshes.push(self.add_mesh(mesh));
			}
		}

		centroid /= num_vertices as f32;

		let model = super::Model {
			meshes: meshes.into_boxed_slice(),
			center: centroid,
		};

		self.add_model(model);

		Ok(())
	}
}
