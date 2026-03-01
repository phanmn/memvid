use std::convert::TryFrom;
use std::env;
use std::fmt;
use std::io::ErrorKind;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use blake3::Hasher;
use fs_err::{self as fs, File, OpenOptions};
use same_file::Handle;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::error::{LockOwnerHint, Result};

const HEADER_SAMPLE_BYTES: usize = 4 * 1024;
const REGISTRY_SUBDIR: &str = "locks";
const ROOT_DIR: &str = ".memvid";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileId {
    raw: String,
}

impl FileId {
    fn new(raw: String) -> Self {
        Self { raw }
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }
}

impl fmt::Display for FileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockRecord {
    pub pid: u32,
    pub cmd: String,
    pub started_at: String,
    pub file_path: String,
    pub file_id: String,
    pub heartbeat_ms: u64,
    pub last_heartbeat: String,
}

impl LockRecord {
    pub fn new(file_id: &FileId, file_path: &Path, cmd: String, heartbeat_ms: u64) -> Result<Self> {
        let started_at = current_timestamp()?;
        Ok(Self {
            pid: std::process::id(),
            cmd,
            started_at: started_at.clone(),
            file_path: file_path.display().to_string(),
            file_id: file_id.as_str().to_string(),
            heartbeat_ms,
            last_heartbeat: started_at,
        })
    }

    #[allow(dead_code)]
    pub fn touch(&mut self) -> Result<()> {
        self.last_heartbeat = current_timestamp()?;
        Ok(())
    }

    pub fn to_owner_hint(&self) -> LockOwnerHint {
        LockOwnerHint {
            pid: Some(self.pid),
            cmd: Some(self.cmd.clone()),
            started_at: Some(self.started_at.clone()),
            file_path: Some(PathBuf::from(&self.file_path)),
            file_id: Some(self.file_id.clone()),
            last_heartbeat: Some(self.last_heartbeat.clone()),
            heartbeat_ms: Some(self.heartbeat_ms),
        }
    }
}

fn current_timestamp() -> Result<String> {
    let now = OffsetDateTime::now_utc();
    now.format(&Rfc3339)
        .map_err(io::Error::other)
        .map_err(Into::into)
}

pub fn compute_file_id(path: &Path) -> Result<FileId> {
    let handle = Handle::from_path(path)?;
    let mut file = File::open(path)?;
    let mut sample = [0u8; HEADER_SAMPLE_BYTES];
    let read = file.read(&mut sample)?;
    let mut hasher = Hasher::new();
    hasher.update(&sample[..read]);

    #[cfg(unix)]
    let prefix = format!(
        "unix-{dev:016x}-{ino:016x}",
        dev = handle.dev(),
        ino = handle.ino()
    );

    #[cfg(windows)]
    let prefix = {
        // Use stable APIs only: canonicalized path + metadata for deterministic ID.
        // The unstable volume_serial_number/file_index_high/file_index_low APIs
        // require nightly (windows_by_handle feature), so we use a hash-based fallback.
        use std::os::windows::fs::MetadataExt;

        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let metadata = handle.as_file().metadata()?;

        // Build a deterministic identifier from stable metadata:
        // - Canonicalized path (primary identifier)
        // - File size (file_size() is stable on Windows)
        // - Creation time (creation_time() is stable on Windows)
        // - Last write time (last_write_time() is stable on Windows)
        let mut path_hasher = Hasher::new();
        path_hasher.update(canonical_path.to_string_lossy().as_bytes());
        path_hasher.update(&metadata.file_size().to_le_bytes());
        path_hasher.update(&metadata.creation_time().to_le_bytes());
        path_hasher.update(&metadata.last_write_time().to_le_bytes());

        let path_hash = path_hasher.finalize();
        format!("win-{}", &path_hash.to_hex()[..32])
    };

    #[cfg(not(any(unix, windows)))]
    let prefix = "other".to_string();

    let identifier = format!("{}-{}", prefix, hasher.finalize().to_hex());
    Ok(FileId::new(identifier))
}

fn registry_root() -> Result<PathBuf> {
    let mut last_err: Option<io::Error> = None;

    for candidate in registry_candidates() {
        match ensure_directory(candidate) {
            Ok(path) => return Ok(path),
            Err(err) if recoverable_dir_error(&err) => {
                last_err = Some(err);
                continue;
            }
            Err(err) => return Err(err.into()),
        }
    }

    Err(last_err
        .unwrap_or_else(|| io::Error::other("failed to establish memvid lock registry directory"))
        .into())
}

fn registry_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(override_root) = env::var_os("MEMVID_LOCK_REGISTRY_DIR") {
        candidates.push(PathBuf::from(override_root));
    }

    candidates.push(env::temp_dir().join(ROOT_DIR).join(REGISTRY_SUBDIR));

    if let Some(home) = dirs_next::home_dir() {
        candidates.push(home.join(ROOT_DIR).join(REGISTRY_SUBDIR));
    }

    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join(ROOT_DIR).join(REGISTRY_SUBDIR));
    }

    candidates
}

