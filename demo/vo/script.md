# DoomScrum Launch VO — locked for final synthesis

Persona: infomercial-narrator × deadpan-hype.  
Rules: every line ≤10 words, numbers spelled as words, no exotic interjections.
The four hero clips speak for themselves; the narrator VO hands off before each clip audio starts.

## Locked beat table

| id | scene | line | words | scratch dur | scene dur | notes |
|---|---|---|---|---|---|---|
| hook | PR hook | This pull request was opened by a swipe. | 8 | 2.687s | 4.0s | Lead with the real PR #45 |
| problem | GitHub issues | Your backlog is rotting. | 4 | 1.759s | 3.8s | Pain beat |
| product1 | Feed reveal | DoomScrum turns it into a feed. | 7 | 2.509s | 5.0s | Product transformation |
| title | Title punch | _(no VO)_ | — | — | 1.0s | SFX-only stinger |
| product2 | Gestures | Swipe right to ship. Swipe left to skip. | 9 | 3.231s | 5.0s | Core mechanic; skip-first contract |
| clipJoke | Proof A (joke) | _(clip audio only — native captions)_ | — | 6.830s | 7.6s | Issue #36, no narrator overlay |
| clipQA | Proof B (QA) | _(clip audio only)_ | — | 5.570s | 6.3s | Issue #40 |
| clipGoblin | Proof C (goblin) | _(clip audio only)_ | — | 6.510s | 7.3s | Issue #39 |
| clipJanitor | Proof D (raccoon) | _(clip audio only)_ | — | 3.970s | 4.7s | Issue #44 |
| swipe | The swipe | Swipe right. Agent cooks it. Pull request opened. | 9 | 4.383s | 6.5s | Targets issue #43 |
| price | Price gag | Three cents a clip. Operators are standing by. | 9 | 3.791s | 5.5s | Infomercial economics |
| cta | CTA | Brew install Doom Scrum. Your backlog is waiting. | 8 | 3.251s | 8.0s | Install command on screen unchanged; final VO uses cleaner read |

## On-screen corrections applied in v2

- Product copy: “SWIPE RIGHT TO SHIP. SWIPE LEFT TO SKIP.” with terminal card matching AGENTS.md skip-first contract.
- Janitor/raccoon beat: headline “RATIFICATION RACCOON”, sticker “ISSUE #44”.
- CTA headline: "GET DOOMSCRUM NOW." On-screen brew command remains exact; final VO calls back to opener.
- Cold-open + PR-proof card now shows the real PR #45 facts.
- Price gag: crossed-out “$1.20/clip → $0.03/clip” with receipt “this entire feed cost $2.80”.

## Final VO delivery request

Please deliver final narrator WAVs to `demo/public/vo/final/` with the same filenames (hook.wav, problem.wav, product1.wav, product2.wav, swipe.wav, price.wav, cta.wav). Beats are designed to tolerate ±0.3s length drift vs scratch before requiring timing re-dial.
