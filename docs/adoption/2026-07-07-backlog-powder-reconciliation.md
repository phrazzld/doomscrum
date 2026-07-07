# backlog.d -> Powder reconciliation (doomscrum-950)

Captured 2026-07-07 16:02:36 CDT / 2026-07-07 21:02:36 UTC.

## Contract

Powder is the durable work ledger for DoomScrum. `backlog.d/` is now only an
import seed: active markdown files should either already be represented by a
Powder card, create a new Powder card, or be archived as stale/self-resolved.

## Counts

Source commands:

- Board: `GET $POWDER_API_BASE_URL/api/v1/cards?repo=doomscrum&limit=200`
- Disk: `find backlog.d -maxdepth 1 -type f -name '*.md' ...`

| Surface | Before | After |
| --- | ---: | ---: |
| Powder cards for `repo=doomscrum` | 18 | 31 |
| Powder status: ready | 3 | 11 |
| Powder status: backlog | 4 | 4 |
| Powder status: blocked | 0 | 5 |
| Powder status: claimed | 1 | 1 |
| Powder status: done | 10 | 10 |
| Active disk specs, non-underscore top level | 29 | 0 |
| Top-level disk markdown files, including ignored `_*.md` | 32 | 0 |
| Archived disk specs in `backlog.d/_done/` | 17 | 49 |

## New Powder Cards

These files still carried live or intentionally deferred scope not fully covered
by an existing board card, so new Powder cards were created and the source seed
was archived.

| Source seed | Powder card | Rationale |
| --- | --- | --- |
| `backlog.d/011-ci-pipeline.md` | `doomscrum-011` | CI workflow shipped, but required-check enforcement is still blocked by repo visibility/account plan. |
| `backlog.d/013-persona-qa-agent.md` | `doomscrum-013` | Persona QA remains a live Gate 2 dogfood loop; Powder cards replace `backlog.d` tickets as the output sink. |
| `backlog.d/014-agent-pr-triage.md` | `doomscrum-014` | `doomscrum-944` absorbs live PR state; the auto-review/comment and stale-queue half remains separate. |
| `backlog.d/016-multi-repo-sync.md` | `doomscrum-016` | Gate 0 proof is covered by `doomscrum-940`; residual portability scope remains around contract docs/degradation and Habitat/Apollo source. |
| `backlog.d/021-cloud-architecture-spike.md` | `doomscrum-021` | Still a valid future spike, blocked until the commercial model decision is resolved. |
| `backlog.d/026-deterministic-audio-spike.md` | `doomscrum-026` | Stills plus TTS pipeline remains a live cheap-render strategy. Note: the created title shell-expanded `$0`; a correction comment is attached because PATCH requires admin scope. |
| `backlog.d/028-local-open-weights-provider.md` | `doomscrum-028` | Local/open-weights rendering remains a live local-first cost lane. |
| `backlog.d/029-swipe-quota-tiers.md` | `doomscrum-029` | Pricing/quota work remains intentionally blocked until the commercial model is decided; it is not a dispatch-bound reintroduction. |
| `backlog.d/030-script-eval-harness.md` | `doomscrum-030` | Standing script evals remain live Gate 1 quality infrastructure. |
| `backlog.d/031-render-verdict-gate.md` | `doomscrum-031` | Render verification remains live Gate 1 quality infrastructure. |
| `backlog.d/039-agent-filesystem-egress-sandbox.md` | `doomscrum-039` | Deferred Gate 3 security work remains real, but blocked under the current local single-operator assumption. |
| `backlog.d/041-commercial-model-decision.md` | `doomscrum-041` | Owner decision remains the explicit blocker for cloud architecture and quota/pricing. |
| `backlog.d/046-meme-product-greatness-audit.md` | `doomscrum-046` | Product judgment on whether the brainrot joke lands remains live and feeds the eval/render cards. |

## Absorbed By Existing Powder Cards

