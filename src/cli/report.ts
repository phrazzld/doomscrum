import { getAppState } from '../core/state';

const state = await getAppState();
const ready = state.renders.filter((render) => render.status === 'ready');
const native = ready.filter((render) => render.audioMode === 'native');
const decisions = state.decisions.reduce<Record<string, number>>((counts, decision) => {
  counts[decision.decision] = (counts[decision.decision] || 0) + 1;
  return counts;
}, {});

console.log(
  JSON.stringify(
    {
      prds: state.prds.length,
      storyboards: state.storyboards.length,
      renders: state.renders.length,
      ready: ready.length,
      nativeAudio: native.length,
      runPackets: state.runPackets.length,
      decisions
    },
    null,
    2
  )
);
