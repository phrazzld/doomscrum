# The backlog contract

DoomScrum reads "specs" from a target repo and turns each one into a feed
card: a shortform video plus a swipe that dispatches a real coding agent. A
repo satisfies the backlog contract if DoomScrum can scan it, distill a clip
from each spec, and dispatch a worktree/branch/PR — **without anything
DoomScrum-specific living in the target repo.**

This is the contract a backlog source must honor. The default source is a
markdown directory; a GitHub-Issues source is the same contract over a
different transport. The `PrdSource` shape in `src/backlog.rs` is the
source-neutral record every source produces.

## The markdown directory source (default)

Point `[repo]` at any repo and `backlog_dir` at a directory inside it:

```toml
[repo]
path = "../some-other-repo"
backlog_dir = "backlog.d"
```

The directory may be dot-named (e.g. `.backlog.d`) — the chrondle repo uses
exactly that. What gets filtered is the **file** prefix, not the directory
name.

### File rules

- One `*.md` file per spec.
- Files prefixed with `_` **or** `.` are ignored — use `_done/`-style or
  dotfile sidecars to archive without polluting the feed. (Code:
  `src/backlog.rs` `scan`.)
- Priority is filename sort order; only the top `feed.max_items` (default
  10) enter the feed. Number your specs (`001-…`, `002-…`) to make priority
  legible, but any sortable name works.
- A missing/empty backlog directory is an **empty feed**, never an error —
  a repo with no specs degrades to a blank feed rather than crashing.

### Title rule

- The spec title is the first `# ` heading.
- If no `# ` heading is present, the title falls back to the filename
  (`001-foo-bar.md` → `"foo bar"`). Title + body is enough; no heading
  required.

## What DoomScrum reads out of a spec (and what it ignores)

DoomScrum's distiller (`src/distill.rs`) is **opt-in by section and
otherwise ignores everything.** This is the graceful-degradation contract:
a spec does not have to be in DoomScrum's own Goal/Oracle house format.

| Section (`## …`) | Used as | If missing |
|---|---|---|
| `## Goal` | the spoken goal line | falls back to the spec title |
| `## User` | the audience | falls back to `"Local operator"` |
| `## Problem` | context | empty |
| `## Acceptance Criteria` | checkboxes → acceptance lines | falls through to `## Oracle` |
| `## Oracle` | checkboxes → acceptance lines | empty → flagged "No acceptance criteria found." |
| `## Risk` | risk notes | empty |

Any other section a repo's house format uses — `Priority`, `Status`,
`Non-Goals`, `Children`, `Notes`, front matter, you name it — is simply
ignored. The distiller never requires a section DoomScrum invented.

### Title + body is enough

A spec with only a heading and free-form prose still distills and compiles a
storyboard: the goal degrades to the title, the user to the default, and the
missing acceptance contract is flagged rather than fatal. A right swipe
still dispatches against it. This is why the chrondle repo's
`.backlog.d/` specs — which use a `Priority` / `Status` / `Non-Goals` /
`Oracle` house format, not DoomScrum's `Goal` / `Oracle` format — render and
dispatch with zero spec edits. (Proven live 2026-07-07; see
`docs/adoption/2026-07-07-gate0-external-dispatch/`.)

The lock-in regression tests:
`distill_title_and_body_only_spec_without_goal_or_oracle` and
`distill_foreign_house_format_recovers_oracle_and_degrades_goal` in
`src/distill.rs`, plus `skips_underscore_and_dot_prefixed_files_but_allows_dot_dir`
in `src/backlog.rs`.

## The GitHub-Issues source (contract for a second transport)

The original MVP called out plugging in Jira, Linear, GitHub Issues, or a
database "without changing the feed/action semantics." The semantics that
must not change are the `PrdSource` contract — the same record the markdown
directory source produces:

| `PrdSource` field | Markdown directory | GitHub-Issues source (contract) |
|---|---|---|
| `id` / `sha256` | sha256 of the raw file bytes | sha256 of the issue body (stable id for renders + decisions) |
| `rel_path` | repo-relative file path | a stable key, e.g. `issues/<number>` |
| `title` | first `# ` heading (or filename) | the issue title |
| `priority` | filename sort order | the source's own ordering (labels, number, created_at, …) |
| `raw` | the full markdown file | the issue body as markdown |

A second source is a **read adapter**: it maps its native records onto
`PrdSource` and stops there. Everything downstream — distillation,
storyboarding, rendering, feed, swipe, dispatch, provenance — is
source-agnostic and consumes `PrdSource` only. Implementing a new source
must not change `distill`, `dispatch`, `render`, or the feed.

### What is **not** required of the target repo

- No DoomScrum section names. `## Goal` / `## Oracle` help, but their
  absence degrades gracefully.
- No DoomScrum tooling, config, or dependencies in the target repo.
- No specific filename convention beyond "markdown files in a directory."
- No front matter or metadata DoomScrum can read — it is all ignored.

### What a target repo **does** need for a swipe to land

Dispatch (a right swipe) is the part that touches the target repo, and it
needs what any agent dispatch needs — none of it spec-format-related:

- `[repo].path` is a git repo with a pushable `origin` (else dispatch
  degrades to `completed_local` and the branch stays local).
- Agent + PR credentials (`opencode auth login`, `gh auth login`) — verified
  by `doomscrum doctor`.
- The first dispatch against a repo raises the one-time consent gate.

See [OPERATIONS.md](OPERATIONS.md) ("Syncing to a repo", "What a swipe
actually does") and [CONFIGURATION.md](CONFIGURATION.md) (`[repo]` table)
for the runtime details.

## Why the contract is this loose

DoomScrum's pitch is "point it at any repo and your specs become feed
clips." If a repo had to adopt DoomScrum's house format to participate,
the demo-day promise ("sync olympus, watch its specs, swipe to dispatch")
would be false. The contract is deliberately the floor below which a spec
can't yield a feed card at all: a title (or filename) and some text. Above
that floor, richer specs just produce richer scripts — the `## Goal` /
`## Oracle` structure DoomScrum's own `backlog.d/` uses is the recommended
shape, not a precondition.
