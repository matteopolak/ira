use std::{fmt, fs::File, io, path::Path};

use bincode::{
	error::{DecodeError, EncodeError},
	Decode, Encode,
};
use flate2::{bufread::DeflateDecoder, write::DeflateEncoder, Compression};
use image_dds::Mipmaps;

use crate::{source::Source, Extent3d, Handle};

pub const CONFIG: bincode::config::Configuration = bincode::config::standard();

#[derive(Debug, Encode, Decode)]
pub struct Drum {
	pub textures: Box<[super::Texture]>,
	pub materials: Box<[super::Material]>,
	pub meshes: Box<[super::Mesh]>,
	pub models: Box<[super::Model]>,
	pub lights: Box<[super::Light]>,

	pub brdf_lut: Option<super::Texture>,
	pub irradiance_map: Option<super::Texture>,
	pub prefiltered_map: Option<super::Texture>,
}

impl fmt::Display for Drum {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		writeln!(f, "Texture count: {}", self.textures.len())?;
		writeln!(f, "Material count: {}", self.materials.len())?;
		writeln!(f, "Mesh count: {}", self.meshes.len())?;
		writeln!(f, "Model count: {}", self.models.len())?;
		writeln!(f, "Light count: {}", self.lights.len())?;
		writeln!(f, "BRDF lut: {}", self.brdf_lut.is_some())?;
		writeln!(f, "Irradiance map: {}", self.irradiance_map.is_some())?;
		writeln!(f, "Prefiltered map: {}", self.prefiltered_map.is_some())?;

		Ok(())
	}
}

impl Drum {
	/// When reading and writing with paths, the buffer size is 1MB.
	pub const BUF_SIZE: usize = 1_024 * 1_024;

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
		let file = io::BufReader::with_capacity(
			Self::BUF_SIZE,
			File::open(path).map_err(|e| DecodeError::Io {
				inner: e,
				additional: 0,
			})?,
		);

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
		let mut file = io::BufWriter::with_capacity(
			Self::BUF_SIZE,
			File::options()
				.create(true)
				.truncate(true)
				.write(true)
				.open(path)
				.map_err(|e| EncodeError::Io { inner: e, index: 0 })?,
		);

		self.write(&mut file)
	}

	/// Compresses and/or generates mipmaps for all textures in the drum.
	///
	/// # Errors
	///
	/// See [`super::material::Texture::compress`] for more information.
	#[tracing::instrument(skip(mipmaps))]
	pub fn prepare_textures<F>(
		&mut self,
		compress: bool,
		srgb: bool,
		mipmaps: F,
	) -> Result<(), image_dds::error::SurfaceError>
	where
		F: Fn(Extent3d) -> Mipmaps + Copy,
	{
		for texture in &mut self.textures {
			texture.process(compress, srgb, mipmaps)?;
		}

		if let Some(brdf_lut) = &mut self.brdf_lut {
			brdf_lut.process(compress, srgb, mipmaps)?;
		}

		if let Some(irradiance_map) = &mut self.irradiance_map {
			irradiance_map.process(compress, srgb, mipmaps)?;
		}

		if let Some(prefiltered_map) = &mut self.prefiltered_map {
			prefiltered_map.process(compress, srgb, mipmaps)?;
		}

		Ok(())
	}

	/// Turns the drum into a builder.
	#[must_use]
	pub fn into_builder(self) -> DrumBuilder {
		DrumBuilder {
			textures: self.textures.into_vec(),
			materials: self.materials.into_vec(),
			meshes: self.meshes.into_vec(),
			models: self.models.into_vec(),
			lights: self.lights.into_vec(),

			brdf_lut: self.brdf_lut,
			irradiance_map: self.irradiance_map,
			prefiltered_map: self.prefiltered_map,
		}
	}
}

#[derive(Debug, Default)]
pub struct DrumBuilder {
	pub textures: Vec<super::Texture>,
	pub materials: Vec<super::Material>,
	pub meshes: Vec<super::Mesh>,
	pub models: Vec<super::Model>,
	pub lights: Vec<super::Light>,

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
			meshes: self.meshes.into_boxed_slice(),
			models: self.models.into_boxed_slice(),
			lights: self.lights.into_boxed_slice(),

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
		Handle::from_vec(&mut self.meshes, mesh)
	}

	pub fn add_model(&mut self, model: super::Model) -> Handle<super::Model> {
		Handle::from_vec(&mut self.models, model)
	}
}

#[cfg(feature = "gltf")]
impl DrumBuilder {
	/// Creates a new drum from a supported source.
	///
	/// # Errors
	///
	/// See [`Source::add_to_drum`] for more information.
	pub fn from_source<S: Source>(source: S) -> Result<Self, S::Error> {
		let mut drum = Self::default();

		drum.add(source)?;

		Ok(drum)
	}

	/// Adds a supported source to the drum.
	///
	/// # Errors
	///
	/// See [`Source::add_to_drum`] for more information.
	pub fn add<S: Source>(&mut self, source: S) -> Result<(), S::Error> {
		source.add_to_drum(self)
	}
}
