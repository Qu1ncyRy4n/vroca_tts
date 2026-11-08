# Reuse Survey: todo_app_project, omnichan, ExocortexCent

Read-only survey by `agent-assistant`, 2026-11-07, of what the Rust rewrite can
copy from sibling repositories. Constraint applied: `rust-spec.md` forbids
async runtimes in core and specifies sync thread-per-task design.

## Fit Summary

| Repo | Stack | Runtime fit | Take |
| --- | --- | --- | --- |
| `~/dev/recreational/anon/omnichan` | Rust 2021, sync, std::thread + mpsc, ureq, rusqlite, lexopt, egui, libmpv2; tokio banned by policy | Excellent | mpv IPC client, dep budget test, migrations + backup, error/exit codes, GUI worker lane, logging, golden CLI tests |
| `~/dev/apps/todo_app_project` | Rust 2024, tokio + turso (async), clap, eframe, `buildRustPackage` flake | Poor for runtime; good for tooling | flake skeleton, workspace lint block, config/path discovery, CLI test harness, decision-record and protocol-doc conventions |
| `~/dev/exocortex/ExocortexCent` | Docs only | Neutral | Standards Vroca must meet: lints, check gate, `tools/` verbs, CLI contract |

None of the three has a Unix-socket server/accept loop, GTK code, or ONNX/FFI
bindings. Those are written from scratch.

## omnichan — Strongest Fit

- **mpv child + JSON IPC, sync**: `crates/omni-engine/src/player.rs`. Spawns
  `mpv --input-ipc-server=<sock> --idle=yes --focus-on=never`, deletes stale
  socket, polls connect 100 × 50 ms, one `UnixStream` per command with 6 s read
  timeout, matches `request_id`, retries 3× with 300/600 ms backoff on mpv's
  small accept backlog. Has get/set_property, pause, seek, volume, `loadfile
  replace|append|append-play`, playlist ops, and a `player_diagnostics()`
  doctor check. Uses `entries[0]`, `response["error"]`, `as u32`; rework for
  `indexing_slicing` and `as_conversions` lints.
- **mpv pitfalls**: `docs/research/mpv.md` (focus, keybinding, IPC).
- **Versioned migrations + pre-migration backup**:
  `crates/omni-store/src/schema.rs` (ordered `MIGRATIONS: &[(u32, &str)]`, one
  transaction each, refuses newer schema) and `src/lib.rs:47-73` (copies to
  `*.bak-<from>-<ts>` first, refuses to migrate if copy fails). Same shape fits
  JSON prefs for DEC-17: `version` field, ordered `fn(Value)->Value` steps,
  backup before write.
- **Std-only error type with exit codes**: `crates/omni-core/src/error.rs`.
  `ErrorKind::exit_code()`; no anyhow/thiserror. Fits `vroca-core`'s serde-only
  rule.
- **Logging without tracing**: `crates/omni-net/src/log.rs`. `Level`, `-v/-vv`,
  1000-entry ring buffer behind a `Mutex`, JSON output. Enables a "recent log"
  protocol query.
- **Sync GUI worker with waker**: `crates/omni-ui/src/worker.rs`.
  `Request`/`Reply` enums over `mpsc`, worker lanes on `std::thread`,
  `ReplySender` calls a `Waker` closure after each send. For GTK use
  `glib::idle_add` or `MainContext::channel` as the waker. App tests at
  `crates/omni-ui/src/app.rs:3028+` drive state with stub channels, no window.
- **Dependency budget test**: `crates/omni-core/tests/dep_budget.rs` counts
  `Cargo.lock` entries against `MAX_TRANSITIVE`/`MAX_DIRECT`; each raise is
  commented. Extend to assert no tokio. Replaces the manifest-grep test flagged
  in `rust-review.md` (DEC-2).
- **Golden CLI tests**: `crates/omni-cli/tests/golden_cli.rs`. Temp XDG dir,
  `OMNI_*` env overrides, `CARGO_BIN_EXE_omni`, exact output and exit code.
- **Workspace conventions**: `Cargo.toml` centralises every dep in
  `[workspace.dependencies]` with a justification comment; `[profile.release]
  lto = "thin", strip = true`.
- **Flake**: `flake.nix` `mkOmni package binary` builds one derivation per
  binary; `wrapProgram --prefix PATH` injects runtime tools (use for `mpv`).
  Do not copy its `shellHook` aliases (forbidden by ExocortexCent standards).
- **libmpv on macOS**: `crates/omni-gui/build.rs` falls back to Homebrew
  libmpv. Only relevant if Vroca ever embeds libmpv instead of a child process.

## todo_app_project — Tooling And Process

- **Flake**: `flake.nix`. `forAllSystems`, version from `[workspace.package]`
  via `builtins.fromTOML`, `lib.fileset.toSource` filtered source,
  `cargoBuildFlags = ["-p" ...]`, `doCheck = false`. Replace its `patchelf`
  GUI step with `wrapGAppsHook4` for GTK.
- **Lint block**: `Cargo.toml` `[workspace.lints.clippy]` denies panic, todo,
  unimplemented, unreachable, as_conversions, unwrap_used, expect_used,
  indexing_slicing. Members opt in with `[lints] workspace = true`.
- **Config/paths**: `crates/todo-store/src/config.rs`. `AppPaths::discover()`
  honours `TODO_CONFIG_HOME`/`TODO_DATA_HOME` then `dirs::*`; `AppConfig` has
  `#[serde(default)]` on every field. Model for `VROCA_CONFIG_HOME` and
  test overrides.
- **CLI harness**: `crates/todo-cli/tests/cli.rs`. `Harness` with `TempDir`
  config/data/work dirs, `Command::new(env!("CARGO_BIN_EXE_todo"))`, clears
  agent env vars, `ok_with` prints stderr on failure.
