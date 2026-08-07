# Vroca Rust Specification

## 1. Status And Conventions

This document is **normative**. It specifies the Rust implementation of Vroca
and supersedes [`rust-design.md`](rust-design.md), which was a discussion draft.

[`vroca.md`](vroca.md) remains the design of record for the *current* public
system. Where this specification intentionally changes public behavior, the
change is recorded here with a decision number and mirrored in
[`legacy-compatibility.md`](legacy-compatibility.md).

### 1.1 Requirement Levels

- **MUST** — binding. An implementation that does not do this is incorrect.
- **SHOULD** — strongly recommended. Departing requires a recorded reason.
- **MAY** — permitted, not required.

### 1.2 Decision States

- **Decided** — binding. Recorded in section 6 with justification. Changing a
  Decided item requires a new decision entry, not an edit in place.
- **Proposed** — a recommendation with no commitment. Implementation MUST NOT
  rely on a Proposed item without promoting it to Decided first.
- **Open** — unresolved. Recorded in section 7 with what it blocks. Code MUST
  NOT make an Open choice implicitly by looking conventional.

### 1.3 Identifier Scheme

Two distinct identifier spaces are used. They MUST NOT be conflated.

| Prefix | Meaning | Defined in |
| --- | --- | --- |
| `D<n>` | A **finding** recorded in the earlier design draft | §5 |
| `N<n>` | A **finding** newly discovered by reading the source and inspecting the live system | §5 |
| `DEC-<n>` | A **decision** | §6 |

`D<n>` and `DEC-<n>` are different things. `D9` is the orphaned-`mpv` finding;
`DEC-9` is the player decision. Cross-references MUST use the `DEC-` prefix when
they mean a decision, because the bare `D` space is already occupied by
findings.

Fixtures, commits, and tests MUST cite these identifiers so that a behavior
change can be traced to the evidence that motivated it.

---

## 2. Purpose, Scope, And Non-Goals

### 2.1 Purpose

Build a Linux-first Rust implementation of Vroca without removing the stable
Python path too early. The Rust implementation improves process safety, API
clarity, testability, and client symmetry. It MUST NOT reproduce Python
accidents merely because they are observable.

### 2.2 Locked Direction

1. Migration is a **staged replacement**.
2. Python remains usable until Rust passes a reviewed parity gate (section 9).
3. Behavioral parity is the first milestone. Approved design improvements may
   intentionally differ from Python where recorded.
4. Linux is the first complete platform.
5. The local interface is public enough for other local programs. Distribution
   and untrusted network access are not current goals.
6. Every GUI capability MUST have one underlying typed operation, reachable
   identically through the CLI and the local interface.
7. CLI actions use subcommands. Flags and options qualify those actions.
8. The legacy plaintext socket remains available during migration. A structured
   versioned protocol lives beside it.
9. Defaults are opinionated and revised through dogfooding.
10. Local engines are the priority. Remote engines remain an extension point.
11. TTS-aware Markdown notation is later work. The architecture MUST leave room
    for it without inventing its syntax now.
12. The Rust daemon becomes the sole owner of runtime state.

### 2.3 Non-Goals For The First Milestone

- Exposing Vroca over TCP or the public internet.
- Authentication for untrusted remote callers.
- A remote engine marketplace or general plugin loader.
- Removing Python before Rust is stable.
- Preserving undocumented crashes, races, or misleading command behavior.
- Finalizing TTS-aware Markdown notation.
- Completing macOS support.

---

## 3. Current System Inventory

This section records what exists today. It is evidence, not a specification of
what Rust must do.

### 3.1 Programs

```text
hotkeys and local programs
          |
          v
     tts shell client -----> tts.sock
                                  |
                                  v
                             Python daemon
                              |          |
                         tts-state.json  mpv IPC
                              |
                              v
                         GTK overlay

     GTK panel -----------> tts.sock
          |
          +---------------> tts-mode        (ownership violation, N14)
```

| Executable | Source | Defined in |
| --- | --- | --- |
| `tts` | `writeShellApplication` wrapping `socat` | `~/nix-dotfiles/home/tts.nix` |
| `tts-daemon` | `python_impl/daemon.py` | same |
| `tts-panel` | `python_impl/panel.py` | same |
| `tts-overlay` | `python_impl/overlay.py` | same |
| `measure.py` | build-time only; emits pitch tables | same, `pitchTables` derivation |

### 3.2 Files And Sockets

| Path | Written by | Mode | Atomic | Notes |
| --- | --- | --- | --- | --- |
| `$XDG_RUNTIME_DIR/tts.sock` | daemon | `0666` | — | unconditionally unlinked at startup (D7) |
| `$XDG_RUNTIME_DIR/tts-mpv.sock` | daemon (`Mpv.__init__`) | `0600` | — | also unconditionally unlinked (N1) |
| `$XDG_RUNTIME_DIR/tts-state.json` | daemon | `0644` | yes, `os.replace` | 20 fields, unversioned |
| `$XDG_RUNTIME_DIR/tts-mode` | **daemon and panel** | `0644` | no | bare string; ownership violation (N14) |
| `$XDG_RUNTIME_DIR/tts-<rand>/` | daemon | `0700` | — | wav cache; never removed (N2) |
| `$XDG_CONFIG_HOME/tts/prefs.json` | daemon | `0644` | yes, `os.replace` | 7 fields, unversioned |
| `$XDG_CONFIG_HOME/tts/voices/*.wav` | user | — | — | zipvoice references |
| `$XDG_CONFIG_HOME/tts/voices/*.txt` | daemon | — | no | cached ASR transcripts |
| `$XDG_CONFIG_HOME/tts/env` | user | — | — | `EnvironmentFile=-%E/tts/env` |

### 3.3 Environment Variables

| Variable | Required | Consumer |
| --- | --- | --- |
| `TTS_MODEL_DIRS` | yes, JSON object | `daemon.main` |
| `TTS_THREADS` | no, default `4` | engine construction |
| `TTS_PITCH_DIR` | no | `load_pitch_table` |
| `TTS_API_BASE` | remote only | `RemoteEngine` |
| `TTS_API_KEY_FILE` | remote only | `RemoteEngine` |
| `TTS_API_MODEL` | no, default `tts-1` | `RemoteEngine` |
| `TTS_API_VOICES` | no, six defaults | `RemoteEngine` |
| `XDG_RUNTIME_DIR` | falls back to tempdir | all paths |
| `XDG_CONFIG_HOME` | falls back to `~/.config` | prefs and voices |

### 3.4 Model Dependencies

All are static Nix store paths. Nothing is downloaded at runtime.

`kokoro` · `supertonic` · `libritts` · `zipvoice` · `vocoder` · `asr`

### 3.5 Hotkeys

Defined in `~/nix-dotfiles/home/cosmic.nix`.

| Binding | Command |
| --- | --- |
| `Super+Z` | `tts read` |
| `Super+Shift+Z` | `tts-panel` |
| `Super+X` | `tts toggle` |
| `Super+C` | `tts back` |
| `Super+V` | `tts next` |
| `Super+Shift+C` | `tts slower` |
| `Super+Shift+V` | `tts faster` |

No hotkey binds `skip`, `clear`, `say`, or `queue`. The hotkey surface is a
strict subset of the socket surface, which bounds the blast radius of the
semantic changes in Decision 1.

### 3.6 Preference Fields (7)

`engine`, `voice`, `speed`, `aligner`, `font_size`, `words_visible`, `position`.

Unversioned. Overlay mode is **not** among them; it lives in `tts-mode` and
persists only because that file happens to survive.

### 3.7 Runtime Snapshot Fields (20)

`sentence`, `index`, `total`, `paused`, `speed`, `rendered`, `rendering`,
`loaded`, `voice`, `word`, `engine`, `engines`, `aligner`, `queue_len`,
`font_size`, `words_visible`, `position`, `voices`, `last_render_ms`,
`avg_render_ms`.

Unversioned. Both GUIs consume it as an API.

### 3.8 GUI Control Map

| Panel control | Sends |
| --- | --- |
| Speed scale | `speed <float>` |
| Engine dropdown | `engine <name>` |
| Voice list, "Use voice" | `voice <int>` |
| "Preview" | `preview <int>` |
| Aligner dropdown | `aligner <name>` |
| **Overlay mode dropdown** | **direct `tts-mode` file write (N14)** |
| Position dropdown | `position <name>` |
| RSVP font size scale | `font_size <int>` |
| Scroll context scale | `words_visible <int>` |
| Skip / Clear / Reset buttons | `skip`, `clear`, `reset` |
| Stop / Reload / Unload / Quit | `stop`, `reload`, `unload`, `quit` |
| Background refresh, 400 ms | `status`, and `catalogue` once per engine |

The overlay renders only. It polls `tts-state.json` and `tts-mode` every 50 ms
and implements `subtitle`, `rsvp`, `scroll_rsvp`, and `off`.

### 3.9 Legacy Socket Exchange

1. Client connects to `tts.sock`.
2. Client writes one plaintext command.
3. Daemon reads at most 4096 bytes, once.
4. Daemon writes one plaintext response.
5. Connection closes.

The connection boundary is the only framing. There is no request ID, protocol
version, structured error, event subscription, or large-message rule.

---

## 4. Live-System Evidence

Observed on the development host by read-only inspection. Three failures that
the earlier draft listed as hypothetical are actually occurring.

