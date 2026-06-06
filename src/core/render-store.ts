import fs from 'node:fs/promises';
import path from 'node:path';
import { readJson, writeJson } from '../lib/fs-utils';
import { rendersDir, storyboardsDir } from '../lib/paths';
import { sha256 } from '../lib/hash';
import type { Storyboard, VideoRender } from '../shared/types';

export async function saveStoryboard(storyboard: Storyboard): Promise<void> {
  await writeJson(path.join(storyboardsDir, `${storyboard.prdSha256}.json`), storyboard);
}

export async function readStoryboards(): Promise<Storyboard[]> {
  try {
    const files = (await fs.readdir(storyboardsDir)).filter((file) => file.endsWith('.json'));
    return Promise.all(files.map((file) => readJson<Storyboard>(path.join(storyboardsDir, file), {} as Storyboard)));
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
      return [];
    }
    throw error;
  }
}

export async function saveRender(render: VideoRender): Promise<void> {
  const dir = path.join(rendersDir, render.prdSha256);
  await writeJson(path.join(dir, `${render.id}.json`), render);
}

export async function readRenders(): Promise<VideoRender[]> {
  try {
    const prdDirs = await fs.readdir(rendersDir);
    const renders: VideoRender[] = [];
    for (const prdDir of prdDirs) {
      const fullDir = path.join(rendersDir, prdDir);
      const stat = await fs.stat(fullDir);
      if (!stat.isDirectory()) {
        continue;
      }
      const files = (await fs.readdir(fullDir)).filter((file) => file.endsWith('.json'));
      for (const file of files) {
        renders.push(await readJson<VideoRender>(path.join(fullDir, file), {} as VideoRender));
      }
    }
    return renders.sort((a, b) => b.createdAt.localeCompare(a.createdAt));
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
      return [];
    }
    throw error;
  }
}

export function storyboardHash(storyboard: Storyboard): string {
  return sha256(JSON.stringify(storyboard));
}
