# Pickup Notes

Updated 2026-11-07 after the `tts` → `vroca` rename and the Rust scaffold review.
Repository: `~/dev/exocortex/vroca_tts`.

Rust work: read [handoff-rust-rewrite.md](handoff-rust-rewrite.md) instead;
it carries the init prompt for the rewrite machine.

Read this first, then [installation.md](installation.md) for commands and
[roadmap.md](roadmap.md) for sequencing. [handoff.md](handoff.md) preserves the
earlier Python audit, fixes, and Rust review warnings.
[rust-spec.md](rust-spec.md) remains normative for Rust;
[rust-review.md](rust-review.md) records the 2026-11-07 audit against it.

## Where Work Stopped

Nix packaging is implemented, verified on Apple Silicon, and committed in two
milestones:

- `5b161ba` — package, overlay, client fixes, approved model pin, and tests.
- `d2334c7` — opt-in Home Manager services, module tests, and installation docs.

The earlier planning commit is `e8c216c`. These commits are local on `main`;
no push or deployment was performed. Inspect `git status` before resuming.

The subsequent GUI work adds cross-platform `vroca-panel`,
`Applications/Vroca.app` in the Mac default bundle, an opt-in Hammerspoon Lua
module, asynchronous panel requests, native GTK and mocked Hammerspoon tests,
and aligned docs. `8cbfca4` records the earlier packaging handoff. No profile,
service, or active Hammerspoon configuration was changed.
The ownership fix is the separate atomic commit `22ae724`; the GUI milestone
follows it. Inspect `git log` for the final GUI commit.

The Mac app launched successfully and was left open for review. A pre-existing
foreground daemon (PID 26167 at discovery) was found during the startup check.
The old duplicate-start cleanup unlinked its socket on refusal, leaving that
process unreachable. `daemon.py` now refuses duplicates before model startup
and only cleans shared socket/state files after acquiring socket ownership.
The packaged regression test passes. The developer approved restarting that
process with the fixed build; the foreground daemon is running again and
`vroca status` reports Kokoro loaded, 54 voices, and a live player. The open panel
can reconnect. No login service was activated. Do not start an old build
against another live daemon.

`~/.hammerspoon/init.lua` is a Home Manager store symlink. Its source is
`~/dev/nix-config/modules/desktop/mac/core/hammerspoon/init.lua`, selected in
`modules/desktop/mac/core/home.nix`. That repository currently pins the remote
`vroca_tts` flake input. Persistent package installation, selecting this local
revision, and enabling the approved bindings require a separately approved
change there; do not replace the managed symlink with an ad hoc local file.

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
| Linux bundle/service | Daemon, CLI, panel, overlay; systemd user unit `vroca` |
| Apple Silicon bundle/service | Daemon, CLI, GTK controls/status panel, `Vroca.app`, Hammerspoon module; GUI-session LaunchAgent `org.nix-community.home.vroca` |
| Client | Forwards all arguments (D5); waits for slow replies instead of exiting after half a second |
| Pitch measurement | References individual inputs so daemon/GUI edits do not repeat 904 voice measurements |

Package installation alone starts nothing. The Home Manager module starts only
the daemon; it does not register hotkeys or start the Linux GUIs.

The developer approved these interfaces:

- Keep the existing model set. Refresh the changed upstream Kokoro v1.0 pin;
  append `em_santa` at ID 53 while retaining IDs 0–52.
- On Mac, honor supplied `XDG_RUNTIME_DIR`; otherwise use
  `~/Library/Caches/vroca`, created privately by the daemon.
- Restart after failure; `vroca quit` exits cleanly and stays stopped.
- On Mac, `vroca log` follows `~/Library/Logs/vroca.log`.
- First Mac GUI: existing GTK controls/status panel with voices, speed, volume,
  and playback; no subtitle/RSVP port. The panel reports an unavailable daemon
  without starting a Mac service. Closing it leaves the daemon running.

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
| Native GTK smoke test (2026-10-29) | Passed: voice filtering, command dispatch, read-only refresh, responsive delayed preview, Mac unavailable state |
| Hammerspoon Lua tests (2026-10-29) | Passed with mocked APIs: argument dispatch, task lifetime, key cleanup, and failures; included in Darwin flake checks |
| Packaged daemon after ownership fix (2026-10-29) | Passed duplicate-start refusal with original socket inode and live status preserved, synthesis, controls, and cleanup |
| Mac app launch (2026-10-29) | `open` succeeded; process remained running with an empty startup error log. Human focus/visual review remains pending |

