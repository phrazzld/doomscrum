# Distribution: installable releases

Priority: P1 · Status: ready · Estimate: M

## Goal
A stranger can install and run this in under two minutes without a Rust toolchain.

## Oracle
- [ ] CI builds signed/notarized macOS arm64 binary (+ linux x86_64) on tag push; GitHub Release with checksums.
- [ ] `brew install <tap>/<name>` works end-to-end on a clean machine.
- [ ] README quickstart updated to the binary path (cargo path remains for devs).

## Notes
Groom 2026-06-17: sequenced **Gate 2** in `docs/VISION.md` (after Gate 0
trustworthy-dispatch + Gate 1 render-quality/onboarding). **Open decision:**
`docs/COMMERCIAL_MODEL.md` calls distribution "the first reversible step," but
the groom evidence argues you shouldn't distribute a dispatch loop that is
unproven-live and leaks secrets — so it waits behind the proof
([[016-multi-repo-sync]] L3). Priority left at P1 (not demoted) pending your
call on that tension.
