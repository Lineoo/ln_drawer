use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

use hashbrown::HashMap;
use redb::{Database, MultimapTableDefinition, ReadableDatabase, ReadableTable, TableDefinition};
#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

use crate::{
    lnwin::Lnwindow,
    render::camera::Camera,
    tools::timer::{Timer, TimerHit},
    world::{Element, Handle, World, WorldError},
};

/// See [`TABLE_METADATA`] and [`SaveMetadata0`].
const FORMAT_VERSION: u32 = 0;

/// The number of backup files.
const BACKUP_SLOT: u32 = 6;

const TABLE_METADATA: TableDefinition<u32, &[u8]> = TableDefinition::new("metadata");
const TABLE_CONTROLS: TableDefinition<u64, &[u8]> = TableDefinition::new("controls");
const TABLE_CONTROLS_LUT_CLASS: MultimapTableDefinition<&str, u64> =
    MultimapTableDefinition::new("controls_lut_class");
const TABLE_CONTROLS_LUT_WITHIN: MultimapTableDefinition<(&str, u64), u64> =
    MultimapTableDefinition::new("controls_lut_within");

/// Will exist between different sessions. Can use to bind dependency or observers.
pub struct SaveControl(u64);

pub struct SaveRead {
    pub class: String,
    pub read: Box<dyn Fn(&World, Handle<SaveControl>)>,
}

pub struct Autosave(pub Box<dyn FnMut(&World)>);

pub struct AutosaveScheduler {
    pub autosave_duration: Duration,
}

struct SaveDatabase(Database);

#[derive(Default)]
struct SaveDatabaseLuts(u64, HashMap<u64, Handle<SaveControl>>);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, bytemuck::AnyBitPattern, bytemuck::NoUninit)]
struct SaveMetadata0 {
    version: u32,
}

impl SaveControl {
    pub fn init_database(world: &mut World) {
        let Err(WorldError::SingletonNoSuch(_)) = world.single::<SaveDatabase>() else {
            log::warn!("duplicated database initialization!");
            return;
        };

        let target = get_file_path(world, "world.lndb");
        SaveDatabase::create_backup(&target);
        if let Ok(db) = Database::open(&target) {
            SaveDatabase::read_redb(world, &db).unwrap();
            world.insert(SaveDatabase(db));
            log::debug!("database loaded");
        } else {
            let db = Database::create(&target).unwrap();
            SaveDatabase::init_redb(world, &db).unwrap();
            world.insert(SaveDatabase(db));
            log::debug!("database created");
        }

        world.flush();
    }

    pub fn create(class: String, world: &World, bytes: &[u8]) -> Handle<SaveControl> {
        Self::try_create(class, world, bytes).unwrap()
    }

    pub fn create_within(
        class: String,
        within: Handle<SaveControl>,
        world: &World,
        bytes: &[u8],
    ) -> Handle<SaveControl> {
        Self::try_create_within(class, within, world, bytes).unwrap()
    }

    pub fn read(&self, world: &World) -> Vec<u8> {
        self.try_read(world).unwrap()
    }

    pub fn write(&self, world: &World, bytes: &[u8]) {
        self.try_write(world, bytes).unwrap()
    }

    pub fn try_create(
        class: String,
        world: &World,
        bytes: &[u8],
    ) -> Result<Handle<SaveControl>, redb::Error> {
        let db = world.single_fetch::<SaveDatabase>().unwrap();
        let mut luts = world.single_fetch_mut::<SaveDatabaseLuts>().unwrap();
        let write = db.0.begin_write()?;
        let mut table = write.open_table(TABLE_CONTROLS)?;
        let mut lut_class = write.open_multimap_table(TABLE_CONTROLS_LUT_CLASS)?;

        // Alloc valid save id
        while table.get(&luts.0)?.is_some() {
            luts.0 += 1;
        }

        let id = luts.0;
        let compressed = zstd::encode_all(bytes, 0).unwrap();
        table.insert(id, &compressed[..])?;
        lut_class.insert(&class[..], id)?;

        drop((table, lut_class));
        write.commit()?;

        let control = world.insert(SaveControl(id));
        luts.1.insert(id, control);

        Ok(control)
    }

