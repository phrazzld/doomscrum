# Five-faces surface decision

Status: ratified for DoomScrum on 2026-07-07 by Powder card
`doomscrum-905`.

The application floor is core plus five product faces: API, CLI, MCP, skill,
and UI. DoomScrum's product premise is consented human dispatch: a right swipe
launches real agents that modify real code and open real PRs. Missing faces are
allowed only when the waiver is named and argued here.

## Face ledger

| Face | Status | In-repo proof |
|---|---|---|
| Core | Exists | Rust crate modules under `src/`: backlog scan, rendering, dispatch receipts, events, garbage collection, and preflight checks. |
| CLI | Exists | `src/main.rs` exposes `serve`, `generate`, `script`, `report`, `gc`, `doctor`, `init`, and `egress`. |
| API | Exists | `src/server.rs` exposes the feed and control API, including `/api/state`, `/api/generate`, `/api/swipe`, `/api/dispatches`, `/api/repo`, `/api/keys`, and `/api/egress`. |
| UI | Exists | `assets/index.html` is the embedded swipe feed, the only sanctioned non-Rust product surface. |
| Skill | Built | `.agents/skills/doomscrum/SKILL.md` is the repo-local operator skill for `serve`, `generate`, `doctor`, and `gc`. |
| MCP | Waived for v1 | No MCP server ships in v1; see the named waiver below. |

## MCP waiver: no dispatch-capable MCP face in v1

DoomScrum's load-bearing action is a human thumb granting repo-specific consent
before real code-changing dispatch. A dispatch-capable MCP server would let an
agent call the dispatch path from another harness, bypassing the feed's swipe
moment, its one-time per-repo confirmation, the visible undo window, and the
operator's immediate awareness that a real PR is being attempted. That is not a
small transport gap; it weakens the consent premise described in `VISION.md`.
For v1, MCP dispatch, paid render generation, key mutation, and repo switching
are waived rather than exposed.

A future read-only MCP face may earn its keep once a concrete consumer needs
agent-readable evidence. Its allowed scope should be receipts and provenance:
feed state, dispatch receipts, render provenance, spend summary, data-egress
disclosure, and `events.ndjson` summaries. It must exclude `/api/swipe`,
`/api/generate` with paid providers, `/api/keys`, and all repo-mutating actions
unless the MCP caller can prove the same per-repo human consent the UI requires.

## Legal and safety constraints read

`docs/LEGAL.md` remains a draft pre-launch checklist, not legal advice. This
decision adds no generated-media redistribution, no new provider calls, and no
new data egress. Any future MCP face that exposes spec text or receipts must
preserve the existing disclosure model in `docs/LEGAL.md` and `docs/OPERATIONS.md`:
spec-derived text can be sensitive, provider egress must stay enumerated, and
network surfaces that can dispatch agents or spend render budget must not be
quietly exposed beyond trusted local use.
