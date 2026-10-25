use etagere::{AllocatorOptions, AtlasAllocator, size2};
use wgpu::*;

use super::raster::{GlyphKind, GlyphRaster};
use crate::render::Render;

pub(super) const GLYPH_ATLAS_SIZE: u32 = 2048;
pub(super) const GLYPH_PADDING: u32 = 1;

pub(super) struct GlyphPlacement {
    pub uv: [f32; 4],
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

pub(super) struct GlyphAtlasPage {
    pub kind: GlyphKind,
    pub bind: BindGroup,
    allocator: AtlasAllocator,
    _texture: Texture,
    size: u32,
}

impl GlyphAtlasPage {
    pub(super) fn new(
        kind: GlyphKind,
        render: &Render,
        atlas_layout: &BindGroupLayout,
        sampler: &Sampler,
    ) -> Self {
        let format = match kind {
            GlyphKind::Mask => TextureFormat::R8Unorm,
            GlyphKind::Color => TextureFormat::Rgba8Unorm,
        };

        let texture = render.device.create_texture(&TextureDescriptor {
            label: Some("text_glyph_atlas"),
            size: Extent3d {
                width: GLYPH_ATLAS_SIZE,
                height: GLYPH_ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&TextureViewDescriptor {
            label: Some("text_glyph_atlas_view"),
            ..Default::default()
        });

        let bind = render.device.create_bind_group(&BindGroupDescriptor {
            label: Some("text_glyph_atlas"),
            layout: atlas_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(sampler),
                },
            ],
        });

        let allocator = AtlasAllocator::with_options(
            size2(GLYPH_ATLAS_SIZE as i32, GLYPH_ATLAS_SIZE as i32),
            &AllocatorOptions {
                alignment: size2(1, 1),
                vertical_shelves: false,
                num_columns: 4,
            },
        );

        Self {
            kind,
            bind,
            allocator,
            _texture: texture,
            size: GLYPH_ATLAS_SIZE,
        }
    }

    pub(super) fn insert(&mut self, queue: &Queue, raster: &GlyphRaster) -> Option<GlyphPlacement> {
        if raster.width == 0 || raster.height == 0 {
            return None;
        }

        let extend = size2(
            (raster.width + GLYPH_PADDING * 2) as i32,
            (raster.height + GLYPH_PADDING * 2) as i32,
        );
        let allocation = self.allocator.allocate(extend)?;

        let x = allocation.rectangle.min.x + GLYPH_PADDING as i32;
        let y = allocation.rectangle.min.y + GLYPH_PADDING as i32;

        let bytes_per_pixel = match self.kind {
            GlyphKind::Mask => 1,
            GlyphKind::Color => 4,
        };

        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self._texture,
                mip_level: 0,
                origin: Origin3d {
                    x: x as u32,
                    y: y as u32,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            &raster.data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(raster.width * bytes_per_pixel),
                rows_per_image: Some(raster.height),
            },
            Extent3d {
                width: raster.width,
                height: raster.height,
                depth_or_array_layers: 1,
            },
        );

        let inv = 1.0 / self.size as f32;
        let u0 = x as f32 * inv;
        let v0 = y as f32 * inv;
        let u1 = (x as f32 + raster.width as f32) * inv;
        let v1 = (y as f32 + raster.height as f32) * inv;

        Some(GlyphPlacement {
            uv: [u0, v0, u1, v1],
            left: raster.left,
            top: raster.top,
            width: raster.width,
            height: raster.height,
        })
    }
}
