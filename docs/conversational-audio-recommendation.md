# Conversational Audio, Channels, And Interruption — Recommendation

**Status:** Recommendation for review. This document does not amend the
normative decisions in [`rust-spec.md`](rust-spec.md). It proposes the decisions
needed before Track C is implemented.

## 1. Outcome

Vroca should remain a strong desktop reader while becoming usable as a local
conversational-audio service. A caller must be able to submit speech, receive a
stable acknowledgement, observe its progress, interrupt it deliberately, and
choose whether Vroca or the caller plays the resulting audio.

The recommended default is **interleaved speech**: one intelligible voice at a
time. Intentional simultaneous speech is also supported, but only when a caller
opts into it and only through a caller-owned audio sink or a dedicated playback
channel. It must never be an accidental result of two requests arriving close
together.

This recommendation preserves all existing `tts.sock` commands. The new API is
additive and lives on a second socket.

## 2. What Exists Today

The Python daemon exposes one request per connection on `tts.sock`. A client
writes one plaintext command such as `say`, `queue`, `stop`, or `status`, reads
one plaintext reply, and closes. All callers share one queue, one active item,
and one `mpv` player.

Per-item voice selection already exists for voices in the currently loaded
engine. It is useful but not a channel: `say` replaces the active speech and
clears every waiting item, irrespective of which program submitted it.

The current engines synthesize a complete sentence into a WAV before playback.
Consequently, `stop` can silence `mpv` immediately, but a slow synthesis call
may still finish in the background. The result is wasted rather than genuinely
cancelled. This distinction matters for responsiveness, CPU use, and a future
audio stream.

## 3. Goals And Non-Goals

### Goals

- Make interruption a declared policy instead of an accident of queue arrival.
- Let independent programs own independent queues and voice defaults.
- Keep a user reading channel protected from chatty agent notifications.
- Support immediate speaker silence, boundary-safe yielding, and resumable
  displacement.
- Permit intentional voice-on-voice overlap for games and simulations.
- Allow a caller to receive PCM and word timings rather than routing everything
  to the desktop audio device.
- Preserve the legacy socket until a documented migration retires it.

### Non-goals

- Exposing Vroca to untrusted network clients.
- Building a general-purpose in-process mixer before a concrete need requires
  ducking, crossfades, or sample-accurate mixing.
- Pretending that a non-streaming engine becomes realtime because its control
  protocol is persistent.
- Making a voice intrinsically interruptible. Interruptibility belongs to the
  role of a submitted utterance; a voice profile supplies only defaults.

## 4. Recommended Model

### 4.1 Channels

A **channel** is a named submission and scheduling scope. It owns a queue,
defaults for voice and playback destination, and a policy describing how it may
affect other channels.

Suggested built-ins are `user`, `reader`, `notification`, and `preview`. Other
callers register application-scoped names such as `game.dialogue` or
`agent.implementer`. The daemon authenticates none of these names because the
socket remains inside the owning user's runtime directory; they are for policy
and observability, not a security boundary.

Operations that replace, stop, pause, or resume speech are channel-scoped by
default. A cross-channel action must name that broader scope explicitly. This
prevents one agent's ordinary message from silently deleting a user's reading
queue.

### 4.2 Interruption Is A Category, Not A Float Threshold

Use a categorical permission to decide whether an item can displace another:

```text
Preemption = Never | AtBoundary | Immediate
Urgency    = Background | Normal | Urgent
```

`Urgency` says what the new item asks to do. `Preemption` says what the
currently playing item permits. The scheduler interrupts only when both sides
allow it.

| New item | Active item permits | Result |
| --- | --- | --- |
| `Background` | any | Wait behind active and queued normal work. |
| `Normal` | any | Queue behind the active item. It may reorder background work. |
| `Urgent` | `Never` | Queue; report that immediate interruption was denied. |
| `Urgent` | `AtBoundary` | Mark pending; yield at a configured clause or sentence boundary. |
| `Urgent` | `Immediate` | Stop playback now, retain the displaced item, then play urgent speech. |

An optional `weight: 0.0..=1.0` may order work *within the same urgency class*.
It must never determine whether an interruption happens. A numeric interruption
threshold looks flexible but becomes priority inflation: every caller selects a
value just above the undocumented boundary, and `0.71` versus `0.69` is not a
meaningful product rule.

Voice profiles may provide defaults. For example, an audiobook narrator might
default to `AtBoundary`, a screen-reader selection to `Immediate`, and a status
notification to `Never` as an active item. The request and channel policy always
remain authoritative, so the same voice can narrate safely in one context and
yield quickly in another.

### 4.3 Resumption

On preemption, the scheduler stores a resumable cursor:

```text
item id, channel, utterance index, chunk index, playback offset, generation
```

After urgent speech finishes, the displaced item resumes unless a later command
explicitly replaced or cancelled it. For a whole-sentence WAV, resume from the
sentence start initially. For chunked or genuinely streaming engines, resume at
the next chunk boundary. Exact sample-level resumption is not required for the
first implementation and adds complexity without improving comprehension much.

### 4.4 Intentional Voice-On-Voice Overlap

Overlap is a separate policy from interruption. The default is `exclusive`;
only one channel is audible through Vroca's desktop player. A caller requesting
`overlap` says that simultaneous audibility is intentional.

```text
MixMode = Exclusive | Overlap
```

There are two supported paths:

1. **Caller-owned sink — recommended for games and simulations.** Vroca returns
   PCM, sample rate, channels, and word timings. The caller owns spatialisation,
   music ducking, gain, pause, and mixing. This is the cleanest way to let two
   voices interrupt or overlap each other.
