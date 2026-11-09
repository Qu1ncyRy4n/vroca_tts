use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeakerIndex(pub i32);

/// A speech channel (§10.4). Queue state is keyed by channel from the start
/// (§10.9), but only `default` exists: how channels are named is §10.8 10-b,
/// still Open, so any other name is rejected rather than decided implicitly.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ChannelId(String);

impl ChannelId {
    pub const DEFAULT: &'static str = "default";

    pub fn new(name: &str) -> Result<Self, String> {
        if name == Self::DEFAULT {
            Ok(Self(name.to_string()))
        } else {
            Err(format!(
                "unknown channel: {name} (only \"{}\" exists; rust-spec.md §10.8 10-b)",
                Self::DEFAULT
            ))
        }
    }

    pub fn is_default(&self) -> bool {
        self.0 == Self::DEFAULT
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ChannelId {
    fn default() -> Self {
        Self(Self::DEFAULT.to_string())
    }
}

impl TryFrom<String> for ChannelId {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl From<ChannelId> for String {
    fn from(id: ChannelId) -> Self {
        id.0
    }
}

/// A voice as a client addresses it, before catalogue resolution (DEC-10).
///
/// The legacy socket accepts an index, a bare catalogue name, or an
/// engine-qualified name. Resolving any of these to a `(ModelId,
/// SpeakerIndex)` is runtime work; the parser only classifies the token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceRef {
    Index(i32),
    Name(String),
    Qualified(VoiceId),
}

impl VoiceRef {
    /// Classify a single token the way `daemon.py` `select_voice` does: an
    /// integer is an index, `engine:name` with a known engine is qualified,
    /// anything else is an engine-local name.
    pub fn from_token(token: &str) -> Self {
        if let Ok(index) = token.parse::<i32>() {
            return Self::Index(index);
        }
        if let Some((engine, rest)) = token.split_once(':') {
            if !rest.is_empty() && EngineId::new(engine).is_ok() {
                return Self::Qualified(VoiceId(token.to_string()));
            }
        }
        Self::Name(token.to_string())
    }
}

/// Validated engine identity (DEC-16). The inner string is private so the
/// only constructor is `new`, and deserialization goes through the same check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EngineId(String);

impl EngineId {
    pub const KNOWN: [&'static str; 5] = ["kokoro", "supertonic", "libritts", "zipvoice", "remote"];

    pub fn new(id: &str) -> Result<Self, String> {
        if Self::KNOWN.contains(&id) {
            Ok(Self(id.to_string()))
        } else {
            Err(format!(
                "unknown engine: {id} (have {})",
                Self::KNOWN.join(", ")
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The factory default engine. Infallible because the name is in `KNOWN`.
    pub fn default_engine() -> Self {
        Self("kokoro".to_string())
    }
}

impl TryFrom<String> for EngineId {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl From<EngineId> for String {
    fn from(id: EngineId) -> Self {
        id.0
    }
}

impl fmt::Display for EngineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Position {
    Bottom,
    Top,
    Center,
}

impl Position {
    pub const NAMES: [&'static str; 3] = ["bottom", "top", "center"];

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "bottom" => Some(Self::Bottom),
            "top" => Some(Self::Top),
            "center" => Some(Self::Center),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayMode {
    Subtitle,
    Rsvp,
    ScrollRsvp,
    Off,
}

impl OverlayMode {
    /// Wire spellings, in the documented cycle order (DEC-7).
    pub const NAMES: [&'static str; 4] = ["subtitle", "rsvp", "scroll_rsvp", "off"];

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "subtitle" => Some(Self::Subtitle),
            "rsvp" => Some(Self::Rsvp),
            "scroll_rsvp" => Some(Self::ScrollRsvp),
            "off" => Some(Self::Off),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Subtitle => "subtitle",
            Self::Rsvp => "rsvp",
            Self::ScrollRsvp => "scroll_rsvp",
            Self::Off => "off",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Subtitle => Self::Rsvp,
            Self::Rsvp => Self::ScrollRsvp,
            Self::ScrollRsvp => Self::Off,
            Self::Off => Self::Subtitle,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Aligner {
    Asr,
    Energy,
}

impl Aligner {
    pub const NAMES: [&'static str; 2] = ["asr", "energy"];

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "asr" => Some(Self::Asr),
            "energy" => Some(Self::Energy),
            _ => None,
        }
    }
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
    pub aligner: Aligner,
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
            engine: EngineId::default_engine(),
            voice: VoiceId("kokoro:af_bella".to_string()),
            speed: 1.0,
            aligner: Aligner::Asr,
            font_size: 24,
            words_visible: 3,
            position: Position::Bottom,
            overlay_mode: OverlayMode::Subtitle,
            unknown_fields: HashMap::new(),
        }
    }
}

/// Daemon health as published in the snapshot (DEC-9 rule 3, N4). A dead
/// player or a failing snapshot write is visible here instead of stalling
/// silently; control operations keep working while degraded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Health {
    #[default]
    Ok,
    Degraded {
        problems: Vec<String>,
    },
}

impl Health {
    /// Record a problem once; repeating the same problem is not a change.
    pub fn degrade(&mut self, problem: String) {
        match self {
            Self::Ok => {
                *self = Self::Degraded {
                    problems: vec![problem],
                }
            }
            Self::Degraded { problems } => {
                if !problems.contains(&problem) {
                    problems.push(problem);
                }
            }
        }
    }
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
    /// `null` until the voice resolves against a catalogue (DEC-10); never a
    /// guessed index.
    pub voice: Option<SpeakerIndex>,
    pub word: i32,
    pub engine: EngineId,
    pub engines: Vec<EngineId>,
    pub aligner: Aligner,
    pub queue_len: usize,
    pub font_size: i32,
    pub words_visible: i32,
    pub position: Position,
    pub voices: Option<usize>,
    pub last_render_ms: Option<u64>,
    pub avg_render_ms: Option<u64>,
    pub health: Health,
}
