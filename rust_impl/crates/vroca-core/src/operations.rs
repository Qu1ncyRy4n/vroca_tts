use crate::schema::{Aligner, ChannelId, EngineId, OverlayMode, Position, VoiceRef};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Replace {
    All,
    Active,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Playback,
    Queue,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Sentence,
    Item,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResetGroup {
    Everything,
    Overlay,
    Speech,
}

impl ResetGroup {
    /// Legacy group names (DEC-15). Bare `reset` carries no group and means
    /// `Everything`, which is why it has no name here.
    pub const NAMES: [&'static str; 2] = ["overlay", "speech"];

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "overlay" => Some(Self::Overlay),
            "speech" => Some(Self::Speech),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Operation {
    // §10.9: queue operations carry a channel from the start. It is optional
    // on the wire (absent means `default`), so v1 requests stay valid. The
    // playback controls (toggle, next, back, pause, resume) carry none:
    // whether they act per channel is §10.8 10-d, still Open.
    Speak {
        #[serde(default, skip_serializing_if = "ChannelId::is_default")]
        channel: ChannelId,
        text: String,
        replace: Replace,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        voice: Option<VoiceRef>,
    },
    Stop {
        #[serde(default, skip_serializing_if = "ChannelId::is_default")]
        channel: ChannelId,
        scope: Scope,
    },
    Skip {
        #[serde(default, skip_serializing_if = "ChannelId::is_default")]
        channel: ChannelId,
        unit: Unit,
    },
    Toggle,
    Read,
    Pause,
    Resume,
    ReadSelection,
    Next,
    Back,
    Faster,
    Slower,
    SetSpeed {
        speed: f32,
    },
    SetVoice {
        voice: VoiceRef,
    },
    SetEngine {
        id: EngineId,
    },
    SetAligner {
        aligner: Aligner,
    },
    SetFontSize {
        size: i32,
    },
    SetWordsVisible {
        count: i32,
    },
    SetPosition {
        pos: Position,
    },
    SetVolume {
        level: i32,
    },
    SetTrim {
        amount: i32,
    },
    CycleMode,
    SetMode {
        mode: OverlayMode,
    },
    Reset {
        group: ResetGroup,
    },
    Status,
    Catalogue,
    Preview {
        voice: VoiceRef,
    },
    Unload,
    Reload,
    Quit,
}

impl Operation {
    /// `Speak` on the default channel.
    pub fn speak(text: impl Into<String>, replace: Replace, voice: Option<VoiceRef>) -> Self {
        Self::Speak {
            channel: ChannelId::default(),
            text: text.into(),
            replace,
            voice,
        }
    }

    /// `Stop` on the default channel.
    pub fn stop(scope: Scope) -> Self {
        Self::Stop {
            channel: ChannelId::default(),
            scope,
        }
    }

    /// `Skip` on the default channel.
    pub fn skip(unit: Unit) -> Self {
        Self::Skip {
            channel: ChannelId::default(),
            unit,
        }
    }

    /// Wire names of every variant, as `#[serde(tag = "type")]` spells them.
    /// The codec uses this to tell an unknown operation (`invalid_request`)
    /// from a bad field inside a known one (`invalid_argument`).
    pub const TYPE_NAMES: &'static [&'static str] = &[
        "speak",
        "stop",
        "skip",
        "toggle",
        "read",
        "pause",
        "resume",
        "read_selection",
        "next",
        "back",
        "faster",
        "slower",
        "set_speed",
        "set_voice",
        "set_engine",
        "set_aligner",
        "set_font_size",
        "set_words_visible",
        "set_position",
        "set_volume",
        "set_trim",
        "cycle_mode",
        "set_mode",
        "reset",
        "status",
        "catalogue",
        "preview",
        "unload",
        "reload",
        "quit",
    ];
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Legacy(String),
    Structured(Operation),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidArgument,
    NotFound,
    InvalidRequest,
    EngineFailure,
    Unavailable,
    Busy,
    Timeout,
}

impl ErrorCode {
    /// Stable process exit code for the CLI, so scripts and hotkey bindings can
    /// tell a bad argument from a missing daemon. 1 is reserved for internal
    /// and I/O failures. Pattern borrowed from omnichan `omni-core/src/error.rs`.
    pub fn exit_code(&self) -> u8 {
        match self {
            ErrorCode::InvalidArgument | ErrorCode::InvalidRequest => 2,
            ErrorCode::NotFound => 3,
            ErrorCode::Unavailable => 4,
            ErrorCode::Busy | ErrorCode::Timeout => 5,
            ErrorCode::EngineFailure => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolError {
    pub code: ErrorCode,
    pub message: String,
}

impl ProtocolError {
    pub fn exit_code(&self) -> u8 {
        self.code.exit_code()
    }

    pub fn invalid_argument(msg: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InvalidArgument,
            message: msg.into(),
        }
    }

    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InvalidRequest,
            message: msg.into(),
        }
    }

    pub fn unavailable(msg: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Unavailable,
            message: msg.into(),
        }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::NotFound,
            message: msg.into(),
        }
    }
}
