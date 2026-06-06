import { sha256 } from '../lib/hash';
import type { PrdSource, SpecBrief, Storyboard, StoryboardBeat } from '../shared/types';

function section(raw: string, heading: string): string | undefined {
  const pattern = new RegExp(`^##\\s+${heading}\\s*\\n([\\s\\S]*?)(?=^##\\s+|\\z)`, 'im');
  return raw.match(pattern)?.[1]?.trim();
}

function bullets(raw: string | undefined): string[] {
  if (!raw) {
    return [];
  }
  return raw
    .split('\n')
    .map((line) => line.replace(/^[-*]\s+/, '').trim())
    .filter(Boolean);
}

export function distillPrd(prd: PrdSource): SpecBrief {
  const user = section(prd.raw, 'User') || 'Local operator';
  const goal = section(prd.raw, 'Goal') || prd.title;
  const acceptanceCriteria = bullets(section(prd.raw, 'Acceptance Criteria'));
  const riskNotes = bullets(section(prd.raw, 'Risk'));
  const ambiguityFlags = acceptanceCriteria.length === 0 ? ['No acceptance criteria found.'] : [];
  const extractedClaims = [goal, ...acceptanceCriteria].filter(Boolean);

  return {
    prdId: prd.id,
    goal,
    user,
    acceptanceCriteria,
    ambiguityFlags,
    riskNotes,
    extractedClaims
  };
}

export function compileStoryboard(prd: PrdSource, brief: SpecBrief): Storyboard {
  const beats: StoryboardBeat[] = [
    {
      label: 'Hook',
      specPayload: brief.goal,
      visualPrompt: `A chaotic vertical short where a software PRD bursts into frame like cursed breaking news: ${brief.goal}`,
      caption: `${prd.title} just entered the chat`
    },
    {
      label: 'Stake',
      specPayload: brief.user,
      visualPrompt: `A local operator cockpit gets flooded with markdown files, neon warning stickers, and absurd meme captions.`,
      caption: `User: ${brief.user}`
    },
    {
      label: 'Payload',
      specPayload: brief.acceptanceCriteria[0] || 'Acceptance criteria are missing.',
      visualPrompt: `Fast jump cuts show the first acceptance criterion as giant karaoke subtitles over glitchy terminal chaos.`,
      caption: brief.acceptanceCriteria[0] || 'Needs acceptance criteria'
    },
    {
      label: 'Risk Chaos Check',
      specPayload: brief.riskNotes[0] || 'No explicit risk recorded.',
      visualPrompt: `The clip becomes a fake safety PSA about not letting vague PRDs drive agent work without receipts.`,
      caption: brief.riskNotes[0] || 'Risk: unlisted'
    },
    {
      label: 'Decision',
      specPayload: 'Inspect, skip, needs-spec, or create a bounded run packet.',
      visualPrompt: `End card with four exaggerated swipe choices, all pointing back to the source PRD hash.`,
      caption: 'Swipe like you mean it'
    }
  ];

  const providerPrompt = [
    'Generate a vertical 9:16 shortform video under 60 seconds with native audio.',
    'Tone: goofy, anti-corporate, meme culture, high-energy, but keep all spec claims accurate.',
    `Title: ${prd.title}`,
    `Goal: ${brief.goal}`,
    `Acceptance: ${brief.acceptanceCriteria.join(' | ') || 'No acceptance criteria found'}`,
    'Do not invent shipped features, metrics, customer names, or implementation details.'
  ].join('\n');

  const briefHash = sha256(JSON.stringify(brief));

  return {
    id: sha256(`${prd.sha256}:${briefHash}:brainrot_v0`),
    prdId: prd.id,
    prdSha256: prd.sha256,
    briefHash,
    tone: 'brainrot_v0',
    targetDurationSec: 24,
    aspectRatio: '9:16',
    beats,
    providerPrompt,
    prohibitedClaims: [
      'Do not claim the PRD has already shipped.',
      'Do not claim tests pass.',
      'Do not describe files not present in the PRD.',
      'Do not turn missing acceptance criteria into fake acceptance criteria.'
    ]
  };
}
