import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';
import { ensureDir } from '../lib/fs-utils';
import { rendersDir } from '../lib/paths';
import { sha256, shortHash } from '../lib/hash';
import { saveRender, storyboardHash } from '../core/render-store';
import type { Storyboard, VideoRender } from '../shared/types';
import type { CostLatencyEstimate, VideoProvider } from './types';

function runFfmpeg(outputPath: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const args = [
      '-y',
      '-f',
      'lavfi',
      '-i',
      'testsrc2=size=540x960:rate=24:duration=6',
      '-f',
      'lavfi',
      '-i',
      'sine=frequency=880:duration=6',
      '-shortest',
      '-pix_fmt',
      'yuv420p',
      '-c:v',
      'libx264',
      '-c:a',
      'aac',
      outputPath
    ];
    const child = spawn('ffmpeg', args, { stdio: 'ignore' });
    child.on('error', reject);
    child.on('exit', (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`ffmpeg exited with ${code}`));
      }
    });
  });
}

export class FakeVideoProvider implements VideoProvider {
  readonly name = 'fake-local';
  readonly model = 'ffmpeg-native-audio-fixture';
  readonly capabilities = {
    textToVideo: true,
    imageToVideo: false,
    nativeAudio: true,
    maxDurationSec: 60,
    asyncJobs: false,
    seedControl: true
  };

  estimate(_storyboard: Storyboard): CostLatencyEstimate {
    return { costUsd: 0, latencyMs: 1200 };
  }

  async render(storyboard: Storyboard): Promise<VideoRender> {
    const started = Date.now();
    const renderId = sha256(`${storyboard.id}:fake-local`);
    const dir = path.join(rendersDir, storyboard.prdSha256);
    await ensureDir(dir);
    const assetPath = path.join(dir, `${renderId}.mp4`);
    const exists = await fs.stat(assetPath).then(() => true).catch(() => false);
    if (!exists) {
      await runFfmpeg(assetPath);
    }
    const render: VideoRender = {
      id: renderId,
      prdId: storyboard.prdId,
      prdSha256: storyboard.prdSha256,
      storyboardId: storyboard.id,
      storyboardHash: storyboardHash(storyboard),
      provider: this.name,
      model: this.model,
      nativeAudioRequested: true,
      audioMode: 'native',
      status: 'ready',
      assetPath,
      assetUrl: `/media/${storyboard.prdSha256}/${renderId}.mp4`,
      providerJobId: `fake-${shortHash(renderId)}`,
      costEstimateUsd: 0,
      actualCostUsd: 0,
      latencyMs: Date.now() - started,
      createdAt: new Date().toISOString()
    };
    await saveRender(render);
    return render;
  }
}
