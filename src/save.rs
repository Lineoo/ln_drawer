use std::{
    io::{Read, Write},
    time::Duration,
};

use crate::{
    elements::{
        palette::{Palette, PaletteDescriptor},
        stroke::{StrokeLayer, StrokeLayerDescriptor},
    }, lnwin::Lnwindow, render::canvas::{Canvas, CanvasDescriptor}, tools::timer::{Timer, TimerHit}, world::{Element, Handle, World}
};

pub struct Save {
    pub period: Duration,
}

impl Default for Save {
    fn default() -> Self {
        Save {
            period: Duration::from_secs(10),
        }
    }
}

impl Element for Save {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        load_from_file(world);
        let timer = world.insert(Timer::new(self.period));
        world.observer(timer, |TimerHit, world, _| {
            save_into_file(world);
        });

        world.dependency(this, world.single::<Lnwindow>().unwrap());
    }

    fn when_remove(&mut self, world: &World, _this: Handle<Self>) {
        save_into_file(world);
    }
}

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

    log::debug!("world saved");
}

pub fn load_from_file(world: &World) {
    let Ok(mut file) = std::fs::File::open("target/world.ln-world") else {
        log::debug!("no world file");
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

    log::debug!("world loaded");
}
