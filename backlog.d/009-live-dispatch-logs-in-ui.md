# Show live agent progress and failures in the feed

Priority: P1 · Status: ready · Estimate: M

## Goal
After a swipe, the operator sees what the agent is doing and exactly why it failed, without leaving the feed.

## Oracle
- [ ] Dispatch card exposes tail of agent log (poll or SSE) while status is agent_running.
- [ ] failed status shows the failing stage + last log lines + a retry affordance.
- [ ] Retry creates a fresh dispatch and is covered by an HTTP-level test.

## Notes
Receipts already persist stage-by-stage; this is surfacing, not new plumbing.
