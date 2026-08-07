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

**Prefer ids over indexes.** An index only means something relative to the
currently loaded engine, and it can shift if a model is updated. Ids are stable.

### Engine and playback

| Command | Argument | Effect |
|:---|:---|:---|
| `engine <name>` | `kokoro`, `supertonic`, `libritts`, `zipvoice`, `remote` | Switch engine. Resets voice to 0. |
| `speed <float>` | `0.5`–`3.0` | Set speed. Pitch-corrected, applied instantly. |
| `faster` / `slower` | — | Step by `0.15`. |
| `aligner <name>` | `asr`, `energy` | Word-timing method. |
| `status` | — | Full state as JSON. |
| `unload` / `reload` | — | Drop or rebuild the engine. |

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
