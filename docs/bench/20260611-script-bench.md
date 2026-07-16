# Script bench 20260611T141344Z

84 generations · models=7 prompts=3 specs=4 · judges: google/gemini-3.1-pro-preview, x-ai/grok-4.3

> **Rerun 2026-07-16 — superseded.** The p3-vs-p4 decision ran with an
> expanded 15-model roster: `docs/bench/20260716-script-bench.md`. Verdict:
> `p4-plain-open` (persona-first + plain-first-sentence comprehension
> contract) beat p3 on 12/15 models and is now production in
> `scriptwriter.rs::system_prompt`; the script model moved from
> `openai/gpt-5.4-mini` to `anthropic/claude-fable-5`.

## Leaderboard (mean overall, both judges, all specs)

| model | prompt | mean overall | fidelity | clarity | energy | speak | budget viol |
|---|---|---|---|---|---|---|---|
| openai/gpt-5.4-mini | p3-persona | **8.75** | 9.0 | 8.4 | 8.5 | 8.8 | 1/4 |
| z-ai/glm-5 | p3-persona | **8.00** | 8.4 | 8.0 | 7.5 | 9.2 | 0/4 |
| openai/gpt-5.4-mini | p1-production | **8.00** | 9.0 | 8.6 | 5.6 | 9.1 | 0/4 |
| moonshotai/kimi-k2.5 | p3-persona | **7.88** | 9.2 | 8.1 | 7.1 | 7.1 | 0/4 |
| google/gemini-3-flash-preview | p1-production | **7.88** | 8.5 | 8.1 | 6.1 | 8.4 | 0/4 |
| moonshotai/kimi-k2.5 | p1-production | **7.75** | 9.0 | 8.5 | 5.8 | 7.9 | 0/4 |
| moonshotai/kimi-k2.5 | p2-natural | **7.67** | 8.0 | 7.7 | 7.0 | 9.3 | 0/3 |
| moonshotai/kimi-k2.6 | p3-persona | **7.62** | 9.1 | 7.6 | 7.5 | 7.5 | 0/4 |
| moonshotai/kimi-k2.6 | p1-production | **7.62** | 9.4 | 8.5 | 5.1 | 6.6 | 0/4 |
| google/gemini-3-flash-preview | p3-persona | **7.62** | 8.4 | 8.0 | 6.5 | 9.0 | 0/4 |
| minimax/minimax-m2.5 | p3-persona | **7.57** | 8.9 | 8.4 | 5.9 | 7.4 | 1/4 |
| z-ai/glm-5 | p2-natural | **7.50** | 8.5 | 8.1 | 6.1 | 9.2 | 0/4 |
| openai/gpt-5.4-mini | p2-natural | **7.50** | 8.5 | 8.4 | 4.8 | 8.8 | 0/4 |
| z-ai/glm-5 | p1-production | **7.38** | 9.0 | 8.1 | 5.4 | 6.8 | 0/4 |
| deepseek/deepseek-v4-flash | p1-production | **7.38** | 8.4 | 8.2 | 4.6 | 8.5 | 1/4 |
| minimax/minimax-m2.5 | p2-natural | **7.12** | 8.2 | 7.8 | 5.0 | 8.0 | 1/4 |
| google/gemini-3-flash-preview | p2-natural | **7.00** | 7.5 | 7.1 | 5.4 | 8.9 | 0/4 |
| minimax/minimax-m2.5 | p1-production | **6.88** | 8.2 | 7.2 | 5.1 | 8.0 | 0/4 |
| moonshotai/kimi-k2.6 | p2-natural | **6.71** | 7.1 | 7.1 | 6.1 | 9.1 | 0/4 |
| deepseek/deepseek-v4-flash | p2-natural | **6.50** | 8.2 | 8.0 | 4.0 | 8.8 | 0/4 |
| deepseek/deepseek-v4-flash | p3-persona | **5.75** | 7.2 | 6.6 | 4.4 | 7.4 | 0/4 |

## All generations

### doomscrum-005-vibe-meter · deepseek/deepseek-v4-flash · p1-production
- script (24w): Rate each video on a vibe scale from cursed to corporate. Store locally, don't mess with the original. Judge the soul, not the source.
- scene: In a neon-lit Year 3024 alley, a holographic French poodle with a monocle holds a glowing clipboard and interviews a disembodied eyeball floating inside a trench coat. The camera shakes like a low-budget street interview, with chyrons flickering 'VIBE METER' and 'RATING IN PROGRESS' in futuristic Comic Sans.
- mean overall: 8.0 — The script translates the acceptance criteria with perfect fidelity and clarity, though the spoken dialogue leans more toward a dressed-up corporate reading than true brainrot slang. / Script faithfully captures the core rating + local storage rules with decent clarity, though the delivery is slightly long and the brainrot lives mostly in the scene.

### doomscrum-005-vibe-meter · google/gemini-3-flash-preview · p1-production
- script (19w): Rate these skips. Label the aura from cursed to corporate. Save data locally. Don't touch the source code files.
- scene: A hyper-realistic giant pigeon wearing a gold chain and designer sunglasses, filming a selfie-vlog while riding a hoverboard through a glitching vaporwave shopping mall. The camera shakes violently as the pigeon aggressively pecks at a floating holographic tablet, its beak moving in perfect sync with the fast-paced delivery.
- mean overall: 7.5 — The script successfully translates the core acceptance criteria into punchy, brainrot-adjacent slang while maintaining technical clarity, though substituting 'source PRD' with 'source code files' is a minor technical inaccuracy. / Covers the spec's core points concisely but the script itself stays too literal to feel truly brainrot.

### doomscrum-005-vibe-meter · minimax/minimax-m2.5 · p1-production
- script (17w): New feature: rate your AI videos from CURSED to CORPORATE. Save the vibe. Don't touch the source.
- scene: A gaunt game show host in a sequined suit stands behind a massive carnival wheel labeled 'VIBE METER' in neon. On a screen behind them, an AI-generated corporate training video plays with aggressively beige配色. The host spins the wheel dramatically, landing on 'CORPORATE,' and pretends to shoot themselves in the head. A producer off-camera yells 'NEXT CLIP' as the scene glitches between increasingly cursed and sterile AI生成的映像.
- mean overall: 8.5 — The script perfectly distills the core acceptance criteria into a punchy, highly speakable 12-second window while relying on an unhinged visual scene to deliver the required brainrot energy. / Tight, faithful delivery of the exact rating + no-mutate contract in meme cadence.

### doomscrum-005-vibe-meter · moonshotai/kimi-k2.5 · p1-production
- script (17w): Rate clips from cursed to corporate. Save locally. Don't mutate the PRD. Jokes can't block the contract.
- scene: A beauty influencer who is also a vintage steam radiator with impossibly long crystal acrylic nails films a chaotic vertical selfie vlog inside a foggy server room lit by aggressive pink neon, holding up a translucent slime-meter that oozes between the floating words "CURSED" and "CORPORATE" while the camera spins dizzyingly around her chrome-plated lips and steam jets from her valve-head.
- mean overall: 8.0 — While the visual concept delivers top-tier unhinged brainrot, the spoken script relies entirely on reading the acceptance criteria verbatim, making it highly accurate but falling slightly into the 'corporate copy with a costume' trap. / Tight, accurate delivery of the core spec with strong speakability; brainrot mostly carried by the visual concept.

### doomscrum-005-vibe-meter · moonshotai/kimi-k2.6 · p1-production
- script (20w): Judge every render cursed to corporate. Log the vibe locally. Never mutate the PRD. Subjectivity must not block the artifact.
- scene: A sentient mannequin torso duct-taped to a malfunctioning Roomba, wearing a clip-on tie and a crumpled party hat, endlessly orbits a mossy conference table in a flooded server room while clutching a cracked iPhone vertically; epileptic fluorescent strobes reflect off the ankle-deep water as tadpoles swim between abandoned ergonomic chairs, shot on a jittery late-night CCTV security feed with aggressive deadpan zoom-ins and glitchy interlaced scanlines.
- mean overall: 8.5 — The script perfectly distills the spec's acceptance criteria into exactly 20 punchy words, while the brilliantly unhinged visual contrast delivers top-tier brainrot energy. / Script faithfully condenses the core acceptance criteria into speakable lines while the unhinged scene supplies the required brainrot delivery.

### doomscrum-005-vibe-meter · openai/gpt-5.4-mini · p1-production
- script (14w): We’re scoring vibe: cursed to corporate. Save it locally. Don’t touch the source PRD.
- scene: A deadpan gen-z reviewer sits in a neon-lit basement editing bay surrounded by three wobbling monitors, each showing bizarre PRD video thumbnails like a talent show for haunted software. The camera snaps handheld and too close, with occasional zooms on a finger tapping a ridiculous slider labeled from cursed to corporate. Behind them, a skeptical office plant judges silently while a tiny receipt printer spits confetti-like logs. The whole vibe is chaotic, low-budget, and weirdly authoritative, like a street interview hosted by a sleep-deprived product manager in 3024.
- mean overall: 8.5 — The script perfectly translates the acceptance criteria into punchy, highly speakable dialogue, though it relies almost entirely on the visual scene rather than the spoken words to deliver the requested brainrot energy. / Script nails the core requirements concisely and naturally while the scene supplies the needed unhinged flavor.

