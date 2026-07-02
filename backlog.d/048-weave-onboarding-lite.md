# weave-onboarding-lite: which "five faces" onboarding concerns actually apply

Priority: P2 · Status: ready · Estimate: M (epic)

## Goal
Decide, with citation, which of the "five faces" onboarding concerns
(landmark/release, cerberus/review-gate, canary/observability, a hosted-deploy
concern, and first-run onboarding) actually apply to DoomScrum given it is
local, BYOK, no-deployment software — and scope a lightweight first-run
onboarding path to exactly what's locally relevant, no more.

## Oracle
- [ ] Local precedent check completed and cited (done as part of filing this
      ticket): grepped this repo's `AGENTS.md`, `VISION.md`, and `backlog.d/`
      for "landmark", "cerberus", "canary", and "five faces" — **no local
      precedent exists** for these as named onboarding-framework concepts in
      this repo. The single "canary" hit in the repo
      (`docs/bench/20260611-script-bench.md`) refers to an unrelated
      canary-*rollout* pattern for a different project's Pi/OpenRouter
      adapter — not a DoomScrum onboarding face. This ticket does not assume
      any five-faces machinery is wanted or half-built; it starts from zero
      and only imports what's justified below.
- [ ] Applicability recorded per concern, one line of reasoning each:
      - **landmark (release process): applies.** DoomScrum ships as
        installable software a stranger runs locally; maps to the existing
        [[017-distribution]] (installable releases, signed binary + tap). No
        new machinery — link the two tickets.
      - **cerberus (review/quality gate): applies.** Maps to the existing
        repo-owned gate `cargo run --bin doomscrum-ci` (fmt + clippy + test,
        per `AGENTS.md`). No new machinery — confirm this is the canonical
        gate and cite it here rather than standing up a parallel one.
      - **canary (agent-infra observability): does not apply.** There is no
        hosted agent fleet or shared infrastructure to observe — DoomScrum
        dispatches a single `opencode` CLI process on the operator's own
        machine, per their own consent, per `VISION.md`'s "local,
        single-operator tool" operating assumption.
      - **hosted-deploy concern (e.g. a "powder"-style deploy face): does not
        apply.** `VISION.md`'s Operating assumption section is explicit:
        local-first today, hosted/SaaS is an open bet, not a decision — no
        deployment surface exists to onboard against.
      - **first-run onboarding: applies, already tracked.**
        [[019-onboarding-first-run]] owns the zero-to-fixture-video on-ramp.
        This ticket folds into or explicitly sequences after 019 rather than
        spawning a second, parallel onboarding surface.
- [ ] Confirmed no new "five-faces" scaffolding (API + CLI + MCP + SDK + skill,
      hosted control plane, cross-service observability, etc.) gets introduced
      by this ticket — the actionable scope is exactly: link
      [[017-distribution]], cite `doomscrum-ci` as the confirmed quality gate,
      and sequence with [[019-onboarding-first-run]].
- [ ] Ticket closes (or folds fully into 019) once the applicability table
      above is the recorded decision, unless the table surfaces a real,
      previously-untracked gap in 017/019/gate coverage — in which case that
      gap gets its own child ticket instead of scope creep here.

## Verification System
- Claim: DoomScrum's onboarding needs are fully covered by its two applicable
  faces (release, quality gate) plus existing first-run work — nothing more.
- Falsifier: a genuine onboarding gap exists that isn't covered by
  017/019/`doomscrum-ci` and isn't one of the explicitly-inapplicable faces
  (canary, hosted-deploy).
- Driver: the repo-wide grep for landmark/cerberus/canary precedent (already
  run, cited above) plus a re-read of 017 and 019's current scope.
- Grader: the five-row applicability table with citations; whether it points
  at real existing tickets instead of inventing new machinery.
- Evidence packet: this ticket, cross-linked from 017 and 019.
- Cadence: one pass now; revisit only if the "hosted/SaaS" open bet in
  `VISION.md` ever gets decided — that would flip canary and the hosted-deploy
  concern to "applies."

## Notes
Filed 2026-07-02 per operator request to assess "five faces" onboarding
applicability for local-only software. Deliberately scoped down from the full
five-faces model: two faces map to work that already exists
([[017-distribution]], the `doomscrum-ci` gate), two faces are explicitly
out of scope under the current local-single-operator model
(canary/observability, hosted-deploy), and first-run onboarding stays owned by
[[019-onboarding-first-run]] rather than forking.
