# Gate-0 evidence packet — real PRs opened on external repos (2026-07-07)

**Card:** Powder `doomscrum-940` — *Gate-0 keystone: one real PR opened live on an
arbitrary external repo (evidence packet).* Closes `backlog.d/043` child 5 (the
last open child) and the whole 043 arc; VISION.md Gate 0's keystone claim —
"it actually ships code" — is now proven against repos that are not DoomScrum.

Every artifact in this directory was captured live on 2026-07-07 (America/Chicago)
from two dispatch runs driven through the real swipe feed in a browser
(agent-browser CLI driving headed Chrome), not through test harnesses or stubs.

## The two external repos

| | Repo 1 | Repo 2 |
|---|---|---|
| Repo | [phrazzld/vanity](https://github.com/phrazzld/vanity) (public) | [misty-step/chrondle](https://github.com/misty-step/chrondle) (public) |
| Layout | `backlog.d/` numbered specs | `.backlog.d/` **dot-dir** specs (different filename + section conventions: Priority/Status/Non-Goals/Oracle house format) |
| Sync path | Runtime **repo switch in the feed** (`repo:` chip → `/api/repo`) from the default doomscrum sync | **Cold setup**: fresh root dir, `doomscrum init --repo …/chrondle`, one-line `backlog_dir = ".backlog.d"` edit, `doctor` green (see `chrondle-cold-init.txt`) |
| Spec swiped | `backlog.d/009-colophon-pause-control.md` — WCAG 2.2.2 pause/stop control for the typewriter colophon | `.backlog.d/004-polish-recovery-and-motion-accessibility.md` — error-boundary link semantics + reduced-motion celebration |
| Dispatch id | `687be905f1…` | `766744e5b5…` |
| PR | [phrazzld/vanity#123](https://github.com/phrazzld/vanity/pull/123) (+401/−76) | [misty-step/chrondle#268](https://github.com/misty-step/chrondle/pull/268) (+124/−6) |
| Wall clock (swipe → PR) | 15m29s (13:03:27Z → 13:18:56Z) | 9m57s (13:06:56Z → 13:16:53Z) |

**An honest failure, kept in the packet:** a second chrondle dispatch,
`.backlog.d/005-improve-order-mode-affordance-and-a11y.md` (`b4119f8f92…`), ran
12m57s and terminated `failed` with note *"agent produced no commits and no
changes"* — the receipt records the failure loudly instead of faking a green
(`chrondle-005-failed.receipt.json`, `chrondle-005-failed.agent-log.txt`), and the
card offers *run it back*. Exactly the anti-demo-ware behavior VISION.md demands.

The PRs are raw, unreviewed agent output left **open for operator review** — the
deliverable here is the proven loop, not merged code.

## The agent is real, and the model is one config line

The dispatched agent is the configured `opencode` CLI on OpenRouter — not a stub.
`live-agent-processes.txt` captures all three agents mid-run:

```
opencode run --dir <fresh worktree> -m openrouter/z-ai/glm-5.2 "Implement the following spec completely. …<UNTRUSTED_SPEC …fenced spec body…>"
```

Model swap is exactly one line in `doomscrum.toml`:
`agent_model = "openrouter/z-ai/glm-5.2"`.

Agent transcripts (secret-masked by `secrets::mask` before persisting):
`vanity-009.agent-log.txt`, `chrondle-004.agent-log.txt`, `chrondle-005-failed.agent-log.txt`;
durable receipts alongside as `*.receipt.json` (stages: worktree → agent →
push → pr, with created/updated timestamps).

## Consent + undo verified live (still active in the flow)

- **Consent:** the first implement-swipe against each repo raised the
  full-screen consent gate naming the repo ("dispatch a real agent? … opens a
  real pull request") — screenshot `06-consent-gate.png` (vanity) and
  `09-chrondle-consent.png` (chrondle). Dispatch proceeded only after
  *Cook it — I get it*; skip/back swipes never prompted.
- **Undo:** dispatch `f1f2b594…` (chrondle `.backlog.d/003`) was right-swiped
  and cancelled inside the 5-second undo window via the cancel affordance's
  endpoint (`POST /api/dispatch/{id}/cancel` — the exact call the card's
  *cancel dispatch* button makes). Receipt: `chrondle-003-cancelled.receipt.json`
  (`status: "cancelled"`, note `"cancelled during the undo window"`). Verified
  zero git side-effects: no worktree created, no branch in the target repo
  (`git branch --list '*rebuild-archive*'` → empty), nothing pushed.
- **Observation filed as product feedback:** the cancel *button* renders via a
  2s log-poll after a 4s state-poll attaches the dispatch to the card, so the
  visible affordance can appear ~2–4s into the 5s window. A human watching the
  card can beat it; automation raced it twice. Carded separately (see Powder,
  repo `doomscrum`).

## Preflight/doctor fails loudly on a cold setup

Captured under simulated cold setups (`env -i` with a bare `$HOME`, plus config
mutations) — `doctor-cold-fail.txt` and `doctor-noremote-nofal.txt`:

- missing `OPENROUTER_API_KEY` → FAIL with `export OPENROUTER_API_KEY=… (or add it to ~/.secrets), or set script.mode = "templates"`
- no stored opencode credential → FAIL with `run \`opencode auth login\` and choose OpenRouter`
- `gh` unauthenticated → FAIL with `run \`gh auth login\``
- target not a git repo → FAIL with `point [repo].path at a git repo (or run \`doomscrum init\`)`
- no push remote → WARN with `add an \`origin\` remote to open PRs` (deliberate: dispatch degrades to `completed_local`, branch stays local)
- `video.provider = "fal"` with no FAL key → FAIL with `set FAL_API_KEY, or use the free fixture provider`

`doctor` exits non-zero on any FAIL. Positive baseline on this machine: 6/6 ok
(see `chrondle-cold-init.txt` for the cold-root readout).

## Security note (recorded per acceptance)

Agent filesystem/network **sandboxing remains explicitly out of scope** for
Gate 0, per VISION.md "Operating assumption (v1)": DoomScrum is a local,
single-operator tool running the operator's own specs with the operator's own
credentials, so untrusted-spec hardening is deferred to
`backlog.d/039-agent-filesystem-egress-sandbox` (Gate 3, multi-tenant trigger).
What *is* active in the flow today, verified live in this packet:

- one-time per-repo **consent** gate before the first real dispatch (above);
- 5s **undo window** with zero-git-side-effect cancellation (above);
- agent process env built from an **allowlist only** (PATH/HOME/locale — no API
  keys; opencode authenticates from its own credential file via HOME);
- spec bodies wrapped in **UNTRUSTED_SPEC fencing** in the agent prompt (visible
  in `live-agent-processes.txt`);
- **secret-shaped-token diff refusal** before any push, and secret masking in
  persisted logs.

## Screenshots

| File | Moment |
|---|---|
| `04-vanity-card4-fresh.png` | vanity feed, card #4 (spec 009) fresh |
| `05-vanity-spec-overlay.png` | tap-to-read source spec: path + sha256 on screen |
| `06-consent-gate.png` | vanity consent gate before first dispatch |
| `08-chrondle-spec-overlay.png` | chrondle card #5 = `.backlog.d/005…` (dot-dir layout live in the feed) |
| `09-chrondle-consent.png` | chrondle consent gate (asked once more for the new repo) |
| `10-chrondle-agent-cooking.png` | AGENT COOKING sticker during the live 005 run |
| `12-chrondle-after-undo.png` | feed after the 003 undo |
