# MVP: prove the dispatch loop on arbitrary repos with a real agent (config-heavy is fine)

Priority: P0 · Status: ready · Estimate: XL (epic — the singular Gate-0 focus)

## Goal
A user clones DoomScrum, runs a guided setup (OpenRouter key, GitHub auth, FAL
key) with preflight sanity checks, points it at an **arbitrary** repo + backlog,
swipes, and a **real PR is opened on that repo by a real OpenRouter-backed coding
agent.** Config-heavy is acceptable; agent security sandboxing is explicitly out
of scope (the local single-operator trust model makes it premature — see
`VISION.md` "Operating assumption").

## Oracle (whole-arc)
- [ ] Implement/shape swipes run a **real** coding agent, not a stub: an
      OpenRouter-backed open-source coding agent, default model **GLM 5.2**,
      changeable in `doomscrum.toml` in one line — replacing the current `codex`
      default (`config.rs`). *(Confirm the exact agent CLI — see Notes.)*
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
1. **Real agent by default.** Swap the dispatched-agent default from `codex` to
   an OpenRouter-backed open-source coding agent (default model GLM 5.2),
   wiring model + key through `doomscrum.toml`. Keep the existing command-template
   pluggability (`implement_cmd`/`shape_cmd`/`pr_cmd`) so any agent still works.
2. **Preflight sanity checks.** A `doctor`/preflight pass that validates
   OpenRouter key, `gh` auth, git remote writability, and FAL presence-vs-need,
   with actionable failure messages.
3. **Guided setup.** An onboarding flow (wizard or `doomscrum init`) that
   captures the three credentials + repo/backlog selection and writes a valid
   `doomscrum.toml`.
4. **Live proof, repo #1.** Dispatch against a real external repo; open a real
   PR; capture the evidence packet. (Supersedes 016 child-3.)
5. **Generality proof, repo #2.** Repeat against a second, differently-shaped
   repo/backlog to prove it isn't repo-specific.

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

**CONFIRM (open question):** "OpenRouter open-code agent" — leading interpretation
is the `opencode` CLI pointed at OpenRouter with GLM 5.2. Alternatives: a thin
custom OpenRouter coding loop, or `codex`/`claude` kept as fallbacks via the
existing command templates. Owner to confirm the default agent CLI before child 1.
