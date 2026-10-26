use cosmic_text::{CacheKey, FontSystem, SwashCache};
use swash::scale::{Source, image::Content};

use crate::color::srgb::srgb_gamma_decode_f32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GlyphKind {
    Mask,
    Color,
}

impl GlyphKind {
    pub(super) fn as_u32(self) -> u32 {
        match self {
            GlyphKind::Mask => 0,
            GlyphKind::Color => 1,
        }
    }
}

pub(super) struct GlyphRaster {
    pub kind: GlyphKind,
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// Rasterizer seam: swash/tiny-skia types stay out of the atlas and renderer.
pub(super) trait GlyphRasterizer: Send {
    fn rasterize(&mut self, font_system: &mut FontSystem, key: CacheKey) -> Option<GlyphRaster>;
}

pub(super) struct SwashRasterizer {
    cache: SwashCache,
}

impl SwashRasterizer {
    pub(super) fn new() -> Self {
        Self {
            cache: SwashCache::new(),
        }
    }
}

/// Color glyphs become `P(L)` premultiplied RGBA, same as `Canvas`.
fn color_premultiplied(source: Source, data: &[u8]) -> Vec<u8> {
    // Swash premultiplies sRGB color outlines; decoded bitmaps are straight.
    let premultiplied = matches!(source, Source::ColorOutline(_));
    let mut data = data.to_vec();
    for px in data.chunks_exact_mut(4) {
        let alpha = px[3] as f32 / 255.0;
        for channel in &mut px[..3] {
            let value = *channel as f32 / 255.0;
            let straight = match premultiplied && alpha > 0.0 {
                true => value / alpha,
                false => value,
            };
            let linear = srgb_gamma_decode_f32(straight.min(1.0)) * alpha;
            *channel = (linear * 255.0).round() as u8;
        }
    }
    data
}

impl GlyphRasterizer for SwashRasterizer {
    fn rasterize(&mut self, font_system: &mut FontSystem, key: CacheKey) -> Option<GlyphRaster> {
        let image = self.cache.get_image(font_system, key);
        let Some(image) = image.as_ref() else {
            log::warn!("swash cannot image {key:?}");
            return None;
        };

        let kind = match image.content {
            Content::Mask => GlyphKind::Mask,
            Content::Color => GlyphKind::Color,
            Content::SubpixelMask => {
                log::warn!("TODO: SubpixelMask");
                return None;
            }
        };

        let data = match kind {
            GlyphKind::Mask => image.data.clone(),
            GlyphKind::Color => color_premultiplied(image.source, &image.data),
        };

        Some(GlyphRaster {
            kind,
            left: image.placement.left,
            top: image.placement.top,
            width: image.placement.width,
            height: image.placement.height,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use swash::scale::StrikeWith;

    use super::*;

    #[test]
    fn test_color_bitmap_straight() {
        let data = color_premultiplied(Source::ColorBitmap(StrikeWith::BestFit), &[128, 0, 0, 128]);
        assert_eq!(data, [28, 0, 0, 128]);
    }

    #[test]
    fn test_color_outline_premultiplied() {
        let data = color_premultiplied(Source::ColorOutline(0), &[64, 0, 0, 128]);
        assert_eq!(data, [27, 0, 0, 128]);
    }

    #[test]
    fn test_color_transparent() {
        let data = color_premultiplied(
            Source::ColorBitmap(StrikeWith::BestFit),
            &[255, 255, 255, 0],
        );
        assert_eq!(data, [0, 0, 0, 0]);
    }
}
