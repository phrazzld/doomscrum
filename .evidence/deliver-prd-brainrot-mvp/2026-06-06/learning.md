# Learning Packet

- Keep video generation as a single provider contract. The provider is responsible for audio when the model supports it; separate TTS is only a degraded fallback.
- Treat `.brainrot` as the audit boundary: storyboard JSON, render provenance, user decisions, and run packets all land there.
- The real provider adapter should stay behind explicit configuration. The UI warns that remote provider calls require config, and `smoke:provider` requires `FAL_KEY`.
- The useful action boundary is run-packet creation, not immediate unconstrained execution. The packet carries PRD hash, render id, timeout, and allowed commands.
- The local fake provider is useful for repeatable QA but is not a substitute for a real slop-quality provider smoke once credentials and budget are available.

