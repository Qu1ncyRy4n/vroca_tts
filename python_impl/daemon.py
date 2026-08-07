#!/usr/bin/env python3
"""Speechify-style reader: synthesize the selection a sentence at a time and
step through it from hotkeys.

Split in two because the two halves have different failure modes: synthesis is
slow and can be worked ahead, playback has to respond to a keypress now. So a
worker thread renders sentence N+1 into a wav cache while mpv plays sentence N,
and the socket handler only ever touches state + mpv.

mpv does the playing because pause/resume/speed on an already-decoded stream is
exactly its job -- and its speed control is pitch-corrected, so faster/slower is
instant instead of a re-synthesis round trip.
"""
import json
import os
import re
import socket
import subprocess
import sys
import tempfile
import threading
import time

RUNTIME = os.environ.get("XDG_RUNTIME_DIR") or tempfile.gettempdir()
SOCK = os.path.join(RUNTIME, "tts.sock")
MPV_SOCK = os.path.join(RUNTIME, "tts-mpv.sock")
STATE = os.path.join(RUNTIME, "tts-state.json")
MODE_FILE = os.path.join(RUNTIME, "tts-mode")
CONFIG_DIR = os.path.join(
    os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config"), "tts")
PREFS = os.path.join(CONFIG_DIR, "prefs.json")
CLONE_DIR = os.path.join(CONFIG_DIR, "voices")

SPEED_MIN, SPEED_MAX, SPEED_STEP = 0.5, 3.0, 0.15
PREFETCH = 5
MODES = ["subtitle", "rsvp", "scroll_rsvp", "off"]
ALIGNERS = ["asr", "energy"]
DEFAULT_ALIGNER = "asr"
DEFAULT_FONT_SIZE = 24
DEFAULT_WORDS_VISIBLE = 3
DEFAULT_POSITION = "bottom"

BOUNDARY = re.compile(r"""[.!?]+["')\]]*(?=\s)""")
ABBREV = {"mr", "mrs", "ms", "dr", "st", "vs", "prof", "sr", "jr", "e.g", "i.e",
          "etc", "fig", "no", "approx", "inc", "ltd", "co"}
INITIAL = re.compile(r"\b[A-Z]$")


def _is_abbrev(text, end):
    """True if the period at `end` terminates an abbreviation, not a sentence."""
    head = text[:end].rstrip(".!?\"')]")
    word = re.split(r"[\s(\[\"']", head)[-1] if head else ""
    return word.lower() in ABBREV or bool(INITIAL.search(head))


def clean_markdown(text):
    """Normalize markdown formatting so headings, lists, bold, and code snippets
    speak naturally without literal symbol reads like 'hashtag' or 'asterisk'."""
    if not text:
        return ""
    text = re.sub(r"```[\s\S]*?```", " Code snippet. ", text)
    text = re.sub(r"`([^`]+)`", r"\1", text)
    text = re.sub(r"\[([^\]]+)\]\([^)]+\)", r"\1", text)
    text = re.sub(r"!\[([^\]]*)\]\([^)]+\)", r"\1 image", text)
    text = re.sub(r"(\*\*|__|\*|_|~~)(.*?)\1", r"\2", text)
    text = re.sub(r"(?m)^\s*#{1,6}\s+(.*)$", r"\1.", text)
    text = re.sub(r"(?m)^\s*[-*+]\s+(.*)$", r"\1.", text)
    text = re.sub(r"(?m)^\s*\d+\.\s+(.*)$", r"\1.", text)
    text = re.sub(r"(?m)^\s*>\s+(.*)$", r"\1.", text)
    return re.sub(r"\s+", " ", text).strip()


def sentences(text):
    text = clean_markdown(text)
    if not text:
        return []
    out, start = [], 0
    for m in BOUNDARY.finditer(text):
        if _is_abbrev(text, m.start() + 1):
            continue
        chunk = text[start:m.end()].strip()
        if chunk:
            out.append(chunk)
        start = m.end()
    tail = text[start:].strip()
    if tail:
        out.append(tail)
    return out


class Mpv:
    def __init__(self, on_eof, on_pos=None):
        self.on_eof = on_eof
        self.on_pos = on_pos
        try:
            os.unlink(MPV_SOCK)
        except FileNotFoundError:
            pass
        self.proc = subprocess.Popen(
            ["mpv", "--idle=yes", "--no-video", "--no-terminal",
             "--keep-open=no", f"--input-ipc-server={MPV_SOCK}"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        self.lock = threading.Lock()
        self.sock = None
        for _ in range(200):
            try:
                s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                s.connect(MPV_SOCK)
                self.sock = s
                break
            except OSError:
                threading.Event().wait(0.025)
        if self.sock is None:
            raise RuntimeError("mpv IPC socket never appeared")
        threading.Thread(target=self._events, daemon=True).start()
        self.cmd("observe_property", 1, "time-pos")

    def cmd(self, *args):
        with self.lock:
            try:
                self.sock.send((json.dumps({"command": list(args)}) + "\n").encode())
            except OSError:
                pass

    def _events(self):
        buf = b""
        while True:
            try:
                chunk = self.sock.recv(4096)
            except OSError:
                return
            if not chunk:
                return
            buf += chunk
            while b"\n" in buf:
                line, buf = buf.split(b"\n", 1)
                try:
                    msg = json.loads(line)
                except ValueError:
                    continue
                ev = msg.get("event")
                if ev == "end-file" and msg.get("reason") == "eof":
                    try:
                        self.on_eof()
                    except Exception:
                        pass
                elif (ev == "property-change" and msg.get("name") == "time-pos"
                      and self.on_pos and isinstance(msg.get("data"), (int, float))):
                    try:
                        self.on_pos(float(msg["data"]))
                    except Exception:
                        pass


def _supertonic_cfg(s, d, threads):
    return s.OfflineTtsModelConfig(
        supertonic=s.OfflineTtsSupertonicModelConfig(
            duration_predictor=f"{d}/duration_predictor.int8.onnx",
            text_encoder=f"{d}/text_encoder.int8.onnx",
            vector_estimator=f"{d}/vector_estimator.int8.onnx",
            vocoder=f"{d}/vocoder.int8.onnx",
            tts_json=f"{d}/tts.json",
            unicode_indexer=f"{d}/unicode_indexer.bin",
            voice_style=f"{d}/voice.bin",
        ),
        num_threads=threads,
    )


def _kokoro_cfg(s, d, threads):
    return s.OfflineTtsModelConfig(
        kokoro=s.OfflineTtsKokoroModelConfig(
            model=f"{d}/model.int8.onnx",
            voices=f"{d}/voices.bin",
            tokens=f"{d}/tokens.txt",
            data_dir=f"{d}/espeak-ng-data",
            lexicon=f"{d}/lexicon-us-en.txt,{d}/lexicon-zh.txt",
        ),
        num_threads=threads,
    )


def _libritts_cfg(s, d, threads):
    return s.OfflineTtsModelConfig(
        vits=s.OfflineTtsVitsModelConfig(
            model=f"{d}/en_US-libritts_r-medium.onnx",
            tokens=f"{d}/tokens.txt",
            data_dir=f"{d}/espeak-ng-data",
        ),
        num_threads=threads,
    )


ENGINES = {
    "kokoro": (_kokoro_cfg, True),
    "supertonic": (_supertonic_cfg, False),
    "libritts": (_libritts_cfg, False),
}
DEFAULT_ENGINE = "kokoro"


class LocalEngine:
    kind = "local"

    def __init__(self, name, model_dir, threads):
        import sherpa_onnx
        builder, self.named = ENGINES[name]
        self.name = name
        self.tts = sherpa_onnx.OfflineTts(
            sherpa_onnx.OfflineTtsConfig(model=builder(sherpa_onnx, model_dir, threads)))

    @property
    def num_speakers(self):
        return self.tts.num_speakers or 1

    def generate(self, text, sid=0, speed=1.0):
        a = self.tts.generate(text, sid=sid, speed=speed)
        return a.samples, a.sample_rate


class CloneEngine:
    kind = "clone"
    named = True

    def __init__(self, model_dir, vocoder, threads, recognizer=None):
        import sherpa_onnx
        d = model_dir
        self.tts = sherpa_onnx.OfflineTts(sherpa_onnx.OfflineTtsConfig(
            model=sherpa_onnx.OfflineTtsModelConfig(
                zipvoice=sherpa_onnx.OfflineTtsZipvoiceModelConfig(
                    encoder=f"{d}/encoder.int8.onnx",
                    decoder=f"{d}/decoder.int8.onnx",
                    vocoder=vocoder,
                    tokens=f"{d}/tokens.txt",
                    lexicon=f"{d}/lexicon.txt",
                    data_dir=f"{d}/espeak-ng-data",
                ),
                num_threads=threads,
            )))
        self.recognizer = recognizer
        self.name = "zipvoice"
        self.refs = list_clone_refs()

    @property
    def num_speakers(self):
        return max(1, len(self.refs))

    def _load_ref(self, path):
        import wave
        import numpy as np
        with wave.open(path, "rb") as w:
            rate = w.getframerate()
            x = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16)
        return (x.astype(np.float32) / 32768.0).tolist(), rate

    def _transcript(self, path, samples, rate):
        txt = os.path.splitext(path)[0] + ".txt"
        try:
            with open(txt) as f:
                s = f.read().strip()
            if s:
                return s
        except OSError:
            pass
        if self.recognizer is None:
            return ""
        import numpy as np
        st = self.recognizer.create_stream()
        st.accept_waveform(rate, np.asarray(samples, dtype=np.float32))
        self.recognizer.decode_stream(st)
        s = st.result.text.strip()
        try:
            with open(txt, "w") as f:
                f.write(s)
        except OSError:
            pass
        return s

    def generate(self, text, sid=0, speed=1.0):
        if not self.refs:
            raise RuntimeError(
                f"no reference wavs in {CLONE_DIR} -- drop a 5-20s clip in there")
        path = self.refs[min(sid, len(self.refs) - 1)]
        samples, rate = self._load_ref(path)
        prompt = self._transcript(path, samples, rate)
        a = self.tts.generate(text, prompt, samples, rate, speed, 4)
        return a.samples, a.sample_rate


class RemoteEngine:
    kind = "remote"
    named = True

    def __init__(self, base, model, key_file, voices):
        self.base = base.rstrip("/")
        self.model = model
        self.key_file = key_file
        self.voices = voices
        self.name = "remote"

    @staticmethod
    def available():
        kf = os.environ.get("TTS_API_KEY_FILE", "")
        return bool(kf and os.path.exists(kf) and os.environ.get("TTS_API_BASE"))

    def _key(self):
        with open(self.key_file) as f:
            return f.read().strip()

    @property
    def num_speakers(self):
        return len(self.voices) or 1

    def generate(self, text, sid=0, speed=1.0):
        import json as _json
        import urllib.request
        import wave as _wave
        import io
        body = _json.dumps({
            "model": self.model,
            "input": text,
            "voice": self.voices[sid] if sid < len(self.voices) else "alloy",
            "speed": speed,
            "response_format": "wav",
        }).encode()
        req = urllib.request.Request(
            f"{self.base}/audio/speech", data=body,
            headers={"Authorization": f"Bearer {self._key()}",
                     "Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=60) as r:
            raw = r.read()
        with _wave.open(io.BytesIO(raw), "rb") as w:
            rate = w.getframerate()
            import array
            pcm = array.array("h")
            pcm.frombytes(w.readframes(w.getnframes()))
        return [v / 32768.0 for v in pcm], rate


def list_clone_refs():
    try:
        return sorted(os.path.join(CLONE_DIR, f) for f in os.listdir(CLONE_DIR)
                      if f.lower().endswith(".wav"))
    except OSError:
        return []


def build_engine(name, model_dirs, threads, recognizer=None):
    if name == "zipvoice":
        return CloneEngine(model_dirs["zipvoice"], model_dirs["vocoder"],
                           threads, recognizer)
    if name == "remote":
        return RemoteEngine(
            os.environ["TTS_API_BASE"],
            os.environ.get("TTS_API_MODEL", "tts-1"),
            os.environ["TTS_API_KEY_FILE"],
            [v.strip() for v in os.environ.get(
                "TTS_API_VOICES", "alloy,echo,fable,onyx,nova,shimmer").split(",")],
        )
    return LocalEngine(name, model_dirs[name], threads)


class Reader:
    def __init__(self, tts, model_dirs, threads, engine=DEFAULT_ENGINE):
        self.tts = tts
        self.model_dirs = model_dirs
        self.threads = threads
        self.engine = engine
        self.aligner = DEFAULT_ALIGNER
        self._asr = None
        self.sents = []
        self.queue = []
        self.idx = 0
        self.speed = 1.0
        self.sid = 0
        self.font_size = DEFAULT_FONT_SIZE
        self.words_visible = DEFAULT_WORDS_VISIBLE
        self.position = DEFAULT_POSITION
        self.paused = False
        self.cache = {}
        self.render_ms = {}
        self.rendering = None
        self.spans = {}
        self.word = -1
        self.tmp = tempfile.mkdtemp(prefix="tts-", dir=RUNTIME)
        self.lock = threading.RLock()
        self.dump_lock = threading.Lock()
        self.wake = threading.Event()
        self.mpv = Mpv(self._on_eof, self._on_pos)
        threading.Thread(target=self._prefetch_loop, daemon=True).start()

    def _align(self, samples, rate, words):
        if self.aligner == "asr":
            try:
                spans = asr_spans(self._recognizer(), samples, rate, words)
                if spans:
                    return spans
            except Exception:
                pass
        return word_spans(samples, rate, words)

    def _recognizer(self):
        if self._asr is None:
            import sherpa_onnx
            d = self.model_dirs["asr"]
            # from_transducer, not create_from_transducer: sherpa-onnx renamed
            # this. The old name raised AttributeError, _align swallowed it, and
            # the daemon silently used the energy aligner while status kept
            # reporting "asr". It also broke zipvoice's reference transcription.
            self._asr = sherpa_onnx.OfflineRecognizer.from_transducer(
                tokens=f"{d}/tokens.txt",
                encoder=f"{d}/encoder-epoch-99-avg-1.int8.onnx",
                decoder=f"{d}/decoder-epoch-99-avg-1.onnx",
                joiner=f"{d}/joiner-epoch-99-avg-1.int8.onnx",
                num_threads=self.threads,
            )
        return self._asr

    def _recognizer_or_none(self):
        if self.engine != "zipvoice":
            return None
        try:
            return self._recognizer()
        except Exception:
            return None

    def set_aligner(self, name):
        if name not in ALIGNERS:
            return f"unknown aligner: {name} (have {', '.join(ALIGNERS)})"
        with self.lock:
            self.aligner = name
            save_prefs(self)
            self._dump()
            return f"aligner {name}"

    def set_font_size(self, size):
        if size is None:
            return "font_size needs a number between 12 and 72"
        self.font_size = max(12, min(72, int(size)))
        save_prefs(self)
        self._dump()
        return f"font_size {self.font_size}"

    def set_words_visible(self, count):
        if count is None:
            return "words_visible needs a number between 1 and 15"
        self.words_visible = max(1, min(15, int(count)))
        save_prefs(self)
        self._dump()
        return f"words_visible {self.words_visible}"

    def set_position(self, pos):
        if pos in ("bottom", "top", "center"):
            self.position = pos
            save_prefs(self)
            self._dump()
            return f"position {self.position}"
        return f"unknown position: {pos}"

    def reset_prefs(self):
        with self.lock:
            self.speed = 1.0
            self.aligner = DEFAULT_ALIGNER
            self.sid = 0
            self.font_size = DEFAULT_FONT_SIZE
            self.words_visible = DEFAULT_WORDS_VISIBLE
            self.position = DEFAULT_POSITION
            save_prefs(self)
            try:
                with open(MODE_FILE, "w") as f:
                    f.write("subtitle")
            except OSError:
                pass
            self._dump()
            return "reset to defaults"

    def unload(self):
        with self.lock:
            self.tts = None
            self._dump()
            return f"unloaded {self.engine}"

    def reload(self):
        with self.lock:
            self.tts = None
            t0 = time.monotonic()
            try:
                self.tts = build_engine(self.engine, self.model_dirs, self.threads,
                                        self._recognizer_or_none())
            except Exception as e:
                self._dump()
                return f"reload failed: {e}"
            ms = int((time.monotonic() - t0) * 1000)
            self._dump()
            return f"reloaded {self.engine} in {ms}ms"

    def _ensure(self):
        if self.tts is None:
            self.tts = build_engine(self.engine, self.model_dirs, self.threads,
                                    self._recognizer_or_none())
        return self.tts

    def set_engine(self, name):
        if name not in self.engines():
            return f"unknown engine: {name} (have {', '.join(self.engines())})"
        if name == "zipvoice" and not list_clone_refs():
            return f"no reference wavs in {CLONE_DIR}"
        with self.lock:
            if name == self.engine and self.tts is not None:
                return f"engine {name}"
            self.engine = name
            self.sid = 0
            self.tts = None
            save_prefs(self)
            self._dump()
            return f"engine {name}"

    def engines(self):
        names = list(ENGINES)
        if list_clone_refs():
            names.append("zipvoice")
        if RemoteEngine.available():
            names.append("remote")
        return names

    def resolve_voice(self, token):
        """Catalogue sid for a voice named by id, display name, or index.

        Callers address voices the way the catalogue prints them, so `af_kore`
        has to work as well as `11`. Returns None when nothing matches, which
        the dispatch reports rather than guessing a voice.
        """
        if token is None:
            return None
        token = token.strip()
        try:
            return int(token)
        except ValueError:
            pass
        want = token.lower()
        for entry in self.catalogue():
            if want in (str(entry.get("id", "")).lower(),
                        str(entry.get("name", "")).lower()):
                return entry["sid"]
        return None

    def set_voice(self, sid):
        if not (isinstance(sid, int) and 0 <= sid < self.voices()):
            return f"voice index out of range: {sid} (0..{self.voices() - 1})"
        with self.lock:
            self.sid = sid
            self.cache.clear()
            save_prefs(self)
            self._dump()
            return f"voice {sid}"

    def voices(self):
        if self.engine == "remote":
            return len(RemoteEngine(
                os.environ.get("TTS_API_BASE", ""),
                os.environ.get("TTS_API_MODEL", "tts-1"),
                os.environ.get("TTS_API_KEY_FILE", ""),
                [v.strip() for v in os.environ.get(
                    "TTS_API_VOICES", "alloy,echo,fable,onyx,nova,shimmer").split(",")],
            ).voices)
        if self.engine == "zipvoice":
            return max(1, len(list_clone_refs()))
        try:
            return self._ensure().num_speakers or 1
        except Exception:
            return 1

    def set_speed(self, speed):
        if speed is None:
            return f"speed needs a number between {SPEED_MIN} and {SPEED_MAX}"
        self.speed = max(SPEED_MIN, min(SPEED_MAX, speed))
        self.mpv.cmd("set_property", "speed", self.speed)
        save_prefs(self)
        self._dump()
        return f"speed {self.speed:.2f}"

    def nudge(self, delta):
        return self.set_speed(self.speed + delta)

    def _wav(self, i):
        if i in self.cache:
            return self.cache[i]
        if not (0 <= i < len(self.sents)):
            return None
        self._ensure()
        out = os.path.join(self.tmp, f"{i}.wav")
        self.rendering = i
        self._dump()
        t0 = time.monotonic()
        try:
            samples, rate = self.tts.generate(self.sents[i], sid=self.sid, speed=1.0)
            write_wav(out, samples, rate)
            self.spans[i] = self._align(samples, rate, self.sents[i].split())
        except Exception:
            self.rendering = None
            self._dump()
            return None
        self.render_ms[i] = int((time.monotonic() - t0) * 1000)
        self.cache[i] = out
        self.rendering = None
        self._dump()
        return out

    def _prefetch_loop(self):
        while True:
            self.wake.wait()
            self.wake.clear()
            with self.lock:
                start, n = self.idx + 1, len(self.sents)
            for i in range(start, min(start + PREFETCH, n)):
                if i not in self.cache:
                    self._wav(i)
                if self.wake.is_set():
                    break

    def _on_pos(self, pos):
        spans = self.spans.get(self.idx)
        if not spans or pos is None:
            return
        w = -1
        for k, (a, b) in enumerate(spans):
            if a <= pos < b:
                w = k
                break
        else:
            if pos >= spans[-1][0]:
                w = len(spans) - 1
        if w != self.word:
            self.word = w
            self._dump()

    def _on_eof(self):
        with self.lock:
            if not self.sents or self.paused:
                return
            if self.idx + 1 < len(self.sents):
                self.idx += 1
                self._play()
                return
            if self.queue:
                self.sents = self.queue.pop(0)
                self.idx = 0
                self.cache.clear()
                self._play()
                return
            self.sents, self.idx = [], 0
            self.cache.clear()
            self._dump()

    def _play(self):
        self.word = -1
        w = self._wav(self.idx)
        if not w:
            return
        self.mpv.cmd("loadfile", w, "replace")
        self.mpv.cmd("set_property", "speed", self.speed)
        self.mpv.cmd("set_property", "pause", False)
        self.paused = False
        self.wake.set()
        self._dump()

    def status(self):
        cur = self.sents[self.idx] if 0 <= self.idx < len(self.sents) else ""
        done = sorted(self.cache)
        ms = list(self.render_ms.values())
        return {
            "sentence": cur,
            "index": self.idx,
            "total": len(self.sents),
            "paused": self.paused,
            "speed": round(self.speed, 2),
            "rendered": len(done),
            "rendering": self.rendering,
            "loaded": self.tts is not None,
            "voice": self.sid,
            "word": self.word,
            "engine": self.engine,
            "engines": self.engines(),
            "aligner": self.aligner,
            "queue_len": len(self.queue),
            "font_size": self.font_size,
            "words_visible": self.words_visible,
            "position": self.position,
            "voices": (self.tts.num_speakers or 1) if self.tts else None,
            "last_render_ms": self.render_ms.get(self.idx),
            "avg_render_ms": int(sum(ms) / len(ms)) if ms else None,
        }

    def _dump(self):
        with self.dump_lock:
            tmp = f"{STATE}.{os.getpid()}.new"
            with open(tmp, "w") as f:
                json.dump(self.status(), f)
            os.replace(tmp, STATE)

    def read(self, text):
        with self.lock:
            self.sents = sentences(text)
            self.idx = 0
            self.cache.clear()
            if not self.sents:
                return "empty"
            self._play()
            return f"reading {len(self.sents)}"

    def stop(self):
        with self.lock:
            self.mpv.cmd("stop")
            self.sents, self.idx, self.paused, self.word = [], 0, False, -1
            self.cache.clear()
            self.spans.clear()
            self._dump()
            return "stopped"

    def toggle(self):
        with self.lock:
            if not self.sents:
                return "idle"
            self.paused = not self.paused
            self.mpv.cmd("set_property", "pause", self.paused)
            self._dump()
            return "paused" if self.paused else "resumed"

    def seek(self, delta):
        with self.lock:
            if not self.sents:
                return "idle"
            self.idx = max(0, min(len(self.sents) - 1, self.idx + delta))
            self._play()
            return f"{self.idx + 1}/{len(self.sents)}"

    def catalogue(self):
        import voices as V
        n = self.voices()
        if self.engine == "kokoro":
            return V.kokoro_catalogue()[:n]
        if self.engine == "remote":
            eng = self._ensure()
            return [{"sid": i, "name": v, "id": v, "lang": "?", "gender": "?"}
                    for i, v in enumerate(eng.voices)]
        if self.engine == "zipvoice":
            return [{"sid": i,
                     "name": os.path.splitext(os.path.basename(p))[0],
                     "id": "clone", "lang": "en-US", "gender": "cloned"}
                    for i, p in enumerate(list_clone_refs())]
        pitches = load_pitch_table(self.engine)
        return [V.anon_entry(i, pitches.get(str(i))) for i in range(n)]

    def preview(self, sid):
        with self.lock:
            if not (isinstance(sid, int) and 0 <= sid < self.voices()):
                return "out of range"
            try:
                samples, rate = self._ensure().generate(
                    "The quick brown fox jumps over the lazy dog.",
                    sid=sid, speed=1.0)
            except Exception as e:
                return f"preview failed: {e}"
            p = os.path.join(self.tmp, f"preview-{self.engine}-{sid}.wav")
            write_wav(p, samples, rate)
            self.mpv.cmd("loadfile", p, "replace")
            self.mpv.cmd("set_property", "pause", False)
            return f"preview {sid}"

    def is_active(self):
        with self.lock:
            return bool(self.sents)

    def enqueue(self, text):
        s = sentences(text)
        if not s:
            return "empty text"
        with self.lock:
            if not self.sents or self.paused:
                self.sents = s
                self.idx = 0
                self.cache.clear()
                self._play()
                return f"playing {len(s)} sentences"
            else:
                self.queue.append(s)
                self._dump()
                return f"queued {len(s)} sentences (queue length: {len(self.queue)})"

    def clear_queue(self):
        with self.lock:
            self.queue.clear()
            self.stop()
            return "queue cleared"

    def skip_queue(self):
        with self.lock:
            if self.queue:
                self.queue.pop(0)
                return f"skipped item (queue length: {len(self.queue)})"
            return self.stop()


def _num(cmd, cast):
    """Numeric argument of `cmd`, or None when missing or unparseable.

    Conversion happens here rather than at the call site because an uncaught
    ValueError in the command dispatch terminates the daemon: a client sending
    `voice af_kore` crash-looped the service 51 times before this existed.
    """
    parts = cmd.split()
    if len(parts) < 2:
        return None
    try:
        return cast(parts[1])
    except ValueError:
        return None


def selection():
    if sys.platform == "darwin":
        cmds = [["pbpaste"]]
    else:
        cmds = [["wl-paste", "--primary", "--no-newline"],
                ["xclip", "-selection", "primary", "-o"]]
    for c in cmds:
        try:
            r = subprocess.run(c, capture_output=True, text=True, timeout=2)
            if r.returncode == 0 and r.stdout.strip():
                return r.stdout
        except (OSError, subprocess.TimeoutExpired):
            continue
    return ""


def _syllables(word):
    w = re.sub(r"[^a-z]", "", word.lower())
    if not w:
        return 1
    n = len(re.findall(r"[aeiouy]+", w))
    if w.endswith("e") and n > 1 and not w.endswith(("le", "ee", "ye")):
        n -= 1
    return max(1, n)


def word_spans(samples, rate, words):
    import numpy as np

    if not words:
        return []
    x = np.asarray(samples, dtype=np.float32)
    dur = len(x) / float(rate)
    if len(words) == 1 or dur <= 0:
        return [(0.0, dur)]

    hop = max(1, int(rate * 0.010))
    n_frames = max(1, len(x) // hop)
    frames = x[: n_frames * hop].reshape(n_frames, hop)
    rms = np.sqrt((frames.astype(np.float64) ** 2).mean(axis=1) + 1e-12)
    if rms.max() <= 0:
        return _proportional(words, dur)
    rms /= rms.max()

    quiet = rms < 0.08
    cands = []
    i = 0
    while i < n_frames:
        if quiet[i]:
            j = i
            while j < n_frames and quiet[j]:
                j += 1
            if (j - i) * hop / rate >= 0.020:
                cands.append(((i + j) / 2.0) * hop / rate)
            i = j
        else:
            i += 1

    syl = [_syllables(w) for w in words]
    total = float(sum(syl))
    acc, expected = 0.0, []
    for s in syl[:-1]:
        acc += s
        expected.append(dur * acc / total)

    widths = [dur * s / total for s in syl]
    floors = [max(0.06, w * 0.45) for w in widths]
    tail = [sum(floors[k + 1:]) for k in range(len(words))]

    bounds = []
    prev = 0.0
    for k, exp in enumerate(expected):
        earliest = prev + floors[k]
        latest = dur - tail[k]
        pick = exp
        if cands and latest > earliest:
            window = max(0.12, widths[k] * 0.6)
            usable = [c for c in cands if earliest <= c <= latest
                      and abs(c - exp) <= window]
            if usable:
                pick = min(usable, key=lambda c: abs(c - exp))
        bounds.append(min(max(pick, earliest), max(latest, earliest)))
        prev = bounds[-1]

    edges = [0.0] + bounds + [dur]
    return [(edges[i], edges[i + 1]) for i in range(len(words))]


def asr_spans(rec, samples, rate, words):
    import numpy as np

    st = rec.create_stream()
    st.accept_waveform(rate, np.asarray(samples, dtype=np.float32))
    rec.decode_stream(st)
    r = st.result
    starts = [t for tok, t in zip(r.tokens, r.timestamps) if tok.startswith(" ")]
    if len(starts) != len(words):
        return None
    dur = len(samples) / float(rate)
    ends = starts[1:] + [dur]
    out = []
    for a, b in zip(starts, ends):
        out.append((max(0.0, a), max(b, a + 0.02)))
    return out


def _proportional(words, dur):
    syl = [_syllables(w) for w in words]
    total = float(sum(syl))
    out, acc = [], 0.0
    for s in syl:
        nxt = acc + dur * s / total
        out.append((acc, nxt))
        acc = nxt
    return out


def write_wav(path, samples, rate):
    import array
    import wave
    pcm = array.array("h", (int(max(-1.0, min(1.0, s)) * 32767) for s in samples))
    if sys.byteorder == "big":
        pcm.byteswap()
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(pcm.tobytes())


def load_pitch_table(engine):
    d = os.environ.get("TTS_PITCH_DIR", "")
    if not d:
        return {}
    try:
        with open(os.path.join(d, f"{engine}.json")) as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def load_prefs():
    try:
        with open(PREFS) as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def save_prefs(reader):
    try:
        os.makedirs(os.path.dirname(PREFS), exist_ok=True)
        tmp = PREFS + ".new"
        with open(tmp, "w") as f:
            json.dump({
                "engine": reader.engine,
                "voice": reader.sid,
                "speed": reader.speed,
                "aligner": reader.aligner,
                "font_size": reader.font_size,
                "words_visible": reader.words_visible,
                "position": reader.position,
            }, f)
        os.replace(tmp, PREFS)
    except OSError:
        pass


def cycle_mode():
    try:
        with open(MODE_FILE) as f:
            cur = f.read().strip()
    except OSError:
        cur = MODES[0]
    nxt = MODES[(MODES.index(cur) + 1) % len(MODES)] if cur in MODES else MODES[0]
    with open(MODE_FILE, "w") as f:
        f.write(nxt)
    return nxt


def main():
    model_dirs = json.loads(os.environ["TTS_MODEL_DIRS"])
    threads = int(os.environ.get("TTS_THREADS", "4"))
    prefs = load_prefs()
    engine = prefs.get("engine", DEFAULT_ENGINE)
    if engine not in ENGINES and not (
            (engine == "remote" and RemoteEngine.available())
            or (engine == "zipvoice" and list_clone_refs())):
        engine = DEFAULT_ENGINE
    reader = Reader(None, model_dirs, threads, engine)
    reader.tts = build_engine(engine, model_dirs, threads,
                              reader._recognizer_or_none())
    reader.aligner = prefs.get("aligner", DEFAULT_ALIGNER)
    reader.speed = prefs.get("speed", 1.0)
    reader.font_size = prefs.get("font_size", DEFAULT_FONT_SIZE)
    reader.words_visible = prefs.get("words_visible", DEFAULT_WORDS_VISIBLE)
    reader.position = prefs.get("position", DEFAULT_POSITION)
    sid = prefs.get("voice")
    if isinstance(sid, int) and 0 <= sid < reader.voices():
        reader.sid = sid
    try:
        os.unlink(SOCK)
    except FileNotFoundError:
        pass
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(SOCK)
    try:
        os.chmod(SOCK, 0o666)
    except OSError:
        pass
    srv.listen(8)
    while True:
        conn, _ = srv.accept()
        with conn:
            data = conn.recv(4096)
            cmd = data.decode(errors="replace").strip()
            if cmd == "read":
                reply = reader.stop() if reader.is_active() else reader.read(selection())
            elif cmd == "stop":
                reply = reader.stop()
            elif cmd == "toggle":
                reply = reader.toggle()
            elif cmd == "next":
                reply = reader.seek(1)
            elif cmd == "back":
                reply = reader.seek(-1)
            elif cmd == "faster":
                reply = reader.nudge(SPEED_STEP)
            elif cmd == "slower":
                reply = reader.nudge(-SPEED_STEP)
            elif cmd == "status":
                reply = json.dumps(reader.status(), indent=2)
            elif cmd == "mode":
                reply = cycle_mode()
            elif cmd == "unload":
                reply = reader.unload()
            elif cmd == "reload":
                reply = reader.reload()
            elif cmd == "clear":
                reply = reader.clear_queue()
            elif cmd == "skip":
                reply = reader.skip_queue()
            elif cmd == "reset" or cmd == "reset_prefs":
                reply = reader.reset_prefs()
            elif cmd == "quit":
                try:
                    conn.send(b"bye")
                except OSError:
                    pass
                reader.mpv.cmd("quit")
                os._exit(0)
            elif cmd.startswith("say ") or cmd.startswith("speak "):
                reply = reader.read(cmd.split(" ", 1)[1])
            elif cmd.startswith("queue "):
                reply = reader.enqueue(cmd.split(" ", 1)[1])
            elif cmd.startswith("speed "):
                reply = reader.set_speed(_num(cmd, float))
            elif cmd.startswith("voice "):
                token = cmd.split(None, 1)[1]
                sid = reader.resolve_voice(token)
                reply = (f"unknown voice: {token}" if sid is None
                         else reader.set_voice(sid))
            elif cmd.startswith("engine "):
                reply = reader.set_engine(cmd.split()[1])
            elif cmd.startswith("aligner "):
                reply = reader.set_aligner(cmd.split()[1])
            elif cmd.startswith("font_size "):
                reply = reader.set_font_size(_num(cmd, int))
            elif cmd.startswith("words_visible "):
                reply = reader.set_words_visible(_num(cmd, int))
            elif cmd.startswith("position "):
                reply = reader.set_position(cmd.split()[1])
            elif cmd == "catalogue":
                reply = json.dumps(reader.catalogue())
            elif cmd.startswith("preview "):
                token = cmd.split(None, 1)[1]
                sid = reader.resolve_voice(token)
                reply = (f"unknown voice: {token}" if sid is None
                         else reader.preview(sid))
            else:
                reply = f"unknown: {cmd}"
            try:
                conn.send(str(reply).encode())
            except OSError:
                pass


if __name__ == "__main__":
    main()
