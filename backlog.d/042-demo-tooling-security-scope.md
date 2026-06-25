# Patch the demo `ws` DoS and fence `demo/` out of the product security boundary

Priority: P3 · Status: ready · Estimate: S

## Goal
The one open Dependabot HIGH (`ws` memory-exhaustion DoS) is patched, and
`demo/` is documented as dev-tooling outside the product security boundary so
future scanner hits on Remotion deps are triaged correctly, not as product holes.

## Oracle
- [ ] `demo/package-lock.json` resolves `ws` ≥ 8.21.0 (the patched range);
      Dependabot alert #3 closes.
- [ ] A one-line scope statement in `demo/`'s README (or AGENTS) marks `demo/`
      as non-product dev tooling, excluded from the product's security/egress
      boundary — matching `CLAUDE.md`'s existing "demo/ is dev tooling, not
      product" framing.

## Notes
**Why:** groom security lane (2026-06-25) verified the "1 high vulnerability on
master" is `ws@8.20.1` in `demo/package-lock.json` — the Remotion *demo* tooling,
not the Rust product crate. No Node in the dispatch/render path. Real but
low-stakes; the bump is a one-line free fix and the scope note stops the alert
from recurring as a phantom product finding. (A stale copy under
`.doomscrum/worktrees/...` is gitignored dispatch debris — not in scope; see
[[038-purge-orphaned-artifacts]].)