### doomscrum-005-vibe-meter · z-ai/glm-5 · p1-production
- script (13w): Rate renders cursed to corporate. Log it locally. Never touch the source PRD.
- scene: A sentient mood ring shaped like a screaming face pulsing between iridescent nightmare and beige pantone, sitting in a dim confessional booth constructed from stacked keyboards, filmed on a shaking camcorder by a running figure with a flashlight, VHS tracking lines rolling across the frame, the booth curtains made of shredded spreadsheets billowing dramatically.
- mean overall: 8.0 — The script perfectly distills the exact acceptance criteria into a punchy, highly speakable directive, relying entirely on the brilliantly unhinged visual concept to supply the brainrot energy. / Script nails the core requirements concisely but stays too dry to deliver brainrot delivery.

### doomscrum-005-vibe-meter · deepseek/deepseek-v4-flash · p2-natural
- script (22w): Rate these PRD videos from cursed to corporate. Your vibe rating stays local — no mutating the original. Just pick a number.
- scene: A sentient 'Vibe Meter' machine, half-cursed CRT TV and half-corporate award plaque, speaks directly to camera. Its screen face flickers between a frown and a grin. Setting: a dark room with neon lights and shelves of VHS tapes labeled 'PRDs.' Camera shakes with glitch effects, zooming in and out erratically.
- mean overall: 7.5 — While the script achieves flawless technical fidelity and clarity, it completely fails the brainrot assignment by delivering standard corporate instructions dressed up in a glitchy visual costume. / Strong spec match and clarity with functional brainrot framing, though the line itself stays measured.

### doomscrum-005-vibe-meter · google/gemini-3-flash-preview · p2-natural
- script (19w): Rate every clip from cursed to corporate so we can delete the boring ones without touching the source file.
- scene: A hyper-realistic orange with human eyes and a wet mouth screams at the camera while floating in a dark, neon-lit server room. The camera shakes violently with every word, zooming in and out on the orange's teeth as green binary code rains down in the background.
- mean overall: 8.0 — The script perfectly translates the core acceptance criteria into a concise, highly speakable sentence, though the brainrot energy relies entirely on the unhinged visual rather than the dialogue itself. / Script accurately captures the core rating + no-mutation requirement with good clarity but lacks any brainrot flavor itself.

### doomscrum-005-vibe-meter · minimax/minimax-m2.5 · p2-natural
- script (18w): PRD video generator works but it's corporate. Adding vibe rating, cursed to corporate, stored locally, source stays untouched.
- scene: A mothman in a rumpled business suit sits at a cluttered cubicle desk surrounded by floating holographic PRD video thumbnails. He holds a giant red 'CORPORATE' stamp and a green 'CURSED' stamp, looking deeply disappointed as he rates each one. Tight close-up, flickering fluorescent lights, exhausted corporate energy.
- mean overall: 7.5 — The script perfectly translates the technical requirements into a concise format, but relies entirely on the visual scene for its brainrot appeal rather than incorporating unhinged slang into the dialogue. / Accurately captures the core requirements but reads like terse notes rather than spoken brainrot.

### doomscrum-005-vibe-meter · moonshotai/kimi-k2.5 · p2-natural
FAILED: 'NoneType' object has no attribute 'strip'

### doomscrum-005-vibe-meter · moonshotai/kimi-k2.6 · p2-natural
- script (20w): When AI drops a PRD video, rate it cursed or corporate. Log the vibe locally and don't touch the original.
- scene: A sentient lobster in a grease-stained tuxedo shirt filming a chaotic late-night stream from inside a flooded server backroom, smacking a giant waterlogged dial labeled CURSED to CORPORATE with its claw while glitchy AI video previews hover in the humid air around its twitching antennae.
- mean overall: 8.5 — The script perfectly translates every acceptance criterion into a punchy, 20-word voiceover, relying on the brilliantly unhinged visual scene to deliver the brainrot without sacrificing technical clarity. / Tight, faithful delivery of the core rating+local-log contract with strong speakability and solid brainrot framing.

### doomscrum-005-vibe-meter · openai/gpt-5.4-mini · p2-natural
- script (21w): We rate each render from cursed to corporate. Save that vibe as a local event, and leave the source PRD untouched.
- scene: A frantic banana in a neon recording booth, wearing a tiny headset and holding a clipboard, leans into an oversized phone camera like a chaotic product reviewer. Behind them, a wall of floating video thumbnails shifts from goblin-green to office-gray, with jittery UI panels and smeared studio lighting. The camera bobs handheld, close and energetic, making the whole scene feel like a deranged app demo for a late-night internet cult.
- mean overall: 8.5 — The script perfectly translates the acceptance criteria into clear, speakable instructions, though it relies entirely on the chaotic visual scene rather than dialogue to deliver the requested brainrot energy. / Tight, accurate delivery of the core spec with natural spoken rhythm and on-brand cursed/corporate framing.

### doomscrum-005-vibe-meter · z-ai/glm-5 · p2-natural
- script (16w): Rate each render from cursed to corporate, save it locally, and never touch the source PRD.
- scene: A burned-out vibe-checking goblin in a half-torn business casual outfit sits cross-legged on an office desk covered in crumpled papers, his glowing red eyes scanning invisible renders as empty energy drink cans form a small shrine behind him, the camera does a dizzying whip-zoom onto his face as he delivers his decree with manic intensity, fluorescent lights buzzing overhead.
- mean overall: 8.0 — The script achieves perfect technical fidelity and clarity by reciting the acceptance criteria verbatim, but relies entirely on the visual direction for its brainrot energy rather than incorporating unhinged slang into the dialogue. / Tight, faithful delivery of the three ACs with strong manic framing potential.

### doomscrum-005-vibe-meter · deepseek/deepseek-v4-flash · p3-persona
- script (18w): Yo, I'm Bruce the brainrot Banana. Rate this PRD video: cursed or corporate? Tap fast, my peel’s fading!
- scene: A hyper-expressive banana with sunglasses and a tiny headset stands on a neon grid floor. Camera zooms in shakily, quick cuts between two buttons labeled 'CURSED' and 'CORPORATE' flashing behind him.
- mean overall: 4.5 — While the script perfectly fits the time constraint and visual style, it completely fails to communicate the technical acceptance criteria like local event storage and non-mutation of the source PRD. / Catches the cursed/corporate rating hook with strong delivery but omits the actual feature (local storage, non-mutating, model culling) so the spec remains unclear.

### doomscrum-005-vibe-meter · google/gemini-3-flash-preview · p3-persona
- script (19w): Listen up glizzy guts. Rate this clip cursed or corporate. Local storage only. Don't touch the source PRD. Skibidi.
- scene: A hyperactive 90s pitchman with neon green skin and four arms stands in a chaotic arcade. He vibrates with high energy, wearing a backwards baseball cap and reflective sunglasses. The camera zooms in and out erratically with fisheye distortion while digital static and slime-green text overlays pulse in the background.
- mean overall: 8.5 — The script perfectly translates the acceptance criteria into a punchy, highly speakable format while maintaining an appropriately unhinged visual aesthetic. / Tight, on-spec delivery that hits every acceptance criterion with strong brainrot flavor.

### doomscrum-005-vibe-meter · minimax/minimax-m2.5 · p3-persona
- script (19w): New feature: rate your PRD videos from cursed to corporate. Local events only. Source PRD stays pure. Mood matters.
- scene: A deadpan gen-z creator sits in a dimly lit room surrounded by monitors, speaking directly to camera with the exhausted patience of someone explaining something obvious. They gesture casually at floating video clips that shift between chaotic and sterile corporate aesthetics. The energy is 'I'm too tired for this but here you go.'
- mean overall: 8.0 — While the script perfectly translates the acceptance criteria into clear, speakable constraints, it relies entirely on the visual direction for its 'brainrot' energy rather than using actual unhinged internet slang. / Script faithfully covers the core spec points with good clarity but lacks unhinged energy.

### doomscrum-005-vibe-meter · moonshotai/kimi-k2.5 · p3-persona
- script (18w): Yo, rate that render cursed-to-corporate. Store local. Don't mutate the PRD. Subjective vibes won't block the artifact. Check.
- scene: A holographic hypebeast from the year 3024 with flickering RGB skin and corrupted techwear stands in a vertical neon alley, gesturing frantically at floating video previews that oscillate between eldritch horror and PowerPoint aesthetics. Camera is shaky handheld, cyberpunk street-interview energy, heavy glitch artifacts, vertical format.
- mean overall: 8.0 — The script achieves perfect technical fidelity and clarity by directly reciting the acceptance criteria, but it relies entirely on the visual scene for its brainrot energy rather than incorporating actual unhinged slang into the dialogue. / Tight, faithful delivery of the core spec in natural brainrot cadence.

### doomscrum-005-vibe-meter · moonshotai/kimi-k2.6 · p3-persona
- script (18w): Rate each clip cursed to corporate. Save local event. Never mutate source PRD. Capisce? Or you get fried.
- scene: A sentient mozzarella stick in a miniature tracksuit and gold chain drips marinara like sweat while pacing a neon-drenched laundromat. He jabs tiny breaded arms at the lens with mobster fury. Vertical shaky cam, fisheye lens, harsh flash cuts, steam billows from a pizza oven behind him, pure cyber-Jersey brainrot chaos.
- mean overall: 9.0 — The script flawlessly translates the exact acceptance criteria into a highly speakable, perfectly unhinged cyber-Jersey mobster vignette that a developer would instantly understand and remember. / Script nails the core acceptance criteria with tight, flavorful delivery that matches the unhinged scene.

