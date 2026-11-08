use crate::schema::{EngineId, Position, VoiceId};
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Operation {
    Speak { text: String, replace: Replace },
    Stop { scope: Scope },
    Skip { unit: Unit },
    Toggle,
    Read,
    Pause,
    Resume,
    ReadSelection,
    Next,
    Back,
    Faster,
    Slower,
    SetSpeed { speed: f32 },
    SetVoiceNumeric { sid: i32 },
    SetVoiceId { id: VoiceId },
    SetEngine { id: EngineId },
    SetAligner { name: String },
    SetFontSize { size: i32 },
    SetWordsVisible { count: i32 },
    SetPosition { pos: Position },
    CycleMode,
    Reset { group: ResetGroup },
    Status,
    Catalogue,
    Preview { sid: i32 },
    Unload,
    Reload,
    Quit,
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

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::NotFound,
            message: msg.into(),
        }
    }
}
