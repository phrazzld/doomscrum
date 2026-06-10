#!/usr/bin/env python3
"""Verify a render speaks its full script inside the clip.

Usage: check_script_fit.py <render.mp4> [expected script text]

Extracts the audio track, transcribes it through fal-ai/whisper (same
account as generation), and reports:
  - the transcript with segment timings
  - whether speech ends before the clip does (no cutoff)
  - what fraction of the expected script's words were actually spoken

Exit code 0 = script fits, 1 = cutoff or low coverage, 2 = usage/infra.
"""

import base64
import json
import re
import subprocess
import sys
import time
import urllib.request
from pathlib import Path


def fal_key() -> str:
    import os

    for name in ("FAL_API_KEY", "FAL_KEY"):
        if os.environ.get(name):
            return os.environ[name]
    secrets = Path.home() / ".secrets"
    if secrets.exists():
        for line in secrets.read_text().splitlines():
            m = re.match(r"export (FAL_API_KEY|FAL_KEY)=(.+)", line.strip())
            if m:
                return m.group(2).strip().strip('"')
    sys.exit("no FAL key in env or ~/.secrets")


def upload(wav: bytes, key: str) -> str:
    """fal rejects data URIs ("Unsupported data URL"); use fal storage."""
    init = json.load(urllib.request.urlopen(urllib.request.Request(
        "https://rest.alpha.fal.ai/storage/upload/initiate",
        data=json.dumps({"file_name": "audio.wav", "content_type": "audio/wav"}).encode(),
        headers={"authorization": f"Key {key}", "content-type": "application/json"},
    )))
    urllib.request.urlopen(urllib.request.Request(
        init["upload_url"], data=wav, method="PUT",
        headers={"content-type": "audio/wav"},
    ))
    return init["file_url"]


def transcribe(mp4: Path, key: str) -> dict:
    wav = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", str(mp4), "-ac", "1", "-ar", "16000",
         "-f", "wav", "-"],
        capture_output=True, check=True,
    ).stdout
    body = json.dumps({
        "audio_url": upload(wav, key),
        "task": "transcribe",
        "language": "en",
        "chunk_level": "segment",
    }).encode()
    req = urllib.request.Request(
        "https://queue.fal.run/fal-ai/whisper",
        data=body,
        headers={"authorization": f"Key {key}", "content-type": "application/json"},
    )
    queued = json.load(urllib.request.urlopen(req))
    status_url, response_url = queued["status_url"], queued["response_url"]
    for _ in range(120):
        sreq = urllib.request.Request(status_url, headers={"authorization": f"Key {key}"})
        status = json.load(urllib.request.urlopen(sreq))["status"]
        if status == "COMPLETED":
            rreq = urllib.request.Request(response_url, headers={"authorization": f"Key {key}"})
            return json.load(urllib.request.urlopen(rreq))
        if status == "FAILED":
            sys.exit("whisper job failed")
        time.sleep(1)
    sys.exit("whisper timed out")


def norm_words(text: str) -> list[str]:
    return [w for w in re.sub(r"[^a-z0-9 ]", " ", text.lower()).split() if w]


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    mp4 = Path(sys.argv[1])
    expected = sys.argv[2] if len(sys.argv) > 2 else ""

    duration = float(subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "format=duration",
         "-of", "csv=p=0", str(mp4)],
        capture_output=True, text=True, check=True,
    ).stdout.strip())

    result = transcribe(mp4, fal_key())
    text = result.get("text", "").strip()
    chunks = result.get("chunks", [])
    speech_end = max((c["timestamp"][1] or duration for c in chunks), default=0.0)

    print(f"clip      : {mp4.name}  ({duration:.2f}s)")
    print(f"transcript: {text}")
    for c in chunks:
        t0, t1 = c["timestamp"]
        print(f"  [{t0:5.2f} – {t1 if t1 is not None else duration:5.2f}] {c['text'].strip()}")
    print(f"speech end: {speech_end:.2f}s of {duration:.2f}s")

    ok = True
    # A final segment with a null/At-end timestamp or ending in the last
    # 0.3s suggests the audio ran out mid-line.
    if speech_end > duration - 0.3:
        print("warning   : speech runs to the very last frame")
        ok = False
    if expected:
        want, got = norm_words(expected), set(norm_words(text))
        missing = [w for w in want if w not in got]
        coverage = 1 - len(missing) / max(len(want), 1)
        print(f"coverage  : {coverage:.0%} of expected words spoken"
              + (f"; missing: {' '.join(missing)}" if missing else ""))
        if coverage < 0.8:
            ok = False
    print("verdict   : COMPLETE" if ok else "verdict   : CUT OFF / INCOMPLETE")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
