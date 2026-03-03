use std::io::{Read, Write};

use crate::{
    elements::{
        palette::{Palette, PaletteDescriptor},
        stroke::{StrokeLayer, StrokeLayerDescriptor},
    },
    render::canvas::{Canvas, CanvasDescriptor},
    world::World,
};

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct SaveFile {
    palettes: Vec<PaletteDescriptor>,
    images: Vec<CanvasDescriptor>,
    stroke: Option<StrokeLayerDescriptor>,
}

pub fn save_into_file(world: &World) {
    let mut save = SaveFile::default();

    world.foreach_fetch::<Palette>(|_, palette| {
        save.palettes.push(palette.to_descriptor());
    });

    world.foreach_fetch::<Canvas>(|_, image| {
        save.images.push(image.to_descriptor());
    });

    if let Ok(stroke) = world.single_fetch_mut::<StrokeLayer>() {
        save.stroke.replace(stroke.to_descriptor(world));
    }

    let Ok(()) = std::fs::create_dir_all("target") else {
        log::warn!("failed to create target folder");
        return;
    };

    let Ok(mut file) = std::fs::File::create("target/world.ln-world") else {
        log::warn!("failed to create save world file");
        return;
    };

    let Ok(bytes) = postcard::to_allocvec(&save) else {
        log::warn!("failed to encode world file through postcard");
        return;
    };

    let Ok(_) = file.write_all(&bytes) else {
        log::warn!("failed to write world file");
        return;
    };
}

pub fn read_from_file(world: &World) {
    let Ok(mut file) = std::fs::File::open("target/world.ln-world") else {
        log::warn!("failed to read world file");
        return;
    };

    let mut bytes = Vec::new();
    let Ok(_) = file.read_to_end(&mut bytes) else {
        log::warn!("failed to read world file");
        return;
    };

    let Ok(save): Result<SaveFile, _> = postcard::from_bytes(&bytes) else {
        log::warn!("failed to decode world file through bincode");
        return;
    };

    for palette in save.palettes {
        world.build(palette);
    }

    for image in save.images {
        world.build(image);
    }

    if let Some(stroke) = save.stroke {
        world.build(stroke);
    }
}
