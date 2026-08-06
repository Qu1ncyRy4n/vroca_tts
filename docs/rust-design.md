# Vroca Rust Design

## Status

This document is the discussion draft for the Rust implementation. It expands
the architecture in [`vroca.md`](vroca.md), which remains the design of record.

Sections marked **Decided** record choices already made with the user. Sections
marked **Proposed** are recommendations, not locked decisions. Sections marked
**Open** must be discussed before their choice shapes implementation.

## Purpose

Build a Linux-first Rust implementation of Vroca without taking away the stable
Python path too early. The Rust implementation should improve process safety,
API clarity, testability, and client symmetry. It should not blindly reproduce
Python accidents.

## Decided Direction

1. Migration is a staged replacement.
2. Python remains usable until Rust is stable and passes a reviewed parity gate.
3. Behavioral parity is the first milestone. Approved design improvements may
   intentionally differ from Python.
4. Linux is the first complete platform.
5. The local interface should be public enough for other local programs.
   Distribution and untrusted network access are not current goals.
6. Every GUI capability must have one underlying typed operation. The same
   capability must be available through the CLI and local interface.
7. CLI actions use subcommands. Flags and options qualify those actions.
8. The existing plaintext Unix socket protocol remains available during
   migration. A structured, versioned protocol may live beside it.
9. Defaults are opinionated and should be revised through dogfooding.
10. Local engines are the priority. Remote engines remain an extension point.
11. TTS-aware Markdown notation is a later design topic. The first architecture
    must leave room for it without inventing its syntax now.

## Non-Goals For The First Rust Milestone

- Exposing Vroca directly over TCP or the public internet.
- Designing authentication for untrusted remote callers.
- Shipping a remote engine marketplace or general plugin loader.
- Removing Python before Rust is stable.
- Preserving undocumented crashes, races, or misleading command behavior.
- Finalizing TTS-aware Markdown notation.
- Completing macOS support.

## Current System

The deployed system has four cooperating programs:

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
          +---------------> tts-mode
