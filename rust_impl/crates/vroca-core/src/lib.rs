//! Vroca domain: typed operations, state transitions, the legacy parser, and
//! the structured codec. Serde only (DEC-2); no I/O, no `unsafe` (§7.6).
#![forbid(unsafe_code)]

pub mod catalogue;
pub mod codec;
pub mod operations;
pub mod parser;
pub mod prefs;
pub mod schema;
pub mod state;
pub mod text;

#[cfg(test)]
pub mod tests;
