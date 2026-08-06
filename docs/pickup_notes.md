# Pickup Notes & Development Roadmap

## Current State

* **Location:** `~/dev/omnicortex/vroca_tts`
* **Active Python Stack:** `python_impl/` containing `daemon.py`, `overlay.py`, `panel.py`, `measure.py`, `voices.py`.
* **Rust Placeholder:** `rust_impl/` directory initialized for future parallel Rust development.

---

## Completed Features

1. **Markdown Text Normalizer:** Strips headings (`#`), bold/italic formatting, links, bullet points, and code blocks before sentence splitting so speech sounds like natural prose without symbol literal reads.
2. **Center Fixation RSVP Scroll (`scroll_rsvp`):** Fixed central column alignment for the active word's ORP character while text streams past.
3. **Speech Command Queueing:** `tts queue "text"`, `tts clear`, `tts skip` so background requests play sequentially without interrupting active audio tracks.
4. **Permissions & Spool:** Permissive `0o666` socket and file permissions for LLM agent integration via `/tmp/tts-speak`.
5. **Session Persistence & Reset Defaults:** All overlay and engine preferences persist across reboots in `~/.config/tts/prefs.json` with a Reset Defaults button in `tts-panel`.
6. **Dark Theme Selector:** High-contrast pitch black (`#121218`) background with white text (`#ffffff`) for voice picker dropdowns and listviews.

---

## Parallel Rust Implementation Roadmap (`rust_impl/`)

When ready to begin the parallel Rust implementation:
1. Initialize `Cargo.toml` workspace in `rust_impl/`.
2. Integrate `sherpa-onnx-rs` for ONNX model synthesis.
3. Use `libmpv-rs` for audio playback and position event pushing.
4. Use `gtk4-rs` + `gtk4-layer-shell-rs` for native overlay rendering.
5. Provide PyO3 bindings for Python interoperability.
