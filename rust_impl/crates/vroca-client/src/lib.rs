#![forbid(unsafe_code)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

use vroca_core::codec::{decode_response, Response};
use vroca_core::operations::{Operation, ProtocolError, Request};

pub struct Client {
    socket_path: std::path::PathBuf,
}

impl Client {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            socket_path: path.as_ref().to_path_buf(),
        }
    }

    pub fn send(&self, op: Operation) -> Result<Response, ProtocolError> {
        let req_bytes = vroca_core::codec::encode_request(&op).map_err(|e| {
            ProtocolError::invalid_request(format!("failed to serialize request: {}", e))
        })?;

        let mut stream = UnixStream::connect(&self.socket_path).map_err(|_| ProtocolError {
            code: vroca_core::operations::ErrorCode::Unavailable,
            message: "daemon socket missing or inaccessible".to_string(),
        })?;

        stream.write_all(&req_bytes).map_err(|e| ProtocolError {
            code: vroca_core::operations::ErrorCode::Unavailable,
            message: format!("write failed: {}", e),
        })?;

        let _ = stream.shutdown(std::net::Shutdown::Write);

        let mut response_bytes = Vec::new();
        stream
            .read_to_end(&mut response_bytes)
            .map_err(|e| ProtocolError {
                code: vroca_core::operations::ErrorCode::Unavailable,
                message: format!("read failed: {}", e),
            })?;

        decode_response(&response_bytes)
    }

    pub fn speak(&self, text: &str) -> Result<Response, ProtocolError> {
        self.send(Operation::Speak {
            text: text.to_string(),
            replace: vroca_core::operations::Replace::All,
        })
    }

    pub fn queue(&self, text: &str) -> Result<Response, ProtocolError> {
        self.send(Operation::Speak {
            text: text.to_string(),
            replace: vroca_core::operations::Replace::None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::thread;

    #[test]
    fn test_client_daemon_integration() {
        let temp_dir = tempfile::tempdir().unwrap();
        let socket_path = temp_dir.path().join("vroca-v1.sock");
        let listener = UnixListener::bind(&socket_path).unwrap();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut req_bytes = Vec::new();
            stream.read_to_end(&mut req_bytes).unwrap();

            let op = vroca_core::codec::decode_request(&req_bytes).unwrap();
            assert!(matches!(op, Operation::Speak { .. }));

            let res = Response::Ok;
            let res_bytes = serde_json::to_vec(&res).unwrap();
            stream.write_all(&res_bytes).unwrap();
        });

        let client = Client::new(&socket_path);
        let res = client.speak("hello world").unwrap();
        assert_eq!(res, Response::Ok);

        handle.join().unwrap();
    }

    #[test]
    fn test_missing_socket() {
        let client = Client::new("/tmp/nonexistent-vroca.sock");
        let err = client.speak("hello").unwrap_err();
        assert_eq!(err.code, vroca_core::operations::ErrorCode::Unavailable);
    }

    #[test]
    fn test_client_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Client>();
    }
}
