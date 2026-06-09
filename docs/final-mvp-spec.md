# Context Packet: PRD Brainrot Swipe MVP

## PRD Summary
- User: A local operator who has PRD-shaped backlog items and wants to triage agent work through intentionally absurd shortform video.
- Problem: Agent execution is increasingly cheap, but backlog/spec review is still dull, scattered, and easy to postpone.
- Goal: Turn local `backlog.d/*.md` PRDs into goofy native-audio MP4s that support inspect, skip, needs-spec, and bounded-run decisions.
- Why now: Current video generation APIs can produce short vertical video with audio, making the original bit feasible enough to prototype.
- UX enabled: The operator watches a cursed shortform rendering of a PRD, flips to inspect the exact source spec, and swipes into a durable local decision.
- Deliverable type: Working prototype.
- Success signal: Five sample PRDs generate playable shortform MP4 artifacts with provenance, then feed gestures write durable local decisions.

## Goal
Build a local-first prototype that converts markdown PRDs into intentionally goofy shortform MP4 review cards and records swipe decisions without losing source-spec fidelity.

## Product Requirements
- P0: Read PRDs from a local `backlog.d/` directory where each markdown file is one PRD.
- P0: Generate an inspectable `storyboard.json` and a provider-native audio/video MP4 for each selected PRD.
- P0: Preserve exact source PRD path and content hash in every generated artifact and decision event.
- P0: Present a vertical swipe feed where the operator can play/pause, inspect the source spec, skip, mark needs-spec, or create a bounded agent-run intent.
- P0: Right-swipe must create a bounded run packet; it must not directly launch an unconstrained agent.
- P0: Provider calls must be explicitly configured and cost/latency must be recorded.
- P1: Generate two or three provider variants per PRD for vibe comparison.
- P1: Support fallback rendering only as an explicit degraded state, not as the main product promise.
- P1: Allow a later implementation to plug in Jira, Linear, GitHub Issues, or a database without changing the feed/action semantics.

## Non-Goals
- No multi-user auth, team voting, comments, social posting, recommendations, or creator studio.
- No corporate slide deck renderer as the default experience.
- No custom video generation model or custom video editing engine.
- No autonomous backlog cleanup.
- No agent run that mutates a repo without explicit run packet bounds.
- No database-backed backlog in MVP.
- No claim that a generated video is a faithful source of truth. The PRD remains authoritative.

## Constraints / Invariants
- `backlog.d/*.md` is the source of truth and remains user-owned.
- Generated artifacts are sidecars under `.brainrot/` or `artifacts/`, never replacements for PRDs.
- Every MP4 has provenance: PRD hash, provider, model, prompt/storyboard hash, audio mode, cost, latency, and status.
- Native provider audio is the default render target when the selected model supports it.
- Separate TTS is an optional fallback or later enhancement; it is not a mandatory MVP stage.
- Skip and needs-spec decisions are durable local events.
- A needs-spec item cannot be right-swiped into agent execution unless explicitly overridden in the run packet.
- Remote provider generation is an explicit data disclosure event because PRD content leaves the machine.

## Authority Order
tests > artifact provenance > source PRD hash > code > docs > lore

## Repo Anchors
- Workspace is currently scratch-only: `/Users/phaedrus/Documents/Codex/2026-06-05/hey-can-we-prototype-an-app`.
- No app code or git repo exists yet.
- Initial implementation should create:
  - `backlog.d/` sample PRDs
  - `src/` prototype app/service
  - `.brainrot/` generated sidecars and ledger
  - `outputs/` only for user-facing spec/demo artifacts

## Prior Art
- Runway API currently exposes per-second video pricing, including `veo3.1` audio and no-audio variants, which supports treating native audio as a first-class provider capability: https://docs.dev.runwayml.com/guides/pricing/
- fal Model APIs provide access to many production-ready image, video, audio, and multimodal models through a single API, with queueing and pay-per-use billing: https://fal.ai/docs/documentation/model-apis/overview
- fal lists video models with sound/audio generation such as Veo 3.1, Sora 2, and LTX-2, supporting a multi-model MVP adapter: https://fal.ai/docs/documentation/model-apis/overview
- Google Cloud documents Veo 3.1 text/image-to-video outputs, vertical aspect ratios, short clip lengths, quotas, and model IDs: https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/veo/3-1-generate

