# Vroca Integration Guide

For programs that call Vroca — agents, scripts, games, editors. If you are
implementing Vroca itself, read [`rust-spec.md`](rust-spec.md) instead.

This describes the **current Python daemon**. Where the Rust replacement will
differ, it is marked **Changing**. Nothing here is removed without notice.

---

## 1. Connecting

The daemon listens on a Unix stream socket:

```
$XDG_RUNTIME_DIR/tts.sock
```

One request per connection. Write one plaintext command, read the reply, close.
The connection boundary *is* the message framing — there is no length prefix and
no terminator.

```sh
printf 'say hello there' | socat - "UNIX-CONNECT:$XDG_RUNTIME_DIR/tts.sock"
```

```python
import os, socket

def tts(cmd: str, timeout: float = 5.0) -> str:
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(timeout)
    s.connect(os.path.join(os.environ["XDG_RUNTIME_DIR"], "tts.sock"))
    s.sendall(cmd.encode())
    out = b""
    while True:                     # read to EOF; replies can be large
        chunk = s.recv(65536)
        if not chunk:
            break
        out += chunk
    s.close()
    return out.decode()
```

**Read to EOF.** The `catalogue` reply is over 100 KB for some engines. A single
`recv()` will truncate it.

The socket is mode `0666`, but it sits inside a `0700` per-user runtime
directory. The real access boundary is **the owning user**. Do not treat the
permissive mode bits as an invitation to share it across accounts.

### If the socket is missing

The daemon is not running. Start it with:

```sh
systemctl --user start tts
```

Do not auto-start it in a loop. If it is failing, `journalctl --user -u tts` has
the reason.

---

## 2. Commands

### Speaking

| Command | Effect |
|:---|:---|
| `say <text>` | Speak now, replacing current speech. `speak` is an alias. |
| `queue <text>` | Append to the queue. |
| `read` | Speak the primary selection. **If already speaking, this stops instead.** |
| `stop` | Stop playback. Waiting queue survives. |
| `clear` | Stop playback *and* drop the waiting queue. |
| `skip` | Drop the next waiting item. |
| `toggle` | Pause, or resume if paused. |
| `next` / `back` | Move one sentence within the current item. |

**Changing.** `say` will clear waiting items too; `queue` will always append even
while paused; `skip` will abandon the *current* item. See `rust-spec.md`
Decision 1. Today's behavior is as described above.

Text is normalized before speaking: Markdown headings, emphasis, links, list
markers and code fences are stripped so they are not read aloud literally.

### Voices

| Command | Effect |
|:---|:---|
| `voice <name>` | Select a voice by catalogue id or display name. |
| `voice <index>` | Select by numeric index. Equivalent. |
| `catalogue` | JSON array of available voices for the current engine. |
| `preview <name\|index>` | Audition a voice with a sample sentence. |

```sh
tts voice af_kore      # by id
tts voice Kore         # by display name
tts voice 5            # by index — same voice
```

Names are matched case-insensitively against the `id` and `name` fields of
`catalogue`. An unmatched token returns `unknown voice: <token>` and changes
nothing.

A catalogue entry looks like:

```json
{"sid": 5, "name": "Kore", "id": "af_kore", "lang": "en-US", "gender": "female"}
```

### Multiple voices in one queue

Each queued item can carry its own voice. This is how you give different
speakers different voices:

```sh
tts queue --voice sid3 The architect has finished reviewing.
tts queue --voice sid4 Acknowledged. Starting implementation.
tts say   --voice sid3 Stop. That breaks the protocol.
```

**Do not select a voice and then queue.** It looks equivalent and is not:

```sh
tts voice sid3 && tts queue "..."     # WRONG
tts voice sid4 && tts queue "..."     # both items speak in sid4
```

Queued items are rendered when they reach the front, not when they are
submitted, so every item would use whichever voice happened to be current at
render time. The voice has to travel with the item, which is what `--voice`
does.