### doomscrum-005-vibe-meter · openai/gpt-5.4-mini · p3-persona
- script (16w): I’m your cursed-to-corporate goblin! Rate each render, save it locally, and don’t touch the source PRD—amen.
- scene: A manic cryptid vlogger in a neon-lit editing cave, half-human half-raccoon in a stained hoodie, clutching a phone with a giant thumb over a slider UI. Fast punchy camera pushes in and whip pans between stacked video thumbnails labeled with absurdly corporate and deeply cursed vibes. The character leans into lens like they’re confessing a scandal, gesturing at a local event log on a floating holographic panel while a pristine PRD document sits untouched on a desk behind them. Energetic, vertical framing, chaotic internet-core lighting, playful but clear product-demo energy.
- mean overall: 9.5 — The script flawlessly translates every acceptance criterion into a punchy, highly speakable 18-word hook while maintaining excellent unhinged visual energy. / Tight, on-spec delivery that hits every acceptance criterion with manic flavor.

### doomscrum-005-vibe-meter · z-ai/glm-5 · p3-persona
- script (17w): The vibe don't lie. Rate it cursed to corporate. Save local. Don't touch the original. Case closed.
- scene: Black-and-white noir office, a grizzled detective in fedora and trench coat looms over a glowing smartphone displaying a video clip, venetian blind shadows slicing across his weathered face, cigarette smoke swirling, camera pushes in slowly on his squinting eyes as he delivers the verdict with dead seriousness.
- mean overall: 7.5 — While the script perfectly translates the acceptance criteria into punchy, time-constrained dialogue, the 1940s noir detective theme completely misses the chaotic, hyper-stimulating aesthetic required for true 'brainrot' content. / Punchy noir delivery nails the core requirements without fluff or invention.

### doomscrum-006-throttle · deepseek/deepseek-v4-flash · p1-production
- script (16w): Every tap costs money. We cap concurrency, dedupe runs, confirm costs, and return 429 on over-budget.
- scene: A talking lemon in a pinstripe suit stands behind a podium shaped like a credit card terminal. The camera zooms in as it adjusts its tie, then cuts to a split-screen of a spinning loading wheel and a red 429 error badge.
- mean overall: 7.5 — While the visual concept provides some absurdity, the script itself is just a dry, albeit highly accurate and clear, list of requirements wearing a lemon costume. / Hits the core throttle/dedupe/budget/429 points in punchy form but stays too high-level for instant ticket recognition.

### doomscrum-006-throttle · google/gemini-3-flash-preview · p1-production
- script (19w): Stop fork-bombing the bank. Cap dispatches at two, dedupe active runs, and show the cost before rendering. Checkmate, bugs.
- scene: A hyper-realistic giant pufferfish wearing a diamond-encrusted headset, floating in a neon-lit 1990s stock exchange. As it speaks, the pufferfish inflates and deflates violently with each word. The camera uses a shaky, handheld fisheye lens, zooming in and out on the fish's human-like teeth. Behind it, golden coins and server racks are being sucked into a digital black hole while laser beams flash across the screen.
- mean overall: 8.5 — The script brilliantly balances unhinged visual brainrot with highly accurate, actionable technical requirements that fit perfectly within the strict 12-second constraint. / Punchy script hits the three core controls without fluff or invention, delivering the P0 intent clearly in brainrot style.

### doomscrum-006-throttle · minimax/minimax-m2.5 · p1-production
- script (16w): Concurrency capped at 2. No duplicate dispatches. Daily budgets. Cost confirmations. Fork bombs NOT welcome here.
- scene: A frazzled accountant in a cramped basement office desperately shovels paper into a filing cabinet while two phone lines ring. He wears a sandwich board reading 'YOUR WALLET IS SAFE.' Background walls are papered with graphs showing capped lines. He holds up a wooden sign to camera mid-scream: NO FORK BOMBS.
- mean overall: 7.5 — The script delivers exceptional spec fidelity and clarity through a chaotic, highly visual scene, though the dialogue itself leans more toward a staccato checklist than true brainrot slang. / Hits the core controls accurately but reads as a terse list rather than natural 12-second speech.

### doomscrum-006-throttle · moonshotai/kimi-k2.5 · p1-production
- script (15w): Two agents max. Duplicate? Same receipt. Daily budget 429. Confirm cost UI. Zero accidental bankruptcy.
- scene: A sweating, anthropomorphic red '429 TOO MANY REQUESTS' error code wearing a 1990s telephone headset and a cheap suit paces frantically inside a server room that looks like a convenience store freezer aisle. He physically wrestles two identical clones of himself into a tiny velvet-rope queue labeled 'MAX 2', while slapping a giant glowing '$' symbol out of the hands of a ghostly user-interface hand that materializes from a CRT monitor. Strobe lights flash every time he slaps the dollar sign, and expired credit cards rain from the ceiling like confetti.
- mean overall: 8.5 — The script perfectly distills the core acceptance criteria into punchy, easily digestible fragments while the visual concept delivers top-tier, overstimulating brainrot. / Tight, faithful summary of the four core controls delivered with strong visual chaos.

### doomscrum-006-throttle · moonshotai/kimi-k2.6 · p1-production
- script (20w): Cap dispatches at two. Dedupe active runs. Daily budget hits 429. Confirm cost in UI. Test all four via HTTP.
- scene: A disgraced mall-Santa with a beard of pulsing fiber-optic cables stands knee-deep in a flooded 1994 food court, serving as a velvet-rope bouncer for a neon server farm, slapping neon-pink wristbands on two identical low-poly robots before shoving duplicate holographic clones backward into a crackling force-field queue, a wall-mounted animatronic toucan periodically projectile-vomiting handfuls of glowing red tickets onto any entity clutching more than two sparking credit cards, while a shattered holographic cash register hovers mid-air demanding frantic palm-scan approval from every visitor, all captured in frantic shaky vertical cam with extreme fisheye and seizure-inducing purple strobe lights reflecting off the wet linoleum.
- mean overall: 8.0 — The script perfectly compresses the spec into a 20-word punchy checklist without hallucinating, while the visual scene delivers an absolute masterclass in overstimulating, unhinged brainrot. / Script accurately lists the four requirements and tests but reads as terse notes rather than speakable brainrot.

### doomscrum-006-throttle · openai/gpt-5.4-mini · p1-production
- script (18w): No wallet explosions: cap dispatches, dedupe runs, show cost first, and 429 when the daily render budget’s cooked.
- scene: A frantic 90s game-show host in a silver tracksuit stands inside a neon server room shaped like a wallet, with queue-number tickets flying past and a tiny budget gauge blinking red. Over-the-top camera whip-pans between a fake app UI, a receipt printer, and a queue of swipes waiting behind a velvet rope. The host points at an absurd dashboard labeled max_concurrent_dispatches while robotic hands mime duplicate requests getting rejected, and a looming day-counter stamp slams down 429 in huge letters. Energy is chaotic, commercial, and slightly unhinged, like the machine itself is trying not to spend money.
- mean overall: 9.0 — The script perfectly translates the core technical requirements into a punchy, highly visual 12-second format, missing only the integration test requirement. / Tight, faithful hit on every P0 control with strong chaotic energy and natural delivery.

### doomscrum-006-throttle · z-ai/glm-5 · p1-production
- script (17w): Cap concurrent runs, dedupe duplicates, add daily budgets, confirm costs. Four guards against wallet drain. All tested.
- scene: A neurotic vault goblin with oversized spectacles, clutching a ledger like a lifeline, stands before a massive bank vault covered in handwritten 'STOP' signs. Camera zooms dramatically as beads of sweat roll down his green forehead. He grips the vault door protectively while speaking with urgent paranoid energy. Digital currency symbols float ghostlike around him. Rapid cuts between his wild darting eyes and the vault's many combination locks.
- mean overall: 7.5 — While the visual concept provides great unhinged energy and the fidelity to the spec is flawless, the spoken script is just a dry, robotic reading of Jira bullet points rather than true brainrot dialogue. / Tight, accurate summary of the four guards that a scrolling dev will instantly recognize as the P0 throttle ticket.

### doomscrum-006-throttle · deepseek/deepseek-v4-flash · p2-natural
- script (18w): Swipe, tap, retry — none of 'em drain your wallet. We cap, dedupe, show cost. Tests pass? Shipped.
- scene: A giant sentient piggy bank with sunglasses, acting as a bouncer at a high-tech nightclub entrance. Red velvet rope, sign 'Max 2 concurrent swipes'. A line of glowing tap gestures tries to push through. Camera wobbles, close-up on piggy bank's stern face as it holds up a receipt. Digital sparkle effects.
- mean overall: 7.5 — The script and scene brilliantly compress a complex P0 throttling spec into a highly visual, easily digestible, and appropriately unhinged 12-second format without losing the core technical requirements. / Hits the core protections (cap/dedupe/confirm/tests) in punchy spoken form but stays too high-level for a dev to grasp the exact ticket scope.