## Alternatives Considered
| Option | Shape | Strength | Failure Mode | Verdict |
|---|---|---|---|---|
| Static cards first | PRD summaries rendered as swipeable cards | Fastest and cheapest | Violates the core joke; no brainrot artifact | Reject |
| Remotion-only meme renderer | Programmatic video with captions, overlays, maybe TTS | Deterministic and cheap | Feels like engineered slides, not AI slop | Reject for default, keep as dev fallback |
| HeyGen avatar first | Talking-head avatar explains each PRD | Simple API and audio | Too corporate; aesthetic mismatch | Reject for default, maybe special gag renderer |
| Direct Veo/Runway first | One premium provider generates MP4 with audio | Strong native audio/video quality | Provider lock-in and cost surprises | Defer as secondary adapter |
| fal multi-model first | One adapter routes to multiple video/audio-capable models | Fast experimentation across weird outputs | Model outputs can drift across updates | Choose |
| Local-only no providers | No PRD content leaves machine | Best privacy | Cannot satisfy generated video premise | Reject |
| Feed before render pipeline | Build swipe UI then attach rendering | Visually motivating | UI built on unknown latency/quality | Reject |
| Render gallery before feed | Generate PRD videos with provenance, then add gestures | Tests core media premise first | Less fun on day one than a feed | Choose first slice |

## Tradeoff Matrix
| Option | Fit | Size | Privacy | Agent-manageable | Reversible | Testable | Operating Burden |
|---|---:|---:|---:|---:|---:|---:|---:|
| Static cards first | 1 | 5 | 5 | 5 | 5 | 5 | 5 |
| Remotion-only meme renderer | 3 | 3 | 5 | 4 | 4 | 5 | 3 |
| HeyGen avatar first | 2 | 4 | 2 | 4 | 4 | 3 | 3 |
| Direct Veo/Runway first | 4 | 3 | 2 | 3 | 3 | 3 | 2 |
| fal multi-model first | 5 | 4 | 2 | 4 | 4 | 4 | 3 |
| Local-only no providers | 1 | 2 | 5 | 5 | 5 | 3 | 4 |
| Render gallery before feed | 5 | 4 | 2 | 5 | 5 | 5 | 4 |

Scoring favors outcome fit for the bit: generated MP4s with audio that are visibly not corporate. Privacy is lower for all remote video providers because PRD text and prompts leave the machine. `fal multi-model first` wins because it keeps provider exploration cheap and reversible. `Render gallery before feed` wins the first implementation slice because it proves the expensive media path before polishing gestures.

## Technical Design
- Chosen architecture: Local markdown backlog plus a provider-backed video artifact pipeline, followed by a local swipe/feed decision layer and bounded agent-run packet creation.
- Files/systems touched:
  - `backlog.d/*.md` for source PRDs
  - `.brainrot/index.json` for discovered PRDs and hashes
  - `.brainrot/storyboards/<prd_hash>.json` for generated storyboard/prompt bundles
  - `.brainrot/renders/<prd_hash>/<render_id>.json` and `.mp4` for render provenance and media
  - `.brainrot/events.ndjson` for decisions and render lifecycle events
  - `.brainrot/run-packets/<event_id>.json` for right-swipe agent intents
  - `src/` for the prototype app and provider adapters
- Data/control flow:
  1. `BacklogIndex` scans `backlog.d/*.md`, parses metadata and hashes content.
  2. `SpecDistiller` extracts user, goal, acceptance criteria, risks, ambiguities, and claims.
  3. `BrainrotStoryboardCompiler` emits a compact storyboard, prompt bundle, captions, tone tags, and prohibited claims.
  4. `VideoArtifactPipeline` submits the prompt bundle to a configured provider/model and waits for a native-audio MP4 when supported.
  5. `RenderStore` saves MP4 plus JSON provenance.
  6. `FeedReview` plays rendered videos and supports inspect, skip, needs-spec, and run-intent gestures.
  7. `AgentDispatch` consumes run packets later; MVP may stop at packet creation if PR automation is not yet configured.
- Build/check boundary:
  - Build fails on type/lint/test errors.
  - Render smoke fails if no playable MP4 artifact with provenance is produced for a sample PRD.
  - Decision tests fail if gestures do not append expected events.
  - Agent-run tests fail if right-swipe bypasses run-packet bounds.
- ADR decision: required before adding a persistent database, cloud sync, or direct Jira/Linear integration. Not required for the initial local file-backed prototype.
- Design X vs Y:
  - Provider-native audio vs mandatory TTS: choose provider-native audio by default; TTS only as explicit degraded/fallback path.
  - fal first vs one premium provider: choose fal first to test model fit; add direct Runway/Veo later if quality justifies lock-in.
  - Gallery first vs feed first: choose render gallery first for the first slice, then feed gestures.

