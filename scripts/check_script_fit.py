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
import urllib.error
import urllib.request
from pathlib import Path


def get_key(*names: str) -> str | None:
    import os

    for name in names:
        if os.environ.get(name):
            return os.environ[name]
    secrets = Path.home() / ".secrets"
    if secrets.exists():
        for line in secrets.read_text().splitlines():
            m = re.match(rf"export ({'|'.join(names)})=(.+)", line.strip())
            if m:
                return m.group(2).strip().strip('"')
    return None


def fal_key() -> str:
    key = get_key("FAL_API_KEY", "FAL_KEY")
    if not key:
        sys.exit("no FAL key in env or ~/.secrets")
    return key


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


def transcribe(mp4: Path, key: str, chunk_level: str = "segment") -> dict:
    wav = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", str(mp4), "-ac", "1", "-ar", "16000",
         "-f", "wav", "-"],
        capture_output=True, check=True,
    ).stdout
    body = json.dumps({
        "audio_url": upload(wav, key),
        "task": "transcribe",
        "language": "en",
        "chunk_level": chunk_level,
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


def transcribe_deepgram(mp4: Path, key: str) -> dict:
    """Deepgram fallback: direct binary upload, no storage hop. Returns the
    same {text, chunks:[{text, timestamp:[t0,t1]}]} shape as fal whisper
    (word-level chunks)."""
    wav = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", str(mp4), "-ac", "1", "-ar", "16000",
         "-f", "wav", "-"],
        capture_output=True, check=True,
    ).stdout
    req = urllib.request.Request(
        "https://api.deepgram.com/v1/listen?model=nova-3&language=en&punctuate=true",
        data=wav,
        headers={"authorization": f"Token {key}", "content-type": "audio/wav"},
    )
    result = json.load(urllib.request.urlopen(req))
    alt = result["results"]["channels"][0]["alternatives"][0]
    return {
        "text": alt.get("transcript", ""),
        "chunks": [
            {"text": w["word"], "timestamp": [w["start"], w["end"]]}
            for w in alt.get("words", [])
        ],
    }


def transcribe_any(mp4: Path, chunk_level: str) -> dict:
    """fal whisper first (account parity with generation); Deepgram when fal
    storage is unavailable (their upload endpoints started 403ing 2026-06-11)."""
    try:
        return transcribe(mp4, fal_key(), chunk_level)
    except urllib.error.HTTPError as e:
        dg = get_key("DEEPGRAM_API_KEY")
        if not dg:
            raise
        print(f"note      : fal transcription unavailable (HTTP {e.code}); using deepgram")
        return transcribe_deepgram(mp4, dg)


def norm_words(text: str) -> list[str]:
    return [w for w in re.sub(r"[^a-z0-9 ]", " ", text.lower()).split() if w]


def main() -> int:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    words_json = None
    for i, a in enumerate(sys.argv[1:]):
        if a == "--words-json":
            words_json = Path(sys.argv[1:][i + 1])
            args = [x for x in args if x != sys.argv[1:][i + 1]]
    if not args:
        print(__doc__)
        return 2
    mp4 = Path(args[0])
    expected = args[1] if len(args) > 1 else ""

    duration = float(subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "format=duration",
         "-of", "csv=p=0", str(mp4)],
        capture_output=True, text=True, check=True,
    ).stdout.strip())

    # Word-level chunks time each spoken word (for caption overlays) and
    # still give a valid speech-end for the cutoff check.
    result = transcribe_any(mp4, "word" if words_json else "segment")
    if words_json:
        words_json.write_text(json.dumps(result.get("chunks", []), indent=1))
        print(f"words     : saved {len(result.get('chunks', []))} word timings -> {words_json}")
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
        import difflib
        want, got = norm_words(expected), set(norm_words(text))
        # Exact match, or close inflection/transcription variant (config ->
        # configure, capisce -> capiche). This corrects measurement noise;
        # the 80% bar itself is unchanged.
        def spoken(w):
            return w in got or any(
                difflib.SequenceMatcher(None, w, g).ratio() >= 0.8 for g in got
            )
        missing = [w for w in want if not spoken(w)]
        coverage = 1 - len(missing) / max(len(want), 1)
        print(f"coverage  : {coverage:.0%} of expected words spoken"
              + (f"; missing: {' '.join(missing)}" if missing else ""))
        if coverage < 0.8:
            ok = False
    print("verdict   : COMPLETE" if ok else "verdict   : CUT OFF / INCOMPLETE")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
