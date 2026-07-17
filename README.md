# DoomScrum

Backlog triage as a TikTok feed. DoomScrum reads specs from a repo's backlog —
markdown files or open GitHub issues — turns each one into a goofy shortform
video, and lets you swipe:

| Gesture | Action |
|---|---|
| **swipe →** | dispatch a coding agent in a fresh git worktree to **implement** the spec and open a PR |
| **swipe ←** | skip to the next spec without mutating the source spec |
| **swipe ↑** | skip to the next spec (recorded, durable) |
| **swipe ↓** | go back to the previous spec |
| **tap** | read the exact source spec (path + sha256) |

Arrow keys mirror the gestures; `space` pauses; `enter` opens the spec.

Right swipes launch **real agents** that modify code and open **real pull
requests**. That is the point. There is no sandbox theater beyond what your
agent CLI provides.

Because that consequence is real, the **first** implement dispatch against a
repo opens a one-time confirmation that names exactly what will happen
("launches a real agent and opens a real PR against `<repo>`") with **Cook it /
Cancel**. Once acknowledged, DoomScrum does not ask again for that repo;
switching to a different repo asks once more. Skip (← or ↑) and back (↓) never
dispatch and never prompt. This is consent, not a quota — dispatch stays
unbounded once you've opted in.

Each spec is translated into one of five live brainrot formats (AI fruit
telenovela, unhinged Gen-Z explainer, cryptid vlog, Italian brainrot, 2080
street interview), rotated by feed position so consecutive videos never look
alike, always speaking the spec's actual goal and first acceptance
criterion — the video communicates the spec, it doesn't just vibe.

## Quick start

No Rust toolchain needed — install the binary and play a sample video in
under two minutes:

```bash
# macOS (Homebrew)
brew install misty-step/doomscrum/doomscrum

# or download directly (macOS arm64 / Linux x86_64) — checksummed
# releases: https://github.com/misty-step/doomscrum/releases/latest

doomscrum init      # scaffold doomscrum.toml
doomscrum doctor    # check your setup (agent auth, gh auth, git remote)
doomscrum serve     # http://127.0.0.1:4173 — bundled sample videos play immediately
```

`init` writes a starter `doomscrum.toml`; `doctor` verifies that the dispatched
agent can authenticate (`opencode auth login`), `gh` is logged in, and the synced
repo has a push remote — so a swipe can actually open a PR. `serve` bootstraps
the bundled sample brainrot videos into `.doomscrum/renders/` on first run, so
the feed has real (pre-rendered) video with zero keys and zero config edits —
tap the splash screen (sound gate), then swipe. Run `doomscrum generate`
afterward to render spec-branded previews locally with the free `fake`
provider; see "Keys you need" below for real generation.

### Building from source (contributors)

```bash
cargo build --release
cargo run --release -- init
cargo run --release -- doctor
cargo run --release -- generate
cargo run --release -- serve        # http://127.0.0.1:4173

# Inspect generated-state cleanup without deleting anything
cargo run --release -- gc --dry-run
```

### Keys you need

| What you're doing | Keys required |
|---|---|
| Local spec previews (`fake` provider) | None; ffmpeg recommended, disclosed embedded fallback otherwise |
| Dispatch (right-swipe an agent) | `opencode auth login` + `gh auth login` |
| Paid script (`script.mode = "llm"`) | `OPENROUTER_API_KEY` |
| Paid render (`--provider fal` or cook with AI) | `FAL_API_KEY` + `OPENROUTER_API_KEY` |

Set keys as env vars or in `~/.secrets` (`FAL_API_KEY=...`,
`OPENROUTER_API_KEY=...`). Real generation costs money per render and sends
spec-derived prompt text to fal.ai and OpenRouter — treat it as a disclosure
event; DoomScrum quotes the estimated batch cost first and enforces spend
caps. The local preview provider (`fake`) is the default and never leaves the
machine. Full cost model, render profiles, and the exact data-egress
enumeration: [docs/OPERATIONS.md](docs/OPERATIONS.md).

## Legal / safety disclosure

DoomScrum is MIT-licensed (see [LICENSE](LICENSE)). **Videos are
AI-generated** and derived from backlog spec text, not verified fact. When a
real provider is used, spec-derived text leaves the machine — `doomscrum
egress` prints the live, code-verified enumeration of exactly what and where.
Current provider-terms and preliminary name-risk review:
[docs/OPERATIONS.md#data-egress](docs/OPERATIONS.md#data-egress) and
[docs/LEGAL.md](docs/LEGAL.md). Project-site display is covered by a dated,
scoped waiver; paid promotion and third-party redistribution remain blocked
until every clip is mapped to its exact model terms. DoomScrum is not
affiliated with id Software, ZeniMax, or Microsoft.

## Development

```bash
cargo run --bin doomscrum-ci  # aggregate local gate: fmt, clippy, Rust + script tests

cargo fmt --check             # focused lanes for debugging
cargo clippy --all-targets -- -D warnings
cargo test                    # unit + end-to-end HTTP tests (stub agents, real git remotes)
```

The end-to-end tests exercise the actual HTTP routes: a right swipe in the
test suite creates a real worktree, runs a stub agent, pushes to a real
bare remote, and asserts the PR command ran. The FAL provider is tested
against a mock queue API; one fixture render is embedded so tests run
offline with zero external dependencies.

### Layout

```
src/
  backlog.rs       spec scanning (markdown dir or GitHub issues via gh), hashing, priority cap
  distill.rs       markdown → brief → storyboard (the brainrot script)
  egress.rs        runtime data-egress disclosure (CLI + /api/egress + UI)
  providers/       fake (local spec preview), stills, and fal video generation
  dispatch.rs      swipe → worktree → agent → commit → push → PR
  events.rs        durable NDJSON decision ledger
  gc.rs            generated-state lifecycle and dry-run reporting
  server.rs        axum API + embedded UI
  main.rs          CLI: serve | generate | script | report | gc
assets/index.html  the feed UI (single embedded file; the only non-Rust surface)
docs/archive/      the original (superseded) MVP spec
```

The UI is one static HTML file embedded in the binary — browsers execute
JS, so the gesture/video shim is the single deliberate non-Rust exception.
All state, logic, and dispatch live in Rust.

## Docs

- [docs/CONFIGURATION.md](docs/CONFIGURATION.md) — complete `doomscrum.toml` field reference, verified against `src/config.rs`.
- [docs/OPERATIONS.md](docs/OPERATIONS.md) — render profiles, data egress, provenance, garbage collection, dispatch receipts, stuck-dispatch recovery, LAN/PWA.
- [docs/FIVE_FACES.md](docs/FIVE_FACES.md) — five-faces floor status, the repo-local skill face, and the v1 MCP waiver.
- [docs/LEGAL.md](docs/LEGAL.md) — pre-launch legal checklist (provider terms, trademark).
- [CHANGELOG.md](CHANGELOG.md) — release history.
