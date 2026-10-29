# Vroca — Speech Synthesis & Assistive Reader

**Vroca** is a modular, zero-latency text-to-speech engine and assistive reading system supporting multi-engine ONNX synthesis, ASR word alignment, RSVP fixation overlays, and local agent input.

---

## Directory Layout

* `python_impl/` — Python 3 daemon, GTK4 overlay, and control panel GUI.
* `rust_impl/` — Staged Rust replacement workspace directory.
* `docs/` — Architecture documentation (`vroca.md`), the guide for programs calling Vroca (`integration.md`), the normative Rust specification (`rust-spec.md`), the migration parity checklist (`legacy-compatibility.md`), the roadmap (`roadmap.md`), the handoff (`handoff.md`), and pickup notes (`pickup_notes.md`).
* `scripts/` — Demos and tooling (`multivoice_demo.py`).
* `flake.nix` — Nix flake environment and deployable package outputs.
* `nix/` — Conventional package expression, overlay support, and opt-in Home Manager service module.

## Nix Installation

Build the command bundle from the local checkout:

```sh
cd ~/dev/exocortex/vroca_tts
nix build
./result/bin/tts-daemon
```

In another terminal, run `./result/bin/tts say "Hello from Vroca."`.
For declarative installation and login startup, see
[`docs/installation.md`](docs/installation.md). The package supports Linux and
Apple Silicon macOS. Run `./result/bin/tts-panel` for controls, or on Mac
`open ./result/Applications/Vroca.app`. See the install guide for the packaged
Hammerspoon module and optional hotkeys.

## Current Priorities

The agreed order is easy Nix installation, working Apple Silicon macOS support,
then the staged Rust replacement. Mac support includes a UI and Hammerspoon
configuration for hotkeys. The approved first Mac GUI ports the existing GTK
controls/status panel; subtitles and RSVP remain Linux-only.

The flake exports the combined `vroca` package, individual Python packages and
models, an overlay, and an opt-in Home Manager service module.
Python remains the production implementation; the Rust scaffold needs review
before its migration stages can be accepted as complete.

Start with [`docs/pickup_notes.md`](docs/pickup_notes.md) for current status and
[`docs/roadmap.md`](docs/roadmap.md) for the work sequence. The Mac plan is in
[`docs/macos-compatibility.md`](docs/macos-compatibility.md).
