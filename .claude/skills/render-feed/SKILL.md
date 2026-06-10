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
  `.doomscrum/renders/*/render.json`; their sum is total spend.
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
python3 scripts/check_script_fit.py <mp4> --expect "<planned script>"
```

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

- Seedance output moderation rejects "unhinged" / "deadly serious" phrasing.
- Accented narrators garble words unless the prompt demands "crisp, clearly
  intelligible English".
- fal rejects data: URIs — upload via
  `https://rest.alpha.fal.ai/storage/upload/initiate` + PUT.
- Stale storyboards for archived specs masquerade as budget bugs — trash
  `.doomscrum/storyboards/` before a full regen.
