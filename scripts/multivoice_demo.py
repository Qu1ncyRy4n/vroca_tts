#!/usr/bin/env python3
"""Two voices, alternating and then overlapping.

Renders a short piece in two distinct voices and plays it twice: once as a
dialogue, once with both voices at the same instant. The point is not the demo
-- it is the comparison. `rust-spec.md` open question 10-a asks whether
multi-voice playback should interleave or genuinely overlap, and that is far
easier to settle by listening than by arguing.

Runs entirely on its own. It renders through `engines.py` directly and plays
through its own `mpv` processes, so the daemon's queue, voice, and player are
never touched. Pass `--daemon` to route the alternating half through the live
daemon instead, which exercises the per-item `--voice` argument.

    python scripts/multivoice_demo.py --render-only   # silent, writes wavs
    python scripts/multivoice_demo.py                 # renders and PLAYS AUDIO
    python scripts/multivoice_demo.py --daemon        # alternating half via daemon
"""
import argparse
import json
import os
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "..", "python_impl"))
import engines  # noqa: E402

# Two voices with real distance between them. libritts is used because it
# renders in ~139ms, so the demo is about the voices rather than the wait.
ENGINE = "libritts"
VOICE_A = 3    # deep male, ~107Hz
VOICE_B = 4    # high female, ~212Hz

# Alternating: a dialogue between the thing and its description.
DIALOGUE = [
    (VOICE_A, "Above the left ear there is a fold of cortex "
              "roughly the size of a thumbprint."),
    (VOICE_B, "Broca found it in a man who could understand everything "
              "and say almost nothing."),
    (VOICE_A, "It does not store words. It orders them."),
    (VOICE_B, "Injure it, and meaning survives while sequence falls away."),
    (VOICE_A, "The vocabulary is entirely intact. It simply will not line up."),
    (VOICE_B, "So the region is not a dictionary. It is closer to a scheduler."),
    (VOICE_A, "This is a digital version of that fold."),
    (VOICE_B, "Text arrives unordered, and leaves as breath."),
]

# Overlapping: two lines meant to be heard at the same moment.
OVERLAP = [
    (VOICE_A, "Text arrives unordered, and leaves as breath, "
              "one sound after another, in the only order that means anything."),
    (VOICE_B, "Two voices at once are two voices lost, "
              "which is the whole reason speech insists on taking turns."),
]


def render(engine, lines, outdir, label):
    """Synthesize each line to its own wav. Returns paths in order."""
    paths = []
    for i, (sid, text) in enumerate(lines):
        out = os.path.join(outdir, f"{label}-{i}-v{sid}.wav")
        t0 = time.monotonic()
        samples, rate = engine.generate(text, sid=sid, speed=1.0)
        ms = int((time.monotonic() - t0) * 1000)
        _write_wav(out, samples, rate)
        dur = len(samples) / float(rate)
        print(f"  voice {sid:<4} {ms:>5}ms render  {dur:>5.1f}s audio   {text[:48]}...")
        paths.append(out)
    return paths


def _write_wav(path, samples, rate):
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


def _play(path, wait=True):
    p = subprocess.Popen(["mpv", "--no-video", "--no-terminal", "--really-quiet", path],
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    if wait:
        p.wait()
    return p


def play_alternating(paths):
    """One at a time. This is what channels would sound like interleaved."""
    print("\n--- ALTERNATING: one voice at a time ---")
    for p in paths:
        _play(p, wait=True)


def play_overlapping(paths):
    """All at once, on independent players.

    The daemon has a single mpv, so genuine overlap needs one player per
    stream. That is the cost being weighed in 10-a: interleaving needs one
    player and a scheduler, overlapping needs N players or a mixer.
    """
    print("\n--- OVERLAPPING: both voices simultaneously ---")
    procs = [_play(p, wait=False) for p in paths]
    for p in procs:
        p.wait()


def via_daemon(lines):
    """Alternating through the live daemon, using the per-item voice argument.

    Selecting a voice and then queueing does NOT work, because rendering
    happens later: every queued item would use whichever voice was current when
    it was finally rendered. The voice has to travel with the item.
    """
    import socket
    sock = os.path.join(os.environ.get("XDG_RUNTIME_DIR", "/tmp"), "tts.sock")

    def send(cmd):
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.settimeout(30)
        s.connect(sock)
        s.sendall(cmd.encode())
        out = b""
        while True:
            chunk = s.recv(65536)
            if not chunk:
                break
            out += chunk
        s.close()
        return out.decode()

    if not os.path.exists(sock):
        sys.exit("daemon not running: systemctl --user start tts")
    print(f"\n--- VIA DAEMON: engine {ENGINE}, per-item voices ---")
    print(" ", send(f"engine {ENGINE}"))
    print(" ", send("stop"))
    for sid, text in lines:
        print(" ", send(f"queue --voice sid{sid} {text}"))


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--render-only", action="store_true",
                    help="synthesize but play nothing")
    ap.add_argument("--daemon", action="store_true",
                    help="send the alternating half to the running daemon")
    ap.add_argument("--engine", default=ENGINE)
    args = ap.parse_args()

    if args.daemon:
        via_daemon(DIALOGUE)
        return

    if "TTS_MODEL_DIRS" not in os.environ:
        sys.exit("TTS_MODEL_DIRS is not set. Run inside the daemon's environment,\n"
                 "or copy it from a running daemon:\n"
                 "  export TTS_MODEL_DIRS=\"$(tr '\\0' '\\n' "
                 "< /proc/$(systemctl --user show tts.service -p MainPID --value)/environ "
                 "| grep '^TTS_MODEL_DIRS=' | cut -d= -f2-)\"")

    md = json.loads(os.environ["TTS_MODEL_DIRS"])
    threads = int(os.environ.get("TTS_THREADS", "4"))
    outdir = tempfile.mkdtemp(prefix="vroca-demo-")

    print(f"loading {args.engine}...")
    engine = engines.build_engine(args.engine, md, threads)

    print("\nrendering dialogue:")
    dialogue = render(engine, DIALOGUE, outdir, "dialogue")
    print("\nrendering overlap:")
    overlap = render(engine, OVERLAP, outdir, "overlap")

    if args.render_only:
        print(f"\nwavs in {outdir}")
        return

    print("\n" + "=" * 60)
    print("PLAYING AUDIO. Ctrl-C to stop.")
    print("=" * 60)
    play_alternating(dialogue)
    time.sleep(0.6)
    play_overlapping(overlap)
    print(f"\nwavs kept in {outdir}")
    print("Which one could you actually follow? That answers 10-a.")


if __name__ == "__main__":
    main()
