import crypto from 'node:crypto';

export function sha256(input: string | Buffer): string {
  return crypto.createHash('sha256').update(input).digest('hex');
}

export function shortHash(hash: string): string {
  return hash.slice(0, 10);
}
