# First-dispatch consent gate (one-time, per repo)

Priority: P1 · Status: ready · Estimate: S

## Goal
A user cannot accidentally dispatch a real agent and open a real PR; the
consequence is acknowledged once per repo before the first implement/shape
swipe — without nagging on every swipe.

## Oracle
- [ ] The first `implement` or `shape` swipe against a given repo opens a
      one-time modal naming the consequence ("launches a real agent and opens a
      real PR against `<repo>`") with explicit **[Cook it] / [Cancel]**; Cancel
      dispatches nothing.
- [ ] Acknowledgment is persisted (localStorage, keyed by repo) so it does not
      re-prompt on reload or on subsequent swipes; switching to a new repo
      re-prompts once.
- [ ] The existing paid-render confirmation (`confirmRealRenderCost`) is
      unchanged and independent — this gate is about dispatch, not spend.

## Notes
From the groom operator-UX lane (2026-06-17), vetted live: the only
`window.confirm` in the UI gates paid renders (index.html:391/413); a right
swipe (index.html:601; arrow key :761) fires `/api/swipe` immediately with no
dispatch confirmation, and the consequence text is only a dismissible splash
line framed as a sound-unlock (index.html:343-344). This was Oracle line 11 of
[[019-onboarding-first-run]]; pulled out as its own small ticket so a P1
onboarding build can't gate a live safety hole. This is dispatch **consent**
(one-time), not a per-swipe quota or bound. Pairs with
[[035-triage-grade-receipts-and-undo]]'s undo: consent covers intent, undo
covers fat-fingers.
