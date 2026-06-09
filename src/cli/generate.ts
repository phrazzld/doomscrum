import { scanBacklog } from '../core/backlog';
import { getProvider, renderPrd } from '../core/pipeline';

const useRealProvider = process.argv.includes('--real-provider') || process.env.RUN_PROVIDER_SMOKE === '1';
const limitArg = process.argv.find((arg) => arg.startsWith('--limit='));
const limit = limitArg ? Number(limitArg.replace('--limit=', '')) : undefined;
const provider = getProvider(useRealProvider ? 'fal' : 'fake');
const prds = (await scanBacklog()).slice(0, limit);

if (prds.length === 0) {
  throw new Error('No PRDs found under backlog.d/.');
}

const renders = [];
for (const prd of prds) {
  renders.push(await renderPrd(prd, provider));
}

console.log(
  JSON.stringify(
    {
      provider: provider.name,
      model: provider.model,
      count: renders.length,
      renders: renders.map((render) => ({
        id: render.id,
        prdId: render.prdId,
        audioMode: render.audioMode,
        assetPath: render.assetPath,
        costEstimateUsd: render.costEstimateUsd,
        latencyMs: render.latencyMs
      }))
    },
    null,
    2
  )
);