| Observation | Command | Result |
| --- | --- | --- |
| Two daemons both listening on one path | `ss -xlp \| grep tts.sock` | pids 207703 and 154595, same path, inodes 2908932 and 2062913. Pid 154595 is unreachable; its inode was unlinked by the newer start. |
| Orphaned players | `pgrep -c mpv` | 11 live `mpv` processes for 2 daemons |
| Leaked temp directories | `ls -d $XDG_RUNTIME_DIR/tts-*` | 61 directories |
| Restart churn | `systemctl --user show tts.service` | `NRestarts=5`, `MainPID=207703` |
| Catalogue response size | `printf catalogue \| socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/tts.sock \| wc -c` | 101,359 bytes for libritts |
| Socket exposure | `ls -l $XDG_RUNTIME_DIR` | `srw-rw-rw-` inside a `0700` directory |
| **D8 crashing in production** | `journalctl --user -u tts.service` | `ValueError: invalid literal for int() with base 10: 'af_kore'` at `daemon.py:1032`, then `status=1/FAILURE`. Restart counter reached **51**. |

### 4.1 The `voice af_kore` Crash

D8 is not a latent risk. It is the active failure mode of this service. A client
sent `voice af_kore` — a voice *name* — and `int('af_kore')` raised, killing the
daemon. systemd restarted it, and the restart counter reached 51.

`af_kore` is a real kokoro voice ID from `voices.py:10`. Someone was addressing a
voice the way the catalogue names it. This is direct evidence for Decision 10:
the numeric-index API is not merely fragile in theory, it is failing the people
using it, and the natural thing to type crashes the daemon.

Two requirements follow, and neither is negotiable:

1. Malformed arguments MUST return a typed error (D8).
2. `voice <name>` MUST be a valid request (Decision 10).

Live preferences at time of inspection:

```json
{"engine": "libritts", "voice": 9, "speed": 1.0, "aligner": "asr",
 "font_size": 25, "words_visible": 2, "position": "center"}
```

Live mode file: `scroll_rsvp`.

These observations are the empirical basis for Decisions 7, 9, and 17, and for
findings N1 through N5.

---

## 5. Mismatch Register

Every entry cites the source that demonstrates it. A mismatch is **not**
automatically a Rust requirement; each is resolved by a decision in section 6 or
left Open in section 7.

### 5.1 Process Safety

| ID | Finding | Evidence |
| --- | --- | --- |
| D7 | Startup unconditionally unlinks the public socket. A second daemon silently orphans a healthy one; both keep calling `accept()` on different inodes. | `daemon.py:974-977`; confirmed live |
| N1 | `Mpv.__init__` also unconditionally unlinks `tts-mpv.sock`, so a second daemon breaks the first daemon's **player IPC**, not only its public socket. Strictly worse than D7 alone. | `daemon.py:96-98` |
| N2 | `tempfile.mkdtemp` runs per daemon start and the directory is never removed. 61 leaked directories observed. | `daemon.py:386` |
| N3 | `quit` calls `os._exit(0)`. With `Restart=on-failure`, exit status 0 means the user service **stops permanently** until manually started. It also skips socket unlink, tmpdir removal, and state cleanup. | `daemon.py:1018-1024`; `tts.nix:213` |
| D9 | Daemon crashes can leave child `mpv` processes alive. No `SIGTERM` handler, no `atexit`, no reaping. | `daemon.py:99-103`; confirmed live |
| N4 | If `mpv` dies, `Mpv._events` returns and `Mpv.cmd` swallows `OSError`. No `end-file` ever arrives, so playback stalls forever with **no error, no health signal, and no state change**. The daemon appears healthy. | `daemon.py:119-135` |

### 5.2 Input Safety

| ID | Finding | Evidence |
| --- | --- | --- |
| D8 | `float()` and `int()` on malformed arguments raise and terminate the daemon: `speed`, `voice`, `font_size`, `words_visible`, `preview`. **Confirmed in production: `voice af_kore` crashed the service to a restart counter of 51.** See §4.1. | `daemon.py:1030,1032,1038,1040,1046` |
| N5 | The response uses `conn.send()`, not `sendall()`, and discards the return value. The libritts `catalogue` response measures **101,359 bytes** against an effective Unix-socket payload capacity of roughly 106 KB. Margin is under 5% and shrinks as voices are added. A partial write silently truncates JSON; the panel's `json.loads` then fails and it shows an empty voice list. | `daemon.py:1049-1052`; measured |
| N6 | A single `conn.recv(4096)`. Longer text is truncated, possibly mid-UTF-8. `errors="replace"` turns that into corruption rather than a failure. | `daemon.py:988-989` |
| N7 | `strip()` runs before dispatch, so `tts say ""` arrives as `say` and returns `unknown: say`. Leading and trailing whitespace in spoken text is also lost. | `daemon.py:989,1025` |

### 5.3 Semantics

| ID | Finding | Evidence |
| --- | --- | --- |
| D2 | Documentation says `say` clears the active queue. Python replaces active sentences and leaves waiting items intact. | `daemon.py:671-679`; `vroca.md` §2 |
| D3 | Documentation says `skip` skips the current item and advances. Python removes the **next waiting** item while current playback continues, and stops when there is none. | `daemon.py:766-771` |
| D4 | `queue` while paused replaces the active speech and starts the new text instead of appending. | `daemon.py:744-758` |
| N8 | `reset` / `reset_prefs` does **not** reset `engine`, contradicting "Resets all settings to factory defaults". | `daemon.py:453-468`; `vroca.md` §2 |
| N9 | `reset_prefs` sets `sid = 0` without clearing the wav cache, so already-rendered audio in the **old voice** keeps playing. | `daemon.py:453-461` |
| N10 | `set_engine` sets `tts = None` without clearing the wav cache, so **audio rendered by the previous engine keeps playing after an engine switch**. | `daemon.py:496-509` |
| N11 | `read()` clears `cache` but not `spans`, leaving stale word timings for indices whose re-render fails. | `daemon.py:671-679` vs `681-688` |
| N12 | `_on_eof` returns early when `paused`, so pausing exactly at a sentence boundary can strand the waiting queue. | `daemon.py:607-610` |
| N13 | `_on_eof` clears `sents` but not `word`. Live state shows `sentence: "", total: 0, word: 6`. | `daemon.py:621-623`; confirmed live |

### 5.4 State Ownership And Consistency

| ID | Finding | Evidence |
| --- | --- | --- |
| N14 | The panel writes `tts-mode` directly, violating the sole-owner rule that `AGENTS.md` already states. | `panel.py:266-270` |
| N15 | `set_speed`, `set_font_size`, `set_words_visible`, and `set_position` mutate state **without** holding `self.lock`, while `status()` reads without it. Torn snapshots are possible. | `daemon.py:433-451,545-550,637-662` |
| N16 | `_dump()` is called from `_on_pos`, i.e. on every mpv position tick. Each dump calls `engines()`, which does `os.listdir(CLONE_DIR)` and `os.path.exists(key_file)`, then serializes 20 fields and rewrites the state file. A filesystem scan and a file rewrite many times per second. | `daemon.py:591-605,653,664-669,511-517` |
| N17 | `render_ms` is never cleared, so `avg_render_ms` mixes engines and documents for the daemon's entire lifetime. | `daemon.py:383,573,661` |
| N18 | The panel gates its catalogue fetch on `st.get("voices")` being truthy, which is `None` when the model is unloaded. After `unload`, the voice list never populates. | `panel.py:388`; `daemon.py:659` |
| D12 | Several failures are swallowed. Callers cannot distinguish invalid input, engine failure, playback failure, and success from a stable error shape. | `daemon.py:119-124,393-401,564-572` |

### 5.5 Range And Documentation Drift

| ID | Finding | Evidence |
| --- | --- | --- |
| N19 | Font size: the daemon clamps 12–72 and the docs say 12–72, but the panel slider is **14–48**. Live value 25. | `daemon.py:434`; `panel.py:206` |
| N20 | Context words: the daemon clamps 1–15 and the docs say 1–15, but the panel slider is **1–9**. Live value 2. | `daemon.py:440`; `panel.py:217` |
| N21 | `_anchor` computes `bottom = (pos == "bottom" and mode != "rsvp" and mode != "scroll_rsvp")`. In both RSVP modes, `position: bottom` and `position: center` produce **identical** vertically-centered output. The documented three positions are two. | `overlay.py:112-123` |
| D1 | No `/tmp/tts-speak` spool watcher exists, despite earlier documentation. Already reconciled in `vroca.md`. | — |
| D5 | The `tts` shell client forwards remaining arguments only for `say`, `speak`, and `queue`. `tts speed 1.2` sends bare `speed`. | `tts.nix:172-180` |
| N22 | Bare `tts` with no arguments means `read`, and `tts log` is a client-side `journalctl` shortcut. Neither appears in the IPC reference table. | `tts.nix:168-170,178` |
| N23 | The CLI prints an error and exits 1 when the socket is missing; the panel instead spawns `systemctl --user start tts`. Two clients, two opposite policies. | `tts.nix:171`; `panel.py:330-338` |
| D6 | The panel's `_starting` latch records one start attempt, ignores the result, and offers no retry or error state, so it can stick on "starting". | `panel.py:330-338` |
| D11 | The socket is `0666`, but the enclosing per-user runtime directory is `0700`. The real trust boundary is the owning user, and the mode bits are misleading. | `daemon.py:981`; confirmed live |

### 5.6 Packaging

| ID | Finding | Evidence |
| --- | --- | --- |
| D10 | The Python remote engine already implements an experimental OpenAI-compatible request, although remote engines are not urgent. | `daemon.py:291-338` |
| N24 | `python_impl/pyproject.toml` declares `sherpa-onnx`, `numpy`, `pygobject`, and `pycairo`, but nothing consumes it. Nix is authoritative. Two dependency surfaces, one unused. | `pyproject.toml`; `flake.nix:16-18` |
| N25 | `flake.nix` advertises `x86_64-darwin` and `aarch64-darwin` dev shells whose package list includes Linux-only `gtk4-layer-shell`. | `flake.nix:10,29` |
| N26 | `flake.nix` contains **no Rust toolchain**, while `AGENTS.md` requires cargo to run through `nix develop`. This blocks all Rust work. | `flake.nix` |

