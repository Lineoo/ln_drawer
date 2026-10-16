pub mod chunk;

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use ln_world::{Element, Handle, World, WorldError};
use redb::{Database, WriteTransaction};
use toml_edit::{DocumentMut, Item, Value};

#[cfg(target_os = "android")]
use crate::lnwin::LnAndroid;
use crate::{
    layer::input::{OffsetCorrection, TiltCorrection},
    lnwin::Lnwindow,
    render::camera::PageCameraArchive,
    tools::timer::{Timer, TimerHit},
};

/// The format version of the database.
const DATABASE_VERSION: u32 = 0;

/// The number of backup files.
const BACKUP_SLOT: u32 = 3;

/// The minimum duration before creating another save backup
const BACKUP_MINIMUM_DURATION: Duration = Duration::from_hours(72);

/// The core database.
pub struct Save {
    sn: SceneFile,
    db: Arc<Database>,
}

impl Save {
    pub fn init(world: &World) {
        let Err(WorldError::SingletonNoSuch(_)) = world.try_single::<Save>() else {
            log::warn!("duplicated database initialization!");
            return;
        };

        let db_file = Save::db_file(world);
        let sn_file = Save::sn_file(world);
        let sn_tmp_file = Save::sn_tmp_file(world);

        std::fs::create_dir_all(&db_file.parent().unwrap()).unwrap();
        Save::create_backup(&db_file, "old", true, BACKUP_SLOT);

        let sn = SceneFile::read(&sn_file);
        let db = if let Ok(mut db) = Database::open(&db_file) {
            log::debug!("database loaded");
            Save::touch(&sn, &sn_file, &sn_tmp_file, &mut db, &db_file).unwrap();
            Arc::new(db)
        } else {
            let mut db = Database::create(&db_file).unwrap();
            log::debug!("database created");
            Save::touch(&sn, &sn_file, &sn_tmp_file, &mut db, &db_file).unwrap();
            Arc::new(db)
        };

        world.insert(Save { sn, db });
    }

    pub fn sn(&self) -> &SceneFile {
        &self.sn
    }

    pub fn db(&self) -> Arc<Database> {
        self.db.clone()
    }

    pub fn db_file(world: &World) -> PathBuf {
        get_file_path(world, "world.lndb")
    }

    pub fn sn_file(world: &World) -> PathBuf {
        get_file_path(world, "world.toml")
    }

    pub fn sn_tmp_file(world: &World) -> PathBuf {
        get_file_path(world, "world.toml.tmp")
    }

