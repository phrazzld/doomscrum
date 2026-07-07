# MVP: prove the dispatch loop on arbitrary repos with a real agent (config-heavy is fine)

Priority: P0 · Status: **absorbed** by Powder card `doomscrum-940` (board is the
ledger of record; closed 2026-07-07 with the live two-repo proof — evidence:
`docs/adoption/2026-07-07-gate0-external-dispatch/`) · Estimate: XL (epic — the
singular Gate-0 focus)

## Goal
A user clones DoomScrum, runs a guided setup (OpenRouter key, GitHub auth, FAL
key) with preflight sanity checks, points it at an **arbitrary** repo + backlog,
swipes, and a **real PR is opened on that repo by a real OpenRouter-backed coding
agent.** Config-heavy is acceptable; agent security sandboxing is explicitly out
of scope (the local single-operator trust model makes it premature — see
`VISION.md` "Operating assumption").

## Oracle (whole-arc)
- [ ] Implement/shape swipes run a **real** coding agent, not a stub: the
      **`opencode` CLI pointed at OpenRouter**, default model **GLM 5.2**,
      changeable in `doomscrum.toml` in one line — replacing the current `codex`
      default (`config.rs`).
- [ ] A live run against an arbitrary **external** repo with a real GitHub origin
      opens a real PR, captured as a dated evidence packet (repo, PR URL, agent
      transcript). Supersedes [[016-multi-repo-sync]] child-3.
- [ ] Proven against **≥2 distinct** arbitrary repos/backlogs (not DoomScrum's
      own) — confirming it isn't hardwired to this repo's conventions.
- [ ] A guided setup flow (TUI/GUI wizard or a documented `doomscrum init`) walks
      the operator through: OpenRouter key, GitHub auth (reuse existing `gh`),
      FAL key (optional — the free fixture path works without it), and repo +
      backlog selection.
- [ ] Preflight sanity checks run before the first dispatch and **fail loudly
      with fix-it guidance** if any of: OpenRouter key missing/invalid, `gh` not
      authenticated, target isn't a git repo or has no writable remote, or an AI
      render is requested with no FAL key.
- [ ] Security note recorded: agent filesystem/network sandboxing is OUT of scope
      here, deferred to [[039-agent-filesystem-egress-sandbox]] (Gate 3). Consent
      + mis-swipe undo stay.

## Verification System
- Claim: a cold operator can configure DoomScrum and open a real PR on their own
  arbitrary repo via a swipe, using a real OpenRouter agent.
- Falsifier: setup that dead-ends without clear guidance; a swipe that runs a
  stub or fails to open a PR on a real external remote; behavior hardwired to
  DoomScrum's own repo layout.
- Driver: a live dispatch at two throwaway external repos with real GitHub
  remotes, real OpenRouter + FAL keys, from a clean config.
- Grader: two real PR URLs + agent transcripts in a dated evidence packet; the
  preflight check demonstrably catches each misconfiguration.
- Evidence packet: `docs/adoption/` (or `.doomscrum/`) dated run record with PR
  links and the setup transcript.
- Cadence: per-child failing test first where unit-testable (agent config,
  preflight); the live two-repo proof at the final child.

## Children (ordered)
1. **Real agent by default.** ✅ DELIVERED 2026-06-25 (branch
   `deliver/043-opencode-default`). Default `implement_cmd`/`shape_cmd` are now
   `opencode run --dir {worktree} -m {model} {prompt}`; new `agent_model` field +
   `{model}` placeholder makes the model a one-line change (default
   `openrouter/z-ai/glm-5.2`, verified in the live catalog). opencode auths from
   its own credential file via HOME, so no key enters the agent env; allowlist +
   denylist untouched. codex/claude remain overrides. Gate green (fmt/clippy/22
   tests) + fresh-context review = SHIP. Live execution is proven in child 4.
2. **Preflight sanity checks.** ✅ DELIVERED 2026-06-25. `doomscrum doctor`
   (pure `preflight::evaluate(Facts)` + thin CLI I/O shell) validates OpenRouter
   key, opencode *stored* auth, `gh` auth, git work-tree + push remote, and FAL
   presence-vs-need, with fix hints; exits non-zero on any FAIL. Live QA caught a
   real gap (opencode env-only, no stored cred). 9 unit tests.
3. **Guided setup.** ✅ DELIVERED 2026-06-25. `doomscrum init` scaffolds a
   starter `doomscrum.toml` (dogfoods the opencode default; no-clobber) and
   prints the setup checklist + a live `doctor` readout. (A fully interactive
   TUI wizard remains a possible follow-up; the non-interactive scaffold is the
   testable, headless-safe core.)
4. **Live proof, repo #1.** ✅ DELIVERED 2026-06-25 — the full loop ran live for
   the first time. An implement swipe on spec 022 dispatched a real
   `opencode`/GLM-5.2 agent (OpenRouter) in a worktree; it implemented the spec
   (+521/−7: LICENSE, docs/LEGAL.md, new src/egress.rs, server/UI disclosure
   wiring), DoomScrum committed it, the pre-push secret scan passed, the branch
   pushed, and `gh` opened a real PR — **https://github.com/phrazzld/doomscrum/pull/8**.
   Stages: worktree✓ agent✓ push✓ pr✓. (Dispatch id 2edf16d4.) NOTE: this was a
   *self*-dispatch (doomscrum→doomscrum). Foreign-repo *routing* is separately
   covered by the e2e test `dispatch_against_a_foreign_repo_routes_to_that_repos_remote`;
   a *live agent against a truly foreign repo* is child 5. PR #8 is raw,
   unreviewed agent output — review/CI it; do not merge blind.
5. **Generality proof, repo #2.** Repeat with a *live* agent against a second,
   truly external repo/backlog to prove it isn't repo-specific. (Still open.)

## Notes
**Why:** owner redirect 2026-06-25 — prioritize making the loop *actually work on
arbitrary codebases and open real PRs* over premature security hardening. The
local single-operator trust model (your machine/repo/key/spec) makes
spec-injection and secret-egress concerns premature, so [[039-agent-filesystem-egress-sandbox]]
is deferred to Gate 3. This aligns the whole stack on OpenRouter (the scriptwriter
already uses OpenRouter), so a v1 operator brings one OpenRouter key + one FAL key
+ their `gh` auth and it works.

**Relationships:** supersedes the live-PR keystone in [[016-multi-repo-sync]]
child-3 (the picker/sync plumbing it built is the substrate this rides on);
overlaps [[019-onboarding-first-run]] (which targets the *fixture-video* first-run
— this adds the *dispatch-config* path; fold or sequence them when 019 is picked
up). Distribution (installable binary, [[017-distribution]]) is NOT required here
— "clone and run" is an acceptable v1 install.

**Default agent (resolved 2026-06-25):** the `opencode` CLI pointed at OpenRouter
with GLM 5.2 — a real open-source coding agent driven by a command template, no
new in-crate agent code. `codex`/`claude` remain available as overrides via the
existing `implement_cmd`/`shape_cmd` templates. Child 1 should verify `opencode`
takes a prompt + worktree non-interactively and respects `OPENROUTER_API_KEY` +
a model flag.