The `--voice` token must be a single word, so catalogue ids work but display
names containing spaces do not. All items share the loaded engine — per-item
voices across *different* engines need two resident models and are not
supported. `status` reports `item_voice` and `queue_voices` so you can verify
what was actually recorded.

### Level

| Command | Argument | Effect |
|:---|:---|:---|
| `volume <int>` | `0`–`100` | Master level. |
| `trim <int>` | `-100`–`100` | Offset for the *current* voice, remembered per voice. |

Voices differ materially in loudness, so switching voice changes perceived
volume. A trim is stored against `engine:sid` and applied whenever that voice
plays, so you set it once. `status` reports `volume`, `trim`, and
`effective_volume`. Gain is applied in the player, so changing a trim never
forces a re-render.

**Prefer ids over indexes.** An index only means something relative to the
currently loaded engine, and it can shift if a model is updated. Ids are stable.

**Names are scoped to the engine that is currently loaded.** `af_kore` is a
kokoro voice, so asking for it while libritts is loaded returns
`unknown voice: af_kore` even though the name is perfectly valid. Switch first:

```sh
tts engine kokoro
tts voice af_kore
```

Or use the **engine-qualified form**, which switches for you:

```sh
tts voice kokoro:af_kore     # -> "voice 5 (engine kokoro)"
tts voice libritts:sid3      # -> "voice 3 (engine libritts)"
```

The reply names the engine when a switch happened, because switching reloads a
model and is not instant. An unrecognized prefix is treated as part of the
name rather than a failed switch, so `voice nosuch:x` reports
`unknown voice: nosuch:x`.

### Engine and playback

| Command | Argument | Effect |
|:---|:---|:---|
| `engine <name>` | `kokoro`, `supertonic`, `libritts`, `zipvoice`, `remote` | Switch engine. Resets voice to 0. |
| `speed <float>` | `0.5`–`3.0` | Set speed. Pitch-corrected, applied instantly. |
| `faster` / `slower` | — | Step by `0.15`. |
| `aligner <name>` | `asr`, `energy` | Word-timing method. |
| `status` | — | Full state as JSON. |
| `unload` / `reload` | — | Drop or rebuild the engine. |
| `volume <int>` | `0`–`100` | Master output level. |
| `trim <int>` | `-100`–`100` | Per-voice level offset. |

`engine` availability is not fixed: `zipvoice` appears only when reference clips
exist, and `remote` only when the API environment is configured. Read the
`engines` array from `status` rather than assuming.

### Overlay

| Command | Argument |
|:---|:---|
| `mode` | cycles `subtitle` → `rsvp` → `scroll_rsvp` → `off` |
| `position <pos>` | `bottom`, `top`, `center` |
| `font_size <int>` | `12`–`72` |
| `words_visible <int>` | `1`–`15` |
| `reset` | Restore defaults. |

In `rsvp` and `scroll_rsvp`, `position bottom` currently renders the same as
`center`.

---

## 3. Reading State

`status` returns a JSON object. The fields you are most likely to want:

| Field | Meaning |
|:---|:---|
| `sentence` | Text currently being spoken |
| `index` / `total` | Position within the current item |
| `word` | Index of the highlighted word, `-1` if none |
| `paused` | Boolean |
| `queue_len` | Items waiting |
| `engine`, `voice`, `speed`, `aligner` | Current selections |
| `engines` | Engines actually available right now |
| `loaded` | Whether a model is resident |
| `last_render_ms`, `avg_render_ms` | Synthesis timing |

The same object is written continuously to
`$XDG_RUNTIME_DIR/tts-state.json`, which is cheaper to poll than opening a
socket. It is replaced atomically, so you will never read a partial file.

**Caveat:** that file is not currently marked with the writing daemon's
identity, so a stale file left by a dead daemon is indistinguishable from a live
one. Check that the socket exists before trusting it.

**Changing.** Both the status object and the state file will gain a `schema`
version field. Ignore fields you do not recognize.

