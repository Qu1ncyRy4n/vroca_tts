//! `prefs.json` loading and migration (DEC-17, DEC-7, review bug 12).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{json, Value};
use vroca_core::prefs::{load, to_json, Loaded, MigrationContext, PrefsError, PREFS_SCHEMA};
use vroca_core::schema::{
    Aligner, EngineId, OverlayMode, Position, Preferences, RuntimeSnapshot, VoiceId,
};

fn load_value(value: Value, ctx: &MigrationContext) -> Loaded {
    load(&serde_json::to_vec(&value).unwrap(), ctx).unwrap()
}

/// The live file recorded in rust-spec.md §4.1, plus the fields daemon.py
/// added since (volume, trims).
fn live_python_file() -> Value {
    json!({"engine": "libritts", "voice": 9, "speed": 1.0, "aligner": "asr",
           "font_size": 25, "words_visible": 2, "position": "center",
           "volume": 80, "trims": {"libritts:9": -15}})
}

#[test]
fn bug12_live_python_file_loads_and_migrates_to_v1() {
    let ctx = MigrationContext {
        legacy_mode: Some("scroll_rsvp\n".into()),
    };
    let loaded = load_value(live_python_file(), &ctx);
    assert_eq!(loaded.from_schema, 0);
    assert!(loaded.writable);
    assert_eq!(loaded.corrections, Vec::<String>::new());
    let p = &loaded.prefs;
    assert_eq!(p.schema, PREFS_SCHEMA);
    assert_eq!(p.engine, EngineId::new("libritts").unwrap());
    assert_eq!(p.voice, 9);
    assert_eq!(p.voice_id, None);
    assert_eq!(p.font_size, 25);
    assert_eq!(p.words_visible, 2);
    assert_eq!(p.position, Position::Center);
    assert_eq!(p.aligner, Aligner::Asr);
    assert_eq!(p.volume, 80);
    assert_eq!(p.trims.get("libritts:9"), Some(&-15));
    // DEC-7: mode moves from the vroca-mode file into prefs.
    assert_eq!(p.overlay_mode, OverlayMode::ScrollRsvp);
}

#[test]
fn dec17_rollback_keeps_python_readable_fields() {
    // Python reads `voice` only if it is an integer; Rust must keep writing
    // the integer so switching back keeps the selected voice.
    let loaded = load_value(live_python_file(), &MigrationContext::default());
    let written: Value = serde_json::from_slice(&to_json(&loaded.prefs).unwrap()).unwrap();
    assert_eq!(written.get("voice"), Some(&json!(9)));
    assert_eq!(written.get("engine"), Some(&json!("libritts")));
    assert_eq!(written.get("schema"), Some(&json!(1)));
    assert_eq!(written.get("overlay_mode"), Some(&json!("subtitle")));
    assert!(written.get("voice_id").is_none());
}

#[test]
fn dec17_missing_file_fields_take_defaults_silently() {
    let loaded = load_value(json!({}), &MigrationContext::default());
    assert_eq!(loaded.corrections, Vec::<String>::new());
    assert_eq!(loaded.prefs, Preferences::default());
}

#[test]
fn dec17_unknown_fields_survive_a_round_trip() {
    let mut file = live_python_file();
    file.as_object_mut()
        .unwrap()
        .insert("future_setting".into(), json!({"nested": [1, 2]}));
    let loaded = load_value(file, &MigrationContext::default());
    let written: Value = serde_json::from_slice(&to_json(&loaded.prefs).unwrap()).unwrap();
    assert_eq!(
        written.get("future_setting"),
        Some(&json!({"nested": [1, 2]}))
    );
    let again = load(
        &to_json(&loaded.prefs).unwrap(),
        &MigrationContext::default(),
    )
    .unwrap();
    assert_eq!(again.prefs, loaded.prefs);
    assert_eq!(again.from_schema, 1);
}

