# Run Packet Seatbelt

## User
Local operator who right-swipes a PRD into agent work.

## Problem
Right-swipe is too powerful if it launches an agent directly.

## Goal
Create a bounded run packet that records repo path, objective, timeout, branch name, and acceptance criteria before any agent can run.

## Acceptance Criteria
- Right-swipe writes a run packet JSON file.
- The packet includes the source PRD hash.
- The app never launches an agent from the raw gesture.

## Risk
Users may think nothing happened if packet creation has weak UI feedback.
