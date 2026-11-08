# Rust Scaffold Review — 2026-11-07

Read-only audit of `rust_impl/` against [`rust-spec.md`](rust-spec.md),
performed by `agent-assistant`. Paths are relative to `rust_impl/`. Binary
crates were renamed `tts*` → `vroca*` on the same day; this review cites the
new names.

## Validation

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Clean |
| `cargo test` | 19 passed, 0 failed (core 8, daemon 3, client 3, `vroca` bin 5) |
| `cargo clippy --all-targets` | No errors; ~13 warnings (clamp patterns `state.rs:132,150,153`, `new_without_default` `player.rs:9`, `large_enum_variant` on codec `Response`, unused `EngineId`/`Request`/`mpv_sock`/`state_tx`, dead `Message::Shutdown`) |

Matches the historical "19 tests, fmt clean" claim in `handoff.md` §6.

## Scope: Stage 3 / §8 Is Violated

§8 forbids sockets, mpv, audio, GTK, sherpa-onnx, and systemd in the first
slice. Present anyway:

- Sockets: `crates/vroca-daemon/src/lib.rs:6,61-67,96-166` binds `vroca.sock`
  and `vroca-v1.sock`; `crates/vroca-client/src/lib.rs:4,26` connects;
  `bins/vroca/src/main.rs:170` builds the socket path.
- Child process: `crates/vroca-daemon/src/player.rs:14` spawns `sleep 100` as a
  fake mpv.
- systemd: `bins/vroca/src/main.rs:150` shells out to `journalctl`.
- DEC-6 contradiction: the structured codec is bound to `vroca-v1.sock` with no
  version field, while the spec requires a test asserting no socket is bound.

GTK and sherpa are absent; `vroca-daemon`, `vroca-panel`, `vroca-overlay` bins
are `fn main() {}` stubs. Resolution needed: record a decision widening the
slice (recommended) or move socket/daemon/CLI code out of first-slice
acceptance.

## Decision Compliance

| DEC | Status | Evidence |
| --- | --- | --- |
| 1 Queue semantics | Partial | Enums `operations.rs:5-40`; D2/D3/D4 tests `tests.rs:128-158`. `Skip{Sentence}` unbounded `state.rs:91-94`; `Stop{Queue}` does not bump generation; no fixtures file; no channel id on `Operation` |
| 2 Crate layout | Implemented | 3 crates + 4 bins. `test_core_dependencies` `tests.rs:212-220` greps the manifest, not the resolved graph |
| 3 Two sockets | Ahead of slice | `lib.rs:50-67`; no permission handling; no cross-protocol misuse tests |
| 4 Concurrency | Partial | State-owner thread `lib.rs:69-92`; generation counter unused; accept loops busy-poll at 10 ms `lib.rs:96,129`; `Message::Shutdown` never sent |
| 5 Client API | Partial | Sync, `Send`, `unavailable` `vroca-client/src/lib.rs:26-29`. No local validation; only `speak`/`queue` helpers; raw `Response` returned; connect error text discarded |
| 6 Structured codec | Contradicted | Codec `codec.rs:25-40` is bound; one round-trip test; no malformed/unknown-op tests |
| 7 Overlay mode | Partial | `schema.rs:63`, cycle `state.rs:158-166`. No `mode_set`; no mode-file mirror |
| 8 State delivery | Partial | `RuntimeSnapshot` has `schema`/`daemon_id` `schema.rs:92-93`; nothing writes it; `Status` answers `Ok` |
| 9 Player | Partial | Concrete struct, no trait `player.rs:4-19`; `stop()` removes every `vroca-*` dir in the runtime dir `lib.rs:185-197`, violating DEC-9 rule 1 |
| 10 VoiceId | Partial | `voice <name>` rejected `parser.rs:60-65` though §4.1 requires it; `SetVoiceNumeric` no-op `state.rs:134-137` |
| 11 Name/traits split | Missing | No catalogue, identity, or traits types |
| 12 Classification | Missing | Offline tooling; not first-slice |
| 13 Context commands | Partial | No-arg verbs accept trailing args `parser.rs:44-53`; `Toggle` when idle flips `paused` `state.rs:102-104` |
| 14 Preview | Missing | Parsed; `state.rs:187` no-op |
| 15 Reset | Partial | `reset <group>` not parsed `parser.rs:95-97`; defaults duplicated as literals |
| 16 Names | Mostly | `EngineId` inner is `pub` `schema.rs:12`, bypassing validation; `Utterance` absent; `Request` unused |
| 17 Persistence | Partial | Unknown-field round-trip `tests.rs:173-209`. No migration, `.bad` quarantine, atomic writer, or clamp report. Live `prefs.json` (`"voice": 9`, no `overlay_mode`) fails to parse |

