# Vroca Rust Design (Superseded)

**This document has been superseded by [`rust-spec.md`](rust-spec.md).**

It was the discussion draft for the Rust implementation: an early architecture
sketch written largely from the documentation rather than from the source. Its
open questions have since been resolved against the actual code and the running
system, and its content — the inventory, the drift list, the operation model,
the protocol and lifecycle proposals, and the parity gate — now lives in the
specification, expanded and made normative.

Go to [`rust-spec.md`](rust-spec.md) for:

- the seventeen binding decisions, each with its justification;
- the mismatch register with stable `D#` and `N#` identifiers;
- the current system inventory and live-system evidence;
- the remaining open questions and what each one blocks;
- the first slice scope and the parity gate.

[`vroca.md`](vroca.md) remains the design of record for the current public
system. [`legacy-compatibility.md`](legacy-compatibility.md) remains the parity
checklist.
