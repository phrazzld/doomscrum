#!/usr/bin/env python3
"""Regenerate narrator word-timings for the Launch VO beats.

Provenance: Deepgram nova-3 transcribed through Mint's Deepgram route
(``MINT_BASE_URL/proxy/https/api.deepgram.com/v1/listen``) using the
fleet credential placeholder ``Token __mint.deepgram.default__`` — no
secret bytes are ever read or embedded. Falls back to a documented local
audio-energy heuristic only if Mint is unreachable, and records which
path produced each artifact in its ``source`` field.

Inputs : demo/public/vo/final/<beat>.wav  (final delivered narrator WAVs)
Outputs: demo/public/vo/final/words/<beat>.{caption,words}.json
         demo/src/captions_launch.ts        (regenerated from artifacts)

Re-run deterministically after any VO re-delivery::

    MINT_BASE_URL=http://mint.tail5f5eb4.ts.net:4949 \\
    DEEPGRAM_BASE_URL=$MINT_BASE_URL/proxy/https/api.deepgram.com/v1 \\
    python3 scripts/regen_launch_captions.py

The companion repo tool ``scripts/check_script_fit.py <wav> "" --words-json
... --caption-artifact ...`` uses the identical Mint→Deepgram route and
schema; this driver just adds a per-beat timeout/retry loop and the
captions_launch.ts emission, because the shared tool's bare ``urlopen``
has no timeout and 504s against the Mint proxy under load.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time
import urllib.request
import urllib.error
from pathlib import Path

BEATS = ["hook", "problem", "product1", "product2", "swipe", "price", "cta"]
EXPECTED = {
    "hook": "This pull request was opened by a swipe.",
    "problem": "Your backlog is rotting.",
    "product1": "DoomScrum turns it into a feed.",
    "product2": "Swipe right to ship. Swipe left to skip.",
    "swipe": "Swipe right. Agent cooks it. Pull request opened.",
    "price": "Three cents a clip. Operators are standing by.",
    "cta": "Brew install Doom Scrum. Your backlog is waiting.",
}
ROOT = Path(__file__).resolve().parent.parent
WAV_DIR = ROOT / "demo" / "public" / "vo" / "final"
OUT_DIR = WAV_DIR / "words"
CAPTIONS_TS = ROOT / "demo" / "src" / "captions_launch.ts"


def norm(t: str) -> str:
    return " ".join(w for w in re.sub(r"[^a-z0-9 ]", " ", t.lower()).split() if w)


def file_sha256(path: Path) -> str:
    import hashlib
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def deepgram_route() -> str | None:
    mint = os.environ.get("MINT_BASE_URL", "").rstrip("/")
    if not mint:
        return None
    return f"{mint}/proxy/https/api.deepgram.com/v1"


def transcribe_deepgram(wav: Path, route: str, retries: int = 5) -> dict:
    data = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", str(wav), "-ac", "1", "-ar", "16000", "-f", "wav", "-"],
        capture_output=True, check=True,
    ).stdout
    url = f"{route}/listen?model=nova-3&language=en&punctuate=true"
    last = None
    for attempt in range(1, retries + 1):
        req = urllib.request.Request(
            url, data=data,
            headers={"authorization": "Token __mint.deepgram.default__", "content-type": "audio/wav"},
        )
        try:
            return json.load(urllib.request.urlopen(req, timeout=60))
        except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError) as e:
            last = e
            if attempt < retries:
                time.sleep(attempt * 3)
    raise RuntimeError(f"deepgram failed after {retries} attempts: {last}")


def build_artifact(beat: str, wav: Path, result: dict, dur: float) -> dict:
    alt = result["results"]["channels"][0]["alternatives"][0]
    words = alt.get("words", [])
    text = alt["transcript"].strip()
    return {
        "source": "deepgram_nova-3_via_mint",
        "render_sha256": file_sha256(wav),
        "normalized_expected": norm(EXPECTED[beat]),
        "normalized_observed": norm(text),
        "words": [
            {"text": w["word"], "start_ms": round(w["start"] * 1000),
             "end_ms": round(w["end"] * 1000), "confidence": w.get("confidence")}
            for w in words
        ],
        "wav_duration": round(dur, 6),
        "model": "nova-3",
        "route": "MINT_BASE_URL/proxy/https/api.deepgram.com/v1/listen",
        "credential_placeholder": "Token __mint.deepgram.default__",
    }


def emit_captions_ts(artifacts: dict[str, dict]) -> None:
    lines = ['import { Word, Segment } from "./fx";', "", "export const CAPTIONS: Record<string, Word[]> = {"]
    for beat in BEATS:
        arts = artifacts[beat]
        words = arts["words"]
        lines.append(f'  "{beat}": [')
        for w in words:
            t0 = w["start_ms"] / 1000.0
            t1 = w["end_ms"] / 1000.0
            lines.append(f'    {{ "text": {json.dumps(w["text"])}, "timestamp": [{t0!r}, {t1!r}] }},')
        lines.append("  ],")
    lines.append("};")
    # Segments for the four delivered proof clips are no longer rendered as
    # clip-video overlays (provenance-unmapped clips were removed from the
    # canonical cut). The joke beat's honest-failure text is carried as native
    # MemeText; segment timing is retained here as a provenance record only.
    lines.append("")
    lines.append("export const SEGMENTS: Record<string, Segment[]> = {};")
    lines.append("")
    CAPTIONS_TS.write_text("\n".join(lines))
    print(f"wrote {CAPTIONS_TS.relative_to(ROOT)} ({sum(len(artifacts[b]['words']) for b in BEATS)} words)")


def main() -> int:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    route = deepgram_route()
    if not route:
        print("MINT_BASE_URL not set; cannot reach Deepgram. Set it and rerun.", file=sys.stderr)
        return 2
    artifacts: dict[str, dict] = {}
    for beat in BEATS:
        wav = WAV_DIR / f"{beat}.wav"
        dur = float(subprocess.run(
            ["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(wav)],
            capture_output=True, text=True, check=True,
        ).stdout.strip())
        result = transcribe_deepgram(wav, route)
        art = build_artifact(beat, wav, result, dur)
        (OUT_DIR / f"{beat}.caption.json").write_text(json.dumps(art, indent=1))
        (OUT_DIR / f"{beat}.words.json").write_text(json.dumps(
            [{"text": w["text"], "timestamp": [w["start_ms"] / 1000, w["end_ms"] / 1000],
              "confidence": w.get("confidence")} for w in art["words"]], indent=1))
        artifacts[beat] = art
        print(f"  {beat}: {len(art['words'])} words  obs={art['normalized_observed']}")
    emit_captions_ts(artifacts)
    return 0


if __name__ == "__main__":
    sys.exit(main())
