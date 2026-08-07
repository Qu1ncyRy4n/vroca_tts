#[cfg(test)]
mod tests {
    use crate::operations::{Operation, Replace, Scope};
    use crate::parser::parse_legacy;
    use crate::schema::{EngineId, Position};
    use crate::state::State;

    #[test]
    fn test_d8_malformed_float_survives() {
        let op = parse_legacy("speed abc");
        assert!(op.is_err());
        let op = parse_legacy("speed");
        assert!(op.is_err());
        let op = parse_legacy("speed 1 2");
        assert!(op.is_err());
        let op = parse_legacy("voice abc");
        assert!(op.is_err());
        let op = parse_legacy("font_size abc");
        assert!(op.is_err());
        let op = parse_legacy("words_visible");
        assert!(op.is_err());
        let op = parse_legacy("preview abc");
        assert!(op.is_err());
        let op = parse_legacy("read xyz");
        assert!(op.is_err());
        let op = parse_legacy("say");
        assert!(op.is_err());
        let op = parse_legacy("queue");
        assert!(op.is_err());
    }

    #[test]
    fn test_legacy_parser_fixtures() {
        assert_eq!(parse_legacy("read").unwrap(), Operation::Read);
        assert_eq!(
            parse_legacy("say hello world").unwrap(),
            Operation::Speak {
                text: "hello world".to_string(),
                replace: Replace::All
            }
        );
        assert_eq!(
            parse_legacy("queue next").unwrap(),
            Operation::Speak {
                text: "next".to_string(),
                replace: Replace::None
            }
        );
        assert_eq!(
            parse_legacy("stop").unwrap(),
            Operation::Stop {
                scope: Scope::Playback
            }
        );
        assert_eq!(
            parse_legacy("clear").unwrap(),
            Operation::Stop { scope: Scope::All }
        );
        assert_eq!(
            parse_legacy("skip").unwrap(),
            Operation::Skip {
                unit: crate::operations::Unit::Item
            }
        );
        assert_eq!(parse_legacy("toggle").unwrap(), Operation::Toggle);
        assert_eq!(
            parse_legacy("speed 1.5").unwrap(),
            Operation::SetSpeed { speed: 1.5 }
        );
        assert_eq!(
            parse_legacy("voice 3").unwrap(),
            Operation::SetVoiceNumeric { sid: 3 }
        );
        assert_eq!(
            parse_legacy("engine supertonic").unwrap(),
            Operation::SetEngine {
                id: crate::schema::EngineId("supertonic".to_string())
            }
        );
        assert!(parse_legacy("engine unknown").is_err());
        assert_eq!(
            parse_legacy("aligner energy").unwrap(),
            Operation::SetAligner {
                name: "energy".to_string()
            }
        );
        assert_eq!(
            parse_legacy("font_size 25").unwrap(),
            Operation::SetFontSize { size: 25 }
        );
        assert_eq!(
            parse_legacy("words_visible 2").unwrap(),
            Operation::SetWordsVisible { count: 2 }
        );
        assert_eq!(
            parse_legacy("position center").unwrap(),
            Operation::SetPosition {
                pos: Position::Center
            }
        );
        assert!(parse_legacy("position up").is_err());
        assert_eq!(parse_legacy("mode").unwrap(), Operation::CycleMode);
        assert_eq!(
            parse_legacy("reset").unwrap(),
            Operation::Reset {
                group: crate::operations::ResetGroup::Everything
            }
        );
        assert_eq!(
            parse_legacy("reset_prefs").unwrap(),
            Operation::Reset {
                group: crate::operations::ResetGroup::Everything
            }
        );
        assert_eq!(parse_legacy("status").unwrap(), Operation::Status);
        assert_eq!(parse_legacy("catalogue").unwrap(), Operation::Catalogue);
        assert_eq!(
            parse_legacy("preview 1").unwrap(),
            Operation::Preview { sid: 1 }
        );
        assert_eq!(parse_legacy("unload").unwrap(), Operation::Unload);
        assert_eq!(parse_legacy("reload").unwrap(), Operation::Reload);
        assert_eq!(parse_legacy("quit").unwrap(), Operation::Quit);
        assert!(parse_legacy("unknown_command args").is_err());
    }

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
    fn test_structured_codec() {
        use crate::codec::{decode_request, encode_request};
        let op = Operation::Speak {
            text: "hello".to_string(),
            replace: Replace::All,
        };
        let enc = encode_request(&op).unwrap();
        let dec = decode_request(&enc).unwrap();
        assert_eq!(op, dec);
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
    fn test_core_dependencies() {
        // Assert that vroca-core does not depend on I/O, async, GUI, or FFI crates.
        let toml_content = include_str!("../Cargo.toml");
        assert!(!toml_content.contains("tokio"));
        assert!(!toml_content.contains("async"));
        assert!(!toml_content.contains("gtk"));
        assert!(!toml_content.contains("sherpa"));
        assert!(!toml_content.contains("libc"));
    }
}
