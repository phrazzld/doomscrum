# DoomScrum commercial model decision

Status: ratified operating decision for the next public-demo/release slice.

## Decision

DoomScrum should ship first as a local-first BYO-keys tool with a free fixture
path, then add paid cloud-render credits only after onboarding and distribution
prove that strangers can reach value without help.

Do not start with a hosted multi-tenant SaaS. The product's sharp edge is that
swipes dispatch real coding agents against private repos; moving that into a
hosted control plane multiplies auth, wallet, queue, repo-permission, and data
handling risk before the core loop has earned it.

The staged path is:

1. Local-first free tool: fixture renders by default, local state, BYO fal key
   for paid media, user-owned agent and GitHub credentials.
2. Optional cloud-render credits: DoomScrum sells or brokers render credits
   while repo data and dispatch remain local.
3. Hosted multi-tenant product only if cloud credits create enough demand to
   justify centralized auth, queues, billing, and repo integration.

## Options

| Option | Wallet risk surface | Infra burden | Privacy story | Pricing sanity | Marketing/onboarding |
|---|---|---|---|---|---|
| Local-first BYO-keys free tool | Lowest for DoomScrum; operator owns fal/GitHub/agent spend. Wallet caps still protect the operator. | Small: binary, docs, CI, release packaging. No hosted queue or billing. | Strongest: specs stay local unless the operator explicitly uses fal; dispatch runs on the operator machine. | Free fixture path is $0. Paid renders use the operator's key; current premium COGS around $1.20/render is disclosed but not DoomScrum's margin problem. | Clear developer pitch: funny local tool, real PRs, no account required. Must disclose right-swipe consequences before first dispatch. |
| Paid cloud-render credits | Medium: DoomScrum owns render spend and must enforce prepaid balances and daily caps. Dispatch can stay local. | Moderate: payment, credit ledger, render queue, artifact storage, support for failed jobs. | Mixed: spec-derived prompts and generated media leave the machine for the render service; code/repo access can remain local. | Works only after cheaper pipelines land. $1.20/render is too high for casual free usage; stills/JIT/render-mix can make credits sane. | Easier nontechnical onboarding: no fal key. Marketing can sell "credits for brainrot backlog clips" while preserving local agent dispatch. |
| Hosted multi-tenant SaaS | Highest: DoomScrum owns render spend, dispatch abuse, PR credentials, rate limits, tenant isolation, and incident response. | Highest: auth, org/repo permissions, worker queues, storage, billing, webhooks, secrets, audit logs, support. | Weakest unless carefully scoped: specs, generated prompts, repo metadata, and possibly agent outputs live in DoomScrum infrastructure. | Needs tiering and quota discipline from day one. $1.20/render COGS plus support makes broad free usage irrational without JIT/stills/local-provider savings. | Cleanest sales motion, but onboarding copy must explain repo permissions, AI content movement, and real code-changing agents. Too much trust to ask before the product has pull. |
| Staged path | Starts low, adds risk only when a step pays for itself. | Incremental: release/distribution first, credits second, SaaS last. | Starts with the strongest privacy story and only weakens it behind explicit paid features. | Lets efficiency work prove unit economics before DoomScrum carries spend. | Best story now: "runs locally, makes real PRs; pay later only if you want hosted rendering convenience." |

## Implications

Wallet risk stays a product surface even in local-first mode. The free default
must remain fixture or local/offline, paid renders must quote cost before
starting, and spend must be visible enough that an operator can stop before
surprise charges.

Privacy copy must be blunt. Specs never leave the machine for fixture renders.
Specs do leave the machine when using fal or any future cloud-render credit.
Right/left swipes run real agent commands against real repos and can open real
pull requests.

Distribution work should optimize for "two minutes to local value": install a
binary, point it at a repo, render fixture videos, tap the exact source spec,
and understand the first dispatch warning without configuring billing.

Onboarding should not ask for a FAL key up front. The first successful path is
fixture video plus repo sync. Paid rendering is an upgrade action once the user
has seen the feed and trusts what will be sent out.

The cloud architecture spike remains blocked until this staged path needs step
2. When it does, the first hosted system is a render-credit service, not a
hosted agent-dispatch service.

## First reversible step

Ship the local-first release path:

- Build installable binaries and a README quickstart around the free fixture
  flow.
- Add first-run onboarding that picks a repo, renders fixtures, and discloses
  paid-render and dispatch consequences before either can run.
- Keep cloud work to a design note until someone has used the local release
  enough to ask for managed render credits.

This step is reversible: if demand points toward hosted credits, the local app
already has the right boundary. Cloud can provide render jobs while the local
binary keeps repo access, agent commands, dispatch receipts, and Git remotes on
the operator machine.