#[test]
fn dec17_malformed_values_are_corrected_and_reported() {
    // A string voice is an identity (see v0_string_voice_becomes_voice_id);
    // a negative index is the malformed case.
    let file = json!({"engine": "espeak", "voice": -3, "speed": 9.5,
                      "aligner": "dtw", "font_size": 200, "words_visible": "lots",
                      "position": "left", "overlay_mode": 3, "volume": -5,
                      "trims": {"kokoro:1": 500, "kokoro:2": "x"}});
    let loaded = load_value(file, &MigrationContext::default());
    let p = &loaded.prefs;
    let d = Preferences::default();
    assert_eq!(p.engine, d.engine);
    assert_eq!(p.voice, d.voice);
    assert_eq!(p.speed, 3.0);
    assert_eq!(p.aligner, d.aligner);
    assert_eq!(p.font_size, 72);
    assert_eq!(p.words_visible, d.words_visible);
    assert_eq!(p.position, d.position);
    assert_eq!(p.overlay_mode, d.overlay_mode);
    assert_eq!(p.volume, 0);
    assert_eq!(p.trims.get("kokoro:1"), Some(&100));
    assert_eq!(p.trims.get("kokoro:2"), None);
    // One report per bad value; nothing corrected silently.
    assert_eq!(loaded.corrections.len(), 11, "{:#?}", loaded.corrections);
    assert!(loaded
        .corrections
        .iter()
        .any(|c| c == "font_size: 200 is outside 12..=72, clamped to 72"));
}

#[test]
fn dec17_corrupt_file_is_an_error_for_the_caller_to_quarantine() {
    for bytes in [&b"{not json"[..], b"", b"[1, 2]", b"\"prefs\""] {
        assert!(matches!(
            load(bytes, &MigrationContext::default()),
            Err(PrefsError::Corrupt(_))
        ));
    }
}

#[test]
fn dec17_newer_schema_loads_read_only() {
    let mut file = live_python_file();
    file.as_object_mut()
        .unwrap()
        .insert("schema".into(), json!(7));
    let loaded = load_value(file, &MigrationContext::default());
    assert!(!loaded.writable);
    assert_eq!(loaded.prefs.schema, 7);
    assert_eq!(loaded.prefs.voice, 9);
    assert_eq!(loaded.corrections.len(), 1, "{:?}", loaded.corrections);
}

#[test]
fn v0_string_voice_becomes_voice_id() {
    let loaded = load_value(
        json!({"engine": "libritts", "voice": "libritts:qorto"}),
        &MigrationContext::default(),
    );
    assert_eq!(loaded.prefs.voice, 0);
    assert_eq!(
        loaded.prefs.voice_id,
        Some(VoiceId("libritts:qorto".into()))
    );
}

#[test]
fn dec7_unknown_legacy_mode_falls_back_to_subtitle() {
    let ctx = MigrationContext {
        legacy_mode: Some("sideways".into()),
    };
    assert_eq!(
        load_value(json!({}), &ctx).prefs.overlay_mode,
        OverlayMode::Subtitle
    );
}

#[test]
fn dec17_snapshot_unknown_fields_round_trip() {
    let mut snapshot = json!({
        "schema": 1, "daemon_id": "1:2", "sentence": "", "index": 0, "total": 0,
        "paused": false, "speed": 1.0, "rendered": 0, "rendering": null,
        "loaded": false, "voice": null, "word": 0, "engine": "kokoro",
        "engines": [], "aligner": "asr", "queue_len": 0, "font_size": 24,
        "words_visible": 3, "position": "bottom", "voices": null,
        "last_render_ms": null, "avg_render_ms": null, "health": {"state": "ok"}
    });
    snapshot
        .as_object_mut()
        .unwrap()
        .insert("player_alive".into(), json!(true));
    let parsed: RuntimeSnapshot = serde_json::from_value(snapshot.clone()).unwrap();
    assert_eq!(
        parsed.unknown_fields.get("player_alive"),
        Some(&json!(true))
    );
    assert_eq!(serde_json::to_value(&parsed).unwrap(), snapshot);
}
