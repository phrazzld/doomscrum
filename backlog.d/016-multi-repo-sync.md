# Plug DoomScrum into arbitrary repos (epic: picker, contract, MCP source)

Priority: P1 · Status: ready · Estimate: L

## Goal
An operator points DoomScrum at any repo satisfying the backlog contract —
pick a repo, sync, watch its specs become feed clips, swipe to dispatch
agents against *that* repo.

## Oracle
- [ ] The backlog contract is documented (docs/): a directory of `.md` specs,
      non-`_`/non-`.` prefixed, title = first heading; nothing DoomScrum-
      specific required of the target repo.
- [x] From the UI (not hand-edited TOML), a user selects a local repo path,
      and the feed switches to that repo's backlog without restart.
- [ ] A right swipe against a foreign repo creates the worktree/branch from
      that repo and opens the PR on that repo's remote (verified live against
      ~/Development/adminifi/olympus).
- [x] Per-repo state dirs: renders/events/dispatches for repo A never bleed
      into repo B; spend tracked per repo.

## Children
1. [done 2026-06-11] Repo picker + sync UX (covers the demo-day flow: select olympus, sync,
   feed populates).
2. [done 2026-06-11, test-level] Foreign-repo dispatch verification: worktree, branch, push, PR remote
   all derive from `repo.path`, with a two-repo test.
3. Backlog contract doc + graceful degradation for specs that lack
   Goal/Oracle structure (title + body is enough to distill).
4. habitat (Apollo) MCP backlog source: read specs from the habitat
   work app's MCP (`mcp__apollo__query_prds` et al.) instead of a local
   directory — second implementation of the same `PrdSource` contract.

## Notes
Verified 2026-06-10: `doomscrum --root <dir>` with `repo.path` pointed at
olympus rendered and served its 10 specs with zero code changes ($0, fake
provider). The gap is UX (picker/sync), foreign-repo *dispatch* (untested),
and the MCP source. Supersedes the old "sync multiple repos" framing —
simultaneous multi-feed is child 1's follow-on, not the demo blocker.
**Why:** owner is demoing soon against adminifi repos (olympus/habitat).
