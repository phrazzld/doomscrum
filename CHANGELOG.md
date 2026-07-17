# Changelog

All notable changes to DoomScrum are documented here, in
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) style. Tagged
releases publish checksummed macOS arm64 and Linux x86_64 binaries plus
generated GitHub release notes; this file stays the durable, hand-curated
summary of *why*, not just *what*.

## [Unreleased]

### Added

- A canonical 64-second launch film built from the live product loop: real
  GitHub issues, three scoped/waived feed clips, an honest failed-quality beat,
  a real swipe-opened PR, current economics, and the public Homebrew install.
  The project site now carries a readable poster, a silent first-party hero
  loop, and a compressed narrated cut; full and share encodes are delivered to
  the Desktop.

### Changed

- Launch model ratified: local-first, BYO keys, truthful free previews, and
  operator-owned GitHub/agent credentials. Hosted multi-tenant dispatch stays
  deferred; cloud render credits remain a reversible later experiment.
- The checked-in dogfood config now uses public defaults (OpenRouter endpoint,
  $25 wallet cap) while continuing to read this repo's open GitHub issues.
  Fleet credential brokers and historical content-batch limits no longer leak
  into the public checkout.
- Legal preflight refreshed against current fal.ai, OpenRouter, and Gemini API
  terms plus a preliminary name search. Project-site use remains a dated WARN;
  paid promotion stays blocked until every clip has exact model provenance and
  formal review.

### Fixed

- The free provider now rasterizes each spec's extracted title, goal, and
  acceptance criterion into a portable PPM frame, then encodes it with the
  standard Homebrew ffmpeg path. Missing encoder capability degrades explicitly
  to the bundled sample and `doctor` verifies the required capability set.
- Budget-exhausted previews persist their degradation reason in the provider's
  first provenance write, removing a race where the feed briefly exposed an
  unbadged substitute.
- Stills rendering removes its per-render scratch directory on success and
  every error return. Failed TTS, composition, persistence, and queue jobs no
  longer leave generated `stills-work-*` trees behind.

## [0.2.2] - 2026-07-16

### Added

- GitHub Issues backlog source: `repo.source = "github-issues"` scans the
  synced repo's open issues via the `gh` CLI instead of `backlog.d/`
  markdown. Issue-sourced dispatches append `Fixes #N` to the PR body so a
  merged PR closes the issue — DoomScrum never mutates issues directly.
  Edited issues re-mint the content hash, so stale receipts get the
  superseded badge. `doomscrum doctor` verifies `gh` auth and a GitHub
  `origin` when the source is active. This repo now dogfoods the issues
  source (its `backlog.d/` is history).
- Stills render pipeline (`stills/ken-burns`): one AI keyframe image
  (seedream v4, $0.03) + local ffmpeg Ken Burns motion + deterministic TTS
  (`[video.stills] tts_cmd`, macOS `say` default) + estimated word-synced
  captions — a fully bespoke ~$0.03 clip, now the heaviest weight in the
  content mix (avg ≈ $0.43/clip, was $0.94). `doomscrum doctor` requires
  ffmpeg/ffprobe when a `stills/` pipeline is configured.
