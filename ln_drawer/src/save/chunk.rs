//! Sidecar storage for canvas chunk blobs.
//!
//! Chunks used to live inside the main redb database, which made redb's startup
//! allocator scan scale with the whole canvas (~300MB). Chunks are immutable,
//! addressed by `(page, ChunkKey)` and only ever read/written one at a time, so
//! they do not need redb's ACID guarantees. Each chunk is stored as an
//! individual self-describing file instead:
//!
//! ```text
//! <db dir>/chunks/<page>/<x>_<y>_<z>.zst
//! ```
//!
//! ## File layout
//!
//! ```text
//! magic      b"LNCK"              4 bytes   fixed forever
//! format     u32 little-endian    4 bytes   fixed forever, chunk format version
//! header_len u32 little-endian    4 bytes   length of the postcard header
//! header     postcard(ChunkHeader) header_len bytes
//! payload                           rest     codec-compressed pixels
//! ```
//!
//! The leading `magic + format` eight bytes are frozen: `format` is the chunk
//! format version that drives [`chunk_migration`], so it must be readable before
//! anything else. `header_len` then frames the variable-length postcard header,
//! which readers that do not understand a newer header can still skip over.
//! The payload is whatever follows the header.

use std::{
    fs, io,
    mem::size_of,
    path::{Path, PathBuf},
};

use crate::layer::ChunkKey;

const CHUNK_HEADER_MAGIC: [u8; 4] = *b"LNCK";

/// Length of the frozen `magic + format` prefix.
const CHUNK_PREFIX_LEN: usize = CHUNK_HEADER_MAGIC.len() + size_of::<u32>();

/// Length of the fixed `magic + format + header_len` framing prefix.
const CHUNK_FRAMING_LEN: usize = CHUNK_PREFIX_LEN + size_of::<u32>();

/// The payload codec stored in the header.
const CHUNK_CODEC_ZSTD: u8 = 0;

/// Historic (now unused) flag marking a chunk as mipmapped.
#[expect(dead_code)]
const CHUNK_FLAG_MIPMAPPED: u8 = 1 << 0;

/// Current chunk format version. See [module docs](self).
pub const CHUNK_FORMAT_CURRENT: u32 = 1;

/// Postcard-encoded chunk header framing the payload.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct ChunkHeader {
    codec: u8,
    flags: u8,
}

impl ChunkHeader {
    fn new() -> Self {
        ChunkHeader {
            codec: CHUNK_CODEC_ZSTD,
            flags: 0,
        }
    }

    /// Decode a whole chunk file into `(format, header, payload)`.
    fn decode(data: &[u8]) -> io::Result<(u32, ChunkHeader, &[u8])> {
        if data.len() < CHUNK_FRAMING_LEN {
            return Err(invalid("chunk file is shorter than its framing"));
        }

        if data[..CHUNK_HEADER_MAGIC.len()] != CHUNK_HEADER_MAGIC {
            return Err(invalid("invalid chunk magic"));
        }

        let format = u32::from_le_bytes(data[4..CHUNK_PREFIX_LEN].try_into().unwrap());
        let header_len = u32::from_le_bytes(
            data[CHUNK_PREFIX_LEN..CHUNK_FRAMING_LEN]
                .try_into()
                .unwrap(),
        );

        let header_end = CHUNK_FRAMING_LEN
            .checked_add(header_len as usize)
            .filter(|end| *end <= data.len())
            .ok_or_else(|| invalid("invalid chunk header length"))?;

        let (header, _) =
            postcard::take_from_bytes::<ChunkHeader>(&data[CHUNK_FRAMING_LEN..header_end])
                .map_err(|err| invalid(format!("failed to decode chunk header: {err}")))?;

        if header.codec != CHUNK_CODEC_ZSTD {
            return Err(invalid(format!("unsupported chunk codec {}", header.codec)));
        }

        Ok((format, header, &data[header_end..]))
    }

