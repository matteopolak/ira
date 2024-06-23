use std::path::PathBuf;

use clap::{Parser, ValueHint};
use ira_drum::{Drum, DrumBuilder, Mipmaps};
use tracing::{info, warn};

#[derive(Parser)]
enum Args {
	/// Packs various game assets (glTF, etc.) into a single Drum.
	Pack {
		/// The entry files to pack.
		#[arg(value_hint = ValueHint::FilePath)]
		assets: Vec<PathBuf>,
		/// The output path for the Drum.
		#[arg(short, long, value_hint = ValueHint::FilePath)]
		output: PathBuf,
		/// A path to the irradiance map.
		#[arg(short, long, value_hint = ValueHint::FilePath)]
		irradiance: Option<PathBuf>,
		/// A path to the prefiltered map.
		#[arg(short, long, value_hint = ValueHint::FilePath)]
		prefiltered: Option<PathBuf>,
		/// A path to the BRDF LUT.
		#[arg(short, long, value_hint = ValueHint::FilePath)]
		brdf: Option<PathBuf>,
		/// Whether to use Block Compression for textures.
		#[arg(short, long)]
		compress: bool,
		/// Whether the assets are in sRGB color space.
		#[arg(short, long)]
		srgb: bool,
		/// The number of mipmaps to generate for textures. If not specified,
		/// mipmaps will be generated automatically.
		#[arg(short, long)]
		mipmaps: Option<Option<u32>>,
	},
	/// Displays information about a Drum.
	Info {
		/// The Drum to display information about.
		#[arg(value_hint = ValueHint::FilePath)]
		drum: PathBuf,
	},
}

fn main() -> anyhow::Result<()> {
	let args = Args::parse();

	tracing_subscriber::fmt::init();

	match args {
		Args::Pack {
			assets,
			compress,
			mipmaps,
			irradiance,
			prefiltered,
			brdf,
			output,
			srgb,
		} => {
			let drum = pack(
				assets,
				compress,
				srgb,
				mipmaps.flatten(),
				irradiance,
				prefiltered,
				brdf,
			)?;

			drum.write_to_path(output)?;
		}
		Args::Info { drum } => {
			let drum = Drum::from_path(drum)?;

			println!("{}", drum);
		}
	}

	Ok(())
}

fn pack(
	assets: Vec<PathBuf>,
	compress: bool,
	srgb: bool,
	mipmaps: Option<u32>,
	irradiance: Option<PathBuf>,
	prefiltered: Option<PathBuf>,
	brdf: Option<PathBuf>,
) -> anyhow::Result<Drum> {
	let mut drum = DrumBuilder::default();

	for asset in assets {
		let Some(ext) = asset.extension() else {
			warn!(path = %asset.display(), "skipping asset without extension");
			continue;
		};

		match ext.to_str() {
			Some("gltf") => {
				info!(path = %asset.display(), "adding glTF asset");

				let gltf = ira_drum::GltfSource::from_path(&asset)?;

				drum.add(gltf)?;
			}
			Some("obj") => {
				info!(path = %asset.display(), "adding OBJ asset");

				let obj = ira_drum::ObjSource::from_path(&asset)?;

				drum.add(obj)?;
			}
			_ => {
				warn!(path = %asset.display(), "skipping unsupported asset");
			}
		}
	}

	if let Some(irradiance) = irradiance {
		drum.set_irradiance_map(ira_drum::Texture::from_path(irradiance)?.into_cubemap()?);
	}

	if let Some(prefiltered) = prefiltered {
		drum.set_prefiltered_map(ira_drum::Texture::from_path(prefiltered)?.into_cubemap()?);
	}

	if let Some(brdf) = brdf {
		drum.set_brdf_lut(ira_drum::Texture::from_path(brdf)?);
	}

	let mut drum = drum.build();
	let mipmaps = mipmaps.map_or(Mipmaps::GeneratedAutomatic, Mipmaps::GeneratedExact);

	info!("processing textures");

	drum.prepare_textures(compress, srgb, |_| mipmaps)?;

	Ok(drum)
}
