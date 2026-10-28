# Vroca Handoff

For whoever picks this up next. Written 2026-08-07, at the end of the Rust
design phase and the first round of Python improvements. Current-priority and
review-status notes updated 2026-10-28; deployment claims below are historical
and have not been reverified in this pickup.

Read [`pickup_notes.md`](pickup_notes.md) for current status and
[`roadmap.md`](roadmap.md) for the active plan. Use this document for historical
evidence and traps; read [`rust-spec.md`](rust-spec.md) before Rust changes.

---

## 1. What Vroca Is

A text-to-speech and assistive reading system. A daemon synthesizes speech and
drives `mpv`; a GTK overlay renders subtitles or RSVP; a control panel
configures it; a shell client and Unix socket let hotkeys, scripts, and local
agents submit speech. Five engines, all local except one HTTP adapter.

Deployment lives in `~/dev/nix-config`, a **separate repository and activation
boundary**. Do not edit it without explicit approval.

---

## 2. Document Map — Which One Answers What

This repository had a source-of-truth problem, which is now fixed. Keep it
fixed.

| Question | Document |
|:---|:---|
| What is the system *promised* to do? | [`vroca.md`](vroca.md) — the public contract |
| What was found in the Python audit? | [`rust-spec.md`](rust-spec.md) §5, the historical mismatch register; §3 below records the subsequent fixes |
| What must Rust build? | [`rust-spec.md`](rust-spec.md) — normative, owns every decision |
| What still needs migration fixtures? | [`legacy-compatibility.md`](legacy-compatibility.md) |
| How do I *call* Vroca from another program? | [`integration.md`](integration.md) |
| How do I install or enable Vroca declaratively? | [`installation.md`](installation.md) — packages, overlay, and Home Manager service |
| How should conversational interruption, channels, and voice overlap work? | [`conversational-audio-recommendation.md`](conversational-audio-recommendation.md) — recommendation pending specification decisions |
| What is the bounded macOS compatibility path? | [`macos-compatibility.md`](macos-compatibility.md) — Apple Silicon daemon/client, UI, and Hammerspoon plan |
| What order does work happen in? | [`roadmap.md`](roadmap.md) |

`vroca.md` deliberately does **not** describe the implementation. Its three
historically flagged command rows were reconciled by the Python sweep below.

---

## 3. State Of Play

### Working and deployed

Python is the production system. Recently fixed and live:

| Finding | Was | Now |
|:---|:---|:---|
| D8 | Malformed arguments terminated the daemon. `voice af_kore` crash-looped it to a restart counter of 51. | Typed error, daemon survives |
| N27 | The ASR aligner had been silently inoperative after a library rename; `status` reported `asr` while energy alignment ran | Repaired |
| — | Voices addressable only by number | Names, display names, and `engine:voice` qualified form |
| A1 | No level control | Master volume plus per-voice trim, persisted, with panel sliders |
| A2 | One voice for the whole queue | Per-item voices via `--voice` |
| A4 | Every `daemon.py` edit re-measured 904 voices | Engine code extracted to `engines.py` |

### The 2026-09-01 bug sweep (published, awaiting deployment)

Eighteen commits fixed the process-safety, input-safety, and semantics
findings of the mismatch register, each CLI-tested against an isolated
daemon (short `XDG_RUNTIME_DIR`, `TTS_MPV_ARGS=--ao=null`):

