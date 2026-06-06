import { appendNdjson, ensureDir } from '../lib/fs-utils';
import { brainrotDir } from '../lib/paths';
import { compileStoryboard, distillPrd } from './distill';
import { saveStoryboard } from './render-store';
import { FakeVideoProvider } from '../providers/fake';
import { FalVideoProvider } from '../providers/fal';
import type { PrdSource, VideoRender } from '../shared/types';
import type { VideoProvider } from '../providers/types';

export function getProvider(kind: 'fake' | 'fal' = 'fake'): VideoProvider {
  return kind === 'fal' ? new FalVideoProvider() : new FakeVideoProvider();
}

export async function renderPrd(prd: PrdSource, provider: VideoProvider): Promise<VideoRender> {
  await ensureDir(brainrotDir);
  const brief = distillPrd(prd);
  const storyboard = compileStoryboard(prd, brief);
  await saveStoryboard(storyboard);
  await appendNdjson(`${brainrotDir}/events.ndjson`, {
    id: storyboard.id,
    prdId: prd.id,
    prdSha256: prd.sha256,
    decision: 'inspect',
    createdAt: new Date().toISOString(),
    note: `Storyboard generated for ${prd.title}`
  });
  return provider.render(storyboard);
}
