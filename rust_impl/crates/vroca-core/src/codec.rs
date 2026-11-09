//! The structured protocol codec (Decision 6), version 1 (Decision 19).
//!
//! Every request is a JSON object carrying `"v": 1` beside the operation's
//! `"type"` and fields; any other version, or none, is `invalid_request`.
//! Bytes that are not a versioned object naming a known operation are
//! `invalid_request`; a known operation with a bad field is
//! `invalid_argument` (DEC-1 "unknown policy value"; review bug 17).

use crate::operations::{Operation, ProtocolError};

use serde::{Deserialize, Serialize};

/// The structured protocol version, bound to `vroca-v1.sock` (DEC-3, DEC-19).
pub const PROTOCOL_VERSION: u64 = 1;

#[derive(Serialize)]
struct Envelope<'a> {
    v: u64,
    #[serde(flatten)]
    op: &'a Operation,
}

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
    serde_json::to_vec(&Envelope {
        v: PROTOCOL_VERSION,
        op,
    })
}

pub fn decode_request(data: &[u8]) -> Result<Operation, ProtocolError> {
    let mut value: serde_json::Value = serde_json::from_slice(data)
        .map_err(|e| ProtocolError::invalid_request(format!("invalid JSON: {e}")))?;
    let Some(object) = value.as_object_mut() else {
        return Err(ProtocolError::invalid_request(
            "request must be a JSON object",
        ));
    };
    // The version is checked first: another version may shape its
    // operations differently, so nothing else is read until it matches.
    match object.remove("v") {
        Some(v) if v.as_u64() == Some(PROTOCOL_VERSION) => {}
        Some(v) => {
            return Err(ProtocolError::invalid_request(format!(
                "unsupported protocol version: {v} (this daemon speaks {PROTOCOL_VERSION})"
            )))
        }
        None => {
            return Err(ProtocolError::invalid_request(format!(
                "request needs \"v\": {PROTOCOL_VERSION}"
            )))
        }
    }
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
