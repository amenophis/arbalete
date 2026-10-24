//! Turns other browsers' data into Arbalette's.

pub mod arc;
pub mod bookmarks;
pub mod browsers;
pub mod chromium;
pub mod firefox;
pub mod safari;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::{env, fs, io, process};

/// A copy of a SQLite database, with its write-ahead log, in a temporary directory removed when
/// dropped. The browser keeps its database locked while it runs, and the log holds its latest
/// changes.
pub(crate) struct Copy(PathBuf);

impl Copy {
    /// `None` when there is no database to copy.
    pub fn of(database: &Path) -> io::Result<Option<Self>> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = env::temp_dir().join(format!("arbalette-import-{}-{n}", process::id()));
        fs::create_dir_all(&dir)?;
        let copy = Self(dir);
        match fs::copy(database, copy.file()) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            result => result?,
        };
        let mut wal = database.as_os_str().to_owned();
        wal.push("-wal");
        match fs::copy(wal, copy.0.join("database-wal")) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            result => _ = result?,
        }
        Ok(Some(copy))
    }

    pub fn file(&self) -> PathBuf {
        self.0.join("database")
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A directory under the system's temporary one for a test's files, removed when dropped.
#[cfg(test)]
pub(crate) struct TestDir(pub std::path::PathBuf);

#[cfg(test)]
impl TestDir {
    /// `name` keeps tests running in parallel apart.
    pub fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("arbalette-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub fn write(&self, path: &str, content: impl AsRef<[u8]>) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    pub fn dir(&self, path: &str) {
        std::fs::create_dir_all(self.0.join(path)).unwrap();
    }
}

#[cfg(test)]
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
