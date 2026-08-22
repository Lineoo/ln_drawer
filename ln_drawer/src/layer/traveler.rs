use std::{collections::VecDeque, sync::Arc};

use glam::{IVec2, UVec2};
use guillotiere::{AllocId, Allocation, AtlasAllocator, euclid::Size2D};
use smallvec::SmallVec;
use wgpu::{
    CommandEncoder, CommandEncoderDescriptor, Extent3d, Origin3d, TexelCopyTextureInfoBase,
    Texture, TextureAspect, TextureDescriptor, TextureDimension, TextureUsages,
};

use crate::{
    layer::{CHUNK_TEXTURE_FORMAT, ChunkKey, Layer, LayerPipeline, chunk_to_rect, rect_to_chunks},
    measures::Rectangle,
};

const UNDO_SNAPSHOTS_LIMIT: usize = 64;
// TODO const UNDO_ATLAS_LIMIT: usize = 8;
const ATLAS_TEXTURE_SIZE: UVec2 = UVec2::splat(2048);

/// The undo/redo system
pub struct Traveler {
    layer: Arc<LayerPipeline>,
    undos: VecDeque<Snapshot>,
    redos: Vec<Snapshot>,
    atlas: Vec<AtlasTexture>,
}

/// snapshot for each nodes of traveler
struct Snapshot {
    sections: SmallVec<[SnapshotSection; 1]>,
    dirty: Rectangle,
    chunk_size: u32,
}

/// components of each snapshot
struct SnapshotSection {
    atlas: IVec2,
    atlas_index: usize,
    atlas_id: AllocId,
    origin: IVec2,
    origin_key: ChunkKey,
    extend: UVec2,
}

struct AtlasTexture {
    allocator: AtlasAllocator,
    texture: Texture,
}

impl Traveler {
    pub fn new(layer: Arc<LayerPipeline>) -> Traveler {
        Traveler {
            layer,
            undos: VecDeque::new(),
            redos: Vec::new(),
            atlas: Vec::new(),
        }
    }

