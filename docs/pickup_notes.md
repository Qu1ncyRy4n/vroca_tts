# Pickup Notes & Development Roadmap

## Current State

* **Location:** `~/dev/omnicortex/vroca_tts`
* **Active Python Stack:** `python_impl/` containing `daemon.py`, `overlay.py`, `panel.py`, `measure.py`, `voices.py`.
* **Rust Placeholder:** `rust_impl/` is reserved for a staged Rust replacement. Python remains usable until the Rust parity gate passes.

---

## Completed Features

1. **Markdown Text Normalizer:** Strips headings (`#`), bold/italic formatting, links, bullet points, and code blocks before sentence splitting so speech sounds like natural prose without symbol literal reads.
2. **Center Fixation RSVP Scroll (`scroll_rsvp`):** Fixed central column alignment for the active word's ORP character while text streams past.
3. **Speech Command Queueing:** `tts queue "text"`, `tts clear`, `tts skip` so background requests play sequentially without interrupting active audio tracks.
4. **Local Agent Input:** Local agents and scripts can submit speech through the `tts` CLI or UNIX socket. The socket is created with mode `0o666` inside the per-user runtime directory.
5. **Session Persistence & Reset Defaults:** All overlay and engine preferences persist across reboots in `~/.config/tts/prefs.json` with a Reset Defaults button in `tts-panel`.
6. **Dark Theme Selector:** High-contrast pitch black (`#121218`) background with white text (`#ffffff`) for voice picker dropdowns and listviews.

---

## Staged Rust Replacement Roadmap (`rust_impl/`)

When ready to begin the staged Rust implementation:
1. Resolve the open decisions in `docs/rust-design.md`.
2. Initialize the reviewed Cargo workspace in `rust_impl/`.
3. Implement typed operations, protocol compatibility, and a local client.
4. Reach daemon, CLI, overlay, and panel parity on Linux.
5. Switch Nix deployment only after the parity gate passes.
6. Retire Python in a later reviewable change.
