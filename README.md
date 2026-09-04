# Vroca — Speech Synthesis & Assistive Reader

**Vroca** is a modular, zero-latency text-to-speech engine and assistive reading system supporting multi-engine ONNX synthesis, ASR word alignment, RSVP fixation overlays, and local agent input.

---

## Directory Layout

* `python_impl/` — Python 3 daemon, GTK4 overlay, and control panel GUI.
* `rust_impl/` — Staged Rust replacement workspace directory.
* `docs/` — Architecture documentation (`vroca.md`), the guide for programs calling Vroca (`integration.md`), the normative Rust specification (`rust-spec.md`), the migration parity checklist (`legacy-compatibility.md`), the roadmap (`roadmap.md`), the handoff (`handoff.md`), and pickup notes (`pickup_notes.md`).
* `scripts/` — Demos and tooling (`multivoice_demo.py`).
* `flake.nix` — Nix flake environment and deployable package outputs.