    /// Touch a existed database, including updating necessary timestamps,
    /// validation, and most of all migration data from older versions.
    fn touch(
        sn: &SceneFile,
        sn_file: &Path,
        sn_tmp_file: &Path,
        db: &mut Database,
        db_file: &Path,
    ) -> Result<(), redb::Error> {
        let mut sn_doc = SceneDocument::read(sn_file);

        if sn.database_version > DATABASE_VERSION {
            panic!(
                "cannot open database from newer version {}",
                sn.database_version
            );
        } else if sn.database_version < DATABASE_VERSION {
            Self::create_backup(db_file, "migration", false, 256);
            Self::migrate_format(db, sn.database_version)?;
            sn_doc.set_database_version(DATABASE_VERSION);
        }

        if chunk::legacy_classic::need_migration(db)? {
            Self::create_backup(db_file, "classic", false, 4);
            chunk::legacy_classic::startup_migration(db)?;
        }

        if sn.compact_on_startup {
            Self::perform_compact(db)?;
            sn_doc.set_compact_on_startup(false);
        }

        sn_doc.write(sn_file, sn_tmp_file);

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

/// read-only deserialized copy from origin scene file
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SceneFile {
    pub database_version: u32,
    #[serde(default)]
    pub compact_on_startup: bool,
    #[serde(default)]
    pub page_camera: PageCameraArchive,
    pub tilt_correction: Option<TiltCorrection>,
    pub offset_correction: Option<OffsetCorrection>,
}

impl SceneFile {
    pub fn read(sn_file: &Path) -> Self {
        match std::fs::read_to_string(&sn_file) {
            Ok(text) => match toml_edit::de::from_str(&text) {
                Ok(sn) => sn,
                Err(err) => {
                    log::warn!("scene toml parse error: {err}, use default scene to continue");
                    SceneFile::default()
                }
            },
            Err(err) => match err.kind() {
                ErrorKind::NotFound => {
                    log::warn!("scene not found: {err}, use default scene to continue");
                    SceneFile::default()
                }
                err => {
                    log::warn!("scene read failed: {err}, use default scene to continue");
                    SceneFile::default()
                }
            },
        }
    }
}

impl Default for SceneFile {
    fn default() -> Self {
        Self {
            database_version: DATABASE_VERSION,
            compact_on_startup: false,
            page_camera: PageCameraArchive::default(),
            tilt_correction: None,
            offset_correction: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct SceneDocument {
    doc: DocumentMut,
    // false if document is from a malformed parsing process
    safe: bool,
}

impl SceneDocument {
    pub fn read(sn_file: &Path) -> Self {
        match std::fs::read_to_string(&sn_file) {
            Ok(text) => match text.parse::<DocumentMut>() {
                Ok(doc) => SceneDocument { doc, safe: true },
                Err(err) => {
                    log::warn!("scene toml parse error: {err}, use empty toml to continue");
                    SceneDocument::default()
                }
            },
            Err(err) => match err.kind() {
                ErrorKind::NotFound => SceneDocument {
                    doc: DocumentMut::new(),
                    safe: true,
                },
                err => {
                    log::warn!("scene read failed: {err}, use empty toml to continue");
                    SceneDocument::default()
                }
            },
        }
    }

    pub fn set_database_version(&mut self, version: u32) {
        self.doc["database_version"] = toml_edit::value(version as i64);
    }

    pub fn set_compact_on_startup(&mut self, compact: bool) {
        self.doc["compact_on_startup"] = toml_edit::value(compact);
    }

    pub fn set_page_camera(&mut self, camera: PageCameraArchive) {
        let camera = toml_edit::ser::to_document(&camera).unwrap();
        self.doc["page_camera"] = match camera.into_item() {
            Item::Table(t) => Item::Table(t),
            Item::Value(Value::InlineTable(t)) => Item::Table(t.into_table()),
            it => it,
        };
    }

    pub fn write(&self, sn_file: &Path, sn_tmp_file: &Path) {
        if !self.safe {
            log::warn!("skip doc write since it is invalid");
            return;
        }

        std::fs::write(&sn_tmp_file, self.doc.to_string()).unwrap();
        std::fs::rename(&sn_tmp_file, &sn_file).unwrap();
    }
}

pub struct Autosave(pub Box<dyn FnMut(&World, AutosaveTask) + Send>);

pub struct AutosaveTask<'a> {
    #[expect(unused)]
    pub write: &'a WriteTransaction,
    #[expect(unused)]
    pub sn: &'a SceneFile,
    pub sn_doc: &'a mut SceneDocument,
}

pub struct AutosaveScheduler {
    pub autosave_duration: Duration,
}

impl Autosave {
    pub fn autosave_all(world: &World) {
        let start = Instant::now();

        let sn_file = Save::sn_file(world);
        let sn_tmp_file = Save::sn_tmp_file(world);

        let save = world.single_fetch_mut::<Save>();
        let mut sn_doc = SceneDocument::read(&sn_file);
        let mut write = save.db.begin_write().unwrap();
        write.set_quick_repair(true);
        world.foreach_fetch_mut::<Autosave>(|mut task| {
            (task.0)(
                world,
                AutosaveTask {
                    write: &write,
                    sn: &save.sn,
                    sn_doc: &mut sn_doc,
                },
            );
        });
        sn_doc.write(&sn_file, &sn_tmp_file);
        write.commit().unwrap();

        let duration = Instant::now().duration_since(start);
        log::debug!("autosave request finished in {duration:?}");
    }
}

#[cfg(target_os = "android")]
pub fn get_file_path(world: &World, filename: &str) -> PathBuf {
    let app = world.single_fetch::<LnAndroid>();
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
