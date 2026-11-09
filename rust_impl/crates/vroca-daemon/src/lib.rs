//! Vroca daemon: the state owner, the player boundary, and (with the
//! `transport` feature) socket lifecycle. Default features build no socket,
//! child process, or systemd code (DEC-18); `tests/dec18_default_features.rs`
//! enforces that.
#![forbid(unsafe_code)]

pub mod owner;
pub mod player;

#[cfg(feature = "transport")]
pub mod transport;
