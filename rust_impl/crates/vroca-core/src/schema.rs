use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeakerIndex(pub i32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineId(pub String);

impl EngineId {
    pub fn new(id: &str) -> Result<Self, &'static str> {
        let valid = ["kokoro", "supertonic", "libritts", "zipvoice", "remote"];
        if valid.contains(&id) {
            Ok(Self(id.to_string()))
        } else {
            Err("unknown engine")
        }
    }
}

impl fmt::Display for EngineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Position {
    Bottom,
    Top,
    Center,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayMode {
    Subtitle,
    Rsvp,
    ScrollRsvp,
    Off,
}

fn default_schema() -> u32 {
    0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default = "default_schema")]
    pub schema: u32,
    pub engine: EngineId,
    pub voice: VoiceId,
    pub speed: f32,
    pub aligner: String,
    pub font_size: i32,
    pub words_visible: i32,
    pub position: Position,
    pub overlay_mode: OverlayMode,
    #[serde(flatten)]
    pub unknown_fields: HashMap<String, serde_json::Value>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            schema: 1,
            engine: EngineId("kokoro".to_string()),
            voice: VoiceId("kokoro:af_bella".to_string()),
            speed: 1.0,
            aligner: "asr".to_string(),
            font_size: 24,
            words_visible: 3,
            position: Position::Bottom,
            overlay_mode: OverlayMode::Subtitle,
            unknown_fields: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Health {
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    pub schema: u32,
    pub daemon_id: String,
    pub sentence: String,
    pub index: usize,
    pub total: usize,
    pub paused: bool,
    pub speed: f32,
    pub rendered: usize,
    pub rendering: Option<usize>,
    pub loaded: bool,
    pub voice: SpeakerIndex,
    pub word: i32,
    pub engine: EngineId,
    pub engines: Vec<EngineId>,
    pub aligner: String,
    pub queue_len: usize,
    pub font_size: i32,
    pub words_visible: i32,
    pub position: Position,
    pub voices: Option<usize>,
    pub last_render_ms: Option<u64>,
    pub avg_render_ms: Option<u64>,
}