- **JSONL event log codec**: `crates/todo-core/src/log.rs`. Only if Vroca
  wants a history log.
- **Docs conventions**: `DI/DI-000N-*.md`, `DR/` with Date/Status/Decision/
  Intent/Source header; `docs/protocols/todo-ledger-event-vN.md` one file per
  protocol version (model for the structured socket protocol doc);
  `.agents/skills/rust-development/SKILL.md` ("don't add an async runtime
  unless needed").
- **Install script**: `dev` `install` case removes stale `cargo install`
  copies that shadow the Nix profile. Port into `tools/install`.

## ExocortexCent — Standards To Meet

- **Vroca charter**: `docs/charters/vroca_tts.md`. Names the CLI `tts`; update
  to `vroca` after the 2026-11-07 rename. Says Vroca provides the shared
  markdown-to-speech normalizer for Teacher_Bot, todo, and Seam.
- **`docs/03-standards.md`**: the 8-deny lint set; gate = `cargo fmt --check`,
  clippy `-D warnings`, `cargo test --workspace`; API before CLI; `--dry-run`
  before writes; typed errors; `--json` output; one capability per crate;
  shared code consumed as a flake input.
- **`docs/05-tools.md`**: one POSIX `sh` script per verb under `tools/`
  (`check` required; `test`, `build`, `fmt`, `run`, `install`), each with
  `set -eu` and `cd "$(git rev-parse --show-toplevel)"`. Forbids shellHook
  aliases, `./dev` dispatchers, justfile/Makefile, `writeShellScriptBin`.
  **Vroca has `scripts/` and no `tools/`; it does not conform yet.**
- **`docs/02-interfaces.md`**: `Arbitrate::Choice` carries `interrupt` and a
  mandatory `reason`; relevant to the interruptible speech bus (Track C), not
  v0.
- **`docs/research/2026-10-01_rust-ui-and-api-style-survey.md`** ~L45-70:
  recommends renderer-free interaction state, presenter module, explicit
  request/reply seam on worker lanes. Supports the omnichan `worker.rs`
  pattern for the GTK panel.

## Ranked Picks

1. mpv IPC client (omnichan `player.rs`), lint-adapted — stage 6.
2. Dependency budget test asserting no tokio — stage 3 acceptance.
3. Versioned migrations + backup for prefs — DEC-17.
4. Std-only error type with exit codes — `vroca-core`.
5. Sync worker + waker for the GTK panel — stage 8.
6. CLI test harness (todo `cli.rs` or omnichan `golden_cli.rs`) — stage 4.
7. Flake skeleton (todo) + `wrapProgram --prefix PATH` for mpv (omnichan).
8. `tools/` verb scripts per ExocortexCent `05-tools.md`.
9. Workspace lint block (todo `Cargo.toml`).

## sherpa-onnx Rust Binding

Researched 2026-11-08 for `rust-spec.md` §7.1.

| Crate | Status | Notes |
| --- | --- | --- |
| `sherpa-rs` (thewh1teagle) | **Deprecated** | 0.6.8, 2025-10-05. README points to the official crate. Forks (`lxxyx-sherpa-rs`, `chobits-sherpa-rs`) track newer libs; none is upstream. |
| `sherpa-onnx` (k2-fsa, official) | Active | 2.0.1 on 2026-10-27; 1.13.8 on 2026-09-11. Apache-2.0. Source: `sherpa-onnx/rust/` in the upstream repo; examples in `rust-api-examples/`. Deps: serde, serde_json, `sherpa-onnx-sys`. |

Facts that matter for Vroca:

- `sherpa-onnx-sys/build.rs` reads `SHERPA_ONNX_LIB_DIR`; when set, it links
  there and downloads nothing. Otherwise it downloads
  `sherpa-onnx-v<ver>-osx-arm64-{static,shared}-lib.tar.bz2` (or linux-x64 /
  linux-aarch64) from GitHub releases, or copies from
  `SHERPA_ONNX_ARCHIVE_DIR`. Features: `static` (default) or `shared`.
  Bindings are pregenerated; no bindgen or clang at build time.
- nixpkgs `sherpa-onnx` is 1.13.8 and ships `lib/libsherpa-onnx-c-api.dylib`
  and `libsherpa-onnx-cxx-api.dylib` (Darwin). The crate version must match
  the library version, so pin `sherpa-onnx = "=1.13.8"` with `shared` until
  nixpkgs moves, or add an overlay for 2.0.x.
- `OfflineTts` is `Send + Sync`. API: `create(&OfflineTtsConfig) -> Option<Self>`,
  `sample_rate()`, `num_speakers()`, `lang()`,
  `generate_with_config(&self, text, &GenerationConfig, Option<F>) -> Option<GeneratedAudio>`
  with `F: FnMut(&[f32], f32) -> bool + 'static`. The callback gets chunks and
  progress; returning `false` aborts. That is the streaming (R4) and
  cancellation (DEC-1 generation counter) hook the Python daemon lacks.
- Model configs exist for Kokoro, VITS (libritts/piper), Matcha, Supertonic,
  ZipVoice, Kitten, Pocket. All five Vroca engines are covered.
- Thread safety of concurrent `generate` on one instance is **not documented**
  (10-f still open). Wrapper crates elsewhere serialise the engine on one
  worker thread. Do the same until measured.
- Expected dependency budget impact: `sherpa-onnx` + `-sys` + build deps
  (ureq, tar, bzip2, zip) land in `Cargo.lock` even when `SHERPA_ONNX_LIB_DIR`
  skips the download. Raise `MAX_TRANSITIVE` deliberately in the same commit.