2. **Dedicated desktop playback channels — limited use.** Each opted-in channel
   receives its own `mpv` instance. This retains pitch-corrected speed control
   and is enough for coarse overlap. It does not promise ducking, crossfade, or
   sample-accurate synchronization.

An `Urgent` request preempts rather than mixes by default, even for an
overlap-capable channel. It may mix only when the requester explicitly sets
`mix_mode: overlap` and the active channel's overlap policy permits it. This
keeps ordinary alerts intelligible and makes every audible collision auditable.

The daemon should cap simultaneous desktop channels at two initially and report
`busy` when the cap is reached. Games using a caller sink may choose their own
mixing limit.

## 5. Structured API

Bind a versioned, owner-only Unix socket at `vroca-v1.sock` beside the legacy
socket. Use a persistent connection with length-prefixed JSON messages. Length
prefixing avoids relying on a quiet gap, permits large text and binary-adjacent
metadata, and leaves room for audio frames on a later transport.

Every request has an `id`; every acknowledgement, terminal result, and error
echoes it. Events carry an `item_id` and `channel`, so a caller can follow only
its own work while a panel or overlay can subscribe to global state.

```json
{
  "type": "speak",
  "id": "request-42",
  "channel": "agent.implementer",
  "text": "The implementation is ready for review.",
  "voice": "kokoro:af_bella",
  "replace": "channel",
  "urgency": "normal",
  "preemption": "at_boundary",
  "mix_mode": "exclusive",
  "sink": "player"
}
```

The minimal operation set is:

```text
ChannelOpen  { channel, defaults }
Speak        { channel, text, voice?, replace, urgency, preemption?, mix_mode?, sink? }
Cancel       { channel, item_id?, scope }
Pause        { channel? }
Resume       { channel? }
Subscribe    { topics, channel? }
Status       { channel? }
```

`replace` becomes `None | Active | Channel | All`. `All` is reserved for an
explicit user-level action; it is never the default for an agent channel.

Suggested events are:

```text
accepted, queued, synthesis_started, audio_ready, playback_started,
interruption_pending, playback_interrupted, resumed, completed, cancelled,
channel_state, health_changed, error
```

The initial structured socket can use only request/response and event framing.
For `Sink::Caller`, first return a short-lived local stream descriptor or an
explicit second framed audio stream. Passing a shared-memory file descriptor is
an optimization to consider only after the simpler copy-based protocol has been
dogfooded.

## 6. Runtime Architecture

Keep the Rust design's single state owner and bounded synthesis workers:

```text
connections -> parser -> state-owner channel -> scheduler -> player or caller sink
                                      |                 -> synthesis workers
                                      -> event broadcaster
```

The state owner alone changes channel queues, active playback, and persistent
preferences. Workers never mutate state directly. Each synthesis job is stamped
with an item ID and generation. Cancellation increments the item's generation;
when non-cancellable local synthesis returns, a stale result is discarded.

This yields instant speaker silence without claiming to abort an engine that
cannot abort. Chunked synthesis reduces the maximum wasted work from a full
sentence to a chunk. A genuinely streaming engine can later replace this with
incremental audio frames without changing channel or scheduler semantics.

## 7. Viability And Rollout

| Phase | Deliverable | Value | Main risk |
| --- | --- | --- | --- |
| 1 | Structured socket, request IDs, channels, event subscription | Safe multi-caller control | Public protocol commitment |
| 2 | Channel scheduler, categorical preemption, generation invalidation | Reliable barge-in and resume | Define hotkey scope and starvation rules |
| 3 | Clause chunks for slow engines | Better first-audio and cancellation bound | Prosody seams, gapless playback |
| 4 | Caller audio sink | Games and custom mixers | Delivery format and bindings |
| 5 | Optional desktop overlap | Simulations and experiments | Multiple-player lifecycle and intelligibility |
| 6 | Native streaming provider | Natural low-latency conversation | Provider capability, cost, privacy |

Phases 1 and 2 are highly viable with the existing Rust architecture. They do
not require a new async runtime or an in-process audio engine. Phase 3 is viable
but should stay engine-specific: fast libritts does not benefit enough to justify
prosody damage. Phase 4 is the recommended overlap path. Phase 5 should remain
experimental until a listening test establishes that it helps its target use
case.

## 8. Decisions Required Before Implementation

1. Confirm the built-in channel names and whether callers must register a
   channel before submitting speech.
2. Decide whether the user hotkeys target the `user` channel, the active
   channel, or have explicit global variants. `stop` should remain globally
   available; pause needs a more careful answer.
3. Choose a starvation guard for `Urgent`, such as a per-channel rate limit and
   a maximum consecutive-preemption count.
4. Set clause and sentence boundaries for `AtBoundary` and specify whether a
   boundary request may be upgraded to immediate by a user action.
5. Confirm the initial desktop-overlap cap of two channels and whether it is
   shipped or remains an experimental feature flag.
6. Choose the initial caller-audio delivery format. The recommendation is
   interleaved PCM frames plus a JSON metadata stream, with Opus offered later
   when a network-facing use case exists.
7. Establish performance tests: interruption-to-silence, request-to-first-audio,
   stale-work discard, resume order, and event delivery under concurrent clients.

## 9. Compatibility And Documentation

`tts.sock`, its plaintext commands, `tts-state.json`, and the current overlay
remain intact during migration. The legacy `say` command retains its documented
global replacement behavior. The structured API defaults to channel scope,
which is deliberately safer for multi-caller applications.

Before binding `vroca-v1.sock`, revise the open items in `rust-spec.md` §10 and
record the accepted choices as normative decisions. Update `integration.md`
with a separate structured-client guide, including error codes, message limits,
and examples. Do not silently change the legacy protocol.
