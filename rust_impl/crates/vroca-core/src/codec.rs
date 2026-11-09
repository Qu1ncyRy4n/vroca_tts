//! The structured protocol codec (Decision 6).
//!
//! Unstable, unversioned, and not bound to any socket; exercised by tests
//! only until slice 2 binds it to `vroca-v1.sock`. Error classification:
//! bytes that are not a JSON object naming a known operation are
//! `invalid_request`; a known operation with a bad field is
//! `invalid_argument` (DEC-1 "unknown policy value"; review bug 17).

use crate::operations::{Operation, ProtocolError};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "status")]
pub enum Response {
    Ok,
    String {
        data: String,
    },
    Status {
        snapshot: Box<crate::schema::RuntimeSnapshot>,
    },
    Catalogue {
        data: serde_json::Value,
    },
    Error {
        error: ProtocolError,
    },
}

pub fn encode_request(op: &Operation) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(op)
}

pub fn decode_request(data: &[u8]) -> Result<Operation, ProtocolError> {
    let value: serde_json::Value = serde_json::from_slice(data)
        .map_err(|e| ProtocolError::invalid_request(format!("invalid JSON: {e}")))?;
    let Some(object) = value.as_object() else {
        return Err(ProtocolError::invalid_request(
            "request must be a JSON object",
        ));
    };
    let Some(kind) = object.get("type").and_then(serde_json::Value::as_str) else {
        return Err(ProtocolError::invalid_request(
            "request needs a string \"type\"",
        ));
    };
    if !Operation::TYPE_NAMES.contains(&kind) {
        return Err(ProtocolError::invalid_request(format!(
            "unknown operation: {kind}"
        )));
    }
    serde_json::from_value(value).map_err(|e| {
        // A missing field is a malformed request; a present field with an
        // unknown enum value or wrong type is a bad argument.
        let text = e.to_string();
        if text.starts_with("missing field") {
            ProtocolError::invalid_request(text)
        } else {
            ProtocolError::invalid_argument(text)
        }
    })
}

pub fn encode_response(res: &Response) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(res)
}

pub fn decode_response(data: &[u8]) -> Result<Response, ProtocolError> {
    serde_json::from_slice(data)
        .map_err(|e| ProtocolError::invalid_request(format!("invalid JSON response: {e}")))
}
