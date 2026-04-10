pub mod stream;

use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

use redb::{
    Database, MultimapTableDefinition, ReadableMultimapTable, ReadableTable, TableDefinition,
    WriteTransaction,
};
use serde_bytes::ByteBuf;
#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

use crate::{
    lnwin::Lnwindow,
    render::camera::Camera,
    tools::timer::{Timer, TimerHit},
    world::{Element, Handle, World, WorldError},
};

/// See [`TABLE_METADATA`] and [`SaveMetadata0`].
///
/// ### History
/// `version`: `format` (the last version that used it)
/// - `v0.1.3-alpha.2`: 0
const FORMAT_VERSION: u32 = 1;

/// The number of backup files.
const BACKUP_SLOT: u32 = 6;

const TABLE_METADATA: TableDefinition<u32, &[u8]> = TableDefinition::new("metadata");

pub struct SaveDatabase(pub Database);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, bytemuck::AnyBitPattern, bytemuck::NoUninit)]
struct SaveMetadata0 {
    /// See [`FORMAT_VERSION`]
    version: u32,
}

impl SaveDatabase {
    pub fn init(world: &mut World) {
        let Err(WorldError::SingletonNoSuch(_)) = world.single::<SaveDatabase>() else {
            log::warn!("duplicated database initialization!");
            return;
        };

        let target = get_file_path(world, "world.lndb");
        SaveDatabase::create_backup(&target);
        if let Ok(db) = Database::open(&target) {
            SaveDatabase::touch(&db).unwrap();
            world.insert(SaveDatabase(db));
            log::debug!("database loaded");
        } else {
            let db = Database::create(&target).unwrap();
            SaveDatabase::fresh(&db).unwrap();
            world.insert(SaveDatabase(db));
            log::debug!("database created");
        }

        world.flush();
    }

    /// Format a fresh, empty database, this contains initializing minimum
    /// sets of data such as metadata and format version.
    fn fresh(db: &Database) -> Result<(), redb::Error> {
        let write = db.begin_write()?;

        Self::update_metadata(&write)?;

        write.commit()?;
        Ok(())
    }

    /// Touch a existed database, including updating necessary timestamps,
    /// validation, and most of all migration data from older versions.
    fn touch(db: &Database) -> Result<(), redb::Error> {
        let write = db.begin_write()?;

        Self::migrate_format(&write)?;
        Self::update_metadata(&write)?;

        write.commit()?;
        Ok(())
    }

    fn update_metadata(write: &WriteTransaction) -> Result<(), redb::Error> {
        let mut metadata = write.open_table(TABLE_METADATA)?;
        metadata.insert(0, bytemuck::bytes_of(&SaveMetadata0::current_version()))?;
        Ok(())
    }

    fn migrate_format(write: &WriteTransaction) -> Result<(), redb::Error> {
        let metadata = write.open_table(TABLE_METADATA)?;
        let access0 = metadata.get(0)?.unwrap();
        let meta0 = *bytemuck::from_bytes::<SaveMetadata0>(access0.value());
        let from_format = meta0.version;

        if meta0.version > FORMAT_VERSION {
            panic!("cannot open database from newer version {}", meta0.version);
        } else if meta0.version == FORMAT_VERSION {
            return Ok(());
        }

        log::info!("start migration from {from_format} to {FORMAT_VERSION}");

        for migrate_format in from_format..FORMAT_VERSION {
            match migrate_format {
                0 => Self::migrate0(&write)?,
                _ => unimplemented!("unsupported migration {migrate_format}"),
            }

            log::info!("finish migration from {migrate_format}");
        }

        log::info!("migration all finished, now {FORMAT_VERSION}");
        Ok(())
    }

    /// Deprecate `SaveControl` and give custom tables to callers themselves to handle with.
    ///
    /// This migration will move StrokeLayer's chunks from main control table to their custom table.
    fn migrate0(write: &WriteTransaction) -> Result<(), redb::Error> {
        const LEGACY_TABLE_CONTROLS: TableDefinition<u64, &[u8]> = TableDefinition::new("controls");
        const LEGACY_TABLE_CONTROLS_LUT_CLASS: MultimapTableDefinition<&str, u64> =
            MultimapTableDefinition::new("controls_lut_class");
        const LEGACY_TABLE_CONTROLS_LUT_WITHIN: MultimapTableDefinition<(&str, u64), u64> =
            MultimapTableDefinition::new("controls_lut_within");

        const TABLE_STROKE: MultimapTableDefinition<(), (i32, i32)> =
            MultimapTableDefinition::new("stroke");
        const TABLE_STROKE_CHUNK: TableDefinition<(i32, i32), &[u8]> =
            TableDefinition::new("stroke_chunk");

        #[derive(serde::Serialize, serde::Deserialize)]
        struct LegacyChunkArchive {
            chunk: (i32, i32),
            bytes: ByteBuf,
        }

        // migrate old data
        {
            let controls = write.open_table(LEGACY_TABLE_CONTROLS)?;
            let class = write.open_multimap_table(LEGACY_TABLE_CONTROLS_LUT_CLASS)?;
            let mut stroke = write.open_multimap_table(TABLE_STROKE)?;
            let mut stroke_chunk = write.open_table(TABLE_STROKE_CHUNK)?;
            for chunk in class.get("canvas_chunk")? {
                let bytes = controls.get(chunk?.value())?.unwrap();
                let bytes = zstd::decode_all(bytes.value()).unwrap();
                let archive = postcard::from_bytes::<LegacyChunkArchive>(&bytes[..]).unwrap();
                let compressed = zstd::encode_all(&archive.bytes[..], 0).unwrap();
                stroke.insert((), archive.chunk)?;
                stroke_chunk.insert(archive.chunk, &compressed[..])?;
            }
        }

        // clean up old table
        {
            write.delete_table(LEGACY_TABLE_CONTROLS)?;
            write.delete_multimap_table(LEGACY_TABLE_CONTROLS_LUT_CLASS)?;
            write.delete_multimap_table(LEGACY_TABLE_CONTROLS_LUT_WITHIN)?;
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
