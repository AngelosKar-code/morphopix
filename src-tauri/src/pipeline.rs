//! Core image processing. Deliberately free of Tauri types so it can be unit tested.

use crate::metadata;
use crate::watermark::{Watermark, WatermarkOptions};
use fast_image_resize as fr;
use fast_image_resize::IntoImageView;
use image::{DynamicImage, ImageDecoder, ImageReader};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::path::{Path, PathBuf};

pub const SUPPORTED_INPUT_EXTENSIONS: [&str; 8] = [
    "jpg", "jpeg", "png", "webp", "bmp", "tiff", "tif", "gif",
];

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Jpeg,
    Png,
    Webp,
    /// Keep each file in its original format.
    Keep,
}

/// What to do with whitespace in output filenames. Web servers escape spaces as `%20`, so
/// hyphenating is the usual choice for anything that ends up in a URL.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SpaceHandling {
    Keep,
    Remove,
    Hyphen,
    Underscore,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ResizeFilter {
    Nearest,
    Triangle,
    CatmullRom,
    Lanczos3,
}

impl From<ResizeFilter> for fr::ResizeAlg {
    fn from(value: ResizeFilter) -> Self {
        match value {
            ResizeFilter::Nearest => fr::ResizeAlg::Nearest,
            ResizeFilter::Triangle => fr::ResizeAlg::Convolution(fr::FilterType::Bilinear),
            ResizeFilter::CatmullRom => fr::ResizeAlg::Convolution(fr::FilterType::CatmullRom),
            ResizeFilter::Lanczos3 => fr::ResizeAlg::Convolution(fr::FilterType::Lanczos3),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessOptions {
    /// Bounding box. The aspect ratio is always preserved.
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    /// Never make an image bigger than it already is (what you want for e-shops).
    pub allow_upscale: bool,
    pub filter: ResizeFilter,
    pub format: OutputFormat,
    /// JPEG / lossy WebP: 1-100.
    pub quality: u8,
    /// WebP only: ignore `quality` and encode losslessly.
    pub webp_lossless: bool,
    /// PNG only: trade encoding time for file size.
    pub png_max_compression: bool,
    /// Appended before the extension, e.g. "_web".
    pub suffix: String,
    /// When a file is already in the target format and nothing would alter its pixels,
    /// copy it instead of decoding and re-encoding it.
    #[serde(default)]
    pub passthrough_same_format: bool,
    /// Whitespace treatment for the output filename.
    #[serde(default = "keep_spaces")]
    pub space_handling: SpaceHandling,
    /// Lowercase the output filename, the other half of the URL-safe convention.
    #[serde(default)]
    pub lowercase_names: bool,
    /// Carry the source EXIF and ICC profile onto the output instead of dropping them.
    #[serde(default)]
    pub preserve_metadata: bool,
    /// Logo to composite onto every image.
    #[serde(default)]
    pub watermark: Option<WatermarkOptions>,
}

fn keep_spaces() -> SpaceHandling {
    SpaceHandling::Keep
}

impl Default for ProcessOptions {
    fn default() -> Self {
        Self {
            max_width: Some(1920),
            max_height: Some(1920),
            allow_upscale: false,
            filter: ResizeFilter::Lanczos3,
            format: OutputFormat::Webp,
            quality: 82,
            webp_lossless: false,
            png_max_compression: false,
            suffix: String::new(),
            passthrough_same_format: false,
            space_handling: SpaceHandling::Keep,
            lowercase_names: false,
            preserve_metadata: false,
            watermark: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessOutcome {
    pub output_path: String,
    pub original_bytes: u64,
    pub output_bytes: u64,
    pub original_width: u32,
    pub original_height: u32,
    pub output_width: u32,
    pub output_height: u32,
}

/// Load an image and apply its EXIF orientation, so portrait photos don't come out sideways
/// once the metadata is dropped during re-encoding.
pub fn load_oriented(path: &Path) -> Result<DynamicImage, String> {
    let reader = ImageReader::open(path)
        .map_err(|e| format!("cannot open: {e}"))?
        .with_guessed_format()
        .map_err(|e| format!("cannot detect format: {e}"))?;

    let mut decoder = reader
        .into_decoder()
        .map_err(|e| format!("cannot decode: {e}"))?;

    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);

    let mut image =
        DynamicImage::from_decoder(decoder).map_err(|e| format!("cannot decode: {e}"))?;
    image.apply_orientation(orientation);

    Ok(image)
}

/// Largest size that fits inside the bounding box while preserving the aspect ratio.
fn fit_dimensions(width: u32, height: u32, bound_w: u32, bound_h: u32) -> (u32, u32) {
    let ratio = (bound_w as f64 / width as f64).min(bound_h as f64 / height as f64);
    (
        ((width as f64 * ratio).round() as u32).max(1),
        ((height as f64 * ratio).round() as u32).max(1),
    )
}

/// Fit inside the bounding box, preserving aspect ratio. Borrows the source untouched when no
/// resizing is needed — with resizing switched off, a batch never copies a decoded frame
/// (~34MB for a 12MP photo) just to hand it to the encoder.
///
/// Uses `fast_image_resize` rather than `image::DynamicImage::resize`: the same convolution
/// filters, but SIMD accelerated. Measured on a 12MP photo down to 1920px with Lanczos3, this
/// is the difference between ~800ms and ~45ms per image — and every batched image pays it.
/// The size an image would be resized to, or `None` when it would be left alone. Shared by
/// the resize itself and by the passthrough check, so the two can never disagree about
/// whether an image is about to change.
pub fn resize_target(
    width: u32,
    height: u32,
    options: &ProcessOptions,
) -> Option<(u32, u32)> {
    let bound_w = options.max_width.unwrap_or(u32::MAX).max(1);
    let bound_h = options.max_height.unwrap_or(u32::MAX).max(1);

    // Resizing switched off entirely.
    if bound_w == u32::MAX && bound_h == u32::MAX {
        return None;
    }

    if !options.allow_upscale && width <= bound_w && height <= bound_h {
        return None;
    }

    let target = fit_dimensions(width, height, bound_w, bound_h);
    if target == (width, height) {
        None
    } else {
        Some(target)
    }
}

pub fn resize_to_fit<'a>(
    image: &'a DynamicImage,
    options: &ProcessOptions,
) -> Cow<'a, DynamicImage> {
    let Some((target_w, target_h)) = resize_target(image.width(), image.height(), options)
    else {
        return Cow::Borrowed(image);
    };

    let resized = resize_simd(image, target_w, target_h, options.filter).unwrap_or_else(|_| {
        // Exotic pixel formats (16-bit, HDR) are not worth a special path; the image
        // crate handles them correctly, just slower.
        image.resize_exact(
            target_w,
            target_h,
            match options.filter {
                ResizeFilter::Nearest => image::imageops::FilterType::Nearest,
                ResizeFilter::Triangle => image::imageops::FilterType::Triangle,
                ResizeFilter::CatmullRom => image::imageops::FilterType::CatmullRom,
                ResizeFilter::Lanczos3 => image::imageops::FilterType::Lanczos3,
            },
        )
    });

    Cow::Owned(resized)
}

fn resize_simd(
    image: &DynamicImage,
    target_w: u32,
    target_h: u32,
    filter: ResizeFilter,
) -> Result<DynamicImage, String> {
    // Normalize to the two layouts we encode from anyway, so the fast path covers
    // everything that reaches an encoder.
    let has_alpha = image.color().has_alpha();
    let source = if has_alpha {
        DynamicImage::ImageRgba8(image.to_rgba8())
    } else {
        DynamicImage::ImageRgb8(image.to_rgb8())
    };

    let pixel_type = source.pixel_type().ok_or("unsupported pixel type")?;
    let mut destination = fr::images::Image::new(target_w, target_h, pixel_type);

    fr::Resizer::new()
        .resize(
            &source,
            &mut destination,
            &fr::ResizeOptions::new().resize_alg(filter.into()),
        )
        .map_err(|e| e.to_string())?;

    let buffer = destination.into_vec();

    if has_alpha {
        image::RgbaImage::from_raw(target_w, target_h, buffer)
            .map(DynamicImage::ImageRgba8)
            .ok_or_else(|| "resized buffer has unexpected size".to_string())
    } else {
        image::RgbImage::from_raw(target_w, target_h, buffer)
            .map(DynamicImage::ImageRgb8)
            .ok_or_else(|| "resized buffer has unexpected size".to_string())
    }
}

fn extension_for(format: OutputFormat, source: &Path) -> String {
    match format {
        OutputFormat::Jpeg => "jpg".to_string(),
        OutputFormat::Png => "png".to_string(),
        OutputFormat::Webp => "webp".to_string(),
        OutputFormat::Keep => source
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_lowercase(),
    }
}

/// Resolve which encoder to use when the format is `Keep`.
fn effective_format(format: OutputFormat, source: &Path) -> OutputFormat {
    match format {
        OutputFormat::Keep => match extension_for(OutputFormat::Keep, source).as_str() {
            "jpg" | "jpeg" => OutputFormat::Jpeg,
            "webp" => OutputFormat::Webp,
            _ => OutputFormat::Png,
        },
        other => other,
    }
}

/// Applies the naming rules to a filename stem. Runs after the suffix is appended, so a
/// suffix containing a space is cleaned up too.
pub fn clean_stem(stem: &str, options: &ProcessOptions) -> String {
    let spaced = match options.space_handling {
        SpaceHandling::Keep => stem.to_string(),
        SpaceHandling::Remove => stem.split_whitespace().collect::<Vec<_>>().join(""),
        SpaceHandling::Hyphen => stem.split_whitespace().collect::<Vec<_>>().join("-"),
        SpaceHandling::Underscore => stem.split_whitespace().collect::<Vec<_>>().join("_"),
    };

    let cleaned = if options.lowercase_names {
        spaced.to_lowercase()
    } else {
        spaced
    };

    // Every rule could in principle empty the name; never hand the encoder a bare extension.
    if cleaned.is_empty() {
        "image".to_string()
    } else {
        cleaned
    }
}

pub fn output_path_for(source: &Path, output_dir: &Path, options: &ProcessOptions) -> PathBuf {
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let ext = extension_for(options.format, source);
    let name = clean_stem(&format!("{stem}{}", options.suffix), options);
    output_dir.join(format!("{name}.{ext}"))
}

/// Encode with the real output settings into memory. The preview uses this to show what the
/// exported file will actually look like, without touching the disk.
pub fn encode_to_bytes(
    image: &DynamicImage,
    source: &Path,
    options: &ProcessOptions,
) -> Result<Vec<u8>, String> {
    let mut buffer = Vec::new();

    match effective_format(options.format, source) {
        OutputFormat::Jpeg => {
            // JPEG has no alpha channel; flatten to RGB8 to avoid a decode error later.
            let rgb = DynamicImage::ImageRgb8(image.to_rgb8());
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                &mut buffer,
                options.quality.clamp(1, 100),
            );
            rgb.write_with_encoder(encoder).map_err(|e| e.to_string())?;
        }
        OutputFormat::Png => {
            let compression = if options.png_max_compression {
                image::codecs::png::CompressionType::Best
            } else {
                image::codecs::png::CompressionType::Default
            };
            let encoder = image::codecs::png::PngEncoder::new_with_quality(
                &mut buffer,
                compression,
                image::codecs::png::FilterType::Adaptive,
            );
            image.write_with_encoder(encoder).map_err(|e| e.to_string())?;
        }
        OutputFormat::Webp => {
            // libwebp only accepts RGB8/RGBA8 buffers.
            let normalized = if image.color().has_alpha() {
                DynamicImage::ImageRgba8(image.to_rgba8())
            } else {
                DynamicImage::ImageRgb8(image.to_rgb8())
            };

            let encoder =
                webp::Encoder::from_image(&normalized).map_err(|e| format!("webp encoder: {e}"))?;

            let encoded = if options.webp_lossless {
                encoder.encode_lossless()
            } else {
                encoder.encode(options.quality.clamp(1, 100) as f32)
            };

            buffer.extend_from_slice(&encoded);
        }
        OutputFormat::Keep => unreachable!("resolved by effective_format"),
    }

    Ok(buffer)
}

/// Everything after resizing: watermark, encode, then metadata. Metadata goes last because
/// it edits the encoded container rather than the pixels.
fn render_output(
    resized: &DynamicImage,
    source: &Path,
    options: &ProcessOptions,
    watermark: Option<&Watermark>,
) -> Result<Vec<u8>, String> {
    let marked = match watermark {
        Some(mark) => Cow::Owned(mark.apply(resized)),
        None => Cow::Borrowed(resized),
    };

    let encoded = encode_to_bytes(&marked, source, options)?;

    if !options.preserve_metadata {
        return Ok(encoded);
    }

    // Needs the source container, not the decoded pixels, so the file is read again here.
    match std::fs::read(source) {
        Ok(source_bytes) => Ok(metadata::apply(encoded, &metadata::read(&source_bytes))),
        Err(_) => Ok(encoded),
    }
}

pub fn encode_to(image: &DynamicImage, path: &Path, options: &ProcessOptions) -> Result<(), String> {
    let bytes = encode_to_bytes(image, path, options)?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

/// Copies a file that is already in the target format and needs no pixel changes, instead
/// of decoding and re-encoding it. Re-encoding a JPEG as a JPEG costs quality for nothing,
/// so this is how a mixed folder can be normalised to one format losslessly.
///
/// Returns `None` when the fast path does not apply and the full pipeline must run.
fn try_passthrough(
    source: &Path,
    output_dir: &Path,
    options: &ProcessOptions,
    watermark: Option<&Watermark>,
) -> Result<Option<ProcessOutcome>, String> {
    if !options.passthrough_same_format || watermark.is_some() {
        return Ok(None);
    }

    // Reads the header only, not the pixels.
    let reader = ImageReader::open(source)
        .map_err(|e| format!("cannot open: {e}"))?
        .with_guessed_format()
        .map_err(|e| format!("cannot detect format: {e}"))?;

    let actual_format = match reader.format() {
        Some(image::ImageFormat::Jpeg) => OutputFormat::Jpeg,
        Some(image::ImageFormat::Png) => OutputFormat::Png,
        Some(image::ImageFormat::WebP) => OutputFormat::Webp,
        _ => return Ok(None),
    };

    if effective_format(options.format, source) != actual_format {
        return Ok(None);
    }

    let (width, height) = reader
        .into_dimensions()
        .map_err(|e| format!("cannot read dimensions: {e}"))?;

    if resize_target(width, height, options).is_some() {
        return Ok(None);
    }

    // An EXIF rotation only survives untouched if the metadata travels with the file.
    let source_bytes = std::fs::read(source).map_err(|e| e.to_string())?;
    let payload = if options.preserve_metadata {
        source_bytes
    } else {
        metadata::strip(source_bytes)
    };

    let output_path = output_path_for(source, output_dir, options);
    std::fs::write(&output_path, &payload).map_err(|e| e.to_string())?;

    Ok(Some(ProcessOutcome {
        output_path: output_path.to_string_lossy().to_string(),
        original_bytes: std::fs::metadata(source).map(|m| m.len()).unwrap_or(0),
        output_bytes: payload.len() as u64,
        original_width: width,
        original_height: height,
        output_width: width,
        output_height: height,
    }))
}

/// Full single-file pipeline: load → orient → resize → watermark → encode → metadata.
pub fn process_file(
    source: &Path,
    output_dir: &Path,
    options: &ProcessOptions,
    watermark: Option<&Watermark>,
) -> Result<ProcessOutcome, String> {
    if let Some(copied) = try_passthrough(source, output_dir, options, watermark)? {
        return Ok(copied);
    }

    let original_bytes = std::fs::metadata(source).map(|m| m.len()).unwrap_or(0);

    let image = load_oriented(source)?;
    let (original_width, original_height) = (image.width(), image.height());

    let resized = resize_to_fit(&image, options);
    let (output_width, output_height) = (resized.width(), resized.height());

    let rendered = render_output(&resized, source, options, watermark)?;
    let output_path = output_path_for(source, output_dir, options);
    std::fs::write(&output_path, &rendered).map_err(|e| e.to_string())?;

    let output_bytes = rendered.len() as u64;

    Ok(ProcessOutcome {
        output_path: output_path.to_string_lossy().to_string(),
        original_bytes,
        output_bytes,
        original_width,
        original_height,
        output_width,
        output_height,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
    pub before_uri: String,
    pub after_uri: String,
    pub original_bytes: u64,
    pub output_bytes: u64,
    pub original_width: u32,
    pub original_height: u32,
    pub output_width: u32,
    pub output_height: u32,
}

/// PNG, so the preview itself adds no compression artifacts on top of the ones we are
/// asking the user to judge. Default compression on purpose: `Fast` tripled the payload
/// crossing the IPC boundary without measurably shortening the round trip.
fn to_png_data_uri(image: &DynamicImage) -> Result<String, String> {
    use base64::Engine;

    let mut buffer = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buffer);
    image
        .write_with_encoder(encoder)
        .map_err(|e| e.to_string())?;

    let encoded = base64::engine::general_purpose::STANDARD.encode(buffer);
    Ok(format!("data:image/png;base64,{encoded}"))
}

/// Runs the real pipeline for one file in memory, returning both sides of the comparison.
/// `output_bytes` is the true size of what would be written, measured at full output
/// resolution — the preview images themselves are only scaled down for display.
pub fn preview_file(
    source: &Path,
    options: &ProcessOptions,
    display_size: u32,
) -> Result<PreviewResult, String> {
    let image = load_oriented(source)?;
    let watermark = load_watermark(options)?;
    preview_from_image(&image, source, options, display_size, None, watermark.as_ref())
}

/// Decodes the configured logo, if any. Callers batching many files should do this once.
pub fn load_watermark(options: &ProcessOptions) -> Result<Option<Watermark>, String> {
    match &options.watermark {
        Some(watermark) => Watermark::load(watermark).map(Some),
        None => Ok(None),
    }
}

/// The untouched side of the comparison. Depends only on the file, so the caller can
/// compute it once per selected image instead of once per settings change.
pub fn source_preview_uri(image: &DynamicImage, display_size: u32) -> Result<String, String> {
    to_png_data_uri(&image.thumbnail(display_size, display_size))
}

/// Same as [`preview_file`], but reuses an already decoded source and, optionally, an
/// already rendered "before" image. Dragging the quality slider only changes the "after".
pub fn preview_from_image(
    image: &DynamicImage,
    source: &Path,
    options: &ProcessOptions,
    display_size: u32,
    before_uri: Option<String>,
    watermark: Option<&Watermark>,
) -> Result<PreviewResult, String> {
    let original_bytes = std::fs::metadata(source).map(|m| m.len()).unwrap_or(0);
    let (original_width, original_height) = (image.width(), image.height());

    let processed = resize_to_fit(image, options);
    let (output_width, output_height) = (processed.width(), processed.height());

    let encoded = render_output(&processed, source, options, watermark)?;
    let output_bytes = encoded.len() as u64;

    // Decode what we just encoded, so the "after" side shows real codec output.
    let decoded = image::load_from_memory(&encoded)
        .map_err(|e| format!("cannot decode preview output: {e}"))?;

    let before_uri = match before_uri {
        Some(uri) => uri,
        None => source_preview_uri(image, display_size)?,
    };

    Ok(PreviewResult {
        before_uri,
        after_uri: to_png_data_uri(&decoded.thumbnail(display_size, display_size))?,
        original_bytes,
        output_bytes,
        original_width,
        original_height,
        output_width,
        output_height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn sample_image(width: u32, height: u32) -> DynamicImage {
        let mut buffer = RgbImage::new(width, height);
        for (x, y, pixel) in buffer.enumerate_pixels_mut() {
            *pixel = Rgb([(x % 256) as u8, (y % 256) as u8, 128]);
        }
        DynamicImage::ImageRgb8(buffer)
    }

    #[test]
    fn resize_fits_inside_bounds_and_keeps_aspect_ratio() {
        let image = sample_image(4000, 2000);
        let options = ProcessOptions {
            max_width: Some(1000),
            max_height: Some(1000),
            ..Default::default()
        };

        let resized = resize_to_fit(&image, &options);

        assert_eq!(resized.width(), 1000);
        assert_eq!(resized.height(), 500);
    }

    #[test]
    fn disabled_resize_borrows_instead_of_copying_the_frame() {
        let image = sample_image(4000, 3000);
        let options = ProcessOptions {
            max_width: None,
            max_height: None,
            ..Default::default()
        };

        let result = resize_to_fit(&image, &options);

        assert!(
            matches!(result, Cow::Borrowed(_)),
            "turning resize off must not copy a decoded frame"
        );
    }

    #[test]
    fn small_images_are_not_upscaled_by_default() {
        let image = sample_image(300, 200);
        let options = ProcessOptions {
            max_width: Some(1920),
            max_height: Some(1920),
            ..Default::default()
        };

        let resized = resize_to_fit(&image, &options);

        assert_eq!((resized.width(), resized.height()), (300, 200));
    }

    #[test]
    fn upscaling_happens_when_explicitly_allowed() {
        let image = sample_image(300, 200);
        let options = ProcessOptions {
            max_width: Some(600),
            max_height: Some(600),
            allow_upscale: true,
            ..Default::default()
        };

        let resized = resize_to_fit(&image, &options);

        assert_eq!((resized.width(), resized.height()), (600, 400));
    }

    #[test]
    fn each_format_writes_a_decodable_file() {
        let dir = std::env::temp_dir().join("imageflow-format-tests");
        std::fs::create_dir_all(&dir).unwrap();

        let image = sample_image(120, 80);
        let source = dir.join("source.png");
        image.save(&source).unwrap();

        for format in [OutputFormat::Jpeg, OutputFormat::Png, OutputFormat::Webp] {
            let options = ProcessOptions {
                max_width: Some(60),
                max_height: Some(60),
                format,
                ..Default::default()
            };

            let outcome = process_file(&source, &dir, &options, None).unwrap();

            assert_eq!(outcome.output_width, 60);
            assert!(outcome.output_bytes > 0, "{format:?} produced an empty file");
            image::open(&outcome.output_path)
                .unwrap_or_else(|e| panic!("{format:?} output is not decodable: {e}"));
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn lossless_webp_round_trips_pixels_but_lossy_does_not() {
        let dir = std::env::temp_dir().join("imageflow-webp-tests");
        std::fs::create_dir_all(&dir).unwrap();

        let original = sample_image(400, 300);
        let source = dir.join("source.png");
        original.save(&source).unwrap();

        // No bounding box, so the pixels reaching the encoder are the source pixels.
        let base = ProcessOptions {
            max_width: None,
            max_height: None,
            format: OutputFormat::Webp,
            ..Default::default()
        };

        let lossless = process_file(
            &source,
            &dir,
            &ProcessOptions {
                webp_lossless: true,
                suffix: "_lossless".into(),
                ..base.clone()
            },
            None,
        )
        .unwrap();

        let lossy = process_file(
            &source,
            &dir,
            &ProcessOptions {
                quality: 50,
                suffix: "_lossy".into(),
                ..base
            },
            None,
        )
        .unwrap();

        let expected = original.to_rgb8();
        let from_lossless = image::open(&lossless.output_path).unwrap().to_rgb8();
        let from_lossy = image::open(&lossy.output_path).unwrap().to_rgb8();

        assert_eq!(
            from_lossless.as_raw(),
            expected.as_raw(),
            "lossless webp changed the pixels"
        );
        assert_ne!(
            from_lossy.as_raw(),
            expected.as_raw(),
            "quality 50 should visibly alter the pixels"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Not an assertion, a measuring stick: `cargo test -- --ignored --nocapture`
    /// prints what a camera-sized product photo costs in each output format.
    #[test]
    #[ignore = "reports numbers, run on demand"]
    fn report_savings_for_a_camera_sized_photo() {
        let dir = std::env::temp_dir().join("imageflow-bench");
        std::fs::create_dir_all(&dir).unwrap();

        // Photo-like: smooth lit areas and soft edges with a little grain. Pure noise would be
        // a pathological worst case for JPEG entropy coding and give misleading timings.
        let mut buffer = RgbImage::new(4000, 3000);
        let mut seed: u32 = 0x9e3779b9;
        for (x, y, pixel) in buffer.enumerate_pixels_mut() {
            let fx = x as f32 / 4000.0;
            let fy = y as f32 / 3000.0;
            let shading = ((fx * 6.0).sin() * (fy * 4.0).cos() * 60.0) + 140.0;
            let vignette = 40.0 * ((fx - 0.5).powi(2) + (fy - 0.5).powi(2));

            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let grain = ((seed >> 24) as f32 / 255.0 - 0.5) * 8.0;

            let base = (shading - vignette + grain).clamp(0.0, 255.0) as u8;
            *pixel = Rgb([
                base,
                base.saturating_sub(12),
                base.saturating_sub(28),
            ]);
        }
        let source = dir.join("product_shot.jpg");
        DynamicImage::ImageRgb8(buffer)
            .save_with_format(&source, image::ImageFormat::Jpeg)
            .unwrap();

        let original_kb = std::fs::metadata(&source).unwrap().len() / 1024;
        println!("\n  source: 4000x3000 JPEG, {original_kb} KB");

        for (label, options) in [
            (
                "WebP q82  ",
                ProcessOptions {
                    format: OutputFormat::Webp,
                    quality: 82,
                    suffix: "_webp".into(),
                    ..Default::default()
                },
            ),
            (
                "JPEG q82  ",
                ProcessOptions {
                    format: OutputFormat::Jpeg,
                    quality: 82,
                    suffix: "_jpeg".into(),
                    ..Default::default()
                },
            ),
            (
                "PNG       ",
                ProcessOptions {
                    format: OutputFormat::Png,
                    suffix: "_png".into(),
                    ..Default::default()
                },
            ),
        ] {
            let started = std::time::Instant::now();
            let outcome = process_file(&source, &dir, &options, None).unwrap();
            let saved =
                100.0 - (outcome.output_bytes as f64 / outcome.original_bytes as f64) * 100.0;
            println!(
                "  {label} {}x{}  {:>6} KB  {saved:>5.1}% smaller  ({:?})",
                outcome.output_width,
                outcome.output_height,
                outcome.output_bytes / 1024,
                started.elapsed(),
            );
        }
        println!();

        std::fs::remove_dir_all(&dir).ok();
    }

    /// How responsive the settings panel can feel: this runs on every debounced change.
    /// `cargo test -- --ignored --nocapture`
    #[test]
    #[ignore = "reports numbers, run on demand"]
    fn report_preview_cost() {
        let dir = std::env::temp_dir().join("imageflow-preview-bench");
        std::fs::create_dir_all(&dir).unwrap();

        let source = dir.join("photo.jpg");
        sample_image(4000, 3000)
            .save_with_format(&source, image::ImageFormat::Jpeg)
            .unwrap();

        let options = ProcessOptions::default();

        // Warm the file cache so we measure work, not disk.
        let _ = preview_file(&source, &options, 700);

        let started = std::time::Instant::now();
        preview_file(&source, &options, 700).unwrap();
        let cold = started.elapsed();

        // What the user feels while dragging the quality slider: the source is already
        // decoded and cached, so only resize + encode + preview rendering happen again.
        let decoded = load_oriented(&source).unwrap();
        let started = std::time::Instant::now();
        let before_uri = source_preview_uri(&decoded, 700).unwrap();
        let preview =
            preview_from_image(&decoded, &source, &options, 700, Some(before_uri), None).unwrap();
        let warm = started.elapsed();

        println!("\n  first preview of a 12MP photo (includes decode): {cold:?}");
        println!("  each settings change afterwards (cached decode):  {warm:?}");
        println!(
            "  payload over IPC: {} KB",
            (preview.before_uri.len() + preview.after_uri.len()) / 1024
        );

        // Where that time actually goes.
        let step = std::time::Instant::now();
        let resized = resize_to_fit(&decoded, &options);
        println!("\n    resize 12MP -> 1920 (Lanczos3): {:?}", step.elapsed());

        let step = std::time::Instant::now();
        let encoded = encode_to_bytes(&resized, &source, &options).unwrap();
        println!("    encode output:                 {:?}", step.elapsed());

        let step = std::time::Instant::now();
        let round_tripped = image::load_from_memory(&encoded).unwrap();
        println!("    decode it back:                {:?}", step.elapsed());

        let step = std::time::Instant::now();
        let before_thumb = decoded.thumbnail(700, 700);
        println!("    'before' thumbnail from 12MP:  {:?}", step.elapsed());

        let step = std::time::Instant::now();
        let after_thumb = round_tripped.thumbnail(700, 700);
        println!("    'after' thumbnail from 1920:   {:?}", step.elapsed());

        let step = std::time::Instant::now();
        to_png_data_uri(&before_thumb).unwrap();
        to_png_data_uri(&after_thumb).unwrap();
        println!("    two png data uris:             {:?}", step.elapsed());
        println!();

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn preview_reports_the_size_the_batch_would_actually_write() {
        let dir = std::env::temp_dir().join("imageflow-preview-tests");
        std::fs::create_dir_all(&dir).unwrap();

        let source = dir.join("photo.png");
        sample_image(1200, 900).save(&source).unwrap();

        let options = ProcessOptions {
            max_width: Some(600),
            max_height: Some(600),
            format: OutputFormat::Webp,
            quality: 70,
            ..Default::default()
        };

        let preview = preview_file(&source, &options, 300).unwrap();
        let written = process_file(&source, &dir, &options, None).unwrap();

        assert_eq!(
            preview.output_bytes, written.output_bytes,
            "preview promised a different file size than the batch produced"
        );
        assert_eq!(
            (preview.output_width, preview.output_height),
            (written.output_width, written.output_height)
        );
        assert!(preview.before_uri.starts_with("data:image/png;base64,"));
        assert!(preview.after_uri.starts_with("data:image/png;base64,"));

        std::fs::remove_dir_all(&dir).ok();
    }

    /// The whole chain: a photo shot sideways, exported with metadata kept. The pixels must
    /// come out upright *and* the orientation tag must no longer ask for a rotation, or
    /// every viewer will turn the image on its side a second time.
    #[test]
    fn preserved_metadata_does_not_rotate_the_image_twice() {
        use img_parts::{Bytes, ImageEXIF};

        let dir = std::env::temp_dir().join("imageflow-metadata-e2e");
        std::fs::create_dir_all(&dir).unwrap();

        // A wide image whose EXIF claims it needs a 90 degree turn.
        let source = dir.join("sideways.jpg");
        sample_image(400, 200)
            .save_with_format(&source, image::ImageFormat::Jpeg)
            .unwrap();

        let mut exif = Vec::new();
        exif.extend_from_slice(b"II");
        exif.extend_from_slice(&42u16.to_le_bytes());
        exif.extend_from_slice(&8u32.to_le_bytes());
        exif.extend_from_slice(&1u16.to_le_bytes());
        exif.extend_from_slice(&0x0112u16.to_le_bytes()); // orientation
        exif.extend_from_slice(&3u16.to_le_bytes());
        exif.extend_from_slice(&1u32.to_le_bytes());
        exif.extend_from_slice(&6u16.to_le_bytes()); // rotate 90 clockwise
        exif.extend_from_slice(&[0, 0]);
        exif.extend_from_slice(&0u32.to_le_bytes());

        let mut jpeg =
            img_parts::jpeg::Jpeg::from_bytes(Bytes::from(std::fs::read(&source).unwrap()))
                .unwrap();
        jpeg.set_exif(Some(Bytes::from(exif)));
        let mut tagged = Vec::new();
        jpeg.encoder().write_to(&mut tagged).unwrap();
        std::fs::write(&source, &tagged).unwrap();

        let options = ProcessOptions {
            max_width: None,
            max_height: None,
            format: OutputFormat::Keep,
            preserve_metadata: true,
            suffix: "_out".into(),
            ..Default::default()
        };

        let outcome = process_file(&source, &dir, &options, None).unwrap();

        // The decoder applied the rotation, so a 400x200 source becomes 200x400.
        assert_eq!(
            (outcome.output_width, outcome.output_height),
            (200, 400),
            "rotation should be baked into the pixels"
        );

        let written =
            img_parts::jpeg::Jpeg::from_bytes(Bytes::from(std::fs::read(&outcome.output_path).unwrap()))
                .unwrap();
        let carried = written.exif().expect("EXIF should have been carried over");

        assert_eq!(
            u16::from_le_bytes([carried[18], carried[19]]),
            1,
            "the orientation tag must be reset to 'no transform'"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// The mixed-folder case: PNGs convert, JPEGs are copied byte for byte rather than
    /// recompressed, and both land in the output folder.
    #[test]
    fn passthrough_copies_matching_files_and_converts_the_rest() {
        let dir = std::env::temp_dir().join("imageflow-passthrough");
        let out = dir.join("out");
        std::fs::create_dir_all(&out).unwrap();

        let jpeg_source = dir.join("already.jpg");
        sample_image(300, 200)
            .save_with_format(&jpeg_source, image::ImageFormat::Jpeg)
            .unwrap();
        let png_source = dir.join("needs_convert.png");
        sample_image(300, 200).save(&png_source).unwrap();

        let options = ProcessOptions {
            max_width: None,
            max_height: None,
            format: OutputFormat::Jpeg,
            passthrough_same_format: true,
            preserve_metadata: true,
            ..Default::default()
        };

        let copied = process_file(&jpeg_source, &out, &options, None).unwrap();
        let converted = process_file(&png_source, &out, &options, None).unwrap();

        assert_eq!(
            std::fs::read(&copied.output_path).unwrap(),
            std::fs::read(&jpeg_source).unwrap(),
            "a file already in the target format should be copied untouched"
        );
        assert!(
            converted.output_path.ends_with(".jpg"),
            "the png should still be converted, got {}",
            converted.output_path
        );

        // Both files reached the output folder, so no manual copying is needed.
        let produced = std::fs::read_dir(&out).unwrap().count();
        assert_eq!(produced, 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn passthrough_steps_aside_when_pixels_must_change() {
        let dir = std::env::temp_dir().join("imageflow-passthrough-resize");
        let out = dir.join("out");
        std::fs::create_dir_all(&out).unwrap();

        let source = dir.join("big.jpg");
        sample_image(800, 600)
            .save_with_format(&source, image::ImageFormat::Jpeg)
            .unwrap();

        let options = ProcessOptions {
            max_width: Some(400),
            max_height: Some(400),
            format: OutputFormat::Jpeg,
            passthrough_same_format: true,
            ..Default::default()
        };

        let outcome = process_file(&source, &out, &options, None).unwrap();

        assert_eq!(
            (outcome.output_width, outcome.output_height),
            (400, 300),
            "a resize must override the copy shortcut"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn naming_rules_produce_url_safe_filenames() {
        let hyphenate = ProcessOptions {
            format: OutputFormat::Webp,
            space_handling: SpaceHandling::Hyphen,
            ..Default::default()
        };

        assert_eq!(
            output_path_for(
                Path::new("/in/Blue Summer Dress.jpg"),
                Path::new("/out"),
                &hyphenate
            )
            .file_name()
            .unwrap(),
            "Blue-Summer-Dress.webp"
        );

        // Runs of whitespace collapse rather than producing "a--b".
        assert_eq!(
            clean_stem("spaced    out", &hyphenate),
            "spaced-out",
            "consecutive spaces should collapse into one separator"
        );

        let strip = ProcessOptions {
            space_handling: SpaceHandling::Remove,
            lowercase_names: true,
            ..Default::default()
        };
        assert_eq!(clean_stem("Blue Summer Dress", &strip), "bluesummerdress");

        let untouched = ProcessOptions::default();
        assert_eq!(clean_stem("Blue Summer Dress", &untouched), "Blue Summer Dress");
    }

    #[test]
    fn a_name_made_entirely_of_spaces_still_yields_a_filename() {
        let options = ProcessOptions {
            space_handling: SpaceHandling::Remove,
            ..Default::default()
        };

        assert_eq!(clean_stem("   ", &options), "image");
    }

    #[test]
    fn suffix_is_cleaned_along_with_the_name() {
        let options = ProcessOptions {
            format: OutputFormat::Webp,
            suffix: " web".into(),
            space_handling: SpaceHandling::Hyphen,
            ..Default::default()
        };

        let path = output_path_for(Path::new("/in/photo.jpg"), Path::new("/out"), &options);

        assert_eq!(path.file_name().unwrap(), "photo-web.webp");
    }

    #[test]
    fn keep_format_preserves_the_source_extension() {
        let options = ProcessOptions {
            format: OutputFormat::Keep,
            suffix: "_web".into(),
            ..Default::default()
        };

        let path = output_path_for(
            Path::new("/tmp/product_shot.JPEG"),
            Path::new("/out"),
            &options,
        );

        assert_eq!(path.file_name().unwrap(), "product_shot_web.jpeg");
    }
}
