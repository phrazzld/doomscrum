# Needs Spec Confessional

## User
Operator triaging unclear PRDs.

## Problem
Underspecified PRDs should not disappear into the feed or accidentally start agent work.

## Goal
Make the needs-spec gesture record generated clarification questions and block run-intent until explicitly overridden.

## Acceptance Criteria
- Needs-spec appends a durable event.
- Active feed status changes to needs-spec.
- Run-intent is blocked unless an override flag is present.

## Risk
Overblocking could slow down toy usage, but silent unsafe execution is worse.
