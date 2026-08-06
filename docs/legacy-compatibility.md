# Vroca Legacy Compatibility Matrix

## Status

This is the executable-contract backlog for the Python-to-Rust migration.
Python source is the current behavior evidence. [`vroca.md`](vroca.md) is the
public design record. [`rust-spec.md`](rust-spec.md) is the normative Rust
specification and owns every decision referenced below.

A difference between Python and the documentation is **not** automatically a
Rust requirement. Each is resolved by a decision in `rust-spec.md` §6 or left
Open in §7.

## How To Use This Document

Before a Rust command may be called compatible, it needs a fixture covering all
five of:

| Code | Fixture |
| --- | --- |
| **V** | valid request accepted and acted on |
| **M** | malformed request rejected without terminating the daemon |
| **R** | response bytes match the legacy shape |
| **S** | resulting state change observed |
| **E** | error behavior for the failure path |

The tables below name the *specific* cases each command needs, not just that
five are required. Where a decision intentionally changes behavior, the row
cites it as `→ DEC-<n>` and the change MUST also appear in `vroca.md`.

Finding identifiers (`D#`, `N#`) refer to the mismatch register in
`rust-spec.md` §5.

---

## Transport Baseline

| Item | Python behavior | Rust compatibility target | Finding |
| --- | --- | --- | --- |
| Path | `$XDG_RUNTIME_DIR/tts.sock` | Preserve byte-identically during staged migration. | — |
| Transport | Unix stream socket | Preserve for legacy clients. | — |
| Framing | One request, one response, connection close | Preserve. | — |
| Encoding | UTF-8 in, replacement decoding on invalid bytes | Preserve for legacy. Structured protocol MUST reject invalid UTF-8 instead. | N6 |
| Request limit | One `recv(4096)`; longer input truncated, possibly mid-UTF-8 | Set an explicit limit; over-limit input MUST produce `invalid_request`, never truncation. Limit value is Open (`rust-spec.md` §7.8). | N6 |
| Response write | `conn.send()` with the return value discarded | MUST use a full write. The libritts `catalogue` response measures **101,359 bytes** against roughly 106 KB of effective buffer — under 5% margin, shrinking as voices are added. | N5 |
| Whitespace | `strip()` before dispatch, so `tts say ""` becomes `unknown: say` and surrounding whitespace in text is lost | Decide explicitly; preserve interior text exactly. | N7 |
| Invalid numerics | `float()`/`int()` raise and **terminate the daemon** | MUST become a typed error. Non-negotiable. | D8 |
| Second socket | none | `vroca-v1.sock` bound in slice 2. Permissions Open (§7.3). | → DEC-3 |
| Trust boundary | socket is `0666` inside a `0700` runtime directory | Legacy keeps `0666` for compatibility. The real boundary is the owning user and MUST be documented as such. | D11 |

---

## Legacy Requests

