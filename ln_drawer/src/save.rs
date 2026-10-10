pub mod chunk;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use ln_world::{Element, Handle, World, WorldError};
use redb::{Database, WriteTransaction};
use toml_edit::{DocumentMut, Item};

#[cfg(target_os = "android")]
use crate::lnwin::LnAndroid;
use crate::{
    lnwin::Lnwindow,
    render::camera::RootCameraDescriptor,
    tools::timer::{Timer, TimerHit},
};

/// The format version of the database.
const DATABASE_VERSION: u32 = 0;

/// The number of backup files.
const BACKUP_SLOT: u32 = 6;

/// The minimum duration before creating another save backup
const BACKUP_MINIMUM_DURATION: Duration = Duration::from_hours(24);

/// The core database.
pub struct Save {
    pub sn: Scene,
    pub db: Arc<Database>,
}

impl Save {
    pub fn init(world: &World) {
        let Err(WorldError::SingletonNoSuch(_)) = world.try_single::<Save>() else {
            log::warn!("duplicated database initialization!");
            return;
        };

        let db_file = get_file_path(world, "world.lndb");
        let sn_file = get_file_path(world, "world.toml");

        std::fs::create_dir_all(&db_file.parent().unwrap()).unwrap();
        Save::create_backup(&db_file, "old", true, BACKUP_SLOT);

        let mut sn = if let Ok(sn) = std::fs::read(&sn_file) {
            let sn = toml_edit::de::from_slice(&sn).unwrap();
            log::debug!("scene loaded");
            sn
        } else {
            let sn = Scene::default();
            log::debug!("scene created");
            let sn_tmp_file = get_file_path(world, "world.toml");
            sn.write_disk(&sn_file, &sn_tmp_file);
            sn
        };

        let db = if let Ok(mut db) = Database::open(&db_file) {
            Save::touch(&mut sn, &mut db, &db_file).unwrap();
            log::debug!("database loaded");
            Arc::new(db)
        } else {
            let db = Database::create(&db_file).unwrap();
            log::debug!("database created");
            Arc::new(db)
        };

        world.insert(Save { sn, db });
    }

    /// Touch a existed database, including updating necessary timestamps,
    /// validation, and most of all migration data from older versions.
    fn touch(sn: &mut Scene, db: &mut Database, db_file: &Path) -> Result<(), redb::Error> {
        if sn.database_version > DATABASE_VERSION {
            panic!(
                "cannot open database from newer version {}",
                sn.database_version
            );
        } else if sn.database_version < DATABASE_VERSION {
            Self::create_backup(db_file, "migration", false, 256);
            Self::migrate_format(db, sn.database_version)?;
            sn.database_version = DATABASE_VERSION;
        }

        if chunk::legacy_classic::need_migration(db)? {
            Self::create_backup(db_file, "classic", false, 4);
            chunk::legacy_classic::startup_migration(db)?;
        }

        if sn.compact_on_startup {
            Self::perform_compact(db)?;
            sn.compact_on_startup = false;
        }

        Ok(())
    }

    fn migrate_format(db: &Database, version: u32) -> Result<(), redb::Error> {
        log::info!("start migration from {version} to {DATABASE_VERSION}");

        let write = db.begin_write()?;
        #[expect(unreachable_code)]
        for migrate_format in version..DATABASE_VERSION {
            match migrate_format {
                _ => unimplemented!("unsupported migration {migrate_format}"),
            }

            log::info!("finish migration from {migrate_format}");
        }
        write.commit()?;

        log::info!("migration all finished");
        Ok(())
    }

    fn perform_compact(db: &mut Database) -> Result<(), redb::Error> {
        log::debug!("database compact started");
        let result = db.compact()?;
        log::debug!("database compact finished, result: {result}");

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
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Scene {
    pub database_version: u32,
    pub compact_on_startup: bool,
    pub main_camera: RootCameraDescriptor,
}

impl Scene {
    pub fn write_disk(&self, sn_file: &Path, sn_tmp_file: &Path) {
        let mut sn_doc = match std::fs::read_to_string(sn_file) {
            Ok(sn_str) => match sn_str.parse::<DocumentMut>() {
                Ok(sn_doc) => sn_doc,
                Err(err) => {
                    log::error!("toml parse error: {err}");
                    DocumentMut::new()
                }
            },
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound => DocumentMut::new(),
                _ => {
                    log::error!("unhandled error: {err}");
                    DocumentMut::new()
                }
            },
        };
        let new_doc = toml_edit::ser::to_document(self).unwrap();
        for (key, item) in new_doc.iter() {
            let item = item.clone();
            let item = match item.into_table() {
                Ok(x) => Item::from(x),
                Err(x) => x,
            };
            sn_doc.insert(key, item);
        }
        std::fs::write(&sn_tmp_file, sn_doc.to_string()).unwrap();
        std::fs::rename(&sn_tmp_file, &sn_file).unwrap();
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            database_version: DATABASE_VERSION,
            compact_on_startup: false,
            main_camera: RootCameraDescriptor::default(),
        }
    }
}

pub struct Autosave(pub Box<dyn FnMut(&World, AutosaveTask) + Send>);

pub struct AutosaveTask<'a> {
    #[expect(unused)]
    pub write: &'a WriteTransaction,
    pub scene: &'a mut Scene,
}

pub struct AutosaveScheduler {
    pub autosave_duration: Duration,
}

impl Autosave {
    pub fn autosave_all(world: &World) {
        let start = Instant::now();

        let mut save = world.single_fetch_mut::<Save>();
        let mut write = save.db.begin_write().unwrap();
        write.set_quick_repair(true);
        world.foreach_fetch_mut::<Autosave>(|mut task| {
            (task.0)(
                world,
                AutosaveTask {
                    write: &write,
                    scene: &mut save.sn,
                },
            );
        });
        write.commit().unwrap();

        let sn_file = get_file_path(world, "world.toml");
        let sn_tmp_file = get_file_path(world, "world.toml.tmp");
        save.sn.write_disk(&sn_file, &sn_tmp_file);

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
        world.dependency(this, world.single::<Lnwindow>());

        let timer = world.insert(Timer::new(self.autosave_duration));
        world.observer(timer, move |TimerHit, world| {
            Autosave::autosave_all(world);
        });
    }
}

impl Element for Save {}
