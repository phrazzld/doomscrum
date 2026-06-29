# DoomScrum — Vision

**Backlog triage as a TikTok feed that actually ships code.**

DoomScrum reads the markdown specs rotting in a repo's backlog, turns each one
into a goofy shortform video, and lets you swipe. Swipe right and a real coding
agent spins up a fresh worktree, implements the spec, and opens a real pull
request. The intended default for swipe left is skip: keep the feed moving
without mutating the source spec. The doomscroll is the hook. The PR is the
point.

This is the canonical north star — what we're building, why, and what kind of
excellence is non-negotiable. The backlog sequences against this file. The agent
contract (`AGENTS.md`) and the strategy companions in `docs/` point here; when a
direction call is ambiguous, this file decides.

## What it is

An **agent-dispatch surface for backlogs** — a backlog-to-PR pipeline wearing a
shortform-feed front end. Not a video toy. Not (yet, and maybe never) a hosted
SaaS. The category is the dispatch, not the brainrot.

It is three things at once, and refuses to pick:

- **A product other people run** on their own repos and backlogs.
- **A proof of taste** — what model-native, agentic product design looks like
  when someone actually cares about the craft.
- **A provocation** — a straight-faced argument that brainrot-as-interface and
  real autonomous shipping belong in the same sentence.

The blend is the identity. Strip out any one of the three and it gets worse.

## The load-bearing magic

**It actually ships code.** Right and left swipes launch real agents that modify
real code and open real PRs against a real remote. There is no sandbox theater
beyond what the agent CLI provides, and that realness is the entire reason the
joke lands. The brainrot is the trojan horse that gets you to pull a trigger
with genuine consequences.

If the dispatch ever became fake — a convincing animation of work that doesn't
happen — DoomScrum would be dead, no matter how good the videos looked.

## Who it's for

Solo devs, indie hackers, and small-team leads whose backlog rots faster than
they triage it. The job-to-be-done: *"act on my backlog without sitting at a
terminal — turn intent into open PRs in minutes, from the couch."* The perfected
version is *easy to start*: point it at your repo and, within a couple of
minutes, you're swiping your own specs into PRs — no account required for the
free path.

## Wedge / hook / moat (the load-bearing distinction)

- **Wedge — the value:** frictionless, one-gesture dispatch of a coding agent
  over a backlog. This is the part you can't replace with a terminal.
- **Hook — the retention mechanic:** brainrot video makes triage fun enough to
  do *daily*. The video is why you open it; the dispatch is why it matters.
  Render quality is a feature, not the product.
- **Moat — the durable asset:** the *closed loop*. Dispatched PRs flow back into
  the feed (merged / failed / needs-rework); receipts and vibe ratings teach
  which specs are agent-ready; and skipped specs can be revisited, shaped, or
  filtered without breaking the feed. Spec quality is the #1 driver of agent-PR
  acceptance, so shaping remains an important supporting workflow, just not the
  default meaning of a left swipe.

## What excellent looks like

- **The first swipe is a held breath.** A new operator's first implement-swipe
  should feel slightly dangerous and then delightful: the confirmation names
  exactly what's about to happen, you tap *Cook it*, and a PR appears.
- **The 6–12 month picture:** *"I open DoomScrum with coffee, swipe through last
  night's backlog — shape the vague specs left, dispatch the sharp ones right —
  and arrive at standup with PRs already open and pre-triaged."* A stranger
  reaches a playing video and their first consented dispatch in two minutes, with
  no account.
- **The brainrot communicates the spec.** Every format quotes the spec's actual
  goal and first acceptance criterion. Unhinged *and* informative — never invents
  features or claims something shipped that didn't. Funny-that-lies is failure.
- **Backlog portability is real.** A new repo and backlog should not require a
  custom parser rewrite. Adapters may vary, but DoomScrum wins only if arbitrary
  local repos and their shaped-work formats can become feed cards.
- **Consequences are legible.** Live status stickers, durable receipts, links to
  the real PR, undo for a mis-swipe. You always know what your thumb just did.
- **The spec stays sacred.** Source specs are authoritative and never mutated by
  generated state; deleting every generated artifact destroys nothing real.
  Every render and decision is provenance-stamped and auditable.
- **The craft is real.** One Rust crate, one HTML surface, deep modules with
  small interfaces — good enough that the project survives being taken seriously,
  because it is also a portfolio piece.

## Non-negotiables (the soul)

- **The brainrot is load-bearing, not decoration.** No sanding the absurd formats
  into tasteful corporate motion. The unhinged fruit telenovela is a feature.
