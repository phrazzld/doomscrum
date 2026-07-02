# Audit whether the core brainrot joke actually lands

Priority: P1 · Status: ready · Estimate: L (epic)

## Goal
Determine, with evidence rather than vibes, whether DoomScrum's core mechanic —
spec-to-brainrot-video — is actually funny/shareable as implemented today, and
close the sharpest gap the evidence surfaces.

## Oracle
- [ ] Evidence table built from the existing bench data
      (`docs/bench/20260611-script-bench.md`) plus a fresh sample of ≥10 renders
      taken from the default `fake` provider path (`providers/fake.rs`), each
      rated for (a) whether the free/default render actually delivers brainrot
      (voice + animated scene) or just spec-title-on-color-card text overlay,
      and (b) whether the *spoken script* is funny independent of the visual
      scene description.
- [ ] The specific failure pattern already visible in the bench is quantified,
      not just noted: count how many scored cells' critiques say the equivalent
      of "relies entirely on the visual scene for brainrot energy" / "dry
      corporate copy" — this pattern recurs in the large majority of the
      7-model × 3-prompt bench and is the leading candidate for "the joke is
      underbaked."
- [ ] A decision recorded: either (a) a prompt/scaffolding fix that makes the
      *spoken dialogue itself* unhinged (not just accurate), landing as a new
      "dialogue brainrot" scoring dimension in [[030-script-eval-harness]], or
      (b) an explicit, evidenced call that the visual scene alone carries the
      joke sufficiently and the dialogue is allowed to stay dry.
- [ ] The free-tier path is evaluated separately from the paid FAL path:
      per the README, the offline fixture only gets ffmpeg `drawtext` text
      overlay (spec title + format name over a colored card) — confirm whether
      a $0 stranger ever sees the actual joke (character, voice, unhinged
      scene) or only a placeholder, since real brainrot generation currently
      requires a paid FAL render.
- [ ] Whatever is decided is captured back into [[030-script-eval-harness]]
      (rubric change) and/or a fresh `docs/bench/<date>-*.md` report — a
      decision that only lives in this ticket doesn't change the product.

## Verification System
- Claim: DoomScrum's default, $0 experience is actually funny/shareable, not
  just accurate.
- Falsifier: the free fixture path never renders anything a stranger would
  screenshot and send to a friend; the spoken script is corporate-dry across a
  representative sample regardless of prompt/model.
- Driver: re-read the existing bench report plus a fresh 10-render sample from
  the `fake` provider on this repo's own backlog.
- Grader: the evidence table plus the named/quantified failure pattern; a
  named decision (fix dialogue vs. accept visual-carries-the-joke).
- Evidence packet: this ticket's Oracle checkboxes plus any new
  `docs/bench/<date>-*.md`.
- Cadence: this is a judgment ticket — one grooming/analysis pass, then either
  closes with the decision recorded or spins a follow-up child ticket for the
  dialogue-prompt fix.

## Notes
Does not duplicate [[030-script-eval-harness]] or [[031-render-verdict-gate]]
— those are eval/QA *plumbing* (repeatable benchmarks, verdict gates). This
ticket is the product judgment call that plumbing exists to serve, using the
plumbing's own output (the 2026-06-11 bench) as primary evidence. Filed
2026-07-02 during a product-groom investigation pass; see the groom's report
for the specific bench citations.