    pub fn stock(&mut self, main: &Layer, dirty: Rectangle) {
        // 0. clear redos
        for snapshot in self.redos.drain(..) {
            for section in &snapshot.sections {
                self.atlas[section.atlas_index]
                    .allocator
                    .deallocate(section.atlas_id);
            }
        }

        // 1. split sections

        let mut keys = Vec::new();
        let (start, end) = rect_to_chunks(dirty, 0, main.chunk_size);
        for x in start.0..end.0 {
            for y in start.1..end.1 {
                let key = (x, y, 0u8);
                keys.push(key);
            }
        }

        let mut encoder = self
            .layer
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("traveler_stock"),
            });

        let mut sections = SmallVec::with_capacity(keys.len());
        for key in keys {
            let chunk_rect = chunk_to_rect(key, main.chunk_size);
            let Some(origin) = chunk_rect.intersect(dirty) else {
                log::warn!("section skipped: no intersection");
                continue;
            };

            // 2. allocate atlas

            let Some((index, allocation)) = self.alloc(origin.extend) else {
                log::error!("failed to alloc new altas!");
                continue;
            };

            let atlas = IVec2::from_array(allocation.rectangle.min.to_array());
            let section = SnapshotSection {
                atlas,
                atlas_index: index,
                atlas_id: allocation.id,
                origin: (origin.origin - chunk_rect.origin).max(IVec2::ZERO),
                origin_key: key,
                extend: origin.extend,
            };

            // 3. copy textures

            let Some(chunk) = main.chunks.get(&key) else {
                log::error!("failed to find chunk");
                continue;
            };

            self.copy_into_atlas(
                &mut encoder,
                &chunk.texture,
                &self.atlas[section.atlas_index].texture,
                &section,
            );

            sections.push(section);
        }

        self.layer.queue.submit([encoder.finish()]);

        // 4. stock snapshot

        self.undos.push_back(Snapshot {
            sections,
            chunk_size: main.chunk_size,
            dirty,
        });

        // 5. remove exceeded

        while self.undos.len() > UNDO_SNAPSHOTS_LIMIT {
            let snapshot = self.undos.pop_front().unwrap();

            for section in &snapshot.sections {
                self.atlas[section.atlas_index]
                    .allocator
                    .deallocate(section.atlas_id);
            }
        }
    }

    pub fn undo(&mut self, main: &Layer) -> Option<Rectangle> {
        let Some(snapshot) = self.undos.pop_back() else {
            return None;
        };

        debug_assert_eq!(snapshot.chunk_size, main.chunk_size);

        let mut encoder = self
            .layer
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("traveler_undo"),
            });

        let mut redo_sections = SmallVec::with_capacity(snapshot.sections.len());
        for section in snapshot.sections {
            let Some(chunk) = main.chunks.get(&section.origin_key) else {
                log::warn!("failed to find undo origin chunk");
                continue;
            };

            // 1. copy back new data

            let Some((index, allocation)) = self.alloc(section.extend) else {
                log::error!("failed to alloc new altas!");
                continue;
            };

            let redo_atlas = IVec2::from_array(allocation.rectangle.min.to_array());
            let redo_section = SnapshotSection {
                atlas: redo_atlas,
                atlas_index: index,
                atlas_id: allocation.id,
                ..section
            };

            self.copy_into_atlas(
                &mut encoder,
                &chunk.texture,
                &self.atlas[redo_section.atlas_index].texture,
                &redo_section,
            );

            // 2. apply old data
            self.copy_into_origin(
                &mut encoder,
                &chunk.texture,
                &self.atlas[section.atlas_index].texture,
                &section,
            );

            // 3. flip into redo
            redo_sections.push(redo_section);

            // 4. clear old section
            self.atlas[section.atlas_index]
                .allocator
                .deallocate(section.atlas_id);
        }

        self.layer.queue.submit([encoder.finish()]);
        self.redos.push(Snapshot {
            sections: redo_sections,
            ..snapshot
        });

        Some(snapshot.dirty)
    }

    pub fn redo(&mut self, main: &Layer) -> Option<Rectangle> {
        let Some(snapshot) = self.redos.pop() else {
            return None;
        };

        debug_assert_eq!(snapshot.chunk_size, main.chunk_size);

        let mut encoder = self
            .layer
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("traveler_redo"),
            });

        let mut undo_sections = SmallVec::with_capacity(snapshot.sections.len());
        for section in snapshot.sections {
            let Some(chunk) = main.chunks.get(&section.origin_key) else {
                log::warn!("failed to find redo origin chunk");
                continue;
            };

            // 1. copy back new data

            let Some((index, allocation)) = self.alloc(section.extend) else {
                log::error!("failed to alloc new altas!");
                continue;
            };

            let undo_atlas = IVec2::from_array(allocation.rectangle.min.to_array());
            let undo_section = SnapshotSection {
                atlas: undo_atlas,
                atlas_index: index,
                atlas_id: allocation.id,
                ..section
            };

            self.copy_into_atlas(
                &mut encoder,
                &chunk.texture,
                &self.atlas[undo_section.atlas_index].texture,
                &undo_section,
            );

            // 2. apply old data
            self.copy_into_origin(
                &mut encoder,
                &chunk.texture,
                &self.atlas[section.atlas_index].texture,
                &section,
            );

            // 3. flip into undo
            undo_sections.push(undo_section);

            // 4. clear old section
            self.atlas[section.atlas_index]
                .allocator
                .deallocate(section.atlas_id);
        }

        self.layer.queue.submit([encoder.finish()]);
        self.undos.push_back(Snapshot {
            sections: undo_sections,
            ..snapshot
        });

        Some(snapshot.dirty)
    }

    pub fn undo_available(&mut self, main: &Layer) -> bool {
        let Some(snapshot) = self.undos.back() else {
            return false;
        };

        for section in &snapshot.sections {
            if !main.chunks.contains_key(&section.origin_key) {
                return false;
            };
        }

        true
    }

    pub fn redo_available(&mut self, main: &Layer) -> bool {
        let Some(snapshot) = self.redos.last() else {
            return false;
        };

        for section in &snapshot.sections {
            if !main.chunks.contains_key(&section.origin_key) {
                return false;
            };
        }

        true
    }

    fn alloc(&mut self, extend: UVec2) -> Option<(usize, Allocation)> {
        let extend = Size2D::from(extend.as_ivec2().to_array());
        (self.atlas)
            .iter_mut()
            .enumerate()
            .find_map(|(i, atlas)| atlas.allocator.allocate(extend).map(|alc| (i, alc)))
            .or_else(|| {
                let (i, atlas) = self.new_atlas();
                atlas.allocator.allocate(extend).map(|alc| (i, alc))
            })
    }

    fn copy_into_atlas(
        &self,
        encoder: &mut CommandEncoder,
        origin: &Texture,
        atlas: &Texture,
        section: &SnapshotSection,
    ) {
        encoder.copy_texture_to_texture(
            TexelCopyTextureInfoBase {
                texture: origin,
                mip_level: 0,
                origin: Origin3d {
                    x: section.origin.x as u32,
                    y: section.origin.y as u32,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            TexelCopyTextureInfoBase {
                texture: atlas,
                mip_level: 0,
                origin: Origin3d {
                    x: section.atlas.x as u32,
                    y: section.atlas.y as u32,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            Extent3d {
                width: section.extend.x,
                height: section.extend.y,
                depth_or_array_layers: 1,
            },
        );
    }

    fn copy_into_origin(
        &self,
        encoder: &mut CommandEncoder,
        origin: &Texture,
        atlas: &Texture,
        section: &SnapshotSection,
    ) {
        encoder.copy_texture_to_texture(
            TexelCopyTextureInfoBase {
                texture: atlas,
                mip_level: 0,
                origin: Origin3d {
                    x: section.atlas.x as u32,
                    y: section.atlas.y as u32,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            TexelCopyTextureInfoBase {
                texture: origin,
                mip_level: 0,
                origin: Origin3d {
                    x: section.origin.x as u32,
                    y: section.origin.y as u32,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            Extent3d {
                width: section.extend.x,
                height: section.extend.y,
                depth_or_array_layers: 1,
            },
        );
    }

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
            usage: TextureUsages::COPY_DST | TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let allocator = AtlasAllocator::new(Size2D::from(ATLAS_TEXTURE_SIZE.as_ivec2().to_array()));
        let atlas = self.atlas.push_mut(AtlasTexture { allocator, texture });

        (index, atlas)
    }
}
