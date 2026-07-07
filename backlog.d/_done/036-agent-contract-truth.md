# Agent-contract truth: real gate, profiles, recovery runbook

Priority: P2 · Status: ready · Estimate: S

## Goal
The checked-in agent contract matches reality, and a cold agent can run, extend,
AND recover the project from what's checked in.

## Oracle
- [ ] AGENTS.md/CLAUDE.md state the real gate — `cargo fmt --check`, `cargo
      clippy --all-targets -- -D warnings`, then `cargo test` — reconciled
      against `.github/workflows/ci.yml`; no second divergent copy (single
      source or symlink).
- [ ] The contract documents the `--profile` dev/content lever (the free-by-
      default safety switch) and a short failed/stuck-dispatch recovery
      procedure.
- [ ] A frozen `agent_running` receipt is surfaced by `doomscrum report` (or a
      documented manual step) so a cold operator can detect and clear it.

## Notes
From the groom docs/agent-readiness lane (2026-06-17), vetted: CLAUDE.md:10 and
AGENTS.md:10 both say "Gate: cargo test" but CI enforces three gates (ci.yml) —
a cold agent that trusts the contract pushes red CI; README.md:149 has it right,
so the agent-facing copy is the stale one. `diff AGENTS.md CLAUDE.md` →
byte-identical (zero-value duplicate). `--profile`/`[profiles.*]` is undocumented
outside `doomscrum.toml`. The recovery-runbook oracle overlaps
[[032-jit-render-lifecycle-followups]]'s reconcile-on-boot — once 032 reconciles
frozen receipts automatically, this ticket's third oracle reduces to documenting
that behavior.
