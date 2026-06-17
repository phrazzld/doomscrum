# Tinder-style swipe quotas and paid tiers

Priority: P3 · Status: blocked · Estimate: M

## Goal
Swipes are the scarce unit: free users get N dispatching swipes per day;
paid tiers buy more swipes and a richer hero-render weight in the mix.

## Oracle
- [ ] Config defines a daily dispatch-swipe quota; exceeding it queues the
      swipe with a visible "out of swipes — resets at midnight / upgrade"
      state instead of dispatching.
- [ ] Quota state survives restarts (events ledger) and resets daily.
- [ ] Tier config maps tier -> (daily swipes, render mix) so the free tier
      rides cheap pipelines and paid tiers carry hero weight.

## Notes
Commercial half of docs/EFFICIENCY.md (strategy 5). Wallet protection is
real and stays; this is product-shaped rationing on top, not agent
"bounds" (dispatch behavior itself remains unbounded by design once a
swipe is spent). BYOK fal key bypasses render quotas but not swipe quotas.
**Why:** owner's tinder-model framing, 2026-06-10 efficiency session.

Groom 2026-06-17: **demoted P2 → P3, blocked-on-adoption.** Rationing the wedge
before strangers can reach the free path (017 distribution / 019 onboarding
unshipped) monetizes a funnel with no top (product-value + premise lanes). This
is sequencing, not a bounds objection — the ticket's own "rationing, not bounds"
framing stands. Unblock after Gate 2 proves strangers reach value. See
`docs/VISION.md`.
