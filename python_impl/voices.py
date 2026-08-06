"""Voice metadata.

sherpa's model packages carry no voice names -- voices.bin is bare float data --
so the catalogue is reconstructed here.
"""

KOKORO_V1_0 = [
    "af_alloy", "af_aoede", "af_bella", "af_heart", "af_jessica", "af_kore",
    "af_nicole", "af_nova", "af_river", "af_sarah", "af_sky", "am_adam",
    "am_echo", "am_eric", "am_fenrir", "am_liam", "am_michael", "am_onyx",
    "am_puck", "am_santa", "bf_alice", "bf_emma", "bf_isabella", "bf_lily",
    "bm_daniel", "bm_fable", "bm_george", "bm_lewis", "ef_dora", "em_alex",
    "ff_siwis", "hf_alpha", "hf_beta", "hm_omega", "hm_psi", "if_sara",
    "im_nicola", "jf_alpha", "jf_gongitsune", "jf_nezumi", "jf_tebukuro",
    "jm_kumo", "pf_dora", "pm_alex", "pm_santa", "zf_xiaobei", "zf_xiaoni",
    "zf_xiaoxiao", "zf_xiaoyi", "zm_yunjian", "zm_yunxi", "zm_yunxia",
    "zm_yunyang",
]

LANGS = {
    "a": "en-US", "b": "en-GB", "e": "es", "f": "fr", "h": "hi",
    "i": "it", "j": "ja", "p": "pt-BR", "z": "zh",
}
GENDERS = {"f": "female", "m": "male"}


def kokoro_entry(sid, name):
    lang = LANGS.get(name[0], "?")
    gender = GENDERS.get(name[1], "?")
    return {
        "sid": sid,
        "name": name.split("_", 1)[1].title(),
        "id": name,
        "lang": lang,
        "gender": gender,
    }


def kokoro_catalogue():
    return [kokoro_entry(i, n) for i, n in enumerate(KOKORO_V1_0)]


def anon_entry(sid, pitch_hz=None):
    if pitch_hz:
        gender = "male" if pitch_hz < 155 else "female"
        register = ("deep" if pitch_hz < 110 else "low" if pitch_hz < 155
                    else "mid" if pitch_hz < 195 else "high")
        name = f"{register} {gender} {pitch_hz:.0f}Hz"
    else:
        gender, name = "?", f"voice {sid}"
    return {"sid": sid, "name": name, "id": f"sid{sid}",
            "lang": "en-US", "gender": gender,
            **({"pitch_hz": round(pitch_hz, 1)} if pitch_hz else {})}
