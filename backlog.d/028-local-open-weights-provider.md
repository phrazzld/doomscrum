# Local open-weights video provider (electricity-priced renders)

Priority: P1 · Status: ready · Estimate: L

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
Research 2026-06-10 (two passes). Model roster per modality, all open
weights / commercially usable:
- **Video:** Wan 2.2 (Apache 2.0, best quality; 14B FP8 needs ~22-26GB →
  RTX 4090/5090; 5B TI2V runs FP8 on 12-15GB; GGUF Q4 squeezes 14B onto
  12GB with CPU offload). LTX-2.3 (fastest; FP8 wants 20GB+, GGUF fits
  8-16GB; full 22B does 4K@50fps w/ native audio but wants 24GB+ and
  ~44GB weights). HunyuanVideo = cinematic but heavy. Only LTX-2.x has
  native audio; Wan clips need TTS compositing (pairs with ticket 026's
  caption/TTS stack).
- **Image (for stills pipeline):** FLUX.2 [klein] 4B runs in ~13GB
  sub-second; FLUX.2 [dev] 32B is the open quality leader; Qwen-Image
  strong at text rendering (useful for on-screen captions); SDXL still the
  LoRA ecosystem king at 10-12GB.
- **TTS:** Kokoro 82M (Apache 2.0, near-API quality, runs anywhere incl.
  Apple Silicon), Qwen3-TTS for long-form, Chatterbox for emotional reads.
Hardware paths:
- **Buy NVIDIA:** one RTX 5090 (32GB) covers Wan 14B FP8 + FLUX.2 dev
  quantized + everything else; a used 4090 (24GB) covers all but the
  largest. Wan 2.1 I2V 720p ≈ 4-9 min/5s clip on a 4090 (SageAttention +
  TeaCache halves it); fine for overnight batch, not interactive.
- **Mac Studio / Mini daisy-chain:** great for LLMs (unified memory), the
  WRONG tool for video diffusion — diffusion is compute-bound, not
  memory-bound; MPS/MLX video pipelines are immature and multi-node
  clustering (exo-style) doesn't shard diffusion well. Macs DO earn a spot
  for TTS (Kokoro) and image gen at small scale.
- **Rent:** fal serverless H100 $1.89/hr or RunPod/Vast 4090s ($0.30-0.60/hr)
  for overnight batch ≈ $0.02-0.08/clip with zero capex — the right first
  step before buying anything.
Recommended sequence: rent first (prove the ComfyUI/endpoint provider
works), buy a single 5090 box only if sustained volume justifies it; skip
the Mac cluster for video. Audience is developers — many own the hardware.
**Why:** owner picked this lane explicitly in the 2026-06-10 efficiency
session and asked for the buy-vs-cluster-vs-rent tradeoff on 2026-06-10.
