import fs from 'node:fs/promises';
import path from 'node:path';
import { saveRender, storyboardHash } from '../core/render-store';
import { sha256 } from '../lib/hash';
import { getSecret } from '../lib/secrets';
import { ensureDir } from '../lib/fs-utils';
import { rendersDir } from '../lib/paths';
import type { Storyboard, VideoRender } from '../shared/types';
import type { CostLatencyEstimate, VideoProvider } from './types';

type FalQueueResponse = {
  video?: { url?: string };
  videos?: Array<{ url?: string }>;
  requestId?: string;
  request_id?: string;
  status_url?: string;
  response_url?: string;
};

async function downloadVideo(url: string, outputPath: string): Promise<void> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`fal video download failed: ${response.status} ${await response.text()}`);
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  await fs.writeFile(outputPath, bytes);
}

export class FalVideoProvider implements VideoProvider {
  readonly name = 'fal';
  readonly model: string;
  readonly capabilities = {
    textToVideo: true,
    imageToVideo: false,
    nativeAudio: true,
    maxDurationSec: 60,
    asyncJobs: true,
    seedControl: false
  };

  constructor(model = process.env.FAL_VIDEO_MODEL || 'fal-ai/veo3.1/fast') {
    this.model = model;
  }

  estimate(_storyboard: Storyboard): CostLatencyEstimate {
    return { costUsd: Number(process.env.FAL_RENDER_ESTIMATE_USD || 1.5), latencyMs: 90_000 };
  }

  async render(storyboard: Storyboard): Promise<VideoRender> {
    if (!getSecret('FAL_API_KEY', 'FAL_KEY')) {
      throw new Error('FAL_API_KEY or FAL_KEY is required for real provider smoke.');
    }
    const started = Date.now();
    const estimate = this.estimate(storyboard);
    const result = await this.submitAndPoll(storyboard);
    const url = result.video?.url || result.videos?.[0]?.url;
    if (!url) {
      throw new Error('fal response did not contain a video URL.');
    }
    const id = sha256(`${storyboard.id}:${this.name}:${this.model}:${url}`);
    const dir = path.join(rendersDir, storyboard.prdSha256);
    await ensureDir(dir);
    const assetPath = path.join(dir, `${id}.mp4`);
    await downloadVideo(url, assetPath);
    const render: VideoRender = {
      id,
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
      assetUrl: `/media/${storyboard.prdSha256}/${id}.mp4`,
      providerJobId: result.requestId || result.request_id || url,
      costEstimateUsd: estimate.costUsd,
      latencyMs: Date.now() - started,
      createdAt: new Date().toISOString()
    };
    await saveRender(render);
    return render;
  }

  private async submitAndPoll(storyboard: Storyboard): Promise<FalQueueResponse> {
    const response = await fetch(`https://queue.fal.run/${this.model}`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        authorization: `Key ${getSecret('FAL_API_KEY', 'FAL_KEY') || ''}`
      },
      body: JSON.stringify({
        prompt: storyboard.providerPrompt,
        aspect_ratio: '9:16',
        duration: `${Math.min(storyboard.targetDurationSec, 8)}s`,
        generate_audio: true
      })
    });
    if (!response.ok) {
      throw new Error(`fal submit failed: ${response.status} ${await response.text()}`);
    }
    const queued = (await response.json()) as FalQueueResponse;
    if (!queued.status_url || !queued.response_url) {
      return queued;
    }
    for (let attempt = 0; attempt < 90; attempt += 1) {
      const statusResponse = await fetch(queued.status_url, {
        headers: { authorization: `Key ${getSecret('FAL_API_KEY', 'FAL_KEY') || ''}` }
      });
      if (!statusResponse.ok) {
        throw new Error(`fal status failed: ${statusResponse.status} ${await statusResponse.text()}`);
      }
      const status = (await statusResponse.json()) as { status?: string };
      if (status.status === 'COMPLETED') {
        const resultResponse = await fetch(queued.response_url, {
          headers: { authorization: `Key ${getSecret('FAL_API_KEY', 'FAL_KEY') || ''}` }
        });
        if (!resultResponse.ok) {
          throw new Error(`fal result failed: ${resultResponse.status} ${await resultResponse.text()}`);
        }
        return (await resultResponse.json()) as FalQueueResponse;
      }
      if (status.status === 'FAILED') {
        throw new Error('fal job failed');
      }
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
    throw new Error('fal job timed out');
  }
}
