# Pickup Notes

Updated 2026-10-28. [`roadmap.md`](roadmap.md) owns sequencing;
[`rust-spec.md`](rust-spec.md) owns Rust behavior and migration decisions.

## Current State

* **Location:** `~/dev/exocortex/vroca_tts`
* **Active Python Stack:** `python_impl/` containing `daemon.py`, `overlay.py`, `panel.py`, `measure.py`, `voices.py`.
* **Rust Scaffold:** `rust_impl/` contains the workspace, domain code, client,
  CLI, and fake daemon/player work. It is unreviewed against the specification;
  panel and overlay binaries are empty stubs. Python remains usable until the
  Rust parity gate passes.
* **Nix Packaging:** Individual packages and models already live in
  `flake.nix`, along with the Rust toolchain. Easy installation and reusable
  service configuration remain to be built.
* **macOS:** Apple Silicon is the target. `pbpaste` support exists, but package
  evaluation, local audio, and service lifecycle have not been verified in this
  pickup. The CLI still unconditionally depends on `systemd`.

---

## Completed Features

1. **Markdown Text Normalizer:** Strips headings (`#`), bold/italic formatting, links, bullet points, and code blocks before sentence splitting so speech sounds like natural prose without symbol literal reads.
2. **Center Fixation RSVP Scroll (`scroll_rsvp`):** Fixed central column alignment for the active word's ORP character while text streams past.
3. **Speech Command Queueing:** `tts queue "text"`, `tts clear`, `tts skip` so background requests play sequentially without interrupting active audio tracks. `queue` always appends, including while paused (D4 fixed 2026-09-01).
4. **Local Agent Input:** Local agents and scripts can submit speech through the `tts` CLI or UNIX socket. The socket is created with mode `0o666` inside the per-user runtime directory.
5. **Session Persistence & Reset Defaults:** All overlay and engine preferences persist across reboots in `~/.config/tts/prefs.json` with a Reset Defaults button in `tts-panel`.
6. **Dark Theme Selector:** High-contrast pitch black (`#121218`) background with white text (`#ffffff`) for voice picker dropdowns and listviews.

---

## Handoff

[`handoff.md`](handoff.md) holds the document map, historical validation, and
the traps that cost real time. Use [`roadmap.md`](roadmap.md) for current work
order.

## Next Work — Agreed 2026-10-28

1. Make Nix installation straightforward: propose a combined package,
   install/run commands, and an opt-in Home Manager service interface.
2. Make the Mac daemon and CLI work: remove Linux-only dependencies, define a
   shared private runtime directory, decide logging and shutdown behavior, and
   validate on Apple Silicon before deployment.
3. Review the Rust scaffold against the spec before substantial new Mac UI
   work. Reconcile scope violations and unresolved decisions, then continue in
   bounded slices. Keep package and CLI interfaces stable through replacement.
4. Deliver the Mac UI and Hammerspoon hotkey configuration. Controls only,
   subtitles, and RSVP scope remain unresolved; do not choose a toolkit yet.

The developer approves interface choices: installation commands, service
behavior, API/CLI shape, and UI behavior. Implementation details are delegated.
Commit only when asked. `~/dev/nix-config` remains a separately approved
deployment boundary. See the roadmap for validation gates and deferred work.
