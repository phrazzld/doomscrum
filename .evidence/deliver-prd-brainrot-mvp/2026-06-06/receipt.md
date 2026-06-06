# PRD Brainrot Swipe MVP Receipt

## Scope

- Local `backlog.d/*.md` PRD reader with five sample PRDs.
- Storyboard distillation into hook/stake/payload/risk/decision beats.
- Shortform MP4 generation pipeline with a fake local provider and a FAL Veo 3.1 queue adapter.
- Swipe-style web UI for inspect, needs-spec, skip, and bounded run-packet creation.
- Append-only `.brainrot` evidence for storyboards, renders, decisions, and run packets.

## Verification

All commands were run from `/Users/phaedrus/Documents/Codex/2026-06-05/hey-can-we-prototype-an-app` on 2026-06-06.

```text
npm install
added 3 packages, removed 77 packages, audited 264 packages; 0 vulnerabilities

npm run build
vite production build passed; server, generate, and report bundles emitted

npm run brainrot:generate
provider=fake-local; count=5; all renders audioMode=native

npm test
acceptance tests passed

npm run typecheck
passed

npm run lint
passed

npm run test:e2e
2 passed: chromium and mobile-chromium

npm run brainrot:report
prds=5; storyboards=5; renders=5; ready=5; nativeAudio=5; runPackets=3
decisions: inspect=10, needs_spec=3, run_intent=3
```

## Artifacts

- MVP spec: `outputs/final-mvp-spec.md`
- Desktop screenshot: `outputs/app-desktop.png`
- Mobile screenshot: `outputs/app-mobile.png`
- Storyboards: `.brainrot/storyboards/*.json`
- Render provenance and MP4s: `.brainrot/renders/*/*`
- Run packets: `.brainrot/run-packets/*.json`
- Event log: `.brainrot/events.ndjson`

## Hashes

```text
7a7ad9a64833215a9324a06c2ed0d87814850f704d2a006a16c1a2cc14064323  outputs/final-mvp-spec.md
1967e5cbfd82fd03e70a187c6d1d3df77f72d1f1fc66f926953ded4fde71d299  outputs/app-desktop.png
9236c78e8ff63179509bad3df119bdf210ac3802a2dc2b3f85db158c6542f454  outputs/app-mobile.png
1372ce8d8c0c20f84acd2dd577cbfccbc5a599bfb79b63f3d27b372c9db4699d  package.json
f42e6a2ef29b2465de664853a251ee06df101e2114692a6e0adf2ff33012410a  package-lock.json
```

## Known Limits

- The default renderer is intentionally a local fake MP4 provider so the app can be verified without spending provider credits.
- The real provider path is implemented as a FAL queue adapter, but it was not smoke-tested because `FAL_KEY` was not present.
- Right swipe creates a bounded run packet; it does not launch a live coding agent yet.
- Swipe gestures are represented by explicit action buttons in the MVP surface.