Linux binaries were not built or run here. Actual Mac audio output, clipboard
access, concurrent startup/stale-socket behavior, and activated LaunchAgent
login/restart behavior remain unverified. No Rust tests ran.

## Reproduce The Main Checks

```sh
cd ~/dev/exocortex/vroca_tts
nix flake check --no-write-lock-file
nix flake check --no-write-lock-file --no-build --all-systems
nix build --no-write-lock-file
python3 tests/test_packaged_daemon.py ./result/bin
nix develop --command python3 tests/test_panel.py
```

Run both native tests in the logged-in Mac session. Even with null audio,
`mpv` initializes AppKit and aborts inside the build sandbox. The same test
passes outside that sandbox; do not relax global sandbox settings to run it.
It uses temporary preferences/runtime state and does not read the clipboard or
play audible sound. Home Manager evaluation commands are in the install guide.

## Next Work

1. Finish Mac checks with real audio and clipboard access. Obtain separate
   approval before changing `~/dev/nix-config`, activating the LaunchAgent,
   or changing active Hammerspoon configuration. Validate login start, crash
   restart, clean quit, duplicate startup, stale-socket recovery, and cleanup.
2. Review the Rust scaffold against the spec before further migration work.
   Settle the first-slice scope violation and relevant open decisions, then
   continue in bounded API/CLI slices with Python rollback preserved.
3. Manually verify the packaged panel and app launch/focus. The approved
   Hammerspoon bindings match Linux/Speechify with Option replacing Super.
   Approve the active module path/configuration before installing it. Loading
   the module binds nothing; `setup()` enables the approved defaults.
   Native GTK tests use a fake daemon and prove no real
   clipboard, audio, Finder, or global-key behavior.

Decision involvement is **interface**: installation commands, service behavior,
API/CLI shape, and UI behavior require developer approval. Implementation
details are delegated. Global Nix configuration, user profiles, and active
services were not changed.

## CLI Rename — 2026-11-07

The developer approved renaming the public command `tts` to `vroca` with full
scope: binaries, flake outputs, systemd unit, runtime files, config dir,
`VROCA_*` env vars, and the Rust bin crates. `tts*` symlink aliases ship in the
bundle; the daemon falls back to `~/.config/tts` and copies `TTS_*` env values
until `~/dev/nix-config` is updated. Migration table:
[installation.md](installation.md#migrating-from-tts). Flake output `vroca`
stays the bundle; the client output is `vroca-cli`.

Deployment still names `tts` in Hyprland/niri bindings, `tts-home.nix`, and the
`TTS_API_KEY` secret. Updating it is a separately approved change; the aliases
keep it working meanwhile.

## Rust Review And Borrowed Patterns — 2026-11-07

The developer chose **egui/eframe** for the Rust GUI (`rust-spec.md` §7.10);
`gtk4` is now a forbidden dependency. Borrowed from sibling repos
([research/2026-11-07-sibling-repo-reuse.md](research/2026-11-07-sibling-repo-reuse.md)):
workspace lint set and centralised `[workspace.dependencies]`, a dependency
budget test (`crates/vroca-core/tests/dep_budget.rs`, forbids tokio/gtk4/anyhow/
tracing and asserts `vroca-core` resolves to serde only), `ErrorCode::exit_code()`
used by the CLI, and `tools/{check,test,build,fmt,run,check-nix}` per
ExocortexCent `05-tools.md`. `tools/check` is the gate and passes (23 tests,
clippy `-D warnings`). The mpv IPC client and prefs migration pattern are
queued for stages 6 and DEC-17, after DEC-18.


`cargo test` passes 19 tests; fmt clean; clippy warnings only. The first slice
violates `rust-spec.md` §8 (sockets, fake player, journalctl present) and the
live Python `prefs.json` cannot be parsed. Seventeen concrete bugs and six
ranked slices are in [rust-review.md](rust-review.md). Next developer decision:
DEC-18 scope acceptance, then §7.2/§7.3/§7.8 and `quit` exit status.
