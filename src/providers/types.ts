import type { Storyboard, VideoRender } from '../shared/types';

export type CostLatencyEstimate = {
  costUsd: number;
  latencyMs: number;
};

export type VideoProviderCapabilities = {
  textToVideo: boolean;
  imageToVideo: boolean;
  nativeAudio: boolean;
  maxDurationSec: number;
  asyncJobs: boolean;
  seedControl?: boolean;
};

export type VideoProvider = {
  name: string;
  model: string;
  capabilities: VideoProviderCapabilities;
  estimate(storyboard: Storyboard): CostLatencyEstimate;
  render(storyboard: Storyboard): Promise<VideoRender>;
};
