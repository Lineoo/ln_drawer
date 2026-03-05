use std::{path::PathBuf, time::Duration};

#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

use crate::{
    lnwin::Lnwindow,
    render::viewport::{Viewport, ViewportDescriptor},
    tools::timer::{Timer, TimerHit},
    world::{Element, Handle, World},
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
    viewport: Option<ViewportDescriptor>,
}

pub fn save_into_file(world: &World) {
    let mut save = SaveFile::default();

    if let Ok(viewport) = world.single_fetch_mut::<Viewport>() {
        save.viewport = Some(ViewportDescriptor {
            size: viewport.size,
            center: viewport.center,
            zoom: viewport.zoom,
        });
    }

    let target = get_file_path(world);

    let Ok(()) = std::fs::create_dir_all(target.parent().unwrap()) else {
        log::warn!("failed to create target folder");
        return;
    };

    let Ok(file) = std::fs::File::create(target) else {
        log::warn!("failed to create save world file");
        return;
    };

    let Ok(_) = postcard::to_io(&save, file) else {
        log::warn!("failed to encode world file through postcard");
        return;
    };

    log::debug!("world saved");
}

pub fn load_from_file(world: &World) {
    let target = get_file_path(world);

    let Ok(file) = std::fs::File::open(target) else {
        log::debug!("no world file");
        return;
    };

    let Ok((save, _)) = postcard::from_io::<SaveFile, _>((file, &mut [0u8; 0xFFFF])) else {
        log::warn!("failed to decode world file through postcard");
        return;
    };

    if let Some(save_viewport) = save.viewport {
        let mut viewport = world.single_fetch_mut::<Viewport>().unwrap();
        viewport.center = save_viewport.center;
        viewport.zoom = save_viewport.zoom;
    }

    log::debug!("world loaded");
}

pub fn get_file_path(world: &World) -> PathBuf {
    #[cfg(target_os = "android")]
    if let Ok(app) = world.single_fetch::<AndroidApp>()
        && let Some(mut path) = app.external_data_path()
    {
        path.push("world.ln-world");
        return path;
    }

    if let Some(mut path) = dirs::data_local_dir() {
        path.push("LnDrawer/world.ln-world");
        return path;
    }

    log::error!("failed to get data directory");
    PathBuf::from("world.ln-world")
}
