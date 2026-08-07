# Vroca Roadmap

Where the project is and what happens next. [`rust-spec.md`](rust-spec.md) owns
the decisions; this is the sequencing.

Last updated 2026-08-07.

---

## Where We Are

**Python is the working system.** Daemon, GTK overlay, control panel, five
engines, deployed through `~/nix-dotfiles`.

**The Rust design phase is complete.** Seventeen decisions recorded with
justification, a 27-entry mismatch register tying every finding to a line of
code, and the first slice bounded. `rust_impl/` has a scaffolded three-crate
workspace.

**Recently fixed in Python.** Malformed arguments no longer kill the daemon
(D8, which had crash-looped the service 51 times); voices can be selected by
name; the ASR aligner works again after a silent API-rename breakage (N27).

---

## Track A — Python Improvements Before Rust

These are worth doing now because they are small, they improve daily use, and
each one de-risks the equivalent Rust work by settling semantics against real
usage first.

### A1. Per-voice and master volume — small

mpv exposes a `volume` property, so this is a set_property call plus a
preference plus a panel slider. Two levels are worth having:

```text
master gain     applies to everything Vroca speaks
per-voice trim  e.g. -15% on a voice that is hot relative to the others
```

The per-voice trim matters more than it sounds: voices differ substantially in
loudness, so switching voices currently changes perceived volume. A stored trim
per `VoiceId` fixes that once rather than every time.

**Cost:** small. Bounded by the panel work, not the daemon work.

### A2. Multiple voices in one queue — small, if voices share an engine

Confirmed by reading the code: `sid` is a **per-call** argument to
`generate()`, not model state. Only one line binds it —
`daemon.py:597` passes `sid=self.sid`.

So a queue item carrying its own voice needs no model reload, no second engine,
no extra memory, and no concurrency:

```text
queue --voice sid5   The architect agent has finished reviewing.
queue --voice sid3   Acknowledged. Starting implementation.
```

Each item records a voice; `_wav()` uses the item's voice instead of the global
one. Playback stays sequential, which is what the agent use case actually wants.

**This covers the architect-versus-implementer case today**, provided both
voices come from the same engine. libritts alone has 904, so that is not much of
a restriction.

**What it does not cover.** Voices from *different* engines require two resident
models, roughly 250 MB each, and that is genuinely harder — it needs the model
residency work of `rust-spec.md` §10.5. Two or three concurrent voices is a
reasonable ceiling to design for.

**Cost:** small for the same-engine case. Do not let it grow into cross-engine
residency; that belongs in Rust.

### A3. Engine-qualified voice names — small

Voice names today resolve only within the loaded engine, so a client asking for
`af_kore` while libritts is loaded gets `unknown voice`. This is exactly what
happened in production. Accepting the qualified form and switching engines when
needed removes the surprise:

```text
voice kokoro:af_kore     switch engine if required, then select
voice af_kore            current engine only, as now
```

This implements `rust-spec.md` Decision 10 early, in the place clients actually
hit it.

**Cost:** small. Note that switching engines forces a model reload, so the reply
must not pretend it was instant.

### A4. Stop `daemon.py` edits from rebuilding the pitch tables — small, needs deployment approval

`~/nix-dotfiles/home/tts.nix` builds `measureSrc` from `measure.py`,
`daemon.py`, and `voices.py`, because `measure.py` imports `daemon` for
`build_engine`. Any daemon edit therefore invalidates the derivation and
re-measures 904 libritts voices, which is about a minute of synthesis per
rebuild.

The fix is to extract engine construction out of `daemon.py` into an
`engines.py` that both import. Then `measureSrc` no longer depends on the file
that changes most often.

**Cost:** small in this repo. The `tts.nix` half is a **separate deployment
change requiring approval**.

### A5. Chunked synthesis for kokoro — medium, deferred

Measured: kokoro takes about 3300 ms per sentence and libritts about 139 ms.
Splitting a sentence in half moved first audio from 4630 ms to 3141 ms without
underrunning.

Deferred deliberately. It trades prosody for latency at every seam
(`rust-spec.md` §10.3), and switching engine is a larger latency lever that
costs nothing. Revisit if kokoro's voices are worth the seams.

---

## Track B — The Rust Replacement

Sequencing from `rust-spec.md` §9.3. Track A does not block any of it.

| Stage | Content | State |
|:---|:---|:---|
| 1 | Lock first-slice decisions | **Done** |
| 2 | Rust toolchain and workspace | **Done** — flake has the toolchain, workspace scaffolded |
| 3 | Typed operations, state transitions, protocol parsers. No audio. | Next |
| 4 | Public client and CLI against a fake daemon | |
| 5 | Daemon lifecycle, fake engine and player | |
| 6 | One local engine, real playback | |
| 7 | Remaining engines and alignment | |
| 8 | Overlay and panel through the shared client | |
| 9 | Parity tests and dogfooding, deployment unchanged | |
| 10 | Update `~/nix-dotfiles` — separately approved | |
| 11 | Run Rust with a documented Python rollback | |
| 12 | Retire Python — later, separate decision | |

Stage 3 is bounded by `rust-spec.md` §8 and must not touch sockets, audio, or
GTK.

---

## Track C — Capabilities Still Being Designed

Recorded in `rust-spec.md` §10. None is Decided; each has open questions naming
what it blocks.

| | Capability | Blocking question |
|:---|:---|:---|
| R1 | Multiple voices, channels, scheduling | Interleaved or genuinely overlapping? |
| R2 | Model residency, preload and unload by `ModelId` | Explicit-only, or eviction under memory pressure? |
| R3 | Local voice sources | Reference clips only, or model files with a trust rule? |
| R4 | Low latency | Chunking policy per engine |
| R5 | Urgency and restricted interruption | Does the `0.0–1.0` weight survive review? |
| R6 | Portable, session-stable designed voices | Settled: ship the clip, not the prompt |
| R7 | Audio sinks, so a caller takes the PCM | Stream, or passed file descriptor? |
| R8 | Volume and per-voice gain | Where gain applies: player, or synthesis |

---

## Track D — External Engines

**Qwen3-TTS.** Apache-2.0, streams at about 97 ms end to end, and does voice
design and cloning. It cannot live inside the daemon: it is a ~2B parameter
transformer wanting PyTorch or vLLM and a GPU, against a stack deliberately
built on C++ onnxruntime with no Python ML stack.

It belongs behind the existing HTTP engine boundary. See
[`integration.md`](integration.md) for the wiring options and their real cost.

Local hosting is constrained here: the available GPU is a GTX 1080, which is
Pascal — no bfloat16, and FlashAttention 2 needs Ampere or newer. A hosted
endpoint is the practical path on this hardware.

---

## Suggested Order

1. **A1 volume** and **A2 same-engine multi-voice** — small, immediately useful,
   and they settle semantics that Rust will need anyway.
2. **A3 engine-qualified names** — removes a live client surprise.
3. **Rust stage 3** — the first real slice.
4. **A4 pitch tables** — whenever a deployment change is being made anyway.
5. Track C questions as they start blocking Track B.