---

## 4. Errors

Replies are plain text, not structured. Success and failure are distinguished
only by reading the string.

```
voice 5                          -> "voice 5"
voice bogus                      -> "unknown voice: bogus"
speed abc                        -> "speed needs a number between 0.5 and 3.0"
engine nope                      -> "unknown engine: nope (have ...)"
<anything unrecognized>          -> "unknown: <command>"
```

Malformed numeric arguments used to **terminate the daemon**. A client sending
`voice af_kore` crash-looped the service 51 times. That is fixed: every
unparseable argument now returns a message and the daemon keeps serving.

**Changing.** A structured protocol with typed error codes is specified in
`rust-spec.md` §6, Decision 3, on a second socket. The plaintext socket keeps
working unchanged.

---

## 5. Limits And Gotchas

**Request size.** The daemon reads a single 4096-byte chunk. Longer text is
silently truncated, possibly mid-character. Split long input into several
`queue` calls. **Changing:** an explicit limit with a real error.

**Empty text.** `say` with no argument is not a speak request — the trailing
space is stripped and it returns `unknown: say`. Check for empty strings before
sending.

**`quit` stops the service.** It exits cleanly, so systemd does not restart it.
Use `stop` to stop *speech*. Only use `quit` if you mean to shut the daemon down.

**`read` is a toggle.** It stops if speech is active. For unconditional
behavior, use `say` with your own text, or check `total` in `status` first.

**Latency is engine-dependent, and the difference is large.** Measured here on
one sentence:

| Engine | Synthesis | Notes |
|:---|---:|:---|
| libritts | ~139 ms | 904 voices, fastest |
| supertonic | ~1100 ms | 10 voices |
| kokoro | ~3300–4600 ms | 53 named voices, default, **slowest** |

Everything else in the path is minor: client spawn ~5 ms, selection grab ~3 ms,
wav write ~23 ms, player start ~33 ms. If speech feels slow to start, the engine
is why. Switch to `libritts` or `supertonic` for latency-sensitive use.

**Concurrency.** There is one queue and one playback state shared by every
client. If two programs both call `say`, the second replaces the first with no
warning. **Changing:** per-client channels and an explicit urgency parameter are
specified in `rust-spec.md` §10.4.

---

## 6. Custom Voices

Drop a 5–20 second `.wav` clip into:

```
~/.config/tts/voices/
```

The `zipvoice` engine then offers it as a clonable voice, named after the file.
Select it with `engine zipvoice` followed by `voice <filename-without-extension>`.

Supply a transcript next to the clip as `<name>.txt`. The daemon can transcribe
automatically, but that path depends on the ASR model and is worth not relying
on.

For a voice that must be **identical across sessions and machines** — a game
character, say — ship the `.wav` as an asset and clone from it at runtime. The
clip is the portable artifact. See `rust-spec.md` §10.6a.

---

## 7. Quick Reference

```sh
tts say "text"          # speak now
tts queue "text"        # append
tts stop                # stop speech
tts status              # JSON state
tts voice af_kore       # by name
tts engine libritts     # fastest engine
tts speed 1.5
tts log                 # follow the journal
tts                     # no args == read selection
```

The `tts` shell client forwards trailing arguments only for `say`, `speak` and
`queue`. `tts speed 1.2` sends a bare `speed`. Use the socket directly for other
commands with arguments.

---

## 8. Using Qwen3-TTS Through Vroca

Qwen3-TTS is a separate model family — Apache-2.0, streaming at about 97 ms,
with natural-language voice design and cloning. It does not run inside the
Vroca daemon (see [`roadmap.md`](roadmap.md) Track D). It connects over HTTP.

Vroca already has the adapter. The engine named `remote` speaks the
OpenAI-compatible `/v1/audio/speech` shape, and both hosted providers and local
servers expose exactly that for Qwen3-TTS.

### Configure it