    /// Encode a whole chunk file from `format` and an already compressed payload.
    fn encode(format: u32, payload: &[u8]) -> io::Result<Vec<u8>> {
        let header = postcard::to_stdvec(&ChunkHeader::new())
            .map_err(|err| invalid(format!("failed to encode chunk header: {err}")))?;

        let mut data = Vec::with_capacity(CHUNK_FRAMING_LEN + header.len() + payload.len());
        data.extend_from_slice(&CHUNK_HEADER_MAGIC);
        data.extend_from_slice(&format.to_le_bytes());
        data.extend_from_slice(&(header.len() as u32).to_le_bytes());
        data.extend_from_slice(&header);
        data.extend_from_slice(payload);

        Ok(data)
    }
}

/// Root directory with the metadata database, `chunks` lives next to it.
pub fn chunk_root(db_file: &Path) -> PathBuf {
    db_file
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("chunks")
}

/// File-based store of canvas chunk blobs.
///
/// Every method addresses a chunk by `(page, key)` and never needs to scan the
/// directory, so opening the store is O(1) regardless of canvas size. Writes
/// are atomic (write to a temporary file then rename).
#[derive(Clone)]
pub struct ChunkStore {
    root: PathBuf,
}

impl ChunkStore {
    /// Open (creating if needed) the chunk directory rooted at `root`.
    pub fn open(root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&root)?;
        Ok(ChunkStore { root })
    }

    /// Load and decode a chunk, or `None` when it does not exist.
    ///
    /// Outdated chunk formats are migrated in memory and rewritten to the
    /// current format before returning.
    pub fn read(&self, page: u64, key: ChunkKey) -> io::Result<Option<Vec<u8>>> {
        let path = self.path(page, key);
        let data = match fs::read(&path) {
            Ok(data) => data,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };

        let (format, _header, payload) = ChunkHeader::decode(&data)?;
        if format > CHUNK_FORMAT_CURRENT {
            return Err(invalid(format!(
                "chunk {key:?} uses newer format {format} (current {CHUNK_FORMAT_CURRENT})"
            )));
        }

        let mut bytes = zstd::decode_all(payload)?;

        if format < CHUNK_FORMAT_CURRENT {
            chunk_migration(&mut bytes, key, format);
            self.write(page, key, &bytes)?;
        }

        Ok(Some(bytes))
    }

    /// Encode and atomically store `rgba` as a chunk in the current format.
    pub fn write(&self, page: u64, key: ChunkKey, rgba: &[u8]) -> io::Result<()> {
        let compressed = zstd::encode_all(rgba, 0)?;
        let data = ChunkHeader::encode(CHUNK_FORMAT_CURRENT, &compressed)?;
        self.write_atomic(&self.path(page, key), &data)
    }

    /// Remove a chunk. Missing files are ignored.
    pub fn remove(&self, page: u64, key: ChunkKey) -> io::Result<()> {
        match fs::remove_file(self.path(page, key)) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }

    /// List every chunk currently on disk. Unrecognized files are ignored with
    /// a warning. Used by active migration sweeps.
    #[cfg_attr(not(test), expect(dead_code))] // tested, but no migration step sweeps yet
    pub fn iter_all(&self) -> io::Result<Vec<(u64, ChunkKey)>> {
        let mut chunks = Vec::new();

        let pages = match fs::read_dir(&self.root) {
            Ok(pages) => pages,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(chunks),
            Err(err) => return Err(err),
        };

        for page_entry in pages {
            let page_entry = page_entry?;
            if !page_entry.file_type()?.is_dir() {
                continue;
            }

            let page_name = page_entry.file_name();
            let Ok(page) = page_name.to_string_lossy().parse::<u64>() else {
                log::warn!("ignoring unrecognized chunk page directory {page_name:?}");
                continue;
            };

            for chunk_entry in fs::read_dir(page_entry.path())? {
                let chunk_entry = chunk_entry?;
                let path = chunk_entry.path();

                if path.extension().and_then(|ext| ext.to_str()) != Some("zst") {
                    continue;
                }

                let key = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .and_then(parse_chunk_key);

                match key {
                    Some(key) => chunks.push((page, key)),
                    None => log::warn!("ignoring unrecognized chunk file {path:?}"),
                }
            }
        }

        Ok(chunks)
    }

    /// Rewrite every chunk into the current chunk format. This is the entry
    /// point future `migrateN` steps use for changes that cannot be applied
    /// lazily on read (codec or layout changes).
    #[cfg_attr(not(test), expect(dead_code))] // tested, but no migration step sweeps yet
    pub fn migrate_all(&self) -> io::Result<()> {
        for (page, key) in self.iter_all()? {
            if let Some(bytes) = self.read(page, key)? {
                self.write(page, key, &bytes)?;
            }
        }

        Ok(())
    }

    /// Absolute path of a chunk file.
    pub fn path(&self, page: u64, key: ChunkKey) -> PathBuf {
        let (x, y, z) = key;
        self.root
            .join(page.to_string())
            .join(format!("{x}_{y}_{z}.zst"))
    }

    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temp = path.with_extension("tmp");
        fs::write(&temp, bytes)?;
        fs::rename(&temp, path)?;

        Ok(())
    }
}

