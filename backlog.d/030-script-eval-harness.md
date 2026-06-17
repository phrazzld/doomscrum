# Grow the script bench into a standing eval suite

Priority: P1 · Status: ready · Estimate: M

## Goal
Script quality is engineered, not vibed: prompt/model changes to the
scriptwriter are gated by a repeatable eval with thresholds, not eyeballs.

## Oracle
- [ ] A golden spec set (≥10 specs spanning structured, messy, and
      foreign-repo formats) lives in the repo as eval fixtures.
- [ ] One command re-runs the bench on the current production prompt+model
      and fails (non-zero exit) if mean overall drops below a recorded
      baseline, or if any budget-violation/json-parse failure rate exceeds
      its threshold.
- [ ] Judge stability is measured: same cell judged twice must correlate;
      report flags dimensions where the two judges disagree by >2 points.
- [ ] Each production prompt/model change lands with a fresh
      docs/bench/<date>-*.md report justifying it.
- [ ] The bench includes at least one downstream render check that scores
      whether generated narration and captions preserve the ticket goal and
      oracle phrase after alignment/transcription.

## Notes
v0 shipped 2026-06-11: scripts/script_bench.py (7 models x 3 prompts x
4 specs, gemini-3.1-pro + grok-4.3 judges, ~$0.70/run) — results in
docs/bench/20260611-script-bench.md. Findings to build on: persona-first
prompting beat coverage-first across nearly all models; gpt-5.4-mini won
(8.75), kimi-k2.5 is the budget runner-up and outscored its pricier k2.6
sibling; deepseek-v4-flash is cheap but flat. Known gaps in v0: n=4
specs, single generation per cell (temp 0.9 variance unmeasured), no
scene-quality dimension fed back from actual video renders, judges not
yet checked for self-consistency. **Why:** owner mandate 2026-06-11 —
"build our own benchmarks and evals... actual science and engineering."

Research 2026-06-13: script quality cannot stop at JSON parse and copy
quality. The production pipeline treats the script as the source of truth, so
the eval should catch scripts that technically fit a duration but lose the
ticket's goal/oracle after TTS, forced alignment, captions, and transcript
normalization.

Groom 2026-06-17: **Gate 1** in `docs/VISION.md` — the quality users judge
first, paired with [[031-render-verdict-gate]]. 030's 5th oracle (downstream
render check) is the upstream sibling of 031's per-render verdict gate and its
new checker-self-test; sequence them together.
