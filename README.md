# Specifi AI

Local desktop prototype for triaging PRD-shaped agent work as shortform video.

Specifi AI reads markdown PRDs from `backlog.d/`, turns them into goofy vertical
AI videos, and lets you inspect, skip, mark needs-spec, or launch a bounded
local Codex run.

## Quick Start

```bash
npm install
npm run build
npm run brainrot:generate
npm run serve
```

Open `http://127.0.0.1:4173`.

## Real Video

Put a FAL key in your shell environment or `~/.secrets`:

```bash
export FAL_API_KEY=...
```

Then use `Generate AI video` in the app, or run a one-item smoke:

```bash
npm run build:server
node build-server/generate.js --real-provider --limit=1
```

Remote video generation sends PRD-derived prompts to the provider. Treat it as
an explicit disclosure event.

## Backlog

Each markdown file in `backlog.d/` is one PRD. Runtime artifacts are written to
the ignored `.brainrot/` directory:

- `storyboards/` for extracted shortform beats and provider prompts.
- `renders/` for MP4 files and render provenance JSON.
- `events.ndjson` for inspect, skip, needs-spec, and run-intent decisions.
- `run-packets/` for bounded agent intents.
- `launches/` for local Codex launch receipts and logs.

## Backlog Config

`backlog.config.json` binds PRDs to execution targets:

```json
{
  "defaults": {
    "repoPath": ".",
    "allowedCommands": ["npm test", "npm run typecheck", "npm run lint"],
    "agentMode": "local-codex",
    "renderProvider": "fal",
    "maxRenderSpendUsd": 20
  },
  "items": {
    "001-cache-chaos-exorcism.md": {
      "repoPath": "."
    }
  }
}
```

Right-swipe launches local Codex when `agentMode` is `local-codex`. Use
`BRAINROT_AGENT_LAUNCH_MODE=dry-run` for tests or demos that should not spawn a
real agent.

## Verification

```bash
npm run build
BRAINROT_AGENT_LAUNCH_MODE=dry-run npm test
npm run typecheck
npm run lint
BRAINROT_AGENT_LAUNCH_MODE=dry-run npm run test:e2e
npm run brainrot:report
```

## Docs

- [MVP spec](docs/final-mvp-spec.md)
