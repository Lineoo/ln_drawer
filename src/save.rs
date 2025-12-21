use crate::{
    elements::{
        image::Image, palette::{Palette, PaletteDescriptor}, stroke::{StrokeLayer, StrokeLayerDescriptor}
    },
    interface::PainterDescriptor,
    world::World,
};

#[derive(Debug, Default, bincode::Encode, bincode::Decode)]
struct SaveFile {
    palettes: Vec<PaletteDescriptor>,
    images: Vec<PainterDescriptor>,
    stroke: Option<StrokeLayerDescriptor>,
}

pub fn save_into_file(world: &World) {
    let mut save = SaveFile::default();

    world.foreach_fetch::<Palette>(|_, palette| {
        save.palettes.push(palette.to_descriptor());
    });

    world.foreach_fetch::<Image>(|_, image| {
        save.images.push(image.to_descriptor());
    });

    if let Some(stroke) = world.single_fetch_mut::<StrokeLayer>() {
        save.stroke.replace(stroke.to_descriptor());
    }

    let Ok(()) = std::fs::create_dir_all("target") else {
        log::warn!("failed to create target folder");
        return;
    };

    let Ok(mut file) = std::fs::File::create("target/world.ln-world") else {
        log::warn!("failed to create save world file");
        return;
    };

    let Ok(_) = bincode::encode_into_std_write(save, &mut file, bincode::config::standard()) else {
        log::warn!("failed to encode world file through bincode");
        return;
    };
}

pub fn read_from_file(world: &World) {
    let Ok(mut file) = std::fs::File::open("target/world.ln-world") else {
        log::warn!("failed to read world file");
        return;
    };

    let Ok(save): Result<SaveFile, _> =
        bincode::decode_from_std_read(&mut file, bincode::config::standard())
    else {
        log::warn!("failed to decode world file through bincode");
        return;
    };

    for palette in save.palettes {
        world.insert(world.build(palette));
    }

    for image in save.images {
        world.insert(Image::new(image, &mut world.single_fetch_mut().unwrap()));
    }

    if let Some(stroke) = save.stroke {
        world.insert(world.build(stroke));
    }
}
