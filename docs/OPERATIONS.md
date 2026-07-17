# Operations reference

Deep-dive reference for running DoomScrum day to day: what leaves the
machine, how generated state is tracked and reclaimed, what a dispatch
receipt looks like, and how to recover a stuck one. The [README](../README.md)
covers the pitch, gestures, and quickstart; this page is what you need once
you're actually running it against a real backlog.

## Render profiles (`dev` vs `content`)

`doomscrum.toml`'s top-level `profile` key (default `"dev"`) selects a
partial `[video]` override from `[profiles.*]`:

| Profile | `[video]` override | Cost |
|---|---|---|
| `dev` (default) | `provider = "fake"`, `mix = []` | free — deterministic local spec previews, the everyday default for working on DoomScrum itself |
| `content` | `provider = "fal"` | real money — the weighted render portfolio (`[[video.mix]]`), roughly $0.43/clip average at the built-in mix |

Unset fields in a profile keep the base `[video]` values, so `content` still
inherits `script.mode`, spend caps, etc. from the top-level config. Switch
profiles either by editing the `profile` key in `doomscrum.toml`, or per-run
with the global `--profile` flag (any subcommand):

```bash
cargo run --release -- --profile content generate --limit 1
```

`dev` is the safety default: a fresh checkout with no config edits never
spends money. Only `content` (or an explicit `--provider fal` regardless of
profile) reaches a real provider. See "Data egress" below and
`doomscrum egress` for exactly what leaves the machine, and
[CONFIGURATION.md](CONFIGURATION.md) for every field these profiles can
override.

## Data egress

DoomScrum is MIT-licensed (see [LICENSE](../LICENSE)). **Videos are
AI-generated** — they do not depict real events, and the spoken content is
derived from backlog spec text, not verified fact.

When a real provider is used (not the default local `fake` preview),
text leaves the machine via exactly two payloads. The runtime is the source
of truth — `doomscrum egress` prints the live enumeration, `GET /api/egress`
returns it as JSON, and the feed UI surfaces it in a disclosure panel (the
`egress` chip) — this section is the prose summary of the same code-verified
list (`src/egress.rs`):

1. **OpenRouter (scriptwriter).** With `script.mode = "llm"`, the full raw
   spec markdown (`prd.raw`) is sent to OpenRouter's chat-completions API to
   generate the spoken script + visual scene. The spec is wrapped in an
   untrusted-data fence (it cannot break out as instructions), but the raw
   text still egresses. Source: `src/scriptwriter.rs` (`request_body`).
2. **fal.ai (render prompt).** With `provider = "fal"`, the spec **title**
   (attacker-controlled — the first `# ` line), **goal**, and **first
   acceptance criterion** are distilled into the spoken script and embedded
   in the composed provider prompt sent to fal.ai's text-to-video model. The
   title also flows into the PR title, commit message, and branch slug (argv
   tokens — no shell injection). Source: `src/distill.rs`
   (`compile_with_format` → `format_prompt`), sent by `src/providers/fal.rs`.

The local `fake` provider and `templates` script mode never egress.
`doomscrum doctor` checks keys and config before a paid run. Current provider
terms and preliminary name-risk findings are recorded in [LEGAL.md](LEGAL.md):
project-site use is WARN/scoped; paid promotion remains blocked pending exact
clip provenance and formal review.

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

## Garbage collection

`doomscrum gc` keeps generated state bounded. It preserves every render JSON
for audit, deletes only superseded MP4 assets (the latest ready render per
spec/provider survives), runs `git worktree prune`, removes terminal dispatch
worktrees past the age policy, and rotates `events.ndjson` by archiving the
full ledger before keeping recent complete event lines. Use `--dry-run` to
print the actions without touching source specs, open dispatches, renders, or
logs.

## What a swipe actually does

Right swipe (implement):

1. `git worktree add .doomscrum/worktrees/<branch> -b doomscrum/impl-<slug>-<id>`
2. Runs your configured agent command in the worktree with the full spec as
   its mission.
3. Commits anything the agent left uncommitted.
4. Pushes the branch and opens a PR with `gh` (when the repo has an `origin`
   remote and `open_pr = true`; otherwise the branch stays local and the
   receipt says so).

