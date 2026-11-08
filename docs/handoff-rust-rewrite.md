# Handoff: The Rust Rewrite

Written 2026-11-08 by `agent-assistant` for the agent and machine that will
carry the Rust rewrite forward. Repository: `~/dev/exocortex/vroca_tts`.
Read this, then [`rust-spec.md`](rust-spec.md) in full, then
[`rust-review.md`](rust-review.md). [`pickup_notes.md`](pickup_notes.md) covers
the Python/Nix/Mac side and is not required for Rust work.

## 1. What Vroca Is, In One Paragraph

A local text-to-speech daemon and assistive reader. A daemon owns a queue,
synthesises sentences with sherpa-onnx engines (Kokoro, Supertonic, LibriTTS
VITS, ZipVoice) or an HTTP engine, and plays them through an `mpv` child
process. Clients talk to it over a Unix socket with one plaintext command per
connection. A panel configures it; an overlay shows subtitles or RSVP; hotkeys
and local agents call the CLI. Python is the production implementation. Rust is
replacing it, slice by slice, with Python rollback preserved until parity.

## 2. State On 2026-11-08

| Area | State |
| --- | --- |
| Python | Production. Bug sweep complete. Public command renamed `tts` → `vroca` (commit `0a16ff6`), aliases kept. |
| Nix | Package, overlay, Home Manager module. Mac bundle builds and checks pass. Not deployed; `~/dev/nix-config` still references `tts`. |
| Rust | Workspace with `vroca-core`, `vroca-client`, `vroca-daemon` crates and `vroca`, `vroca-daemon`, `vroca-panel`, `vroca-overlay` bins (last three: panel/overlay are empty stubs). 23 tests pass. Clippy clean under `-D warnings` with the strict lint set. |
| Rust review | Done ([`rust-review.md`](rust-review.md)): 17 decisions graded, 17 bugs, parser gaps, 6 ranked slices. |
| Blocking decision | **Decision 18** (first-slice scope) is Proposed, not Decided. |

Recent commits, newest first:

```
0a16ff6 Rename the public command from tts to vroca
b7485ca Adopt workspace lints, a dependency budget, exit codes, and tools/ verbs
49542b5 Rename Rust binaries from tts to vroca
```

## 3. Decisions Made This Week (Developer-Approved)

- **egui/eframe for the Rust GUI**, not GTK (`rust-spec.md` §7.10). `gtk4` is
  a forbidden dependency. Panel and overlay stay separate processes. The
  Wayland overlay has no layer-shell path through egui; options are recorded
  in §7.10 and the choice is open until slice 8 touches the overlay.
- **Public command is `vroca`.** Rust bins follow. The daemon binary package is
  `vroca-daemon-bin` with `[[bin]] name = "vroca-daemon"` to avoid colliding
  with the library crate.
- **Borrowed patterns** are in (see
  [`research/2026-11-07-sibling-repo-reuse.md`](research/2026-11-07-sibling-repo-reuse.md)):
  ExocortexCent lint set, centralised `[workspace.dependencies]` with reasons,
  `crates/vroca-core/tests/dep_budget.rs`, `ErrorCode::exit_code()`, and
  `tools/` verbs. Queued, not yet ported: omnichan's mpv IPC client (slice 6),
  migrations-with-backup (DEC-17), the GUI worker lane (slice 8), golden CLI
  tests (slice 4).

## 4. Decisions Waiting On The Developer

In order of how soon they block:

1. **Decision 18, first-slice scope.** Section 8 forbade sockets, `mpv`, and
   systemd in the first slice; the code that landed has all three. The
   proposal in `rust-spec.md` accepts the overreach with per-crate acceptance
   gates and promotes the structured codec to a bound, versioned socket
   (`"v": 1` required). Alternative is pulling the socket code back out. One
   sentence from the developer settles it. **Nothing in slices 3 to 5 can be
   accepted until then**, though the work itself can proceed.
2. **§7.1 engine binding.** Research done: use the official `sherpa-onnx`
   crate (k2-fsa), `shared` feature, `SHERPA_ONNX_LIB_DIR` pointed at the
   nixpkgs library, pinned to the nixpkgs version (1.13.8). Needs a yes.
3. **§7.2** start-failure message, **§7.3** socket permissions, **§7.8**
   request size cap, **`quit` exit status.** Each is a one-line answer; the
   recommendations are in the spec. Block slice 5 acceptance.
