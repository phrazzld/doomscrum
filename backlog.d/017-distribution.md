# Distribution: installable releases

Priority: P1 · Status: ready · Estimate: M

## Goal
A stranger can install and run this in under two minutes without a Rust toolchain.

## Oracle
- [ ] CI builds signed/notarized macOS arm64 binary (+ linux x86_64) on tag push; GitHub Release with checksums.
- [ ] `brew install <tap>/<name>` works end-to-end on a clean machine.
- [ ] README quickstart updated to the binary path (cargo path remains for devs).