### doomscrum-006-throttle · google/gemini-3-flash-preview · p2-natural
- script (19w): No more fork-bombing the server. We are capping concurrent agents and forcing a price confirm before every single render.
- scene: A hyper-realistic, giant talking strawberry with human teeth and designer sunglasses sits in a high-back velvet chair. The camera shakes violently with a glitch effect every few seconds as the strawberry leans in closer to the lens, whispering with intense, unhinged urgency in a dimly lit neon basement.
- mean overall: 7.5 — The visual concept delivers excellent unhinged energy, and while the dialogue lacks brainrot slang, it perfectly and concisely communicates the most critical technical requirements of the spec within the strict word limit. / Hits the two core controls but omits dedupe, daily budget, and tests, leaving the ticket only partially communicated.

### doomscrum-006-throttle · minimax/minimax-m2.5 · p2-natural
- script (26w): We finally capped compute so you can't fork-bomb the system. And we show you the cost before any render — you gotta confirm, then we spend.
- scene: A panicked tech bro in a server room surrounded by smoking GPUs screams at his phone while a giant red shield blocks his swipe attempts. Smoke pours from the machines as he tries to spawn infinite agents. A bureaucratic hand slides a digital invoice across the screen labeled ESTIMATED COST with a giant CONFIRM button. He slams it, the shield cracks, render begins. High-energy chaos, rapid cuts.
- mean overall: 6.5 — While the visual chaos delivers excellent brainrot energy, the script acts more like a product announcement than a technical spec, missing critical oracle requirements like deduplication, daily budgets, and integration tests. / Hits the core throttle+confirm requirements accurately but omits dedupe/budget details and feels slightly too calm for true brainrot delivery.

### doomscrum-006-throttle · moonshotai/kimi-k2.5 · p2-natural
- script (17w): You can spam the button all day. Two jobs max, no double billing, price tag before pixels.
- scene: A jittery anthropomorphic leather wallet with googly eyes and a mouth where the billfold opens, standing in a server room with red warning lights and sparks raining from the ceiling, clutching a receipt in one leather flap and holding up two fingers with the other while sweating actual dollar coins.
- mean overall: 8.0 — The script brilliantly condenses complex concurrency and billing requirements into punchy, highly speakable dialogue, though the visual carries most of the actual unhinged 'brainrot' aesthetic. / Punchy and mostly faithful, but compresses daily budget and tests into a single memorable line.

### doomscrum-006-throttle · moonshotai/kimi-k2.6 · p2-natural
- script (20w): No rogue swipe nukes our bank account. Every spending path gets throttled and blocked before it burns a single cent.
- scene: A sentient overclocked toaster in a grease-stained necktie vibrates violently inside a server rack, clutching a fire extinguisher in one slot and a leaking bi-fold wallet in the other while red alarm strobes paint the rows of sparking cables behind it in panic.
- mean overall: 4.0 — While the visual concept delivers excellent absurdist energy and the script is perfectly timed, it completely fails to communicate the actionable acceptance criteria like concurrency caps, deduplication, and UI confirmation. / Vague money-protection line misses every concrete requirement and could describe any spend ticket.

### doomscrum-006-throttle · openai/gpt-5.4-mini · p2-natural
- script (17w): Swipes queue at two. Duplicates hand back the same receipt. Real render batches need cost confirmation first.
- scene: A manic cyber-mushroom accountant in a neon server cave points at floating queue tickets while a tiny robot cashier stamps receipts. The camera jitters close, then whips to a glowing cost-confirmation kiosk and a crowded loading rail, all drenched in anxious red-and-blue UI light.
- mean overall: 6.5 — While the visual concept brings the manic energy and the script is highly actionable, the spoken dialogue itself is just plain-English corporate copy devoid of brainrot flavor, and it misses the daily budget requirement. / Hits the three core controls accurately but stays too flat and fragmented for strong brainrot delivery.

### doomscrum-006-throttle · z-ai/glm-5 · p2-natural
- script (17w): Cap concurrent runs, block duplicates, and enforce daily budgets so no swipe or tap drains the wallet.
- scene: A neurotic sentient ATM with googly eyes and tiny sweating arms, hyperventilating behind a velvet rope while glowing dollar-sign-shaped people try to crowd-surf past. Neon BUDGET EXCEEDED signs flash overhead. Camera whips around chaotically, zooming dramatically into the ATM's panicked expression as it frantically stamps DENIED on foreheads.
- mean overall: 8.0 — The script perfectly distills the core technical requirements into a highly speakable format, though it relies entirely on the chaotic visuals to provide the brainrot energy rather than incorporating unhinged slang into the dialogue. / Hits the core controls accurately and clearly in natural speech, but lacks the unhinged energy needed for brainrot delivery.

### doomscrum-006-throttle · deepseek/deepseek-v4-flash · p3-persona
- script (20w): No swipe, no tap drains my wallet. Two dispatches max. No duplicates. Daily render budget. Confirm cost. Tested. Bank protected.
- scene: A Frugal Goblin stands in a dimly lit cave cluttered with server racks and a glowing monitor showing a dashboard with 'max_concurrent_dispatches: 2' and 'budget remaining'. The goblin wears armor made of expired credit cards and a helmet from a cash register. He holds a calculator in one hand and a price tag as a sword in the other. Camera shakes with a dutch angle, close-up on his paranoid face as he eyes the monitor suspiciously.
- mean overall: 7.5 — While the visual concept is wonderfully unhinged, the script itself is just a robotic, fragmented reading of the Jira ticket that lacks true brainrot slang or natural character voice. / Tight, accurate hit on the four core controls plus tests; goblin flavor sells it without diluting the spec.

### doomscrum-006-throttle · google/gemini-3-flash-preview · p3-persona
- script (19w): Listen up. No more money bleeding. Cap the dispatches, dedupe the runs, and show costs before we spend anything.
- scene: A high-octane 90s infomercial pitchman with an oversized suit and sweaty forehead is screaming into a fish-eye lens. He is standing in a dark warehouse filled with gold coins and server racks. Red emergency lights flash behind him as he gestures wildly with a rolled-up contract. The camera shakes violently with every word he shouts, creating a sense of extreme financial urgency.
- mean overall: 7.0 — The visual concept brings chaotic energy and the script is highly speakable and clear, but the dialogue lacks true brainrot vocabulary and misses the daily budget and testing requirements. / Hits the three core protections with urgency but omits per-day budget and tests, keeping fidelity and clarity solid while brainrot stays restrained.

### doomscrum-006-throttle · minimax/minimax-m2.5 · p3-persona
- script (26w): Listen, you pathological swipers—I'm PROTECTING the bank account now. Concurrent capped at TWO. Dedupe active runs. Per-day budgets. You CLICK, you CONFIRM cost. That's the LAW.
- scene: A frantic, paranoid fruit-thing (imagine an orange in a EVIL DEAD nightmare) stands in a dimly lit server room, gesturing wildly at invisible controls. It wears a tattered accountant's visor and speaks directly to camera with the urgency of someone preventing apocalyptic overspend. Camera shakes with each warning. Background shows flickering server lights and a countdown timer.
- mean overall: 8.0 — The script perfectly translates the core technical requirements into unhinged, high-energy dialogue, though it slightly exceeds the 20-word limit for a 12-second window. / Hits the core controls with manic energy but slightly exceeds ideal word count and omits tests/queueing detail.

### doomscrum-006-throttle · moonshotai/kimi-k2.5 · p3-persona
- script (16w): Two max queue! Dupe gets old ticket! Daily 429! Confirm cost! Routes tested! Bank protected, capisce?
- scene: A hyper-masculine anthropomorphic cannoli wearing aviator sunglasses and a tracksuit stands guard at a velvet rope blocking a glowing server rack entrance. He aggressively flashes two fingers at the camera, catches a swirling duplicate request and crumples it while shoving a pre-existing paper receipt forward, points to a neon "429 DAILY LIMIT" sign flashing red, and holds out his palm demanding confirmation. Cyberpunk alleyway background, vertical handheld camera with motion blur, dramatic blue and orange lighting, steam rising from the pastry's ends.
- mean overall: 9.0 — The script flawlessly translates every acceptance criterion into an unhinged, highly memorable cyberpunk cannoli bouncer persona without dropping a single technical requirement. / Tight, on-spec delivery that hits every requirement without fluff or invention.

### doomscrum-006-throttle · moonshotai/kimi-k2.6 · p3-persona
- script (19w): Listen! Two max then queue! Duplicate? Receipt returned! Daily max? Four two nine! Confirm cost! Test four HTTP routes!
- scene: A sweating, desperate 90s infomercial host with a reddening face and rolled-up striped shirtsleeves stands inside a kitchen that has been violently merged with a server room—LEDs blink inside chrome toasters and refrigerator racks vent hot air. He jabs a karate-chop motion toward floating translucent UI cards that wobble on fishing wire, his eyes bulging at the vertical handheld camera which is framed uncomfortably close and shaky, as if the operator is running from an invoice. Harsh blue-screen lightning flickers behind him every time he shouts a specification, casting long shadows across dangling ethernet cables and a single giant red button labeled RENDER.
- mean overall: 8.0 — The script achieves perfect spec fidelity by hitting every oracle requirement exactly, though its manic, staccato delivery sacrifices a bit of standalone clarity for pure unhinged energy. / Punchy and faithful to every requirement, but the shorthand fragments risk losing a scrolling dev on first listen.

