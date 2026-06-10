# Local open-weights video provider (electricity-priced renders)

Priority: P2 · Status: ready · Estimate: L

## Goal
`provider = "local"` renders feed clips on the operator's own GPU via
open-weight models (LTX-2 distilled / Wan), making the wallet cap
irrelevant for self-hosters.

## Oracle
- [ ] One feed clip rendered end-to-end on local hardware (ComfyUI or
      equivalent endpoint) with cost_estimate_usd = 0 in provenance.
- [ ] The clip passes scripts/check_script_fit.py when the local model
      supports audio, or is composed with TTS audio when it does not.
- [ ] Falls back to the configured fal mix with a clear message when no
      local endpoint is reachable.

## Notes
Research 2026-06-10: LTX-2 19B distilled FP8 runs on 16GB consumer VRAM;
Wan 2.1 1.3B does 5s/720p in ~45s on a 4090 (14B needs a 5090 and
minutes). Audience is developers — many own the hardware. Alternate lane:
fal serverless H100 at $1.89/hr for overnight batch queues (~$0.02-0.05
per clip) without owning a GPU. **Why:** owner picked this lane explicitly
in the 2026-06-10 efficiency session.
