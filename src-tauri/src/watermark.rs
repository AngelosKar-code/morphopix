//! Compositing a logo onto processed images.
//!
//! Size and margin are expressed as a share of the output width rather than in pixels: the
//! same batch routinely mixes a 4000px product shot with an 800px thumbnail, and a fixed
//! pixel logo would swamp one and vanish on the other.

use image::{DynamicImage, GenericImageView, RgbaImage};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum WatermarkPosition {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatermarkOptions {
    pub path: String,
    pub position: WatermarkPosition,
    /// Logo width as a percentage of the output width.
    pub size_percent: f32,
    /// 0 = invisible, 100 = as supplied.
    pub opacity_percent: f32,
    /// Distance from the edges, as a percentage of the output width.
    pub margin_percent: f32,
}

/// A logo decoded once and reused for the whole batch. Decoding it per image would repeat
/// the same work thousands of times.
pub struct Watermark {
    image: RgbaImage,
    options: WatermarkOptions,
}

impl Watermark {
    pub fn load(options: &WatermarkOptions) -> Result<Self, String> {
        let image = image::open(&options.path)
            .map_err(|e| format!("cannot open watermark: {e}"))?
            .to_rgba8();

        if image.width() == 0 || image.height() == 0 {
            return Err("watermark image is empty".into());
        }

        Ok(Self {
            image,
            options: options.clone(),
        })
    }

    /// Draws the logo onto `canvas`, scaled and positioned relative to its size.
    pub fn apply(&self, canvas: &DynamicImage) -> DynamicImage {
        let (canvas_w, canvas_h) = canvas.dimensions();

        let target_w = ((canvas_w as f32) * (self.options.size_percent.clamp(1.0, 100.0) / 100.0))
            .round()
            .max(1.0) as u32;
        let ratio = self.image.height() as f32 / self.image.width() as f32;
        let target_h = ((target_w as f32) * ratio).round().max(1.0) as u32;

        let logo = DynamicImage::ImageRgba8(self.image.clone()).resize_exact(
            target_w,
            target_h,
            image::imageops::FilterType::CatmullRom,
        );
        let logo = fade(logo.to_rgba8(), self.options.opacity_percent);

        let margin = ((canvas_w as f32) * (self.options.margin_percent.max(0.0) / 100.0)).round()
            as i64;
        let (x, y) = place(
            self.options.position,
            canvas_w as i64,
            canvas_h as i64,
            target_w as i64,
            target_h as i64,
            margin,
        );

        let mut composed = canvas.to_rgba8();
        image::imageops::overlay(&mut composed, &logo, x, y);
        DynamicImage::ImageRgba8(composed)
    }
}

/// Scales every alpha value, so a logo can sit softly over the picture.
fn fade(mut logo: RgbaImage, opacity_percent: f32) -> RgbaImage {
    let factor = (opacity_percent.clamp(0.0, 100.0)) / 100.0;
    if (factor - 1.0).abs() < f32::EPSILON {
        return logo;
    }

    for pixel in logo.pixels_mut() {
        pixel.0[3] = (pixel.0[3] as f32 * factor).round() as u8;
    }
    logo
}

fn place(
    position: WatermarkPosition,
    canvas_w: i64,
    canvas_h: i64,
    logo_w: i64,
    logo_h: i64,
    margin: i64,
) -> (i64, i64) {
    use WatermarkPosition::*;

    let left = margin;
    let center_x = (canvas_w - logo_w) / 2;
    let right = canvas_w - logo_w - margin;

    let top = margin;
    let center_y = (canvas_h - logo_h) / 2;
    let bottom = canvas_h - logo_h - margin;

    let (x, y) = match position {
        TopLeft => (left, top),
        TopCenter => (center_x, top),
        TopRight => (right, top),
        CenterLeft => (left, center_y),
        Center => (center_x, center_y),
        CenterRight => (right, center_y),
        BottomLeft => (left, bottom),
        BottomCenter => (center_x, bottom),
        BottomRight => (right, bottom),
    };

    // A logo bigger than its canvas would otherwise be pushed off the top left corner.
    (x.max(0), y.max(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn logo(width: u32, height: u32, alpha: u8) -> RgbaImage {
        RgbaImage::from_pixel(width, height, Rgba([255, 0, 0, alpha]))
    }

    fn watermark(position: WatermarkPosition, size_percent: f32, opacity: f32) -> Watermark {
        watermark_from(logo(100, 50, 255), position, size_percent, opacity)
    }

    fn watermark_from(
        image: RgbaImage,
        position: WatermarkPosition,
        size_percent: f32,
        opacity: f32,
    ) -> Watermark {
        Watermark {
            image,
            options: WatermarkOptions {
                path: String::new(),
                position,
                size_percent,
                opacity_percent: opacity,
                margin_percent: 0.0,
            },
        }
    }

    #[test]
    fn logo_scales_to_a_share_of_the_canvas_not_to_fixed_pixels() {
        let small = DynamicImage::ImageRgba8(RgbaImage::new(400, 300));
        let large = DynamicImage::ImageRgba8(RgbaImage::new(4000, 3000));
        let mark = watermark(WatermarkPosition::TopLeft, 25.0, 100.0);

        let on_small = mark.apply(&small);
        let on_large = mark.apply(&large);

        // 25% of the width in both cases: 100px and 1000px respectively.
        assert_eq!(on_small.get_pixel(99, 0)[3], 255, "logo should reach 25% across");
        assert_eq!(on_small.get_pixel(101, 0)[3], 0, "and stop there");
        assert_eq!(on_large.get_pixel(999, 0)[3], 255);
        assert_eq!(on_large.get_pixel(1001, 0)[3], 0);
    }

    #[test]
    fn position_anchors_to_the_requested_corner() {
        let canvas = DynamicImage::ImageRgba8(RgbaImage::new(400, 400));
        let mark = watermark(WatermarkPosition::BottomRight, 25.0, 100.0);

        let result = mark.apply(&canvas);

        assert_eq!(result.get_pixel(399, 399)[3], 255, "bottom right must be covered");
        assert_eq!(result.get_pixel(0, 0)[3], 0, "top left must be untouched");
    }

    #[test]
    fn opacity_scales_the_alpha_channel() {
        let canvas = DynamicImage::ImageRgba8(RgbaImage::new(400, 400));
        let mark = watermark(WatermarkPosition::TopLeft, 25.0, 50.0);

        let result = mark.apply(&canvas);

        let alpha = result.get_pixel(10, 10)[3];
        assert!(
            (120..=135).contains(&alpha),
            "expected roughly half alpha, got {alpha}"
        );
    }

    #[test]
    fn a_logo_taller_than_the_canvas_stays_on_screen() {
        let canvas = DynamicImage::ImageRgba8(RgbaImage::new(100, 100));
        // Portrait logo at full width becomes 100x400 on a 100x100 canvas, so anchoring it
        // to the bottom would compute a negative offset and push it out of frame.
        let mark = watermark_from(
            logo(50, 200, 255),
            WatermarkPosition::BottomRight,
            100.0,
            100.0,
        );

        let result = mark.apply(&canvas);

        assert_eq!(result.get_pixel(0, 0)[3], 255, "should be clamped to the origin");
        assert_eq!(result.get_pixel(99, 99)[3], 255, "and still cover the canvas");
    }
}