    pub fn try_create_within(
        class: String,
        within: Handle<SaveControl>,
        world: &World,
        bytes: &[u8],
    ) -> Result<Handle<SaveControl>, redb::Error> {
        let db = world.single_fetch::<SaveDatabase>().unwrap();
        let mut luts = world.single_fetch_mut::<SaveDatabaseLuts>().unwrap();
        let within = world.fetch(within).unwrap().0;
        let write = db.0.begin_write()?;
        let mut table = write.open_table(TABLE_CONTROLS)?;
        let mut lut_class = write.open_multimap_table(TABLE_CONTROLS_LUT_CLASS)?;
        let mut lut_within = write.open_multimap_table(TABLE_CONTROLS_LUT_WITHIN)?;

        // Alloc valid save id
        while table.get(&luts.0)?.is_some() {
            luts.0 += 1;
        }

        let id = luts.0;
        let compressed = zstd::encode_all(bytes, 0).unwrap();
        table.insert(id, &compressed[..])?;
        lut_class.insert(&class[..], id)?;
        lut_within.insert((&class[..], within), id)?;

        drop((table, lut_class, lut_within));
        write.commit()?;

        let control = world.insert(SaveControl(id));
        luts.1.insert(id, control);

        Ok(control)
    }

    pub fn try_read(&self, world: &World) -> Result<Vec<u8>, redb::Error> {
        let db = world.single_fetch::<SaveDatabase>().unwrap();
        let read = db.0.begin_read()?;
        let table = read.open_table(TABLE_CONTROLS)?;

        let compressed = table.get(&self.0)?.unwrap();
        Ok(zstd::decode_all(compressed.value()).unwrap())
    }

    pub fn try_write(&self, world: &World, bytes: &[u8]) -> Result<(), redb::Error> {
        let db = world.single_fetch::<SaveDatabase>().unwrap();
        let write = db.0.begin_write()?;
        let mut table = write.open_table(TABLE_CONTROLS)?;

        let compressed = zstd::encode_all(bytes, 0).unwrap();
        table.insert(self.0, &compressed[..])?;

        drop(table);
        write.commit()?;

        Ok(())
    }
}

impl SaveRead {
    pub fn read(
        world: &World,
        class: &str,
        mut action: impl FnMut(&World, Handle<SaveControl>),
    ) -> Result<(), redb::Error> {
        let db = world.single_fetch::<SaveDatabase>().unwrap();
        let read = db.0.begin_read()?;
        let lut_class = read.open_multimap_table(TABLE_CONTROLS_LUT_CLASS)?;
        for id in lut_class.get(class)? {
            let luts = world.single_fetch::<SaveDatabaseLuts>().unwrap();
            let control = *luts.1.get(&id?.value()).unwrap();
            action(world, control);
        }

        Ok(())
    }

    pub fn read_single(
        world: &World,
        class: &str,
        action: impl FnOnce(&World, Option<Handle<SaveControl>>),
    ) -> Result<(), redb::Error> {
        let db = world.single_fetch::<SaveDatabase>().unwrap();
        let read = db.0.begin_read()?;
        let lut_class = read.open_multimap_table(TABLE_CONTROLS_LUT_CLASS)?;

        let mut single = None;
        for id in lut_class.get(class)? {
            let luts = world.single_fetch::<SaveDatabaseLuts>().unwrap();
            let control = *luts.1.get(&id?.value()).unwrap();
            let replaced = single.replace(control);
            if replaced.is_some() {
                log::warn!("singleton too many");
                break;
            };
        }

        action(world, single);

        Ok(())
    }
}

