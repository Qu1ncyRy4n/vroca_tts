//! State-machine tests. Parser fixtures live in `tests/legacy_fixtures.rs`
//! and codec tests in `tests/codec.rs`, both against the public API.

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod cases {
    use crate::operations::Operation;
    use crate::parser::parse_legacy;
    use crate::schema::{Aligner, OverlayMode};
    use crate::state::State;

    #[test]
    fn test_d2_say_clears_waiting() {
        let mut state = State::new(Default::default());
        state.apply(parse_legacy("say first").unwrap());
        state.apply(parse_legacy("queue second").unwrap());
        assert_eq!(state.waiting_queue.len(), 1);
        state.apply(parse_legacy("say third").unwrap());
        assert_eq!(state.active_item.as_ref().unwrap().text, "third");
        assert_eq!(state.waiting_queue.len(), 0);
    }

    #[test]
    fn test_d3_skip_abandons_current() {
        let mut state = State::new(Default::default());
        state.apply(parse_legacy("say first").unwrap());
        state.apply(parse_legacy("queue second").unwrap());
        assert_eq!(state.active_item.as_ref().unwrap().text, "first");
        state.apply(parse_legacy("skip").unwrap());
        assert_eq!(state.active_item.as_ref().unwrap().text, "second");
        assert_eq!(state.waiting_queue.len(), 0);
    }

    #[test]
    fn test_d4_queue_paused_appends() {
        let mut state = State::new(Default::default());
        state.apply(parse_legacy("say first").unwrap());
        state.apply(parse_legacy("toggle").unwrap());
        assert!(state.paused);
        state.apply(parse_legacy("queue second").unwrap());
        assert_eq!(state.active_item.as_ref().unwrap().text, "first");
        assert_eq!(state.waiting_queue.len(), 1);
    }

    #[test]
    fn test_dec7_mode_cycle_and_set() {
        let mut state = State::new(Default::default());
        assert_eq!(state.prefs.overlay_mode, OverlayMode::Subtitle);
        for expected in [
            OverlayMode::Rsvp,
            OverlayMode::ScrollRsvp,
            OverlayMode::Off,
            OverlayMode::Subtitle,
        ] {
            state.apply(Operation::CycleMode);
            assert_eq!(state.prefs.overlay_mode, expected);
        }
        state.apply(parse_legacy("mode_set scroll_rsvp").unwrap());
        assert_eq!(state.prefs.overlay_mode, OverlayMode::ScrollRsvp);
    }

    #[test]
    fn test_aligner_is_typed() {
        let mut state = State::new(Default::default());
        state.apply(parse_legacy("aligner energy").unwrap());
        assert_eq!(state.prefs.aligner, Aligner::Energy);
    }

    #[test]
    fn test_schema_migration_and_unknown_fields() {
        use crate::schema::Preferences;
        use serde_json::json;

        let unversioned_json = json!({
            "engine": "libritts",
            "voice": "libritts:qorto",
            "speed": 1.5,
            "aligner": "asr",
            "font_size": 25,
            "words_visible": 2,
            "position": "center",
            "overlay_mode": "scroll_rsvp",
            "some_future_field": "hello"
        });

        let mut prefs: Preferences = serde_json::from_value(unversioned_json).unwrap();
        assert_eq!(prefs.schema, 0); // Unversioned defaults to 0
        assert_eq!(
            prefs
                .unknown_fields
                .get("some_future_field")
                .unwrap()
                .as_str()
                .unwrap(),
            "hello"
        );

        // Migrate
        prefs.schema = 1;
        let written = serde_json::to_value(&prefs).unwrap();
        assert_eq!(written.get("schema").unwrap().as_u64().unwrap(), 1);
        assert_eq!(
            written.get("some_future_field").unwrap().as_str().unwrap(),
            "hello"
        );
    }

    #[test]
    fn test_dec16_engine_id_rejects_unknown_on_deserialize() {
        // Review bug 11: the inner field used to be `pub`, so `EngineId("x")`
        // bypassed `new`. Now construction and deserialization both validate.
        use crate::schema::EngineId;
        let bad: Result<EngineId, _> = serde_json::from_str("\"espeak\"");
        assert!(bad.is_err());
        let ok: EngineId = serde_json::from_str("\"kokoro\"").unwrap();
        assert_eq!(ok.as_str(), "kokoro");
    }
}
