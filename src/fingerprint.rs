//! File fingerprinting for watch mode and the incremental cache.
//!
//! A fingerprint is a fast, order-independent hash over file paths, sizes
//! and modification times — no file contents are read.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Directories never worth watching or hashing.
const SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", ".x-cache"];

fn hash_file(hasher: &mut DefaultHasher, path: &Path) {
    path.hash(hasher);
    if let Ok(meta) = fs::metadata(path) {
        meta.len().hash(hasher);
        if let Ok(mtime) = meta.modified() {
            mtime.hash(hasher);
        }
    }
}

fn scan(dir: &Path, hashes: &mut Vec<u64>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            let name = entry.file_name();
            if SKIP_DIRS.contains(&name.to_string_lossy().as_ref()) {
                continue;
            }
            scan(&path, hashes);
        } else if kind.is_file() {
            let mut hasher = DefaultHasher::new();
            hash_file(&mut hasher, &path);
            hashes.push(hasher.finish());
        }
    }
}

/// Fingerprint a set of input paths (files or directories).
pub fn fingerprint_inputs(inputs: &[PathBuf]) -> u64 {
    let mut hashes = Vec::new();
    for input in inputs {
        if input.is_dir() {
            scan(input, &mut hashes);
        } else if input.is_file() {
            let mut hasher = DefaultHasher::new();
            hash_file(&mut hasher, input);
            hashes.push(hasher.finish());
        }
    }
    hashes.sort_unstable();

    let mut hasher = DefaultHasher::new();
    for hash in hashes {
        hash.hash(&mut hasher);
    }
    hasher.finish()
}

/// Fingerprint a whole directory tree (skipping heavy/irrelevant dirs).
pub fn fingerprint_dir(dir: &Path) -> u64 {
    fingerprint_inputs(&[dir.to_path_buf()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("cargo-x-fp-{}-{label}-{nanos}", std::process::id()))
    }

    #[test]
    fn fingerprint_changes_with_content_and_stays_stable() {
        let dir = temp_dir("stable");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.txt"), "hello").unwrap();

        let first = fingerprint_dir(&dir);
        assert_eq!(first, fingerprint_dir(&dir));

        fs::write(dir.join("b.txt"), "new file").unwrap();
        assert_ne!(first, fingerprint_dir(&dir));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn fingerprint_skips_target_and_node_modules() {
        let dir = temp_dir("skip");
        fs::create_dir_all(dir.join("target")).unwrap();
        fs::create_dir_all(dir.join("node_modules")).unwrap();
        fs::write(dir.join("src.txt"), "x").unwrap();

        let before = fingerprint_dir(&dir);
        fs::write(dir.join("target/build.o"), "junk").unwrap();
        fs::write(dir.join("node_modules/pkg.js"), "junk").unwrap();
        assert_eq!(before, fingerprint_dir(&dir));

        fs::remove_dir_all(dir).unwrap();
    }
}
