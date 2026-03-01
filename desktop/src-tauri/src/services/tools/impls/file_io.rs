//! Shared low-level file write helpers for tool implementations.
//!
//! Provides crash-safe atomic replacement semantics (`write temp + fsync + rename`)
//! used by Write/Edit tools.

use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

/// Atomically replace `path` with `bytes`.
///
/// Writes to a temp file in the same directory, fsyncs it, then renames into place.
/// This avoids partially-written destination files on crashes or cancellation.
pub(crate) fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Path has no parent directory: {}", path.display()),
        )
    })?;

    if !parent.exists() {
        fs::create_dir_all(parent)?;
    }

    let file_name = path.file_name().unwrap_or_else(|| OsStr::new("file"));
    let temp_name = format!(
        ".{}.{}.tmp",
        file_name.to_string_lossy(),
        uuid::Uuid::new_v4()
    );
    let temp_path = parent.join(temp_name);

    let existing_permissions = fs::metadata(path).ok().map(|m| m.permissions());

    let result = (|| -> io::Result<()> {
        let mut tmp = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;

        tmp.write_all(bytes)?;
        tmp.sync_all()?;

        if let Some(perms) = existing_permissions {
            fs::set_permissions(&temp_path, perms)?;
        }

        fs::rename(&temp_path, path)?;

        #[cfg(unix)]
        {
            if let Ok(dir_file) = fs::File::open(parent) {
                let _ = dir_file.sync_all();
            }
        }

        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_atomic_write_bytes_replaces_content() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, "old").unwrap();

        atomic_write_bytes(&file, b"new").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "new");
    }
}