- Release workflow: tag-push (`v*`) builds checksummed macOS-arm64 and
  Linux-x86_64 binaries, publishes a GitHub Release with auto-generated
  notes, and updates the `misty-step/homebrew-doomscrum` tap formula —
  `brew install misty-step/doomscrum/doomscrum` is a real install path.
  (#19, #20, #21, #22 — doomscrum-903)
- Demo cartridge: pre-rendered sample brainrot videos bundled in the binary
  (`assets/samples/`) and bootstrapped into `.doomscrum/renders/` on first
  `init`/`generate`/`serve`, so the feed shows real video with zero keys and
  zero config edits. Samples are badged "sample video" in the feed. (#12)
- Runtime reliability epic: durable cost ledger, reconcile-on-boot for
  stranded dispatches (`queued`/`agent_running`/`opening_pr` → `failed` on
  restart), and self-healing renders. (#16 — doomscrum-931)
- Card-anatomy pass: dedicated caption zone in the feed card layout. (#15 —
  doomscrum-941)
- First-run on-ramps: every empty state (no specs, no renders, first swipe)
  becomes an obvious next action instead of a dead end. (#14 —
  doomscrum-942)
- Shortform feel epic: video preload, drag-commit stamps, live captions,
  progress indicator, installable PWA (`/manifest.webmanifest` + icons) for
  phone-on-the-couch triage. (#17 — doomscrum-943)
- One-time first-dispatch consent gate per synced repo ("Cook it / Cancel")
  before the first real agent dispatch against that repo. (#6)
- Just-in-time viewport rendering: the feed renders at most
  `feed.prefetch_depth` specs ahead of the cursor instead of the whole
  backlog up front. (#5)
- `@misty-step/aesthetic` visual language adopted for the feed UI. (#3)

### Changed

- Project identity moved to the `misty-step` org: install path is
  `brew install misty-step/doomscrum/doomscrum`, tap is
  `misty-step/homebrew-doomscrum`, site/README/demo URLs point at
  `github.com/misty-step/doomscrum`. Personal-repo evidence links follow
  the owner's `phrazzld` → `moomooskycow` rename. GitHub-side transfers
  executed 2026-07-16: source repo and tap now live under `misty-step`
  (public), so the release workflow's MIGRATION GATE is lifted and the
  formula uses plain public release URLs instead of the authenticated
  private-repo download strategy. Captured agent logs/receipts under
  `docs/adoption/` keep their original verbatim URLs.
- Scriptwriter reevaluated on 2026-07-16
  (`docs/bench/20260716-script-bench.md`). The prompt now requires the first
  sentence to name the ask plainly while slang carries delivery, never content
  words. The default moved to `google/gemini-3.1-flash-lite`, the affordable
  finalist that satisfied the full comprehension contract at roughly
  $0.0008/script.
- Release intelligence via Landmark: `.landmark.yml` declares the release
  contract and the tag workflow prepends a user-facing “What’s New” section
  after creating each GitHub Release.

### Removed

- Sora-2 dropped from the default model and content mix: fal deprecated
  the `fal-ai/sora-2/text-to-video` endpoint (verified 2026-07-15).
  veo3.1/lite carries the cheap native-video weight; seedance-2.0/fast
  stays the hero tier.

### Fixed

- Wallet counted only `provider == "fal"` entries toward the spend caps;
  any paid render (e.g. stills keyframes) now counts, so the total/daily
  guards gate every dollar regardless of provider name.
- The feed's full-backlog scan passed `usize::MAX` to `gh issue list
  --limit`, which gh rejects; the issues adapter clamps the limit to 1000
  (pagination beyond `feed.max_items` is a non-goal).
- Homebrew tap formula: correct the custom download-strategy header
  injection for private-repo release-asset downloads, plus timed audit
  evidence for the release path. (#22 — doomscrum-903)
- Release workflow: escape `$stdout` in the generated Homebrew formula's
  Ruby heredoc. (#21 — doomscrum-903)
- Demo cartridge samples embedded directly in the binary (rather than
  fetched at runtime) and private-repo tap downloads authenticated
  correctly. (#20 — doomscrum-903)
- `quinn-proto` bumped 0.11.14 → 0.11.15 (RUSTSEC-2026-0185, HIGH). (#10)

### Documentation

- Gate-0 evidence packet: real PRs opened live against two external repos,
  demonstrating the swipe → worktree → agent → PR path end to end. (#13)
- Render profiles (`dev` vs `content`) and stuck-dispatch recovery
  documented. (#18 — doomscrum-930)
- Key matrix clarified: `OPENROUTER_API_KEY` is required for paid renders,
  not just `FAL_API_KEY`; macOS CI notes. (#11)
- Legal/safety baseline ahead of public launch: AI-generated-video
  disclosure, data-egress enumeration (`doomscrum egress`, `/api/egress`),
  and the pre-launch legal checklist (`docs/LEGAL.md`). (#8)

### Chore

- CI: preserve `master` branch CI run history. (#9)
- Dependencies: bump `esbuild` and `@remotion/cli` in `/demo`. (#4)