---

## 6. Decisions

Seventeen binding decisions. Each was raised and answered before being recorded.

---

### Decision 1 — Queue And Replacement Semantics

> **Revision pending (§10.4).** Multi-voice channels add a scope axis to
> replacement. `Replace::All` must become channel-scoped by default, or one
> agent's speech will discard another's.

**Status:** Decided.

**Current behavior.** `say` replaces active sentences and leaves waiting items
(D2). `queue` appends while playing but **replaces** while idle or paused (D4).
`stop` clears active sentences and cache but keeps the waiting queue. `clear`
drops the waiting queue then stops. `skip` removes the **next waiting** item, or
stops if there is none (D3).

**Alternatives considered.**

- *Preserve Python exactly and correct the documentation.* Rejected: it makes
  the accidental behavior a permanent contract, and D4 in particular is not
  defensible as a design — "append to the queue" silently discarding your
  reading because you happened to be paused is a bug wearing a feature's name.
- *Documented behavior with no policy parameter.* Rejected: a fixed meaning per
  operation means any future variant is a new operation and a protocol change.

**Decision.** The documented behavior is canonical. Variation is expressed as a
typed policy parameter on the operation, not as additional verbs.

```rust
Speak { text: String, replace: Replace }   // Replace::{All, Active, None}
Stop  { scope: Scope }                     // Scope::{Playback, Queue, All}
Skip  { unit: Unit }                       // Unit::{Sentence, Item}
```

Legacy verbs MUST map to the documented defaults:

| Legacy | Typed form |
| --- | --- |
| `say` / `speak` | `Speak { replace: Replace::All }` |
| `queue` | `Speak { replace: Replace::None }` — always appends |
| `stop` | `Stop { scope: Scope::Playback }` |
| `clear` | `Stop { scope: Scope::All }` |
| `skip` | `Skip { unit: Unit::Item }` — abandons the **current** item |
| `next` / `back` | sentence movement within the current item, unchanged |

**Justification.** `vroca.md` is the design of record and is what users and
local agents were told. Three of these divergences (D2, D3, D4) are cases where
the code quietly disagrees with the promise. Making the promise true is the
smaller surprise. The typed policy keeps the variation addressable without
multiplying verbs, and it costs nothing because the parameter is an enum the
parser fills in.

**Normal operation.** A client speaks, queues, and skips with the meanings the
documentation states.

**Malformed input.** An unknown policy value MUST produce `invalid_argument`.
Legacy verbs carry no policy token, so they cannot produce this error.

**Concurrent clients.** `Replace::All` from one client discards work submitted
by another. This is intended and matches a hotkey-driven product where the human
at the keyboard wins. Ordering across clients is the state owner's arrival
order; see Decision 4.

**Crash and restart.** Queue contents are session state and are **not**
persisted. A restart starts empty. Persisting a queue would resurrect speech the
user has forgotten submitting.

**Evolution.** New policy variants extend the enums. The wire format carries
policy names as strings, so an unknown variant is a clean `invalid_argument`
rather than a misparse.

**Privacy and trust boundary.** Submitted text may be private. It lives in
memory and in the wav cache under a `0700` directory, and MUST be removed on
shutdown (Decision 9).

**Test evidence required.** Fixtures for each legacy verb showing the state
transition; explicit tests that `queue` while paused appends (proving D4 is
gone), that `skip` abandons the current item (proving D3 is gone), and that
`say` clears waiting items (proving D2 is gone).

---

### Decision 2 — Crate Layout

**Status:** Decided.

**Current behavior.** None. `rust_impl/` contains only `.gitkeep`.

**Alternatives considered.**

- *One library crate plus four binaries.* Rejected: the public client is a real
  published surface and deserves its own manifest and version.
- *The draft's seven crates.* Rejected: five of those boundaries are guesses.
  Seven manifests before one feature works is cost paid for structure not yet
  known to be correct.

**Decision.** Three crates plus four binaries.

```text
rust_impl/
  Cargo.toml                 workspace
  crates/
    vroca-core/              domain, operations, state transitions,
                             legacy parser, structured codec.
                             MUST NOT depend on any I/O, async,
                             GUI, or FFI crate.
    vroca-client/            public local client API.
                             depends on vroca-core only.
    vroca-daemon/            lifecycle, engines, player, workers.
  bins/
    tts/  tts-daemon/  tts-panel/  tts-overlay/
```

**Justification.** These three boundaries already exist in reality. `vroca-core`
being provably dependency-free is what makes the first slice testable with no
runtime, no sockets, and no audio. `vroca-client` is consumed by three separate
binaries and by third-party local programs, which is the definition of a public
surface. Everything else is one program's internals until proven otherwise.
Crates are extracted when a dependency boundary becomes real — notably to
isolate `sherpa-onnx` FFI or GTK, both of which remain Open (section 7).

**Normal operation.** `cargo check -p vroca-core` succeeds with no transitive
I/O dependency. This is enforceable in CI and MUST be enforced.

**Malformed input.** Not applicable.

**Concurrent clients.** Not applicable.

**Crash and restart.** Not applicable.

**Evolution.** Extraction is additive and does not break callers if the
re-exported paths are kept.

**Privacy and trust boundary.** Keeping FFI out of `vroca-core` means the crate
carrying the domain logic contains no `unsafe`.

**Test evidence required.** A dependency assertion that `vroca-core`'s
lockfile-resolved graph contains no async runtime, socket, GUI, or FFI crate.

---

### Decision 3 — Legacy And Structured Protocol Coexistence

**Status:** Decided.

**Current behavior.** One socket, one plaintext format, no version.

**Alternatives considered.**

- *Sniff the first byte on one socket, treating `{` as JSON.* Rejected as
  unsafe: `say {"text":"hi"}` is a legal legacy command that begins with `{`.
  Content sniffing makes a legitimate input undecidable.
- *A fixed handshake preface on one socket.* Rejected: less fragile than
  sniffing, but the daemon must buffer before deciding, and retiring the legacy
  format later means changing the dispatch of a live socket.

**Decision.** Two socket paths.

| Path | Format |
| --- | --- |
| `$XDG_RUNTIME_DIR/tts.sock` | legacy plaintext, byte-identical, unchanged |
| `$XDG_RUNTIME_DIR/vroca-v1.sock` | structured, versioned (bound in slice 2, see Decision 6) |

**Justification.** No guessing, therefore no class of misparse bugs. Retiring
the legacy protocol later is simply declining to create the path, which is a
one-line change with no effect on the structured listener. The two paths can
also carry different permissions, which matters because the legacy socket's
`0666` (D11) is a compatibility obligation the new one need not inherit.

**Normal operation.** Both listeners accept concurrently. Both dispatch into the
same typed `Operation`, so behavior cannot diverge between them.

**Malformed input.** Each listener MUST reject malformed input for its own
format with a typed error and MUST NOT terminate the daemon. A legacy client
connecting to the structured path receives a structured `invalid_request`.

**Concurrent clients.** Requests from both sockets are serialized by the state
owner (Decision 4). Neither path has priority.

**Crash and restart.** Both paths are created and removed as one unit. A partial
state where one exists and the other does not MUST NOT persist past startup.

**Evolution.** A future `vroca-v2.sock` can be bound alongside `v1` and retired
independently. The version is in the path, so path presence is capability
discovery.

**Privacy and trust boundary.** The legacy socket keeps `0666` for
compatibility. Permissions for `vroca-v1.sock` are **Open** (section 7.3); the
recommendation is owner-only.

**Test evidence required.** Every documented legacy command round-tripped over
`tts.sock`; structured requests over `vroca-v1.sock`; cross-protocol misuse in
both directions producing typed errors; both sockets removed on clean shutdown.

---

### Decision 4 — Concurrency Model

> **Revision pending (§10.5).** Concurrent synthesis across voices depends on
> `sherpa-onnx` thread safety, which is unverified (§10.8, 10-f). Note also
> that the engines cannot abort mid-synthesis, so the generation counter
> discards rather than cancels; chunking (§10.3) bounds the waste.

**Status:** Decided.

**Current behavior.** Python uses a prefetch thread, an mpv event thread, and a
blocking accept loop, with an `RLock` that four setters fail to take (N15).

**Alternatives considered.**

- *Tokio.* Rejected. Async pays for many jobs that are mostly waiting on a
  network. Vroca has one user, a few clients, and one expensive CPU job.
  `sherpa-onnx` is blocking C++ and would land on `spawn_blocking` anyway, so
  the async would wrap something that cannot be async. Tokio would also colour
  the public client API, which Decision 5 fixes as synchronous, and GTK runs its
  own event loop that does not compose with Tokio without a bridge.

**Decision.** Plain OS threads.

```text
state owner    one thread. owns ALL mutable state. receives typed
               messages over a channel. never blocks on synthesis.
synthesis      a bounded worker pool. results are returned stamped
               with a generation number.
player         one thread reading mpv IPC events.
connections    one short-lived thread per accepted connection.
```

Cancellation is by **generation counter**: the state owner increments a
generation on stop, engine change, voice change, or shutdown; a worker's result
is discarded if its stamp is stale.

**Justification.** No runtime dependency, so `vroca-core` stays clean
(Decision 2) and domain tests are plain function calls. Blocking FFI is natural.
The one thing Tokio would genuinely give — structured cancellation — is
recovered by the generation counter, which is a small, well-understood
mechanism, and which the daemon needs regardless to solve N9 and N10.

