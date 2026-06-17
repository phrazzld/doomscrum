# DoomScrum — product vision

Status: working vision (groomed 2026-06-17). Companion to
`docs/COMMERCIAL_MODEL.md` (the staged commercial path). This file is the
product north star the backlog sequences against; COMMERCIAL_MODEL is *how* it
gets sold, this is *what it is and why it wins*.

## One line
Clear your backlog by swiping: DoomScrum turns a repo's specs into a shortform
video feed, and a swipe dispatches a real coding agent that opens a real PR.

## Audience & job-to-be-done
Primary: solo devs, indie hackers, and small-team leads whose backlog rots
faster than they triage it. The job: *"act on my backlog without sitting at a
terminal — turn intent into open PRs in minutes, from the couch."*

## Category
An **agent-dispatch surface for backlogs** — a backlog-to-PR pipeline with a
shortform-feed front end. Not a video toy; not a hosted SaaS (yet — see
COMMERCIAL_MODEL).

## Wedge / hook / moat (the load-bearing distinction)
- **Wedge — the value:** frictionless, one-gesture dispatch of a coding agent
  over a backlog. This is the part you can't replace with a terminal.
- **Hook — the retention mechanic:** brainrot video makes triage fun enough to
  do *daily*. The video is why you open it; the dispatch is why it matters.
  Render quality is a feature, not the product.
- **Moat — the durable asset:** the *closed loop*. Dispatched PRs flow back into
  the feed (merged / failed / needs-rework); receipts + vibe ratings teach which
  specs are agent-ready; and the swipe-left **shape** gesture upgrades specs
  before they're implemented. Spec quality is the #1 driver of agent-PR
  acceptance (industry evidence: human-refined specs cut LLM code errors ~50%),
  and DoomScrum is the only tool that makes spec-sharpening a first-class swipe.

## What excellent looks like in 6–12 months
"I open DoomScrum with coffee, swipe through last night's backlog — shape the
vague specs left, dispatch the sharp ones right — and arrive at standup with PRs
already open and pre-triaged." A stranger installs a binary, points it at a
repo, and reaches a playing video + their first (consented) dispatch in two
minutes, with no account.

## Non-goals (for now)
- Hosted multi-tenant SaaS (deferred — COMMERCIAL_MODEL step 3).
- Rationing / quotas / paywalls before strangers can reach the free wedge.
- Maximizing render fidelity ahead of dispatch trust and the closed loop.
- Re-introducing dispatch **bounds**. Dispatch volume and agent autonomy are
  unbounded by owner design. Wallet caps and dispatch **trust** (consent, undo,
  prompt/secret sandboxing) are *not* bounds and do not violate that.

## Strategy & sequence (why the backlog is ordered the way it is)
The product's entire sharp edge is the agent PR — so **trust and proof of the
dispatch loop outrank distribution of it.**

- **Gate 0 — Trustworthy dispatch (now):** the core loop must be safe to run
  live and proven live once: first-dispatch consent (034), untrusted-spec +
  secret-egress hardening (033), one real foreign-repo PR opened live (016
  child 3), mis-swipe undo (035). Demoing or marketing dispatch before it's safe
  and proven is selling vapor.
- **Gate 1 — The hook users judge first + the on-ramp:** render-quality verdict
  gate (031) + script eval (030), then guided first-run (019).
- **Gate 2 — Distribution & growth (only after Gate 0–1):** installable releases
  (017), marketing site (018), local open-weights renders (028), persona-QA
  dogfood (013), agent-PR triage (014).
- **Later / deferred:** swipe quotas & tiers (029); cloud render credits and the
  architecture spike (021).

## Anti-vision (how we'd know we got it wrong)
A low-quality-PR firehose: swipe-spam producing stale, duplicate, or abandoned
PRs that burn human review (industry baseline: ~46% of agent PRs rejected,
mostly for *relevance/abandonment*, not bad code). The feed must make a good
spec the easy path and triaging the result a glance — or the wedge becomes
noise.
