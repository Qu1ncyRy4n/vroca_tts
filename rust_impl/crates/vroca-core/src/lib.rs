//! Vroca domain: typed operations, state transitions, the legacy parser, and
//! the structured codec. Serde only (DEC-2); no I/O, no `unsafe` (§7.6).
#![forbid(unsafe_code)]

pub mod codec;
pub mod operations;
pub mod parser;
pub mod schema;
pub mod state;

#[cfg(test)]
pub mod tests;
