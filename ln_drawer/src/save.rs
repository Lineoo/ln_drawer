mod legacy;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use ln_world::{Element, Handle, World, WorldError};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition, WriteTransaction};

#[cfg(target_os = "android")]
use crate::lnwin::LnAndroid;
use crate::{
    lnwin::Lnwindow,
    render::camera::Camera,
    tools::timer::{Timer, TimerHit},
};

/// See [`TABLE_METADATA`] and [`SaveMetadata0`].
///
/// ### History
/// `version`: `format` (the last version that used it)
/// - `v0.1.3-alpha.2`: 0
/// - `v0.1.3-alpha.3`: 1
const FORMAT_VERSION: u32 = 2;

/// The number of backup files.
const BACKUP_SLOT: u32 = 6;

/// The minimum duration before creating another save backup
const BACKUP_MINIMUM_DURATION: Duration = Duration::from_hours(24);

const TABLE_METADATA: TableDefinition<u32, &[u8]> = TableDefinition::new("metadata");

/// The core database.
///
/// ## Tables
///
/// | name | key | value |
/// |------|-----|-------|
/// | `metadata`            | `u32`                     | `&[u8]`   |
/// | `stroke_chunk`        | `(u64, ChunkKey)`         | `&[u8]`   |
/// | `stroke_chunk_meta`   | `((u64, ChunkKey), u32)`  | `&[u8]`   |
/// | `camera`              | ` &str`                   | `&[u8]`   |
#[derive(Clone)]
pub struct SaveDatabase(pub Arc<Database>);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, bytemuck::AnyBitPattern, bytemuck::NoUninit)]
pub struct SaveMetadata0 {
    /// See [`FORMAT_VERSION`]
    pub version: u32,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct SaveMetadata1 {
    pub compact_on_startup: bool,
}

impl SaveDatabase {
    pub fn init(world: &World) {
        let Err(WorldError::SingletonNoSuch(_)) = world.single::<SaveDatabase>() else {
            log::warn!("duplicated database initialization!");
            return;
        };

        let file = get_file_path(world, "world.lndb");
        std::fs::create_dir_all(&file.parent().unwrap()).unwrap();
        SaveDatabase::create_backup(&file, "old", true, BACKUP_SLOT);
        if let Ok(mut db) = Database::open(&file) {
            SaveDatabase::touch(&mut db, &file).unwrap();
            world.insert(SaveDatabase(Arc::new(db)));
            log::debug!("database loaded");
        } else {
            let db = Database::create(&file).unwrap();
            SaveDatabase::fresh(&db).unwrap();
            world.insert(SaveDatabase(Arc::new(db)));
            log::debug!("database created");
        }
    }

    /// Format a fresh, empty database, this contains initializing minimum
    /// sets of data such as metadata and format version.
    fn fresh(db: &Database) -> Result<(), redb::Error> {
        let write = db.begin_write()?;

        let mut metadata = write.open_table(TABLE_METADATA)?;
        metadata.insert(0, bytemuck::bytes_of(&SaveMetadata0::current_version()))?;
        let meta1 = postcard::to_stdvec(&SaveMetadata1::default_value()).unwrap();
        metadata.insert(1, &meta1[..])?;
        drop(metadata);

        write.commit()?;
        Ok(())
    }

    /// Touch a existed database, including updating necessary timestamps,
    /// validation, and most of all migration data from older versions.
    fn touch(db: &mut Database, file: &Path) -> Result<(), redb::Error> {
        let write = db.begin_write()?;
        Self::migrate_format(&write, file)?;
        write.commit()?;

        Self::perform_compact(db)?;

        Ok(())
    }

    fn migrate_format(write: &WriteTransaction, file: &Path) -> Result<(), redb::Error> {
        let mut metadata = write.open_table(TABLE_METADATA)?;

        let access0 = metadata.get(0)?.unwrap();
        let meta0 = *bytemuck::from_bytes::<SaveMetadata0>(access0.value());
        let from_format = meta0.version;
        drop(access0);

        if meta0.version > FORMAT_VERSION {
            panic!("cannot open database from newer version {}", meta0.version);
        } else if meta0.version == FORMAT_VERSION {
            return Ok(());
        }

        SaveDatabase::create_backup(file, "migration", false, 256);
        log::info!("start migration from {from_format} to {FORMAT_VERSION}");

        for migrate_format in from_format..FORMAT_VERSION {
            match migrate_format {
                0 => legacy::migrate0(&write).unwrap(),
                1 => legacy::migrate1(&write).unwrap(),
                _ => unimplemented!("unsupported migration {migrate_format}"),
            }

            log::info!("finish migration from {migrate_format}");
        }

        // update metadata
        metadata.insert(0, bytemuck::bytes_of(&SaveMetadata0::current_version()))?;

        log::info!("migration all finished");
        Ok(())
    }

