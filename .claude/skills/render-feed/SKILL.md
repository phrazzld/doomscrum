---
name: render-feed
description: >-
  Generate and verify paid fal.ai video renders for the DoomScrum swipe feed.
  Use when: "regenerate content", "render the feed", "new renders", "bake-off
  a model", "switch video model", "re-roll a clip", or any task that spends
  FAL money. Trigger: /render-feed.
---

# render-feed

Paid renders for the swipe feed. Every dollar is tracked; every clip is
transcript-verified before it counts.

## Money rules

- Unit cost is quoted by `fal::unit_cost(cfg)` (snapped clip duration × model
  price). Spend cap lives in `doomscrum.toml` (`max_total_spend_usd`); raise
  it only with the owner's say-so. Provenance = render JSONs in
  `.doomscrum/renders/<spec-sha>/<render-id>.json`; the sum of their
  `cost_estimate_usd` (all statuses, including retired) is total spend.
- To remove a render from the feed without losing spend provenance, set its
  `status` to `"retired"` — never delete render dirs.
- FAL key: env `FAL_API_KEY`/`FAL_KEY` or `~/.secrets`. Never print it.
- Failed seedance moderation (422 after render) is unbilled; silent duds are
  billed. Budget ~1.3x for re-rolls.

## Model facts (verified on fal 2026-06-10 — re-verify before adopting)

Authoritative table: `src/providers/fal.rs` (`model_price_per_second`,
`clip_duration`, per-family request schemas) + comments in `doomscrum.toml`.

| Model | $/s w/ audio | Cap | Note |
|---|---|---|---|
| `bytedance/seedance-2.0/fast/text-to-video` | 0.2419 @720p | 15s | hero: native word-synced on-screen captions |
| `fal-ai/sora-2/text-to-video` | 0.10 | 12s | won 8s script-fit bake-off |
| `fal-ai/kling-video/o3/standard/text-to-video` | 0.112 | 15s | 1080p, structured `multi_prompt` |
| `fal-ai/veo3.1/lite` | 0.05 @720p, 0.08 @1080p | 8s ("4s"/"6s"/"8s") | cheapest dialogue; 9:16 native |
| `fal-ai/pixverse/v6/text-to-video` | 0.060 @720p (0.025 @360p silent) | 15s | draft tier for prompt iteration |

Schema check for a new model:
`curl -s "https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=<id>"`.

## Iron rule: no unverified paid render

After every render:

```sh
python3 scripts/check_script_fit.py <mp4> "<planned script>" [--words-json out.json]
```

`--words-json` saves word-level timings (caption overlays for the demo).

COMPLETE (exit 0) or it gets re-rolled/retired. The planned script comes from
the storyboard in `.doomscrum/storyboards/`. Word budget is
`(duration - 2) * 2.0` words (`src/distill.rs`); do not pad scripts past it.

## Commands

```sh
cargo build --release
./target/release/doomscrum generate --root . [--model <id>] [--spec <filter>]
```

Run long renders in the background with the absolute binary path and
`--root` pinned — cwd drift breaks relative paths. `rm` is hook-blocked;
use `/usr/bin/trash`.

## Render gotchas (earned, not theoretical)

- **Exhausted fal balance masquerades as everything else** (2026-06-11): a
  locked account produced storage 403s, LTX 422s at result-fetch, and
  600-poll queue timeouts across models before any request said "Exhausted
  balance". When two unrelated failure shapes appear in one batch, probe
  with a cheap submit and read the 403 body before debugging models.
  Our spend cap tracks OUR ledger, not the prepaid fal balance.
- **ltx-2.3 is banned from the content mix** (verified 2026-06-11): 1 of 4
  clips had catastrophic diction ("no vibes merges, capiche" -> "No merges.
  Copy, sheep."), 1 cut its last word. veo3.1/lite carries the cheap weight.
- **Transcription fallback**: fal storage upload may 403 independently of
  the queue; `check_script_fit.py` falls back to Deepgram through the exact
  Mint route derived from `MINT_BASE_URL`, using
  `__mint.deepgram.default__` — direct binary upload, no storage hop and no
  raw Deepgram key in the agent process.
- Scripts must spell numbers as words and skip exotic interjections
  ("capisce") — voice models garble them and the transcript gate eats the
  miss. The scriptwriter prompt enforces this; don't hand-write scripts
  that violate it.

- Seedance output moderation rejects "unhinged" / "deadly serious" phrasing.
- Accented narrators garble words unless the prompt demands "crisp, clearly
  intelligible English".
- fal rejects data: URIs — upload via
  `https://rest.alpha.fal.ai/storage/upload/initiate` + PUT.
- Stale storyboards for archived specs masquerade as budget bugs — trash
  `.doomscrum/storyboards/` before a full regen.
