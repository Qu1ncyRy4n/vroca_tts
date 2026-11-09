//! Voice resolution through `(ModelId, SpeakerIndex)` (DEC-10, §10.5, §10.9).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use vroca_core::catalogue::{Catalogue, CatalogueEntry, ModelId};
use vroca_core::operations::ErrorCode;
use vroca_core::schema::{EngineId, SpeakerIndex, VoiceId, VoiceRef};

fn kokoro() -> Catalogue {
    let entry = |sid, name: &str| CatalogueEntry {
        speaker: SpeakerIndex(sid),
        name: name.to_string(),
    };
    Catalogue::new(
        EngineId::new("kokoro").unwrap(),
        vec![
            entry(0, "af_alloy"),
            entry(1, "af_bella"),
            entry(2, "af_kore"),
        ],
    )
}

#[test]
fn dec10_every_reference_form_resolves_to_model_and_speaker() {
    let catalogue = kokoro();
    for voice in [
        VoiceRef::Index(2),
        VoiceRef::Name("af_kore".into()),
        VoiceRef::Name("AF_KORE".into()),
        VoiceRef::Qualified(VoiceId("kokoro:af_kore".into())),
    ] {
        let resolved = catalogue.resolve(&voice).unwrap();
        assert_eq!(resolved.model, ModelId::for_engine(&catalogue.engine));
        assert_eq!(resolved.speaker, SpeakerIndex(2));
        assert_eq!(resolved.id, VoiceId("kokoro:af_kore".into()));
    }
}

#[test]
fn dec10_unknown_voice_is_not_found_never_voice_zero() {
    let catalogue = kokoro();
    let e = catalogue.resolve(&VoiceRef::Index(9)).unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
    assert_eq!(e.message, "voice index out of range: 9 (0..2)");
    let e = catalogue
        .resolve(&VoiceRef::Name("bm_nobody".into()))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
    assert_eq!(e.message, "unknown voice: bm_nobody");
}

#[test]
fn catalogue_rows_keep_python_sid_key() {
    let json = serde_json::to_value(&kokoro().voices).unwrap();
    assert_eq!(
        json.get(2).unwrap(),
        &serde_json::json!({"sid": 2, "name": "af_kore"})
    );
}
