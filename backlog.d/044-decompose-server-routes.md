# Decompose server.rs HTTP handlers into route submodules

Priority: P3 · Status: pending · Estimate: M

## Goal
`server.rs` is the axum transport layer and nothing else — the HTTP handlers
live in cohesive route submodules instead of one ~1250-line file.

## Oracle
- [ ] `server.rs` drops below ~900 lines: the dispatch routes (`api_swipe`,
      `api_dispatches`, `api_dispatch_log`, `api_dispatch_cancel`), the feed/render
      routes (`api_state`, `api_generate`, `api_vibe`, the prefetch orchestration),
      and media streaming each live in a `server/` submodule.
- [ ] `AppCtx` stays the shared state; no route reaches into another route's
      internals. No behavior test edited.

## Notes
Surfaced finishing [[037-extract-render-module]] (2026-06-25). 037 extracted the
render *pipeline* + *wallet* (one `render_spec`, one `cap_breach`) and cut
server.rs 1383→1254, but the `<900` target wasn't reached: the remaining bulk is
genuine HTTP handlers plus the `maybe_prefetch`/`spawn_render_job` orchestration,
which manipulate `AppCtx`'s private `cooking`/`wallet` state — moving them out
would leak that state. This is a transport-layer decomposition (a different
concern from render extraction), so it's its own ticket. Lower priority: the
high-value duplication 037 targeted is already gone; this is tidiness.
