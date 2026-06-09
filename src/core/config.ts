import path from 'node:path';
import { readJson } from '../lib/fs-utils';
import { backlogConfigPath, projectRoot } from '../lib/paths';
import type { BacklogConfig } from '../shared/types';

const defaultConfig: BacklogConfig = {
  defaults: {
    repoPath: projectRoot,
    allowedCommands: ['npm test', 'npm run typecheck', 'npm run lint'],
    agentMode: 'local-codex',
    renderProvider: 'fal',
    maxRenderSpendUsd: 20
  },
  items: {}
};

export async function readBacklogConfig(): Promise<BacklogConfig> {
  const raw = await readJson<Partial<BacklogConfig>>(backlogConfigPath, {});
  return {
    defaults: {
      ...defaultConfig.defaults,
      ...raw.defaults,
      repoPath: path.resolve(projectRoot, raw.defaults?.repoPath || defaultConfig.defaults.repoPath)
    },
    items: raw.items || {}
  };
}

