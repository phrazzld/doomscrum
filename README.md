# Specifi

Backlog triage as a TikTok feed. Specifi reads markdown specs from a repo's
backlog, turns each one into a goofy shortform video, and lets you swipe:

| Gesture | Action |
|---|---|
| **swipe →** | dispatch a coding agent in a fresh git worktree to **implement** the spec and open a PR |
| **swipe ←** | dispatch an agent to **shape** the spec — sharpen it, add acceptance criteria and context — and open a PR |
| **swipe ↑** | skip to the next spec (recorded, durable) |
| **swipe ↓** | go back to the previous spec |
| **tap** | read the exact source spec (path + sha256) |

Arrow keys mirror the gestures; `space` pauses; `enter` opens the spec.

Right and left swipes launch **real agents** that modify code and open
**real pull requests**. That is the point. There is no sandbox theater
beyond what your agent CLI provides.

## Quick start

```bash
cargo build --release

# 1. Render videos for the top specs (offline fixture provider)
cargo run --release -- generate

# 2. Serve the feed
cargo run --release -- serve        # http://127.0.0.1:4173
```

Tap the splash screen (sound gate), then swipe.

### Real AI video

Put a FAL key in your environment or `~/.secrets` (`FAL_API_KEY=...`), then:

```bash
cargo run --release -- generate --provider fal --limit 1
```

or use **cook with AI** in the app. Real generation costs money per render
and sends spec-derived prompt text to fal.ai — treat it as a disclosure
event. The fixture provider (`fake`) is the default and never leaves the
machine.

## Syncing to a repo

Point `specifi.toml` at any repository:

```toml
[repo]
path = "../some-other-repo"
backlog_dir = "backlog.d"
```

Each `*.md` file in the backlog directory is one spec. Priority is filename
sort order; only the top `feed.max_items` (default 10) enter the feed.
Files starting with `_` are ignored (use `_done/`-style prefixes for
archives).

## What a swipe actually does

Right swipe (implement) / left swipe (shape):

1. `git worktree add .specifi/worktrees/<branch> -b specifi/<impl|shape>-<slug>-<id>`
2. Runs your configured agent command in the worktree with the full spec as
   its mission (implement it, or improve the spec file in place).
3. Commits anything the agent left uncommitted.
4. Pushes the branch and opens a PR with `gh` (when the repo has an
   `origin` remote and `open_pr = true`; otherwise the branch stays local
   and the receipt says so).

Every dispatch writes a staged receipt to `.specifi/dispatches/<id>.json`
(status: `queued → agent_running → opening_pr → pr_opened | completed_local
| failed`) plus a full agent log. The feed shows live status stickers and
links to opened PRs. The agent command is yours to choose in
`specifi.toml` — codex by default; point it at claude, or anything else
that can take a prompt and edit a worktree.

## Provenance

The source spec stays authoritative. Every render records provider, model,
spec sha256, storyboard hash, latency, and job id in
`.specifi/renders/<spec-sha>/<render-id>.json`. Every decision (skip,
dispatch) is appended to `.specifi/events.ndjson`. Deleting `.specifi/`
destroys only generated state — never specs.

## Development

```bash
cargo test           # unit + end-to-end HTTP tests (stub agents, real git remotes)
cargo clippy --all-targets
cargo fmt --check
```

The end-to-end tests exercise the actual HTTP routes: a right swipe in the
test suite creates a real worktree, runs a stub agent, pushes to a real
bare remote, and asserts the PR command ran. The FAL provider is tested
against a mock queue API; one fixture render is embedded so tests run
offline with zero external dependencies.

### Layout

```
src/
  backlog.rs       spec scanning, hashing, priority cap
  distill.rs       markdown → brief → storyboard (the brainrot script)
  providers/       fake (embedded fixture) and fal (real) video generation
  dispatch.rs      swipe → worktree → agent → commit → push → PR
  events.rs        durable NDJSON decision ledger
  server.rs        axum API + embedded UI
  main.rs          CLI: serve | generate | report
assets/index.html  the feed UI (single embedded file; the only non-Rust surface)
backlog.d/         this repo's own sample specs
docs/archive/      the original (superseded) MVP spec
```

The UI is one static HTML file embedded in the binary — browsers execute
JS, so the gesture/video shim is the single deliberate non-Rust exception.
All state, logic, and dispatch live in Rust.