4. **Overlay on Wayland** (§7.10 options a/b/c). Blocks the overlay half of
   slice 8 only.

Decision involvement level is **interface**: API/CLI shape, service behavior,
and UI behavior need approval before implementation. Function names, control
flow, and module layout do not. Commit mode is **commit only when asked**
unless the developer authorises goal-driven atomic commits for a slice.

## 5. The Work, In Order

Each slice is API first, then tests, then CLI if it has one. Finish a slice
and run `tools/check` before starting the next. Cite spec identifiers
(`DEC-n`, `D-n`, `N-n`, `§x.y`) in test names and commit messages.

### Slice A — `vroca-core` parser and codec hardening (no decisions needed)

From `rust-review.md` §4 and §6. All inside `vroca-core`, serde only.

- Trailing whitespace and newline tolerance in `parser.rs` (bug 1); reject
  trailing arguments on no-argument verbs (bug 2, DEC-13).
- Add verbs: `mode_set`, `volume`, `trim`, `voice <name|engine:name>`,
  `preview <name>`, `--voice TOKEN` prefix on `say/speak/queue`,
  `reset <group>`, aligner enum (`asr|energy`).
- Python-shaped error replies: `unknown: <full command>` and the other rows of
  [`legacy-compatibility.md`](legacy-compatibility.md).
- Replace inline asserts with a fixtures table, one row per
  `legacy-compatibility.md` entry, each citing its D/N identifier.
- Codec negative tests: malformed JSON, unknown operation, wrong field type.
  Unknown policy value maps to `invalid_argument` (bug 17).
- `#![forbid(unsafe_code)]` in `vroca-core` and `vroca-daemon`.
- Make `EngineId`'s inner field private (bug 11).

### Slice B — Schema and state machine (DEC-17, §10.9)

- `Preferences` must load the live Python file: `"voice": 9` integer form,
  missing `overlay_mode` and `schema` (bug 12). Write a v0→v1 migration as a
  pure `fn(Value) -> Result<Value, _>` step in an ordered list; the daemon does
  the I/O, including the `.bad` quarantine and a pre-migration backup
  (omnichan `omni-store` pattern).
- Unknown-field preservation on `RuntimeSnapshot` too.
- `State::apply` returns `Result<Reply, ProtocolError>` so `Status`,
  `Catalogue`, `Preview`, `Quit` stop lying with `Ok` (bug 5).
- Populate `sentences` on `Speak` (port the Python sentence splitter), bound
  `Next`/`Back`/`Skip{Sentence}` (bug 3), `Toggle` while idle returns `idle`
  (bug 4), preview restore point (DEC-14).
- Add `channel: ChannelId` to `Operation` with a single default channel, a
  `(ModelId, SpeakerIndex)` resolver type, and a channel-keyed queue map.
  §10.9 requires these shapes in the first slice even though scheduling is
  Track C.

### Slice C — Daemon transport (needs Decision 18 and §7.3/§7.8)

- Framed legacy reads: read to quiet gap or EOF with the §7.8 cap; over-limit
  is an error, never truncation (bug 7; Python N6 semantics).
- Clean up only paths this daemon created (bug 6, DEC-9 rule 1). Fix the
  cleanup test that passes because of over-deletion.
- Real `Message::Shutdown`; blocking accept with shutdown by closing the
  listener; stop ignoring `WouldBlock` vs real errors (bug 9); fix the
  check-then-bind race (bug 10).
- `Player` trait with the fake impl; health enum with a degraded state on
  player death (N4); snapshot writer with `daemon_id` on every state change
  (DEC-8).
- `"v": 1` on the structured socket, rejection test (Decision 18 item 4).

### Slice D — Client and CLI (needs §7.2)

- Local validation in `vroca-client` (speed range, enum values) per DEC-5.
- Typed per-operation methods returning typed results, not raw `Response`.
- `unavailable` message names the start command once §7.2 is decided.
- Drop the `/tmp` fallback for `XDG_RUNTIME_DIR` (bug 13; see
  [`agent-input-security.md`](agent-input-security.md)).
- Golden CLI tests with a temp runtime dir and `CARGO_BIN_EXE_vroca`
  (todo_app_project `crates/todo-cli/tests/cli.rs` pattern).

### Slice E — First real engine (needs §7.1 approval)

- New crate `vroca-engine` wrapping the official `sherpa-onnx` crate. Raise
  `MAX_TRANSITIVE` in `dep_budget.rs` in the same commit, with the measured
  number.