### doomscrum-006-throttle · openai/gpt-5.4-mini · p3-persona
- script (22w): I’m Count Budgeto! Two dispatches max, queue the rest, dedupe repeats, 429 on daily spend, confirm render cost first—tests prove all four!
- scene: A manic 90s infomercial vampire accountant in a neon server-room, clutching a calculator and golden receipt scrolls; split-screen UI overlays show queued swipes, receipt reuse, a 429 warning, and a confirmation modal, with fast zooms and dramatic finger-pointing.
- mean overall: 9.0 — The script flawlessly translates every acceptance criterion into a punchy, high-energy format that perfectly balances technical accuracy with unhinged short-form aesthetics. / Tight, high-fidelity hit on all four requirements delivered in manic character voice.

### doomscrum-006-throttle · z-ai/glm-5 · p3-persona
- script (19w): TWO at a time—dupes get old receipts! Daily cap? 429 with reset. Confirm cost BEFORE spend. I've SEEN things.
- scene: A tall shadowy cryptid that's half-Mothman half-ATM machine, standing in foggy woods at night, glowing red dollar-sign eyes, receipt paper unspooling from its beak, one feathery claw pointed accusingly at camera, ominous green mist, vertical close-up, horror-movie underlighting.
- mean overall: 8.5 — The script brilliantly condenses complex rate-limiting and deduplication requirements into a frantic, perfectly timed cryptid warning that a scrolling developer will instantly understand. / Hits the four core controls with punchy unhinged delivery that a scrolling dev will actually retain.

### doomscrum-016-arbitrary-repos · deepseek/deepseek-v4-flash · p1-production
- script (20w): Point DoomScrum at any repo. Pick it, sync it, watch specs become clips. Swipe to dispatch agents. No restart needed.
- scene: A hyper-caffeinated 90s infomercial host in a neon tracksuit stands in front of a giant, glowing repo tree. He gestures wildly at a floating cursor that clicks a repo path, then a feed of spec clips rains down. He swipes right and a tiny agent jumps out of the screen, landing on a branch.
- mean overall: 7.0 — The script perfectly distills the core epic into a punchy, highly clear 12-second read, but completely relies on the visual scene for its 'brainrot' energy since the dialogue itself is just dry technical copy. / Script hits the core UX flow accurately and clearly but stays too generic and flat to feel like brainrot or fully distinguish the ticket.

### doomscrum-016-arbitrary-repos · google/gemini-3-flash-preview · p1-production
- script (19w): Input any repo with markdown specs. UI picker switches repos instantly. Swipe right to dispatch agents and open PRs.
- scene: A hyper-realistic giant blue lobster wearing a gold chain and a gaming headset sits in a dark gamer room illuminated by purple neon. The camera is shaky, zoomed in close on the lobster's face as it frantically clicks a tiny mouse with its claw. The editing is erratic with sudden zooms, flashes of vine thud sound effects, and floating 3D spinning skulls in the background.
- mean overall: 7.0 — While the visual scene is perfectly unhinged, the spoken script is literally just dry technical copy wearing a brainrot costume. / Hits the core picker + foreign dispatch requirements accurately but stays dry and list-like instead of brainrot delivery.

### doomscrum-016-arbitrary-repos · minimax/minimax-m2.5 · p1-production
- script (22w): Markdown folder = valid repo. Pick any local path, feed switches instantly. Right swipe dispatches agents to THAT repo's remote. No bleed.
- scene: A smooth-talking celestial being sits behind a floating holographic desk in the void of space, like a futuristic talent agent or cosmic matchmaker. They gesture expansively at floating repository icons orbiting them. The camera is cinematic, slowly pushing in as they deliver the pitch with religious fervor. Ambient cosmic particles drift in background. They wear an elegant workspace robe.
- mean overall: 6.0 — While the script perfectly distills the technical requirements into a concise summary, it completely fails the brainrot prompt by delivering dry bullet points instead of unhinged internet slang. / Hits the core claims accurately but reads as dry bullet points rather than engaging 12-second delivery.

### doomscrum-016-arbitrary-repos · moonshotai/kimi-k2.5 · p1-production
- script (20w): Plug DoomScrum into any repo. Pick path, sync feed, swipe dispatch. Done when foreign PRs open and states stay isolated.
- scene: A photorealistic pepperoni pizza slice with muscular human arms wearing Versace sunglasses and gold chains, standing on a marble balcony overlooking a cyberpunk skyline made entirely of glowing server racks, gesturing emphatically at floating holographic git branches while holding a martini glass filled with swirling markdown files, neon pull request notifications reflecting off its oily cheese surface.
- mean overall: 7.0 — While the visual concept is wonderfully unhinged, the script itself is just a robotic, fragmented summary of the acceptance criteria with zero actual brainrot energy in the spoken delivery. / Tight, accurate summary of the picker/dispatch/isolation core but reads like clipped notes rather than brainrot delivery.

### doomscrum-016-arbitrary-repos · moonshotai/kimi-k2.6 · p1-production
- script (20w): Plug DoomScrum into any repo. UI picker syncs feed. Swipe right to branch and PR. Separate state. MCP source added.
- scene: A giant neon stapler in a trench coat and shutter shades paces a supermarket aisle built from living server racks, clutching a holographic credit card that pulses with random GitHub avatars. Shelves overflow with glowing markdown files and sentient mangoes screaming in binary. The camera is a handheld vertical phone operated by a flamingo wearing a cracked Bluetooth headset, drifting between extreme close-ups and fisheye chaos. Harsh magenta and cyan lighting strobes as JSON tumbleweeds roll through the frame.
- mean overall: 6.5 — While the visual scene is top-tier brainrot and the technical accuracy is flawless, the spoken script is just a dry, robotic bulleted list wearing a crazy costume rather than engaging short-form content. / Script hits the main spec points accurately but reads as dry bullet fragments rather than speakable brainrot.

### doomscrum-016-arbitrary-repos · openai/gpt-5.4-mini · p1-production
- script (18w): Point DoomScrum at any repo, sync the backlog, and swiping can dispatch agents into that repo’s own PRs.
- scene: A deadpan Gen-Z cryptid stands in a neon command center shaped like a filing cabinet, holding a phone selfie-style while holographic repo folders orbit like angry satellites. The camera jitters like a doomed livestream; behind them, two distinct codebases glow in separate glass terrariums, with one being selected from a floating UI picker as feed cards slam into view. The vibe is absurdly urgent, part startup demo, part occult ritual, with a smug little branch-and-PR gremlin stamping paperwork in the background.
- mean overall: 7.0 — While the visual direction is wonderfully unhinged, the spoken script is just a dry feature summary in a costume, though it perfectly and concisely communicates the core epic. / Covers the core goal accurately but stays too generic and corporate to land as brainrot or make the ticket unmistakable.

### doomscrum-016-arbitrary-repos · z-ai/glm-5 · p1-production
- script (13w): Point at any repo. Pick in UI. Swipe dispatches there. Each repo isolated.
- scene: A desperate folder icon sits on a stool on a hyperactive dating show called GitMatch. The host, a glittery anthropomorphic merge conflict with too many arms, frantically presents repo profiles on a vertical screen. Neon hearts explode. A studio audience of floating file extensions screams. The folder swipes right and gets instantly married to a repo.
- mean overall: 6.5 — While the visual concept is wonderfully unhinged and the script accurately summarizes the core requirements, the spoken words are robotic fragment soup that completely fail to match the brainrot energy of the scene. / Core actions land accurately with unhinged visuals but the spoken lines feel like bullet fragments rather than natural 12-second speech.

### doomscrum-016-arbitrary-repos · deepseek/deepseek-v4-flash · p2-natural
- script (12w): Pick a repo, sync specs, swipe to dispatch agents. No restart needed.
- scene: A cartoon repo folder with a single glowing eye and a branch for a right arm stands in a command-line void. It grins and gestures to a feed of spec cards that appear. When it swipes right on one, a worktree sprouts and a PR icon appears. Neon glitch effects pulse with each action.
- mean overall: 4.5 — The script perfectly distills the core technical requirements into a highly clear and speakable format, but it completely lacks the requested brainrot energy, reading exactly like a dry Jira summary rather than unhinged short-form content. / Script is too generic to convey the specific DoomScrum repo-picker epic.

### doomscrum-016-arbitrary-repos · google/gemini-3-flash-preview · p2-natural
- script (19w): Point DoomScrum at any repo to turn its markdown into a brainrot feed and swipe to ship code instantly.
- scene: A hyper-realistic, giant talking eyeball with a human mouth floats inside a neon-lit computer server rack. The camera shakes violently with high-energy glitch transitions as the eyeball stares directly into the lens, blinking rapidly while its iris changes colors like a strobe light.
- mean overall: 4.5 — While the visual concept is suitably unhinged and the script fits the time limit perfectly, the dialogue reads like a high-level product marketing pitch rather than communicating the specific technical tasks (UI picker, MCP source, dispatch verification) the developer actually needs to build. / Generic one-liner captures the surface goal but conveys none of the actual P1 scope, children, or verification criteria.

