use std::fs::{self, File};
use std::io::{self, ErrorKind, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Writes JSON atomically and durably: a crash or power loss never leaves a truncated file.
pub fn save<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    let mut file = File::create(&tmp)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, path)
}

/// `Ok(None)` when the file is missing, or corrupt (kept aside as `.bak`).
/// Other read errors are returned, after retrying transient ones (antivirus, sync clients holding
/// the file), so the caller never mistakes "couldn't read" for "empty" and overwrites real data.
pub fn load<T: DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    let mut attempts = 0;
    let bytes = loop {
        match fs::read(path) {
            Ok(bytes) => break bytes,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(_) if attempts < 5 => {
                attempts += 1;
                thread::sleep(Duration::from_millis(100));
            }
            Err(err) => return Err(err),
        }
    };
    match serde_json::from_slice(&bytes) {
        Ok(value) => Ok(Some(value)),
        Err(_) => {
            fs::rename(path, path.with_extension("bak"))?;
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_sets_aside_corrupt_files() {
        let dir = std::env::temp_dir().join(format!("solace-persist-test-{}", std::process::id()));
        let file = dir.join("nested/session.json");

        assert_eq!(load::<Vec<u32>>(&file).unwrap(), None);
        save(&file, &vec![1u32, 2, 3]).unwrap();
        assert_eq!(load::<Vec<u32>>(&file).unwrap(), Some(vec![1, 2, 3]));

        fs::write(&file, "{ not json").unwrap();
        assert_eq!(load::<Vec<u32>>(&file).unwrap(), None);
        assert!(file.with_extension("bak").exists());
        assert!(!file.exists());

        fs::remove_dir_all(&dir).unwrap();
    }
}
