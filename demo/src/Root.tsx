import React from "react";
import { Composition } from "remotion";
import { Demo, DEMO_DURATION_FRAMES, FPS } from "./Demo";

export const RemotionRoot: React.FC = () => (
  <Composition
    id="DoomScrumDemo"
    component={Demo}
    durationInFrames={DEMO_DURATION_FRAMES}
    fps={FPS}
    width={1080}
    height={1920}
  />
);
