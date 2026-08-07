"""Synthesis engine construction, separated from the daemon.

Split out of `daemon.py` so the build-time pitch measurement does not depend on
it. `measure.py` needs only `build_engine`, but importing it from `daemon`
dragged the whole daemon into the Nix `measureSrc` derivation, so every edit to
socket handling or playback re-measured 904 LibriTTS voices -- about a minute of
synthesis per rebuild. Engines change rarely; the daemon changes constantly.
"""
import os

CONFIG_DIR = os.path.join(
    os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config"), "tts")
CLONE_DIR = os.path.join(CONFIG_DIR, "voices")

DEFAULT_ENGINE = "kokoro"


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
