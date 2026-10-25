use cosmic_text::{CacheKey, FontSystem, SwashCache};
use swash::scale::image::Content;

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

impl GlyphRasterizer for SwashRasterizer {
    fn rasterize(&mut self, font_system: &mut FontSystem, key: CacheKey) -> Option<GlyphRaster> {
        let image = self.cache.get_image(font_system, key);
        let Some(image) = image.as_ref() else {
            log::warn!("swash cannot image");
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

        Some(GlyphRaster {
            kind,
            left: image.placement.left,
            top: image.placement.top,
            width: image.placement.width,
            height: image.placement.height,
            data: image.data.clone(),
        })
    }
}
