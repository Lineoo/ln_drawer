use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use hashbrown::HashMap;
use serde_bytes::ByteBuf;
#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

use crate::{
    lnwin::Lnwindow,
    render::viewport::{Viewport, ViewportDescriptor},
    tools::timer::{Timer, TimerHit},
    world::{Element, Handle, World, WorldError},
};

/// Will exist between different sessions.
pub struct SaveControl(String, u64);

pub struct SaveControlRead {
    pub name: String,
    pub read: Box<dyn Fn(&World, Handle<SaveControl>)>,
}

pub struct SaveControlWrite(pub Box<dyn FnMut(&World)>);

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SaveDatabase(HashMap<u64, (String, ByteBuf)>, u64);

pub struct AutosaveScheduler {
    pub autosave_duration: Duration,
}

/// The event is triggered on [`SaveScheduler`].
/// TODO use Element instead of Event
#[deprecated]
pub struct AutosaveRequest;

impl SaveControl {
    pub fn create(name: String, world: &World, bytes: &[u8]) -> Handle<SaveControl> {
        let mut db = world.single_fetch_mut::<SaveDatabase>().unwrap();

        while db.0.contains_key(&db.1) {
            db.1 += 1;
        }

        let id = db.1;
        let compressed = zstd::encode_all(bytes, 0).unwrap();
        let ret = db.0.insert(id, (name.clone(), compressed.into()));
        debug_assert!(ret.is_none());
        world.insert(SaveControl(name, id))
    }

    pub fn read(&self, world: &World) -> Vec<u8> {
        let db = world.single_fetch::<SaveDatabase>().unwrap();
        let raw = &db.0.get(&self.1).unwrap().1;
        zstd::decode_all(raw.as_slice()).unwrap()
    }

    pub fn write(&self, world: &World, bytes: &[u8]) {
        let mut db = world.single_fetch_mut::<SaveDatabase>().unwrap();
        let buf = &mut *db.0.get_mut(&self.1).unwrap().1;
        buf.clear();

        zstd::stream::copy_encode(bytes, buf, 0).unwrap();
    }
}

impl SaveControlRead {
    fn expand_foreach(&mut self, world: &World) {
        world.foreach::<SaveControl>(|control| {
            let control = world.fetch(control).unwrap();
            if control.0 == self.name {
                let handle = control.handle();
                drop(control);
                (self.read)(world, handle);
            }
        });
    }
}

impl AutosaveScheduler {
    fn autosave(&mut self, world: &World, this: Handle<Self>) {
        let start = Instant::now();

        world.foreach_fetch_mut::<SaveControlWrite>(|mut write| {
            (write.0)(world);
        });

        world.trigger(this, &AutosaveRequest);

        let cost = Instant::now().duration_since(start);
        log::debug!("autosave request finished in {cost:?}");

        let db = world.single_fetch::<SaveDatabase>().unwrap();
        db.flush(world);
    }
}

impl SaveDatabase {
    pub fn init(world: &mut World) {
        let Err(WorldError::SingletonNoSuch(_)) = world.single::<SaveDatabase>() else {
            log::warn!("duplicated database initialization!");
            return;
        };

        let target = get_file_path(world, "world.ln-world");

        if let Ok(file) = std::fs::File::open(target) {
            let mmap = unsafe { memmap2::Mmap::map(&file).unwrap() };

            let Ok(db) = postcard::from_bytes::<SaveDatabase>(&mmap) else {
                log::error!("failed to decode world file through postcard");
                return;
            };

            for (id, (name, _)) in &db.0 {
                world.insert(SaveControl(name.clone(), *id));
            }

            world.insert(db);
            world.flush();

            log::debug!("database loaded");
        } else {
            world.insert(SaveDatabase(HashMap::new(), 0));
            world.flush();

            log::debug!("database created");
        }
    }

    pub fn flush(&self, world: &World) {
        let target = get_file_path(world, "world.ln-world");

        let Ok(()) = std::fs::create_dir_all(target.parent().unwrap()) else {
            log::warn!("failed to create target folder");
            return;
        };

        let Ok(file) = std::fs::File::create(target) else {
            log::warn!("failed to create save world file");
            return;
        };

        let Ok(_) = postcard::to_io(self, file) else {
            log::warn!("failed to encode world file through postcard");
            return;
        };

        log::debug!("database flushed");
    }
}

impl Element for SaveControl {}

impl Element for SaveControlRead {
    fn when_insert(&mut self, world: &World, _this: Handle<Self>) {
        self.expand_foreach(world);
    }
}

impl Element for SaveControlWrite {}

impl Element for AutosaveScheduler {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.dependency(this, world.single::<Lnwindow>().unwrap());

        let timer = world.insert(Timer::new(self.autosave_duration));
        world.observer(timer, move |TimerHit, world| {
            let mut fetched = world.fetch_mut(this).unwrap();
            fetched.autosave(world, this);
        });
    }

    fn when_remove(&mut self, world: &World, this: Handle<Self>) {
        self.autosave(world, this);
    }
}

impl Element for SaveDatabase {
    fn when_remove(&mut self, world: &World, _this: Handle<Self>) {
        self.flush(world);
    }
}

// [ deprecated ] //

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
        world.observer(timer, |TimerHit, world| {
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

    let target = get_file_path(world, "world.ln-world");

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
    let target = get_file_path(world, "world.ln-world");

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

pub fn get_file_path(world: &World, filename: &str) -> PathBuf {
    #[cfg(target_os = "android")]
    if let Ok(app) = world.single_fetch::<AndroidApp>()
        && let Some(mut path) = app.external_data_path()
    {
        path.push(filename);
        return path;
    }

    if let Some(mut path) = dirs::data_local_dir() {
        path.push("LnDrawer");
        path.push(filename);
        return path;
    }

    log::error!("failed to get data directory");
    PathBuf::from(filename)
}
