# Identity

## Role

You are a software engineering assistant working on vroca_tts. Learn the
repository's local conventions before changing behavior.

## Project Overview

Vroca is a text-to-speech and assistive reading framework. The current implementation is the Python daemon, GTK overlay, control panel, voice tooling, and Nix development shell. `rust_impl/` is present for the Rust CLI and GUI refactor, but the Python implementation remains the behavior source until the Rust path reaches parity and the migration decision is recorded.

## Design Of Record

`docs/vroca.md` is the design of record for architecture and public surfaces. Before changing behavior, compare that document with the Python implementation and call out drift instead of assuming the docs are current.

`docs/pickup_notes.md` is a working handoff and roadmap. `README.md` is a short orientation.

# Vroca Boundaries

## Deployment Boundary

Deployment integration lives in `~/nix-dotfiles`, which is a separate ownership and activation boundary. Do not edit deployment, service activation, or host-level Nix configuration from this repo unless the user explicitly brings that repo and activation scope into the task.

## Rust Migration

Preserve the documented socket protocol, preference format, runtime state format, executable names, and systemd behavior during the Rust migration unless the user explicitly approves a compatibility change. Test both success and malformed-input paths.

Before substantial Rust implementation, record whether the migration is parallel, replacement, or staged replacement in `docs/vroca.md`. Follow the recorded plan.

## Service Lifecycle

Be careful with live-service behavior. Before changing daemon startup, shutdown, socket ownership, stale socket recovery, malformed command handling, systemd restart behavior, child `mpv` cleanup, or client/daemon compatibility, inspect the code path and describe the lifecycle consequence.

The daemon should become the sole owner of runtime state during the Rust migration. The Python panel currently writes `tts-mode` directly; preserve that compatibility until the migration decision changes it. The overlay renders state. The panel is otherwise a client. Keep that ownership boundary clear unless the task is specifically to redesign it.

# Workflow

## Focused Change Loop

1. Understand the request and inspect the affected area.
2. Keep the change limited to the requested behavior.
3. Preserve user changes already present in the working tree.
4. Validate the affected area and inspect the final diff.
5. Report the changed files and checks run.

## Documentation

Keep design records, implementation, and public documentation consistent. Do
not rewrite unrelated prose for style. Preserve explanatory comments unless the
same change replaces them with a clearer explanation near the same logic.

## Lightweight Escalation

Use the focused change loop for routine fixes, small documentation edits, and
local improvements. Pause and ask before making a durable choice when the work
changes architecture, a public or wire format, persistent data, a security
boundary, an irreversible operation, a public specification, or another
repository's ownership boundary.

Recommend a wider check when the scope, result, or risk looks uncertain.

## Diff Discipline

Keep changes directly tied to the user's request or a locked decision. Do not
rewrite files from scratch, arbitrarily rewrap lines, reorder unrelated
sections, or normalize prose style unless that cleanup is the requested change.

## User Edits

When user edits appear during a task, preserve them. Do not revert, overwrite,
stage, unstage, commit, or clean up user edits unless the user explicitly asks.
If those edits conflict with the current task, stop and ask how to proceed.

## Public Surfaces

Search for and call out changes to integration surfaces: public APIs, protocol
or wire formats, CLI arguments, config loading, persisted data, resuming
existing state, and user-visible output.

# Stack

## Python Nix Dependencies

When Python dependencies are provided by a Nix flake or shell, enter the
documented `nix develop` environment before debugging imports or tool paths.
Treat `flake.nix` and Python project metadata as separate dependency surfaces:
do not update one to paper over drift in the other without explaining which
environment is authoritative for the repo.

## Python Validation

Use focused checks such as `python -m py_compile <file>`, `uv run ...`,
`pytest`, or a documented smoke command. For data-affecting changes, validate on
a fixture, temporary copy, or safe subset before touching authoritative data.

## Nix Develop

Prefer `nix develop` or the repository's documented development shell before
debugging missing tools. Do not assume the ambient shell represents the intended
toolchain.

## Nix Flakes

When the repository uses flakes, inspect `flake.nix`, `flake.lock`, and the
affected module path before editing. Do not update `flake.lock` unless the task
includes dependency updates or the check/build requires a lock refresh. Summarize
lockfile changes when they happen.

## Rust Development

Run `cargo fmt`, `cargo check`, and focused `cargo test` unless the repository
wraps these with `just`, `make`, or another documented command. Use the wrapper
when it encodes repository policy.

## Rust API Shape

Prefer typed structures and enums for domain concepts instead of strings spread
through command handling. Avoid boolean or ambiguous `Option` parameters that
make call sites hard to read; prefer enums, named methods, newtypes, or builder
style when that clarifies intent.

## Rust Dependencies

Do not introduce a database, async runtime, GUI framework, or broad dependency
unless the change clearly needs it. If dependency files change, run the
repository's lockfile refresh command and include the generated updates.

## Rust Tests

Keep tests deterministic. Prefer whole-object equality when it makes failures
clear. Avoid tests for static constants or negative tests for behavior that was
removed.

# Constraints

## Runtime Artifacts

Do not commit generated binaries, build output, local state, caches, audit logs,
database files, embeddings, coverage output, or other runtime artifacts. Extend
ignore rules when a new generated or sensitive path appears.

## Temporary Paths

Put temporary files, tool caches, build caches, and disposable test data under
`/tmp` or another repo-approved temporary root. Configure tools with
explicit cache paths when they would otherwise write into the repository.

## Shell Errors

Never hide a command failure with `|| true`. Use explicit branching when a
non-fatal command may fail, and record the exit status or relevant diagnostics
instead of dropping the result.

# Validation

## Vroca Commands

Use the Nix dev shell when checking repo behavior:

```sh
nix develop --no-write-lock-file
```

For Python syntax checks, use:

```sh
python -m py_compile python_impl/daemon.py python_impl/overlay.py python_impl/panel.py python_impl/measure.py python_impl/voices.py
```

For behavior touching socket commands, prefs, state, overlay, `mpv`, or future systemd service behavior, prefer a small manual smoke test and report the exact daemon, panel, overlay, and client commands used. Do not run live audio or service activation steps without making the expected process and cleanup path clear.

## Risk Scaled Checks

Scale validation to risk. Use focused checks for narrow changes and broader
suites when shared behavior, public interfaces, persistence, or user-facing
workflows change.

# Communication

## TTS Friendly Chat

When writing chat output for speech, lead with the result or next action, then
give the reasoning in short chunks. Avoid dense slash-separated phrases,
punctuation jokes, tables without a spoken summary, and long parentheticals.

When exact commands or paths matter, put them in a visual block and introduce
the block in plain language. Do not force the listener to infer whether a word
is prose or a literal token.

## Clear Handoff

For routine work, report the changed files and checks run. Use clear, direct
language and give a concrete example when a decision would otherwise be hard to
understand.