| Request | Python behavior | Decision and migration note | Required fixtures |
| --- | --- | --- | --- |
| `read` | Reads primary selection; **stops** if speech is active. | → **DEC-13**. Kept exactly. Defined as `ReadSelection` when idle, `Stop` when active, evaluated atomically inside the state owner. | V idle-reads-selection; V active-stops; M trailing argument → `invalid_request`; R legacy reply text; S both directions; E empty selection |
| `say TEXT` | Replaces active sentences; **leaves waiting queue intact**. | → **DEC-1**. Intentional change: now clears active *and* waiting. Resolves D2. | V with queue present proves queue cleared; M `say` with no text (N7); R reply; S queue emptied; E empty text after normalization |
| `speak TEXT` | Alias of `say`. | → **DEC-1**. Alias retained during migration. `speak` is canonical in the CLI. | V aliasing proven identical to `say`; M as above; R; S; E |
| `queue TEXT` | Appends when active; **replaces** when idle or paused. | → **DEC-1**. Intentional change: always appends. Resolves D4. | V append while playing; V append while **paused** proves D4 gone; V while idle; M no text; R reply includes queue length; S; E |
| `stop` | Stops player, clears active sentences and cache; waiting queue survives. Clears `spans`. | → **DEC-1**. `Stop { scope: Playback }`. Queue survival preserved. | V while playing; V while idle; R `stopped`; S cache and spans cleared, queue retained; E none |
| `clear` | Clears waiting queue then calls stop. | → **DEC-1**. `Stop { scope: All }`. | V with queued items; R; S queue and playback both cleared; E while idle |
| `skip` | Drops the **next waiting** item; stops if none. | → **DEC-1**. Intentional change: abandons the **current** item and advances. Resolves D3. | V with current + waiting proves current abandoned; V with current only; V while idle; R; S; E |
| `toggle` | Pauses or resumes active speech; `idle` when nothing active. | → **DEC-13**. Kept exactly, atomic. | V pause; V resume; V idle; R `paused`/`resumed`/`idle`; S; E |
| `next` | Plays next sentence within the active item. | Sentence-level meaning preserved. | V mid-item; V at last sentence (clamped); R `n/total`; S index; E while idle → `idle` |
| `back` | Plays previous sentence within the active item. | Sentence-level meaning preserved. | V mid-item; V at first sentence (clamped); R; S; E while idle |
| `faster` | Adds 0.15, capped at 3.0. | Typed speed with defined rounding. | V normal; V at cap; R `speed N.NN`; S persisted to prefs; E none |
| `slower` | Subtracts 0.15, floored at 0.5. | Typed speed with defined rounding. | V normal; V at floor; R; S; E none |
| `speed FLOAT` | Clamps 0.5–3.0. **Malformed float kills the daemon.** | Invalid input MUST be safe. | V in range; V clamped high and low; **M `speed abc`, `speed`, `speed 1 2` — daemon MUST survive**; R; S; E `invalid_argument` |
| `voice INTEGER` | Sets engine-local speaker index. **Malformed integer kills the daemon.** Clears cache. | → **DEC-10**. Public identity becomes an engine-qualified `VoiceId`; numeric form still resolves against the catalogue. | V numeric legacy form; V string `VoiceId`; **M `voice abc`, `voice` — daemon MUST survive**; V out-of-range → `not_found`, never silent fallback to 0; R; S cache cleared; E |
| `engine NAME` | Selects an available engine, lazily unloads the old one. **Does not clear the render cache**, so previous-engine audio keeps playing. | Bug fix required: engine change MUST clear the cache. Resolves N10. | V switch; V same engine twice; V unknown name; V `zipvoice` with no reference wavs; R; S **cache cleared**; E |
| `aligner NAME` | Sets `asr` or `energy`. | Capability and fallback behavior to define. | V both values; M unknown name; R; S persisted; E |
| `font_size INTEGER` | Clamps 12–72. **Malformed integer kills the daemon.** Panel slider is only 14–48. | → **DEC-17**. Clamping MUST be reported, not silent. Panel range must match the daemon. Resolves N19. | V in range; V clamped both ends with report; **M `font_size abc` — daemon MUST survive**; R; S; E |
| `words_visible INTEGER` | Clamps 1–15. **Malformed integer kills the daemon.** Panel slider is only 1–9. | → **DEC-17**. As above. Resolves N20. | V in range; V clamped both ends with report; **M malformed — daemon MUST survive**; R; S; E |
| `position NAME` | Sets `bottom`, `top`, or `center`. In both RSVP modes, `bottom` and `center` render identically. | Resolve N21: either make `bottom` mean bottom in RSVP, or document that RSVP supports two positions. Currently undecided. | V all three values in `subtitle`; V all three in `rsvp` and `scroll_rsvp` documenting actual anchor; M unknown name; R; S; E |
| `mode` | Cycles the separate `tts-mode` file. | → **DEC-7**. Daemon becomes sole owner; mode moves into `prefs.json`; `tts-mode` becomes a read-only mirror. Resolves N14. | V full cycle `subtitle → rsvp → scroll_rsvp → off → subtitle`; V from an unknown file value; R new mode name; S prefs updated **and** mirror refreshed; E unwritable mirror |
| `reset`, `reset_prefs` | Restores some prefs, writes mode `subtitle`, **leaves engine unchanged**, **does not clear the cache**. | → **DEC-15**. Intentional change: resets everything including engine, stops playback, clears cache. Resolves N8 and N9. | V both verbs identical; V engine reset proven; V cache empty after; R; S every field at default; E |
| `status` | Returns pretty-printed JSON snapshot, 20 fields. | → **DEC-8**, **DEC-17**. Fields preserved; gains a schema version. | V full field set; V while idle; V while playing; R exact field names and JSON shape; S none; E daemon degraded still answers |
| `catalogue` | Returns JSON voice metadata for the current engine. **101,359 bytes for libritts.** | → **DEC-10**, **DEC-11**. Shape preserved or versioned; entries gain stable `VoiceId` and separated traits. | V per engine (kokoro named, libritts/supertonic generated, zipvoice from filenames); **V large-response integrity — full 101 KB body received (N5)**; R; S none; E engine unloaded (N18) |
| `preview INTEGER` | Synthesizes a fixed sentence into the **main** player, interrupting the reading and never restoring it. **Malformed integer kills the daemon.** | → **DEC-14**. Intentional change: interrupts then restores sentence and pause state. | V during playback restores position; V while paused stays paused; V while idle; **M malformed — daemon MUST survive**; V out of range → `not_found` with playback undisturbed; R; S; E failing preview still restores |
| `unload` | Drops the selected engine from memory. | Usefulness after engine redesign is Open (`rust-spec.md` §7.13). Note the panel's voice list breaks afterwards (N18). | V; R `unloaded <engine>`; S `loaded` false, `voices` null; E subsequent operation reloads cleanly |
| `reload` | Rebuilds the engine synchronously; returns timing. | Long-operation response and cancellation to define. | V success with timing; V failure message; R; S; E `engine_failure` |
| `quit` | Sends `bye`, quits mpv, then `os._exit(0)`. Exit status 0 with `Restart=on-failure` **permanently stops the user service**. Leaves sockets, temp directories, and children behind. | → **DEC-9**. Replaced by graceful shutdown: stop accepting, stop playback, terminate children, remove owned sockets and temp audio. Exit status must not defeat the restart policy. Resolves N2, N3, D9. | V reply `bye` before exit; S no orphan `mpv`; S no leaked temp directory; S both sockets removed; E shutdown while synthesizing |
| unknown | `unknown: <cmd>` | Preserve the reply shape. | V arbitrary garbage; V empty request; V 8 KB request (N6); R exact prefix; E daemon survives all |