**Normal operation.** Control operations (`status`, `stop`, `pause`) MUST remain
responsive while synthesis runs. This is the property N16 currently violates in
the opposite direction, by making cheap state publication expensive.

**Malformed input.** Handled at the connection edge before reaching the state
owner. A parse failure never becomes a state message.

**Concurrent clients.** Serialized by arrival at the state owner's channel.
There is exactly one writer of state, so N15's torn-snapshot class is
structurally impossible rather than merely avoided.

**Crash and restart.** A panicking connection thread MUST NOT take down the
daemon. A panicking state owner is fatal by design and MUST exit non-zero so
systemd restarts it, which is correct because state is then unknown.

**Evolution.** Worker count and prefetch depth are policy, not structure; both
are **Open** (section 7.4).

**Privacy and trust boundary.** Text passes through channels in-process only.

**Test evidence required.** A stop during in-flight synthesis discards the
stale result; concurrent submissions from two clients produce a deterministic
serialized order; a panicking connection handler leaves the daemon serving.

---

### Decision 5 — Public Client API Shape

**Status:** Decided.

**Current behavior.** No Rust client. Python clients open a socket and write
bytes, and the panel duplicates that logic (`panel.py:96-111`).

**Alternatives considered.**

- *Asynchronous API.* Rejected: it would force a runtime choice contradicting
  Decision 4, and none of the three real consumers wants it.

**Decision.** `vroca-client` is synchronous. A call blocks until the daemon
replies and returns a typed result or typed error. An async adapter MAY be added
later behind a feature flag; none is written now.

**Justification.** Every operation is a cheap control message, so the wait is
sub-millisecond. The CLI runs one command and exits. The panel already performs
blocking socket calls off the GTK loop. Shell scripts and local agents are
synchronous by nature. The wire protocol and daemon are indifferent to caller
style, so an async wrapper is purely additive later — nothing is foreclosed.

**Normal operation.** `client.speak(text)?` returns when the daemon has answered.

**Malformed input.** The client MUST validate what it can locally (speed range,
known enum values) and surface the daemon's typed error otherwise.

**Concurrent clients.** Each client owns its connection. The library MUST be
`Send` so callers can use it from their own threads.

**Crash and restart.** A connection failure MUST surface as `unavailable`, not
as a panic. The client MUST NOT auto-start the daemon; that policy is **Open**
(section 7.2) and is the subject of N23's inconsistency.

**Evolution.** Adding an async adapter behind a feature flag is additive.

**Privacy and trust boundary.** The client connects to a user-owned path and
performs no network access.

**Test evidence required.** Client and daemon integration over a temporary
socket; typed error on a missing socket; `Send` asserted in a compile test.

---

### Decision 6 — Structured Protocol Timing

**Status:** Decided.

**Current behavior.** No structured protocol exists.

**Alternatives considered.**

- *Defer all structured work to slice 2.* Rejected: the codec is largely derive
  macros over types slice 1 defines anyway, and writing it early proves those
  types serialize sanely.
- *Publish version 1 in slice 1.* Rejected: version 1 is a compatibility
  obligation owed forever, and locking it before anything has been dogfooded
  converts every early mistake into a permanent one.

**Decision.** The structured codec is **implemented in slice 1**, marked
explicitly unstable, carrying no version number, and **not bound to any socket**.
It is versioned as `1` and bound to `vroca-v1.sock` in slice 2.

**Justification.** This separates the cheap part (serialization, which validates
the domain types immediately) from the expensive commitment (a public version
promise). The code gets written and tested while the promise waits for evidence.

**Normal operation.** In slice 1 the codec is exercised only by round-trip
tests. No client can reach it.

**Malformed input.** Round-trip tests MUST include malformed JSON, unknown
operation names, wrong field types, and missing required fields.

**Concurrent clients.** Not applicable until slice 2.

**Crash and restart.** Not applicable until slice 2.

**Evolution.** The version number is introduced exactly once, at the moment the
socket is bound. Before that, the format may change freely.

**Privacy and trust boundary.** Not reachable in slice 1, so no exposure.

**Test evidence required.** Round-trip for every `Operation` variant; malformed
input rejection; a test asserting no socket is bound for the structured protocol
in slice 1.

---

### Decision 7 — Overlay Mode Ownership

**Status:** Decided. **Resolves N14.**

**Current behavior.** Mode lives in `tts-mode`, a bare-string file written by
both the daemon (`cycle_mode`) and the panel (`panel.py:266-270`). It is the
one user setting absent from `prefs.json`, and it persists only because the file
happens to survive.

**Alternatives considered.**

- *Session-only, resetting to `subtitle` each start.* Rejected: mode does
  persist today, and the live value is `scroll_rsvp`, so this would change
  behavior the user relies on.
- *Keep the separate file as the storage format, daemon-only writer.* Rejected:
  two persistence formats for one set of settings, and an unversioned bare-string
  file cannot participate in the migration scheme of Decision 17.

**Decision.** Overlay mode becomes a durable preference in `prefs.json`, owned
solely by the daemon. `tts-mode` remains written as a **read-only mirror** so
the Python overlay keeps working during migration. No other process may write
it.

**Justification.** `AGENTS.md` already states the daemon is the sole runtime
state owner, and the panel writing this file is the clearest violation of that
rule in the codebase. Moving mode into `prefs.json` also makes its persistence
intentional rather than incidental, and brings it under schema versioning.

**Normal operation.** The panel sends an operation like every other setting. The
daemon writes `prefs.json` atomically and refreshes the mirror.

**Malformed input.** An unknown mode string MUST produce `invalid_argument`. A
corrupt mirror file MUST be overwritten from authoritative state, not read back.

**Concurrent clients.** Single writer, so last write wins deterministically.
Today two writers race with no atomicity at all.

**Crash and restart.** Mode is restored from `prefs.json`. If the mirror is
missing or stale at startup it is rewritten from preferences.

**Evolution.** The mirror is deleted when the Python overlay is retired. Its
removal is a documented compatibility break, not silent.

**Privacy and trust boundary.** Unchanged; both files stay in user-owned
directories.

**Test evidence required.** Mode survives restart via `prefs.json`; the mirror
matches preferences after each change; an externally corrupted mirror is
corrected rather than trusted; a legacy `mode` command still cycles the
documented order `subtitle → rsvp → scroll_rsvp → off`.

---

### Decision 8 — State Delivery

**Status:** Decided.

**Current behavior.** The overlay re-reads `tts-state.json` every 50 ms whether
or not anything changed (`overlay.py:25,98-110`). The daemon rewrites that file
on every mpv position tick (N16).

**Alternatives considered.**

- *File only, permanently.* Rejected: polling at 20 Hz to catch word changes is
  both wasteful and still coarse, and it entrenches N16.
- *Push only, deleting the file.* Rejected: it breaks any script reading the
  file, removes the ability to inspect state with `cat`, and forces
  subscriptions to exist before the Rust overlay can ship.

**Decision.** The versioned snapshot file remains the compatibility path.
Push notifications are added to the structured socket in a later slice, and the
Rust overlay subscribes instead of polling. The file continues to be written
until no known reader needs it.

**Justification.** Nothing breaks at any point in the migration, which is the
governing constraint of a staged replacement. The file also remains a genuinely
useful debugging affordance and a zero-dependency integration point for scripts
in any language.

**Normal operation.** The daemon writes the snapshot atomically on **state
change**, not on every position tick. This alone resolves N16.

**Malformed input.** Readers MUST tolerate a snapshot they cannot parse by
retaining their last good state, not by rendering empty.

**Concurrent clients.** Any number of readers. Exactly one writer.

**Crash and restart.** A stale snapshot from a dead daemon is misleading. The
snapshot MUST carry the writing daemon's identity so a reader can detect
staleness. This is new; today nothing distinguishes a live snapshot from a
leftover one.

**Evolution.** Subscriptions are additive. File retirement is a separate,
documented decision.

**Privacy and trust boundary.** The snapshot currently contains the full current
sentence. Whether it should expose full submitted text is **Open**
(section 7.5).

**Test evidence required.** Snapshot written on state change and not on position
ticks; atomic replacement under concurrent readers; unparseable snapshot leaves
the reader's last good state intact; staleness detectable after daemon exit.

---

### Decision 9 — Player

> **Confirmed by §10.7.** The player trait is what keeps overlapping voices
> and in-process mixing reachable without touching domain code. Chunked
> playback (§10.3) adds a gapless requirement to the player interface.

**Status:** Decided. **Resolves N1, N4, D9. Contributes to N2.**

**Current behavior.** `mpv` runs as a child process with a JSON IPC socket at a
fixed path. The daemon unlinks that path unconditionally (N1), never reaps
children (D9), and silently stalls forever if `mpv` dies (N4). 11 orphan
processes are live.

**Alternatives considered.**

- *Embed libmpv.* Rejected for now: it brings `unsafe` FFI and callback
  boundaries into the daemon, converts a player crash into a daemon crash, and
  needs the FFI policy that is still Open (section 7.6).
- *Pure Rust audio (rodio/cpal).* Rejected: pitch-corrected time stretching
  would have to be implemented or vendored, and `faster`/`slower` are the most
  used hotkey pair in the product. Largest new risk for the least migration
  value.

**Decision.** `mpv` remains a supervised child process, with these MUSTs:

1. The IPC socket path MUST be unique per daemon instance. No daemon may unlink
   a path it does not own.
2. The daemon MUST terminate its `mpv` child on clean shutdown, on panic, and on
   `SIGTERM`.
3. Player death MUST transition health state to a visible degraded state, and
   control operations MUST keep working.