- Nix: `SHERPA_ONNX_LIB_DIR = "${pkgs.sherpa-onnx}/lib"` in the dev shell and
  the Rust package. Pin the crate version to the nixpkgs library version.
- One engine (Kokoro) behind one worker thread. Use the
  `generate_with_config` callback for cancellation via the generation counter.
- **Spike 10-f first:** call `generate` concurrently on one `OfflineTts` from
  two threads with the same model and record whether output is correct and
  whether it crashes. Write the result into `rust-spec.md` §10.8.
- Port omnichan's mpv IPC client (`crates/omni-engine/src/player.rs`) as the
  real `Player`, adapted to the lint set.

### Slice F onward

Remaining engines and alignment (slice 7), egui panel through the shared
client with a worker lane (slice 8), parity tests and dogfooding (slice 9).
The overlay waits on the §7.10 Wayland decision.

## 6. Setting Up On A New Machine

Prerequisites: Nix with flakes, Git, a checkout of this repository. The flake
dev shell provides the Rust toolchain, Python, mpv, socat, and sherpa-onnx.

```sh
git clone <origin> ~/dev/exocortex/vroca_tts
cd ~/dev/exocortex/vroca_tts
nix develop --no-write-lock-file        # or direnv allow
tools/check                              # fmt, clippy -D warnings, 23 tests, py_compile
```

Expected: `tools/check` exits 0. If `cargo` is missing you are outside the
dev shell. Linux is fine for all Rust slices through E; only the Mac bundle
checks need Apple Silicon.

Other verbs: `tools/test`, `tools/build`, `tools/fmt`, `tools/run -- status`,
`tools/check-nix` (slow; evaluates every system then builds this host's
checks).

## 7. Conventions That Will Bite You

- `vroca-core` must resolve to serde and serde_json only. The dependency
  budget test fails the build otherwise.
- No tokio, async-std, futures, gtk4, anyhow, tracing. The test names each
  with its reason; remove an entry only in the commit that justifies it.
- Clippy denies `unwrap`, `expect`, `panic`, `indexing_slicing`,
  `as_conversions`. Test modules carry a local `#[allow]` for the first three.
- Every third-party crate goes in `[workspace.dependencies]` with a comment
  saying why; members use `name.workspace = true`.
- Spec identifiers: `DEC-n` is a decision, `D-n` is a documentation mismatch
  finding, `N-n` is a new finding. They are not interchangeable.
- The spec warns against code that resolves an open question implicitly. If a
  slice needs an answer, ask; do not pick one and move on.
- Deployment lives in `~/dev/nix-config`, a separate activation boundary. Do
  not edit it without explicit approval.
- No commit trailers or agent signatures. Decision notes use
  `agent-assistant`.

## 8. Initialization Prompt For The Next Agent

Paste the block below as the first message in a fresh session on the new
machine.

```text
You are picking up the Rust rewrite of Vroca, a local text-to-speech daemon,
in ~/dev/exocortex/vroca_tts. Read, in order: AGENTS.md,
docs/handoff-rust-rewrite.md, docs/rust-spec.md (all of it; it is normative),
docs/rust-review.md, docs/research/2026-11-07-sibling-repo-reuse.md. Then run
`nix develop --no-write-lock-file --command tools/check` and confirm it passes
before touching code.

Context you need:
- Python is production; Rust replaces it slice by slice with rollback kept.
- The public command is `vroca`. Rust bins are vroca, vroca-daemon,
  vroca-panel, vroca-overlay (last two are stubs).
- GUI toolkit is egui/eframe (decided). No tokio, no gtk4, no anyhow.
- vroca-core is serde-only; a dependency budget test enforces it.
- Decision 18 in rust-spec.md is Proposed, not Decided. Slices can proceed
  but nothing is accepted until I approve it. Ask me for that first.
- Decision involvement is "interface": show me API/CLI shape and behavior
  before implementing; function-level choices are yours.
- Commit only when asked. No agent signatures or trailers.

Your first task is Slice A from docs/handoff-rust-rewrite.md §5 (vroca-core
parser and codec hardening). It needs no decisions from me. Start by
presenting the fixture table format and the list of parser changes, then
implement, then run tools/check, then report. After that, ask me to resolve
Decision 18 and §7.1 before Slice C or E.

Report at the end of each slice: what changed, what tools/check ran, what
remains, and any open question that blocks the next slice.
```