Put this in `~/.config/tts/env`, which the systemd unit reads and which is
deliberately outside the repository and the Nix store:

```sh
TTS_API_BASE=https://api.deepinfra.com/v1/openai
TTS_API_KEY_FILE=/home/you/.config/tts/key
TTS_API_MODEL=Qwen/Qwen3-TTS-VoiceDesign
TTS_API_VOICES=Vivian,Ryan,Serena
```

Keep the key in its own file, read at call time. Never put it in a Nix
expression — derivations land in world-readable `/nix/store`.

Then `engine remote` and the usual `voice`, `say`, and `queue` commands work
unchanged. `remote` only appears in the `engines` list when both the base URL
and the key file are present.

For a local server instead, point `TTS_API_BASE` at it. vLLM and several
community FastAPI wrappers serve the same endpoint shape.

### What works today, and what does not

| Capability | Status through Vroca |
|:---|:---|
| Preset voices | **Works now.** Configuration only, no code change. |
| Cloning profiles, where the server exposes `voice="clone:Name"` | **Works now** — it is just a voice string. |
| Natural-language voice design (`instruct`) | **Needs a small adapter change.** The parameter is not in OpenAI's schema, so the current request builder has nowhere to put it. |
| Streaming, the 97 ms figure | **Not available.** The adapter reads the whole response, then parses a WAV. Getting real streaming needs incremental playback — the same machinery as chunking in `rust-spec.md` §10.3. |
| Retries, timeouts, error mapping | Minimal. One 60-second timeout, no retry, failures surface as a bare string. |

So: **preset voices are a configuration change**, voice design is a small patch,
and low latency is a real project rather than a flag.

### Cost and privacy

Hosted inference is billed per character — DeepInfra lists Qwen3-TTS-VoiceDesign
at $20 per million characters at time of writing. Reading long documents aloud
will consume that quickly, so `remote` is better suited to short, deliberate
speech than to bulk reading.

**Text leaves the machine.** Every other engine is local and nothing is
transmitted. Choosing `remote` sends whatever you speak to the configured
endpoint. If that endpoint is `localhost`, nothing leaves — but the engine name
does not distinguish the two, so verify `TTS_API_BASE` rather than trusting the
label. This is tracked as a design problem in `rust-spec.md` §10.6b.

---

## 9. Custom Voices: How Cloning Actually Works

Worth understanding, because it determines what you need to supply.

**You supply ordinary speech.** Not a phoneme inventory, not a spectrum sweep,
not a test tone. Five to twenty seconds of someone talking normally. The
reference clip shipped with this repo is 5.1 seconds of a plain spoken sentence.

**It does not need to contain every sound.** This is the part that seems
impossible and is not. The model was pretrained on thousands of speakers, so it
already knows how English phonemes are articulated in general. Your clip is not
teaching it to speak. It is answering a much narrower question: *whose* voice.

From the clip the model extracts a speaker embedding — a vector capturing
timbre, pitch range, resonance, and speaking style. Synthesis then renders any
text through that identity, including sounds absent from your reference.

The useful analogy is instrument identification rather than sampling. A sampler
needs every note recorded. This identifies *which instrument* from a short
phrase, then plays notes it never heard.

**So yes — a plain spoken sample is all it takes**, and that is the whole
interface. Practical guidance:

- Clean audio matters more than length. Background noise gets modelled as part
  of the voice.
- Consistent delivery helps. A clip swinging between whisper and shout produces
  an unstable identity.
- Supply the transcript as `<name>.txt` beside the clip rather than relying on
  automatic transcription.

**Why ship a `.wav` rather than the embedding vector.** The vector is smaller and
skips the encoding step, so shipping it looks more efficient. It is not, for
anything long-lived. An embedding lives in one model's latent space, so a model
upgrade or a switch to a different engine invalidates it. A `.wav` re-clones
correctly against any future model, including ones that do not exist yet. For a
voice that must stay stable for years — a game character — the audio is the
durable artifact and the vector is a cache.
