# nu_plugin_image

A [Nushell](https://www.nushell.sh/) plugin for transforming images with chainable pipeline commands. Load an image as binary data, apply transformations, and save the result — all within a single Nushell pipeline.

## Features

- **Resize** images to exact or aspect-ratio-preserving dimensions
- **Convert** images between formats (JPEG, PNG, WebP, BMP)
- **Chainable** — commands pass binary image data through the pipeline, so multiple transformations can be composed in one expression
- Pure Rust implementation via the [`image`](https://crates.io/crates/image) crate — no external native dependencies required

## Requirements

- [Nushell](https://www.nushell.sh/) 0.115.0 or later
- Rust toolchain (to build from source)

## Installation

### Build from source

```sh
cargo build --release
```

Then register the plugin with Nushell:

```nushell
plugin add ./target/release/nu_plugin_image
plugin use nu_plugin_image
```

## Commands

### `image resize`

Resizes piped image binary data.

```
Usage:
  <binary> | image resize [--width <int>] [--height <int>] [--maintain-aspect]

Flags:
  -w, --width <int>       Target width in pixels
  -h, --height <int>      Target height in pixels
  -m, --maintain-aspect   Preserve the original aspect ratio

At least one of --width or --height must be provided.
```

### `image convert`

Converts piped image binary data to another format.

```
Usage:
  <binary> | image convert <target_format>

Parameters:
  target_format   Output format: jpeg, png, webp, or bmp
```

## Examples

Resize an image to exactly 800×600:

```nushell
open photo.png | image resize --width 800 --height 600 | save resized.png
```

Resize while preserving the aspect ratio (fit within 1280×720):

```nushell
open photo.jpg | image resize --width 1280 --height 720 --maintain-aspect | save resized.jpg
```

Convert a PNG to WebP:

```nushell
open image.png | image convert webp | save image.webp
```

Chain resize and format conversion in one pipeline:

```nushell
open photo.png | image resize --width 400 --maintain-aspect | image convert jpeg | save thumbnail.jpg
```

## Supported Formats

| Format | Read | Write |
|--------|------|-------|
| JPEG   | ✅   | ✅    |
| PNG    | ✅   | ✅    |
| WebP   | ✅   | ✅    |
| BMP    | ✅   | ✅    |

## License

This project is available under the MIT License.
