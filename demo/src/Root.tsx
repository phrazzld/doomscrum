import React from "react";
import { Composition } from "remotion";
import { Demo, DEMO_DURATION_FRAMES, FPS } from "./Demo";
import { Launch, LAUNCH_DURATION_FRAMES } from "./Launch";

export const RemotionRoot: React.FC = () => (
  <>
    <Composition
      id="DoomScrumDemo"
      component={Demo}
      durationInFrames={DEMO_DURATION_FRAMES}
      fps={FPS}
      width={1080}
      height={1920}
    />
    <Composition
      id="DoomScrumLaunch"
      component={Launch}
      durationInFrames={LAUNCH_DURATION_FRAMES}
      fps={FPS}
      width={1920}
      height={1080}
    />
  </>
);