4. Temporary audio MUST be removed on shutdown.

**Justification.** `mpv` provides pitch-corrected speed control on an
already-decoded stream, which is exactly why `faster` and `slower` feel instant
instead of costing a re-synthesis round trip. That is real DSP work to replace.
Keeping the process boundary also means a crashing player cannot take the daemon
with it — which is worth more than the IPC protocol costs, given that the live
system currently demonstrates the failure mode in the other direction.

**Normal operation.** One daemon, one `mpv`, one owned socket path.

**Malformed input.** Unparseable mpv IPC lines are skipped without affecting the
event loop.

**Concurrent clients.** Irrelevant; the player is daemon-internal and never
directly reachable.

**Crash and restart.** If `mpv` dies, the daemon MUST report degraded health and
MAY restart it. It MUST NOT stall silently, which is the current behavior.
Restart policy for `mpv` is **Open** (section 7.7).

**Evolution.** The player sits behind a trait, so libmpv or a native backend can
replace it without touching domain code.

**Privacy and trust boundary.** Rendered audio derived from private text lives
in a `0700` directory and MUST be removed on shutdown, resolving N2's leak.

**Test evidence required.** Fake-player tests for playback events and child
failure; a killed player produces degraded health rather than a stall; two
daemons started concurrently do not disturb each other's player; no temp
directory or child process survives clean shutdown.

---

### Decision 10 — Voice Identity

> **Refinement pending (§10.5).** Residency is keyed by model, not voice, so
> `VoiceId` resolves through `(ModelId, SpeakerIndex)`. Live evidence for this
> decision is in §4.1: `voice af_kore` crashed the service 51 times.

**Status:** Decided.

**Current behavior.** A voice is an engine-local integer. `prefs.json` currently
holds `voice: 9` for libritts. If a model ships reordered voices, that 9
silently becomes a different speaker. Kokoro has 53 real names in `voices.py`;
supertonic has 10 unnamed and libritts 904 unnamed voices, described only by
measured pitch.

**Alternatives considered.**

- *Keep the numeric index as public identity.* Rejected: a number is meaningless
  without knowing the loaded engine, and model upgrades repoint it silently.
- *Derive names from an acoustic fingerprint at build time.* Rejected for now:
  the fingerprint would have to survive floating-point differences across
  onnxruntime versions, or names change when the runtime updates. That is a
  design problem of its own, and 904 fingerprints must also not collide.

**Decision.** A voice is identified by an engine-qualified stable string.
Pronounceable names for unnamed voices are minted **once** as **proquints** and
**committed to the repository**.

```text
kokoro:af_bella          existing real names retained
libritts:qorto           proquint, minted once, committed
supertonic:walom         proquint, minted once, committed
zipvoice:<filename>      derived from the reference clip
```

A proquint is a pronounceable encoding of an integer as alternating consonants
and vowels. The shared agent library already adopts proquint handles for durable
coordination artifacts (`cdint:engineering/coordination-ids/proquint-handles`),
so this reuses an existing convention rather than inventing a name scheme.

The mint-then-freeze distinction is essential:

- The proquint encoding is used **to generate** the initial name for each voice,
  which is why no arbitrary word list has to be authored or reviewed.
- Once minted, the mapping is **frozen in the repository**. A name is NOT
  recomputed from the index at read time. After a model reorders its voices, a
  frozen name deliberately no longer equals the proquint of its new index.

That second point is the whole design. A name computed from position would be
exactly as unstable as the position, which is the defect this decision exists to
remove. Freezing converts a model upgrade from a silent reshuffle into a
reviewable diff.

Legacy `voice N` MUST continue to work, resolved against the catalogue.

**Justification.** Committing the table makes stability a property of the
repository rather than of a clever algorithm. A model upgrade then produces a
reviewable `git diff` showing exactly which voices moved, and a human decides
what to remap — silent drift becomes an explicit review step. Pronounceable
names also matter in a product whose output is speech: you can say "qorto" out
loud, and you cannot usefully say "sid341". Proquints give that pronounceability
from a deterministic encoding, so minting 904 names needs no authored word list
and no aesthetic judgement.

**Normal operation.** Preferences store `"voice": "libritts:qorto"`.

**Malformed input.** An unknown `VoiceId` MUST produce `not_found` and MUST NOT
silently fall back to voice 0, which is the current failure mode.

**Concurrent clients.** Voice change is a single state transition and MUST clear
the render cache, resolving N9 and N10.

**Crash and restart.** An unresolvable saved voice MUST be reported in health
state and the previous index retained, not silently replaced.

**Evolution.** New engines add their own table. Remapping after a model upgrade
is a reviewed commit.

**Privacy and trust boundary.** Names are static repository data. Zipvoice names
derive from user filenames and therefore MUST NOT be logged or transmitted
anywhere outside the local machine.

**Test evidence required.** Legacy numeric resolution against the catalogue;
unknown ID producing `not_found`; a preference referencing a removed voice
surfacing in health state rather than silently changing voice.

---

### Decision 11 — Name And Classification Coupling

**Status:** Decided.

**Current behavior.** `anon_entry` (`voices.py:43-53`) builds a display name out
of the classification itself, e.g. `"deep male 104Hz"`. Name and guess are the
same string.

**Alternatives considered.**

- *Names chosen to match the classified voice* (heavier names for deeper
  voices). Rejected: names are frozen as identity under Decision 10, so a
  misclassification becomes permanent — a feminine-sounding name on a bass voice
  cannot be corrected without breaking every saved preference pointing at it.
  The classifier is known to have been wrong before.
- *Matched names with a separate opaque identity token.* Rejected: it restores
  an opaque token as the real identity, which is most of what was wrong with
  `sid9`, and requires looking up two things instead of one.

**Decision.** The frozen name is **neutral** and carries no claim. Gender, pitch
band, and description live in a **separate, correctable traits file**.

```text
voices/libritts.json          frozen identity
  "0": "qorto"

voices/libritts-traits.json   correctable metadata
  "qorto": { "pitch_hz": 128, "band": "low",
             "gender": "male", "confidence": 0.91 }
  "walom": { "pitch_hz": 171, "band": "mid",
             "gender": "uncertain", "confidence": 0.44 }
```

The panel displays `qorto · low male · 128Hz`.

**Justification.** Identity is a promise that must never change; classification
is a guess that will sometimes be wrong. Binding them makes the guess inherit
the promise. Separating them means classification can be re-run at any time,
and a wrong label is a one-line fix that breaks nobody's preferences.

**Normal operation.** Catalogue entries join identity and traits at read time.

**Malformed input.** A missing or unparseable traits entry MUST degrade to
identity-only display, never to a wrong claim.

**Concurrent clients.** Both files are read-only at runtime.

**Crash and restart.** Static data; nothing to recover.

**Evolution.** Traits may be regenerated freely. Identity may not.

**Privacy and trust boundary.** Repository data, no user content.

**Test evidence required.** Catalogue joins identity and traits correctly;
missing traits degrade gracefully; a traits change does not alter any `VoiceId`.

---

### Decision 12 — Voice Classification Method

**Status:** Decided.

**Current behavior.** `measure.py:22-39` estimates F0 by raw autocorrelation
peak-picking over a 70–350 Hz band with no octave-error guard. `voices.py:43-53`
then splits gender on a single threshold at 155 Hz. Previous runs misclassified
gender and pitch range.

**Root cause.** Two compounding errors. Autocorrelation routinely locks onto
half or double the true frequency, so a 220 Hz voice reads as 110 Hz and is
labelled male. Separately, male and female F0 distributions genuinely overlap
between roughly 145 and 185 Hz, so a single threshold in that band is close to a
coin flip even when the pitch is measured correctly.

**Alternatives considered.**

- *Report measured properties only and never assert gender.* Rejected: it cannot
  be wrong, but it drops a genuinely wanted feature, and searching the panel for
  a female voice stops working.
- *Automated pass plus a human audition session.* Not rejected — see Evolution.
  It is deferred because it presupposes a classifier good enough to be worth
  correcting.

**Decision.** Redo classification with three changes:

1. Replace autocorrelation with a YIN-style estimator plus an octave-jump
   penalty, removing the half-and-double errors.
2. Add features beyond pitch — spectral centroid and formant-related measures,
   which track vocal tract length and separate voices that overlap in pitch.
3. Output `male | female | uncertain`, each with a confidence value. The
   classifier MUST be permitted to decline.

**Justification.** The previous attempt failed because a weak measurement fed a
weak classifier, and neither could express doubt. Fixing the estimator removes
the largest error source outright. Allowing `uncertain` means a wrong answer
becomes an honest one, which matters because Decision 11 renders these labels
directly to the user.

**Normal operation.** Classification runs offline and its output is committed.

**Malformed input.** A voice that fails to synthesize is recorded as `uncertain`
with zero confidence, not skipped silently as it is today.

**Concurrent clients.** Not applicable; offline tooling.

**Crash and restart.** Not applicable.

**Evolution.** A later human audition pass MAY record corrections in an
overrides file that automated re-runs never overwrite. This is the natural next
step once the classifier is worth correcting.

**Privacy and trust boundary.** Operates only on model-generated audio.

**Test evidence required.** The estimator recovers known synthetic F0 including
deliberate octave traps; overlapping-pitch voices produce `uncertain` rather
than a confident coin flip; a synthesis failure yields `uncertain`, not a
missing entry.

---

### Decision 13 — Context-Dependent Commands

**Status:** Decided.

**Current behavior.** `read` reads the primary selection, but stops instead if
speech is active (`daemon.py:990-991`). `toggle` flips pause and resume. Both
are one key with two meanings.

**Alternatives considered.**

