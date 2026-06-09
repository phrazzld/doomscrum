# Real Provider and Local Codex Slice Receipt

## Scope

- Loaded `FAL_API_KEY` from `~/.secrets` without printing the secret.
- Added `backlog.config.json` so PRDs can be tied to target repositories and launch policy.
- Added a real FAL render endpoint and UI action.
- Downloaded provider MP4s into local `.brainrot/renders`.
- Added a local Codex worker adapter and launch receipts.
- Kept dry-run launch mode for automated tests.

## Verification

```text
node build-server/generate.js --real-provider --limit=1
provider=fal
model=fal-ai/veo3.1/fast
count=1
audioMode=native
latencyMs=79077

ffprobe real FAL MP4
video: h264, 720x1280, 24fps, duration 8.000000
audio: aac, stereo, duration 8.000000

npm run build
passed

BRAINROT_AGENT_LAUNCH_MODE=dry-run npm test
passed

npm run typecheck
passed

npm run lint
passed

BRAINROT_AGENT_LAUNCH_MODE=dry-run npm run test:e2e
2 passed: chromium and mobile-chromium

npm run brainrot:report
prds=5; storyboards=5; renders=6; ready=6; nativeAudio=6
```

## Artifacts

- Real render MP4: `.brainrot/renders/a86401120db2e5ae84b956cf49670e23a62ccf95932e3642f4bd02b9188d170a/fe39832ce6b7d0f28624a4cae0d76eec545994fe7e898174ba90db0f50367da4.mp4`
- Real render provenance: `.brainrot/renders/a86401120db2e5ae84b956cf49670e23a62ccf95932e3642f4bd02b9188d170a/fe39832ce6b7d0f28624a4cae0d76eec545994fe7e898174ba90db0f50367da4.json`
- Screenshot: `outputs/app-real-fal.png`

## Notes

- The app now prefers real provider renders over QA fixture renders for the selected PRD.
- Right-swipe launches local Codex unless `BRAINROT_AGENT_LAUNCH_MODE=dry-run` is set or the item is configured as `packet-only`.
- Remote/cloud worker integration remains isolated behind the launch adapter boundary.

