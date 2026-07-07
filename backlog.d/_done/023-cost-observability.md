# Cost & operational observability: spend ledger, economics, failures, logs

Priority: P2 · Status: pending · Estimate: M

## Goal
Spend survives state wipes and answers "what did this spec cost me" and "what's my burn this week".

## Oracle
- [ ] Durable append-only cost ledger (separate from renders dir) written on every real render.
- [ ] `doomscrum report` shows per-spec, per-day, and total spend from the ledger.
- [ ] Wallet gate reads the ledger, not just surviving render JSONs.
- [ ] Spend rolls up per-model and per-spec (the data already exists on each
      render: `model` + `cost_estimate_usd`), so model bake-offs have an
      economics readout.
- [ ] `doomscrum report` prints daily-vs-cap with % consumed and reset time, a
      render/dispatch **status breakdown** (ready/cooking/failed; queued/running/
      opened/failed) naming recent failures, and queue depth vs free slots —
      answering "spent today, cooking now, what broke" from one command.
- [ ] Render/dispatch failures append a durable event (events.rs already has a
      `kind` slot) so they survive restart instead of dying with the in-memory
      `cooking` map.
- [ ] **Child (separate slice):** `serve`/dispatch/render emit structured
      `tracing` logs with levels — today observability is bare `println!` in
      main.rs and a long-running server is a black box.

## Notes
Today spend is summed from render provenance — wiping .doomscrum/renders resets the meter while the money stays spent.

Groom 2026-06-17 (observability + ops lanes): the keystone is the durable ledger;
everything else hangs off it. Vetting correction — current `gc` removes only the
MP4 *asset* (gc.rs:165) and **preserves** every render JSON, so `gc` does NOT
cause re-spend; the risk is a manual `.doomscrum/` wipe or a future pruning
policy, exactly as this ticket's original note says. The richer numbers
(`daily_usd`, `pending_usd`, `daily_reset_at`, `price_per_render_usd`) already
exist but only in `/api/state` JSON (server.rs:587-596) — surface them in
`report`.