---

## Client Surface Baseline

The deployed shell client lives in `~/nix-dotfiles/home/tts.nix` and is a
separate ownership boundary. These are its observed behaviors.

| Behavior | Python client | Rust target | Finding |
| --- | --- | --- | --- |
| Argument forwarding | Remaining arguments forwarded only for `say`, `speak`, `queue`. `tts speed 1.2` sends bare `speed`. | The Rust CLI MUST NOT copy this limitation. | D5 |
| No arguments | Bare `tts` means `read`. | Undocumented. Decide whether to keep; document either way. | N22 |
| `tts log` | Client-side `journalctl --user -u tts -u tts-overlay -f`. Never reaches the socket. | Undocumented. Keep as a CLI-only subcommand. | N22 |
| Missing daemon | Prints an error and exits 1. Does **not** auto-start. | The panel does the opposite (spawns `systemctl --user start tts`). Policy is Open (`rust-spec.md` §7.2). | N23, D6 |

CLI fixtures MUST cover argument parsing, standard output, standard error, and
exit status **separately** from socket fixtures.

---

## File And Ownership Baseline

| Path | Current writers | Rust target | Finding |
| --- | --- | --- | --- |
| `tts-state.json` | daemon only, atomic | Gains schema version. Written on state change, **not** on every mpv position tick. Must be detectably stale after daemon exit. | N16, → DEC-8 |
| `tts-mode` | **daemon and panel** | Daemon only, as a read-only mirror of a `prefs.json` field. | N14, → DEC-7 |
| `prefs.json` | daemon only, atomic; corrupt file silently reverts every setting | Gains schema version, explicit migration, unknown-field preservation, and quarantine-plus-report on corruption. | → DEC-17 |
| `tts-mpv.sock` | daemon; path unconditionally unlinked at startup | Per-instance path. No daemon may unlink a path it does not own. | N1, → DEC-9 |
| `tts-<rand>/` | daemon; never removed (61 observed) | Removed on shutdown. | N2, → DEC-9 |
| `tts.sock` | daemon; unconditionally unlinked at startup | MUST prove no live owner exists before unlinking. | D7 |

---

## State And Preference Fixtures

Fixtures MUST also cover:

- missing, malformed, and unknown preference fields;
- every current `prefs.json` field, plus the new `overlay_mode` (D7) and
  `schema` (DEC-17);
- every current `tts-state.json` field;
- migration from the unversioned Python format to schema 1;
- unknown fields surviving a read-modify-write cycle (downgrade safety);
- a corrupt preferences file being quarantined, defaults applied, and the
  failure reported in health state;
- separate overlay mode file compatibility during migration;
- atomic-write behavior under interruption;
- malformed client messages on both sockets;
- **duplicate daemon start** — the second MUST NOT orphan the first (D7, N1);
- **player death** — degraded health, not a silent stall (N4);
- **clean shutdown** — no orphan `mpv`, no leaked temp directory, no stale
  socket (N2, N3, D9).

The last three are not hypothetical. All three were observed on the development
host; see `rust-spec.md` §4.
