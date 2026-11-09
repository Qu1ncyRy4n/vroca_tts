//! `prefs.json` on disk (DEC-17): the I/O around the pure `prefs::load`.
//!
//! - A missing file means defaults, silently.
//! - A corrupt file is renamed to `prefs.json.bad` (rename keeps its
//!   permissions), defaults are used, and the problem is reported.
//! - A file that needed migration or correction is first copied to
//!   `prefs.json.bak-<schema>-<unix-ms>`, then rewritten. If the backup
//!   fails, nothing is written: the original is the only copy of the
//!   evidence (omnichan `omni-store` pattern).
//! - A file from a newer schema, or one that could not be read, is never
//!   written back.
//! - Writes are atomic (temp file + rename) and keep the existing mode.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use vroca_core::prefs::{load, to_json, MigrationContext, PrefsError, PREFS_SCHEMA};
use vroca_core::schema::Preferences;

pub const PREFS_FILE: &str = "prefs.json";

#[derive(Debug)]
pub struct PrefsStore {
    path: PathBuf,
    writable: bool,
}

#[derive(Debug)]
pub struct Opened {
    pub store: PrefsStore,
    pub prefs: Preferences,
    /// Health problems to publish, one line each.
    pub problems: Vec<String>,
}

impl PrefsStore {
    /// Open `<prefs_dir>/prefs.json`. `legacy_mode_file` is the old
    /// `vroca-mode` file, read once to migrate overlay mode (DEC-7).
    pub fn open(prefs_dir: &Path, legacy_mode_file: Option<&Path>) -> Opened {
        let path = prefs_dir.join(PREFS_FILE);
        let mut problems = Vec::new();
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return Opened {
                    store: Self {
                        path,
                        writable: true,
                    },
                    prefs: Preferences::default(),
                    problems,
                };
            }
            Err(e) => {
                problems.push(format!(
                    "prefs: cannot read {}: {e}; using defaults and not saving",
                    path.display()
                ));
                return Opened {
                    store: Self {
                        path,
                        writable: false,
                    },
                    prefs: Preferences::default(),
                    problems,
                };
            }
        };
        let ctx = MigrationContext {
            legacy_mode: legacy_mode_file.and_then(|p| fs::read_to_string(p).ok()),
        };
        let mut store = Self {
            path,
            writable: true,
        };
        match load(&bytes, &ctx) {
            Err(PrefsError::Corrupt(reason)) => {
                let bad = store.sibling(".bad");
                match fs::rename(&store.path, &bad) {
                    Ok(()) => problems.push(format!(
                        "prefs: {} was corrupt ({reason}); moved to {}, using defaults",
                        store.path.display(),
                        bad.display()
                    )),
                    Err(e) => {
                        store.writable = false;
                        problems.push(format!(
                            "prefs: {} is corrupt ({reason}) and could not be moved aside \
                             ({e}); using defaults and not saving",
                            store.path.display()
                        ));
                    }
                }
                Opened {
                    store,
                    prefs: Preferences::default(),
                    problems,
                }
            }
            Ok(loaded) => {
                store.writable = loaded.writable;
                problems.extend(loaded.corrections.iter().map(|c| format!("prefs: {c}")));
                let rewrite = loaded.from_schema < PREFS_SCHEMA || !loaded.corrections.is_empty();
                if store.writable && rewrite {
                    let backup =
                        store.sibling(&format!(".bak-{}-{}", loaded.from_schema, unix_ms()));
                    match fs::copy(&store.path, &backup) {
                        Ok(_) => {
                            if let Err(e) = store.save(&loaded.prefs) {
                                problems.push(format!("prefs: {e}"));
                            }
                        }
                        Err(e) => {
                            store.writable = false;
                            problems.push(format!(
                                "prefs: could not back up {} before rewriting ({e}); not saving",
                                store.path.display()
                            ));
                        }
                    }
                }
                Opened {
                    store,
                    prefs: loaded.prefs,
                    problems,
                }
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn writable(&self) -> bool {
        self.writable
    }

    /// Atomically replace the file. A read-only store saves nothing.
    pub fn save(&self, prefs: &Preferences) -> Result<(), String> {
        if !self.writable {
            return Ok(());
        }
        let bytes = to_json(prefs).map_err(|e| e.to_string())?;
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        }
        let tmp = self.sibling(&format!(".tmp-{}", std::process::id()));
        fs::write(&tmp, bytes).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        if let Ok(meta) = fs::metadata(&self.path) {
            // DEC-17: settings keep the permissions they had.
            let _ = fs::set_permissions(&tmp, meta.permissions());
        }
        fs::rename(&tmp, &self.path).map_err(|e| format!("replace {}: {e}", self.path.display()))
    }

    fn sibling(&self, suffix: &str) -> PathBuf {
        let mut name = self.path.as_os_str().to_owned();
        name.push(suffix);
        PathBuf::from(name)
    }
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn written(dir: &Path) -> serde_json::Value {
        serde_json::from_slice(&fs::read(dir.join(PREFS_FILE)).unwrap()).unwrap()
    }

    #[test]
    fn missing_file_is_defaults_and_nothing_written() {
        let dir = tempfile::tempdir().unwrap();
        let opened = PrefsStore::open(dir.path(), None);
        assert_eq!(opened.prefs, Preferences::default());
        assert!(opened.problems.is_empty());
        assert!(names(dir.path()).is_empty());
        opened.store.save(&opened.prefs).unwrap();
        assert_eq!(names(dir.path()), [PREFS_FILE]);
    }

    #[test]
    fn dec17_python_file_is_backed_up_then_migrated() {
        let dir = tempfile::tempdir().unwrap();
        let original = br#"{"engine": "libritts", "voice": 9, "speed": 1.0}"#;
        fs::write(dir.path().join(PREFS_FILE), original).unwrap();
        let mode = dir.path().join("vroca-mode");
        fs::write(&mode, "scroll_rsvp").unwrap();

        let opened = PrefsStore::open(dir.path(), Some(&mode));
        assert!(opened.problems.is_empty(), "{:?}", opened.problems);
        assert_eq!(opened.prefs.voice, 9);

        let files = names(dir.path());
        let backup = files
            .iter()
            .find(|n| n.starts_with("prefs.json.bak-0-"))
            .expect("pre-migration backup");
        assert_eq!(fs::read(dir.path().join(backup)).unwrap(), original);
        let now = written(dir.path());
        assert_eq!(now.get("schema"), Some(&serde_json::json!(1)));
        assert_eq!(now.get("voice"), Some(&serde_json::json!(9)));
        assert_eq!(
            now.get("overlay_mode"),
            Some(&serde_json::json!("scroll_rsvp"))
        );
    }

    #[test]
    fn dec17_current_file_is_not_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = to_json(&Preferences::default()).unwrap();
        fs::write(dir.path().join(PREFS_FILE), &bytes).unwrap();
        let opened = PrefsStore::open(dir.path(), None);
        assert!(opened.problems.is_empty());
        assert_eq!(names(dir.path()), [PREFS_FILE]);
    }

    #[test]
    fn dec17_corrupt_file_is_quarantined_with_its_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(PREFS_FILE);
        fs::write(&path, b"{ not json").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

        let opened = PrefsStore::open(dir.path(), None);
        assert_eq!(opened.prefs, Preferences::default());
        assert_eq!(opened.problems.len(), 1);
        assert!(
            opened.problems[0].contains("was corrupt"),
            "{:?}",
            opened.problems
        );
        let bad = dir.path().join("prefs.json.bad");
        assert_eq!(fs::read(&bad).unwrap(), b"{ not json");
        assert_eq!(
            fs::metadata(&bad).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(!path.exists());
        assert!(opened.store.writable());
    }

    #[test]
    fn dec17_corrected_values_are_reported_backed_up_and_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = br#"{"schema": 1, "font_size": 500}"#;
        fs::write(dir.path().join(PREFS_FILE), bytes).unwrap();
        let opened = PrefsStore::open(dir.path(), None);
        assert_eq!(opened.prefs.font_size, 72);
        assert_eq!(
            opened.problems,
            ["prefs: font_size: 500 is outside 12..=72, clamped to 72"]
        );
        assert!(names(dir.path())
            .iter()
            .any(|n| n.starts_with("prefs.json.bak-1-")));
        assert_eq!(
            written(dir.path()).get("font_size"),
            Some(&serde_json::json!(72))
        );
    }

    #[test]
    fn dec17_newer_schema_is_never_written() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = br#"{"schema": 9, "voice": 3, "shiny": true}"#;
        fs::write(dir.path().join(PREFS_FILE), bytes).unwrap();
        let opened = PrefsStore::open(dir.path(), None);
        assert!(!opened.store.writable());
        assert_eq!(opened.prefs.voice, 3);
        opened.store.save(&Preferences::default()).unwrap();
        assert_eq!(fs::read(dir.path().join(PREFS_FILE)).unwrap(), bytes);
        assert_eq!(names(dir.path()), [PREFS_FILE]);
    }

    #[test]
    fn dec17_save_keeps_the_existing_mode() {
        let dir = tempfile::tempdir().unwrap();
        let opened = PrefsStore::open(dir.path(), None);
        opened.store.save(&opened.prefs).unwrap();
        let path = dir.path().join(PREFS_FILE);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        opened.store.save(&opened.prefs).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