```

The daemon owns synthesis, queueing, playback commands, preferences, and most
runtime state. The overlay polls `tts-state.json` and `tts-mode`. The panel uses
the socket for most actions but writes `tts-mode` directly.

Deployment lives in the separately owned `~/nix-dotfiles` repository:

- `home/tts.nix` packages the Python programs and models, defines `tts` and the
  GUI executables, and defines `tts.service` plus `tts-overlay.service`.
- `home/cosmic.nix` defines the Vroca hotkeys.
- `flake.nix` includes Vroca as a path input.

Rust deployment work must change those files only in a separately approved
deployment task.

### Existing Public Surfaces

- Executables: `tts`, `tts-daemon`, `tts-panel`, and `tts-overlay`.
- Legacy socket: `$XDG_RUNTIME_DIR/tts.sock`.
- mpv socket: `$XDG_RUNTIME_DIR/tts-mpv.sock`.
- Runtime snapshot: `$XDG_RUNTIME_DIR/tts-state.json`.
- Overlay mode: `$XDG_RUNTIME_DIR/tts-mode`.
- Preferences: `$XDG_CONFIG_HOME/tts/prefs.json`.
- Clone voices: `$XDG_CONFIG_HOME/tts/voices/*.wav`.
- Optional remote configuration: `$XDG_CONFIG_HOME/tts/env` through systemd.
- Nix option: `tts.enable` in the deployment repository.
- Hotkeys defined by the deployment repository.

### Current Preference Fields

The Python daemon writes an unversioned JSON object containing:

```text
engine, voice, speed, aligner, font_size, words_visible, position
```

Overlay mode is stored separately. Unknown fields, migration behavior, and
corrupt-value handling are not specified.

### Current Runtime Snapshot Fields

The Python daemon writes an unversioned JSON object containing:

```text
sentence, index, total, paused, speed, rendered, rendering, loaded,
voice, word, engine, engines, aligner, queue_len, font_size,
words_visible, position, voices, last_render_ms, avg_render_ms
```

The overlay and panel treat this object as an API even though it has no schema
or version.

## Known Documentation And Behavior Drift

These differences need decisions. They are not automatic Rust requirements.

1. No `/tmp/tts-speak` spool watcher exists. Whether a spool remains useful
   after the public local API is designed is open.
2. Documentation says `say` clears the active queue. Python replaces the active
   sentences but leaves waiting queue items intact.
3. Documentation says `skip` skips the current queued item and advances. Python
   removes the next waiting item while current playback continues. With no
   waiting item, it stops playback.
4. `queue` while paused replaces the active speech and starts the new text.
5. The deployed shell client forwards all text for `say`, `speak`, and `queue`,
   but drops additional arguments for commands such as `tts speed 1.2`.
6. The panel can get stuck on “starting” because it records one start attempt,
   ignores the result, and does not expose a retry or error state.
7. Daemon startup unconditionally unlinks socket paths. A duplicate process can
   make a healthy daemon unreachable.
8. Invalid numeric command arguments can terminate the daemon.
9. Daemon crashes can leave child `mpv` processes alive.
10. The Python remote engine already implements an experimental
    OpenAI-compatible HTTP request, although remote engines are not urgent.
11. The socket is described as mode `0666`, but the enclosing per-user runtime
    directory normally limits access. The actual trust boundary is not stated.
12. Several failures are swallowed. Callers cannot distinguish invalid input,
    engine failure, playback failure, and success from a stable error shape.

## Legacy Compatibility Matrix

[`legacy-compatibility.md`](legacy-compatibility.md) is the handoff artifact
for old socket behavior. It lists each legacy request, Python behavior, known
drift, and the fixture needed before Rust replaces Python. Update that matrix
when a decision intentionally changes compatibility.

## Core Design Principle

One typed application operation must underlie every user-facing action:

```text
                 typed operation
                 /      |      \
             CLI     GUI client  local IPC client
                 \      |      /
                    daemon
```

The GUI must not write daemon-owned files or duplicate business rules. The CLI
must not construct ad hoc protocol text outside the shared client. The daemon
must validate every operation before changing state.

This rule does not mean every operation needs a CLI flag. Actions should be
subcommands. Flags should supply inputs or select output formatting.

## IPC Primer

Inter-process communication, or IPC, lets separate programs exchange data. In
Vroca, the panel and CLI are separate processes from the daemon. A Unix domain
socket gives them a local connection identified by a filesystem path.

The current exchange is:

1. Client connects to `tts.sock`.
2. Client writes one plaintext command.
3. Daemon reads at most 4096 bytes once.
4. Daemon writes one plaintext response.
5. Connection closes.

The connection boundary acts as message framing. There is no request ID,
protocol version, structured error, event subscription, or large-message rule.
This is simple, but parsing, evolution, and diagnostics are weak.

IPC does not imply networking. Unix sockets can remain local to one machine and
one user. TCP, remote access, authentication, and encryption are separate later
decisions.

## Operation Model

### Proposed

Define one exhaustive Rust enum for daemon operations. Parse both the legacy
protocol and the future structured protocol into that enum. Return one typed
result or typed error.

Conceptual shape:

```rust
enum Operation {
    Speak { text: String, policy: SpeakPolicy },
    Enqueue { text: String },
    Stop,
    Pause,
    Resume,
    SeekSentence { direction: Direction },
    SetSpeed { speed: PlaybackSpeed },
    SetVoice { voice: VoiceId },
    SetEngine { engine: EngineId },
    SetAligner { aligner: AlignerId },
    SetOverlayMode { mode: OverlayMode },
    SetOverlayPosition { position: OverlayPosition },
    SetFontSize { points: FontPoints },
    SetContextWords { count: ContextWordCount },
    ClearQueue,
    SkipCurrent,
    ResetPreferences,
    GetStatus,
    GetCatalogue,
    PreviewVoice { voice: VoiceId },
    LoadEngine,
    UnloadEngine,
    Shutdown,
}
```

This example exposes questions. It is not final API code.

### Open Operation Questions

1. Should `read` remain a context-dependent toggle, or split into explicit
   `read-selection` and `stop` operations?
2. Should `toggle` remain public, or should clients query state and issue
   explicit `pause` or `resume`?
3. Does `say` clear only active speech, both active and waiting speech, or use an
   explicit replacement policy?
4. Does `skip` mean skip the current sentence, current speech item, or next
   waiting item?
5. Is a queue item one submitted text, one paragraph, or one sentence list?
6. Does changing engine or voice restart current speech, affect only future
   speech, or reject while active?
7. Does preview temporarily interrupt playback, mix with it, or use a separate
   preview player?
8. Should shutdown be available to every local client, or only service control?
9. Which operations are durable preferences and which are session-only state?
10. Should reset affect all preferences or accept named preference groups?

## CLI Shape

### Proposed

Keep `tts` as the main executable. Give each operation an explicit subcommand.
Use stable machine-readable output when requested.

```text
tts speak TEXT
tts queue TEXT
tts stop
tts pause
tts resume
tts next
tts back
tts speed set 1.25
tts speed increase
tts speed decrease
tts engine list
tts engine set kokoro
tts voice list
tts voice set VOICE
tts overlay mode subtitle
tts overlay position bottom
tts status [--output human|json]
tts daemon shutdown
tts log --follow
```

GUI controls must call the same client operations. CLI parsing belongs at the
edge. Domain code must not depend on terminal strings.

### Open CLI Questions

1. Is the verb `speak` canonical, with `say` retained as an alias?
2. Should speed use `tts speed 1.25` or the more explicit `tts speed set 1.25`?
3. Should voice selection use a stable string ID instead of engine-local
   numeric speaker indexes?
4. Should human output go to standard output and errors to standard error, with
   stable process exit codes?
5. Is JSON output per-command through `--output json`, or one global `--json`?
6. Should the CLI auto-start the service when unavailable, or report a clear
   error and leave startup to systemd?

## Protocol Evolution

### Legacy Compatibility

The Rust daemon should initially accept one legacy plaintext command per
connection. Invalid commands must return an error and must never terminate the
daemon. Compatibility tests should capture every documented legacy command.

### Proposed Structured Protocol

Use UTF-8, newline-delimited JSON over a second Unix socket or an explicitly
distinguishable connection preface. Start with one request and one response per
connection. JSON is larger than a binary format but easy to inspect, script,
test, and evolve.

Conceptual request:

```json
{"protocol":"vroca","version":1,"id":17,"operation":"set_speed","speed":1.25}
```

Conceptual response:

```json
{"protocol":"vroca","version":1,"id":17,"ok":true,"result":{"speed":1.25}}
```

Conceptual error:

```json
{"protocol":"vroca","version":1,"id":17,"ok":false,"error":{"code":"invalid_argument","message":"speed must be between 0.5 and 3.0"}}
```

### Open Protocol Questions

1. Use a second path such as `vroca-v1.sock`, or detect legacy versus JSON on
   the existing socket?
2. Keep one request per connection, or allow persistent connections?
3. Add subscriptions for state changes, or keep clients polling snapshots?
4. Is newline framing sufficient, or should messages use a length prefix?
5. What is the maximum request size, especially for long text?
6. Are unknown JSON fields ignored for forward compatibility or rejected to
   expose mistakes?
7. Is protocol version negotiated per connection or carried on every request?
8. Are request IDs integers, strings, or absent for single-request connections?
9. Does the public local API promise source compatibility, wire compatibility,
   or both?
10. Should the socket be accessible only to the owning user? The recommended
    local default is owner-only access.

## Runtime State

### Proposed

The daemon is the sole mutable-state owner. It publishes immutable snapshots.
The panel and overlay request changes through the client API.

Separate these concepts:

- Preferences: durable user choices.
- Session state: selected engine, loaded models, and overlay visibility for the
  current daemon session where not durable.
- Playback state: active item, sentence, word, pause state, and timing.
- Queue state: current item and waiting items.
- Capability state: available engines, voices, aligners, and platform features.
- Health state: daemon, engine, player, and last recoverable error.

Use a schema version for persisted preferences and published snapshots. Write
preferences atomically. Treat malformed files as visible recoverable errors,
not silent defaults, unless the recovery behavior is explicitly chosen.

### Open State Questions

1. Which current preference fields remain durable?
2. Does overlay mode move into `prefs.json`?
3. Is the JSON state file still a public interface, or only a compatibility
   artifact while clients move to IPC?
4. Should state publication be event-driven over IPC, atomic file snapshots, or
   both?
5. Must queue contents survive daemon restart?
6. Must active playback resume after a crash or login restart?
7. Should the state expose full submitted text, current sentence only, or both?
   Full text may contain private data.
8. How long should errors remain visible in state?
9. Should unavailable engines remain listed with reasons, or disappear as they
   do now?

## Daemon Lifecycle

### Proposed Invariants

- Exactly one daemon owns the public socket.
- Startup never unlinks a socket until it proves no live owner exists.
- Every accepted request produces a bounded response or clean disconnect.
- Malformed input cannot terminate the daemon.
- Slow synthesis cannot block status, stop, or pause control.
- Shutdown stops accepting work, stops playback, terminates owned children,
  removes owned sockets, and removes temporary audio.
- A crash does not leave reachable stale sockets or unmanaged players.
- systemd restart behavior cannot create two public owners.

### Open Lifecycle Questions

1. Use systemd socket activation, daemon-owned socket creation, or both?
2. Keep `mpv` as a child process, embed libmpv, or use another audio backend?
3. Should the daemon exit when `mpv` dies, restart `mpv`, or enter degraded
   health while retaining control operations?
4. What request timeout should clients use for cheap control operations?
5. Should synthesis requests return after acceptance or after first audio is
   ready?
6. How should in-flight synthesis be cancelled on stop, engine change, or
   shutdown?

## Concurrency Model

### Proposed

Use one authoritative state task. Other tasks submit typed messages to it.
Synthesis workers perform expensive work outside the state owner, then return
results tagged with generation IDs so stale work cannot overwrite newer state.

Keep playback control responsive while synthesis runs. Bound the work queue and
prefetch count. Make cancellation explicit.

### Open Concurrency Questions

1. Use Tokio, standard threads and channels, or a smaller async executor?
2. How many synthesis workers may run per engine?
3. Is prefetch depth fixed, configured, or adapted from render speed?
4. Can two independent clients submit speech concurrently? If so, what ordering
   and replacement rules apply?
5. What backpressure occurs when the queue or request rate is too large?

## Engine Interface

### Proposed

Local and remote engines implement one conceptual interface. The interface
describes capabilities instead of assuming every engine supports every option.

```text
identity and metadata
available voices
supported languages and controls
load and unload
synthesize utterance
cancel synthesis when supported
health and diagnostics
```

Engine output should carry samples, sample rate, channel count, and optional
timing metadata. Alignment should be a separate interface so synthesis engines
do not own overlay policy.

### Open Engine Questions

1. Use `sherpa-onnx` through a Rust crate, C API bindings, or a narrow local FFI
   wrapper?
2. Are engines compiled into the daemon, dynamically loaded, or registered by
   configuration? Compiled-in engines are the safest first milestone.
3. Is `EngineId` a stable string such as `kokoro`, or a namespaced identifier
   such as `local.kokoro.v1`?
4. Is `VoiceId` stable across model upgrades? Numeric speaker IDs alone are not
   stable enough for a public API.
5. Should model loading be lazy, eager for the selected engine, or configurable?
6. Should speed be playback-only, synthesis-only, or a typed choice per engine?
7. Which capability differences are errors, warnings, or automatic fallbacks?
8. Does voice cloning remain an engine, or become a voice-source capability?

### Remote Engines

Remote engines are not urgent. Preserve a provider boundary now so adding one
does not change daemon, CLI, GUI, or protocol operations later. Start with local
engines and local defaults.

When remote work begins, an OpenAI-compatible adapter is a reasonable first
adapter. Credentials must remain outside the repository and Nix store. Network
timeouts, retries, rate limits, privacy, streaming, cancellation, and provider
error mapping require a separate review.

## Text Pipeline And Future TTS Markdown

### Proposed

Separate input text from derived speech units:

```text
source document
  -> format parser
  -> normalized speech document
  -> utterances
  -> synthesis requests
  -> audio and timing
```

The current regex Markdown cleanup is compatibility evidence, not a sufficient
long-term parser. A typed intermediate document would allow later TTS-aware
notation for pauses, pronunciation, voice, language, emphasis, skipping, and
non-spoken annotations.

### Open Text Questions

1. Is TTS notation an extension of CommonMark, a fenced directive syntax,
   front matter, or a separate sidecar document?
2. Must ordinary Markdown remain valid unchanged?
3. Are directives allowed to select engines or remote providers, or only speech
   presentation?
4. What trust rules apply when a document requests files, voices, or networked
   engines?
5. Is sentence segmentation deterministic and exposed as a public operation?
6. Must queue items retain original Markdown for display or only normalized
   speech text?

## GUI And Overlay

### Proposed

Keep the control panel and non-focus-stealing overlay as separate concerns, even
if they share a GUI crate. Both use the public client API. Neither writes daemon
state directly.

Every interactive GUI feature needs:

1. a typed operation or query;
2. a CLI representation;
3. public client API documentation;
4. success, error, and unavailable states;
5. a test below the widget layer.

### Open GUI Questions

1. Use GTK4 and gtk4-layer-shell in Rust, or another Linux GUI stack?
2. Keep panel and overlay as separate processes or one process with two
   surfaces?
3. Should the panel start the daemon, ask systemd to start it, or only report
   that it is unavailable?
4. Should GUI controls update optimistically or wait for daemon confirmation?
5. Should the overlay poll, subscribe to state events, or use shared memory?
6. Which accessibility guarantees are required for keyboard navigation, screen
   readers, contrast, motion, and reduced distraction?

## Crate And Code Structure

### Proposed Baseline

Do not create crates only to mirror directories. Split at dependency and public
API boundaries. A plausible starting workspace is:

```text
rust_impl/
  Cargo.toml
  crates/
    vroca-core/       typed domain, operations, state transitions
    vroca-protocol/   legacy and structured wire formats
    vroca-client/     public local client API
    vroca-engine/     engine and aligner traits
    vroca-daemon/     lifecycle, orchestration, workers
    vroca-cli/        command-line edge
    vroca-gui/        shared GTK client code
  bins/
    tts-daemon/
    tts-panel/
    tts-overlay/
```

This may be too many crates for the first slice. One alternative is a single
`vroca` library with modules plus four binary targets, extracting crates only
when dependency boundaries become real.

### Open Structure Questions

1. Start with one library crate or the multi-crate workspace above?
2. Are engine implementations separate crates to isolate native dependencies?
3. Is the public client API synchronous, asynchronous, or both?
4. Does the protocol crate expose Serde wire types directly, or convert to
   separate stable domain types?
5. Is any Python interoperability needed during migration? Staged replacement
   does not automatically require PyO3.
6. What is the policy for `unsafe` FFI code and native callback boundaries?

## Important Names

Names become public concepts. Decide these before they spread:

1. `Operation`, `Command`, or `Request` for an action sent to the daemon.
   Recommendation: `Operation` in the domain and `Request` on the wire.
2. `DaemonSnapshot`, `RuntimeState`, or `Status` for published state.
   Recommendation: `RuntimeSnapshot` for data; `Health` for component health.
3. `SpeechItem`, `QueueItem`, `Document`, or `Utterance` for submitted text.
   Recommendation: `SpeechItem` contains source text and derived utterances.
4. `Utterance` or `Sentence` for one synthesis unit. Recommendation:
   `Utterance`, because future notation may split text differently from grammar.
5. `VoiceId` versus `SpeakerId`. Recommendation: public `VoiceId`; engine-local
   `SpeakerIndex` where a model exposes only numbers.
6. `EngineId` versus `EngineName`. Recommendation: validated `EngineId` newtype.
7. `OverlayMode::ScrollRsvp` spelling and public wire spelling
   `scroll_rsvp` versus `scroll-rsvp`.
8. Product binary names. Compatibility favors keeping `tts`, `tts-daemon`,
   `tts-panel`, and `tts-overlay`; Rust crate names can use `vroca-*`.

## Error Model

### Proposed

Define stable public error categories without exposing internal library errors:

```text
invalid_request
invalid_argument
unsupported_operation
unavailable
conflict
not_found
busy
timeout
engine_failure
player_failure
internal
```

Errors carry a safe human message and optional structured details. Internal
causes go to logs. One bad client request never ends the server loop.

### Open Error Questions

1. Which errors are safe to retry?
2. Should accepted long operations return an operation ID for later status?
3. How are partial failures represented, such as speech synthesized but playback
   unavailable?
4. Should the daemon retain a bounded diagnostic history for the panel?

## Testing And Parity Gate

### Required Test Layers

- Domain tests for state transitions and queue semantics.
- Parser tests for every legacy command, malformed input, limits, and aliases.
- Fixtures derived from the legacy compatibility matrix before each command is
  declared compatible.
- Structured protocol round-trip and compatibility tests.
- Preference and snapshot schema migration tests.
- Fake-engine tests for success, delay, failure, and cancellation.
- Fake-player tests for playback events and child failure.
- Lifecycle tests using temporary runtime and config directories.
- CLI tests for arguments, output, errors, and exit status.
- Client and daemon integration tests over temporary Unix sockets.
- GUI logic tests below the widget layer.
- Focused Linux smoke tests for GTK layer-shell and real audio.

### Proposed Parity Gate

Rust may replace Python in deployment only when:

1. every approved operation exists in the typed API, CLI, and relevant GUI;
2. legacy protocol compatibility tests pass;
3. preference migration and rollback behavior are tested;
4. duplicate startup, malformed input, player death, engine failure, and clean
   shutdown tests pass;
5. the overlay and panel work on the target Linux desktop;
6. dogfooding confirms normal reading, queueing, seeking, speed, voices, modes,
   and hotkeys;
7. Nix builds and service integration are reviewed in `~/nix-dotfiles`;
8. switching back to Python remains documented and tested for the rollout.

## Migration Stages

1. Lock semantics and resolve the open decisions needed for the first slice.
2. Add the Rust toolchain and reviewed workspace shape.
3. Implement typed operations, state transitions, and protocol parsers with no
   audio dependency.
4. Implement the public client and CLI against a fake daemon.
5. Implement daemon lifecycle with fake engine and fake player.
6. Add one local synthesis engine and real playback.
7. Add remaining approved local engines and alignment.
8. Add overlay and panel through the shared client.
9. Run parity tests and dogfood without changing default deployment.
10. Update `~/nix-dotfiles` in a separately approved deployment change.
11. Observe the Rust service with a documented Python rollback.
12. Retire Python only in a later decision and change.

## Decision Queue

Resolve decisions only as they become necessary, but before code makes them
expensive. Suggested discussion order:

1. Exact semantics of `speak`, `queue`, `skip`, `read`, pause, and preview.
2. Domain names: operation, speech item, utterance, voice ID, runtime snapshot.
3. One library crate versus an initial multi-crate workspace.
4. Standard threads versus Tokio.
5. mpv child process versus embedded libmpv versus another player.
6. Legacy and structured socket coexistence strategy.
7. Polling snapshots versus event subscriptions.
8. Preference and runtime-state schemas.
9. Engine binding strategy and first engine.
10. GTK process layout and daemon-start behavior.
11. TTS-aware Markdown notation.
12. Remote provider and network design.