fn ensure_directory(path: PathBuf) -> io::Result<PathBuf> {
    fs::create_dir_all(&path)?;
    let sentinel = path.join(".write_test");
    match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&sentinel)
    {
        Ok(_) => {
            let _ = fs::remove_file(sentinel);
            Ok(path)
        }
        Err(err) => Err(err),
    }
}

fn recoverable_dir_error(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::PermissionDenied | ErrorKind::NotFound | ErrorKind::ReadOnlyFilesystem
    )
}

fn record_path(file_id: &FileId) -> Result<PathBuf> {
    Ok(registry_root()?.join(format!("{}.json", file_id.as_str())))
}

pub fn write_record(record: &LockRecord) -> Result<()> {
    let path = record_path(&FileId::new(record.file_id.clone()))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    serde_json::to_writer(&mut file, record).map_err(io::Error::other)?;
    file.flush()?;
    file.sync_all()?;
    Ok(())
}

#[allow(dead_code)]
pub fn heartbeat(file_id: &FileId) -> Result<Option<LockRecord>> {
    let Some(mut record) = read_record(file_id)? else {
        return Ok(None);
    };
    record.touch()?;
    write_record(&record)?;
    Ok(Some(record))
}

pub fn read_record(file_id: &FileId) -> Result<Option<LockRecord>> {
    let path = record_path(file_id)?;
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let record: LockRecord = serde_json::from_reader(file).map_err(io::Error::other)?;
    Ok(Some(record))
}

pub fn remove_record(file_id: &FileId) -> Result<()> {
    let path = record_path(file_id)?;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

pub fn is_stale(record: &LockRecord, grace: Duration) -> bool {
    match OffsetDateTime::parse(&record.last_heartbeat, &Rfc3339) {
        Ok(last) => match Duration::try_from(OffsetDateTime::now_utc() - last) {
            Ok(elapsed) => elapsed > grace,
            Err(_) => false,
        },
        Err(_) => true,
    }
}

pub fn to_owner_hint(record: Option<LockRecord>) -> Option<LockOwnerHint> {
    record.map(|r| r.to_owner_hint())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper: create a real file inside a temp dir and return its path.
    fn create_temp_file(dir: &TempDir, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        let mut f = File::create(&path).unwrap();
        f.write_all(content).unwrap();
        f.flush().unwrap();
        path
    }

    #[test]
    fn compute_file_id_is_deterministic() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.mv2", b"hello world data");
        let id1 = compute_file_id(&path).unwrap();
        let id2 = compute_file_id(&path).unwrap();
        assert_eq!(id1, id2);
    }

    #[test]
    fn file_id_display_is_non_empty() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "display.mv2", b"some content");
        let id = compute_file_id(&path).unwrap();
        let display = format!("{id}");
        assert!(!display.is_empty());
    }

    #[test]
    fn lock_record_new_creates_valid_record() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "record.mv2", b"data");
        let file_id = compute_file_id(&path).unwrap();
        let record = LockRecord::new(&file_id, &path, "test-cmd".to_string(), 2000).unwrap();
        assert_eq!(record.pid, std::process::id());
        assert_eq!(record.cmd, "test-cmd");
        assert_eq!(record.heartbeat_ms, 2000);
        assert!(!record.started_at.is_empty());
        assert!(!record.last_heartbeat.is_empty());
    }

    #[test]
    fn is_stale_returns_false_for_fresh_record() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "fresh.mv2", b"data");
        let file_id = compute_file_id(&path).unwrap();
        let record = LockRecord::new(&file_id, &path, "test".to_string(), 2000).unwrap();
        // A just-created record should not be stale with a 60s grace period
        assert!(!is_stale(&record, Duration::from_secs(60)));
    }

    #[test]
    fn write_read_remove_record_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "roundtrip.mv2", b"data");
        let file_id = compute_file_id(&path).unwrap();
        let record = LockRecord::new(&file_id, &path, "roundtrip-cmd".to_string(), 1000).unwrap();

        // Write
        write_record(&record).unwrap();

        // Read back
        let read_back = read_record(&file_id).unwrap();
        assert!(read_back.is_some());
        let read_back = read_back.unwrap();
        assert_eq!(read_back.cmd, "roundtrip-cmd");
        assert_eq!(read_back.pid, record.pid);
        assert_eq!(read_back.file_id, record.file_id);

        // Remove
        remove_record(&file_id).unwrap();
        let after_remove = read_record(&file_id).unwrap();
        assert!(after_remove.is_none());
    }

    #[test]
    fn file_id_as_str_matches_display() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "str_test.mv2", b"content");
        let id = compute_file_id(&path).unwrap();
        assert_eq!(id.as_str(), &format!("{id}"));
    }
}
