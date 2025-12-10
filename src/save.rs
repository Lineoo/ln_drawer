use std::io::Read;

use crate::{
    elements::palette::{Palette, PaletteDescriptor},
    world::World,
};

#[derive(Debug, Default, bincode::Encode, bincode::Decode)]
struct SaveFile {
    palettes: Vec<PaletteDescriptor>,
}

pub fn save_into_file(world: &World) {
    let mut save = SaveFile::default();

    world.foreach_fetch::<Palette>(|_, palette| {
        save.palettes.push(palette.to_descriptor());
    });

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
        world.build(palette);
    }
}
