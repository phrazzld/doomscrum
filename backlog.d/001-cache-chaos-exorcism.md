# Cache Chaos Exorcism

## User
Operators reviewing agent-delivered web app changes in the local Specifi feed,
especially after running a fake or fal provider smoke to refresh the MP4 tied to
a backlog spec.

## Problem
The feed can keep showing stale render provenance after generation succeeds.
Renders are generated state under `.specifi/renders/<spec-sha>/`, and the
source `backlog.d/*.md` file remains authoritative. Today the server reloads
render JSON through `load_renders`, selects one render in `latest_render`, and
returns it from `/api/state`; the browser then avoids rebuilding the card unless
the selected render id changes. That means a successful provider smoke can be
hidden if the newly generated MP4 reuses a render id/asset URL, is overwritten
in place, or loses selection to an older ready render.

Repo context for the implementer:
- `src/providers/mod.rs` defines `VideoRender`, `save_render`, and
  `load_renders`; `load_renders` currently sorts render JSON newest-first by
  `created_at`.
- `src/server.rs` owns `/api/state`, `/api/generate`, and `latest_render`.
  `latest_render` currently filters to ready renders for a PRD and prefers any
  non-`fake-local` render before falling back to the first ready render.
- `assets/index.html` calls `/api/state`, posts to `/api/generate`, and uses
  `item.render.id` in the DOM rebuild signature, with a 4s poll after playback
  starts.
- `README.md` documents the provenance contract:
  `.specifi/renders/<spec-sha>/<render-id>.json` records provider, model, spec
  sha256, storyboard hash, latency, and job id, while deleting `.specifi/`
  destroys only generated state and never source specs.

## Goal
Make each successful render generation produce cache-distinct provenance and
media selection so `/api/state` and the gallery show the newest successful MP4
for that PRD after refresh, without mutating the source PRD or deleting older
render JSON.

## Acceptance Criteria
- A route-level test proves that when two ready `VideoRender` JSON files exist
  for the same PRD, `/api/state` selects the newest successful render by
  provenance timestamp or equivalent monotonic freshness field, not an older
  stale render.
- A generation test proves that a second successful generation for the same PRD
  writes a distinct render JSON file and a cache-distinct `asset_url` or render
  id, instead of overwriting the earlier JSON in place. The earlier JSON remains
  readable from `.specifi/renders/<spec-sha>/` for audit.
- A gallery-facing test or documented browser smoke proves that after
  `/api/generate` returns and `load()` refreshes `/api/state`, the rendered card
  uses the new render id or media URL in its DOM signature so the MP4 element is
  rebuilt for the latest provenance.
- Existing no-cache behavior for `/media/{sha}/{file}` remains intact, but the
  fix does not rely on browser cache headers alone; the selected render metadata
  must change when a new render is produced.
- The source backlog file for the PRD is unchanged by generation. Verify by
  comparing the PRD sha256 before and after the render path, or by an equivalent
  route/CLI test assertion.
- The implementation keeps the normal offline path deterministic enough for CI:
  fake-provider tests still run without network or credentials, and fal-specific
  behavior stays behind existing mocked-provider tests or explicit credential
  smoke.
- Run and pass the relevant repo gate for this slice: at minimum `cargo test`.
  If browser smoke is manual instead of automated, record the exact local route
  and observed render id or URL before and after refresh.

## Risk
Selecting "latest" blindly could hide provider failure or regress the useful
"real provider beats fixture" behavior if the operator intentionally keeps an
older real render while testing fake output. The recommended default is:
newest ready render wins after successful generation, and failed generation
does not replace the selected render or produce a fake pass.

Ambiguities to resolve before implementation:
- Should a forced fake-provider rerender supersede an older fal render in the
  gallery? Recommended answer: yes, if it is the newest successful render,
  because the operator explicitly generated it; provider/model remain visible
  in the metadata.
- Should CLI `cargo run -- generate --force` and UI `/api/generate` share the
  same cache-busting behavior? Recommended answer: yes, put the invariant near
  provider save/selection so both entry points benefit.
- What is the authoritative freshness key if two JSON files have the same
  `created_at` string? Recommended answer: define a deterministic tie-breaker
  such as render id or file modified time and cover it in the selection test.
- How should a failed fal smoke be surfaced while an old ready render exists?
  Recommended answer: keep returning the old ready render from `/api/state`,
  return the existing error from `/api/generate`, and ensure the UI toast makes
  failure visible rather than implying the feed was refreshed.
