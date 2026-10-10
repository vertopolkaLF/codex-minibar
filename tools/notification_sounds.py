"""Synthesizes the notification chimes in assets/sounds/.

Each sound is a few soft bell-like notes: a sine with faint upper partials, a short
attack, an exponential decay, and a quiet echo tail. Peaks are kept well below full
scale so the chimes sit under ordinary system sounds instead of shouting over them.

Usage: python tools/notification_sounds.py
"""

import pathlib
import wave

import numpy as np

RATE = 44_100
PEAK_DBFS = -20.0
OUT = pathlib.Path(__file__).resolve().parent.parent / "assets" / "sounds"


def hz(note: str) -> float:
    names = {"C": -9, "D": -7, "E": -5, "F": -4, "G": -2, "A": 0, "B": 2}
    semis = names[note[0]]
    rest = note[1:]
    if rest.startswith("#"):
        semis += 1
        rest = rest[1:]
    elif rest.startswith("b"):
        semis -= 1
        rest = rest[1:]
    return 440.0 * 2 ** ((semis + (int(rest) - 4) * 12) / 12)


def bell(freq: float, length: float, decay: float, warmth: float) -> np.ndarray:
    t = np.arange(int(RATE * length)) / RATE
    attack = np.minimum(t / 0.006, 1.0)
    tone = np.sin(2 * np.pi * freq * t)
    tone += warmth * 0.35 * np.sin(2 * np.pi * freq * 2 * t) * np.exp(-t / (decay * 0.5))
    tone += 0.08 * np.sin(2 * np.pi * freq * 3.01 * t) * np.exp(-t / (decay * 0.25))
    return tone * attack * np.exp(-t / decay)


def chime(notes, gap: float, decay: float = 0.16, warmth: float = 1.0) -> np.ndarray:
    length = gap * (len(notes) - 1) + decay * 6
    out = np.zeros(int(RATE * length))
    for i, note in enumerate(notes):
        start = int(RATE * gap * i)
        tone = bell(hz(note), length - gap * i, decay, warmth)
        # Later notes slightly softer so a phrase settles instead of climbing in volume.
        out[start : start + len(tone)] += tone * (0.85 ** i)
    for delay, gain in ((0.075, 0.22), (0.15, 0.1)):
        shift = int(RATE * delay)
        out[shift:] += gain * out[: len(out) - shift].copy()
    fade = min(len(out), int(RATE * 0.04))
    out[-fade:] *= np.linspace(1.0, 0.0, fade)
    return out / np.max(np.abs(out)) * 10 ** (PEAK_DBFS / 20)


SOUNDS = {
    "info": chime(["E6"], 0.0, decay=0.12),
    "success": chime(["C6", "G6"], 0.085),
    "reset": chime(["C6", "E6", "G6"], 0.07, decay=0.14),
    "warning": chime(["A5", "E5"], 0.12, decay=0.15),
    "error": chime(["E5", "C5"], 0.13, decay=0.13, warmth=1.6),
    "update": chime(["G5", "C6", "E6", "G6"], 0.075, decay=0.15),
}


def main() -> None:
    for name, samples in SOUNDS.items():
        pcm = np.round(samples * 32767).astype("<i2")
        with wave.open(str(OUT / f"{name}.wav"), "wb") as f:
            f.setnchannels(1)
            f.setsampwidth(2)
            f.setframerate(RATE)
            f.writeframes(pcm.tobytes())


if __name__ == "__main__":
    main()