- *Explicit operations only.* Rejected: it does not remove the toggle, it
  relocates it. Either the CLI grows the same conditional logic, or the hotkey
  definitions in `~/nix-dotfiles` change — and that is a separate ownership and
  approval boundary.

**Decision.** `read` and `toggle` keep their exact current behavior as
convenience operations. Underneath, each is defined as a shortcut over explicit
typed operations that clients may call directly.

```text
ReadSelection · Stop { scope } · Pause · Resume

read   == ReadSelection when idle, Stop when active
toggle == Pause when playing, Resume when paused
```

**Justification.** `Super+Z` and `Super+X` are the product. Muscle memory built
on them is the thing being preserved. At the same time, scripts and local agents
need operations that do exactly one thing, because a context-dependent command
is unusable when you cannot observe the context atomically. Defining the
convenience form in terms of the explicit form gives both without duplicating
logic.

**Normal operation.** Hotkeys behave identically to today.

**Malformed input.** Neither takes arguments; extra arguments MUST produce
`invalid_request`.

**Concurrent clients.** The toggle is evaluated **inside** the state owner, so
the observe-and-act sequence is atomic. Today it is not: `is_active()` and the
subsequent action are separate lock acquisitions.

**Crash and restart.** Both are stateless requests.

**Evolution.** Convenience forms may be deprecated independently of the explicit
operations.

**Privacy and trust boundary.** `ReadSelection` reads the X or Wayland primary
selection, which may contain anything currently selected — including passwords.
This is an existing, deliberate capability, and it MUST NOT be logged.

**Test evidence required.** `read` while idle reads the selection; `read` while
active stops; `toggle` in both directions; the explicit four operations tested
independently; atomicity under a concurrent state change.

---

### Decision 14 — Preview Playback

> **Simplification available (§10.4).** If channels are adopted, preview
> becomes an ephemeral high-priority channel and the interrupt-then-restore
> machinery mostly disappears.

**Status:** Decided.

**Current behavior.** `preview` synthesizes a fixed sentence and loads it into
the **main** player, cutting off whatever was playing and never restoring it
(`daemon.py:724-738`). Clicking Preview in the panel mid-article loses your
place with no warning and no undo.

**Alternatives considered.**

- *A separate preview player.* Rejected for now: two players to supervise, and
  both audible at once unless the reading is ducked or paused first, which is
  the same restore problem in a different place.
- *Keep current behavior.* Rejected: auditioning voices is the panel's main
  purpose, and it should not cost the user their position.

**Decision.** Preview interrupts the main player, then restores the reading to
the sentence and pause state it was in.

**Justification.** One player, no overlapping audio, and position is preserved.
The cost is state machinery that must be correct in every case, including
preview failing partway — which is exactly the kind of thing the typed state
owner in Decision 4 exists to make tractable.

**Normal operation.** Reading pauses, preview plays, reading resumes at the same
sentence with its previous pause state.

**Malformed input.** An out-of-range `VoiceId` MUST produce `not_found` and MUST
NOT disturb playback at all.

**Concurrent clients.** Preview MUST be serialized with playback control. A
second preview during a preview replaces the first without losing the saved
restore point.

**Crash and restart.** If preview synthesis or playback fails, the reading MUST
still be restored. The restore point MUST NOT be lost by an error path.

**Evolution.** A separate preview player remains possible later without changing
the operation's contract.

**Privacy and trust boundary.** Preview audio is generated from a fixed sentence
and contains no user text.

**Test evidence required.** Preview during active playback restores sentence and
pause state; preview while paused leaves the reading paused; a failing preview
still restores; preview while idle does not create phantom state.

---

### Decision 15 — Reset

**Status:** Decided. **Resolves N8 and N9.**

**Current behavior.** `reset` restores speed, aligner, voice, font size, context
words, and position, writes `subtitle` to the mode file, and **leaves the engine
unchanged** (N8) while **not clearing the render cache** (N9), so old-voice
audio can keep playing afterwards. `vroca.md` documents it as resetting all
settings to factory defaults.

**Alternatives considered.**

- *Preserve Python's behavior and correct the documentation.* Rejected: it
  avoids a model reload, but corrects the design of record toward the accident,
  which is the direction this migration explicitly avoids.
- *Reset everything with no group parameter.* Rejected: there is no way to reset
  overlay appearance without also discarding the engine choice and paying a
  model reload.

**Decision.** `reset` restores every preference including engine, stops
playback, and clears rendered audio. The typed operation accepts named groups.

```text
reset            everything
reset overlay    mode, position, font size, context words
reset speech     engine, voice, speed, aligner
```

**Justification.** It makes the documented promise true, and it fixes a genuine
correctness bug in the same stroke: clearing the cache is required whenever
voice or engine changes, which is the same underlying rule as N10. Named groups
preserve the useful narrow case without a second verb.

**Normal operation.** Preferences return to defaults and are written atomically.

**Malformed input.** An unknown group name MUST produce `invalid_argument`. The
legacy `reset` and `reset_prefs` verbs carry no group and map to "everything".

**Concurrent clients.** A single state transition; in-flight synthesis is
invalidated by generation bump (Decision 4).

**Crash and restart.** Preferences are written atomically, so a crash mid-reset
leaves either the old or the new set, never a mixture.

**Evolution.** New groups are additive.

**Privacy and trust boundary.** Reset MUST clear cached audio, which removes
rendered private text from disk. This is a privacy improvement over current
behavior.

**Test evidence required.** Reset restores every field including engine; the
render cache is empty afterwards; audio from the previous voice cannot play;
group reset touches only its group; both legacy verbs behave identically.

---

### Decision 16 — Public Names

**Status:** Decided.

**Current behavior.** Python uses `Reader`, `sents`, `sid`, `status`, and passes
command strings through a long `if/elif` chain in `main()`.

**Alternatives considered.**

- *Match today's vocabulary* (`Command`, `Status`, `Sentence`). Rejected on two
  points. `Command` would name both the domain concept and the wire bytes, which
  is precisely the ambiguity the split exists to remove. And `Sentence` becomes
  wrong the moment TTS-aware notation splits text against grammar, which is
  planned work.

**Decision.**

| Concept | Name |
| --- | --- |
| An action, in the domain | `Operation` |
| The same action, on the wire | `Request` |
| Published state | `RuntimeSnapshot` |
| Component health | `Health` |
| Submitted text plus derived utterances | `SpeechItem` |
| One synthesis unit | `Utterance` |
| Public voice identity | `VoiceId` |
| Engine-local numeric speaker | `SpeakerIndex` |
| Validated engine identity | `EngineId` (newtype) |
| Overlay mode variant | `OverlayMode::ScrollRsvp`, wire spelling `scroll_rsvp` |

Binaries remain `tts`, `tts-daemon`, `tts-panel`, `tts-overlay`. Crates are
`vroca-core`, `vroca-client`, `vroca-daemon`.

**Justification.** Names become public concepts across the domain, the wire
format, the CLI, and the documentation, so they are expensive to change later.
Separating `Operation` from `Request` keeps wire concerns out of domain code.
Separating `VoiceId` from `SpeakerIndex` is what makes Decision 10 expressible.
The wire spelling `scroll_rsvp` matches what the `tts-mode` file contains today,
so no existing data needs rewriting. Keeping the binary names preserves every
hotkey, script, and systemd unit.

**Normal operation, malformed input, concurrency, crash, privacy.** Not
applicable; this decision is nomenclature.

**Evolution.** Renaming a public type after release is a breaking change, which
is why these are settled before code spreads them.

**Test evidence required.** Wire spelling round-trips as `scroll_rsvp`; binary
names asserted in packaging; `EngineId` rejects unknown values at construction.

---

### Decision 17 — Persistence Schema And Corruption Handling

**Status:** Decided.

**Current behavior.** `prefs.json` and `tts-state.json` carry no version field.
`load_prefs` catches `OSError` and `ValueError` and returns `{}`, so a corrupt
preferences file **silently reverts every setting to defaults with no message
anywhere** (`daemon.py:915-920`). Unknown fields are dropped on write-back
because `save_prefs` rebuilds the object from scratch.

**Alternatives considered.**

- *Version and migrate, but reject unknown fields.* Rejected: an older daemon
  would refuse to start after a newer one had written the file, and rollback to
  the Python path is explicitly part of the parity gate.
- *Version and migrate, keep silent default fallback.* Rejected: silent loss of
  every setting is the worst failure mode available here, and it is the one that
  exists today.

**Decision.** Both files carry a schema version. Specifically:

1. Both MUST include a `schema` field.
2. Older versions MUST be upgraded by explicit, tested migration steps.
3. Unrecognized fields MUST be preserved on write-back, so a newer daemon's
   setting survives a downgrade.
4. A corrupt file MUST be renamed aside (`prefs.json.bad`), defaults used, and
   the failure reported in health state so the panel can display it.
5. Writes MUST remain atomic.

**Justification.** Losing settings silently is the failure that matters most
here, because the user discovers it by noticing their voice changed. Renaming
aside preserves the evidence and makes recovery possible. Preserving unknown
fields is what makes downgrade safe, and downgrade to Python is a required
property of the rollout (section 9).

**Normal operation.** The daemon reads, migrates if needed, and rewrites at the
current version.

**Malformed input.** Covered by rule 4. A malformed **value** in an otherwise
valid file MUST be clamped or rejected per its own field rules, and the
correction reported — not silently accepted, which is how `font_size` and
`words_visible` behave today.

**Concurrent clients.** One writer only, so no coordination is needed.

**Crash and restart.** Atomic replacement means a crash leaves either the
complete old file or the complete new one.

**Evolution.** Each schema bump adds one migration step with its own test.