| Finding | Fix |
|:---|:---|
| N1 | mpv IPC socket is per-pid; no unconditional unlink, no cross-daemon player clobbering |
| N5 | `sendall` for replies; the 101 KB catalogue arrives intact |
| N6 | Requests read to a quiet gap with a 1 MiB cap; over-limit is an error, never truncation (~25 ms idle gap per command is the price) |
| N7 | Verb/argument split; empty arguments answer usage errors; `speed 1 2` rejected |
| N10 | Engine switch clears cache, spans, and render timings |
| N17 | Render timings reset per item; `avg_render_ms` no longer mixes engines forever |
| N12 | Eof while paused advances (staying paused) instead of stranding the queue |
| D2 | `say` clears the waiting queue — documented behavior is now true |
| D4 | `queue` while paused appends instead of replacing |
| D3 | `skip` abandons the current item and advances |
| N8, N9 | `reset` restores every preference including engine, clears cache and queue |
| N15 | All setters hold the state lock |
| N16 | Engine availability scan cached; position-driven dumps gated at 50 ms |
| D7 | Startup refuses when a live daemon owns the socket; stale sockets still recoverable |
| N4 | mpv death surfaces as `player_alive: false` plus a journal line; daemon keeps serving |
| N2, D9, N3 | atexit cleanup removes sockets, snapshot, and tmpdir and terminates mpv; SIGTERM raises cleanly; `quit` cleans up |
| N14 | Panel sends `mode_set`; daemon is the sole writer of `tts-mode` |
| N18, N19, N20, D6 | Panel: catalogue fetch gated on load state, sliders span the daemon ranges, start retries every 3 s with a journal hint after 30 s |

Already true before the sweep, verified rather than re-fixed: N11 and N13
(`read` clears spans; eof clears `word`).

Not fixed deliberately, still open: N21 (`position bottom` equals `center`
in RSVP modes — a UX decision), N23 (client auto-start policy — §7.2),
N24/N25 packaging questions (§7.13). D5 (shell client argument forwarding) was
subsequently fixed in this repository's Nix package; see `installation.md`.

Deploy note: these commits are **not** on the running daemon yet. Update the
`vroca_tts` input in `~/dev/nix-config`, review and commit its lockfile, then
rebuild the host to deploy them.

### Design complete

`rust-spec.md` holds 17 binding decisions with justification, 27 findings each
citing a line of code, and 21 open questions each naming what it blocks.

### Rust started, needs review

`rust_impl/` has the workspace and domain, client/CLI, and fake daemon/player
code. The earlier handoff recorded 19 passing tests and clean `fmt`; these were
not rerun in this pickup. Panel and overlay binaries are empty stubs. See §6:
spec review and migration-stage acceptance remain pending.

---

## 4. How To Work On This

### Ground rules that matter

1. **Python is behavior evidence, not correct design.** A difference between
   code and documentation is a finding, not automatically a requirement.
2. **Do not resolve an Open question implicitly.** `rust-spec.md` §7 and §10
   list them. Choosing whatever looks conventional is the specific failure the
   spec exists to prevent. Ask.
3. **Cite identifiers.** `D#` and `N#` are findings; `DEC-#` are decisions.
   `D9` and `DEC-9` are different things. Commits and tests should cite them so
   a behavior change traces to the evidence that motivated it.
4. **`~/dev/nix-config` needs explicit approval.** Every time.
5. **Measure before asserting.** Most of the useful findings here came from
   measuring rather than reasoning — the 101,359-byte catalogue against a
   106 KB buffer, kokoro at 3300 ms versus libritts at 139 ms, the aligner that
   silently was not running.

### Checks

```sh
nix develop --command python3 -m py_compile python_impl/*.py
cd rust_impl && nix develop --command cargo fmt --check
cd rust_impl && nix develop --command cargo check
cd rust_impl && nix develop --command cargo test
```

For daemon behavior, run an isolated daemon rather than touching the live one:

```sh
T=/tmp/vt$$; mkdir -p "$T/cfg"          # keep the path SHORT
MD="$(tr '\0' '\n' < /proc/$(systemctl --user show tts.service -p MainPID --value)/environ \
      | grep '^TTS_MODEL_DIRS=' | cut -d= -f2-)"
XDG_RUNTIME_DIR="$T" XDG_CONFIG_HOME="$T/cfg" TTS_MODEL_DIRS="$MD" \
  nix develop --command python3 python_impl/daemon.py &
```

Unix socket paths cap at 108 bytes — a long temp path fails with a confusing
"socket never appeared".

---

## 5. Traps

Each of these cost real time. They are not hypothetical.

**The deployment lock is a snapshot.** `~/dev/nix-config/flake.lock` pins the
public `vroca_tts` GitHub revision. Committing here changes nothing until
`nix flake update vroca_tts` runs in the deployment repo and its lockfile is
reviewed and committed. A rebuild may leave the old process running; check
`MainPID` and restart explicitly.

