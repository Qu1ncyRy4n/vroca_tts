# Vroca — Speech Synthesis & Assistive Reader Architecture

**Vroca** (named after Broca's area, the cerebral cortex region responsible for speech production and language processing) is a modular, zero-latency text-to-speech engine and assistive reading system.

---

## 1. System Architecture

```
┌───────────────────────────────────────────────────────────┐
│                    LLM Agents / CLI / Scripts             │
│                 (tts say "..." | socket)                  │
└──────────────────────────────┬────────────────────────────┘
                               │ IPC (UNIX Socket)
┌──────────────────────────────▼────────────────────────────┐
│                    Vroca Daemon (tts-daemon)              │
│  - Multi-Engine Synthesis (Kokoro / Supertonic / LibriTTS)│
│  - ASR Zipformer Alignment & Energy Floor Snapping        │
│  - Playback Queue Management & Pre-fetching               │
└──────────────┬──────────────────────────────┬─────────────┘
               │ JSON State                   │ mpv PCM
┌──────────────▼─────────────┐ ┌──────────────▼─────────────┐
│  GTK4 Overlay (tts-overlay)│ │ Audio Hardware / PipeWire  │
│  - Subtitle / RSVP / Scroll│ └────────────────────────────┘
│  - Center Fixation Column  │
└────────────────────────────┘
```

### Rust Migration Plan

Vroca uses a staged replacement plan. The Python implementation remains the
behavior source and usable production path while the Rust implementation is
built and tested. Rust must reach an explicitly reviewed parity gate before
deployment switches to Rust. Python retirement is a later, separate change.

Parity protects the public behavior that users and local programs rely on. It
does not require copying accidental Python behavior. Deliberate improvements
may replace Python behavior when the compatibility consequence is documented
and approved.

The normative Rust specification, compatibility inventory, and unresolved
decisions are maintained in [`rust-spec.md`](rust-spec.md).

### Which Document Wins

These documents answer different questions. Confusing them is how the drift
below happened in the first place.

| Question | Authority |
|:---|:---|
| What is the system *promised* to do? | This document. It is the public contract. |
| What does the Python code *actually* do today? | [`rust-spec.md`](rust-spec.md) §5, the mismatch register. Where Python departs from the contract, the departure is recorded there with a `D` or `N` identifier and flagged inline in the table below. |
| What must the Rust implementation build? | [`rust-spec.md`](rust-spec.md). It is normative and it owns every migration decision, including approved departures from this contract. |
| Which behaviors still need migration fixtures? | [`legacy-compatibility.md`](legacy-compatibility.md). |

This document does **not** describe the Python implementation. It describes the
contract. Three rows below are marked where the current implementation does not
honor it; the Rust decisions make the contract true rather than amending it.

---

## 2. UNIX Socket IPC API Reference

The Vroca daemon listens on `$XDG_RUNTIME_DIR/tts.sock`. Any agent or shell script can send plaintext commands.

The socket itself is created mode `0o666`, but it lives inside the per-user
runtime directory, which is mode `0700`. The effective trust boundary is
therefore the **owning user**, not other local accounts. The permissive mode
bits on the socket grant nothing and are retained only for compatibility.

| Command | Arguments | Description |
|:---|:---|:---|
| `say <text>` / `speak <text>` | String | Clears active queue and immediately speaks text. **Python leaves waiting queue items intact — see D2.** |
| `queue <text>` | String | Appends text block to the sequential speech queue. |
| `read` | None | Reads current primary selection (or stops if currently reading). |
| `stop` | None | Stops active speech and clears sentence cache. |
| `clear` | None | Clears the speech queue and stops active audio. |
| `skip` | None | Skips current queued item to advance to next in queue. **Python drops the next waiting item instead — see D3.** |
| `toggle` | None | Pauses or resumes playback in place (`Super+X`). |
| `next` / `back` | None | Steps forward or backward by one sentence. |
| `faster` / `slower` | None | Increases/decreases speed by `0.15x`. |
| `speed <float>` | `0.5` – `3.0` | Sets playback speed. |
| `voice <int>` | `sid` | Switches active voice index. |
| `engine <name>` | `kokoro`, `supertonic`, `libritts`, `zipvoice`, `remote` | Switches synthesis engine. |
| `aligner <name>` | `asr` or `energy` | Sets word alignment engine. |
| `font_size <int>` | `12` – `72` | Sets RSVP overlay font size (pt). |
| `words_visible <int>` | `1` – `15` | Sets surrounding context words in `scroll_rsvp` mode. |
| `position <pos>` | `bottom`, `top`, `center` | Sets overlay screen anchor. In `rsvp` and `scroll_rsvp`, `bottom` currently renders identically to `center`. |
| `mode` | None | Cycles overlay mode (`subtitle` -> `rsvp` -> `scroll_rsvp` -> `off`). |
| `reset` / `reset_prefs` | None | Resets all settings to factory defaults. **Python leaves `engine` unchanged — see N8.** |
| `status` | None | Returns complete daemon state in JSON format. |
| `catalogue` | None | Returns voice catalogue JSON for current engine. |
| `preview <int>` | `sid` | Auditions a 1-sentence sample in voice `sid`. |
| `unload` / `reload` | None | Drops ONNX model from memory or forces model reload. |
| `quit` | None | Closes mpv IPC and terminates daemon. |

### `tts` Client Behavior

Two behaviors of the deployed `tts` shell client are not socket commands:

| Invocation | Behavior |
|:---|:---|
| `tts` with no arguments | Sends `read`. |
| `tts log` | Runs `journalctl --user -u tts -u tts-overlay -f`. Never contacts the socket. |

The client also forwards trailing arguments only for `say`, `speak`, and
`queue`. Other commands are sent without arguments, so `tts speed 1.2` sends a
bare `speed`. Use the socket directly for those until the Rust CLI replaces it.

---

## 3. Implemented Capabilities

### A. Synthesis Engines
- **Kokoro v1.0:** Default engine. 53 real-named voices. RTF ~0.99.
- **Supertonic:** Ultra-low latency (RTF ~0.30), 10 voices.
- **LibriTTS-R:** High-variety catalogue (904 voices, RTF ~0.03), catalogued by median F0 (pitch).
- **ZipVoice:** Zero-shot voice cloning from reference 5-20s `.wav` clips in `~/.config/tts/voices`.
- **Remote Endpoint:** OpenAI-compatible `/v1/audio/speech` (e.g. DeepInfra) via `~/.config/tts/env`.

### B. Display Overlay Modes
- **Subtitle Strip:** Bottom-anchored GTK4 layer-shell strip with word-by-word highlight.
- **RSVP (Rapid Serial Visual Presentation):** Single-word presentation with Optimal Recognition Point (ORP) fixed column alignment.
- **Side-Scrolling RSVP (`scroll_rsvp`):** Horizontal sliding window of context words with exact center-column fixation alignment.

### C. Text Normalization & Local Agent Input
- **Markdown Text Normalizer:** Strips headings (`#`), bold/italic formatting, links, bullet points, and code blocks before sentence splitting so speech sounds like natural prose.
- **Local Agent Input:** Local agents and scripts submit speech through the `tts` CLI or UNIX socket. A watched spool file is not implemented.

---

## 4. Future Roadmap

1. **Lazy Pre-rendered Voice Previews:** Cache pre-rendered audio preview clips for all catalogue voices.
2. **Interactive Copy & Paste Reader GUI:** Floating reader pad with live sentence highlighting, paragraph bookmarking, and speed ramping.
3. **Voice Character Creator:** Persona tuning (noise scale, pitch offset, speech rate, emotion triggers).
4. **OCR & LaTeX Normalization:** Screenshot-to-speech pipeline converting mathematical notation (LaTeX) and terminal code snippets into readable spoken prose.
5. **Staged Rust Replacement (`rust_impl/`):** Build and dogfood the Linux Rust CLI, daemon, overlay, and panel behind compatibility tests before switching deployment. Retire Python only after the Rust path is stable.