- **Real consequences, by consent, unbounded.** Dispatch is gated by one human
  consent moment per repo, then it is unbounded by design. The wallet protects
  spend; nothing else caps your thumb. We do not re-introduce run-packet
  "bounds" — wallet caps and dispatch *trust* (consent, undo) are *not* bounds.
  (Agent sandboxing is deferred under the local single-operator model — see
  Operating assumption.)
- **Disclosure is honest.** Real renders cost money and send spec-derived text to
  a third party; we say so, quote the cost first, and enforce hard spend caps. No
  surprise bills, no quiet egress.

## Operating assumption (v1)

DoomScrum today is a **local, single-operator tool**: you run it on your own
machine, point it at *your* repo and *your* backlog, using *your* OpenRouter /
GitHub / FAL credentials, against specs *you* wrote. Under that model the spec is
not an adversary and the credentials are already yours — so agent sandboxing and
secret-egress defense are **premature**. We do not harden against untrusted specs
until DoomScrum runs *other people's* specs (shared / multi-tenant / SaaS). Don't
gold-plate security before the product is proven and interesting.

## Strategy & sequence (why the backlog is ordered the way it is)

The product's entire sharp edge is the agent PR — so **proof that the loop
actually works outranks both distribution and premature hardening.** The order is
make-it-work → make-it-good → make-it-spread → (only if it goes multi-tenant)
make-it-safe-for-strangers.

- **Gate 0 — Make it work (now):** the loop must *actually run against an
  arbitrary repo and open a real PR* — a real OpenRouter-backed coding agent (not
  a stub), an onboarding/config path with preflight sanity checks, and the
  keystone: **one real PR opened live against an external repo.** Config-heavy is
  fine. Consent + undo stay (cheap and honest); the agent *sandbox* does not
  (deferred — see Operating assumption).
- **Gate 1 — The hook users judge first + the on-ramp:** render-quality verdict
  gate and script evals (the videos are what users judge first), then guided
  first-run — stranger to playing video in 60 seconds, no key required.
- **Gate 2 — Distribution & growth (only after 0–1):** installable releases, a
  marketing site, local open-weights renders, self-dogfooding persona-QA, and
  agent-PR triage that surfaces PR state back on the feed card.
- **Gate 3 — Trust for strangers (deferred until multi-tenant):** agent
  filesystem/network sandboxing and untrusted-spec defense-in-depth. Earns its
  place only when DoomScrum runs specs that aren't the operator's own.

## Failure modes (how we'd know we got it wrong)

- **The low-quality-PR firehose.** Swipe-spam producing stale, duplicate, or
  abandoned PRs that burn human review. (Industry baseline: ~46% of agent PRs
  rejected, mostly for relevance/abandonment, not bad code.) The feed must make a
  good spec the easy path and triaging the result a glance — or the wedge becomes
  noise.
- **Demo-ware.** Looking like it ships code without really opening PRs. The
  realness is the whole bet.
- **Sanitized UX.** Filing the edges off the brainrot until it's safe and
  forgettable.
- **Generic and interchangeable.** "Jira with a chatbot." It can become a
  product — even a paid one — but the day it could be swapped for three other AI
  PM tools without anyone noticing, the soul is gone. Commercialize the costs,
  never the character.

## Open bets (named, not decided)

These are deliberately undecided. Named here so they get *chosen*, not drifted
into:

- **How it's distributed and sold — reopened.** Local-first-free is real today,
  but the path beyond it (OSS self-host · BYO-key · cloud render *credits* ·
  hosted SaaS) is a live question, not a settled plan. `docs/COMMERCIAL_MODEL.md`
  holds the options analysis and is **under review** — it is no longer a ratified
  decision. The one hard constraint: *trivial to start on your own backlog,* and
  no rationing before strangers can reach the free wedge.
- **Where it ultimately runs** — a perfected single-operator local tool vs. a
  cloud, multi-repo product. The local CLI is the seed; whether it stays the
  whole thing done beautifully or grows a hosted feed is open.
- **How far autonomy goes** — today one swipe is one dispatch. Whether DoomScrum
  grows toward continuous or queued autonomy is open; the consent-and-wallet
  model is the floor any answer must respect.
- **Swipe-left alignment debt** — current implementation still contains
  shape-agent behavior around left swipe. The product direction is skip-first;
  any shaping action should become explicit enough that it is not confused with
  "move to the next ticket."

## Companions

- `docs/COMMERCIAL_MODEL.md` — distribution & pricing options *(under review; the
  staged path is no longer ratified — see Open bets)*.
- `docs/EFFICIENCY.md` — the cost north-star: drive $/clip from ~$1.20 flat
  toward ~$0.05 (stills) and ~$0 (local GPU) via a stacking strategy ladder.
- `docs/VIDEO_QUALITY_PIPELINE.md` — how renders become bespoke-per-spec and
  caption-correct.
