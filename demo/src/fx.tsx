import React from "react";
import {
  AbsoluteFill,
  Audio,
  interpolate,
  random,
  Sequence,
  spring,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";

const IMPACT = "Impact, 'Arial Black', sans-serif";

// ----- impact frame: 2-frame white flash + 1-frame invert at scene start ----
export const Flash: React.FC<{ at?: number }> = ({ at = 0 }) => {
  const frame = useCurrentFrame() - at;
  if (frame < 0 || frame > 4) return null;
  const opacity = interpolate(frame, [0, 1, 4], [0.95, 0.55, 0], {
    extrapolateRight: "clamp",
  });
  return (
    <AbsoluteFill
      style={{ background: "#fff", opacity, pointerEvents: "none" }}
    />
  );
};

// ----- camera shake: decaying jitter after a cut ----------------------------
export const useShake = (amplitude = 6, decay = 0.12): string => {
  const frame = useCurrentFrame();
  const a = amplitude * Math.exp(-decay * frame);
  const x = (random(`sx-${frame}`) - 0.5) * 2 * a;
  const y = (random(`sy-${frame}`) - 0.5) * 2 * a;
  return `translate(${x.toFixed(2)}px, ${y.toFixed(2)}px)`;
};

// ----- punch-in: overshoot scale on entry + slow creep for the whole scene --
export const usePunchIn = (creepTo = 1.05): number => {
  const frame = useCurrentFrame();
  const { fps, durationInFrames } = useVideoConfig();
  const settle = spring({ fps, frame, config: { damping: 11, stiffness: 180 } });
  const punch = 1.16 - 0.16 * settle;
  const creep = interpolate(frame, [0, durationInFrames], [1, creepTo]);
  return punch * creep;
};

// ----- karaoke captions from whisper word timings ----------------------------
export type Word = { timestamp: [number, number]; text: string };

/**
 * Active spoken word, huge, with the neighbors dimmed — readable with
 * sound off. `words` come straight from check_script_fit.py --words-json.
 */
export const Karaoke: React.FC<{
  words: Word[];
  bottom?: number;
  size?: number;
  color?: string;
}> = ({ words, bottom = 290, size = 86, color = "#b6ff2e" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const t = frame / fps;
  let active = -1;
  for (let i = 0; i < words.length; i++) {
    const [t0, t1] = words[i].timestamp;
    if (t >= t0 && t < (t1 ?? t0 + 0.4)) {
      active = i;
      break;
    }
    if (t >= t0) active = i; // hold the last spoken word between words
  }
  if (active < 0) return null;
  const win = words.slice(Math.max(0, active - 1), active + 2);
  return (
    <div
      style={{
        position: "absolute",
        bottom,
        left: 40,
        right: 40,
        display: "flex",
        justifyContent: "center",
        gap: 22,
        flexWrap: "wrap",
      }}
    >
      {win.map((w, i) => {
        const isActive = words.indexOf(w) === active;
        const pop = spring({
          fps,
          frame: frame - Math.round(w.timestamp[0] * fps),
          config: { damping: 10, stiffness: 260 },
        });
        return (
          <span
            key={`${w.timestamp[0]}-${i}`}
            style={{
              fontFamily: IMPACT,
              fontSize: isActive ? size : size * 0.62,
              color: isActive ? color : "#ffffff",
              opacity: isActive ? 1 : 0.55,
              textTransform: "uppercase",
              WebkitTextStroke: "3px #000",
              textShadow: "7px 7px 0 rgba(0,0,0,.9)",
              transform: `scale(${isActive ? pop : 1}) rotate(${
                isActive ? (random(`kr-${w.timestamp[0]}`) - 0.5) * 6 : 0
              }deg)`,
              alignSelf: "center",
            }}
          >
            {w.text.trim()}
          </span>
        );
      })}
    </div>
  );
};

// ----- segment-timing captions (delivered clip segments) --------------------
export type Segment = { start: number; end: number; text: string };

export const SegmentKaraoke: React.FC<{
  segments: Segment[];
  bottom?: number;
  size?: number;
  color?: string;
}> = ({ segments, bottom = 200, size = 64, color = "#b6ff2e" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const t = frame / fps;

  // Hold the last segment after it ends so the screen never empties mid-scene.
  let idx = segments.findIndex((s) => t >= s.start && t < s.end);
  if (idx < 0 && segments.length > 0 && t >= segments[segments.length - 1].end) {
    idx = segments.length - 1;
  }
  if (idx < 0) return null;

  const seg = segments[idx];
  const pop = spring({
    fps,
    frame: frame - Math.round(seg.start * fps),
    config: { damping: 10, stiffness: 220 },
  });
  const drift = Math.sin(frame * 0.03 + idx) * 3;

  return (
    <div
      style={{
        position: "absolute",
        bottom,
        left: 40,
        right: 40,
        textAlign: "center",
        transform: `translateX(${drift}px)`,
      }}
    >
      <span
        style={{
          fontFamily: IMPACT,
          fontSize: size,
          lineHeight: 1.1,
          color,
          textTransform: "uppercase",
          WebkitTextStroke: "3px #000",
          textShadow: "7px 7px 0 rgba(0,0,0,.9)",
          display: "inline-block",
          transform: `scale(${0.9 + pop * 0.15})`,
        }}
      >
        {seg.text}
      </span>
    </div>
  );
};

export const SegmentStarburstFlash: React.FC<{
  segments: Segment[];
  color?: string;
  bg?: string;
}> = ({ segments, color = "#c00", bg = "#ffd400" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const t = frame / fps;
  const boundary = segments.some((s) => Math.abs(t - s.start) < 0.12);
  if (!boundary) return null;
  return (
    <Starburst size={160} bg={bg} color={color} style={{ top: 160, right: 90 }}>
      BOOM!
    </Starburst>
  );
};

// ----- 90s infomercial starburst ---------------------------------------------
export const Starburst: React.FC<{
  children: React.ReactNode;
  size?: number;
  bg?: string;
  color?: string;
  delay?: number;
  style?: React.CSSProperties;
}> = ({ children, size = 360, bg = "#ffd400", color = "#c00", delay = 0, style }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const s = spring({ fps, frame: frame - delay, config: { damping: 9, stiffness: 200 } });
  const wobble = Math.sin((frame - delay) * 0.35) * 4;
  if (frame < delay) return null;
  const spikes = 14;
  const pts: string[] = [];
  for (let i = 0; i < spikes * 2; i++) {
    const r = i % 2 === 0 ? 50 : 36;
    const a = (Math.PI * i) / spikes;
    pts.push(`${50 + r * Math.cos(a)},${50 + r * Math.sin(a)}`);
  }
  return (
    <div
      style={{
        position: "absolute",
        width: size,
        height: size,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        transform: `scale(${s}) rotate(${wobble}deg)`,
        ...style,
      }}
    >
      <svg viewBox="0 0 100 100" width="100%" height="100%" style={{ position: "absolute" }}>
        <polygon points={pts.join(" ")} fill={bg} stroke="#000" strokeWidth="1.6" />
      </svg>
      <div
        style={{
          position: "relative",
          fontFamily: IMPACT,
          fontSize: size / 7.2,
          color,
          textTransform: "uppercase",
          textAlign: "center",
          lineHeight: 1.02,
          transform: "rotate(-8deg)",
          width: "62%",
        }}
      >
        {children}
      </div>
    </div>
  );
};

// ----- sound design helpers ---------------------------------------------------
/** Boom at a scene-relative frame. */
export const Boom: React.FC<{ at?: number; volume?: number }> = ({
  at = 0,
  volume = 0.65,
}) => (
  <Sequence from={at} durationInFrames={45} layout="none">
    <Audio src={staticFile("sfx/boom.wav")} volume={volume} />
  </Sequence>
);

export const Riser: React.FC<{ at?: number; volume?: number }> = ({
  at = 0,
  volume = 0.5,
}) => (
  <Sequence from={at} durationInFrames={40} layout="none">
    <Audio src={staticFile("sfx/riser.wav")} volume={volume} />
  </Sequence>
);

export const Hit: React.FC<{ at?: number; volume?: number }> = ({
  at = 0,
  volume = 0.6,
}) => (
  <Sequence from={at} durationInFrames={12} layout="none">
    <Audio src={staticFile("sfx/hit.wav")} volume={volume} />
  </Sequence>
);

// ----- VHS chromatic aberration overlay (use sparingly: cold open + close) ---
export const VhsTag: React.FC<{ label?: string }> = ({ label = "PLAY ▶" }) => {
  const frame = useCurrentFrame();
  const blink = Math.floor(frame / 18) % 2 === 0;
  return (
    <>
      <div
        style={{
          position: "absolute",
          top: 56,
          left: 56,
          fontFamily: "ui-monospace, Menlo, monospace",
          fontSize: 40,
          color: "#fff",
          textShadow: "2px 0 0 #f0f, -2px 0 0 #0ff",
          opacity: blink ? 1 : 0.25,
          letterSpacing: "0.12em",
        }}
      >
        {label}
      </div>
      <div
        style={{
          position: "absolute",
          top: 56,
          right: 56,
          fontFamily: "ui-monospace, Menlo, monospace",
          fontSize: 34,
          color: "#fff",
          textShadow: "2px 0 0 #f0f, -2px 0 0 #0ff",
          opacity: 0.8,
        }}
      >
        SP 0:00:{String(Math.floor(frame / 30)).padStart(2, "0")}
      </div>
    </>
  );
};