**Vroca owns its package derivations.** `nix-config` consumes the flake's
exported daemon, client, overlay, and panel packages. Keep model fetches,
Python environments, and package wrappers in Vroca; integration owns only the
service units and desktop hotkeys.

**`catalogue` is over 100 KB.** 101,359 bytes for libritts against roughly
106 KB of effective socket buffer. Read to EOF; a single `recv()` truncates.
The original `send()` defect (N5) was fixed with `sendall()` in the Python
sweep; clients still need to read the entire response.

**Select-then-queue does not set per-item voices.** Items render when they reach
the front, not when submitted. The voice must travel with the item.

**Engine choice dominates latency.** kokoro ~3300 ms per sentence, libritts
~139 ms. Everything else — client spawn, selection, wav write, alignment,
player start — totals under 70 ms. Check which engine is loaded before
investigating a latency complaint.

**Swallowed failures hide real breakage.** N27 sat silently for an unknown
period because `_align` catches everything. When something "works but seems
off", check whether an exception is being eaten.

---

## 6. The Rust Code — Read Before Trusting

Committed at `94ac41a`. Historical validation: compiled, 19 tests passed,
`fmt` clean. **Not reviewed during the design session; no fresh checks in the
2026-10-28 pickup.**

Good: tests cite spec identifiers. `test_d2_say_clears_waiting`,
`test_d3_skip_abandons_current` and `test_d4_queue_paused_appends` prove the
three intentional semantic changes of DEC-1. `test_d8_malformed_float_survives`
covers the production crash. `test_core_dependencies` enforces DEC-2.

Needs attention:

- **It exceeds the §8 first-slice scope.** That section says no sockets and no
  mpv; there are eight `UnixListener`/`UnixStream` references and `vroca-daemon`
  mentions mpv. The boundary moved without a recorded decision. Either record
  one or pull it back.
- Check `forbid(unsafe_code)` across all three crates; §7.6 recommends it.
- Nothing has been checked against the open questions. Code that resolves one
  implicitly is exactly what the spec warns about.

---

## 7. What To Do Next

**2026-10-28 pickup:** Nix packaging is implemented and verified on Apple
Silicon, with Linux interfaces evaluated. Package and service changes are
committed locally; no deployment was activated. Start at
[`pickup_notes.md`](pickup_notes.md) for the implementation map, exact checks,
remaining Mac validation, and Git state.

The active sequence is now owned by [`roadmap.md`](roadmap.md): easy Nix
installation, Mac daemon/client support, Rust review before substantial Mac UI
work, then the Mac UI and Hammerspoon integration. Interface decisions require
developer approval. The Mac UI's controls/subtitle/RSVP scope is still open.

The items below retain the earlier handoff's backlog context; they are not a
competing priority order.

1. **Review the Rust against the spec** (§6 above). Settle the scope question
   first — it changes what "done" means for the slice.
2. **Deploy the Python sweep**: update `vroca_tts` in `~/dev/nix-config`,
review and commit `flake.lock`, then rebuild the target host (each step is
approval-sensitive). Verify with
   `tts speed abc` (typed error), a second `tts-daemon` start (refusal), and
   `quit` + socket check (clean exit).
3. **Run `scripts/multivoice_demo.py`.** The overlapping half is an experiment,
   not a demo: it can inform overlap implementation and intelligibility checks.
   User direction so far: overlap should be a per-caller choice (games and
   simulations want it; GUI tools and notifications must not) — recorded in
   `rust-spec.md` §10.
4. **Track D is live in first config**: DeepInfra Qwen3-TTS through the
   `remote` engine, preset voices, ~3.7 s/sentence non-streaming. Streaming
   playback is the latency fix and is Rust-slice-sized. See
   `roadmap.md` Track D.
5. **Track C capabilities** — multi-voice channels, model residency, audio
   sinks, urgency, level control — are specified in `rust-spec.md` §10 with
   their open questions. None is Decided.

Deliberately deferred: chunked synthesis for kokoro (trades prosody for latency;
switching engine is a bigger lever), cross-engine simultaneous voices (needs
model residency), and TTS-aware Markdown (`rust-spec.md` §7.11).
