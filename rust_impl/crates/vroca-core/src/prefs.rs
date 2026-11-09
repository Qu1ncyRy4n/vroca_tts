//! Loading `prefs.json` (DEC-17): schema versioning, ordered migrations, and
//! per-field correction. Pure functions over bytes and JSON values; the
//! daemon owns the file I/O, the pre-migration backup, and the `.bad`
//! quarantine.
//!
//! - Bytes that are not a JSON object are `Corrupt`: the caller quarantines
//!   the file and uses defaults.
//! - An older file runs each migration step in order up to `PREFS_SCHEMA`.
//! - A malformed value in an otherwise valid file is clamped or replaced by
//!   its default, and each correction is reported, never applied silently.
//! - A file from a newer schema loads best-effort but is marked read-only,
//!   so this daemon never rewrites data it does not fully understand.
//! - Unknown fields are kept in `unknown_fields` and written back.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::parser::{
    FONT_SIZE_MAX, FONT_SIZE_MIN, SPEED_MAX, SPEED_MIN, TRIM_MAX, TRIM_MIN, VOLUME_MAX, VOLUME_MIN,
    WORDS_VISIBLE_MAX, WORDS_VISIBLE_MIN,
};
use crate::schema::{Aligner, EngineId, OverlayMode, Position, Preferences, VoiceId};

pub const PREFS_SCHEMA: u32 = 1;

/// Inputs a migration may need from outside the file. Still pure: the daemon
/// reads them and passes them in.
#[derive(Debug, Clone, Default)]
pub struct MigrationContext {
    /// Contents of the legacy `vroca-mode` file, if any (DEC-7).
    pub legacy_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub prefs: Preferences,
    /// The schema the file had before migration.
    pub from_schema: u32,
    /// One line per value that was clamped, replaced, or dropped.
    pub corrections: Vec<String>,
    /// False for a newer-schema file: use it, but do not write it back.
    pub writable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrefsError {
    Corrupt(String),
}

type Step = fn(Map<String, Value>, &MigrationContext) -> Map<String, Value>;

/// `(from_schema, step)`, in order. Each step lifts a file by one version and
/// has its own test (DEC-17 evolution rule).
const MIGRATIONS: &[(u32, Step)] = &[(0, v0_to_v1)];

/// Schema 0 is daemon.py's unversioned file: `"voice"` is an integer and
/// overlay mode lives in the separate `vroca-mode` file.
fn v0_to_v1(mut file: Map<String, Value>, ctx: &MigrationContext) -> Map<String, Value> {
    if !file.contains_key("overlay_mode") {
        let mode = ctx
            .legacy_mode
            .as_deref()
            .map(str::trim)
            .and_then(OverlayMode::from_name)
            .unwrap_or(OverlayMode::Subtitle);
        file.insert("overlay_mode".into(), Value::from(mode.as_str()));
    }
    // A string voice could only come from an early Rust build; it is an
    // identity, not an index.
    if let Some(Value::String(id)) = file.get("voice").cloned() {
        file.remove("voice");
        file.entry("voice_id").or_insert(Value::String(id));
    }
    file.insert("schema".into(), Value::from(1));
    file
}

pub fn load(bytes: &[u8], ctx: &MigrationContext) -> Result<Loaded, PrefsError> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|e| PrefsError::Corrupt(e.to_string()))?;
    let Value::Object(mut file) = value else {
        return Err(PrefsError::Corrupt(
            "prefs.json is not a JSON object".into(),
        ));
    };
    let mut corrections = Vec::new();

    let from_schema = match file.get("schema") {
        None => 0,
        Some(v) => match v.as_u64().and_then(|n| u32::try_from(n).ok()) {
            Some(n) => n,
            None => {
                corrections.push(format!("schema: invalid value {v}, treated as 0"));
                0
            }
        },
    };
    let writable = from_schema <= PREFS_SCHEMA;
    if !writable {
        corrections.push(format!(
            "schema: {from_schema} is newer than this daemon ({PREFS_SCHEMA}); loaded read-only"
        ));
    }
    for (from, step) in MIGRATIONS {
        if *from >= from_schema && *from < PREFS_SCHEMA {
            file = step(file, ctx);
        }
    }

    let mut prefs = sanitize(file, &mut corrections);
    prefs.schema = from_schema.max(PREFS_SCHEMA);
    Ok(Loaded {
        prefs,
        from_schema,
        corrections,
        writable,
    })
}

pub fn to_json(prefs: &Preferences) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec_pretty(prefs)
}

