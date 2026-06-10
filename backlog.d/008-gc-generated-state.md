# Garbage-collect generated state: renders, worktrees, events

Priority: P1 · Status: ready · Estimate: M

## Goal
.doomscrum never grows unbounded: superseded renders, finished worktrees, and stale logs have a lifecycle.

## Oracle
- [ ] `doomscrum gc` removes superseded renders (keep latest per provider per spec; provenance JSON preserved for audit), prunes merged/stale worktrees via `git worktree prune` + age policy, rotates events.ndjson past a size threshold.
- [ ] Dry-run mode prints what would be deleted.
- [ ] gc never touches source specs or open-dispatch state.

## Notes
Today: every --force re-render orphans an MP4 (~3-15MB each); worktrees accumulate forever.
