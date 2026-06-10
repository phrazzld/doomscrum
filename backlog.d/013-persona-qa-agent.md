# Autonomous persona QA agent that files tickets

Priority: P1 · Status: ready · Estimate: L

## Goal
A scheduled agent walks the running app as a skeptical first-time user and files structured tickets into backlog.d when it finds friction or breakage.

## Oracle
- [ ] Repo-local QA skill (via /create-repo-skill qa) defines the persona walk: launch, generate, swipe all gestures, read spec, check statuses, attempt a dispatch with stub agent.
- [ ] Findings land as backlog.d tickets with Goal/Oracle, severity, and evidence (screenshot/log path) — and never duplicate an open ticket for the same finding.
- [ ] One scheduled run (cron/launchd) wired and documented; its first real finding is linked in the ticket that closes this.

## Notes
The dogfood loop: the product that turns specs into agent work should generate its own specs from QA.
