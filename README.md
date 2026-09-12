# MorphoPix

A desktop batch image optimiser for product photography. Point it at a folder, choose what
should happen to every image, and it writes optimised copies without touching the originals.

Built with [Tauri](https://tauri.app) (Rust) and [Svelte](https://svelte.dev).

## Install

Download from the [latest release](https://github.com/AngelosKar-code/morphopix/releases/latest):

| Download | Use it if |
| --- | --- |
| `MorphoPix_..._x64-setup.exe` | You want a normal install with a Start menu entry |
| `MorphoPix_..._x64_portable.exe` | You just want to run it, no install |

Windows 10 or 11, 64-bit. The installers are small because WebView2 ships with Windows and
is not bundled.

### About the Windows warning

The downloads are not code-signed, so Windows SmartScreen shows **"Windows protected your
PC"** with an unknown-publisher notice. That message is about publisher reputation, not about
anything detected in the file: signing certificates cost a few hundred euros a year, which is
hard to justify for a project like this.

To run it anyway: **More info → Run anyway**.

If you would rather verify the download first, compare its hash against
`SHA256SUMS.txt` from the same release:

```powershell
Get-FileHash .\MorphoPix_0.1.0_x64-setup.exe -Algorithm SHA256
```

That confirms the file arrived exactly as it was published. It is a reasonable habit for any
unsigned download, this one included.

## What it does

- **Resize** to a bounding box with Lanczos3, Catmull-Rom, Triangle or Nearest resampling
- **Convert** to WebP, JPEG or PNG, or keep each file's own format
- **Watermark** with a logo, sized and positioned as a share of each image
- **Rename** with a suffix, URL-safe hyphenation and lowercasing
- **Keep or drop metadata** — EXIF, GPS and ICC colour profiles
- **Compare** any queued image before and after, with zoom, so settings can be judged
  against the real encoder output rather than guessed

Every stage is independently switchable, and combinations can be saved as templates.

## Why there is no ImageMagick

ImageMagick has no codecs of its own — it delegates to libwebp, libjpeg and libpng. MorphoPix
calls those codecs directly from Rust instead, which keeps the same quality controls (WebP
quality and lossless mode, JPEG quality, PNG compression) without bundling a ~100MB binary
or paying subprocess overhead per file.

The trade-off is format coverage: PDF rasterisation, PSD, layered TIFF and camera RAW are
out of scope. JPEG, PNG and WebP are in.

## Performance

Measured on 300 mixed product photos (81MB total) on a 12-core machine:

| Workers | Time | Peak memory |
| --- | --- | --- |
| 12 | 8.3s | 466 MB |
| 4 | 13.7s | 296 MB |

Memory scales with worker count, not file count — each worker holds one decoded frame, so a
2,000 file run peaks no higher than a 300 file one. The Performance setting is therefore both
the speed dial and the memory dial.

A 12MP photo resized to 1920px and encoded to WebP q82 takes about 276ms of CPU and comes out
around 97% smaller than the camera original.

## Development

```bash
npm install
npm run tauri dev
```

Tests:

```bash
npm test                      # frontend: naming, destination paths, built-in templates
cd src-tauri && cargo test    # pipeline, watermark, metadata
```

On-demand measurements (they print numbers rather than asserting):

```bash
cd src-tauri
cargo test --lib -- --ignored --nocapture
cargo run --release --example gen_test_images -- ../test-images 300
cargo run --release --example bench_batch -- ../test-images ../out 12
```

### A note on build profiles

`Cargo.toml` raises `opt-level` for this crate as well as for dependencies in dev builds.
This is not a micro-optimisation: `fast_image_resize` is generic, so its SIMD resize loops
are monomorphised into this crate and compiled at *this* crate's opt-level. Left at the
default, a 12MP Lanczos3 resize took 2.6s instead of 30ms.

## Build

```bash
npm run tauri build
```

Produces `MorphoPix.exe` and an NSIS installer under `src-tauri/target/release/`.

Releases are built by CI rather than by hand: pushing a tag `v*` runs
[the release workflow](.github/workflows/release.yml), which tests, builds, and opens a draft
release with the installer, the portable binary and checksums. The workflow fails if the tag
disagrees with the version in `src-tauri/tauri.conf.json`.

## Licence

MIT.
