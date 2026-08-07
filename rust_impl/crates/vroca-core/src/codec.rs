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
        snapshot: crate::schema::RuntimeSnapshot,
    },
    Catalogue {
        data: serde_json::Value,
    },
    Error {
        error: ProtocolError,
    },
}

/// Unstable structured protocol codec (Decision 6).
/// Currently unversioned and unbound to any socket.
pub fn encode_request(op: &Operation) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(op)
}

pub fn decode_request(data: &[u8]) -> Result<Operation, ProtocolError> {
    serde_json::from_slice(data)
        .map_err(|e| ProtocolError::invalid_request(format!("invalid JSON: {}", e)))
}

pub fn encode_response(res: &Response) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(res)
}

pub fn decode_response(data: &[u8]) -> Result<Response, ProtocolError> {
    serde_json::from_slice(data)
        .map_err(|e| ProtocolError::invalid_request(format!("invalid JSON response: {}", e)))
}
