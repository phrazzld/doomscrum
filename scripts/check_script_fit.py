#!/usr/bin/env python3
"""Verify a render speaks its full script inside the clip.

Usage: check_script_fit.py <render.mp4> [expected script text]

Extracts the audio track, transcribes it through fal-ai/whisper (same
account as generation), and reports:
  - the transcript with segment timings, or a fresh caption artifact
  - whether speech ends before the clip does (no cutoff)
  - what fraction of the expected script's words were actually spoken

Exit code 0 = script fits, 1 = cutoff or low coverage, 2 = usage/infra.
"""

import base64
import hashlib
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
    (word-level chunks plus confidence when present)."""
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
            {
                "text": w["word"],
                "timestamp": [w["start"], w["end"]],
                "confidence": w.get("confidence"),
            }
            for w in alt.get("words", [])
        ],
    }


def transcribe_any(mp4: Path, chunk_level: str) -> tuple[str, dict]:
    """fal whisper first (account parity with generation); Deepgram when fal
    storage is unavailable (their upload endpoints started 403ing 2026-06-11)."""
    try:
        return "fal_whisper", transcribe(mp4, fal_key(), chunk_level)
    except urllib.error.HTTPError as e:
        dg = get_key("DEEPGRAM_API_KEY")
        if not dg:
            raise
        print(f"note      : fal transcription unavailable (HTTP {e.code}); using deepgram")
        return "deepgram", transcribe_deepgram(mp4, dg)


def norm_words(text: str) -> list[str]:
    return [w for w in re.sub(r"[^a-z0-9 ]", " ", text.lower()).split() if w]


def norm_text(text: str) -> str:
    return " ".join(norm_words(text))


def file_sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def load_caption_artifact(path: Path) -> dict:
    artifact = json.loads(path.read_text())
    if not isinstance(artifact.get("words"), list):
        raise ValueError(f"{path} is missing words[]")
    return artifact


def load_fresh_caption_artifact(path: Path, render_sha256: str) -> dict | None:
    if not path.exists():
        return None
    artifact = load_caption_artifact(path)
    return artifact if artifact.get("render_sha256") == render_sha256 else None


def chunk_to_caption_word(chunk: dict, duration: float) -> dict:
    t0, t1 = chunk["timestamp"]
    start = 0 if t0 is None else t0
    end = duration if t1 is None else t1
    return {
        "text": str(chunk.get("text", "")).strip(),
        "start_ms": round(float(start) * 1000),
        "end_ms": round(float(end) * 1000),
        "confidence": chunk.get("confidence"),
    }


def result_to_caption_artifact(
    source: str,
    result: dict,
    expected: str,
    render_sha256: str,
    duration: float,
) -> dict:
    chunks = result.get("chunks", [])
    text = result.get("text", "").strip()
    if not text:
        text = " ".join(str(c.get("text", "")).strip() for c in chunks).strip()
    return {
        "source": source,
        "render_sha256": render_sha256,
        "normalized_expected": norm_text(expected),
        "normalized_observed": norm_text(text),
        "words": [chunk_to_caption_word(c, duration) for c in chunks],
    }


def caption_artifact_to_result(artifact: dict) -> dict:
    words = artifact.get("words", [])
    chunks = [
        {
            "text": word.get("text", ""),
            "timestamp": [
                float(word.get("start_ms", 0)) / 1000,
                float(word.get("end_ms", 0)) / 1000,
            ],
        }
        for word in words
    ]
    text = " ".join(str(word.get("text", "")).strip() for word in words).strip()
    return {
        "text": text or artifact.get("normalized_observed", ""),
        "chunks": chunks,
    }


def parse_args(argv: list[str]) -> tuple[Path | None, Path | None, list[str]]:
    words_json = None
    caption_artifact = None
    positional = []
    i = 0
    while i < len(argv):
        arg = argv[i]
        if arg == "--words-json":
            i += 1
            if i >= len(argv):
                raise ValueError("--words-json requires a path")
            words_json = Path(argv[i])
        elif arg == "--caption-artifact":
            i += 1
            if i >= len(argv):
                raise ValueError("--caption-artifact requires a path")
            caption_artifact = Path(argv[i])
        elif arg.startswith("--"):
            raise ValueError(f"unknown option {arg}")
        else:
            positional.append(arg)
        i += 1
    return words_json, caption_artifact, positional


def main() -> int:
    try:
        words_json, caption_artifact, args = parse_args(sys.argv[1:])
    except ValueError as e:
        print(e)
        print(__doc__)
        return 2
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
    render_sha256 = file_sha256(mp4)

    # Word-level chunks time each spoken word (for caption overlays), and a
    # same-render artifact keeps QA from paying to re-transcribe the render.
    artifact = (
        load_fresh_caption_artifact(caption_artifact, render_sha256)
        if caption_artifact
        else None
    )
    if artifact:
        result = caption_artifact_to_result(artifact)
        print(f"captions  : reused fresh artifact -> {caption_artifact}")
    else:
        source, result = transcribe_any(mp4, "word" if (words_json or caption_artifact) else "segment")
        if caption_artifact:
            artifact = result_to_caption_artifact(
                source,
                result,
                expected,
                render_sha256,
                duration,
            )
            caption_artifact.parent.mkdir(parents=True, exist_ok=True)
            caption_artifact.write_text(json.dumps(artifact, indent=1))
            print(f"captions  : saved {len(artifact['words'])} word timings -> {caption_artifact}")
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
