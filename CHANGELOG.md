# Changelog

All notable changes to DoomScrum are documented here, in
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) style. No versioned
release has tagged yet — everything below is `[Unreleased]`. Once the first
`vX.Y.Z` tag is pushed, `.github/workflows/release.yml` builds signed binaries
for macOS arm64 and Linux x86_64 and creates the GitHub Release with
`generate_release_notes: true`, so every future tagged release carries its
own auto-generated notes going forward; this file stays the durable,
hand-curated summary of *why*, not just *what*.

## [Unreleased]

### Added

- Release workflow: tag-push (`v*`) builds checksummed macOS-arm64 and
  Linux-x86_64 binaries, publishes a GitHub Release with auto-generated
  notes, and updates the `phrazzld/homebrew-doomscrum` tap formula —
  `brew install phrazzld/doomscrum/doomscrum` is a real install path.
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

### Fixed

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
