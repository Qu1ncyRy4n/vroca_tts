# Vroca Legacy Compatibility Matrix

## Status

This is the executable-contract backlog for the Python-to-Rust migration.
Python source is the current behavior evidence. `docs/vroca.md` is the public
design record. A difference is not automatically a Rust requirement.

Before a Rust command is called compatible, add a fixture that covers its valid
request, malformed request, response, state change, and error behavior. Mark
an intentional behavior change with its decision and migration note.

## Transport Baseline

| Item | Python behavior | Rust compatibility target |
| --- | --- | --- |
| Path | `$XDG_RUNTIME_DIR/tts.sock` | Preserve during staged migration. |
| Transport | Unix stream socket | Preserve for legacy clients. |
| Framing | One request, one response, connection close | Preserve initially. |
| Encoding | UTF-8 input, replacement decoding on invalid bytes | Decide whether structured protocol differs. |
| Request limit | Daemon reads one 4096-byte chunk | Set an explicit limit. Test over-limit input. |
| Response | Plain text or JSON for selected commands | Preserve legacy shapes where required. |
| Invalid input | Can terminate daemon for several numeric commands | Must become a typed error. |

## Legacy Requests

| Request | Python behavior | Documentation or migration note | Rust fixture status |
| --- | --- | --- | --- |
| `read` | Reads primary selection, or stops if speech is active. | Decide explicit `read-selection` and `stop` behavior. | Required |
| `say TEXT` | Replaces current sentences; leaves waiting queue intact. | Docs say it clears the queue. Decide intended replacement policy. | Required |
| `speak TEXT` | Alias of `say`. | Decide canonical CLI spelling; retain alias during migration. | Required |
| `queue TEXT` | Appends when active; replaces paused speech. | Paused behavior is likely accidental. | Required |
| `stop` | Stops player, clears active sentences and cache; does not clear waiting queue. | Decide whether stop clears queue. | Required |
| `clear` | Clears waiting queue and calls stop. | Candidate explicit “clear all” operation. | Required |
| `skip` | Drops next waiting item; otherwise stops. | Docs imply skipping current item. Decide semantics. | Required |
| `toggle` | Pauses or resumes active speech. | Keep as convenience; consider explicit pause and resume. | Required |
| `next` | Plays next sentence within active item. | Keep sentence-level meaning if approved. | Required |
| `back` | Plays previous sentence within active item. | Keep sentence-level meaning if approved. | Required |
| `faster` | Adds 0.15 to speed, capped at 3.0. | Define typed speed and rounding. | Required |
| `slower` | Subtracts 0.15 from speed, floored at 0.5. | Define typed speed and rounding. | Required |
| `speed FLOAT` | Sets clamped speed. Malformed float can kill daemon. | Invalid input must be safe. | Required |
| `voice INTEGER` | Sets engine-local numeric speaker index. Invalid integer can kill daemon. | Decide stable public `VoiceId`. | Required |
| `engine NAME` | Selects available engine and lazily unloads the old engine. | Preserve named local engine IDs initially. | Required |
| `aligner NAME` | Sets `asr` or `energy`. | Define capability and fallback behavior. | Required |
| `font_size INTEGER` | Clamps from 12 through 72. | Durable preference decision pending. | Required |
| `words_visible INTEGER` | Clamps from 1 through 15. | Durable preference decision pending. | Required |
| `position NAME` | Sets `bottom`, `top`, or `center`. | Durable preference decision pending. | Required |
| `mode` | Cycles a separate `tts-mode` file. | Rust daemon should own this state. | Required |
| `reset`, `reset_prefs` | Restores selected prefs and writes mode `subtitle`. | Define preference groups and migration. | Required |
| `status` | Returns pretty JSON runtime snapshot. | Preserve snapshot fields until structured API replaces it. | Required |
| `catalogue` | Returns JSON voice metadata. | Preserve output shape or version it. | Required |
| `preview INTEGER` | Plays fixed text through main player and interrupts current audio. | Decide preview playback policy. | Required |
| `unload` | Drops selected engine from memory. | Preserve only if useful after engine design. | Required |
| `reload` | Reloads selected engine synchronously. | Define long-operation response and cancellation. | Required |
| `quit` | Sends `bye`, quits mpv, then exits daemon immediately. | Replace with graceful shutdown. | Required |

## CLI Baseline

The deployed shell client in `~/nix-dotfiles/home/tts.nix` passes all remaining
arguments only for `say`, `speak`, and `queue`. Other commands are sent without
arguments. The Rust CLI must not copy this limitation.

Fixtures must cover command parsing, standard output, standard error, and exit
status separately from socket fixtures.

## State And Preference Baseline

Fixtures must also cover:

- missing, malformed, and unknown preference fields;
- every current `prefs.json` field;
- every current `tts-state.json` field;
- separate overlay mode file compatibility;
- atomic-write and restart behavior;
- malformed client messages;
- duplicate daemon start, stale socket, player failure, and clean shutdown.
