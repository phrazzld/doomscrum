# Script bench 20260716T172708Z

120 generations · models=15 prompts=4 specs=4 · judges: google/gemini-3.1-pro-preview, x-ai/grok-4.3

## Leaderboard (mean overall, both judges, all specs)

| model | prompt | mean overall | fidelity | clarity | energy | speak | budget viol |
|---|---|---|---|---|---|---|---|
| anthropic/claude-fable-5 | p4-plain-open | **8.38** | 8.5 | 8.8 | 6.9 | 8.9 | 0/4 |
| openai/gpt-5.6-luna | p4-plain-open | **7.88** | 8.5 | 8.5 | 4.2 | 7.2 | 0/4 |
| openai/gpt-5.6-sol | p4-plain-open | **7.62** | 8.6 | 8.1 | 5.6 | 7.0 | 0/4 |
| moonshotai/kimi-k3 | p4-plain-open | **7.62** | 8.4 | 8.0 | 6.4 | 8.9 | 0/4 |
| z-ai/glm-5 | p4-plain-open | **7.50** | 8.8 | 7.5 | 5.9 | 8.1 | 0/4 |
| anthropic/claude-sonnet-5 | p4-plain-open | **7.50** | 8.0 | 7.9 | 7.4 | 9.4 | 0/4 |
| openai/gpt-5.4-mini | p4-plain-open | **7.38** | 8.4 | 8.2 | 3.9 | 7.2 | 0/4 |
| moonshotai/kimi-k2.5 | p4-plain-open | **7.38** | 8.0 | 8.0 | 5.0 | 6.6 | 0/4 |
| x-ai/grok-4.5 | p4-plain-open | **7.25** | 8.1 | 7.6 | 5.4 | 8.2 | 0/4 |
| openai/gpt-5.4-mini | p3-persona | **7.25** | 8.0 | 7.8 | 7.4 | 9.0 | 0/4 |
| x-ai/grok-4.5 | p3-persona | **7.12** | 8.6 | 7.1 | 6.6 | 7.0 | 0/4 |
| moonshotai/kimi-k3 | p3-persona | **7.12** | 8.5 | 6.8 | 7.2 | 8.8 | 0/4 |
| moonshotai/kimi-k2.6 | p4-plain-open | **6.88** | 8.0 | 6.6 | 7.6 | 8.5 | 0/4 |
| deepseek/deepseek-v4-flash | p4-plain-open | **6.88** | 7.1 | 8.5 | 4.0 | 7.6 | 2/4 |
| meta/muse-spark-1.1 | p3-persona | **6.75** | 7.9 | 6.4 | 7.9 | 8.5 | 0/4 |
| google/gemini-3-flash-preview | p3-persona | **6.75** | 7.8 | 6.9 | 6.6 | 9.0 | 0/4 |
| aion-labs/aion-3.0 | p4-plain-open | **6.75** | 7.9 | 6.8 | 5.5 | 8.8 | 0/4 |
| anthropic/claude-fable-5 | p3-persona | **6.62** | 6.5 | 7.2 | 7.4 | 9.1 | 0/4 |
| openai/gpt-5.6-sol | p3-persona | **6.50** | 8.2 | 6.5 | 7.1 | 5.8 | 0/4 |
| meta/muse-spark-1.1 | p4-plain-open | **6.38** | 7.5 | 6.6 | 4.8 | 6.2 | 0/4 |
| google/gemini-3-flash-preview | p4-plain-open | **6.12** | 6.8 | 7.1 | 4.1 | 8.6 | 0/4 |
| minimax/minimax-m2.5 | p4-plain-open | **5.88** | 6.4 | 7.1 | 3.1 | 8.2 | 0/4 |
| openai/gpt-5.6-luna | p3-persona | **5.75** | 7.5 | 5.8 | 4.6 | 5.4 | 0/4 |
| z-ai/glm-5 | p3-persona | **5.38** | 6.5 | 5.1 | 7.5 | 8.1 | 0/4 |
| minimax/minimax-m2.5 | p3-persona | **5.25** | 6.0 | 5.1 | 5.8 | 7.8 | 0/4 |
| anthropic/claude-sonnet-5 | p3-persona | **5.25** | 6.0 | 5.2 | 7.1 | 9.1 | 0/4 |
| moonshotai/kimi-k2.5 | p3-persona | **4.88** | 5.6 | 4.9 | 8.2 | 8.9 | 0/4 |
| moonshotai/kimi-k2.6 | p3-persona | **4.75** | 6.6 | 4.2 | 6.5 | 5.6 | 0/4 |
| aion-labs/aion-3.0 | p3-persona | **4.75** | 6.1 | 4.2 | 6.4 | 8.6 | 0/4 |
| deepseek/deepseek-v4-flash | p3-persona | **3.62** | 4.5 | 4.4 | 4.8 | 6.9 | 0/4 |

## Initial verdict (prompt retained; model superseded 2026-07-16)

