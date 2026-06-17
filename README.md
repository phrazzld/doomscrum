# DoomScrum

Backlog triage as a TikTok feed. DoomScrum reads markdown specs from a repo's
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

# Inspect generated-state cleanup without deleting anything
cargo run --release -- gc --dry-run
```

Tap the splash screen (sound gate), then swipe.

### Real AI video

Put a FAL key in your environment or `~/.secrets` (`FAL_API_KEY=...`), then:

```bash
cargo run --release -- generate --provider fal --limit 1
```

or use **cook with AI** in the app. Real generation costs money per render
and sends spec-derived prompt text to fal.ai — treat it as a disclosure
event. DoomScrum quotes the estimated batch cost before the UI starts a real
render, enforces both `max_total_spend_usd` and an independent
`max_daily_spend_usd`, and returns `429` with the next reset time when the
daily budget is exhausted. The fixture provider (`fake`) is the default and
never leaves the machine.

## Brainrot formats

Each spec is translated into one of five live brainrot formats, rotated by
feed position so consecutive videos never look alike:

1. **AI fruit drama** — anthropomorphic fruits in a telenovela kitchen
   confrontation; the betrayal *is* the spec goal.
2. **Gen-Z explainer** — unhinged ring-light talking head, punch-in zooms,
   word-by-word captions, "no cap."
3. **Cryptid vlog** — Bigfoot GoPro selfie vlog hyping the spec like a
   day-in-the-life.
4. **Italian brainrot** — surreal hybrid creature reveal with a bombastic
   pseudo-Italian opera narrator.
5. **2080 street interview** — fake future documentary asking an elderly
   gen-z developer if they remember the spec.

The spoken dialogue in every format quotes the spec's actual goal and first
acceptance criterion — the video must communicate the spec, not just vibe.
Prompts forbid inventing features or claiming anything shipped.

The offline fixture provider stays free and local. When `ffmpeg` is available
with the `drawtext` filter, it renders a short spec-specific MP4 with the spec
title and brainrot format over distinct format colors. Without that filter, it
falls back to the embedded fixture so tests and demos still work with no
runtime media dependency.

## Syncing to a repo

Point `doomscrum.toml` at any repository:

```toml
[repo]
path = "../some-other-repo"
backlog_dir = "backlog.d"
```

Each `*.md` file in the backlog directory is one spec. Priority is filename
sort order; only the top `feed.max_items` (default 10) enter the feed.
Files starting with `_` are ignored (use `_done/`-style prefixes for
archives).

Real renders happen just-in-time: serving the feed renders at most
`feed.prefetch_depth` (default 3) specs ahead of your viewport, so a long
backlog costs a handful of clips, not one per spec. Specs deeper in the feed
stay free until you scroll toward them, and if the wallet cap is exhausted a
spec degrades to a free fixture (badged "render budget exhausted") instead of
breaking the feed.

## What a swipe actually does

Right swipe (implement) / left swipe (shape):

1. `git worktree add .doomscrum/worktrees/<branch> -b doomscrum/<impl|shape>-<slug>-<id>`
2. Runs your configured agent command in the worktree with the full spec as
   its mission (implement it, or improve the spec file in place).
3. Commits anything the agent left uncommitted.
4. Pushes the branch and opens a PR with `gh` (when the repo has an
   `origin` remote and `open_pr = true`; otherwise the branch stays local
   and the receipt says so).

Every dispatch writes a staged receipt to `.doomscrum/dispatches/<id>.json`
(status: `queued → agent_running → opening_pr → pr_opened | completed_local
| failed`) plus a full agent log. The feed shows live status stickers and
links to opened PRs. The agent command is yours to choose in
`doomscrum.toml` — codex by default; point it at claude, or anything else
that can take a prompt and edit a worktree.

Agent work is throttled by `agent.max_concurrent_dispatches` (default `2`).
Swipes beyond the limit remain as visible `queued` receipts until a slot
opens, and swiping the same spec/action while a receipt is still active
returns that receipt instead of launching a duplicate agent.

## Provenance

The source spec stays authoritative. Every render records provider, model,
spec sha256, storyboard hash, latency, and job id in
`.doomscrum/renders/<spec-sha>/<render-id>.json`. Render IDs and media URLs
are cache-distinct for each successful generation, and the feed selects the
newest ready provenance for a spec while leaving older JSON readable for
audit. MP4s are served with HTTP byte ranges and streamed from disk, so
browser seek and loop requests do not buffer the full render in memory. Every
decision (skip, dispatch) and human vibe rating is appended to
`.doomscrum/events.ndjson`; ratings point at the render id they judge instead
of mutating render JSON or source specs. Deleting `.doomscrum/` destroys only
generated state — never specs.

`doomscrum gc` keeps generated state bounded. It preserves every render JSON
for audit, deletes only superseded MP4 assets (the latest ready render per
spec/provider survives), runs `git worktree prune`, removes terminal dispatch
worktrees past the age policy, and rotates `events.ndjson` by archiving the
full ledger before keeping recent complete event lines. Use `--dry-run` to
print the actions without touching source specs, open dispatches, renders, or
logs.

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
  gc.rs            generated-state lifecycle and dry-run reporting
  server.rs        axum API + embedded UI
  main.rs          CLI: serve | generate | script | report | gc
assets/index.html  the feed UI (single embedded file; the only non-Rust surface)
backlog.d/         this repo's own sample specs
docs/archive/      the original (superseded) MVP spec
```

The UI is one static HTML file embedded in the binary — browsers execute
JS, so the gesture/video shim is the single deliberate non-Rust exception.
All state, logic, and dispatch live in Rust.
