#!/usr/bin/env python3
"""Measure median F0 per voice, at build time.

Supertonic and LibriTTS-R ship no voice names, so the catalogue describes them
by pitch instead. That's a real measurement rather than invented metadata --
but it costs a synthesis pass per voice, so it runs once in a derivation and
lands in the store as JSON, not on every daemon start.

    measure.py <engine> <model_dir> <out.json>
"""
import json
import sys

import numpy as np

sys.path.insert(0, __file__.rsplit("/", 1)[0])
import engines  # noqa: E402

PROBE = "The quick brown fox jumps over the lazy dog."


def median_f0(samples, rate, lo=70, hi=350):
    x = np.asarray(samples, dtype=np.float32)
    fl, hop = int(0.04 * rate), int(0.02 * rate)
    vals = []
    for i in range(0, max(0, len(x) - fl), hop):
        f = x[i:i + fl]
        if np.sqrt((f ** 2).mean()) < 0.04:
            continue
        f = f - f.mean()
        a = np.correlate(f, f, "full")[fl - 1:]
        klo, khi = int(rate / hi), int(rate / lo)
        if khi >= len(a):
            continue
        seg = a[klo:khi]
        if seg.max() <= 0:
            continue
        vals.append(rate / (klo + int(seg.argmax())))
    return float(np.median(vals)) if vals else 0.0


def main():
    engine, model_dir, out = sys.argv[1], sys.argv[2], sys.argv[3]
    eng = engines.build_engine(engine, {engine: model_dir}, 4)
    table = {}
    for sid in range(eng.num_speakers):
        try:
            samples, rate = eng.generate(PROBE, sid=sid, speed=1.0)
            f0 = median_f0(samples, rate)
            if f0:
                table[str(sid)] = round(f0, 1)
        except Exception:
            continue
    with open(out, "w") as f:
        json.dump(table, f)
    print(f"{engine}: measured {len(table)}/{eng.num_speakers} voices")


if __name__ == "__main__":
    main()
