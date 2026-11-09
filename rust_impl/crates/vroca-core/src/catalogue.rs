//! Voice resolution (DEC-10, refined by §10.5 and required in shape by §10.9).
//!
//! A client names a voice with a `VoiceRef`; resolution turns it into a
//! `ResolvedVoice` keyed by `(ModelId, SpeakerIndex)`, because residency is
//! per model, not per voice. A `Catalogue` is one engine's voice list. It is
//! static repository data (DEC-11), so resolution needs no loaded engine.

use serde::{Deserialize, Serialize};

use crate::operations::ProtocolError;
use crate::schema::{EngineId, SpeakerIndex, VoiceId, VoiceRef};

/// A loadable model: the unit of residency, memory accounting, and
/// `model load`/`unload` (§10.5). One model per engine today.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ModelId(String);

impl ModelId {
    pub fn for_engine(engine: &EngineId) -> Self {
        Self(engine.as_str().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedVoice {
    pub id: VoiceId,
    pub model: ModelId,
    pub speaker: SpeakerIndex,
}

/// One catalogue row. Serialized with Python's `sid` key so the legacy
/// `catalogue` reply keeps its shape (DEC-10, DEC-11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogueEntry {
    #[serde(rename = "sid")]
    pub speaker: SpeakerIndex,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalogue {
    pub engine: EngineId,
    pub model: ModelId,
    pub voices: Vec<CatalogueEntry>,
}

impl Catalogue {
    pub fn new(engine: EngineId, voices: Vec<CatalogueEntry>) -> Self {
        Self {
            model: ModelId::for_engine(&engine),
            engine,
            voices,
        }
    }

    /// Resolve a reference against this catalogue. The engine prefix of a
    /// qualified id is the caller's business: it picks the catalogue.
    /// Unknown voices are `not_found`, never a silent fallback to voice 0
    /// (DEC-10); messages are daemon.py's reply text.
    pub fn resolve(&self, voice: &VoiceRef) -> Result<ResolvedVoice, ProtocolError> {
        let entry = match voice {
            VoiceRef::Index(index) => self
                .voices
                .iter()
                .find(|e| e.speaker.0 == *index)
                .ok_or_else(|| {
                    ProtocolError::not_found(format!(
                        "voice index out of range: {index} (0..{})",
                        self.voices.len().saturating_sub(1)
                    ))
                })?,
            VoiceRef::Name(name) => self.by_name(name)?,
            VoiceRef::Qualified(id) => {
                let name = id.0.split_once(':').map_or(id.0.as_str(), |(_, n)| n);
                self.by_name(name)?
            }
        };
        Ok(ResolvedVoice {
            id: VoiceId(format!("{}:{}", self.engine, entry.name)),
            model: self.model.clone(),
            speaker: entry.speaker.clone(),
        })
    }

    fn by_name(&self, name: &str) -> Result<&CatalogueEntry, ProtocolError> {
        let want = name.to_lowercase();
        self.voices
            .iter()
            .find(|e| e.name.to_lowercase() == want)
            .ok_or_else(|| ProtocolError::not_found(format!("unknown voice: {name}")))
    }
}