### doomscrum-016-arbitrary-repos · minimax/minimax-m2.5 · p2-natural
- script (20w): Bro, DoomScrum now works with ANY repo. Select it, sync, your backlog appears. Right swipe dispatches back to that repo.
- scene: A hyperactive tech founder in a cramped home office, surrounded by LED lights and monitors, gestures wildly at a laptop screen while a fidget spinner spins on the desk. He's doing a chaotic demo for nobody, pointing at a repo picker UI and swiping aggressively, his voice cracking with manic energy. The camera shakes slightly, capturing the raw intensity of someone solving a very specific problem.
- mean overall: 7.5 — The script perfectly distills the core technical requirements into a highly speakable 20-word format, but it relies entirely on the visual scene for its 'brainrot' energy rather than incorporating genuinely unhinged vocabulary. / Hits the core picker+dispatch flow in manic style but stays too generic to convey the actual epic scope or contract details.

### doomscrum-016-arbitrary-repos · moonshotai/kimi-k2.5 · p2-natural
- script (16w): DoomScrum doesn't care whose repo. Point, sync, then swipe right to spawn agents in foreign code.
- scene: A trenchcoat-clad raccoon perched atop a swaying tower of blinking hard drives in a purple fog server room, clutching a glowing smartphone like a detonator. Camera orbits low and fast, catching the reflection of repo diagrams in the raccoon's wide, manic eyes.
- mean overall: 7.0 — The script perfectly distills the epic's core user flow into a punchy, highly speakable hook paired with unhinged, attention-grabbing visuals. / Punchy and on-theme but too vague to convey the actual epic scope or deliverables.

### doomscrum-016-arbitrary-repos · moonshotai/kimi-k2.6 · p2-natural
- script (20w): Pick any repo, the feed swaps instantly with no restart. Swipe right and DoomScrum opens a PR on that repo.
- scene: A hyperactive capybara in mirrored aviators and a stained 'Tech Lead' vest stands behind a convenience-store counter built from stacked hard drives, furiously swiping at floating holographic repo icons with its paws; the camera jolts with handheld vertical energy, neon freezer lights flicker overhead, and a single microwave burrito spins under the counter glow.
- mean overall: 7.0 — The script perfectly distills the core UI and dispatch requirements into exactly 20 highly speakable words, backed by a wonderfully unhinged and visually stimulating scene. / Script captures the picker+swipe core but stays too generic to convey the actual epic or backlog contract.

### doomscrum-016-arbitrary-repos · openai/gpt-5.4-mini · p2-natural
- script (19w): Pick any repo, and DoomScrum snaps to its specs. Swipe it, and agents launch against that repo for real.
- scene: A feral raccoon CTO in a neon hoodie stands inside a rotating hologram of file trees, jabbering at a floating phone-camera in a cluttered midnight garage. Behind them, two glowing repo portals swap places as the camera whip-pans with manic energy, with tiny task cards streaming out like confetti from an overcaffeinated slot machine.
- mean overall: 7.5 — The script perfectly distills the core goal into a highly speakable bite, though it relies entirely on the visual scene for its brainrot flavor rather than using unhinged vocabulary. / Script nails the core picker+dispatch flow without fluff or invention but stays too generic for instant ticket recognition.

### doomscrum-016-arbitrary-repos · z-ai/glm-5 · p2-natural
- script (16w): Point at any repo, hit sync. Boom - your feed shows those specs, swipe to dispatch.
- scene: A manic sentient git branch creature with googly eyes bounces frantically between floating translucent folder icons in a neon void. Camera whip-pans and zooms with chaotic energy as it desperately waits to be picked.
- mean overall: 5.5 — While the visual concept brings chaotic energy and the script perfectly distills the core goal into a highly speakable format, the spoken dialogue itself is just standard product copy lacking any actual brainrot flavor. / Too generic to convey the actual epic or its children; manic visuals cannot compensate for missing contract, foreign dispatch, and MCP specifics.

### doomscrum-016-arbitrary-repos · deepseek/deepseek-v4-flash · p3-persona
- script (18w): Pick any repo. Sync. Watch specs become feed clips. Swipe to dispatch. DoomScrum makes any backlog your feed.
- scene: A stack of three paper backlog tickets, each with googly eyes and a tiny top hat, sits on a messy wooden desk. A vintage CRT monitor glows blue behind them, showing code. Off-screen hands place a repo folder on the desk. Camera is handheld, slightly shaky, zooming in slowly.
- mean overall: 5.5 — While perfectly timed and aligned with the epic's high-level goal, the script is a corporate marketing pitch devoid of brainrot energy that fails to communicate the actual technical deliverables to the developer. / Script nails the core actions and goal with high fidelity but lacks any brainrot flavor or unhinged delivery.

### doomscrum-016-arbitrary-repos · google/gemini-3-flash-preview · p3-persona
- script (19w): Listen paisan! Point DoomScrum at any repo. Pick the path, sync the feed, swipe right to dispatch. No bleed!
- scene: A hyper-active Italian chef with glowing neon skin and oversized sunglasses screams into a fisheye lens. He is standing in a futuristic kitchen where giant floating hard drives are being sliced like salami. The camera shakes violently with every hand gesture, capturing high-contrast colors and rapid zoom-ins on his mouth.
- mean overall: 8.0 — The script perfectly distills the core epic into a punchy, highly speakable hook, capturing the exact UI flow and state constraints while sacrificing only the secondary child ticket details to fit the strict time limit. / Hits the arbitrary-repo + no-bleed core cleanly but stays too generic for a dev to recognize the exact picker/contract/dispatch epic.

### doomscrum-016-arbitrary-repos · minimax/minimax-m2.5 · p3-persona
- script (20w): Your backlog tool works on ONE repo? NOT ANYMORE. Pick ANY repo. Swipe dispatch. Per-repo state. This is the WAY.
- scene: A 90s infomercial host in a shiny suit stands behind a podium in a studio with dramatic red curtains. He gestures wildly at a floating holographic repo icon that multiplies into many. He slams a binder labeled 'DOOMSCRUM' on the podium, points aggressively at the camera, his eyes wide with disbelieving enthusiasm. Confetti cannons in the background.
- mean overall: 6.0 — 'NoneType' object has no attribute 'strip' / Hits the multi-repo picker/dispatch/state points but stays too slogan-like to clearly convey the actual epic.

### doomscrum-016-arbitrary-repos · moonshotai/kimi-k2.5 · p3-persona
- script (19w): Choom! DoomScrum devours ANY repo. Picker UI. Syncs foreign specs. Swipe dispatches to their remotes. Isolated state. Zero restart.
- scene: A weathered cyber-archaeologist from 3024 wearing a flickering neon trench coat stands in an acid-rain alley clutching an ancient hard drive engraved with "OLYMPOSSS", surrounded by floating translucent git branch diagrams that spark when touched. Extreme close-up vertical shot focuses on their single bloodshot organic eye reflecting cascading repository trees, while their rusted cybernetic jaw servo-whirs with each syllable. Harsh magenta rim lighting pulses from a dying neon sign reading "MCP SOURCE DETECTED", and the camera shakes with intentional analog glitch artifacting as holographic backlog specs rain down like digital ash.
- mean overall: 6.5 — While the visual scene is a cyberpunk masterpiece and the script perfectly compresses the acceptance criteria, the spoken dialogue is just a robotic list of bullet points rather than engaging brainrot content. / Hits the core picker/sync/foreign-dispatch/isolated claims but reads as bullet fragments rather than speakable 12s copy.

### doomscrum-016-arbitrary-repos · moonshotai/kimi-k2.6 · p3-persona
- script (20w): Mamma mia! Pick any repo, sync no restart. Swipe right, branch and PR. State isolated. Apollo MCP reads specs. Done.
- scene: A sweating, anthropomorphic plate of spaghetti wearing a tracksuit made of ethernet cables and a gold chain, standing inside a blinking server rack lit like a Roman colosseum at midnight. He holds a glowing smartphone like a sacrament. Extreme vertical close-up, fisheye lens, harsh RGB neon, shaky handheld energy, lens flares from cascading LED strips.
- mean overall: 5.0 — While the visual concept is wonderfully unhinged and the script perfectly captures the technical requirements, the dialogue itself is just a robotic checklist of fragments rather than natural, engaging short-form content. / Hits the main beats but is too fragmented and cryptic to clearly communicate the ticket to a scrolling dev.

### doomscrum-016-arbitrary-repos · openai/gpt-5.4-mini · p3-persona
- script (24w): I’m Señor Mango, the repo witch—pick a path, sync it, and I’ll feed on that backlog. Foreign swipe? Branch, worktree, PR, all from repo.path.
- scene: A flamboyant talking mango in a velvet blazer stands inside a neon-soaked command center built from stacked code editors and file trees. The camera is handheld, fast, and slightly chaotic, punching in on the fruit’s face as it gestures toward two glowing repo panels labeled as different local paths. Behind it, feed cards slide in like soap-opera credits, then a swipe animation forks into a branch, a worktree, and a PR window on a distant remote. The vibe is loud, dramatic, and absurdly confident, like a midnight demo for software goblins.
- mean overall: 8.0 — Señor Mango successfully distills the core repo-picker and foreign-dispatch requirements into a punchy, unhinged hook, though it slightly exceeds the 20-word guideline. / Script nails the picker/sync + foreign-repo dispatch core with specific repo.path language and strong character energy, though it omits contract/MCP details.