/// Build `Preferences` field by field, so one bad value costs that value,
/// not the whole file. A missing field takes its default silently; a present
/// but bad one is corrected and reported.
fn sanitize(mut file: Map<String, Value>, corrections: &mut Vec<String>) -> Preferences {
    let defaults = Preferences::default();
    let mut out = Fields {
        file: &mut file,
        corrections,
    };
    let engine = out
        .take("engine", |v| v.as_str().and_then(|s| EngineId::new(s).ok()))
        .unwrap_or(defaults.engine);
    let voice = out
        .take("voice", |v| {
            v.as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .filter(|n| *n >= 0)
        })
        .unwrap_or(defaults.voice);
    let voice_id = out.take("voice_id", |v| v.as_str().map(|s| VoiceId(s.to_string())));
    let speed = out
        .number("speed", SPEED_MIN, SPEED_MAX)
        .unwrap_or(defaults.speed);
    let aligner = out
        .take("aligner", |v| v.as_str().and_then(Aligner::from_name))
        .unwrap_or(defaults.aligner);
    let font_size = out
        .integer("font_size", FONT_SIZE_MIN, FONT_SIZE_MAX)
        .unwrap_or(defaults.font_size);
    let words_visible = out
        .integer("words_visible", WORDS_VISIBLE_MIN, WORDS_VISIBLE_MAX)
        .unwrap_or(defaults.words_visible);
    let position = out
        .take("position", |v| v.as_str().and_then(Position::from_name))
        .unwrap_or(defaults.position);
    let overlay_mode = out
        .take("overlay_mode", |v| {
            v.as_str().and_then(OverlayMode::from_name)
        })
        .unwrap_or(defaults.overlay_mode);
    let volume = out
        .integer("volume", VOLUME_MIN, VOLUME_MAX)
        .unwrap_or(defaults.volume);
    let trims = out.trims();
    out.file.remove("schema");

    Preferences {
        schema: PREFS_SCHEMA,
        engine,
        voice,
        voice_id,
        speed,
        aligner,
        font_size,
        words_visible,
        position,
        overlay_mode,
        volume,
        trims,
        unknown_fields: file.into_iter().collect(),
    }
}

struct Fields<'a> {
    file: &'a mut Map<String, Value>,
    corrections: &'a mut Vec<String>,
}

impl Fields<'_> {
    /// Remove `key`; if present but rejected by `parse`, report it.
    fn take<T>(&mut self, key: &str, parse: impl Fn(&Value) -> Option<T>) -> Option<T> {
        let value = self.file.remove(key)?;
        let parsed = parse(&value);
        if parsed.is_none() {
            self.corrections
                .push(format!("{key}: invalid value {value}, using the default"));
        }
        parsed
    }

    fn integer(&mut self, key: &str, min: i32, max: i32) -> Option<i32> {
        let n = self.take(key, |v| v.as_i64())?;
        let clamped = n.clamp(i64::from(min), i64::from(max));
        if clamped != n {
            self.corrections.push(format!(
                "{key}: {n} is outside {min}..={max}, clamped to {clamped}"
            ));
        }
        i32::try_from(clamped).ok()
    }

    fn number(&mut self, key: &str, min: f32, max: f32) -> Option<f32> {
        let n = self.take(key, |v| f32::deserialize(v).ok().filter(|n| n.is_finite()))?;
        let clamped = n.clamp(min, max);
        if clamped != n {
            self.corrections.push(format!(
                "{key}: {n} is outside {min}..={max}, clamped to {clamped}"
            ));
        }
        Some(clamped)
    }

    fn trims(&mut self) -> BTreeMap<String, i32> {
        let mut trims = BTreeMap::new();
        let Some(value) = self.file.remove("trims") else {
            return trims;
        };
        let Value::Object(entries) = value else {
            self.corrections
                .push(format!("trims: invalid value {value}, using none"));
            return trims;
        };
        for (key, amount) in entries {
            match amount.as_i64() {
                Some(n) => {
                    let clamped = n.clamp(i64::from(TRIM_MIN), i64::from(TRIM_MAX));
                    if clamped != n {
                        self.corrections.push(format!(
                            "trims.{key}: {n} is outside {TRIM_MIN}..={TRIM_MAX}, clamped to {clamped}"
                        ));
                    }
                    if let Ok(n) = i32::try_from(clamped) {
                        if n != 0 {
                            trims.insert(key, n);
                        }
                    }
                }
                None => self
                    .corrections
                    .push(format!("trims.{key}: invalid value {amount}, dropped")),
            }
        }
        trims
    }
}
