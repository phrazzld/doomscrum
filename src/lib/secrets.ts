import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

let loaded = false;

function parseSecretLine(line: string): [string, string] | undefined {
  const trimmed = line.trim();
  if (!trimmed || trimmed.startsWith('#')) {
    return undefined;
  }
  const match = trimmed.match(/^(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)=(.*)$/);
  if (!match) {
    return undefined;
  }
  const key = match[1];
  let value = match[2].trim();
  if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
    value = value.slice(1, -1);
  }
  return [key, value];
}

export function loadUserSecrets(): void {
  if (loaded) {
    return;
  }
  loaded = true;
  const secretPath = path.join(os.homedir(), '.secrets');
  if (!fs.existsSync(secretPath)) {
    return;
  }
  const raw = fs.readFileSync(secretPath, 'utf8');
  for (const line of raw.split('\n')) {
    const parsed = parseSecretLine(line);
    if (!parsed) {
      continue;
    }
    const [key, value] = parsed;
    if (!process.env[key]) {
      process.env[key] = value;
    }
  }
}

export function getSecret(...keys: string[]): string | undefined {
  loadUserSecrets();
  for (const key of keys) {
    if (process.env[key]) {
      return process.env[key];
    }
  }
  return undefined;
}

