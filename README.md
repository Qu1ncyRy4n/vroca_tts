# Vroca — Speech Synthesis & Assistive Reader

**Vroca** is a modular, zero-latency text-to-speech engine and assistive reading system supporting multi-engine ONNX synthesis, ASR word alignment, RSVP fixation overlays, and LLM agent spooling.

---

## Directory Layout

* `python_impl/` — Python 3 daemon, GTK4 overlay, and control panel GUI.
* `rust_impl/` — Parallel Rust implementation workspace directory.
* `docs/` — Architecture documentation (`vroca.md`) and pickup notes (`pickup_notes.md`).
* `flake.nix` — Nix flake environment.
