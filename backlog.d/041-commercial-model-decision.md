# Re-ratify or formally defer the reopened commercial model

Priority: P1 · Status: ready · Estimate: S (decision, not code)

## Goal
Convert `docs/COMMERCIAL_MODEL.md`'s reopened "UNDER REVIEW" state back into a
dated owner decision — re-ratify the staged path or formally defer with a named
trigger — so the backlog can sequence against it again.

## Oracle
- [ ] `docs/COMMERCIAL_MODEL.md` status line reads `RATIFIED <date>` or
      `DEFERRED until <named trigger>` — not "UNDER REVIEW".
- [ ] The chosen path names which Gate the cloud-render-credits step (step 2)
      waits behind, consistent with `VISION.md`'s open bet and its one hard
      constraint ("trivial to start, no rationing before strangers reach the free
      wedge").
- [ ] `VISION.md`'s "Open bets → how it's distributed and sold" paragraph is
      reconciled with the verdict (still an open bet, or resolved).

## Notes
**Why:** the commercial model was reopened this session (2026-06-24); the
four-way options analysis already exists (`COMMERCIAL_MODEL.md` table: local-first
BYO-keys / cloud credits / hosted SaaS / staged, compared on wallet-risk, infra,
privacy, pricing, onboarding). The gap is a *verdict*, not more analysis — this
ticket forces the decision, it does not re-open the study.

**What it unblocks:** [[029-swipe-quota-tiers]] (blocked, P3) and
[[021-cloud-architecture-spike]] (blocked, P3, "blocked-on: 020") both reference
this decision; neither can start until it closes. Owner-only call — do not pick
the model in an agent lane.
