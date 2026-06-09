import { scanBacklog } from './backlog';
import { readEvents } from './events';
import { readRenders, readStoryboards } from './render-store';
import fs from 'node:fs/promises';
import path from 'node:path';
import { runPacketsDir } from '../lib/paths';
import type { AgentRunPacket, AppState } from '../shared/types';
import { readBacklogConfig } from './config';
import { getSecret } from '../lib/secrets';

async function readRunPackets(): Promise<AgentRunPacket[]> {
  try {
    const files = (await fs.readdir(runPacketsDir)).filter((file) => file.endsWith('.json'));
    return Promise.all(
      files.map(async (file) => JSON.parse(await fs.readFile(path.join(runPacketsDir, file), 'utf8')) as AgentRunPacket)
    );
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
      return [];
    }
    throw error;
  }
}

export async function getAppState(): Promise<AppState> {
  const [prds, storyboards, renders, decisions, runPackets, config] = await Promise.all([
    scanBacklog(),
    readStoryboards(),
    readRenders(),
    readEvents(),
    readRunPackets(),
    readBacklogConfig()
  ]);

  return {
    prds,
    storyboards,
    renders,
    decisions,
    runPackets,
    providerConfigured: Boolean(getSecret('FAL_API_KEY', 'FAL_KEY')),
    config
  };
}
