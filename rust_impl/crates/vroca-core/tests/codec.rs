//! Structured codec tests (DEC-6): a round trip for every `Operation`
//! variant, and the malformed-input classes the decision names — malformed
//! JSON, unknown operation, wrong field type, missing field — plus the
//! unknown-policy-value case DEC-1 requires to be `invalid_argument`
//! (review bug 17).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use vroca_core::codec::{decode_request, encode_request};
use vroca_core::operations::{ErrorCode, Operation, Replace, ResetGroup, Scope, Unit};
use vroca_core::schema::{Aligner, EngineId, OverlayMode, Position, VoiceId, VoiceRef};

fn every_operation() -> Vec<Operation> {
    vec![
        Operation::Speak {
            text: "hello".into(),
            replace: Replace::All,
            voice: None,
        },
        Operation::Speak {
            text: "hello".into(),
            replace: Replace::None,
            voice: Some(VoiceRef::Qualified(VoiceId("kokoro:af_kore".into()))),
        },
        Operation::Stop {
            scope: Scope::Queue,
        },
        Operation::Skip {
            unit: Unit::Sentence,
        },
        Operation::Toggle,
        Operation::Read,
        Operation::Pause,
        Operation::Resume,
        Operation::ReadSelection,
        Operation::Next,
        Operation::Back,
        Operation::Faster,
        Operation::Slower,
        Operation::SetSpeed { speed: 1.25 },
        Operation::SetVoice {
            voice: VoiceRef::Index(9),
        },
        Operation::SetVoice {
            voice: VoiceRef::Name("af_kore".into()),
        },
        Operation::SetEngine {
            id: EngineId::new("libritts").unwrap(),
        },
        Operation::SetAligner {
            aligner: Aligner::Energy,
        },
        Operation::SetFontSize { size: 30 },
        Operation::SetWordsVisible { count: 4 },
        Operation::SetPosition { pos: Position::Top },
        Operation::SetVolume { level: 70 },
        Operation::SetTrim { amount: -10 },
        Operation::CycleMode,
        Operation::SetMode {
            mode: OverlayMode::ScrollRsvp,
        },
        Operation::Reset {
            group: ResetGroup::Overlay,
        },
        Operation::Status,
        Operation::Catalogue,
        Operation::Preview {
            voice: VoiceRef::Index(1),
        },
        Operation::Unload,
        Operation::Reload,
        Operation::Quit,
    ]
}

#[test]
fn every_operation_round_trips() {
    let ops = every_operation();
    for op in &ops {
        let bytes = encode_request(op).unwrap();
        let back = decode_request(&bytes).unwrap_or_else(|e| panic!("{op:?}: {e:?}"));
        assert_eq!(&back, op);
    }
    // The table above and `TYPE_NAMES` must cover the same variants.
    let mut seen: Vec<String> = ops
        .iter()
        .map(|op| {
            let v: serde_json::Value = serde_json::to_value(op).unwrap();
            v.get("type").and_then(|t| t.as_str()).unwrap().to_string()
        })
        .collect();
    seen.sort();
    seen.dedup();
    let mut names: Vec<String> = Operation::TYPE_NAMES
        .iter()
        .map(|s| s.to_string())
        .collect();
    names.sort();
    assert_eq!(seen, names, "TYPE_NAMES and the round-trip table disagree");
}

#[test]
fn dec16_scroll_rsvp_wire_spelling() {
    let bytes = encode_request(&Operation::SetMode {
        mode: OverlayMode::ScrollRsvp,
    })
    .unwrap();
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        r#"{"v":1,"type":"set_mode","mode":"scroll_rsvp"}"#
    );
}

#[test]
fn dec19_version_is_required_and_must_be_1() {
    let e = decode_request(br#"{"type":"status"}"#).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidRequest);
    assert_eq!(e.message, "request needs \"v\": 1");
    for bytes in [
        &br#"{"v":2,"type":"status"}"#[..],
        br#"{"v":0,"type":"status"}"#,
        br#"{"v":"1","type":"status"}"#,
        br#"{"v":1.5,"type":"status"}"#,
        br#"{"v":null,"type":"status"}"#,
    ] {
        let e = decode_request(bytes).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidRequest, "{bytes:?}: {e:?}");
        assert!(
            e.message.starts_with("unsupported protocol version"),
            "{e:?}"
        );
    }
    assert_eq!(
        decode_request(br#"{"v":1,"type":"status"}"#).unwrap(),
        Operation::Status
    );
}

#[test]
fn malformed_json_is_invalid_request() {
    for bytes in [&b"{"[..], b"not json", b"", b"[1,2]", b"\"speak\"", b"42"] {
        let e = decode_request(bytes).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidRequest, "{bytes:?}: {e:?}");
    }
}

#[test]
fn unknown_operation_is_invalid_request() {
    let e = decode_request(br#"{"v":1,"type":"frobnicate"}"#).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidRequest);
    assert_eq!(e.message, "unknown operation: frobnicate");
    let e = decode_request(br#"{"v":1,"text":"hi"}"#).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidRequest);
    let e = decode_request(br#"{"v":1,"type":7}"#).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidRequest);
}

#[test]
fn missing_required_field_is_invalid_request() {
    let e = decode_request(br#"{"v":1,"type":"speak","replace":"all"}"#).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidRequest, "{e:?}");
    assert!(e.message.starts_with("missing field"), "{e:?}");
}

#[test]
fn wrong_field_type_is_invalid_argument() {
    let e = decode_request(br#"{"v":1,"type":"set_speed","speed":"fast"}"#).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidArgument, "{e:?}");
    let e = decode_request(br#"{"v":1,"type":"speak","text":5,"replace":"all"}"#).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidArgument, "{e:?}");
}

#[test]
fn dec1_unknown_policy_value_is_invalid_argument() {
    // Review bug 17: this used to map to invalid_request.
    for bytes in [
        &br#"{"v":1,"type":"speak","text":"hi","replace":"maybe"}"#[..],
        br#"{"v":1,"type":"stop","scope":"everything"}"#,
        br#"{"v":1,"type":"skip","unit":"word"}"#,
        br#"{"v":1,"type":"reset","group":"audio"}"#,
        br#"{"v":1,"type":"set_mode","mode":"sideways"}"#,
        br#"{"v":1,"type":"set_aligner","aligner":"dtw"}"#,
        br#"{"v":1,"type":"set_engine","id":"espeak"}"#,
    ] {
        let e = decode_request(bytes).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidArgument, "{bytes:?}: {e:?}");
    }
}

#[test]
fn speak_without_voice_omits_the_field() {
    let bytes = encode_request(&Operation::Speak {
        text: "hi".into(),
        replace: Replace::All,
        voice: None,
    })
    .unwrap();
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        r#"{"v":1,"type":"speak","text":"hi","replace":"all"}"#
    );
}
