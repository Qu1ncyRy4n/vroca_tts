# Vroca Handoff

For whoever picks this up next. Written 2026-08-07, at the end of the Rust
design phase and the first round of Python improvements.

Read this first, then [`rust-spec.md`](rust-spec.md).

---

## 1. What Vroca Is

A text-to-speech and assistive reading system. A daemon synthesizes speech and
drives `mpv`; a GTK overlay renders subtitles or RSVP; a control panel
configures it; a shell client and Unix socket let hotkeys, scripts, and local
agents submit speech. Five engines, all local except one HTTP adapter.

Deployment lives in `~/nix-dotfiles`, a **separate repository and activation
boundary**. Do not edit it without explicit approval.

---

## 2. Document Map — Which One Answers What

This repository had a source-of-truth problem, which is now fixed. Keep it
fixed.

| Question | Document |
|:---|:---|
| What is the system *promised* to do? | [`vroca.md`](vroca.md) — the public contract |
| What does the Python code *actually* do? | [`rust-spec.md`](rust-spec.md) §5, the mismatch register |
| What must Rust build? | [`rust-spec.md`](rust-spec.md) — normative, owns every decision |
| What still needs migration fixtures? | [`legacy-compatibility.md`](legacy-compatibility.md) |
| How do I *call* Vroca from another program? | [`integration.md`](integration.md) |
| What order does work happen in? | [`roadmap.md`](roadmap.md) |

`vroca.md` deliberately does **not** describe the implementation. Three rows in
its command table are marked where the code does not honor the contract.

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

### Design complete

`rust-spec.md` holds 17 binding decisions with justification, 27 findings each
citing a line of code, and 21 open questions each naming what it blocks.

### Rust started, needs review

`rust_impl/` has the three-crate workspace, 19 passing tests, clean `fmt`. See
§6 below — it was not authored during the design session and has not been
reviewed against the spec.

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
4. **`~/nix-dotfiles` needs explicit approval.** Every time.
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

**The deployment lock is a snapshot.** `~/nix-dotfiles/flake.lock` pins
`vroca_tts` by content hash, so committing here changes nothing until
`nix flake update vroca_tts` — note the **underscore**. And a rebuild may leave
the old process running; check `MainPID` and restart explicitly.

Use `scripts/deploy.sh` instead while iterating. It overrides the input for one
invocation, so the lock never goes stale and never needs updating:

```sh
./scripts/deploy.sh --dry     # evaluate only
./scripts/deploy.sh           # rebuild and restart the service
```

**`path:` copies gitignored files; `git+file:` does not.** The deployment input
was `path:`, which has no git awareness and copied the whole directory into the
store — 461 MB, almost all of it `rust_impl/target`, with a hash that changed on
every `cargo build`. That is why the lock went stale constantly once Rust work
started. `git+file:` respects `.gitignore` and copies 540 KB. Measured, not
estimated. `deploy.sh` uses `git+file:` for exactly this reason.

**`measureSrc` and `engines.py` are coupled.** `~/nix-dotfiles/home/tts.nix`
copies specific files for the pitch-table derivation. If `measure.py` gains an
import that is not copied, the build fails with `ModuleNotFoundError`. This
already happened once.

**`catalogue` is over 100 KB.** 101,359 bytes for libritts against roughly
106 KB of effective socket buffer. Read to EOF; a single `recv()` truncates.
The daemon still uses `send()` and discards the return value (N5).

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

Committed at `94ac41a`. Compiles, 19 tests pass, `fmt` clean. **Not authored or
reviewed during the design session.**

Good: tests cite spec identifiers. `test_d2_say_clears_waiting`,
`test_d3_skip_abandons_current` and `test_d4_queue_paused_appends` prove the
three intentional semantic changes of DEC-1. `test_d8_malformed_float_survives`
covers the production crash. `test_core_dependencies` enforces DEC-2.

Needs attention:

- **It exceeds the §8 first-slice scope.** That section says no sockets and no
  mpv; there are eight `UnixListener`/`UnixStream` references and `vroca-daemon`
  mentions mpv. The boundary moved without a recorded decision. Either record
  one or pull it back.
- `forbid(unsafe_code)` is in one crate; §7.6 recommends all three.
- Nothing has been checked against the open questions. Code that resolves one
  implicitly is exactly what the spec warns about.

---

## 7. What To Do Next

1. **Review the Rust against the spec** (§6 above). Settle the scope question
   first — it changes what "done" means for the slice.
2. **Run `scripts/multivoice_demo.py`.** The overlapping half is an experiment,
   not a demo: whether the overlap is followable decides open question 10-a,
   which is currently open on an assumption.
3. **Continue Rust stage 3+** per [`roadmap.md`](roadmap.md) Track B.
4. **Track C capabilities** — multi-voice channels, model residency, audio
   sinks, urgency, level control — are specified in `rust-spec.md` §10 with
   their open questions. None is Decided.

Deliberately deferred: chunked synthesis for kokoro (trades prosody for latency;
switching engine is a bigger lever), cross-engine simultaneous voices (needs
model residency), and TTS-aware Markdown (`rust-spec.md` §7.11).
