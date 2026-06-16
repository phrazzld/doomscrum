# Adopt the @misty-step/aesthetic language — loud, in the family

DoomScrum stays loud — the acid, the pink, the cyan, the 9:16 swipe
feed, the gesture hints — but the embedded UI now speaks the design
system's language: pinned dark on the aesthetic surface, Geist as the
loud face, the scheme steered, and the law held where it counts.

Because the feed is a single self-contained page embedded in the Rust
binary (`assets/index.html`, `include_str!`), this adopts the
aesthetic **language** (tokens, type, and law) directly rather than a
package import — the honest path for a bespoke embedded surface.

## What changed (all in `assets/index.html`)

- **Pinned dark on the system surface.** `--bg`/`--ink`/`--line`/
  `--dead` become the aesthetic dark tokens (`#121212` / `#ededed` /
  `#262626` / `#5c5c5c`); `<html class="dark">`, `color-scheme: dark`.
- **Geist is the loud face.** Impact / Arial Black retire across the
  splash, captions, stickers, overlays, and buttons — the loud
  register is now Geist weight 800 (loud by weight, never a display
  family); Geist Mono carries the terminal/meta voice.
- **The scheme is steered.** Acid (`#b6ff2e`) is the accent; pink and
  cyan stay as **project tokens**. They spend themselves on borders,
  glyphs, and ink — not on filled neon pills. The status stickers
  (`queued`/`running`/`opening_pr`/`pr_opened`/`failed`/`skipped`)
  become hairline tag chips whose **border and text** carry the hue;
  `failed` takes the system's danger ink.
- **Nothing ambient.** Motion is feedback now: the scanline overlay,
  the radial grain glow, the marquee ticker crawl, the splash jitter,
  and every idle sticker throb are **gone**. The card still answers
  your swipe; buttons still answer your press; a `prefers-reduced-
  motion` guard kills even those.
- **Hairlines, radius 0.** The 3px black borders and brutalist
  `3px 3px 0 #000` offset shadows give way to 1px `--line` hairlines;
  corners were already sharp.

## Verification

- `cargo build` — green; the rewritten page is embedded.
- Served locally (`doomscrum serve --port …`) and walked headless at
  430×932, **zero console errors**; before/after of the splash plus
  the live feed and empty state captured in `docs/adoption/`.

### Before / after — the splash

The glitchy Impact wordmark (black stroke + pink drop-shadow, jittering)
becomes a clean, confident Geist 800 acid wordmark on the system
surface — still unmistakably DoomScrum, now unmistakably Misty Step:

![before](docs/adoption/before-splash.png)
![after](docs/adoption/after-splash.png)

The feed (`after-feed.png`): a quiet chrome-register ticker (no crawl),
the `REPO` chip as a hairline cyan tag, hairline card frames, the
gesture hints and the scheme intact.

## One deliberate deviation

The primary "COOK FIXTURE FEED" action keeps an acid fill — the loud
consumer's one earned accent moment. Flag it if you'd rather it go ink
like the rest.

## Post-review remediation

Fresh-context review (two model families + a live browser walk) surfaced three
fixes, now applied — still all in `assets/index.html`:

- **Fonts self-hosted.** Geist + Geist Mono are embedded as base64 `woff2` (OFL)
  in a local `@font-face`; the three `fonts.googleapis.com`/`gstatic.com`
  `<link>`s are gone. The page makes **zero external requests** again — the
  "self-contained" claim above is now literally true, with no third-party font
  CDN reached from a local tool. Verified: 0 `fonts.g*` requests on load.
- **Contrast.** `--dead` (`#5c5c5c`, 2.80:1 on `--bg`) was readable text for the
  spec-overlay path+sha256, the picker label, and the spend meter — all below
  WCAG AA. Moved to `--muted` (`#8f8f8f`, 5.79:1).
- **The success state got its color back.** `pr_opened` rendered acid —
  identical to a fresh/default sticker — so "PR opened" (the payoff of a
  right-swipe) was indistinguishable at a glance. It and `completed_local` now
  use `--ok` (`#6fd2a8`), completing the status palette (queued/running
  `--warn`, opening_pr `--cyan`, failed `--err`, done `--ok`).

🤖 Generated with [Claude Code](https://claude.com/claude-code)
