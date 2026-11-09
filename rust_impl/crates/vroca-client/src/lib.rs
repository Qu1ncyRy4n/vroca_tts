#![forbid(unsafe_code)]
//! Public local client (DEC-5). The socket round trip is behind the
//! `transport` feature (DEC-18); the typed API that sits above it is slice D.

use std::path::Path;

#[cfg(feature = "transport")]
mod transport;

pub struct Client {
    // Read only by the transport.
    #[cfg_attr(not(feature = "transport"), allow(dead_code))]
    socket_path: std::path::PathBuf,
}

impl Client {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            socket_path: path.as_ref().to_path_buf(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Client>();
    }
}
