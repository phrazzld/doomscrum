import fs from 'node:fs/promises';
import path from 'node:path';
import { backlogDir, eventsPath, indexPath } from '../lib/paths';
import { readJson, writeJson } from '../lib/fs-utils';
import { sha256 } from '../lib/hash';
import type { FeedDecision, PrdSource, PrdStatus } from '../shared/types';

type IndexState = {
  firstSeen: Record<string, string>;
};

function extractTitle(raw: string, filename: string): string {
  const heading = raw.match(/^#\s+(.+)$/m);
  return heading?.[1]?.trim() || filename.replace(/\.md$/, '').replaceAll('-', ' ');
}

function statusFromDecisions(prdId: string, decisions: FeedDecision[]): PrdStatus {
  const relevant = decisions.filter((decision) => decision.prdId === prdId);
  const last = relevant.at(-1);
  if (!last) {
    return 'new';
  }
  if (last.decision === 'skip') {
    return 'skipped';
  }
  if (last.decision === 'needs_spec') {
    return 'needs_spec';
  }
  if (last.decision === 'run_intent') {
    return 'run_intent_created';
  }
  return 'rendered';
}

async function loadEvents(): Promise<FeedDecision[]> {
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

export async function scanBacklog(): Promise<PrdSource[]> {
  await fs.mkdir(backlogDir, { recursive: true });
  const index = await readJson<IndexState>(indexPath, { firstSeen: {} });
  const decisions = await loadEvents();
  const files = (await fs.readdir(backlogDir)).filter((file) => file.endsWith('.md')).sort();
  const prds: PrdSource[] = [];
  let changed = false;

  for (const file of files) {
    const fullPath = path.join(backlogDir, file);
    const raw = await fs.readFile(fullPath, 'utf8');
    const hash = sha256(raw);
    if (!index.firstSeen[hash]) {
      index.firstSeen[hash] = new Date().toISOString();
      changed = true;
    }
    prds.push({
      id: hash,
      path: path.relative(process.cwd(), fullPath),
      sha256: hash,
      title: extractTitle(raw, file),
      discoveredAt: index.firstSeen[hash],
      status: statusFromDecisions(hash, decisions),
      raw
    });
  }

  if (changed) {
    await writeJson(indexPath, index);
  }

  return prds;
}
