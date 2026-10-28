# Pickup Notes

Updated 2026-10-28 after implementing Nix packaging.
Repository: `~/dev/exocortex/vroca_tts`.

Read this first, then [installation.md](installation.md) for commands and
[roadmap.md](roadmap.md) for sequencing. [handoff.md](handoff.md) preserves the
earlier Python audit, fixes, and Rust review warnings.
[rust-spec.md](rust-spec.md) remains normative for Rust.

## Where Work Stopped

Nix packaging is implemented, verified on Apple Silicon, and committed in two
milestones:

- `5b161ba` — package, overlay, client fixes, approved model pin, and tests.
- `d2334c7` — opt-in Home Manager services, module tests, and installation docs.

The earlier planning commit is `e8c216c`. These commits are local on `main`;
no push or deployment was performed. Inspect `git status` before resuming.

Python remains the production path. Rust has domain, client/CLI, and fake
daemon/player code, but spec review and stage acceptance are pending. The Rust
panel and overlay binaries are empty stubs. The historical 19-test result has
not been rerun.

## What Was Implemented

| File or interface | Result |
| --- | --- |
| `nix/package.nix` | Conventional `pkgs.callPackage` package; models, Python dependencies, and wrappers remain repository-owned |
| Flake `default` / `vroca` | Combined command bundle; existing individual outputs retained |
| `overlays.default` | Exposes `pkgs.vroca` |
| `homeModules.default` | Opt-in `services.vroca.enable` and package override |
| Linux bundle/service | Daemon, CLI, panel, overlay; systemd user unit `tts` |
| Apple Silicon bundle/service | Daemon and CLI; GUI-session LaunchAgent `org.nix-community.home.vroca` |
| Client | Forwards all arguments (D5); waits for slow replies instead of exiting after half a second |
| Pitch measurement | References individual inputs so daemon/GUI edits do not repeat 904 voice measurements |

Package installation alone starts nothing. The Home Manager module starts only
the daemon; it does not register hotkeys or start the Linux GUIs.

The developer approved these interfaces:

- Keep the existing model set. Refresh the changed upstream Kokoro v1.0 pin;
  append `em_santa` at ID 53 while retaining IDs 0–52.
- On Mac, honor supplied `XDG_RUNTIME_DIR`; otherwise use
  `~/Library/Caches/vroca`, created privately by the daemon.
- Restart after failure; `tts quit` exits cleanly and stays stopped.
- On Mac, `tts log` follows `~/Library/Logs/vroca.log`.

The model pin review and declarative installation examples are in
[installation.md](installation.md). `flake.lock` was not changed.

## Validation Recorded In This Session

| Check | Result |
| --- | --- |
| Apple Silicon default bundle build | Passed, including models and pitch tables: 10/10 Supertonic and 904/904 LibriTTS voices |
| `nix flake check --no-write-lock-file` on Mac | Passed; seven packaged-client tests |
| Native `tests/test_packaged_daemon.py` against the built bundle | Passed startup, catalogue/voice selection, speed, real synthesis with null audio, and shutdown cleanup |
| `nix flake check --no-write-lock-file --no-build --all-systems` | Passed evaluation for both Linux systems and Apple Silicon |
| `tests/home-manager.nix` with locally cached Home Manager/nixpkgs | Passed Linux, Mac, and disabled module cases |
| Overlay versus flake default | Same derivation on all three advertised systems |
| Nix formatting, local Markdown links, `git diff --check` | Passed |

Linux binaries were not built or run here. Actual Mac audio output, clipboard
access, duplicate startup/stale-socket behavior, and activated LaunchAgent
login/restart behavior remain unverified. No Rust tests ran.

## Reproduce The Main Checks

```sh
cd ~/dev/exocortex/vroca_tts
nix flake check --no-write-lock-file
nix flake check --no-write-lock-file --no-build --all-systems
nix build --no-write-lock-file
python3 tests/test_packaged_daemon.py ./result/bin
```

Run the final command in the logged-in Mac session. Even with null audio,
`mpv` initializes AppKit and aborts inside the build sandbox. The same test
passes outside that sandbox; do not relax global sandbox settings to run it.
It uses temporary preferences/runtime state and does not read the clipboard or
play audible sound. Home Manager evaluation commands are in the install guide.

## Next Work

1. Finish Mac checks with real audio and clipboard access. Obtain separate
   approval before changing `~/dev/nix-config`, activating the LaunchAgent,
   or changing active Hammerspoon configuration. Validate login start, crash
   restart, clean quit, duplicate startup, stale-socket recovery, and cleanup.
2. Review the Rust scaffold against the spec before substantial Mac UI work.
   Settle the first-slice scope violation and relevant open decisions, then
   continue in bounded API/CLI slices with Python rollback preserved.
3. Deliver the Mac UI and Hammerspoon configuration. UI is required, and
   Hammerspoon owns hotkeys. Controls-only versus subtitles/RSVP, the toolkit,
   and bindings remain open; no choice has been approved implicitly.

Decision involvement is **interface**: installation commands, service behavior,
API/CLI shape, and UI behavior require developer approval. Implementation
details are delegated. Global Nix configuration, user profiles, and active
services were not changed.