**Privacy and trust boundary.** A quarantined `.bad` file may contain user
settings and MUST inherit the same permissions as the original.

**Test evidence required.** Migration from every prior version; unknown fields
round-trip through a write; a corrupt file is quarantined, defaults are used and
health reports it; malformed values are clamped or rejected with a report;
atomic write verified under interruption.

---

## 7. Open Questions

These are **not** decided. Implementation MUST NOT resolve them implicitly.

### 7.1 Engine Binding Strategy

**Question.** Use `sherpa-onnx` through an existing Rust crate, direct C API
bindings, or a narrow local FFI wrapper?

**Blocks.** Slice 6 (first real synthesis engine). Does **not** block slice 1.

**Recommendation.** A narrow local FFI wrapper isolated in its own crate, so
`unsafe` is confined to one reviewable surface and `vroca-core` stays clean per
Decision 2.

### 7.2 Daemon Start Policy

**Question.** Should the CLI or panel start the daemon, ask systemd to start it,
or report that it is unavailable? Today they disagree (N23), and the panel's
attempt has no retry or error state (D6).

**Blocks.** Slice 4 (CLI) and slice 8 (panel).

**Recommendation.** Neither client starts the daemon. Both report `unavailable`
with the exact command to start it. Auto-start hides failures, which is how D6
became possible.

### 7.3 Structured Socket Permissions

**Question.** What mode is `vroca-v1.sock` created with? The legacy socket's
`0666` is misleading (D11).

**Blocks.** Slice 5 (daemon lifecycle).

**Recommendation.** Owner-only `0600`. The enclosing directory is already `0700`,
so `0666` grants nothing and only misstates the boundary.

### 7.4 Worker Count And Prefetch Depth

**Question.** How many synthesis workers per engine? Is prefetch depth fixed
(currently 5), configured, or adapted from measured render speed?

**Blocks.** Slice 5. Affects memory and disk use of the wav cache.

**Recommendation.** One worker per engine initially, because engines are not
known to be thread-safe. Keep prefetch fixed at 5 until dogfooding says
otherwise.

**Escalated by §10.5.** Multi-voice makes this urgent rather than deferrable,
and "not known to be thread-safe" must become a tested fact (§10.8, 10-f)
before concurrent synthesis is built on it. Chunking (§10.3) also changes the
prefetch unit from the sentence to the chunk.

### 7.5 Snapshot Text Exposure

**Question.** Should the published snapshot expose full submitted text, the
current sentence only, or both? Full text may be private.

**Blocks.** Slice 3 (snapshot schema). This is a privacy decision, not a
convenience one.

**Recommendation.** Current sentence only, matching today's behavior. Full text
is available to the submitting client, which already has it.

### 7.6 Unsafe FFI Policy

**Question.** What is the policy for `unsafe` code and native callback
boundaries? `AGENTS.md` requires one before FFI lands.

**Blocks.** 7.1, and any future libmpv work.

**Recommendation.** `unsafe` permitted only inside dedicated binding crates,
each function documenting its safety contract, with `#![forbid(unsafe_code)]` in
`vroca-core`, `vroca-client`, and `vroca-daemon`.

### 7.7 Player Restart Policy

**Question.** When `mpv` dies, should the daemon restart it, exit, or stay
degraded? Decision 9 requires visible degradation but does not choose recovery.

**Blocks.** Slice 6.

**Recommendation.** Restart with backoff, capped, then remain degraded. Never
exit — control operations and state publication must survive a dead player.

### 7.8 Request Size Limits

**Question.** What is the maximum request size, especially for long text? Today
it is an accidental 4096 bytes with silent truncation (N6).

**Blocks.** Slice 3 (parsers).

**Recommendation.** An explicit limit, generous for text, with over-limit input
producing `invalid_request` rather than truncation. The exact number needs a
real-use estimate.

### 7.9 Error Retry Semantics

**Question.** Which error categories are safe to retry? Should long operations
return an operation ID for later status? How are partial failures represented,
such as audio synthesized but playback unavailable?

**Blocks.** Slice 4 (client error surface).

**Recommendation.** Mark `unavailable`, `busy`, and `timeout` retryable; the
rest not. Defer operation IDs until a genuinely long operation exists.

### 7.10 GUI Process Layout

**Question.** GTK4 with gtk4-layer-shell in Rust, or another stack? Panel and
overlay as separate processes or one process with two surfaces? Optimistic UI
updates or wait for daemon confirmation? Which accessibility guarantees are
required?

**Blocks.** Slice 8.

**Recommendation.** Keep GTK4 and layer-shell, since the layer-shell dependency
is what makes a non-focus-stealing overlay possible at all, and keep the two as
separate processes so an overlay crash cannot take the panel with it.

### 7.11 TTS-Aware Markdown Notation

**Question.** Is TTS notation an extension of CommonMark, a fenced directive
syntax, front matter, or a sidecar document? Must ordinary Markdown remain valid
unchanged? May directives select engines or remote providers? What trust rules
apply when a document requests files, voices, or networked engines?

**Blocks.** Nothing currently. Explicitly later work.

**Recommendation.** Do not invent syntax now. Keep a typed intermediate document
between parsing and utterance generation so notation can be added without
reshaping the pipeline. The trust question is the hard part and deserves its own
review.

### 7.12 Remote Engines And Credentials

**Question.** Network timeouts, retries, rate limits, streaming, cancellation,
privacy, and provider error mapping.

**Blocks.** Nothing currently. Remote engines are not urgent.

**Recommendation.** Preserve the provider boundary now so adding one later
changes no daemon, CLI, GUI, or protocol operation. Credentials MUST remain
outside the repository and outside the Nix store, as `tts.nix` already
documents.

### 7.13 Undecided Compatibility Details

- ~~Whether `unload` remains useful once engine loading is redesigned.~~
  **Answered by §10.5:** it survives and is promoted into a model residency
  surface keyed by `ModelId`. N18 remains a defect to fix in the panel.
- Whether the `tts-state.json` file is a public interface or only a migration
  artifact (interacts with Decision 8).
- Whether a spool file remains useful now that a public local API is specified
  (D1).
- Whether `python_impl/pyproject.toml` should be removed or made authoritative
  (N24), and whether the flake should stop advertising darwin shells (N25).

---

## 8. First Slice Scope

The first Rust change is bounded. It MUST contain only:

1. The workspace and three crates of Decision 2.
2. `vroca-core`: typed `Operation`, state transitions, and queue semantics per
   Decision 1, using the names of Decision 16.
3. The legacy plaintext parser, where **every** malformed input case returns a
   typed error instead of terminating the process (D8).
4. The structured codec of Decision 6 — unstable, unversioned, unbound.
5. Preference and snapshot schema types with migration scaffolding
   (Decision 17).
6. Fixtures derived from [`legacy-compatibility.md`](legacy-compatibility.md).

It MUST NOT contain: sockets, `mpv`, audio, GTK, `sherpa-onnx`, systemd
integration, or any change under `~/nix-dotfiles`.

**Prerequisite.** `flake.nix` must gain a Rust toolchain (N26). That is itself a
reviewed change requiring `nix develop` approval and a lockfile refresh.

---

## 9. Testing And Parity Gate

### 9.1 Required Test Layers

- Domain tests for state transitions and queue semantics.
- Parser tests for every legacy command, malformed input, limits, and aliases.
- Fixtures from the compatibility matrix before any command is called compatible.
- Structured protocol round-trip and compatibility tests.
- Preference and snapshot schema migration tests.
- Fake-engine tests for success, delay, failure, and cancellation.
- Fake-player tests for playback events and child failure.
- Lifecycle tests using temporary runtime and config directories.
- CLI tests for arguments, output, errors, and exit status.
- Client and daemon integration tests over temporary Unix sockets.
- GUI logic tests below the widget layer.
- Focused Linux smoke tests for GTK layer-shell and real audio.

### 9.2 Parity Gate

Rust MAY replace Python in deployment only when all of the following hold.

1. Every approved operation exists in the typed API, the CLI, and the relevant
   GUI.
2. Legacy protocol compatibility tests pass for every command in
   [`legacy-compatibility.md`](legacy-compatibility.md).
3. Preference migration and rollback behavior are tested (Decision 17).
4. Duplicate startup (D7, N1), malformed input (D8), player death (N4), engine
   failure, and clean shutdown (N2, N3, D9) tests pass.
5. The overlay and panel work on the target Linux desktop.
6. Dogfooding confirms normal reading, queueing, seeking, speed, voices, modes,
   and hotkeys.
7. Nix builds and service integration are reviewed in `~/nix-dotfiles` as a
   separately approved change.
8. Switching back to Python remains documented and tested.

### 9.3 Migration Stages

1. Lock the decisions needed for the first slice. **Complete — section 6.**
2. Add the Rust toolchain and the reviewed workspace shape. **Done**
3. Typed operations, state transitions, and protocol parsers with no audio
   dependency. **Done**
4. Public client and CLI against a fake daemon. **Done**
5. Daemon lifecycle with fake engine and fake player. **Done**
6. One local synthesis engine and real playback.
7. Remaining approved local engines and alignment.
8. Overlay and panel through the shared client.
9. Parity tests and dogfooding without changing default deployment.
10. Update `~/nix-dotfiles` in a separately approved deployment change.
11. Observe the Rust service with a documented Python rollback.
12. Retire Python in a later, separate decision.

---

## 10. Multi-Voice, Model Residency, And Playback Latency

**Status: Proposed and Open.** This section records requirements raised after the
first seventeen decisions were locked. Nothing here is Decided. It exists so the
first slice does not foreclose these capabilities, and so the affected decisions
are revisited deliberately rather than discovered to be wrong later.

### 10.1 The Requirements