These seeds were archived because their scope is already represented by the
named board card.

| Source seed | Powder card | Rationale |
| --- | --- | --- |
| `backlog.d/017-distribution.md` | `doomscrum-903` | Installable releases, Homebrew path, and timed install audit are absorbed and done. |
| `backlog.d/018-marketing-site.md` | `doomscrum-902` | One-page marketing site and go-public launch are absorbed by the launch epic. |
| `backlog.d/019-onboarding-first-run.md` | `doomscrum-942` | The file already marked itself absorbed by the first-run app card. |
| `backlog.d/022-legal-safety-baseline.md` | `doomscrum-902` | Remaining trademark/provider terms work is launch-gating legal scope in the go-public epic. |
| `backlog.d/023-cost-observability.md` | `doomscrum-931` | Durable cost ledger/reporting is absorbed by the runtime reliability epic. |
| `backlog.d/032-jit-render-lifecycle-followups.md` | `doomscrum-931` | Crash recovery and render/dispatch self-healing are absorbed by the runtime reliability epic. |
| `backlog.d/036-agent-contract-truth.md` | `doomscrum-930` | Profile docs and stuck-dispatch recovery runbook are absorbed and done. |
| `backlog.d/037-extract-render-module.md` | `doomscrum-932` | The render module work is done; the remaining server route split is carried by `doomscrum-932` via `044`. |
| `backlog.d/038-purge-orphaned-artifacts.md` | `doomscrum-932` | Deletion ratification/proposals are carried by the code hygiene epic. |
| `backlog.d/040-close-the-loop-spec-readiness.md` | `doomscrum-944` | Readiness score and live PR state are absorbed by the close-the-loop product card. |
| `backlog.d/042-demo-tooling-security-scope.md` | `doomscrum-932` | Demo dependency patch and non-product boundary note are absorbed by the code hygiene epic. |
| `backlog.d/043-mvp-real-dispatch-arbitrary-repos.md` | `doomscrum-940` | Gate 0 external dispatch proof is absorbed and done. |
| `backlog.d/044-decompose-server-routes.md` | `doomscrum-932` | Server route decomposition is carried by the code hygiene epic. |
| `backlog.d/045-adopt-misty-step-comic-ops-aesthetic.md` | `doomscrum-901` | The live board now carries the narrower current decision: repoint away from the dead comic-ops pin or record an exception. |
| `backlog.d/047-byok-local-run-install-friction.md` | `doomscrum-903`, `doomscrum-onramp-timing-guards` | Timed install audit is done in `903`; regression guards remain as the timing-guards card. |

## Stale Or Self-Resolved Archives

These seeds no longer represent live board work.

| Source seed | Disposition | Rationale |
| --- | --- | --- |
| `backlog.d/_002-run-packet-seatbelt.md` | stale -> archive | Old bounded run-packet framing conflicts with the current unbounded dispatch model and consent/undo approach. |
| `backlog.d/_004-needs-spec-confessional.md` | stale -> archive | Left swipe is skip-first by contract; shaping remains explicit backend/control action, not the default left gesture. |
| `backlog.d/_025-draft-tier-renders.md` | stale -> archive | The file's own groom note says render profiles (`dev` vs `content`) supersede the draft-tier ticket. |
| `backlog.d/048-weave-onboarding-lite.md` | self-resolved -> archive | The ticket is an applicability table that already concludes no new five-faces scaffolding is warranted; current residual five-faces work is on `doomscrum-905`. |

## Verification

- Created 13 Powder cards via `POST /api/v1/cards`.
- Moved 32 import-seed markdown files into `backlog.d/_done/` via `git mv`.
- Re-read Powder with `GET /api/v1/cards?repo=doomscrum&limit=200`: 31 cards.
- Re-read disk with `find backlog.d -maxdepth 1 -type f -name '*.md'`: 0 top-level markdown seeds.