impl Autosave {
    pub fn autosave_all(world: &World) {
        let start = Instant::now();

        world.foreach_enter::<Camera>(|_| {
            world.foreach_fetch_mut::<Autosave>(|mut write| {
                (write.0)(world);
            });
        });

        let duration = Instant::now().duration_since(start);
        log::debug!("autosave request finished in {duration:?}");
    }

    fn write_init(&mut self, world: &World) {
        (self.0)(world);
    }
}

impl SaveDatabase {
    fn init_redb(world: &World, db: &Database) -> Result<(), redb::Error> {
        let write = db.begin_write()?;

        let mut table_metadata = write.open_table(TABLE_METADATA)?;
        table_metadata.insert(0, bytemuck::bytes_of(&SaveMetadata0::current_version()))?;

        write.open_multimap_table(TABLE_CONTROLS_LUT_CLASS)?;
        write.open_multimap_table(TABLE_CONTROLS_LUT_WITHIN)?;

        drop(table_metadata);
        write.commit()?;

        world.insert(SaveDatabaseLuts::default());
        Ok(())
    }

    fn read_redb(world: &World, db: &Database) -> Result<(), redb::Error> {
        let mut read = db.begin_read()?;

        let metadata = read.open_table(TABLE_METADATA)?;
        let access = metadata.get(0)?.unwrap();
        let metadata = bytemuck::from_bytes::<SaveMetadata0>(access.value());

        // migration
        if metadata.version > FORMAT_VERSION {
            panic!(
                "cannot open database from newer version {}",
                metadata.version
            );
        } else if metadata.version < FORMAT_VERSION {
            Self::migrate_format(world, db, metadata.version)?;

            // reopen read transaction
            read = db.begin_read()?;
        }

        let table = read.open_table(TABLE_CONTROLS)?;
        let mut cnt = 0;
        let mut luts = HashMap::new();
        for entry in table.range::<u64>(..)? {
            let id = entry?.0.value();
            let control = world.insert(SaveControl(id));
            luts.insert(id, control);

            if id > cnt {
                cnt = id;
            }
        }

        world.insert(SaveDatabaseLuts(cnt, luts));
        Ok(())
    }

    fn migrate_format(_world: &World, db: &Database, from_format: u32) -> Result<(), redb::Error> {
        let _write = db.begin_write()?;
        for migrate_format in from_format..FORMAT_VERSION {
            match migrate_format {
                _ => panic!("unsupported migration {migrate_format}"),
            }
        }

        Ok(())
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

            let Ok(metadata) = std::fs::metadata(&temp) else {
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

impl SaveMetadata0 {
    const fn current_version() -> Self {
        SaveMetadata0 {
            version: FORMAT_VERSION,
        }
    }
}

#[cfg(target_os = "android")]
pub fn get_file_path(world: &World, filename: &str) -> PathBuf {
    let app = world.single_fetch::<AndroidApp>().unwrap();
    let mut path = app.external_data_path().unwrap();
    path.push(filename);
    path
}

#[cfg(not(target_os = "android"))]
pub fn get_file_path(_world: &World, filename: &str) -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap();
    path.push("LnDrawer");
    path.push(filename);
    path
}

impl Element for SaveControl {}

impl Element for SaveRead {
    fn when_insert(&mut self, world: &World, _this: Handle<Self>) {
        Self::read(world, &self.class, &self.read).unwrap();
    }
}

impl Element for Autosave {
    fn when_insert(&mut self, world: &World, _this: Handle<Self>) {
        self.write_init(world);
    }
}

impl Element for AutosaveScheduler {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.dependency(this, world.single::<Lnwindow>().unwrap());

        let timer = world.insert(Timer::new(self.autosave_duration));
        world.observer(timer, move |TimerHit, world| {
            Autosave::autosave_all(world);
        });
    }
}

impl Element for SaveDatabase {}

impl Element for SaveDatabaseLuts {}
