---
name: doomscrum
description: >-
  Operate DoomScrum from this repo: run the swipe-feed server, generate local
  feed renders, check dispatch readiness, and garbage-collect generated state.
  Use when asked to operate, diagnose, or clean up DoomScrum itself. Trigger:
  /doomscrum.
---

# DoomScrum

DoomScrum turns repo backlog specs into a swipeable video feed. Right swipes
launch real coding agents and can open real PRs, so preserve the consent model:
do not dispatch on the operator's behalf unless the task explicitly authorizes
it. Left swipe is skip-first and must not mutate the source spec.

Read `VISION.md`, `AGENTS.md`, and `docs/FIVE_FACES.md` before changing product
direction. Use the Rust CLI as the control surface; `assets/index.html` is the
only sanctioned non-Rust product surface.

## Core verbs

### Serve

```sh
cargo run --release -- serve
```

Default URL: `http://127.0.0.1:4173`. Bind a non-loopback host only on a trusted
network; the feed can dispatch real agents and spend render budget.

### Generate

```sh
cargo run --release -- generate --provider fake
```

Use `fake` for normal iteration: it is offline, free, and does not egress.
Anything that spends fal.ai money or uses `--profile content` belongs to the
project `render-feed` skill and needs explicit operator approval.

### Doctor

```sh
cargo run --release -- doctor
```

Run this before claiming dispatch readiness. It checks agent auth, GitHub auth,
repo git state, origin remote, and configured keys.

### Garbage collect

```sh
cargo run --release -- gc --dry-run
cargo run --release -- gc
```

Use `--dry-run` first. GC preserves source specs and render provenance, prunes
terminal dispatch worktrees by policy, and rotates the events ledger.

## Useful reads

```sh
cargo run --release -- report
cargo run --release -- egress
```

`report` summarizes specs, renders, dispatch receipts, failures, and spend.
`egress` prints the runtime data-egress disclosure.

## Gate

```sh
cargo run --bin doomscrum-ci
```

Behavior changes need a failing test first. Do not lower fmt, clippy, tests, or
wallet/consent gates to get green.
