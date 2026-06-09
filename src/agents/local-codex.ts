import { spawn } from 'node:child_process';
import path from 'node:path';
import { appendDecision } from '../core/events';
import { ensureDir, writeJson } from '../lib/fs-utils';
import { sha256 } from '../lib/hash';
import { launchesDir } from '../lib/paths';
import type { AgentLaunchReceipt, AgentRunPacket, PrdSource } from '../shared/types';

function launchPrompt(packet: AgentRunPacket, prd: PrdSource): string {
  return [
    `Implement this PRD on branch ${packet.branchName}.`,
    '',
    `Target repo: ${packet.repoPath}`,
    `Objective: ${packet.objective}`,
    `Allowed validation commands: ${packet.allowedCommands.join(', ')}`,
    `Timeout budget: ${packet.timeoutSec}s`,
    '',
    'Source PRD:',
    prd.raw,
    '',
    'Stop after implementation and validation. Leave a concise receipt in your final response.'
  ].join('\n');
}

export async function launchLocalCodex(packet: AgentRunPacket, prd: PrdSource): Promise<AgentLaunchReceipt> {
  await ensureDir(launchesDir);
  const startedAt = new Date().toISOString();
  const id = sha256(`${packet.id}:local-codex:${startedAt}`);
  const outputPath = path.join(launchesDir, `${id}.jsonl`);
  const receiptPath = path.join(launchesDir, `${id}.json`);
  const dryRun = process.env.BRAINROT_AGENT_LAUNCH_MODE === 'dry-run' || prd.agentMode === 'packet-only';
  const command = dryRun
    ? ['codex', 'exec', '--cd', packet.repoPath, launchPrompt(packet, prd)]
    : [
        'codex',
        'exec',
        '--cd',
        packet.repoPath,
        '--sandbox',
        'workspace-write',
        '--json',
        '--output-last-message',
        path.join(launchesDir, `${id}.last-message.txt`),
        launchPrompt(packet, prd)
      ];
  const receipt: AgentLaunchReceipt = {
    id,
    mode: dryRun ? 'dry-run' : 'local-codex',
    status: dryRun ? 'completed' : 'launched',
    command,
    cwd: packet.repoPath,
    outputPath,
    startedAt
  };

  if (dryRun) {
    receipt.completedAt = new Date().toISOString();
    await writeJson(receiptPath, receipt);
    return receipt;
  }

  const child = spawn(command[0], command.slice(1), {
    cwd: packet.repoPath,
    detached: true,
    stdio: ['ignore', 'pipe', 'pipe']
  });
  receipt.pid = child.pid;
  const output = await import('node:fs').then((fs) => fs.createWriteStream(outputPath, { flags: 'a' }));
  child.stdout?.pipe(output);
  child.stderr?.pipe(output);
  child.unref();
  await writeJson(receiptPath, receipt);
  await appendDecision({
    prdId: prd.id,
    prdSha256: prd.sha256,
    decision: 'run_intent',
    note: `Launched local Codex run ${id}`,
    metadata: { packetId: packet.id, launchId: id, pid: child.pid || 0 }
  });
  return receipt;
}

