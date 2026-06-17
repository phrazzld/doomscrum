# Render the viewport, not the backlog

Priority: P1 · Status: ready · Estimate: M

## Goal
Real renders happen just-in-time for specs entering the top of the feed —
a 200-ticket backlog costs a handful of renders, not 200.

## Oracle
- [ ] Serving the feed with N unrendered top specs triggers at most
      `prefetch_depth` (config, default 3) background renders; specs deeper
      in the feed cost $0 until they approach the viewport.
- [ ] A spec watched, skipped, and revisited replays its cached render —
      no second spend (content-hash keyed, as today).
- [ ] Wallet gate still applies to JIT renders; over-cap requests degrade
      to the fake provider with a visible "render budget exhausted" badge
      instead of failing the feed.

## Notes
The unit-count half of docs/EFFICIENCY.md (strategy 1). Pairs with the
render mix (shipped): JIT picks each spec's pipeline from `[[video.mix]]`.
Follow-up idea (not in scope): engagement-driven promotion — re-render a
spec on a hero pipeline once users demonstrably linger on it.
**Why:** cost-efficiency directive 2026-06-10.
