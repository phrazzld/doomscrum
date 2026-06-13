# Video quality pipeline research

Status: research packet and implementation direction, 2026-06-13.

## Goal

Make DoomScrum clips more coherent, legible, and intelligible without making
native-audio video models the default cost floor.

The core decision: keep the ticket script as the source of truth, generate
audio from that exact script, align captions to that audio, then compose the
visual layer separately. Native-audio video models are a hero tier, not the
quality baseline.

## What the market is doing

The mature stacks are converging on orchestration instead of one magic model.

- Runway exposes a portfolio of image, video, and audio models with explicit
  per-second costs. Its pricing makes the trade clear: Gen-4 Turbo is 5
  credits/s ($0.05/s at $0.01/credit), Veo 3.1 Fast is 10 credits/s without
  audio and 15 credits/s with audio, and full Veo 3.1 is 20 credits/s without
  audio and 40 credits/s with audio.
  Source: https://docs.dev.runwayml.com/guides/pricing/
- fal's Veo 3.1 page exposes the same shape: 720p/1080p standard is $0.20/s
  without audio or $0.40/s with audio; the Fast tier is $0.10/s without audio
  or $0.15/s with audio. fal also emphasizes that dialogue must be short
  enough to fit the 8-second clip window.
  Source: https://fal.ai/models/fal-ai/veo3.1
- Luma describes a text-to-production workflow: briefs become shot lists,
  prompts, generated clips, captions, subtitles, karaoke-style word timing
  when timestamps exist, and code-assisted editing in Remotion. That is the
  right product pattern for us: plan, generate, compose, evaluate.
  Source: https://lumalabs.ai/learning-hub/luma-natural-language-text-instructions-to-production-workflows
- Remotion treats captions as a first-class timeline item. Its `Caption`
  shape is the same kind of data we should persist: text, start/end
  milliseconds, timestamp, and confidence. Its editor flow extracts audio,
  transcribes it, converts output into captions, then lets users or code adjust
  tokens, typography, page duration, and word timings.
  Sources: https://www.remotion.dev/docs/editor-starter/captioning and
  https://www.remotion.dev/docs/captions/caption
- ElevenLabs Forced Alignment takes known spoken audio plus known text and
  returns a time-aligned transcript. That is better than blind ASR for our
  deterministic TTS path because we already know what the script should say.
  Source: https://elevenlabs.io/docs/overview/capabilities/forced-alignment
- Deepgram documents the fallback ASR path for WebVTT/SRT generation from
  prerecorded audio, including utterance timestamps and caption serialization.
  Source: https://developers.deepgram.com/docs/automatically-generating-webvtt-and-srt-captions
- Kokoro-82M is the relevant cheap/local TTS baseline: open weights, Apache
  licensed, 82M parameters, and the model card reports served API rates under
  $1 per million characters as of April 2025. For our 8-12 second clips, TTS
  should be effectively free compared to video generation.
  Source: https://huggingface.co/hexgrad/Kokoro-82M

## Recommended DoomScrum pipeline

### 1. Script is authoritative

Keep the current spec-derived script constraints, but make them stricter for
media generation:

- Target 35-45 spoken words for 12-second clips and 22-30 words for 8-second
  clips.
- Preserve the exact ticket goal and one oracle phrase, not a paraphrase.
- Store `expected_script.txt` in render provenance before any paid media call.

### 2. Generate deterministic narration

Default to local or ultra-cheap TTS:

- Local/dev: Kokoro-82M or fixture audio.
- Paid quality: ElevenLabs/Runway Eleven, OpenAI TTS, or another provider only
  when a profile asks for it.
- Never ask a native video model to invent the spoken content on the baseline
  path.

### 3. Align captions to the known script

Primary path: forced alignment of `expected_script.txt` against the rendered
narration. Fallback path: ASR transcription with a normalized diff against the
expected script.

Persist a provider-neutral caption artifact:

```json
{
  "source": "forced_alignment",
  "words": [
    {
      "text": "Render",
      "start_ms": 120,
      "end_ms": 360,
      "confidence": 0.98
    }
  ],
  "normalized_expected": "render the viewport not the backlog",
  "normalized_observed": "render the viewport not the backlog"
}
```

This mirrors Remotion's caption data model while staying Rust-friendly.

### 4. Compose visuals separately

Default mix rung:

- Generate one bespoke keyframe image from the ticket scene prompt.
- Add Ken Burns/parallax/camera shake locally.
- Add deterministic narration.
- Burn word-synced captions with a conservative high-contrast style.

Motion upgrade rung:

- Generate a silent or visual-only video with a cheaper model.
- Keep deterministic narration and burned captions.
- Use native generated audio only when it is aesthetic background, not the
  source of truth for the spoken script.

Hero rung:

- Use Veo/Sora/Seedance/Kling-style native-audio models only for selected
  cards or paid tiers.
- Still run transcript/caption verification afterward.
- If transcript differs from the expected script, keep the visual only when it
  can be salvaged by replacing audio and captions; otherwise reject.

## QA gate

Upgrade `scripts/check_script_fit.py` from a fit check into a render verdict.
The verdict should be stored next to every clip and should block feed admission
for paid renders.

Minimum checks:

- Audio stream exists, has non-trivial loudness, and does not clip.
- Speech ends with a buffer before the last frame.
- Caption artifact covers at least 95% of expected normalized words on ASR
  paths, and exactly covers the expected script on forced-alignment paths.
- No word confidence below the configured floor for words that appear in the
  ticket goal or oracle.
- Burned caption pages never exceed two lines, never exceed the safe width,
  and maintain contrast against sampled background frames.
- A review thumbnail sheet and transcript diff are emitted as artifacts.

Failure policy:

- If audio/caption QA fails on the deterministic path, re-run TTS or alignment,
  not the visual render.
- If visual legibility fails, restyle/recompose captions first.
- If a native-audio video fails transcript verification, do not spend another
  native render until a cheaper deterministic salvage attempt has failed.

## Cost ladder

| Tier | Path | Expected role |
|---|---|---|
| $0 | Fixture/local composition | Dev, tests, first-run demos |
| cents | Kokoro/fixture TTS + local composition | Default quality floor |
| ~$0.05/clip | AI keyframe + Ken Burns + cheap/local TTS + captions | Main paid mix weight |
| ~$0.25-$0.80/clip | Cheap silent/visual video + deterministic audio/captions | Motion upgrade |
| $1+ | Native-audio premium video | Hero cards and paid tiers |

Exact vendor prices must be quoted at render time. This document is a routing
decision, not a hard-coded price table.

## Implementation order

1. Land a provider-neutral caption artifact and provenance schema.
2. Make the feed overlay/burn-in path consume that artifact.
3. Build the stills + TTS + forced-alignment pipeline behind a profile.
4. Promote `check_script_fit.py` into `render_verdict.py` with screenshot,
   transcript, caption, and audio checks.
5. Add a small golden render eval: fixture audio, one deterministic TTS clip,
   one native-audio clip, and one deliberately bad caption/audio sample.

## Product implication

The product should market "legible backlog clips" rather than "fully native
AI video." The boring pipeline is the one that makes users trust the ticket
content, and trust matters more than cinematic novelty for a tool that can
open real pull requests.
