use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

use crate::{
    lnwin::Lnwindow,
    tools::timer::{Timer, TimerHit},
    world::{Element, Handle, World, WorldError},
};

const BACKUP_SLOT: u32 = 6;
const CONTROLS_TABLE: TableDefinition<u64, (&str, &[u8])> = TableDefinition::new("controls");

/// Will exist between different sessions.
pub struct SaveControl(String, u64);

pub struct SaveControlRead {
    pub name: String,
    pub read: Box<dyn Fn(&World, Handle<SaveControl>)>,
}

pub struct SaveControlWrite(pub Box<dyn FnMut(&World)>);

pub struct SaveDatabase(Database, u64);

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
        let write = db.0.begin_write().unwrap();
        let mut table = write.open_table(CONTROLS_TABLE).unwrap();

        while table.get(&db.1).unwrap().is_some() {
            db.1 += 1;
        }

        let compressed = zstd::encode_all(bytes, 0).unwrap();
        table.insert(db.1, (&name[..], &compressed[..])).unwrap();

        drop(table);
        write.commit().unwrap();

        world.insert(SaveControl(name, db.1))
    }

    pub fn read(&self, world: &World) -> Vec<u8> {
        let db = world.single_fetch::<SaveDatabase>().unwrap();
        let read = db.0.begin_read().unwrap();
        let table = read.open_table(CONTROLS_TABLE).unwrap();

        let access = table.get(&self.1).unwrap().unwrap();
        let compressed = access.value().1;
        zstd::decode_all(compressed).unwrap()
    }

    pub fn write(&self, world: &World, bytes: &[u8]) {
        let db = world.single_fetch::<SaveDatabase>().unwrap();
        let write = db.0.begin_write().unwrap();
        let mut table = write.open_table(CONTROLS_TABLE).unwrap();

        let access = table.get(&self.1).unwrap().unwrap();
        let name = String::from(access.value().0);
        let compressed = zstd::stream::encode_all(bytes, 0).unwrap();

        drop(access);
        table.insert(self.1, (&name[..], &compressed[..])).unwrap();

        drop(table);
        write.commit().unwrap();
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

        let duration = Instant::now().duration_since(start);
        log::debug!("autosave request finished in {duration:?}");
    }
}

impl SaveDatabase {
    pub fn init(world: &mut World) {
        let Err(WorldError::SingletonNoSuch(_)) = world.single::<SaveDatabase>() else {
            log::warn!("duplicated database initialization!");
            return;
        };

        let target = get_file_path(world, "world.lndb");
        SaveDatabase::create_backup(&target);
        if let Ok(db) = Database::open(&target)
            && let Ok(id) = SaveDatabase::read_redb(world, &db)
        {
            world.insert(SaveDatabase(db, id));
            log::debug!("database loaded");
        } else {
            let db = Database::create(&target).unwrap();

            world.insert(SaveDatabase(db, 0));
            log::debug!("database created");
        }

        world.flush();
    }

    fn read_redb(world: &World, db: &Database) -> Result<u64, redb::Error> {
        let read = db.begin_read()?;
        let table = read.open_table(CONTROLS_TABLE)?;
        let mut max_id = 0;
        for entry in table.range::<u64>(..)? {
            let entry = entry?;
            let id = entry.0.value();
            let name = entry.1.value().0;
            world.insert(SaveControl(name.into(), id));

            if id > max_id {
                max_id = id;
            }
        }

        Ok(max_id)
    }

    fn create_backup(target: &Path) {
        let Ok(true) = std::fs::exists(target) else {
            return;
        };

        let mut backup = PathBuf::new();
        let mut temp = PathBuf::new();
        let mut oldest = Duration::ZERO;
        for i in 0..BACKUP_SLOT {
            temp.clear();
            temp.push(target);
            temp.add_extension(&i.to_string());
            temp.add_extension("old");

            let Ok(metadata) = std::fs::metadata(&backup) else {
                backup.clone_from(&temp);
                break;
            };

            let Ok(modified) = metadata.modified() else {
                backup.clone_from(&temp);
                break;
            };

            let duration = SystemTime::now()
                .duration_since(modified)
                .unwrap_or_default();

            if duration > oldest {
                backup.clone_from(&temp);
                oldest = duration;
            }
        }

        log::debug!("backup file is written to {backup:?}");
        std::fs::copy(target, backup).unwrap();
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
}

impl Element for SaveDatabase {}

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