`#![forbid(unsafe_code)]` is absent from `vroca-core` and `vroca-daemon`.

## Legacy Parser Gaps vs `python_impl/daemon.py`

Missing: `mode_set`, `volume`, `trim`, `voice <name|engine:name>`,
`preview <name>`, `--voice TOKEN` prefix on `say/speak/queue`, `reset <group>`,
aligner enum validation. Request framing is a single 4096-byte `read()`
(`lib.rs:102-103`), reintroducing N6 truncation.

## Bugs

1. `parser.rs:7-17` keeps a trailing `\n`; `speed 1.5\n` from socat fails.
2. `parser.rs:44-53,94-108` accepts trailing garbage on no-arg verbs.
3. `state.rs:91-94,115-118` unbounded sentence index; `sentences` never populated.
4. `state.rs:102-104` `Toggle` while idle sets `paused`.
5. `state.rs:187` `Status/Catalogue/Preview/Unload/Reload/Quit` silently `Ok`.
6. `lib.rs:185-197` shutdown deletes other daemons' dirs; the cleanup test passes because of it.
7. `lib.rs:102-103` mid-UTF-8 cut drops the request with no reply; client hangs.
8. `lib.rs:106,109` error replies lack Python shapes (`unknown: <full cmd>`).
9. `lib.rs:96-99,129` accept loop ignores `WouldBlock` vs real errors.
10. `lib.rs:202-207` `bind_socket` check-then-bind race.
11. `schema.rs:12` `pub` inner on `EngineId`.
12. `schema.rs:52-66` cannot load the live Python prefs file.
13. `main.rs:169` falls back to `/tmp` (rejected by `agent-input-security.md`).
14. `main.rs:147` `log` exit status is journalctl's; unit names assumed.
15. `vroca-client/src/lib.rs:98` hard-coded `/tmp` path in test.
16. `tests.rs:212-220` dependency test is a substring grep.
17. `codec.rs:29-31` unknown policy value maps to `invalid_request`, spec says `invalid_argument`.

## Open Decisions Blocking Stages 4–6

§7.2 start-failure message, §7.3 socket permissions, §7.5 snapshot text
exposure, §7.7 player restart, §7.8 request size limits, §7.9 retryable
errors, §10.9 slice-shape MUSTs (channel id, `(ModelId, SpeakerIndex)`,
channel-keyed queue), `quit` exit status, and whether binding `vroca-v1.sock`
is promoted to slice 2 with a version number.

## Recommended Next Slices

1. **Scope decision** (doc only): DEC-18 accepting slices 1+2+partial 3 as
   landed, with acceptance per crate.
2. **Parser hardening** in `vroca-core`: whitespace tolerance, trailing-arg
   rejection, missing verbs, Python-shaped replies, fixtures table from
   `legacy-compatibility.md`, codec negative tests, `forbid(unsafe_code)`.
3. **Schema**: v0→v1 migration, `.bad` quarantine as a pure function,
   unknown-field preservation on snapshot, channel id and voice resolver types,
   private `EngineId`.
4. **State machine**: sentence splitter, bounded navigation, idle `Toggle`,
   typed `State::apply` results, preview restore point.
5. **Daemon transport**: framed reads with cap, own-path-only cleanup, real
   shutdown, blocking accept, player trait and health enum, snapshot writer.
6. **Client/CLI**: local validation, typed methods, exit-code mapping, no
   `/tmp` fallback.

Developer decisions are required before 5 and 6.

## Applied The Same Day

Workspace lints (ExocortexCent set), centralised dependencies, the dependency
budget test (replaces the manifest-grep test under DEC-2), `ErrorCode::exit_code()`
with CLI exit mapping, the two production `unwrap`s in the daemon and CLI
removed, `Response::Status` boxed, and `tools/` verbs. Clippy is clean under
`-D warnings`; 23 tests pass. The unused `_mpv_sock`/`_state_tx` and dead
`Shutdown` variant are annotated, not fixed; they stay on the list above.
