# Vroca

## Project Overview

Vroca is a text-to-speech and assistive reading framework. The current implementation is the Python daemon, GTK overlay, control panel, voice tooling, and Nix development shell. `rust_impl/` is present for the Rust daemon, CLI, and GUI replacement, but the Python implementation remains the behavior source until the Rust path reaches parity and the migration decision is recorded.

## Design Of Record

`docs/vroca.md` is the design of record for architecture and public surfaces. Before changing behavior, compare that document with the Python implementation and call out drift instead of assuming the docs are current.

`docs/pickup_notes.md` is a working handoff and roadmap. `README.md` is a short orientation.

## Deployment Boundary

Deployment integration lives in `~/nix-dotfiles`, which is a separate ownership and activation boundary. `home/tts.nix` packages Vroca and defines the user services. `home/cosmic.nix` defines Vroca hotkeys. `flake.nix` owns the path input. Do not edit deployment, service activation, or host-level Nix configuration from this repo unless the user explicitly brings that repo and activation scope into the task.

## Rust Migration

Preserve the documented socket protocol, preference format, runtime state format, executable names, and systemd behavior during the Rust migration unless the user explicitly approves a compatibility change. Test both success and malformed-input paths.

Before substantial Rust implementation, record whether the migration is parallel, replacement, or staged replacement in `docs/vroca.md`. Follow the recorded plan.

## Service Lifecycle

Be careful with live-service behavior. Before changing daemon startup, shutdown, socket ownership, stale socket recovery, malformed command handling, systemd restart behavior, child `mpv` cleanup, or client/daemon compatibility, inspect the code path and describe the lifecycle consequence.

The daemon should become the sole owner of runtime state during the Rust migration. The overlay renders state. The panel is otherwise a client. Keep that ownership boundary clear unless the task is specifically to redesign it.

## Validation

Use the Nix dev shell when checking repo behavior:

```sh
nix develop
```

Retain `flake.lock` when Nix creates or updates it, and report the lockfile
change in the handoff.

For Python syntax checks, use:

```sh
python -m py_compile python_impl/daemon.py python_impl/overlay.py python_impl/panel.py python_impl/measure.py python_impl/voices.py
```

For behavior touching socket commands, prefs, state, overlay, `mpv`, or future systemd service behavior, prefer a small manual smoke test and report the exact daemon, panel, overlay, and client commands used. Do not run live audio or service activation steps without making the expected process and cleanup path clear.

## Approval-Sensitive Commands

When sandboxing rejects a command that is necessary for the task, request
approval for the exact command family and state its target. Do not ask for a
universal build, service, or shell permission.

| Need | Command family | Notes |
| --- | --- | --- |
| Enter the project environment | `nix develop` | May contact the local Nix daemon and fetch cached dependencies. |
| Evaluate the flake | `nix flake check --no-build`, `nix flake metadata --json`, `nix eval` | Read-only evaluation. |
| Build a known output | `nix build --no-link <exact-attribute>` | Show the exact attribute first. Do not request a broad build rule. |
| Inspect the live service | `systemctl --user show tts.service`, `journalctl --user -u tts.service` | Read-only. |
| Query daemon state | `tts status` | Read-only socket request. |
| Change live playback or service state | `tts <mutation>`, `systemctl --user restart tts` | Describe the expected process and cleanup path first. Request approval for the exact action. |
| Change deployment | patch files under `~/nix-dotfiles` | Separate repository and activation boundary. Ask before writing or activating. |

Run Rust commands through the approved Nix shell once the Rust toolchain exists:

```sh
nix develop --command cargo fmt --check
nix develop --command cargo check
nix develop --command cargo test
```
