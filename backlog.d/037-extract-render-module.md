# Extract a `render` module (pipeline + wallet) from server.rs

Priority: P2 · Status: ready · Estimate: L

## Goal
The "spec → video" pipeline and the wallet/budget gate live behind one deep
module with a small interface, instead of being reimplemented across `server.rs`
and `main.rs`.

## Oracle
- [ ] `render::pipeline::render_spec(ctx, provider, prd) -> VideoRender` is the
      single owner of the spec→storyboard→render flow; `main.rs` generate and
      `api_generate`/`spawn_render_job` both call it (the duplicated `render_one`
      at main.rs:176-201 is gone — exactly one `scriptwriter::storyboard` call
      site outside tests).
- [ ] One budget gate (lifetime + daily + pending) consumed by `api_generate`,
      `maybe_prefetch`, and `main.rs`; spend-cap arithmetic appears once, and the
      existing `render_plan_degrades_*` and cap tests pass unchanged.
- [ ] `RenderReservation` + reservation lifecycle move into the wallet; `AppCtx`
      holds an opaque handle; `server.rs` drops below ~900 lines with no behavior
      test edited.

## Verification System
- Claim: render orchestration + wallet are one module; `server.rs` is routing + JSON shaping.
- Falsifier: a behavior test had to change; spend arithmetic still appears in
  more than one place; or a second `render_one` survives.
- Driver: `cargo test` (unchanged suite); `grep` for `scriptwriter::storyboard`
  and spend-cap arithmetic call sites.
- Grader: green suite with zero test edits; a single call site for each.
- Evidence packet: before/after `wc -l` + the grep results.
- Cadence: this refactor; afterward every render/spend change rides through it.

## Notes
From the groom architecture lane (2026-06-17). `server.rs` (~1294 lines) mixes
the axum router with a render-orchestration subsystem (budget math 265-328,
RenderPlan 376-410, spawn/prefetch/render_one 416-517 & 857-890,
`RenderReservation` bolted onto `AppCtx`). The highest-risk duplication is the
**wallet gate**: spend-vs-cap is computed three times (api_generate 721-801,
render_plan 453-510, main.rs 150-167 — the last lacking the pending term), and
this product's whole safety story is that arithmetic. `distill.rs` (1151) and
`providers/` are NOT targets — already deep modules with small interfaces. Pure
refactor: no behavior change, the gate is an unedited test suite. Mirrors the
`f37c176` caption discipline (provider-neutral artifacts in `providers/`).
