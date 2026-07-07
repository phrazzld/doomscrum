# [PROPOSAL — needs ratification] Purge orphaned artifacts and one dead fn

Priority: P3 · Status: proposed · Estimate: S

## Goal
Remove regenerable/orphaned weight and a dead function without touching live
behavior. Deletions require owner ratification — this ticket is the proposal,
not an approved change. Nothing here has been deleted.

## Oracle (each item independently ratifiable; one commit per item)
- [ ] (a) `rm -rf .brainrot/` (5.8M orphaned pre-rename state, no `src/`
      references; includes artifacts of the removed run-packet concept) + drop
      `/.brainrot/` from `.gitignore` and the "pre-rename state dir" notes in
      CLAUDE.md/AGENTS.md.
- [ ] (b) Stop tracking `demo/public/*.mp4` + `sfx/*.wav` (41M of regenerable
      dev-tooling fixtures; `demo/out/` is already ignored). Gitignore them and
      document regeneration via the `/demo` skill. (Stops future bloat; shrinking
      the 176M `.git` would be a separate history-rewrite decision.)
- [ ] (c) Delete `clip_words` (distill.rs:179) + its test (distill.rs:1014-1017):
      dead code — only its own test calls it; `tighten` is the live clipper.
- [ ] (d) Delete `docs/adoption/` (a landed PR's body + screenshots,
      unreferenced); decide keep-or-archive `docs/archive/` (historical MVP spec).
- [ ] Gate after each: `cargo test` green; `git grep clip_words` empty; `git grep
      '\.brainrot' src/` empty; `git ls-files demo/public | grep mp4` empty;
      `/demo` still regenerates MP4s.

## Notes
From the groom simplification lane (2026-06-17). Sizes verified live: `.brainrot`
5.8M, `demo/public` 41M (4 mp4 + 3 wav tracked), `.git` 176M. The code is
otherwise lean — clean clippy, no dead config fields, no migration shims — and
the 6 brainrot formats all earn their keep (each tested + distinct). DELETIONS
NOT APPLIED: awaiting owner ratification per the groom contract (deletions stay
proposals).