- **Prompt: `p4-plain-open` adopted** into `scriptwriter.rs::system_prompt`
  (failing test first: `system_prompt_carries_comprehension_contract`).
  p4 beat p3 on 12/15 models; for the incumbent gpt-5.4-mini it wins
  7.38 vs 7.25 with clarity 8.2 vs 7.8. Energy dips slightly on some
  models (the contract spends words on the ask) but the decision rule -
  "wins clarity + stranger_recall without tanking brainrot_energy" - holds.
- **Model: `anthropic/claude-fable-5` initially adopted** in
  `doomscrum.toml` + `ScriptConfig::default`. It was the only top scorer that
  kept energy high (6.9) while winning clarity (8.8): 8.38 overall, +0.5 over
  runner-up. This was superseded by the affordable-model reevaluation below.
- Judges (gemini-3.1-pro-preview, grok-4.3) share no family with the
  winner. Leaderboard reputation (EQ-bench etc.) was treated as signal
  only; this in-repo bench on real tickets decided.
- Full per-generation artifacts: `.doomscrum/bench/20260716T172708Z/`
  (gitignored; this file pins the decision evidence).

## Affordable-model reevaluation (adopted 2026-07-16)

Fable's ~$0.035/script cost was rejected. We queried the live OpenRouter
catalog, selected eleven sub-$3/M input candidates across model families, and
ran a twelve-task deterministic Crucible battery over four real DoomScrum
specs. The battery checks ask naming, strict JSON, instruction following,
word budget, and second-person style. The declared eval lives at
`docs/bench/crucible/doomscrum-script-v0.json`.

The initial 300-token cap truncated reasoning-heavy models. A 4,000-token
rerun recovered DeepSeek V4 Flash and MiniMax M2.5 to 12/12; Gemini 3.1 Flash
Lite, Gemini 3 Flash Preview, and GPT-5.4 Mini also passed 12/12. Crucible's
paired comparison found Gemini Lite and GPT Mini identical on all twelve
tasks (McNemar: no discordance, inside noise floor).

The deterministic battery could not choose on prose quality, so the five
best price/contract finalists were generated on the same four specs with
`p4-plain-open` and judged independently by Gemini 3.1 Pro Preview and
Grok 4.3:

| model | judged overall | fidelity | clarity | energy | speak | budget violations | representative script cost |
|---|---:|---:|---:|---:|---:|---:|---:|
| google/gemini-3.1-flash-lite | **6.75** | 6.9 | 7.5 | 4.9 | 9.0 | 0/4 | **$0.0008** |
| mistralai/mistral-small-2603 | 6.38 | 6.6 | 6.5 | 5.2 | 8.8 | 0/4 | $0.00042 |
| openai/gpt-5.4-nano | 6.12 | 7.0 | 7.5 | 3.4 | 6.8 | 2/4 | $0.00065 |
| qwen/qwen3.5-flash-02-23 | 5.38 | 6.6 | 5.9 | 3.5 | 7.4 | 0/4 | $0.000182 |
| bytedance-seed/seed-2.0-mini | 4.38 | 5.0 | 4.9 | 3.4 | 8.8 | 0/4 | $0.00028 |

That first judging pass used a flawed `brainrot_energy` rubric that demanded
the *spoken words* be unhinged. Design doctrine (owner-confirmed 2026-07-16)
is the opposite: the script clearly communicates the spec; the scene, the
character, and the delivery carry the joke. Re-judged with the corrected
rubric (energy scores the scene + delivery; plain content words are never
penalized), same generations, same judges:

| model | judged overall | fidelity | clarity | energy | speak | budget violations | representative script cost |
|---|---:|---:|---:|---:|---:|---:|---:|
| qwen/qwen3.5-flash-02-23 | **7.88** | 7.6 | 7.6 | 8.9 | 8.6 | 0/4 | $0.000182 |
| google/gemini-3.1-flash-lite | **7.50** | 7.4 | 7.2 | 9.0 | 9.1 | 0/4 | **$0.0008** |
| openai/gpt-5.4-nano | 7.38 | 7.6 | 7.8 | 8.1 | 7.2 | 2/4 | $0.00065 |
| mistralai/mistral-small-2603 | 6.00 | 6.0 | 5.9 | 8.5 | 8.9 | 0/4 | $0.00042 |
| bytedance-seed/seed-2.0-mini | 5.50 | 5.0 | 5.2 | 8.5 | 9.2 | 0/4 | $0.00028 |

Cost assumes 2,000 input and 200 output tokens at the live 2026-07-16
OpenRouter rates. `google/gemini-3.1-flash-lite` stays adopted: the 0.38
judged gap to Qwen (n=4 specs x 2 judges) is inside noise, Gemini posts the
top corrected energy (9.0) and speakability (9.1), and it is the only
affordable finalist that passed all twelve Crucible contract checks — Qwen's
one miss leaked a literal `</think>` tag into strict-JSON output, a
production parsing hazard. Qwen 3.5 Flash (4.4x cheaper) is the value
challenger if a later run shows it contract-clean. Gemini remains
contract-equivalent to GPT Mini in Crucible, costs one-third as much as GPT
Mini, and roughly one-thirty-eighth as much as Fable. Judged artifacts:
`.doomscrum/bench/20260716T213213Z/` (original rubric) and
`.doomscrum/bench/20260716T213213Z-rubric2/` (corrected rubric).