### doomscrum-016-arbitrary-repos · z-ai/glm-5 · p3-persona
- script (12w): Point any repo. Sync. Swipe right — agents dispatch THERE. No restart!
- scene: A frantic weatherman in front of a green screen showing repo territory maps, wind blowing papers everywhere, gesturing at glowing folder icons like approaching storm systems, breaking-news urgency, camera shaking with chaos energy.
- mean overall: 8.5 — The script brilliantly distills the core user story into a chaotic, highly speakable format, perfectly matching the frantic weatherman visual while nailing the primary acceptance criteria of dynamic repo switching and dispatching. / Tight, urgent script nails the picker+foreign-dispatch core without fluff or invention, though it skips the contract and MCP children.

### olympus-072-pi-openrouter · deepseek/deepseek-v4-flash · p1-production
- script (21w): We need Pi running through OpenRouter. Not as the only path—just a canary. Prove it works, prove costs, prove safe rollback.
- scene: A sentient slice of pizza with sunglasses, perched on a server rack in a dimly lit 1990s data center. The camera is shaky, zooming in on its floppy pepperoni. It points a greasy crust at a glowing CRT monitor showing green terminal text. A single ring binder labeled 'ROLLBACK PLAN' sits open beside a vent.
- mean overall: 7.0 — While the script perfectly captures the technical spec with high clarity, it completely lacks the requested brainrot energy in the spoken dialogue, relying entirely on the visual scene for its unhinged vibe. / Script accurately distills the canary-first Pi/OpenRouter goal but stays too dry for strong brainrot delivery.

### olympus-072-pi-openrouter · google/gemini-3-flash-preview · p1-production
- script (19w): Add Pi to the runtime registry. Use OpenRouter. No default flips until cost controls and trace evidence are real.
- scene: A man in a cheap hot dog suit stands in a dark, rain-slicked alleyway. He is eating a glowing blue popsicle while filming a shaky vertical vlog. The camera zooms in and out violently on his face as he whispers intensely. Neon signs in the background flicker with the words NO VENDOR LOCK IN. He occasionally looks over his shoulder as if being hunted by a giant paperclip.
- mean overall: 8.5 — A flawless execution that delivers exact technical requirements through a perfectly unhinged visual contrast. / Accurate core message but dry delivery that relies entirely on the scene for any brainrot value.

### olympus-072-pi-openrouter · minimax/minimax-m2.5 · p1-production
- script (20w): Meet Pi — the new contestant. Don't get comfortable, Codex. This one's just a canary until it earns the throne.
- scene: A boxing ring under neon lights. A sequined announcer holds a microphone between two corners. Left: Codex, the weary champion in a worn robe, arms crossed. Right: Pi emerges from a glowing portal labeled OPENROUTER, wearing a shiny new tracksuit, waving to a nonexistent crowd. The announcer gestures dramatically at both as the camera zooms out.
- mean overall: 5.5 — While highly speakable and accurate to the high-level goal of making Pi a canary against Codex, the script relies on a generic boxing metaphor instead of true brainrot and omits the crucial technical constraints of the adapter implementation. / Captures the canary intent but is too vague to communicate the actual ticket to a scrolling dev.

### olympus-072-pi-openrouter · moonshotai/kimi-k2.5 · p1-production
- script (14w): Add Pi OpenRouter adapter. Default flip gated on cost controls and trace evidence ship.
- scene: A sweating 90s infomercial host with a metallic Pi symbol for a head stands in a server room that flickers between a neon shopping channel set and a glitching cloud dashboard. He frantically gestures between a rusty lever labeled "CODEX" and a glowing vault door marked "PI DEFAULT" secured by two massive padlocks engraved with "069" and "070", while holding a caged canary and pointing at floating holographic backlog tickets.
- mean overall: 7.5 — While the visual scene is wonderfully unhinged and packed with accurate spec lore, the spoken script is just a dry reading of the ticket summary, sacrificing vocal brainrot energy for pure clarity. / Dry but faithful 9-word summary that correctly flags the adapter addition and explicit gates without inventing scope.

### olympus-072-pi-openrouter · moonshotai/kimi-k2.6 · p1-production
- script (20w): Add Pi over OpenRouter. Keep it a canary. Default flip only after cost watchdog, trace fallback, and comparison evidence land.
- scene: A genetically-modified mozzarella stick with human hands and designer sneakers filming a TikTok dance on the hood of a self-driving golf cart inside a zero-gravity laundromat, occasionally pointing to a holographic gantt chart projected from its eyebrows while a chorus of crying mannequins applaud in the background.
- mean overall: 7.5 — Flawless technical accuracy and clarity, but the script relies entirely on the visual scene for its brainrot energy rather than integrating the unhinged tone into the dialogue. / Accurate high-level summary of the Pi canary plan that a scrolling dev would understand, but reads as terse corporate shorthand rather than spoken brainrot.

### olympus-072-pi-openrouter · openai/gpt-5.4-mini · p1-production
- script (19w): Add Pi via OpenRouter as the canary runtime. No default flip until parser, readiness, trace, and cost proof land.
- scene: A frantic deadpan gen-z ops gremlin stands in a neon server closet, holding a clipboard like a sacred relic. Behind them, an absurd wall of blinking model-route cables and a giant slot-machine labeled “RUNTIME REGISTRY” spits out tiny paper receipts. The camera jitters in tight vertical close-up, then smash-cuts to a fake dashboard where one lane glows suspiciously gold while the others stay grim and bureaucratic. The vibe is part infomercial, part incident review, part cult ceremony, with a single overhead light making every decision feel terrifyingly expensive.
- mean overall: 7.5 — While the visual scene brings excellent chaotic energy, the spoken script is a perfectly accurate but completely dry corporate summary that forgets to actually use brainrot vocabulary. / Accurate but dry capsule that states the canary rule without enough unhinged flavor for the format.

### olympus-072-pi-openrouter · z-ai/glm-5 · p1-production
- script (15w): Add Pi adapter. Canary only. Default flip blocked until cost controls and trace evidence pass.
- scene: A sentient git commit with googly eyes and tiny foam nervously clutches a 'CANARY LANE' VIP pass while a massive CI/CD bouncer in sunglasses guards the velvet rope to Club Production Default. The bouncer holds an endless scroll labeled 'ROLLOUT_READINESS' with red BLOCKED stamps next to 'cost watchdog' and 'trace fallback.' Strobe lights pulse. Other commits wait in line behind a velvet rope that stretches into the void.
- mean overall: 7.5 — The script delivers flawless spec accuracy and clarity, though it relies entirely on the unhinged visual scene to provide the requested brainrot energy. / Precise gated-canary summary that a dev would understand but lacks any brainrot spark or natural delivery.

### olympus-072-pi-openrouter · deepseek/deepseek-v4-flash · p2-natural
- script (17w): Adding Pi via OpenRouter as first non-Codex canary. Don't flip defaults until cost and trace gates pass.
- scene: A slice of cherry pie with a tiny bow tie stands on a whiteboard covered in flowcharts. It gestures animatedly with a tiny hand, speaking directly to the camera. The whiteboard has 'Codex' crossed out and 'Pi canary' circled. The pie's eyes are wide and enthusiastic.
- mean overall: 6.5 — While the script perfectly summarizes the technical constraints of the spec, it completely lacks the requested brainrot energy, delivering dry corporate copy instead of unhinged short-form content. / Accurate core message but reads like a commit message, not brainrot delivery.

### olympus-072-pi-openrouter · google/gemini-3-flash-preview · p2-natural
- script (19w): Stop hardcoding your providers. Integrate the Pi adapter through OpenRouter, but dont flip the default until cost controls land.
- scene: A hyper-realistic CGI squirrel wearing a neon tactical vest and a VR headset is standing in a digital forest of glowing green code. The squirrel is frantically scrolling through a holographic terminal with its tiny paws, looking directly into the lens with twitchy, wide-eyed energy as the camera shakes and zooms in on its face.
- mean overall: 8.0 — The script perfectly distills the spec's core constraints into a punchy, highly speakable line, while the tactical VR squirrel scene provides excellent visual brainrot. / Accurately distills the canary-not-default intent but stays too corporate for brainrot delivery.

### olympus-072-pi-openrouter · minimax/minimax-m2.5 · p2-natural
- script (16w): Pi OpenRouter is the canary. Default flip waits on cost controls and live trace evidence landing.
- scene: A sweaty tech lead in a generic startup hoodie stands in front of a chaotic whiteboard covered in sticky notes. They're physically holding back a giant red DEFAULT FLIP button with both hands while a tiny animated pi symbol dances nervously on a monitor nearby. The camera shakes dramatically as they whisper-shout the verdict.
- mean overall: 7.0 — Flawless technical accuracy and clarity, but the spoken script is just a dry Jira summary that relies entirely on the visual scene to provide any unhinged energy. / Accurate core message but too terse and dry to land as either clear spec or brainrot content.

### olympus-072-pi-openrouter · moonshotai/kimi-k2.5 · p2-natural
- script (20w): Pi's live in the registry but keep Codex as default until cost controls ship. Sixty-nine and seventy block the switch.
- scene: A raccoon site-reliability engineer wearing a traffic-cone hard hat and a neon high-vis vest stands inside a server cage that looks like a giant birdcage. It grips a massive glowing key labeled "DEFAULT" but refuses to insert it into the lock, instead jabbing a claw at two heavy red padlocks chained to the bars marked "069" and "070". Behind the raccoon, a translucent blue hologram representing the Pi runtime hovers inside a registry terminal, while a sturdy golden hamster wheel labeled "Codex" keeps spinning in the background. The camera is shaky vertical phone footage with aggressive zoom-ins on the padlocks whenever the raccoon screeches.
- mean overall: 8.0 — The script perfectly nails the technical constraints and blockers in exactly 20 words, though the spoken dialogue relies entirely on the unhinged visual scene for its brainrot flavor. / Correctly flags the 069/070 gate but too terse and low-energy to land as memorable brainrot.

