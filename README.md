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

For a real provider smoke:

```bash
FAL_KEY=... FAL_VIDEO_MODEL=fal-ai/veo3.1/fast npm run smoke:provider
```

Remote provider calls send PRD-derived prompts outside the machine and should be treated as explicit disclosure events.

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

