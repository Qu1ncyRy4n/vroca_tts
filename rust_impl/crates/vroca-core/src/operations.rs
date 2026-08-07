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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolError {
    pub code: ErrorCode,
    pub message: String,
}

impl ProtocolError {
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
