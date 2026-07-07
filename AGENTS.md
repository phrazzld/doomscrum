# DoomScrum — agent contract

Backlog specs → brainrot videos → right swipes dispatch real coding agents.
Left swipe is skip-first and must not mutate the source spec or dispatch a
shape agent. Shape-agent machinery exists as an explicit backend/control action;
do not bind it to the default left gesture without a fresh product decision.
Single Rust crate (`doomscrum`); `assets/index.html` is the only sanctioned
non-Rust product surface. `demo/` is Remotion dev tooling, outside the product
security/egress boundary.

North star is `VISION.md` (why this exists, the soul, the non-goals). When a
direction call is ambiguous, that file decides — read it, don't guess.

- Config: `doomscrum.toml` (model, durations, spend cap). State:
  `.doomscrum/` (renders, storyboards, launches — gitignored).
- Render profiles: `doomscrum.toml`'s `profile` key / `--profile` flag picks
  a `[profiles.*]` override — `dev` (default, free fixtures) vs `content`
  (real paid renders). Details: README.md "Render profiles".
- A stuck/frozen `agent_running` (or `queued`/`opening_pr`) dispatch means
  the agent process died without updating its receipt. Detect via
  `doomscrum report`; recover by restarting `doomscrum serve` (boot
  reconcile flips stranded in-flight receipts to `failed`, then `doomscrum
  gc` reclaims the worktree). Details: README.md "Recovering a stuck
  dispatch".
- Gate: `cargo run --bin doomscrum-ci` (wraps fmt, clippy, and test).
  Behavior changes get a failing test first.
- Anything that spends FAL money: use the `render-feed` skill
  (`.claude/skills/render-feed/`). No unverified paid render.
- Demo recuts: use the project `demo` skill (`.claude/skills/demo/`).
- Dispatch is unbounded by design (owner call) — wallet protection is real,
  run-packet "bounds" are not. Don't re-introduce them.
- Tickets live in `backlog.d/` (Goal + Oracle format; `_done/` archive).
