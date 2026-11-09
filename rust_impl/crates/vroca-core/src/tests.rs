//! State-machine tests: each legacy verb's state change (fixture code S in
//! legacy-compatibility.md) and reply text, cited to its spec identifier.
//! Parser fixtures live in `tests/legacy_fixtures.rs`, codec tests in
//! `tests/codec.rs`.

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod cases {
    use crate::catalogue::{Catalogue, CatalogueEntry};
    use crate::operations::{ErrorCode, Operation};
    use crate::parser::parse_legacy;
    use crate::schema::{
        Aligner, EngineId, OverlayMode, Position, Preferences, SpeakerIndex, VoiceId,
    };
    use crate::state::{Reply, State};

    fn state() -> State {
        State::new(Preferences::default())
    }

    /// Apply a legacy command and return its reply text.
    fn run(state: &mut State, command: &str) -> String {
        match state.apply(parse_legacy(command).unwrap()) {
            Ok(Reply::Text(text)) => text,
            other => panic!("{command}: {other:?}"),
        }
    }

    fn fail(state: &mut State, command: &str) -> (ErrorCode, String) {
        let e = state.apply(parse_legacy(command).unwrap()).unwrap_err();
        (e.code, e.message)
    }

    fn catalogue(engine: &str, names: &[&str]) -> Catalogue {
        let voices = (0..)
            .zip(names)
            .map(|(sid, name)| CatalogueEntry {
                speaker: SpeakerIndex(sid),
                name: name.to_string(),
            })
            .collect();
        Catalogue::new(EngineId::new(engine).unwrap(), voices)
    }

    fn with_catalogues() -> State {
        let mut s = state();
        s.set_catalogue(catalogue("kokoro", &["af_alloy", "af_bella", "af_kore"]));
        s.set_catalogue(catalogue("libritts", &["qorto", "walom"]));
        s
    }

    fn sentence_index(state: &State) -> Option<usize> {
        state.active_item().map(|i| i.index)
    }

    // --- DEC-1 queue semantics -------------------------------------------

    #[test]
    fn dec1_d2_say_clears_waiting() {
        let mut s = state();
        assert_eq!(run(&mut s, "say first"), "reading 1");
        assert_eq!(
            run(&mut s, "queue second"),
            "queued 1 sentences (queue length: 1)"
        );
        assert_eq!(run(&mut s, "say Third one. And more."), "reading 2");
        assert_eq!(s.active_item().unwrap().text, "Third one. And more.");
        assert_eq!(s.waiting_len(), 0);
    }

    #[test]
    fn dec1_d3_skip_abandons_current() {
        let mut s = state();
        run(&mut s, "say first");
        run(&mut s, "queue second");
        assert_eq!(run(&mut s, "skip"), "skipped item (queue length: 0)");
        assert_eq!(s.active_item().unwrap().text, "second");
        // Nothing waiting: skip stops (daemon.py).
        assert_eq!(run(&mut s, "skip"), "stopped");
        assert!(!s.is_active());
    }

    #[test]
    fn dec1_d4_queue_while_paused_appends() {
        let mut s = state();
        run(&mut s, "say first");
        assert_eq!(run(&mut s, "toggle"), "paused");
        run(&mut s, "queue second");
        assert_eq!(s.active_item().unwrap().text, "first");
        assert_eq!(s.waiting_len(), 1);
        assert!(s.paused);
    }

    #[test]
    fn dec1_queue_while_idle_plays() {
        let mut s = state();
        assert_eq!(run(&mut s, "queue One. Two."), "playing 2 sentences");
        assert!(s.is_active());
    }

    #[test]
    fn dec1_stop_keeps_queue_clear_drops_it() {
        let mut s = state();
        run(&mut s, "say first");
        run(&mut s, "queue second");
        assert_eq!(run(&mut s, "stop"), "stopped");
        assert!(!s.is_active());
        assert_eq!(s.waiting_len(), 1);
        run(&mut s, "say again");
        run(&mut s, "queue more");
        assert_eq!(run(&mut s, "clear"), "queue cleared");
        assert!(!s.is_active());
        assert_eq!(s.waiting_len(), 0);
    }

    #[test]
    fn sentences_are_populated_on_speak() {
        let mut s = state();
        run(&mut s, "say Dr. Smith went home. He slept!");
        let item = s.active_item().unwrap();
        assert_eq!(item.sentences, vec!["Dr. Smith went home.", "He slept!"]);
        assert_eq!(item.index, 0);
    }

    #[test]
    fn text_that_normalizes_to_nothing_is_rejected_untouched() {
        let mut s = state();
        run(&mut s, "say keep me");
        let before = s.clone();
        assert_eq!(
            fail(&mut s, "say **"),
            (ErrorCode::InvalidArgument, "empty".into())
        );
        assert_eq!(
            fail(&mut s, "queue **"),
            (ErrorCode::InvalidArgument, "empty text".into())
        );
        assert_eq!(s, before);
    }

    // --- Review bugs 3 and 4 -----------------------------------------------

    #[test]
    fn bug3_next_and_back_are_bounded() {
        let mut s = state();
        assert_eq!(run(&mut s, "next"), "idle");
        assert_eq!(run(&mut s, "back"), "idle");
        run(&mut s, "say One. Two. Three.");
        assert_eq!(run(&mut s, "back"), "1/3");
        assert_eq!(run(&mut s, "next"), "2/3");
        assert_eq!(run(&mut s, "next"), "3/3");
        assert_eq!(run(&mut s, "next"), "3/3");
        assert_eq!(sentence_index(&s), Some(2));
        let skip = s.apply(Operation::skip(crate::operations::Unit::Sentence));
        assert_eq!(skip.unwrap(), Reply::Text("3/3".into()));
    }

    #[test]
    fn bug3_seek_replays_and_invalidates_audio() {
        let mut s = state();
        run(&mut s, "say One. Two.");
        let g = s.generation;
        run(&mut s, "back");
        assert!(s.generation > g, "the clamped sentence replays");
    }

    #[test]
    fn bug4_toggle_while_idle_is_idle_and_changes_nothing() {
        let mut s = state();
        assert_eq!(run(&mut s, "toggle"), "idle");
        assert!(!s.paused);
        run(&mut s, "say hello");
        assert_eq!(run(&mut s, "toggle"), "paused");
        assert_eq!(run(&mut s, "toggle"), "resumed");
        run(&mut s, "stop");
        assert!(!s.paused, "stopping clears pause");
    }

    #[test]
    fn dec13_explicit_pause_and_resume() {
        let mut s = state();
        assert_eq!(
            s.apply(Operation::Pause).unwrap(),
            Reply::Text("idle".into())
        );
        run(&mut s, "say hello");
        assert_eq!(
            s.apply(Operation::Pause).unwrap(),
            Reply::Text("paused".into())
        );
        assert_eq!(
            s.apply(Operation::Pause).unwrap(),
            Reply::Text("paused".into())
        );
        assert_eq!(
            s.apply(Operation::Resume).unwrap(),
            Reply::Text("resumed".into())
        );
    }

    #[test]
    fn dec13_read_stops_when_active_and_is_unsupported_when_idle() {
        let mut s = state();
        run(&mut s, "say hello");
        assert_eq!(run(&mut s, "read"), "stopped");
        assert_eq!(fail(&mut s, "read").0, ErrorCode::UnsupportedOperation);
        assert_eq!(
            s.apply(Operation::ReadSelection).unwrap_err().code,
            ErrorCode::UnsupportedOperation
        );
    }

    // --- Review bug 5: honest replies ----------------------------------------

    #[test]
    fn bug5_status_catalogue_quit_reload_do_not_claim_ok() {
        let mut s = state();
        assert_eq!(s.apply(Operation::Status).unwrap(), Reply::Status);
        assert_eq!(
            s.apply(Operation::Catalogue).unwrap_err().code,
            ErrorCode::UnsupportedOperation
        );
        assert_eq!(run(&mut s, "quit"), "bye");
        assert_eq!(fail(&mut s, "reload").0, ErrorCode::EngineFailure);
        assert_eq!(run(&mut s, "unload"), "unloaded kokoro");
        let mut s = with_catalogues();
        match s.apply(Operation::Catalogue).unwrap() {
            Reply::Catalogue(c) => assert_eq!(c.voices.len(), 3),
            other => panic!("{other:?}"),
        }
    }

    // --- Settings replies ------------------------------------------------------

    #[test]
    fn speed_replies_and_rounds_to_exact_steps() {
        let mut s = state();
        assert_eq!(run(&mut s, "faster"), "speed 1.15");
        for _ in 0..9 {
            run(&mut s, "faster");
        }
        assert_eq!(s.prefs.speed, 2.5);
        assert_eq!(run(&mut s, "speed 9"), "speed 3.00");
        assert_eq!(run(&mut s, "faster"), "speed 3.00");
        assert_eq!(run(&mut s, "speed 0.1"), "speed 0.50");
        assert_eq!(run(&mut s, "slower"), "speed 0.50");
    }

    #[test]
    fn dec17_clamped_settings_report_the_applied_value() {
        let mut s = state();
        assert_eq!(run(&mut s, "font_size 200"), "font_size 72");
        assert_eq!(run(&mut s, "font_size 1"), "font_size 12");
        assert_eq!(run(&mut s, "words_visible 40"), "words_visible 15");
        assert_eq!(run(&mut s, "volume 150"), "volume 100");
    }

    #[test]
    fn simple_setting_replies() {
        let mut s = state();
        assert_eq!(run(&mut s, "aligner energy"), "aligner energy");
        assert_eq!(s.prefs.aligner, Aligner::Energy);
        assert_eq!(run(&mut s, "position top"), "position top");
        assert_eq!(s.prefs.position, Position::Top);
        assert_eq!(run(&mut s, "mode"), "rsvp");
        assert_eq!(run(&mut s, "mode_set off"), "off");
        assert_eq!(s.prefs.overlay_mode, OverlayMode::Off);
    }

    #[test]
    fn dec7_mode_cycles_in_documented_order() {
        let mut s = state();
        let order: Vec<String> = (0..4).map(|_| run(&mut s, "mode")).collect();
        assert_eq!(order, ["rsvp", "scroll_rsvp", "off", "subtitle"]);
    }

    #[test]
    fn r8_trim_is_per_voice_and_removed_at_zero() {
        let mut s = state();
        run(&mut s, "volume 60");
        assert_eq!(run(&mut s, "trim -15"), "trim kokoro:0 -15 (effective 45)");
        assert_eq!(
            run(&mut s, "trim 500"),
            "trim kokoro:0 +100 (effective 100)"
        );
        assert_eq!(run(&mut s, "trim 0"), "trim kokoro:0 +0 (effective 60)");
        assert!(s.prefs.trims.is_empty());
    }

    #[test]
    fn n10_engine_change_invalidates_audio_and_resets_voice() {
        let mut s = state();
        s.prefs.voice = 4;
        let g = s.generation;
        assert_eq!(run(&mut s, "engine libritts"), "engine libritts");
        assert!(s.generation > g);
        assert_eq!(s.prefs.voice, 0);
        let g = s.generation;
        assert_eq!(run(&mut s, "engine libritts"), "engine libritts");
        assert_eq!(s.generation, g, "same engine is not a change");
    }

    // --- DEC-10 voices ---------------------------------------------------------

    #[test]
    fn dec10_voice_by_index_name_and_qualified_name() {
        let mut s = with_catalogues();
        assert_eq!(run(&mut s, "voice 1"), "voice 1");
        assert_eq!(s.prefs.voice_id, Some(VoiceId("kokoro:af_bella".into())));
        // §4.1: the request that crashed Python 51 times.
        let g = s.generation;
        assert_eq!(run(&mut s, "voice af_kore"), "voice 2");
        assert!(s.generation > g, "N9: a voice change invalidates audio");
        assert_eq!(
            run(&mut s, "voice libritts:walom"),
            "voice 1 (engine libritts)"
        );
        assert_eq!(s.prefs.engine, EngineId::new("libritts").unwrap());
        assert_eq!(s.prefs.voice, 1);
    }

    #[test]
    fn dec10_unknown_voice_is_not_found_and_changes_nothing() {
        let mut s = with_catalogues();
        run(&mut s, "voice 1");
        let before = s.clone();
        assert_eq!(
            fail(&mut s, "voice 9"),
            (
                ErrorCode::NotFound,
                "voice index out of range: 9 (0..2)".into()
            )
        );
        assert_eq!(
            fail(&mut s, "voice bm_nobody"),
            (ErrorCode::NotFound, "unknown voice: bm_nobody".into())
        );
        assert_eq!(s, before);
        // Without a catalogue nothing resolves, and nothing falls back to 0.
        let mut bare = state();
        assert_eq!(fail(&mut bare, "voice 1").0, ErrorCode::NotFound);
    }

    #[test]
    fn speak_voice_prefix_travels_with_the_item() {
        let mut s = with_catalogues();
        run(&mut s, "say --voice af_kore hello");
        let voice = s.active_item().unwrap().voice.clone().unwrap();
        assert_eq!(voice.speaker, SpeakerIndex(2));
        assert_eq!(s.prefs.voice, 0, "the current voice is not changed");
        assert_eq!(
            fail(&mut s, "queue --voice nobody hi").0,
            ErrorCode::NotFound
        );
        assert_eq!(s.waiting_len(), 0);
    }

    // --- DEC-14 preview restore point -----------------------------------------

    fn preview(state: &mut State, voice: &str) {
        match state.apply(parse_legacy(&format!("preview {voice}")).unwrap()) {
            Ok(Reply::Preview(_)) => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn dec14_preview_during_playback_restores_sentence_and_pause() {
        let mut s = with_catalogues();
        run(&mut s, "say One. Two. Three.");
        run(&mut s, "next");
        preview(&mut s, "af_kore");
        assert!(s.previewing());
        assert!(s.paused, "the reading pauses while the preview plays");
        s.end_preview();
        assert!(!s.previewing());
        assert_eq!(sentence_index(&s), Some(1));
        assert!(!s.paused);
    }

    #[test]
    fn dec14_preview_while_paused_stays_paused() {
        let mut s = with_catalogues();
        run(&mut s, "say One. Two.");
        run(&mut s, "toggle");
        preview(&mut s, "0");
        s.end_preview();
        assert!(s.paused);
    }

    #[test]
    fn dec14_second_preview_keeps_the_first_restore_point() {
        let mut s = with_catalogues();
        run(&mut s, "say One. Two. Three.");
        run(&mut s, "next");
        preview(&mut s, "0");
        preview(&mut s, "1");
        s.end_preview();
        assert_eq!(sentence_index(&s), Some(1));
        assert!(!s.paused);
    }

    #[test]
    fn dec14_preview_while_idle_creates_no_phantom_state() {
        let mut s = with_catalogues();
        preview(&mut s, "af_bella");
        assert!(!s.paused);
        s.end_preview();
        assert!(!s.is_active());
        assert!(!s.paused);
    }

    #[test]
    fn dec14_unknown_preview_voice_does_not_disturb_playback() {
        let mut s = with_catalogues();
        run(&mut s, "say One. Two.");
        let before = s.clone();
        assert_eq!(fail(&mut s, "preview 99").0, ErrorCode::NotFound);
        assert_eq!(s, before);
    }

    // --- DEC-15 reset -------------------------------------------------------------

    #[test]
    fn dec15_reset_everything_restores_all_and_stops() {
        let mut s = state();
        s.prefs
            .unknown_fields
            .insert("future".into(), serde_json::json!(1));
        run(&mut s, "engine libritts");
        run(&mut s, "speed 2");
        run(&mut s, "volume 40");
        run(&mut s, "say hello");
        run(&mut s, "queue more");
        let g = s.generation;
        assert_eq!(run(&mut s, "reset"), "reset to defaults");
        assert_eq!(s.prefs.engine, EngineId::default_engine(), "N8: engine too");
        assert_eq!(s.prefs.speed, 1.0);
        assert_eq!(s.prefs.volume, 100);
        assert!(!s.is_active());
        assert_eq!(s.waiting_len(), 0);
        assert!(s.generation > g, "N9: rendered audio invalidated");
        assert_eq!(
            s.prefs.unknown_fields.get("future"),
            Some(&serde_json::json!(1))
        );
        // Both legacy verbs behave identically.
        run(&mut s, "speed 2");
        assert_eq!(run(&mut s, "reset_prefs"), "reset to defaults");
        assert_eq!(s.prefs.speed, 1.0);
    }

    #[test]
    fn dec15_group_reset_touches_only_its_group() {
        let mut s = state();
        run(&mut s, "font_size 40");
        run(&mut s, "mode_set rsvp");
        run(&mut s, "speed 2");
        run(&mut s, "engine libritts");
        assert_eq!(run(&mut s, "reset overlay"), "reset overlay to defaults");
        assert_eq!(s.prefs.font_size, 24);
        assert_eq!(s.prefs.overlay_mode, OverlayMode::Subtitle);
        assert_eq!(s.prefs.speed, 2.0);
        assert_eq!(run(&mut s, "reset speech"), "reset speech to defaults");
        assert_eq!(s.prefs.speed, 1.0);
        assert_eq!(s.prefs.engine, EngineId::default_engine());
    }

    #[test]
    fn test_dec16_engine_id_rejects_unknown_on_deserialize() {
        // Review bug 11: the inner field used to be `pub`, so `EngineId("x")`
        // bypassed `new`. Now construction and deserialization both validate.
        let bad: Result<EngineId, _> = serde_json::from_str("\"espeak\"");
        assert!(bad.is_err());
        let ok: EngineId = serde_json::from_str("\"kokoro\"").unwrap();
        assert_eq!(ok.as_str(), "kokoro");
    }
}
