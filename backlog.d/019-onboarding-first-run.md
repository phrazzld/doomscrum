# First-run onboarding: repo, key, first video in 60s

Priority: P1 · Status: ready · Estimate: M

## Goal
First launch walks a new user from zero to a playing (fixture) video and explains exactly what swipes do and what costs money.

## Oracle
- [ ] First run with no config: feed offers pick-repo + sample specs; fixture videos render without any key.
- [ ] FAL key entry surface explains per-render price + sets a starter budget; never required for the free path.
- [ ] (Split to [[034-first-dispatch-consent-gate]] — the safety-critical one-time dispatch consent.) This ticket owns the zero-to-fixture-video on-ramp; 034 owns the consent gate, so a P1 onboarding build can't gate a live safety hole.

## Notes
Groom 2026-06-17: this is **Gate 1** (the on-ramp), sequenced after Gate 0
(trustworthy dispatch) and the render-quality gate ([[031-render-verdict-gate]]).
The operator-UX lane confirmed no first-run path exists today: a fresh `serve`
with no renders lands on the per-spec "cook fixture" empty card, and the README
quick start requires a manual `generate` before `serve`. Empty-backlog and
switch-repo dead-ends (no recovery button) belong here too. See `docs/VISION.md`.
