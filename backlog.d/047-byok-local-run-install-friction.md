# BYOK local-run install friction audit

Priority: P1 · Status: ready · Estimate: L (epic)

## Goal
A stranger with no context should get from "found this repo" to "first
fixture video playing, feed running" in well under two minutes, on their own
machine, using their own OpenRouter/FAL/`gh` credentials — measure what's real
today versus aspirational, since DoomScrum's distribution model is
download-and-run-locally with no hosted service.

## Oracle
- [ ] Full dependency chain enumerated and timed on a clean checkout: Rust
      toolchain, `cargo build --release`, optional `ffmpeg` (needs the
      `drawtext` filter for the fixture text overlay), the `opencode` CLI +
      `opencode auth login` (required for dispatch — the agent runs with a
      scrubbed env, so only opencode's *stored* credential reaches it, per
      `src/preflight.rs`), `gh` CLI + `gh auth login` (required for PR
      opening), optional `FAL_API_KEY`. Each dependency marked
      required-for-fixture-video vs. required-for-dispatch vs.
      required-for-real-render.
- [ ] Confirm or deny "single command install": today's README quick start is
      `cargo build --release` then four more commands (`init`, `doctor`,
      `generate`, `serve`) before a video plays — measure actual wall-clock
      from cold clone to first playing video and cite the number against the
      "under two minutes" bar.
- [ ] Cross-platform pass: identify macOS/Linux-only assumptions in the build
      and runtime (Cargo.toml deps — `reqwest`/`tokio`/`rustls-tls` are
      cross-platform; `headless_chrome` is dev/test-only; audit `git worktree`
      usage in `src/dispatch.rs` and `~/.secrets`/`$HOME` path handling in
      `src/secrets.rs`) and either file concrete Windows follow-ups or record
      an explicit "Windows out of scope for v1" decision.
- [ ] BYOK key setup UX rated: a key can live in three different places today
      — `OPENROUTER_API_KEY`/`FAL_API_KEY` env vars, a `~/.secrets` file, or
      `opencode`'s own stored auth (`opencode auth login`) — assess whether
      that's confusing for a first-timer with zero DoomScrum context. Review
      `doctor`'s fix-it messages (`src/preflight.rs`) for whether they're
      sufficient to unblock a stranger unassisted.
- [ ] Relationship to [[017-distribution]] recorded explicitly: that ticket
      targets a signed single binary + Homebrew tap and is deliberately
      sequenced behind Gate 0/1 per its own Notes. This ticket's friction
      audit is evidence that should inform whether 017 gets pulled forward —
      it does not duplicate 017's distribution-mechanism scope (binary
      signing, tap, release CI).

## Verification System
- Claim: a stranger can go from clone to a playing fixture video, BYOK, in
  under two minutes, with no hosted service involved.
- Falsifier: the real wall-clock time exceeds two minutes; any required
  dependency (ffmpeg, opencode, gh) fails silently or with an unclear error
  instead of `doctor`'s fix-it guidance; the flow breaks on a platform the
  README implies it supports.
- Driver: a timed, cold run of the documented quick start on a clean checkout
  (or clean container/VM per OS), following only README + `doctor` output.
- Grader: the measured wall-clock time, the dependency-chain table, and the
  cross-platform findings table.
- Evidence packet: this ticket's Oracle checkboxes, with the timed run logged
  inline (or in a dated `docs/adoption/` note if it's substantial).
- Cadence: one pass per platform claimed in the README; re-run after any
  change to `init`/`doctor`/the quick-start docs.

## Notes
Baseline to audit against, not rebuild: `doomscrum doctor` (preflight checks)
and `doomscrum init` (config scaffold) already shipped in 043 children 2-3 —
this ticket measures whether that existing onboarding is actually fast and
cross-platform, it doesn't re-implement it. Filed 2026-07-02 during a
product-groom investigation pass.


## Lead groom additions (2026-07-02, supervisor — from the fact packet)
- The free path is better than feared: `fake` provider needs zero keys and even
  ffmpeg is optional (embedded fixture fallback, providers/fake.rs:13,35-41). The
  demo cartridge (046) should hang off that existing fixture mechanism.
- Child: fix README's key documentation — it names only FAL_API_KEY for real
  renders, but script.mode="llm" (the default, config.rs:179) also requires
  OPENROUTER_API_KEY when paid_render=true. Document the actual matrix.
- Child: macOS CI lane. Zero cfg(target_os) is good, but CI is ubuntu-only and
  the product's stated distribution is "download and run on your own machine" —
  the operator's machine is macOS. At minimum build+test on macos-latest
  (browser e2e can stay ubuntu-only).
