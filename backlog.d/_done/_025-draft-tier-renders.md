# Draft-tier renders for cheap prompt iteration before hero spend

Priority: P2 · Status: pending · Estimate: S

## Goal
Iterate on prompts at pennies (pixverse v6 360p silent ≈ $0.025/s) and only
pay hero rates once a prompt is locked.

## Oracle
- [ ] `generate --draft` renders the feed on the draft model/tier and marks
      renders as drafts (excluded from the default feed).
- [ ] Draft spend appears in the wallet ledger like any other render.

## Notes
Research 2026-06-10: industry-standard fal pattern is draft-on-cheap,
final-on-premium; a 20-revision prompt cycle costs ~$3 on pixverse vs ~$40
on premium models. Our observed seedance re-roll rate (~1 in 3) is exactly
the retry tier this is designed to absorb. **Why:** cost lane from the
2026-06-10 provider research.

## Notes (groom 2026-06-10)
Demoted pending deletion ratification: superseded by render profiles
(`--profile dev` free iteration, `--profile content` paid mix) and the
render mix itself; the remaining delta (drafts hidden from the feed) is
not worth a ticket.
