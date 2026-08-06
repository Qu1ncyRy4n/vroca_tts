# Vroca — Speech Synthesis & Assistive Reader Architecture

**Vroca** (named after Broca's area, the cerebral cortex region responsible for speech production and language processing) is a modular, zero-latency text-to-speech and assistive reading framework.

---

## 1. Architecture Overview

```
┌───────────────────────────────────────────────────────────┐
│                    LLM Agents / CLI / Scripts             │
│   (echo "done" > /tmp/tts-speak | tts say "..." | socket) │
└──────────────────────────────┬────────────────────────────┘
                               │ IPC (UNIX Socket / Spool)
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

---

## 2. Implemented Capabilities

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

### C. Agent Spool & Queue Management
- **Watched Spool File:** `/tmp/tts-speak` (`0o666` permissions) for agent write triggers (`echo "..." > /tmp/tts-speak`).
- **Sequential Speech Queue:** Command queue (`queue`, `clear`, `skip`) so agent notifications do not cut off active reading sessions.
