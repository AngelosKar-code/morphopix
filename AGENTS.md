# Notes for working on MorphoPix

Context that is not obvious from reading the code, and the traps already paid for once.

## Releasing

**There is a GitHub Action: `.github/workflows/release.yml`.** Pushing a tag `v*` builds on
Windows, runs all three test suites, creates a **draft** release, and attaches the NSIS
installer, the portable binary and `SHA256SUMS.txt`. Review the draft, then publish it.

Do not build and upload releases by hand — that is what produced the first release before
the workflow existed.

```bash
# bump "version" in src-tauri/tauri.conf.json first, then:
git tag v0.2.0 && git push origin v0.2.0
```

The workflow **fails deliberately** if the tag does not match `version` in
`src-tauri/tauri.conf.json`. The version is read at runtime by `getVersion()` and shown next
to the logo, so a mismatch would make the app claim the wrong version.

Only NSIS is bundled (`bundle.targets`). MSI was dropped on purpose: it exists for
centralised corporate deployment, which is not this project's audience, and a third choice
on the download page is a decision the visitor should not have to make.

## The build profile is not a micro-optimisation

`src-tauri/Cargo.toml` raises `opt-level` for **this crate**, not just dependencies.
`fast_image_resize` is generic, so its SIMD resize loops are monomorphised into this crate
and compiled at *this* crate's opt-level. Left at the default, a 12MP Lanczos3 resize took
**2.6s instead of 30ms**. If resizing suddenly feels slow in dev, check this first.

## Deliberate decisions, not oversights

- **No ImageMagick.** It has no codecs of its own; it delegates to libwebp/libjpeg/libpng,
  which this app calls directly. PDF, PSD, layered TIFF and camera RAW are therefore out of
  scope. Revisit only if one of those is actually needed.
- **No thumbnails in the queue.** A thumbnail costs a full decode (~130ms for 12MP; the
  `image` crate has no scaled-decode path). For 2,000 files that is minutes of CPU competing
  with the batch itself. Visual checking lives in the Compare dialog, on one image, on
  demand.
- **`resize_to_fit` returns `Cow`.** With resizing off it borrows, so a batch never copies a
  ~34MB decoded frame just to hand it to the encoder. There is a test asserting the borrow.
- **EXIF orientation is reset when metadata is preserved.** The decoder already baked the
  rotation into the pixels, so copying the tag unchanged would make viewers rotate the image
  a second time. `metadata::neutralize_orientation` handles this; there is an end-to-end
  test.
- **Quality is hidden for PNG.** The PNG encoder ignores it entirely, so showing the slider
  would be a lie.

## Two implementations of the same rule

Output filenames are computed in **both** `src-tauri/src/pipeline.rs` (`clean_stem`) and
`src/lib/naming.ts` (`previewOutputName`), because the panel previews the name without a
round trip. The same cases are asserted in both test suites. **Change both, or the preview
starts lying.**

## Performance shape

Memory scales with worker count, not file count: each worker holds one decoded frame.
Measured on 300 mixed photos, 12 cores took 8.3s peaking at 466MB; 4 cores took 13.7s
peaking at 296MB. The Performance selector is therefore the memory dial as much as the speed
dial.

## Measurement tools

```bash
cd src-tauri
cargo test --lib -- --ignored --nocapture                      # prints timings, asserts nothing
cargo run --release --example gen_test_images -- ../test-images 300
cargo run --release --example bench_batch -- ../test-images ../out 12
```

`test-images/` is gitignored; regenerate it rather than committing photos.

## Layout traps hit before

- Cards in the steps column need `shrink-0`, or flexbox compresses them instead of scrolling
  the column, silently clipping whatever card is open.
- The header must **not** have `overflow-hidden`: it clips its own dropdown menus. Horizontal
  overflow is prevented by `min-w-0` on the shrinking path buttons plus `overflow: hidden` on
  `html, body`.
