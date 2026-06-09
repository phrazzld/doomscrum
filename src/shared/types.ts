export type PrdStatus = 'new' | 'rendered' | 'skipped' | 'needs_spec' | 'run_intent_created';

export type PrdSource = {
  id: string;
  path: string;
  sha256: string;
  title: string;
  repoPath: string;
  allowedCommands: string[];
  agentMode: 'local-codex' | 'packet-only';
  discoveredAt: string;
  status: PrdStatus;
  raw: string;
};

export type SpecBrief = {
  prdId: string;
  goal: string;
  user: string;
  acceptanceCriteria: string[];
  ambiguityFlags: string[];
  riskNotes: string[];
  extractedClaims: string[];
};

export type StoryboardBeat = {
  label: string;
  specPayload: string;
  visualPrompt: string;
  caption: string;
};

export type Storyboard = {
  id: string;
  prdId: string;
  prdSha256: string;
  briefHash: string;
  tone: 'brainrot_v0';
  targetDurationSec: number;
  aspectRatio: '9:16';
  beats: StoryboardBeat[];
  providerPrompt: string;
  prohibitedClaims: string[];
};

export type AudioMode = 'native' | 'silent' | 'fallback_tts' | 'failed';

export type VideoRender = {
  id: string;
  prdId: string;
  prdSha256: string;
  storyboardId: string;
  storyboardHash: string;
  provider: string;
  model: string;
  nativeAudioRequested: boolean;
  audioMode: AudioMode;
  status: 'queued' | 'running' | 'ready' | 'failed';
  assetPath?: string;
  assetUrl?: string;
  providerJobId?: string;
  costEstimateUsd?: number;
  actualCostUsd?: number;
  latencyMs?: number;
  error?: string;
  createdAt: string;
};

export type FeedDecision = {
  id: string;
  prdId: string;
  prdSha256: string;
  renderId?: string;
  decision: 'inspect' | 'skip' | 'needs_spec' | 'run_intent' | 'vibe_rating';
  createdAt: string;
  note?: string;
  metadata?: Record<string, string | number | boolean>;
};

export type AgentRunPacket = {
  id: string;
  prdId: string;
  prdSha256: string;
  repoPath: string;
  objective: string;
  allowedCommands: string[];
  timeoutSec: number;
  budgetUsd?: number;
  branchName: string;
  acceptanceCriteria: string[];
  status: 'created' | 'blocked' | 'launched' | 'completed' | 'failed';
  launch?: AgentLaunchReceipt;
  createdAt: string;
};

export type AgentLaunchReceipt = {
  id: string;
  mode: 'local-codex' | 'dry-run';
  status: 'launched' | 'completed' | 'failed';
  command: string[];
  cwd: string;
  outputPath: string;
  pid?: number;
  exitCode?: number;
  startedAt: string;
  completedAt?: string;
  error?: string;
};

export type BacklogConfig = {
  defaults: {
    repoPath: string;
    allowedCommands: string[];
    agentMode: 'local-codex' | 'packet-only';
    renderProvider: 'fal' | 'fake';
    maxRenderSpendUsd: number;
  };
  items?: Record<string, Partial<Pick<PrdSource, 'repoPath' | 'allowedCommands' | 'agentMode'>>>;
};

export type AppState = {
  prds: PrdSource[];
  storyboards: Storyboard[];
  renders: VideoRender[];
  decisions: FeedDecision[];
  runPackets: AgentRunPacket[];
  providerConfigured: boolean;
  config: BacklogConfig;
};