| # | Requirement | Origin |
| --- | --- | --- |
| R1 | Multiple voices usable at the same time, so different local agents speak in different voices — for example an architect agent and an implementation agent. | User, this session |
| R2 | Preload and unload models through the API and CLI. | User, this session |
| R3 | Voices sourced from a local directory, so a repository can carry the voices its agents use. | User, this session |
| R4 | Lower playback latency than waiting for a whole sentence to render. | User, this session |

### 10.2 Measured Evidence

Measured on this host through the Nix dev shell, against the deployed models.
These numbers constrain the design, so they are recorded rather than summarized.

**Synthesis is not streaming for the current engines.** `sherpa-onnx`'s
`OfflineTts.generate` accepts a `callback(samples, progress)` that can return
non-zero to stop early. For kokoro and libritts the callback fires **once, at the
end**. There is no partial audio to play and no mid-synthesis abort.

| Engine | Sentence audio | Full synthesis | RTF | Callback chunks |
| --- | --- | --- | --- | --- |
| kokoro | 5263 ms | 4630 ms | 0.88 | 1 |
| libritts | 4679 ms | 139 ms | 0.03 | 1 |
| supertonic | 6344 ms | 1106 ms | 0.17 | 1 |

**Splitting the sentence does work.** Same sentence, cut in two:

| Engine | Whole sentence | First chunk | First audio at | Saved | Underrun? |
| --- | --- | --- | --- | --- | --- |
| kokoro | 4630 ms | 3562 ms audio in 3141 ms | 3141 ms | 1489 ms | No — chunk 2 took 2117 ms against 3562 ms of playback |
| supertonic | 1106 ms | 4409 ms audio in 895 ms | 895 ms | 211 ms | No — 650 ms against 4409 ms |

**The wav file is not the bottleneck.** `$XDG_RUNTIME_DIR` is tmpfs, so writing
the wav is a memcpy into RAM, not disk I/O.

**The governing relationship.** RTF is roughly constant across chunk sizes, so:

```text
first-audio latency  ≈  RTF × duration of the FIRST chunk
```

Any engine with RTF < 1 renders chunk N+1 faster than chunk N plays, so the
pipeline sustains indefinitely once started. Latency is therefore controlled
almost entirely by how small the first chunk is — not by the transport, and not
by total sentence length.

### 10.3 Consequence For R4 — Latency

**Recommendation.** Chunk synthesis below the sentence. Make the first chunk
deliberately small, then grow subsequent chunks to preserve prosody. Keep writing
chunks to tmpfs and keep mpv; a pipe or an in-process ring buffer optimizes
something already measured to be a memcpy.

Do **not** pursue the engine streaming callback as a latency fix for these
models. It was measured and it yields one chunk.

Costs that MUST be accepted or designed around:

- Prosody discontinuity at chunk boundaries, because each chunk receives its own
  utterance-level intonation. Splitting on clause boundaries limits this.
- Word-level highlight timing must be stitched across chunks, since mpv reports
  position per file.
- Gapless playback across chunks becomes a player requirement.

A useful side effect: chunking improves cancellation granularity. Because the
engines cannot abort mid-synthesis, the generation counter of Decision 4 wastes
at most one chunk instead of one whole sentence.

### 10.4 Consequence For R1 — Multiple Voices

The distinction that decides the architecture:

- **Interleaved** — the architect speaks, then the implementer speaks. Different
  voices, one audio stream at a time.
- **Overlapping** — both audible simultaneously.

Overlapping speech is close to unintelligible, so interleaved is assumed to be
the real requirement. **This assumption must be confirmed before it shapes
code**, because it is the difference between one player and a mixer.

**Proposed model: channels.** Each client owns a named channel carrying its own
voice binding and its own queue. A scheduler serializes channels onto the player.

This introduces a genuinely new axis and **modifies Decision 1**. Today
`Speak{replace: All}` clears everything, so the architect agent speaking would
discard the implementer's queued speech. Replacement MUST become
channel-scoped by default, with cross-channel effects explicit:

```text
Speak { channel, text, replace: Replace::{None, Active, Channel, All} }
Stop  { channel, scope:  Scope::{Playback, Queue, All} }
```

Open consequences:

- Do the `read` and `toggle` hotkeys of Decision 13 act on a user channel, the
  active channel, or globally? Stopping should probably be global; pausing is
  less obvious.
- What is the scheduling policy between channels — priority, round-robin, or
  strict arrival order?
- Preview (Decision 14) becomes cleaner as an ephemeral high-priority channel,
  which would simplify the interrupt-then-restore machinery.

### 10.5 Consequence For R2 — Model Residency

`unload` and `reload` today act on *the* single selected engine. R2 requires a
real residency surface, which resolves the §7.13 question of whether `unload`
survives: it does, promoted rather than removed.

**The residency unit is the model, not the voice.** libritts serves 904 voices
from one model, so loading it once serves all of them; kokoro is a separate
model with separate memory. This **refines Decision 10**, which separates
`VoiceId` from `SpeakerIndex` but never names the model:

```text
VoiceId  ->  (ModelId, SpeakerIndex)
```

Residency, memory accounting, and load/unload are all keyed by `ModelId`.

Proposed operations, each with a CLI and API form per the core principle:

```text
model list                 resident models, memory, load state
model load <ModelId>
model unload <ModelId>
```

Memory is the real constraint: the journal records a 249.7 MB peak for a single
resident engine. Multiple resident models multiply that, so a residency policy is
required — explicit-only, or an eviction rule. **Open.** This also makes the
§7.4 worker-count question urgent rather than deferrable, because concurrent
synthesis across voices requires knowing whether a `sherpa-onnx` engine is
safe to call from multiple threads. That is unverified and MUST be tested
before any concurrency is built on it.

### 10.6 Consequence For R3 — Local Voice Sources

This is a **trust boundary change** and §2.3 does not cover it. Two very
different things are easily conflated:

| | Reference clips | Model files |
| --- | --- | --- |
| What | Short `.wav` recordings for zero-shot cloning | ONNX graphs |
| Risk | Low. Audio data parsed as audio. | High. An arbitrary computation graph loaded into the daemon by `onnxruntime`. |
| Already supported | Yes — zipvoice clones from `~/.config/tts/voices/*.wav` | No. All models are Nix store paths from `TTS_MODEL_DIRS`. |

**The per-repo agent-voice requirement is almost certainly satisfied by
reference clips, not model files.** zipvoice already performs zero-shot cloning
from a 5–20 second clip. Making the clip directory configurable per repository
delivers "the architect agent has its own voice" while keeping every executable
model in the Nix store, hash-verified, with nothing fetched at runtime.

Recommended position:

1. Support per-repository **reference clips** — a configurable clone directory.
   Low risk, mostly existing machinery.
2. Treat per-repository **model files** as requiring an explicit, separate trust
   decision. Loading an ONNX graph out of a cloned repository is a code-execution
   surface. If it is ever supported it MUST be opt-in, never implied by opening a
   directory, and confined to an allowlist.

A cloned repository is untrusted input. The current design's guarantee — every
model is a hash-verified store path — is worth more than the convenience of
dropping an `.onnx` file into a project.

### 10.7 Does This Require An In-App Player?

**Not for the stated requirements.** Decision 9 already places the player behind
a trait precisely so this stays reversible, and that decision holds up.

| Need | Player required |
| --- | --- |
| Interleaved voices (R1 as assumed) | One mpv. Channels and a scheduler live above the player. |
| Chunked low-latency playback (R4) | One mpv, using append-style gapless playback. |
| Genuinely overlapping voices | N mpv instances, one per channel — cheap, and keeps pitch-corrected speed. |
| Ducking, crossfade, precise mixing | In-process audio. This is where mpv stops being the right tool. |

Recommendation: keep mpv, add channels, chunk synthesis, and revisit in-process
audio only when overlap or ducking becomes a real requirement. Replacing mpv
would forfeit pitch-corrected speed control, which is the single most-used
feature on the hotkey surface, in exchange for capabilities not yet needed.

### 10.8 Open Questions Raised By This Section

| # | Question | Blocks |
| --- | --- | --- |
| 10-a | Interleaved or genuinely overlapping playback? Decides one player versus a mixer. | R1 architecture |
| 10-b | Channel identity: client-declared name, connection identity, or explicit registration? | Protocol, DEC-1 |
| 10-c | Channel scheduling policy: priority, round-robin, or arrival order? | Scheduler |
| 10-d | Do the `read` and `toggle` hotkeys act per-channel or globally? | DEC-13 |
| 10-e | Chunking policy: fixed first-chunk size, clause boundaries, or adaptive from measured RTF? | R4 |
| 10-f | Is `sherpa-onnx` safe to call concurrently on one engine instance? **Unverified — must be tested.** | §7.4, R1, R2 |
| 10-g | Model residency policy: explicit-only, or eviction under memory pressure? | R2 |
| 10-h | Is per-repository model loading ever permitted, and under what trust rule? | R3, §2.3 |
| 10-i | Does a channel bind one voice, or may it switch voices mid-queue? | DEC-1, DEC-10 |

### 10.9 Effect On The First Slice

Section 8 is **unchanged**. None of this requires new work in the first slice,
but three things there MUST be shaped so this stays cheap:

1. `Operation` variants carry a channel identifier from the start, even if only
   one channel exists. Retrofitting an identifier onto a wire format is a
   breaking change; reserving one is free.
2. `VoiceId` resolves through `(ModelId, SpeakerIndex)` rather than assuming one
   loaded engine.
3. Queue state is keyed by channel internally, even with a single channel.

Do not build channels, residency, chunking, or local voice sources in the first
slice. Reserve the shape; decide the questions in §10.8 first.
