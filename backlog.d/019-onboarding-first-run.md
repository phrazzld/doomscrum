# First-run onboarding: repo, key, first video in 60s

Priority: P1 · Status: absorbed → Powder card **doomscrum-942** · Estimate: M

> **Absorbed 2026-07-07 by doomscrum-942** (first-run inside the app: every
> empty state becomes an on-ramp). The "no first-run path exists today" note
> below is stale: the demo cartridge (PR #12) bootstraps 3 sample brainrot
> videos on first `serve` with zero keys (`providers/samples.rs::bootstrap`).
> Remaining scope (in-app key sheet, empty/error-state on-ramps, gesture
> coach, consequence-framed consent) shipped under doomscrum-942.

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
switch-repo dead-ends (no recovery button) belong here too. See `VISION.md`.
