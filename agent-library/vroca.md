# Vroca

## Project Overview

Vroca is a text-to-speech and assistive reading framework. The current implementation is the Python daemon, GTK overlay, control panel, voice tooling, and Nix development shell. `rust_impl/` is present for the Rust CLI and GUI refactor, but the Python implementation remains the behavior source until the Rust path reaches parity and the migration decision is recorded.

## Design Of Record

`docs/vroca.md` is the design of record for architecture and public surfaces. Before changing behavior, compare that document with the Python implementation and call out drift instead of assuming the docs are current.

`docs/pickup_notes.md` is a working handoff and roadmap. `README.md` is a short orientation.

## Deployment Boundary

Deployment integration lives in `~/nix-dotfiles`, which is a separate ownership and activation boundary. Do not edit deployment, service activation, or host-level Nix configuration from this repo unless the user explicitly brings that repo and activation scope into the task.

## Rust Migration

Preserve the documented socket protocol, preference format, runtime state format, executable names, and systemd behavior during the Rust migration unless the user explicitly approves a compatibility change. Test both success and malformed-input paths.

Before substantial Rust implementation, record whether the migration is parallel, replacement, or staged replacement in `docs/vroca.md`. Follow the recorded plan.

## Service Lifecycle

Be careful with live-service behavior. Before changing daemon startup, shutdown, socket ownership, stale socket recovery, malformed command handling, systemd restart behavior, child `mpv` cleanup, or client/daemon compatibility, inspect the code path and describe the lifecycle consequence.

The daemon should become the sole owner of runtime state during the Rust migration. The Python panel currently writes `tts-mode` directly; preserve that compatibility until the migration decision changes it. The overlay renders state. The panel is otherwise a client. Keep that ownership boundary clear unless the task is specifically to redesign it.

## Validation

Use the Nix dev shell when checking repo behavior:

```sh
nix develop --no-write-lock-file
```

For Python syntax checks, use:

```sh
python -m py_compile python_impl/daemon.py python_impl/overlay.py python_impl/panel.py python_impl/measure.py python_impl/voices.py
```

For behavior touching socket commands, prefs, state, overlay, `mpv`, or future systemd service behavior, prefer a small manual smoke test and report the exact daemon, panel, overlay, and client commands used. Do not run live audio or service activation steps without making the expected process and cleanup path clear.
