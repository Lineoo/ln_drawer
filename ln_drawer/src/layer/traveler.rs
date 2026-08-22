use std::{collections::VecDeque, sync::Arc};

use glam::{IVec2, UVec2};
use guillotiere::{AllocId, AtlasAllocator, euclid::Size2D};
use wgpu::{Extent3d, Texture, TextureDescriptor, TextureDimension, TextureUsages};

use crate::{
    layer::{CHUNK_TEXTURE_FORMAT, ChunkKey, Layer, LayerPipeline, chunk_to_rect, rect_to_chunks},
    measures::Rectangle,
};

const ATLAS_TEXTURE_SIZE: UVec2 = UVec2::splat(2048);

/// The undo/redo system
pub struct Traveler {
    layer: Arc<LayerPipeline>,
    undos: VecDeque<Snapshot>,
    redos: VecDeque<Snapshot>,
    atlas: Vec<AtlasTexture>,
}

/// snapshot for each nodes of traveler
struct Snapshot {
    sections: Vec<SnapshotSection>,
    chunk_size: usize,
}

/// components of each snapshot
struct SnapshotSection {
    atlas_index: usize,
    atlas_rect: Rectangle,
    atlas_id: AllocId,
    origin_key: ChunkKey,
    origin_rect: Rectangle,
}

struct AtlasTexture {
    allocator: AtlasAllocator,
    texture: Texture,
}

impl Traveler {
    pub fn stock(&mut self, main: &Layer, dirty: Rectangle) {
        // 1. split sections

        let mut keys = Vec::new();
        let (start, end) = rect_to_chunks(dirty, 0, main.chunk_size);
        for x in start.0..end.0 {
            for y in start.1..end.1 {
                let key = (x, y, 0u8);
                keys.push(key);
            }
        }

        // 2. allocate atlas

        let mut sections = Vec::with_capacity(keys.len());
        for key in keys {
            let rect = chunk_to_rect(key, main.chunk_size);
            let size = Size2D::from(rect.extend.as_ivec2().to_array());
            let atlas = (self.atlas)
                .iter_mut()
                .enumerate()
                .find_map(|(i, atlas)| atlas.allocator.allocate(size).map(|alc| (i, alc)))
                .or_else(|| {
                    let (i, atlas) = self.new_atlas();
                    atlas.allocator.allocate(size).map(|alc| (i, alc))
                });

            if let Some((index, allocation)) = atlas {
                sections.push(SnapshotSection {
                    atlas_index: index,
                    atlas_rect: Rectangle::new_minmax(
                        IVec2::from_array(allocation.rectangle.min.to_array()),
                        IVec2::from_array(allocation.rectangle.max.to_array()),
                    ),
                    atlas_id: allocation.id,
                    origin_key: key,
                    origin_rect: rect,
                });
            } else {
                log::error!("failed to alloc new altas!");
            }
        }

        // 3. copy texture

        for section in &sections {
            let Some(chunk) = main.chunks.get(&section.origin_key) else {
                log::warn!("failed to copy texture: chunk does not exist");
                continue;
            };

            
        }
    }

    pub fn undo(&mut self) {}

    pub fn redo(&mut self) {}

    fn new_atlas(&mut self) -> (usize, &mut AtlasTexture) {
        let index = self.atlas.len();

        let texture = self.layer.device.create_texture(&TextureDescriptor {
            label: Some("traveler_atlas"),
            size: Extent3d {
                width: ATLAS_TEXTURE_SIZE.x,
                height: ATLAS_TEXTURE_SIZE.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: CHUNK_TEXTURE_FORMAT,
            usage: TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        let allocator = AtlasAllocator::new(Size2D::from(ATLAS_TEXTURE_SIZE.as_ivec2().to_array()));
        let atlas = self.atlas.push_mut(AtlasTexture { allocator, texture });

        (index, atlas)
    }
}