## Data Model
```ts
type PrdSource = {
  id: string
  path: string
  sha256: string
  title: string
  discoveredAt: string
  status: 'new' | 'rendered' | 'skipped' | 'needs_spec' | 'run_intent_created'
}

type SpecBrief = {
  prdId: string
  goal: string
  user: string
  acceptanceCriteria: string[]
  ambiguityFlags: string[]
  riskNotes: string[]
  extractedClaims: string[]
}

type Storyboard = {
  id: string
  prdId: string
  briefHash: string
  tone: 'brainrot_v0'
  targetDurationSec: number
  aspectRatio: '9:16'
  beats: Array<{ label: string; specPayload: string; visualPrompt: string; caption: string }>
  providerPrompt: string
  prohibitedClaims: string[]
}

type VideoRender = {
  id: string
  prdId: string
  storyboardId: string
  provider: string
  model: string
  nativeAudioRequested: boolean
  audioMode: 'native' | 'silent' | 'fallback_tts' | 'failed'
  status: 'queued' | 'running' | 'ready' | 'failed'
  assetPath?: string
  providerJobId?: string
  costEstimateUsd?: number
  actualCostUsd?: number
  latencyMs?: number
  error?: string
}

type FeedDecision = {
  id: string
  prdId: string
  renderId: string
  decision: 'inspect' | 'skip' | 'needs_spec' | 'run_intent'
  createdAt: string
  note?: string
}

type AgentRunPacket = {
  id: string
  prdId: string
  prdSha256: string
  repoPath: string
  objective: string
  allowedCommands: string[]
  timeoutSec: number
  budgetUsd?: number
  branchName: string
  acceptanceCriteria: string[]
  status: 'created' | 'blocked' | 'launched' | 'completed' | 'failed'
}
```

## Agent Readiness
- Profile source: missing.
- Stack feedback strength: Use TypeScript for the browser/local prototype because the first slice is UI plus provider SDK integration; use strict TypeScript, unit tests, Playwright for the feed, and provider contract tests. This is a deliberate non-Rust exception for browser and SDK velocity.
- ADR decision: required if the prototype becomes durable infrastructure or if core state moves from JSON/NDJSON into a database.
- Infrastructure path: local app/service with explicit provider API keys; no implicit cloud sync.
- Gate: `npm test`, `npm run typecheck`, `npm run lint`, and a provider smoke command behind an env flag such as `RUN_PROVIDER_SMOKE=1`.
- Evidence storage: `.brainrot/` for local generated artifacts; `outputs/` for user-facing demo/spec artifacts.
- Mock policy impact: improved if fake provider contract tests are used for normal CI and exactly one opt-in real provider smoke is kept for integration proof.

## Delegation Evidence
- Roster providers used: Codex CLI as adversarial architecture critic; Pi CLI as product/oracle critic; previous Thinktank Pi/OpenRouter bench for video-pipeline critique.
- Native subagents used: none in this turn.
- Accepted evidence:
  - Make provider-native audio the default and move separate TTS out of the required MVP path.
  - Persist a real spec artifact because the workspace previously had no final MVP file.
  - Make artifact provenance and right-swipe run packets explicit module boundaries.
  - Treat fallback rendering as degraded, not as the product promise.
- Rejected evidence:
  - Pi suggested cutting everything except PRD to MP4 to swipe save/delete; useful for focus, but final MVP keeps inspect and needs-spec because they are core workflow gestures.
  - Codex suggested fallback TTS as an oracle requirement; final MVP records fallback support as degraded/optional, not required for V0 pass.
- Waivers:
  - Claude lane not retried in this pass because prior session hit account spend limits; Codex and Pi covered the fresh critic floor.
  - No formal delegation receipts were written because this scratch workspace has no harness receipt script.

## Premise Source
Premise Source Waiver: The premise is from the current chat thread and no safe repo-local transcript artifact exists yet.
Residual risk: Future implementers cannot independently verify exact wording unless the conversation is later redacted into a premise artifact.

## Exemplar Techniques
- None found in the scratch project root.

## Oracle (Definition of Done)
- [ ] `backlog.d/` contains at least five sample markdown PRDs.
- [ ] `npm test` passes unit tests for PRD parsing, storyboard generation, render provenance, decision logging, and run-packet creation.
- [ ] `npm run typecheck` passes.
- [ ] `npm run lint` passes.
- [ ] Fake provider test produces a deterministic MP4 fixture or fixture stub plus `VideoRender` JSON with `audioMode`, provider, model, cost, latency, PRD hash, and storyboard hash.
- [ ] Opt-in real provider smoke, `RUN_PROVIDER_SMOKE=1 npm run smoke:provider`, generates at least one playable 9:16 MP4 under 60 seconds from a sample PRD, with native audio requested when the selected model supports it.
- [ ] Render gallery shows PRD title, source hash, storyboard, provider/model, audio mode, cost/latency, and playable video.
- [ ] Feed UI supports play/pause, inspect source spec, skip, needs-spec, and run-intent gestures.
- [ ] Inspect opens the exact source PRD path and hash used for the render.
- [ ] Needs-spec appends a durable event and does not create an agent run packet.
- [ ] Skip appends a durable event and removes the item from the active feed.
- [ ] Run-intent appends a bounded `AgentRunPacket` and does not directly invoke an agent unless an explicit launch command is separately configured.

