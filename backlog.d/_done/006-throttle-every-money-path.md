# Throttle and budget every path that spends money or compute

Priority: P0 · Status: ready · Estimate: M

## Goal
No user action — swipe, tap, retry, or bug — can drain a wallet or fork-bomb the machine.

## Oracle
- [ ] Concurrent agent dispatches capped (config `max_concurrent_dispatches`, default 2); excess swipes queue with visible status.
- [ ] Duplicate dispatch for a spec with an active run returns the existing receipt instead of spawning again.
- [ ] Per-day render budget independent of lifetime cap; exceeding returns 429 with reset time.
- [ ] UI confirms estimated cost before any real render batch.
- [ ] Integration tests cover all four through HTTP routes.

## Notes
Lifetime cap (`max_total_spend_usd`) shipped 2026-06-09; this adds rate + concurrency + dedupe + confirm. The "protect our bank account" P0.
