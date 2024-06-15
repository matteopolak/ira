# Ira CLI

A command-line interface for the Ira game engine.

```bash
Usage: ira <COMMAND>

Commands:
  pack  Packs various game assets (glTF, etc.) into a single Drum
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

## Installation

```bash
cargo install ira_cli
```

## Packing assets

```bash
> ira pack --help
Packs various game assets (glTF, etc.) into a single Drum

Usage: ira pack [OPTIONS] --output <OUTPUT> [ASSETS]...

Arguments:
  [ASSETS]...  The entry files to pack

Options:
  -o, --output <OUTPUT>            The output path for the Drum
  -i, --irradiance <IRRADIANCE>    A path to the irradiance map
  -p, --prefiltered <PREFILTERED>  A path to the prefiltered map
  -b, --brdf <BRDF>                A path to the BRDF LUT
  -c, --compress                   Whether to use Block Compression for textures
  -s, --srgb                       Whether the assets are in sRGB color space
  -m, --mipmaps [<MIPMAPS>]        The number of mipmaps to generate for textures. If not specified, mipmaps will be generated automatically
  -h, --help                       Print help

> ira pack models/bottled_car/scene.gltf -i ibl_irradiance_map.png -p ibl_prefilter_map.png -b ibl_brdf_lut.png --compress --mipmaps -o car.drum --srgb
```

