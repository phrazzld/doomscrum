import fs from 'node:fs/promises';
import path from 'node:path';
import { appendNdjson, ensureDir, writeJson } from '../lib/fs-utils';
import { eventsPath, runPacketsDir } from '../lib/paths';
import { sha256, shortHash } from '../lib/hash';
import type { AgentRunPacket, FeedDecision, PrdSource, VideoRender } from '../shared/types';

export async function readEvents(): Promise<FeedDecision[]> {
  try {
    const raw = await fs.readFile(eventsPath, 'utf8');
    return raw
      .split('\n')
      .filter(Boolean)
      .map((line) => JSON.parse(line) as FeedDecision);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
      return [];
    }
    throw error;
  }
}

export async function appendDecision(decision: Omit<FeedDecision, 'id' | 'createdAt'>): Promise<FeedDecision> {
  const createdAt = new Date().toISOString();
  const event: FeedDecision = {
    ...decision,
    id: sha256(`${decision.prdId}:${decision.decision}:${createdAt}:${decision.note || ''}`),
    createdAt
  };
  await appendNdjson(eventsPath, event);
  return event;
}

export async function createRunPacket(prd: PrdSource, render: VideoRender | undefined): Promise<AgentRunPacket> {
  await ensureDir(runPacketsDir);
  const createdAt = new Date().toISOString();
  const packet: AgentRunPacket = {
    id: sha256(`${prd.sha256}:run:${createdAt}`),
    prdId: prd.id,
    prdSha256: prd.sha256,
    repoPath: prd.repoPath,
    objective: `Implement the PRD: ${prd.title}`,
    allowedCommands: prd.allowedCommands,
    timeoutSec: 180,
    budgetUsd: render?.actualCostUsd ? Number((render.actualCostUsd * 5).toFixed(2)) : 5,
    branchName: `agent/${shortHash(prd.sha256)}-${prd.title.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')}`,
    acceptanceCriteria: prd.raw
      .split('\n')
      .filter((line) => /^[-*]\s+/.test(line))
      .map((line) => line.replace(/^[-*]\s+/, '').trim()),
    status: 'created',
    createdAt
  };
  await writeJson(path.join(runPacketsDir, `${packet.id}.json`), packet);
  await appendDecision({
    prdId: prd.id,
    prdSha256: prd.sha256,
    renderId: render?.id,
    decision: 'run_intent',
    note: `Created bounded run packet ${packet.id}`
  });
  return packet;
}