#[cfg_attr(not(test), expect(dead_code))] // only reached through `iter_all`
fn parse_chunk_key(stem: &str) -> Option<ChunkKey> {
    let mut parts = stem.split('_');
    let x = parts.next()?.parse().ok()?;
    let y = parts.next()?.parse().ok()?;
    let z = parts.next()?.parse().ok()?;

    if parts.next().is_some() {
        return None;
    }

    Some((x, y, z))
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// Bring decoded chunk bytes from an older chunk format up to
/// [`CHUNK_FORMAT_CURRENT`].
pub(crate) fn chunk_migration(bytes: &mut [u8], key: ChunkKey, from: u32) {
    for migrate_format in from..CHUNK_FORMAT_CURRENT {
        match migrate_format {
            0 => {
                fn linear_to_srgb(v: f32) -> f32 {
                    return match v < 0.0031308 {
                        true => 1.055 * v.powf(1.0 / 2.4) - 0.055,
                        false => v * 12.92,
                    };
                }

                let (chunks, _) = bytes.as_chunks_mut();
                for [r, g, b, _] in chunks {
                    *r = (linear_to_srgb(*r as f32 / 255.) * 255.) as u8;
                    *g = (linear_to_srgb(*g as f32 / 255.) * 255.) as u8;
                    *b = (linear_to_srgb(*b as f32 / 255.) * 255.) as u8;
                }

                log::debug!("gamma fix applied on {key:?}");
            }
            _ => unimplemented!("unsupported chunk migration {migrate_format}"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct TempStore {
        store: ChunkStore,
        path: PathBuf,
    }

    impl TempStore {
        fn new(name: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let mut path = std::env::temp_dir();
            path.push(format!(
                "ln_drawer_chunk_test_{}_{}_{}",
                std::process::id(),
                name,
                nanos
            ));

            let store = ChunkStore::open(path.clone()).unwrap();
            TempStore { store, path }
        }
    }

    impl Drop for TempStore {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn sample(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn round_trip() {
        let temp = TempStore::new("round_trip");
        let key = (-3, 5, 0);
        let data = sample(512 * 512 * 4);

        temp.store.write(0, key, &data).unwrap();
        assert_eq!(temp.store.read(0, key).unwrap().unwrap(), data);

        temp.store.remove(0, key).unwrap();
        assert!(temp.store.read(0, key).unwrap().is_none());
        temp.store.remove(0, key).unwrap();
    }

    #[test]
    fn missing_is_none() {
        let temp = TempStore::new("missing");
        assert!(temp.store.read(7, (0, 0, 0)).unwrap().is_none());
    }

    #[test]
    fn overwrite_is_atomic() {
        let temp = TempStore::new("overwrite");
        let key = (1, 1, 0);

        temp.store.write(0, key, &sample(64)).unwrap();
        temp.store.write(0, key, &sample(128)).unwrap();
        assert_eq!(temp.store.read(0, key).unwrap().unwrap(), sample(128));

        assert!(!temp.store.path(0, key).with_extension("tmp").exists());
    }

    fn encode_with(format: u32, header: ChunkHeader, payload: &[u8]) -> Vec<u8> {
        let header = postcard::to_stdvec(&header).unwrap();
        let mut file = CHUNK_HEADER_MAGIC.to_vec();
        file.extend_from_slice(&format.to_le_bytes());
        file.extend_from_slice(&(header.len() as u32).to_le_bytes());
        file.extend_from_slice(&header);
        file.extend_from_slice(payload);
        file
    }

    #[test]
    fn migrates_legacy_format() {
        let temp = TempStore::new("migrates_legacy");
        let key = (0, 0, 0);

        let raw = [255u8, 128, 0, 255];
        let compressed = zstd::encode_all(&raw[..], 0).unwrap();
        let file = ChunkHeader::encode(0, &compressed).unwrap();
        temp.store
            .write_atomic(&temp.store.path(0, key), &file)
            .unwrap();

        let migrated = temp.store.read(0, key).unwrap().unwrap();
        assert_eq!(migrated.len(), raw.len());
        assert_ne!(migrated[1], raw[1]);

        let rewritten = fs::read(temp.store.path(0, key)).unwrap();
        let (format, _, _) = ChunkHeader::decode(&rewritten).unwrap();
        assert_eq!(format, CHUNK_FORMAT_CURRENT);
        assert_eq!(temp.store.read(0, key).unwrap().unwrap(), migrated);
    }

    #[test]
    fn iter_all_round_trip() {
        let temp = TempStore::new("iter_all");
        let expected = [(0u64, (0, 0, 0)), (0, (-4, 2, 1)), (2, (7, -8, 3))];

        for (page, key) in expected {
            let value = (page as u8)
                .wrapping_add(key.0 as u8)
                .wrapping_add(key.1 as u8);
            temp.store.write(page, key, &[value; 8]).unwrap();
        }

        let mut found = temp.store.iter_all().unwrap();
        found.sort();
        let mut expected = expected.to_vec();
        expected.sort();
        assert_eq!(found, expected);

        temp.store.migrate_all().unwrap();
        for (page, key) in expected {
            assert!(temp.store.read(page, key).unwrap().is_some());
        }
    }

    #[test]
    fn ignores_unrelated_files() {
        let temp = TempStore::new("unrelated");
        let path = temp.store.path(0, (0, 0, 0));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path.with_extension("tmp"), b"junk").unwrap();
        fs::create_dir_all(temp.store.root.join("not-a-page")).unwrap();
        assert!(temp.store.iter_all().unwrap().is_empty());
    }

    #[test]
    fn rejects_malformed() {
        let temp = TempStore::new("malformed");
        let path = temp.store.path(0, (0, 0, 0));
        fs::create_dir_all(path.parent().unwrap()).unwrap();

        let payload = zstd::encode_all(&[0u8; 4][..], 0).unwrap();

        fs::write(&path, b"LNCK").unwrap();
        assert!(temp.store.read(0, (0, 0, 0)).is_err());

        let mut file = encode_with(CHUNK_FORMAT_CURRENT, ChunkHeader::new(), &payload);
        file[0] = b'X';
        fs::write(&path, file).unwrap();
        assert!(temp.store.read(0, (0, 0, 0)).is_err());

        let mut header = ChunkHeader::new();
        header.codec = 42;
        let file = encode_with(CHUNK_FORMAT_CURRENT, header, &payload);
        fs::write(&path, file).unwrap();
        assert!(temp.store.read(0, (0, 0, 0)).is_err());

        let file = encode_with(CHUNK_FORMAT_CURRENT + 1, ChunkHeader::new(), &payload);
        fs::write(&path, file).unwrap();
        assert!(temp.store.read(0, (0, 0, 0)).is_err());
    }

    #[test]
    fn chunk_root_is_sibling_of_db() {
        assert_eq!(
            chunk_root(Path::new("/data/world.lndb")),
            PathBuf::from("/data/chunks")
        );
    }
}