    fn perform_compact(db: &mut Database) -> Result<(), redb::Error> {
        let read = db.begin_read()?;
        let metadata = read.open_table(TABLE_METADATA)?;
        let access = metadata.get(1)?;
        drop(metadata);
        drop(read);

        if let Some(access) = access
            && let Ok(meta1) = postcard::from_bytes::<SaveMetadata1>(access.value())
            && meta1.compact_on_startup
        {
            log::debug!("database compact started");
            let result = db.compact()?;
            log::debug!("database compact finished, result: {result}");
        }

        let write = db.begin_write()?;
        let mut metadata = write.open_table(TABLE_METADATA)?;

        // update metadata
        let meta1 = postcard::to_stdvec(&SaveMetadata1::default_value()).unwrap();
        metadata.insert(1, &meta1[..])?;

        drop(metadata);
        write.commit()?;

        Ok(())
    }

    fn create_backup(file: &Path, key: &'static str, skippable: bool, slot: u32) {
        let Ok(true) = std::fs::exists(file) else {
            return;
        };

        let mut temp = PathBuf::new();
        let mut backup = PathBuf::new();
        let mut oldest_backup = PathBuf::new();
        let mut newest_backup = PathBuf::new();
        let mut oldest = None;
        let mut newest = None;
        for i in 0..slot {
            temp.clear();
            temp.push(file);
            temp.add_extension(&i.to_string());
            temp.add_extension(key);

            let Ok(metadata) = std::fs::metadata(&temp) else {
                log::debug!("backup slot {temp:?} is empty");
                backup.clone_from(&temp);
                break;
            };

            let Ok(modified) = metadata.modified() else {
                log::debug!("cannot reach metadata of {temp:?}");
                backup.clone_from(&temp);
                break;
            };

            let duration = SystemTime::now()
                .duration_since(modified)
                .unwrap_or_default();

            if oldest.is_none_or(|it| duration > it) {
                backup.clone_from(&temp);
                oldest_backup.clone_from(&temp);
                oldest = Some(duration);
            }

            if newest.is_none_or(|it| duration < it) {
                newest_backup.clone_from(&temp);
                newest = Some(duration);
            }
        }

        log::debug!("newest {newest_backup:?} {newest:?}");
        log::debug!("oldest {oldest_backup:?} {oldest:?}");

        if skippable && newest.is_some_and(|it| it < BACKUP_MINIMUM_DURATION) {
            log::debug!("backup skipped: very recent backup file");
            return;
        }

        log::debug!("backup file is written to {backup:?}");
        std::fs::copy(file, backup).unwrap();
    }

    pub fn write_compact(db: &Database) -> Result<(), redb::Error> {
        let write = db.begin_write()?;
        let mut metadata = write.open_table(TABLE_METADATA)?;

        // update metadata
        let meta1 = postcard::to_stdvec(&SaveMetadata1 {
            compact_on_startup: true,
        })
        .unwrap();
        metadata.insert(1, &meta1[..])?;

        drop(metadata);
        write.commit()?;

        Ok(())
    }
}

impl SaveMetadata0 {
    const fn current_version() -> Self {
        SaveMetadata0 {
            version: FORMAT_VERSION,
        }
    }
}

impl SaveMetadata1 {
    const fn default_value() -> Self {
        SaveMetadata1 {
            compact_on_startup: false,
        }
    }
}

pub struct Autosave(pub Box<dyn FnMut(&World, &WriteTransaction)>);

pub struct AutosaveScheduler {
    pub autosave_duration: Duration,
}

impl Autosave {
    pub fn autosave_all(world: &World) {
        let start = Instant::now();

        world.foreach_enter::<Camera>(|_| {
            let db = world.single_fetch::<SaveDatabase>().unwrap();
            let write = db.0.begin_write().unwrap();
            world.foreach_fetch_mut::<Autosave>(|mut task| {
                (task.0)(world, &write);
            });
            write.commit().unwrap();
        });

        let duration = Instant::now().duration_since(start);
        log::debug!("autosave request finished in {duration:?}");
    }
}

#[cfg(target_os = "android")]
pub fn get_file_path(world: &World, filename: &str) -> PathBuf {
    let app = world.single_fetch::<LnAndroid>().unwrap();
    let mut path = app.0.external_data_path().unwrap();
    path.push(filename);
    path
}

#[cfg(not(target_os = "android"))]
pub fn get_file_path(_world: &World, filename: &str) -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap();
    match option_env!("LNDRAWER_RELEASE").is_some() {
        true => path.push("LnDrawer"),
        false => path.push("LnDrawerDev"),
    }
    path.push(filename);
    path
}

impl Element for Autosave {}

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