## Deliverable
- Output: A local prototype that reads `backlog.d/*.md`, generates native-audio shortform MP4 artifacts through a configured provider, displays a review gallery/feed, and records local decisions.
- Acceptance oracle: Commands above plus an inspected provider smoke artifact when credentials are available.
- Evidence artifacts:
  - `.brainrot/storyboards/*.json`
  - `.brainrot/renders/**/*.json`
  - `.brainrot/renders/**/*.mp4`
  - `.brainrot/events.ndjson`
  - `.brainrot/run-packets/*.json`
  - Playwright screenshot or local browser smoke for gallery/feed
- Residual risk:
  - Provider model quality, moderation behavior, pricing, and native audio behavior can drift.
  - Humor quality remains partly subjective; the oracle checks non-corporate generated video plus spec fidelity, not universal funniness.
  - Remote generation may expose private PRD content to providers.

## Observability Plan
- Changed behavior to watch: render success rate, native-audio success rate, render latency, cost per usable MP4, provider failures, decision distribution, right-swipe to accepted run outcomes.
- Named signal or evidence surface: `.brainrot/events.ndjson`, `VideoRender` JSON, provider smoke artifacts, run packets.
- Instrumentation debt if no signal exists: add a local summary command, for example `npm run brainrot:report`, before adding multi-provider ranking.

## Acceptance Evidence
- Acceptance source: five sample PRDs, fake provider fixture contract, one opt-in real provider smoke, rendered gallery/feed route.
- Evidence that proves it: command output for tests/typecheck/lint, generated `.brainrot` artifacts, playable MP4, and browser screenshot.
- Exact command/path/route exercised:
  - `npm test`
  - `npm run typecheck`
  - `npm run lint`
  - `RUN_PROVIDER_SMOKE=1 npm run smoke:provider`
  - local gallery/feed route, for example `http://localhost:5173`
- Oracle / acceptance artifact hash: to be recorded after implementation creates sample PRDs and provider fixtures.
- Contract-change acknowledgment: no acceptance contract exists yet; this packet establishes it.
- Residual risk: real provider smoke depends on credentials and current provider availability.

## Formal Spec
- Formal Spec Required: yes. Trigger criteria: cross-service provider contract, user-facing gesture behavior, expensive-to-detect regressions in dispatch safety, and multiple-agent/provider implementation risk.
- Informal spec: A PRD can become a goofy generated MP4, but the source PRD remains authoritative and all decisions are durable local events.
- Formal examples:
  - Fixture PRDs in `backlog.d/`
  - Fake provider response fixture
  - Generated `VideoRender` JSON golden file
  - Gesture decision event fixtures
  - `AgentRunPacket` fixture for run-intent
- Acceptance oracle: tests and smoke commands listed above.
- Hardening budget: 2 hours for provider contract tests, gesture event fixture tests, and one Playwright happy path; no mutation testing for MVP.
- Waiver path: If provider credentials are unavailable, skip only the real provider smoke and record `Provider smoke waiver: <reason>` plus fake provider evidence.

## Implementation Sequence
1. Create the local project skeleton, strict TypeScript config, test runner, and `backlog.d/` sample PRDs.
2. Implement `BacklogIndex` and PRD parsing with content hashes.
3. Implement `SpecDistiller` and `BrainrotStoryboardCompiler` with deterministic fake outputs for tests.
4. Implement provider abstraction and fake provider contract that writes fixture MP4/provenance.
5. Add fal provider adapter and opt-in real provider smoke for native-audio MP4 generation.
6. Build render gallery to inspect PRD, storyboard, provider metadata, and video.
7. Build swipe/feed UI with play/pause, inspect, skip, needs-spec, and run-intent controls.
8. Implement local event ledger and run-packet creation.
9. Add Playwright smoke for gallery/feed and verify no direct agent launch occurs from a raw gesture.

## Risk + Rollout
- Provider drift: keep model/provider in metadata and use a fake provider for stable CI.
- Cost runaway: require a configured per-render and per-session budget before real provider calls.
- Privacy leakage: display a remote-generation disclosure before first provider call; support local fake/dev mode.
- Misleading video: store prohibited claims and extracted claims in the storyboard; inspect always opens the source PRD.
- Unsafe execution: right-swipe creates a run packet only; actual agent launch is a separate explicit command.
- Rollback: delete `.brainrot/` generated artifacts and no source PRDs are changed.
