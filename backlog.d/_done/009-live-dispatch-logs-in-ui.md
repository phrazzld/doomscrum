# Show live agent progress and failures in the feed

Priority: P1 · Status: shipped · Estimate: M

## Goal
After a swipe, the operator sees what the agent is doing and exactly why it failed, without leaving the feed.

## Oracle
- [x] Dispatch card exposes tail of agent log (poll or SSE) while status is agent_running.
- [x] failed status shows the failing stage + last log lines + a retry affordance.
- [x] Retry creates a fresh dispatch and is covered by an HTTP-level test.

## Notes
Receipts already persist stage-by-stage; this is surfacing, not new plumbing.
