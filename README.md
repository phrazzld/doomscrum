# PRD Brainrot Swipe

Local-first prototype for triaging PRD-shaped agent work as shortform video.

## Run

```bash
npm install
npm run build
npm run brainrot:generate
npm run serve
```

Open `http://127.0.0.1:4173`.

## Backlog

Each markdown file in `backlog.d/` is one PRD. Generated state is written under `.brainrot/`:

- `storyboards/` contains extracted shortform beats and provider prompts.
- `renders/` contains MP4 files and render provenance JSON.
- `events.ndjson` contains inspect, skip, needs-spec, and run-intent decisions.
- `run-packets/` contains bounded agent run intents.

## Provider Modes

The default provider is `fake-local`, which uses `ffmpeg` to create repeatable MP4 fixtures with native-audio metadata for local QA.

For real AI video, add a FAL key to your environment or `~/.secrets`:

```bash
export FAL_API_KEY=...
```

Then use the app's `Generate AI video` button, or run a one-item smoke:

```bash
node build-server/generate.js --real-provider --limit=1
```

Remote provider calls send PRD-derived prompts outside the machine and should be treated as explicit disclosure events.

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

Right-swipe creates a run packet and launches local Codex when `agentMode` is `local-codex`. Use `BRAINROT_AGENT_LAUNCH_MODE=dry-run` for test runs that should not start a real agent.

## Verification

```bash
npm run build
npm run brainrot:generate
npm test
npm run typecheck
npm run lint
npm run test:e2e
npm run brainrot:report
```
