//! Generates a folder of photo-like JPEGs for exercising the batch UI.
//!
//!     cargo run --example gen_test_images -- <output-dir> [count]
//!
//! Also writes a `subfolder/` with a tenth of the images, so the "Include subfolders"
//! toggle has something to find.

use image::{DynamicImage, Rgb, RgbImage};
use std::path::Path;

fn photo_like(width: u32, height: u32, seed_offset: u32) -> DynamicImage {
    let mut buffer = RgbImage::new(width, height);
    let mut seed: u32 = 0x9e3779b9u32.wrapping_add(seed_offset.wrapping_mul(2654435761));

    let hue_shift = (seed_offset % 7) as f32 * 12.0;

    for (x, y, pixel) in buffer.enumerate_pixels_mut() {
        let fx = x as f32 / width as f32;
        let fy = y as f32 / height as f32;

        let shading = ((fx * 6.0 + seed_offset as f32 * 0.3).sin()
            * (fy * 4.0).cos()
            * 60.0)
            + 140.0;
        let vignette = 60.0 * ((fx - 0.5).powi(2) + (fy - 0.5).powi(2));

        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let grain = ((seed >> 24) as f32 / 255.0 - 0.5) * 10.0;

        let base = (shading - vignette + grain).clamp(0.0, 255.0);
        *pixel = Rgb([
            (base + hue_shift).clamp(0.0, 255.0) as u8,
            (base - 12.0).clamp(0.0, 255.0) as u8,
            (base - 28.0 + hue_shift * 0.5).clamp(0.0, 255.0) as u8,
        ]);
    }

    DynamicImage::ImageRgb8(buffer)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let output_dir = args.next().unwrap_or_else(|| {
        eprintln!("usage: cargo run --example gen_test_images -- <output-dir> [count]");
        std::process::exit(1);
    });
    let count: u32 = args.next().and_then(|n| n.parse().ok()).unwrap_or(300);

    let root = Path::new(&output_dir);
    let sub = root.join("subfolder");
    std::fs::create_dir_all(&sub).expect("cannot create output directories");

    // A spread of resolutions, the way a real product-photo dump looks.
    let sizes = [(4000u32, 3000u32), (2400, 1600), (1600, 1200), (800, 600)];

    let started = std::time::Instant::now();
    for index in 0..count {
        let (width, height) = sizes[(index as usize) % sizes.len()];
        let image = photo_like(width, height, index);

        let target = if index % 10 == 9 {
            sub.join(format!("nested_{index:04}.jpg"))
        } else {
            root.join(format!("product_{index:04}.jpg"))
        };

        image
            .save_with_format(&target, image::ImageFormat::Jpeg)
            .expect("cannot write image");

        if (index + 1) % 25 == 0 {
            println!("  {}/{count}", index + 1);
        }
    }

    println!(
        "\nWrote {count} images to {} in {:?}",
        root.display(),
        started.elapsed()
    );
}