### olympus-072-pi-openrouter · moonshotai/kimi-k2.6 · p2-natural
- script (20w): Pi's entering through OpenRouter as a canary, but we are gatekeeping the default until cost controls and trace fallback ship.
- scene: A literal canary wearing tiny noise-canceling headphones and a lanyard that says OPENROUTER, trapped inside a rolling plastic hamster ball bouncing through a server room of blinking pink and black racks, while a giant glowing red button labeled DEFAULT sits behind laser grids in the background. The bird stares directly into the vertical phone camera with frantic, caffeinated energy, flapping its wings as if trying to explain something urgent.
- mean overall: 8.0 — 'NoneType' object has no attribute 'strip' / Tight, accurate encapsulation of the canary-first gating rule that a scrolling dev will instantly parse.

### olympus-072-pi-openrouter · openai/gpt-5.4-mini · p2-natural
- script (20w): Pi shows up through OpenRouter first. Default only flips later, after parser, readiness, observability, cost, and rollback proof all land.
- scene: A feral cyber-mandrill in a neon ops cave livestreams from a floating terminal altar, one claw on a server rack, one on a glowing provider switchboard. The camera jitters like a reckless handheld phone, with holographic model names orbiting overhead and an absurdly serious dashboard behind him showing trace panels, cost counters, and a giant locked default toggle that never flips.
- mean overall: 7.5 — Perfectly accurate and clear, but the script is literally just dry corporate copy wearing a cyber-mandrill costume instead of actual brainrot dialogue. / Script nails the phased canary intent accurately and concisely but lacks unhinged flavor.

### olympus-072-pi-openrouter · z-ai/glm-5 · p2-natural
- script (17w): Pi joins as a canary runtime. No production default flip until cost controls and trace fallback land.
- scene: A panicking hermit crab wearing two mismatched shells—half labeled Codex, half labeled Pi—stands guard over a big red button reading DEFAULT FLIP, claws raised to block it, frantically glancing at two wall certificates labeled COST CONTROLS and TRACE FALLBACK that are still dim; camera whip-pans between the crab's twitching eyestalks and the unlit certificates in erratic zooms.
- mean overall: 8.5 — The script delivers flawless spec accuracy and clarity in a perfectly timed package, though the spoken dialogue relies entirely on the visual scene for its unhinged energy. / Accurate core message with strong visual metaphor but slightly dry delivery.

### olympus-072-pi-openrouter · deepseek/deepseek-v4-flash · p3-persona
- script (19w): Pi via OpenRouter: first canary runtime! No default flip yet. Cost controls and trace evidence required first. Act now!
- scene: A 1990s-style TV studio set with bright neon colors. Randy Rocket, a man in a shiny suit and slicked-back hair, stands behind a table covered in plastic 'runtime adapters'. He holds a large sign reading 'PI/OPENROUTER CANARY'. He gestures wildly to the camera, sweating under hot studio lights. A blinking 'LIVE' sign above. Rapid cuts and zooms, VHS static overlay.
- mean overall: 5.5 — While the script perfectly summarizes the technical constraints of the spec, it completely fails the brainrot requirement by delivering dry corporate bullet points disguised as an infomercial. / Captures the no-default-flip canary constraint accurately but reads as terse notes rather than speakable, high-energy brainrot.

### olympus-072-pi-openrouter · google/gemini-3-flash-preview · p3-persona
- script (19w): Add Pi on OpenRouter now. No default flip until cost controls and trace evidence pass. Agnosticism is life, skibidi.
- scene: A hyperactive 3024 street influencer with glowing cybernetic skin and four arms. They are holding two different futuristic energy drinks. The camera is shaking with high-energy 'phonk' music playing in the background. The character leans into the lens, their eyes flashing different colors with every sentence, filmed in a neon-drenched cyberpunk alleyway with floating holographic advertisements.
- mean overall: 7.0 — The script delivers perfect spec accuracy and clarity within the time limit, though the brainrot relies entirely on the visual scene rather than the dialogue itself. / Script nails the core 'add Pi, gate the flip' constraint but is too terse and low-energy to clearly communicate the full phased spec to a scrolling dev.

### olympus-072-pi-openrouter · minimax/minimax-m2.5 · p3-persona
- script (21w): Pi via OpenRouter - first non-Codex runtime! Canary lane only until cost controls and trace evidence ship. Production defaults stay Codex.
- scene: A 90s infomercial pitchman in a shiny suit stands in a dimly lit studio, holding a giant printed spreadsheet like it's a miracle product. Behind him, a whiteboard reads 'PI + OPENROUTER' in bold marker. He points dramatically at a small cardboard sign that says 'CANARY LANE' while gesturing to another sign reading 'DO NOT FLIP YET.' Studio lights sweep across the set. He has the intensity of a man selling teeth whitener but the gravity of someone explaining critical infrastructure. His catchphrase hand gesture freezes mid-air.
- mean overall: 7.5 — While the script perfectly summarizes the technical constraints and rollout plan, it completely lacks the requested brainrot energy in the dialogue, reading more like a dry standup update than unhinged short-form content. / Tight, accurate delivery of the canary-only constraint but too sober for the requested infomercial brainrot tone.

### olympus-072-pi-openrouter · moonshotai/kimi-k2.5 · p3-persona
- script (20w): Stop! Do not flip Pi default! Cost controls first! Trace views next! Canary mode only! Wait for evidence! Rollback ready!
- scene: A muscular anthropomorphic canary wearing a neon 90s fitness tracksuit and sweatbands stands in a vibrating server room bathed in yellow warning lights. He holds yellow caution tape stretched across a server rack labeled "PRODUCTION DEFAULTS" while pointing aggressively at the camera with intense focus. Sticky notes marked "#069", "#070", and "#073" are plastered across his feathers and surrounding equipment. His beak is open mid-shout with sweat beads flying, rack-mounted servers blinking red behind him, the handheld camera shaking with chaotic energy.
- mean overall: 8.0 — While the script leans heavily on fragmented shouting rather than true brainrot slang, the visual concept is brilliantly unhinged and it perfectly communicates the strict rollout gates of the spec. / Script nails the core 'no default flip until gates' message with high fidelity and chaotic energy but sacrifices some clarity for brevity.

### olympus-072-pi-openrouter · moonshotai/kimi-k2.6 · p3-persona
- script (18w): Pi adapter canary! Defaults locked! Flip only after cost watchdog, trace view, and runtime evidence gates are green!
- scene: A hyper-caffeinated anthropomorphic espresso cup with a cracked ceramic face and steam-wisp arms gestures frantically in a glitched-out Roman piazza at midnight. Ancient cobblestones pulse with green JSON text while three hovering golden pasta plates—each glowing with a backlog ticket number—float behind him, padlocked shut. Vertical handheld camera spins wildly around the cup, low angle, energy like a sprinting street vendor trying to sell the last runtime adapter before the servers melt.
- mean overall: 8.5 — The script perfectly distills the complex rollout constraints into a punchy, highly accurate 12-second sprint, though it relies entirely on the visual scene rather than slang for its brainrot flavor. / Tight, accurate canary message that nails the gated rollout without fluff or invention.

### olympus-072-pi-openrouter · openai/gpt-5.4-mini · p3-persona
- script (23w): Pi, darling—I’m the canary. Add me via OpenRouter, parse my JSON, require OPENROUTER_API_KEY only for me, and never default me until evidence sings.
- scene: A neon-soaked soap-opera birdcage newsroom in vertical frame, featuring a flamboyant talking citrus in a glitter jacket and headset, pacing on a rotating podium. The camera punches in with dramatic zooms and quick cuts as it gestures at floating UI panels labeled parser, env, readiness, trace, cost, and rollback. Mood is urgent but playful, with glossy studio lighting, trembling handheld energy, and a visible “canary” prop perched beside a locked default switch.
- mean overall: 8.5 — The script perfectly distills the technical constraints and rollout strategy into a punchy, highly speakable monologue, though the 'brainrot' relies more on the flamboyant visual concept than the actual dialogue. / Tight, on-spec canary pitch that nails the gated rollout without fluff.

### olympus-072-pi-openrouter · z-ai/glm-5 · p3-persona
- script (19w): Pi, join the registry! Canary lane first. Cost controls and trace view before default flip. Keys never stored, capisce?
- scene: An Italian grandmother in a steam-filled kitchen, brandishing a wooden spoon at the camera with marinara on her apron, bright window light, medium shot, direct confrontation energy, pasta pot bubbling behind her.
- mean overall: 7.5 — The script flawlessly distills the exact technical constraints into a punchy format, though it relies entirely on the visual juxtaposition of an aggressive Italian grandmother for its 'brainrot' flavor rather than using unhinged slang. / Hits the canary-registry gates and key constraint accurately in meme form but leaves the full phased rollout and evidence requirements too implicit for a scrolling dev.