Every dispatch writes a staged receipt to `.doomscrum/dispatches/<id>.json`
plus a full agent log; the feed shows live status stickers and links to
opened PRs. The default agent is the [`opencode`](https://opencode.ai) CLI on
OpenRouter (model `openrouter/z-ai/glm-5.2`) — run `opencode auth login` once
to store your OpenRouter key, then swipe. Change the model with a one-line
`agent_model = "…"` in `doomscrum.toml`, or point `implement_cmd` at codex,
claude, or anything else that takes a prompt and edits a worktree. The
shape-agent backend remains available as an explicit action for future/control
surfaces, but the default left swipe is skip-first.

Agent work is throttled by `agent.max_concurrent_dispatches` (default `2`).
Swipes beyond the limit remain as visible `queued` receipts until a slot
opens, and swiping the same spec/action while a receipt is still active
returns that receipt instead of launching a duplicate agent.

### Dispatch receipt schema

Each `.doomscrum/dispatches/<id>.json` is a `DispatchReceipt`
(`src/dispatch.rs`):

| Field | Type | Meaning |
|---|---|---|
| `id` | string | Receipt id. |
| `prd_id` | string | Spec content hash at dispatch time. |
| `prd_sha256` | string | Full spec sha256. |
| `prd_title` | string | Spec title at dispatch time. |
| `prd_rel_path` | string | Spec's repo-relative path — the stable key across content edits (`prd_id`/`prd_sha256` are content hashes and change on re-shape); used to badge a receipt `superseded` once its spec is re-shaped to a new sha. |
| `kind` | `"implement"` \| `"shape"` | Right swipe (implement the spec) vs. explicit shape action (sharpen the spec itself). |
| `branch` | string | Git branch created for the dispatch. |
| `worktree` | string | Worktree path under `.doomscrum/worktrees/`. |
| `status` | string | One of `queued`, `agent_running`, `opening_pr`, `pr_opened`, `completed_local`, `failed`, `cancelled`. |
| `stages` | array of `{name, command, exit_code, ok}` | Per-stage record (worktree add, agent run, commit, push, PR create). |
| `diff_lines` | integer, optional | Added+removed lines in the agent's diff — the triage size signal. |
| `plan` | string, optional | The agent's own one-line summary (its HEAD commit subject). |
| `pr_url` | string, optional | Opened PR URL, once one exists. |
| `note` | string, optional | Free-text note (e.g. why a PR wasn't opened). |
| `agent_log` | string | Full agent stdout/stderr log. |
| `created_at` / `updated_at` | RFC3339 timestamps | Receipt lifecycle timestamps. |

### Recovering a stuck dispatch

**Detect it:** run `doomscrum report`. The `== dispatches ==` section prints
counts per status (`queued=… running=… opening_pr=…`) and lists the 10 most
recent receipts with their status and `updated_at`. A receipt sitting in
`agent_running` (or `queued`/`opening_pr`) with an `updated_at` far in the
past — the agent process died, panicked, or the machine slept mid-run — is
stuck; it will never self-resolve while the server keeps running, and GC will
keep protecting its worktree from cleanup as long as the status reads
in-flight.

**Recover it:** restart the `doomscrum serve` process. On startup, before it
accepts traffic, the server reconciles every receipt still in an in-flight
status (`queued` / `agent_running` / `opening_pr`) to `failed` — logging
`reconciled stranded dispatch <id> (<title>) -> failed` for each one — on the
premise that the tokio task that owned it died with the previous process.
There is no live (non-restart) command to force this today; a restart is the
supported manual recovery step. Once a receipt reads `failed`, its worktree
is no longer protected and `doomscrum gc` (or `--dry-run` to preview first)
reclaims it.

## Phone on the couch (LAN + PWA)

The server binds `127.0.0.1` by default. To triage from a phone on the same
network, bind a LAN-reachable host:

```bash
cargo run --release -- serve --host 0.0.0.0          # or your machine's LAN IP
# then on the phone: http://<your-machine's-LAN-IP>:4173
```

The feed is an installable PWA (`/manifest.webmanifest` + icons): open it on
the phone and use **Add to Home Screen** (iOS Safari share sheet, or Chrome's
install prompt) to get a standalone, full-screen app icon.

Only bind a non-loopback host on a network you trust: the feed can dispatch
real agents and spend the render budget, and it ships no authentication.

## Syncing to a repo

Point `doomscrum.toml` at any repository:

```toml
[repo]
path = "../some-other-repo"
backlog_dir = "backlog.d"
```

Each `*.md` file in the backlog directory is one spec. Priority is filename
sort order; only the top `feed.max_items` (default 10) enter the feed. Files
starting with `_` are ignored (use `_done/`-style prefixes for archives).

Real renders happen just-in-time: serving the feed renders at most
`feed.prefetch_depth` (default 3) specs ahead of your viewport, so a long
backlog costs a handful of clips, not one per spec. Deeper cards stay free
until you scroll toward them. The local preview provider never leaves the
machine; if a paid wallet cap is exhausted, the card degrades to a free
preview (badged “render budget exhausted”) instead of breaking the feed.
