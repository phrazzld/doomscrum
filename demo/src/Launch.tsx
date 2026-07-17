import React from "react";
import {
  AbsoluteFill,
  Audio,
  Easing,
  interpolate,
  OffthreadVideo,
  Sequence,
  spring,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";
import {
  Boom,
  Flash,
  Hit,
  Karaoke,
  Riser,
  Starburst,
  usePunchIn,
  useShake,
  VhsTag,
} from "./fx";
import { CAPTIONS } from "./captions_launch";

export const FPS = 30;
const sec = (s: number) => Math.round(s * FPS);

// ----- scene timing (seconds) ---------------------------------------------
export const LAUNCH_T = {
  hook: 4.0,
  problem: 3.8,
  product1: 5.0,
  title: 1.0, // tight instrumental punch, no VO
  product2: 5.0,
  tapSpec: 4.0, // silent beat: tap ripple -> spec sheet slide-up -> lockup
  proofJoke: 6.0, // honest-failure beat — native card, no clip
  proofQA: 5.0,
  proofGoblin: 5.5,
  proofJanitor: 4.5,
  swipe: 6.5,
  price: 5.5,
  cta: 8.0,
};
// Total: 64.3s

export const PR_TITLE = "DoomScrum impl: Plug DoomScrum into arbitrary repos (epic: picker, contract, MCP source)";
export const PR_NUMBER = "45";
export const PR_REPO_PATH = "misty-step/doomscrum → master";
export const PR_STATS = "+235 −2 · 6 files · Fixes #43 · opened by a swipe";
export const LAUNCH_ORDER: (keyof typeof LAUNCH_T)[] = [
  "hook",
  "problem",
  "product1",
  "title",
  "product2",
  "tapSpec",
  "proofJoke",
  "proofQA",
  "proofGoblin",
  "proofJanitor",
  "swipe",
  "price",
  "cta",
];

const startOf = (k: keyof typeof LAUNCH_T) =>
  sec(LAUNCH_ORDER.slice(0, LAUNCH_ORDER.indexOf(k)).reduce((a, x) => a + LAUNCH_T[x], 0));

export const LAUNCH_DURATION_FRAMES = sec(
  LAUNCH_ORDER.reduce((a, x) => a + LAUNCH_T[x], 0)
);

// ----- final VO durations (48 kHz) -----------------------------------------
export const VO_DURATIONS = {
  hook: 2.391667,
  problem: 2.03175,
  product1: 2.391667,
  product2: 3.610729,
  swipe: 4.713667,
  price: 3.123104,
  cta: 3.250813,
};

const DUCK_RANGES: [number, number][] = ([
  "hook",
  "problem",
  "product1",
  "product2",
  "swipe",
  "price",
  "cta",
] as (keyof typeof VO_DURATIONS)[]).map((k) => [
  startOf(k as keyof typeof LAUNCH_T),
  startOf(k as keyof typeof LAUNCH_T) + Math.round(VO_DURATIONS[k] * FPS),
]);

const musicVolume = (frame: number): number => {
  let target = 0.42;
  for (const [rs, re] of DUCK_RANGES) {
    if (frame >= rs && frame <= re) return 0.10;
    if (frame > rs - 10 && frame < rs) {
      target = interpolate(frame, [rs - 10, rs], [0.42, 0.10], {
        extrapolateLeft: "clamp",
        extrapolateRight: "clamp",
      });
    }
    if (frame > re && frame < re + 20) {
      target = interpolate(frame, [re, re + 20], [0.10, 0.42], {
        extrapolateLeft: "clamp",
        extrapolateRight: "clamp",
      });
    }
  }
  return target;
};

// ----- palette -------------------------------------------------------------
const ACID = "#b6ff2e";
const PINK = "#ff2ea6";
const INK = "#e8ffe0";
const BG = "#07090b";
const IMPACT = "Impact, 'Arial Black', sans-serif";
const MONO = "ui-monospace, 'SF Mono', Menlo, monospace";

// ----- real issue data (live-verified 2026-07-17 via gh) -------------------
// Creative headlines are the brainrot-format titles; issue numbers/titles are real.
type ProofIssue = {
  issue: string;
  realTitle: string;
  headline: string;
  sticker: string;
  badge: string;
  honestFailure?: string;
};

const PROOF_ISSUES: Record<string, ProofIssue> = {
  joke: {
    issue: "#36",
    realTitle: "Audit whether the core brainrot joke actually lands",
    headline: "AUDIT BRAINROT JOKE",
    sticker: "ISSUE #36",
    badge: "OPEN",
    honestFailure: "EVIDENCE SAYS NO.",
  },
  qa: {
    issue: "#40",
    realTitle: "Autonomous persona QA agent that files tickets",
    headline: "QA WALKS, FILES ISSUES",
    sticker: "ISSUE #40",
    badge: "OPEN",
  },
  goblin: {
    issue: "#39",
    realTitle: "Local open-weights video provider (electricity-priced renders)",
    headline: "BUDGET GOBLIN GPU",
    sticker: "ISSUE #39",
    badge: "OPEN",
  },
  janitor: {
    issue: "#44",
    realTitle: "[PROPOSAL — needs ratification] Purge orphaned artifacts and one dead fn",
    headline: "RATIFICATION RACCOON",
    sticker: "ISSUE #44",
    badge: "OPEN",
  },
};

// ----- utility layout blocks -----------------------------------------------
const Scanlines: React.FC = () => (
  <AbsoluteFill
    style={{
      background:
        "repeating-linear-gradient(0deg, rgba(0,0,0,.18) 0 2px, transparent 2px 6px)",
      mixBlendMode: "overlay",
      pointerEvents: "none",
    }}
  />
);

const MemeText: React.FC<{
  children: React.ReactNode;
  size?: number;
  color?: string;
  delay?: number;
  style?: React.CSSProperties;
}> = ({ children, size = 92, color = "#fff", delay = 0, style }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const s = spring({ fps, frame: frame - delay, config: { damping: 12, stiffness: 200 } });
  return (
    <div
      style={{
        fontFamily: IMPACT,
        fontSize: size,
        lineHeight: 1.04,
        color,
        textTransform: "uppercase",
        WebkitTextStroke: "3px #000",
        textShadow: "8px 8px 0 rgba(0,0,0,.85)",
        textAlign: "center",
        transform: `scale(${s})`,
        opacity: frame >= delay ? 1 : 0,
        ...style,
      }}
    >
      {children}
    </div>
  );
};

const Sticker: React.FC<{
  children: React.ReactNode;
  bg?: string;
  color?: string;
  rotate?: number;
  style?: React.CSSProperties;
}> = ({ children, bg = ACID, color = "#000", rotate = 6, style }) => (
  <div
    style={{
      position: "absolute",
      fontFamily: IMPACT,
      textTransform: "uppercase",
      fontSize: 34,
      letterSpacing: "0.06em",
      padding: "10px 18px",
      border: "4px solid #000",
      boxShadow: "8px 8px 0 #000",
      background: bg,
      color,
      transform: `rotate(${rotate}deg)`,
      zIndex: 20,
      ...style,
    }}
  >
    {children}
  </div>
);

/** Widescreen Phone Frame for 9:16 overlay */
const PhoneFrame: React.FC<{
  children: React.ReactNode;
  scale?: number;
  shake?: string;
}> = ({ children, scale = 1, shake = "translate(0px, 0px)" }) => {
  return (
    <div
      style={{
        position: "relative",
        height: 720,
        aspectRatio: "9 / 16",
        background: "#000",
        border: "12px solid #222",
        borderRadius: 40,
        boxShadow: `0 0 100px rgba(0,0,0,0.8), 12px 12px 0 ${ACID}`,
        overflow: "hidden",
        transform: `${shake} scale(${scale})`,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      {/* Notch */}
      <div
        style={{
          position: "absolute",
          top: 0,
          width: 140,
          height: 25,
          background: "#222",
          borderBottomLeftRadius: 16,
          borderBottomRightRadius: 16,
          zIndex: 50,
        }}
      />
      <AbsoluteFill>{children}</AbsoluteFill>
    </div>
  );
};

/** Opaque scrim masking baked caption zones in phone-frame clip B-roll. */
const CaptionScrim: React.FC<{ height?: string }> = ({ height = "18%" }) => (
  <div
    style={{
      position: "absolute",
      bottom: 0,
      left: 0,
      right: 0,
      height,
      background: "rgba(7,9,11,0.96)",
      borderTop: "2px solid #222",
      zIndex: 15,
      pointerEvents: "none",
    }}
  />
);

/** Animated gradient backdrop — replaces ClipEcho (no unmapped clip footage) */
const PulseBg: React.FC<{ hue?: string }> = ({ hue = PINK }) => {
  const frame = useCurrentFrame();
  const pulse = Math.sin(frame * 0.06) * 0.5 + 0.5;
  return (
    <AbsoluteFill style={{ zIndex: 0 }}>
      <AbsoluteFill style={{ background: BG }} />
      <AbsoluteFill
        style={{
          background: `radial-gradient(circle at 30% 40%, ${hue}22, transparent 60%), radial-gradient(circle at 70% 70%, ${ACID}11, transparent 50%)`,
          opacity: 0.5 + pulse * 0.3,
        }}
      />
    </AbsoluteFill>
  );
};

const HitBurst: React.FC = () => {
  const frame = useCurrentFrame();
  const opacity = interpolate(frame, [12, 24], [1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  return (
    <div style={{ opacity }}>
      <Starburst size={150} bg={PINK} color="#fff" style={{ top: 20, right: 10 }}>
        HIT!
      </Starburst>
    </div>
  );
};

const PrCard: React.FC<{ scale?: number }> = ({ scale = 1 }) => (
  <div
    style={{
      width: 1100,
      background: "#161b22",
      border: "1px solid #30363d",
      borderRadius: 18,
      padding: 40,
      transform: `scale(${scale})`,
      fontFamily: "-apple-system, 'Segoe UI', Helvetica, sans-serif",
      color: "#e6edf3",
      boxShadow: "0 20px 50px rgba(0,0,0,0.5)",
    }}
  >
    <div style={{ fontSize: 30, fontWeight: 600, lineHeight: 1.2 }}>
      {PR_TITLE} <span style={{ color: "#8b949e" }}>#{PR_NUMBER}</span>
    </div>
    <div style={{ display: "flex", alignItems: "center", gap: 14, marginTop: 20 }}>
      <span
        style={{
          background: "#238636",
          borderRadius: 999,
          padding: "6px 18px",
          fontSize: 22,
          fontWeight: 600,
        }}
      >
        ✓ Open
      </span>
      <span style={{ fontFamily: MONO, fontSize: 18, color: "#8b949e" }}>
        {PR_REPO_PATH}
      </span>
    </div>
    <div style={{ marginTop: 24, fontSize: 20, color: "#8b949e" }}>
      {PR_STATS}
    </div>
  </div>
);

// ----- scenes ---------------------------------------------------------------

/** 1. Cold Open (0s - 4s): outcomes-first. Swipe opens a real PR. */
const HookScene: React.FC = () => {
  const { fps } = useVideoConfig();
  const frame = useCurrentFrame();
  const enter = spring({ fps, frame, config: { damping: 10, stiffness: 220 } });
  const shake = useShake(6);
  // Hold the outcomes-first PR cold open, then swipe the recognizable feed in
  // immediately after the 2.39s hook line without changing scene timing.
  const feedReveal = interpolate(frame, [70, 76], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const feedX = interpolate(frame, [70, 76], [260, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  return (
    <AbsoluteFill
      style={{
        background: "#0d1117",
        justifyContent: "center",
        alignItems: "center",
        flexDirection: "column",
        gap: 40,
        transform: shake,
      }}
    >
      <Boom volume={0.55} />
      <PrCard scale={enter} />
      <div style={{ display: "flex", gap: 20, flexDirection: "row" }}>
        <MemeText size={72} color={ACID} delay={Math.round(0.4 * fps)}>
          This PR is REAL.
        </MemeText>
        <MemeText size={72} color={PINK} delay={Math.round(0.8 * fps)}>
          Opened by a swipe.
        </MemeText>
      </div>
      <div
        style={{
          position: "absolute",
          right: 44,
          top: 150,
          zIndex: 12,
          transform: `translateX(${feedX}px)`,
        }}
      >
        <PhoneFrame scale={0.82 * feedReveal}>
          <div style={{ width: "100%", height: "100%", position: "relative" }}>
            <OffthreadVideo
              src={staticFile("infomercial.mp4")}
              muted
              style={{ width: "100%", height: "100%", objectFit: "cover" }}
            />
            <CaptionScrim />
            <Sticker style={{ top: 20, right: 20 }}>FEED</Sticker>
            <Sticker bg={PINK} color="#fff" rotate={-4} style={{ bottom: 30, left: 20 }}>
              ISSUE #36
            </Sticker>
          </div>
        </PhoneFrame>
      </div>
      <Karaoke words={CAPTIONS.hook} bottom={40} size={64} color={INK} />
      <VhsTag label="COLD OPEN" />
      <Flash />
    </AbsoluteFill>
  );
};

/** 2. Problem (4s - 7.8s): rotting backlog table. */
const ProblemScene: React.FC = () => {
  const { fps } = useVideoConfig();
  const frame = useCurrentFrame();
  const shake = useShake(5);
  const scale = usePunchIn(1.02);

  // Real issue titles (live-verified via gh, 2026-07-17)
  const issues = [
    { id: "#36", title: "Audit whether the core brainrot joke actually lands", age: "OPEN", col: PINK },
    { id: "#40", title: "Autonomous persona QA agent that files tickets", age: "OPEN", col: PINK },
    { id: "#39", title: "Local open-weights video provider (electricity-priced renders)", age: "OPEN", col: PINK },
    { id: "#43", title: "Plug DoomScrum into arbitrary repos (epic: picker, contract, MCP)", age: "OPEN", col: PINK },
  ];

  return (
    <AbsoluteFill
      style={{
        background: BG,
        justifyContent: "center",
        alignItems: "center",
        flexDirection: "column",
        gap: 30,
        transform: `${shake} scale(${scale})`,
      }}
    >
      <Boom volume={0.55} />
      <MemeText size={64} color={PINK}>YOUR BACKLOG IS ROTTING.</MemeText>
      <Karaoke words={CAPTIONS.problem} bottom={40} size={58} color={INK} />

      {/* GitHub Issue backlog native layout */}
      <div
        style={{
          width: 1200,
          background: "#161b22",
          border: "2px solid #30363d",
          borderRadius: 16,
          overflow: "hidden",
          boxShadow: `0 0 50px rgba(255, 46, 166, 0.15)`,
          fontFamily: "-apple-system, 'Segoe UI', sans-serif",
          color: "#e6edf3",
        }}
      >
        <div style={{ background: "#21262d", padding: "16px 24px", fontWeight: 600, borderBottom: "1px solid #30363d", fontSize: 24, display: "flex", justifyContent: "space-between" }}>
          <span>Open Backlog (4)</span>
          <span style={{ color: PINK }}>⚠ ALERT: Critical Decays</span>
        </div>
        {issues.map((issue, idx) => {
          const rowSlid = spring({
            fps,
            frame: frame - Math.round(idx * 0.15 * fps),
            config: { damping: 12 },
          });
          const opacity = interpolate(frame, [0, Math.max(1, Math.round(idx * 0.15 * fps))], [0, 1], { extrapolateRight: "clamp" });
          const glow = Math.sin(frame * 0.1 + idx) * 3 + 4;
          return (
            <div
              key={issue.id}
              style={{
                padding: "20px 24px",
                borderBottom: idx < issues.length - 1 ? "1px solid #30363d" : "none",
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                transform: `translateX(${(1 - rowSlid) * -50}px)`,
                opacity,
              }}
            >
              <div style={{ display: "flex", gap: 16, alignItems: "center" }}>
                <span style={{ color: "#3fb950", fontSize: 22 }}>☉</span>
                <span style={{ fontWeight: 600, fontSize: 22, marginRight: 8, color: "#8b949e" }}>{issue.id}</span>
                <span style={{ fontSize: 20, color: "#e6edf3" }}>{issue.title}</span>
              </div>
              <div
                style={{
                  background: "rgba(255, 46, 166, 0.15)",
                  color: PINK,
                  border: `1px solid ${PINK}`,
                  padding: "4px 12px",
                  borderRadius: 6,
                  fontFamily: MONO,
                  fontSize: 16,
                  textTransform: "uppercase",
                  boxShadow: `0 0 ${glow}px ${PINK}`,
                }}
              >
                {issue.age}
              </div>
            </div>
          );
        })}
      </div>
      <Scanlines />
    </AbsoluteFill>
  );
};

/** 3. Product1 (7.8s - 12.8s): Feed reveal. Uses waived infomercial.mp4 as feed B-roll. */
const Product1Scene: React.FC = () => {
  const { fps } = useVideoConfig();
  const frame = useCurrentFrame();
  const shake = useShake(4);
  const scale = usePunchIn(1.03);

  return (
    <AbsoluteFill
      style={{
        background: BG,
        justifyContent: "center",
        alignItems: "center",
        transform: shake,
      }}
    >
      <Boom volume={0.55} />
      <div
        style={{
          display: "flex",
          width: "100%",
          height: "100%",
          padding: 80,
          alignItems: "center",
          justifyContent: "space-around",
          flexDirection: "row",
        }}
      >
        <div style={{ width: 800, textAlign: "left" }}>
          <MemeText size={78} color={ACID}>DOOMSCRUM</MemeText>
          <MemeText size={56} style={{ marginTop: 24, textAlign: "left" }}>
            TURNS IT INTO A FEED.
          </MemeText>
          <div style={{ fontFamily: MONO, fontSize: 30, color: INK, marginTop: 40, opacity: 0.8, lineHeight: 1.5 }}>
            &gt; Generating video summaries...<br />
            &gt; Embedding issue tickets into TikTok formats...<br />
            &gt; Hooking cognitive loops to backlog health...
          </div>
        </div>

        {/* 9:16 Phone frame inset — waived clip as feed B-roll */}
        <PhoneFrame scale={scale}>
          <div style={{ width: "100%", height: "100%", position: "relative" }}>
            <OffthreadVideo
              src={staticFile("infomercial.mp4")}
              muted
              style={{ width: "100%", height: "100%", objectFit: "cover" }}
            />
            <CaptionScrim />
            <Sticker style={{ top: 20, right: 20 }}>FEED</Sticker>
            <Sticker bg={PINK} color="#fff" rotate={-4} style={{ bottom: 30, left: 20 }}>
              ISSUE #36
            </Sticker>
          </div>
        </PhoneFrame>
      </div>
      <Karaoke words={CAPTIONS.product1} bottom={50} size={56} color={INK} />
      <Scanlines />
      <Flash />
    </AbsoluteFill>
  );
};

/** 4. Title Flash (12.8s - 14.8s): extreme short beat. */
const TitleScene: React.FC = () => {
  const frame = useCurrentFrame();
  const color = frame % 4 < 2 ? ACID : PINK;
  const shake = useShake(20, 0.08);
  return (
    <AbsoluteFill
      style={{
        background: "#000",
        justifyContent: "center",
        alignItems: "center",
        transform: shake,
      }}
    >
      <Riser volume={0.8} />
      <MemeText size={180} color={color}>DOOMSCRUM</MemeText>
      <VhsTag label="SYSTEM SHOCK" />
      <Scanlines />
      <Flash />
    </AbsoluteFill>
  );
};

/** 5. Product2 (14.8s - 19.8s): Gestures. Uses waived cryptid_vlog.mp4. */
const Product2Scene: React.FC = () => {
  const { fps } = useVideoConfig();
  const frame = useCurrentFrame();
  const shake = useShake(5);

  // Swipe right motion inside the phone frame
  const cardSlid = interpolate(frame, [fps * 1.5, fps * 3.0], [0, 500], {
    easing: Easing.in(Easing.cubic),
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const cardRot = interpolate(frame, [fps * 1.5, fps * 3.0], [0, 15], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <AbsoluteFill
      style={{
        background: BG,
        justifyContent: "center",
        alignItems: "center",
        transform: shake,
      }}
    >
      <Hit volume={0.45} />
      <div
        style={{
          display: "flex",
          width: "100%",
          height: "100%",
          padding: 80,
          alignItems: "center",
          justifyContent: "space-around",
          flexDirection: "row",
        }}
      >
        <PhoneFrame>
          <div
            style={{
              width: "100%",
              height: "100%",
              position: "relative",
              transform: `translateX(${cardSlid}px) rotate(${cardRot}deg)`,
            }}
          >
            <OffthreadVideo
              src={staticFile("cryptid_vlog.mp4")}
              muted
              style={{ width: "100%", height: "100%", objectFit: "cover" }}
            />
            {/* Gesture overlay */}
            <div
              style={{
                position: "absolute",
                inset: 0,
                background: "rgba(182, 255, 46, 0.15)",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                opacity: frame > fps * 1.0 ? 1 : 0,
              }}
            >
              <div
                style={{
                  fontFamily: IMPACT,
                  fontSize: 100,
                  color: ACID,
                  textShadow: "4px 4px 0 #000",
                }}
              >
                SWIPE RIGHT ➔
              </div>
            </div>
            <Sticker style={{ top: 20, right: 20 }}>SWIPING</Sticker>
          </div>
        </PhoneFrame>

        <div style={{ width: 800, textAlign: "left" }}>
          <MemeText size={64} color={ACID} style={{ textAlign: "left" }}>
            SWIPE RIGHT TO SHIP.
          </MemeText>
          <MemeText size={64} color={PINK} style={{ marginTop: 24, textAlign: "left" }}>
            SWIPE LEFT TO SKIP.
          </MemeText>
          {/* karaoke moved to scene level (see below) */}
          <div style={{ fontFamily: MONO, fontSize: 32, color: "#fff", marginTop: 40, background: "#161b22", padding: 24, border: "2px solid #30363d", borderRadius: 8 }}>
            $ doomscrum list<br />
            &gt; swipe right: real agent implements, opens a real PR<br />
            &gt; swipe left: skip. spec untouched<br />
            &gt; tap: read the exact spec
          </div>
        </div>
      </div>
      <Karaoke words={CAPTIONS.product2} bottom={50} size={52} color={INK} />
      <Scanlines />
      <Flash />
    </AbsoluteFill>
  );
};

/** 5b. Tap-to-spec (19.8s - 23.8s): silent consent/transparency beat. No VO —
 *  on-screen text + SFX carry it. Backing loop stays up, no duck.
 *  Feed card is a native Remotion UI composition (no third-party clip). */
const TapSpecScene: React.FC = () => {
  const { fps } = useVideoConfig();
  const frame = useCurrentFrame();
  const shake = useShake(3);
  const scale = usePunchIn(1.02);

  const tapFrame = Math.round(0.6 * fps);
  const tapPulse = spring({
    fps,
    frame: frame - tapFrame,
    config: { damping: 10, stiffness: 300 },
  });
  const tapOpacity = interpolate(frame, [tapFrame + 2, tapFrame + 24], [1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const sheetStart = tapFrame + 6;
  const sheetIn = spring({
    fps,
    frame: frame - sheetStart,
    config: { damping: 16, stiffness: 120 },
  });
  const sheetY = interpolate(sheetIn, [0, 1], [720, 0]);

  return (
    <AbsoluteFill style={{ background: BG, transform: shake }}>
      <Hit at={tapFrame} volume={0.55} />
      <Riser at={sheetStart} volume={0.35} />
      <div
        style={{
          display: "flex",
          width: "100%",
          height: "100%",
          padding: 80,
          alignItems: "center",
          justifyContent: "space-around",
          flexDirection: "row",
        }}
      >
        <PhoneFrame scale={scale}>
          <AbsoluteFill>
            {/* Native feed card — issue #36 as a Remotion UI composition */}
            <div style={{ width: "100%", height: "100%", background: "#0d1117", display: "flex", flexDirection: "column", padding: "40px 20px 20px" }}>
              <div style={{ fontSize: 12, color: "#8b949e", fontFamily: MONO, marginBottom: 8 }}>github-issues/36.md</div>
              <div style={{ fontFamily: IMPACT, fontSize: 28, lineHeight: 1.1, color: "#fff", textTransform: "uppercase", textShadow: "2px 2px 0 #000" }}>
                Audit whether the core brainrot joke actually lands
              </div>
              <div style={{ display: "flex", gap: 8, marginTop: 12 }}>
                <span style={{ background: "#238636", borderRadius: 999, padding: "3px 10px", fontSize: 13, fontWeight: 600, color: "#fff" }}>✓ Open</span>
                <span style={{ border: "1px solid #30363d", borderRadius: 999, padding: "3px 10px", fontSize: 13, color: "#8b949e" }}>#36</span>
              </div>
              <div style={{ marginTop: 16, fontFamily: MONO, fontSize: 12, color: "#c8d0c0", lineHeight: 1.6, flex: 1, overflow: "hidden" }}>
                {`## Goal\nDetermine, with evidence rather than\nvibes, whether DoomScrum's core\nmechanic is actually funny/shareable\nas implemented today.\n\n## Oracle\n[ ] Evidence table built\n[ ] Failure pattern quantified`}
              </div>
            </div>
            <Sticker style={{ top: 20, right: 20 }}>ISSUE #36</Sticker>
            <Sticker bg={ACID} color="#000" rotate={-4} style={{ top: 20, left: 20 }}>
              FRESH
            </Sticker>
            <div
              style={{
                position: "absolute",
                left: 20,
                right: 20,
                bottom: 26,
                fontFamily: IMPACT,
                fontSize: 34,
                lineHeight: 1.05,
                color: "#fff",
                textTransform: "uppercase",
                textShadow: "4px 4px 0 rgba(0,0,0,.85)",
                opacity: frame < sheetStart + 6 ? 1 : 0,
              }}
            >
              Audit whether the core brainrot joke lands
            </div>

            {/* Tap ripple + finger dot */}
            <div
              style={{
                position: "absolute",
                left: "50%",
                top: "54%",
                width: 110,
                height: 110,
                marginLeft: -55,
                marginTop: -55,
                borderRadius: "50%",
                border: `5px solid ${ACID}`,
                opacity: frame >= tapFrame ? tapOpacity : 0,
                transform: `scale(${1 + tapPulse * 1.6})`,
                pointerEvents: "none",
              }}
            />
            <div
              style={{
                position: "absolute",
                left: "50%",
                top: "54%",
                width: 26,
                height: 26,
                marginLeft: -13,
                marginTop: -13,
                borderRadius: "50%",
                background: "#fff",
                boxShadow: `0 0 20px ${ACID}`,
                opacity: frame >= tapFrame && frame < tapFrame + 18 ? 1 : 0,
              }}
            />

            {/* Spec sheet: recreates the real in-app overlay */}
            <div
              style={{
                position: "absolute",
                inset: 0,
                zIndex: 30,
                transform: `translateY(${sheetY}px)`,
                background: "rgba(5,7,9,0.97)",
                borderTop: `2px solid ${ACID}`,
                padding: "16px 16px 0",
                boxSizing: "border-box",
                fontFamily: MONO,
                color: INK,
                overflow: "hidden",
              }}
            >
              <div style={{ display: "flex", justifyContent: "flex-end" }}>
                <div
                  style={{
                    border: "1px solid #444",
                    borderRadius: 6,
                    padding: "3px 8px",
                    fontSize: 11,
                    letterSpacing: "0.05em",
                    color: "#aaa",
                  }}
                >
                  CLOSE [ESC]
                </div>
              </div>
              <div style={{ display: "flex", justifyContent: "space-between", gap: 10, marginTop: 10 }}>
                <div
                  style={{
                    fontFamily: IMPACT,
                    fontSize: 21,
                    lineHeight: 1.1,
                    color: "#fff",
                    textTransform: "uppercase",
                    maxWidth: "56%",
                  }}
                >
                  Audit whether the core brainrot joke actually lands
                </div>
                <div style={{ fontSize: 10.5, color: "#8b949e", lineHeight: 1.5, textAlign: "right" }}>
                  github-issues/36.md
                  <br />· sha256 bb55b067…
              </div>
              </div>
              <div style={{ borderTop: "1px solid #2a2a2a", margin: "12px 0" }} />
              <div style={{ fontSize: 11.5, lineHeight: 1.6, color: "#c8d0c0", whiteSpace: "pre-wrap" }}>
{`## Goal
Determine, with evidence rather than
vibes, whether DoomScrum's core
mechanic — spec-to-brainrot-video —
is actually funny/shareable as
implemented today.

## Oracle
- [ ] Evidence table built from the
      existing bench data plus a fresh
      sample of >=10 renders
- [ ] The specific failure pattern
      already visible in the bench is
      quantified, not just noted`}
              </div>
            </div>
          </AbsoluteFill>
        </PhoneFrame>

        <div style={{ width: 800, textAlign: "left" }}>
          <MemeText size={64} color={ACID} style={{ textAlign: "left" }} delay={sheetStart + 10}>
            EVERY CARD IS A REAL TICKET
          </MemeText>
          <MemeText
            size={64}
            color={ACID}
            style={{ marginTop: 24, textAlign: "left" }}
            delay={sheetStart + 22}
          >
            TAP = THE ACTUAL SPEC
          </MemeText>
          <div
            style={{
              fontFamily: MONO,
              fontSize: 30,
              color: PINK,
              marginTop: 32,
              opacity: interpolate(frame, [sheetStart + 30, sheetStart + 40], [0, 1], {
                extrapolateLeft: "clamp",
                extrapolateRight: "clamp",
              }),
            }}
          >
            path + sha256. receipts, not vibes.
          </div>
        </div>
      </div>
      <Scanlines />
      <Flash />
    </AbsoluteFill>
  );
};

/** Re-usable native proof-card scene — first-party Remotion UI, no unmapped clips.
 *  Each shows a real GitHub issue rendered as a DoomScrum feed card. */
const ProofCardScene: React.FC<{
  issueKey: keyof typeof PROOF_ISSUES;
}> = ({ issueKey }) => {
  const { fps } = useVideoConfig();
  const frame = useCurrentFrame();
  const scale = usePunchIn(1.02);
  const shake = useShake(4);
  const data = PROOF_ISSUES[issueKey];

  return (
    <AbsoluteFill style={{ background: BG, justifyContent: "center", alignItems: "center" }}>
      <PulseBg hue={issueKey === "joke" ? PINK : ACID} />
      <Boom volume={0.5} />
      <div
        style={{
          display: "flex",
          width: "100%",
          height: "100%",
          padding: "80px 80px 140px 80px",
          alignItems: "center",
          justifyContent: "space-between",
          flexDirection: "row",
          position: "relative",
          zIndex: 2,
        }}
      >
        <div style={{ width: 880, height: "100%", position: "relative", zIndex: 3 }}>
          <div style={{ textAlign: "left" }}>
            <MemeText size={72} color={ACID} style={{ textAlign: "left" }}>
              {data.headline}
            </MemeText>
          </div>
          <div style={{ marginTop: 20 }}>
            <MemeText size={64} color={ACID} style={{ textAlign: "left" }} delay={5}>
              {data.issue}
            </MemeText>
          </div>
          <div style={{ marginTop: 18, maxWidth: 760 }}>
            <MemeText
              size={40}
              color={INK}
              delay={10}
              style={{ textAlign: "left", WebkitTextStroke: "2px #000" }}
            >
              {data.realTitle}
            </MemeText>
          </div>
          {data.honestFailure && (
            <div style={{ marginTop: 28, maxWidth: 720 }}>
              <MemeText
                size={56}
                color={PINK}
                delay={20}
                style={{ textAlign: "left" }}
              >
                {data.honestFailure}
              </MemeText>
              <MemeText
                size={40}
                color={INK}
                delay={32}
                style={{ textAlign: "left", marginTop: 10, WebkitTextStroke: "2px #000" }}
              >
                Free path needs demo cartridge, not dry cards.
              </MemeText>
            </div>
          )}
          {issueKey === "joke" && <HitBurst />}
        </div>

        {/* Phone showing a native feed card for the issue */}
        <PhoneFrame scale={scale} shake={shake}>
          <AbsoluteFill>
            <div style={{ width: "100%", height: "100%", background: "#0d1117", display: "flex", flexDirection: "column", padding: "40px 20px 20px" }}>
              <div style={{ fontSize: 12, color: "#8b949e", fontFamily: MONO, marginBottom: 8 }}>
                github-issues/{data.issue.replace("#", "")}.md
              </div>
              <div style={{ fontFamily: IMPACT, fontSize: 26, lineHeight: 1.1, color: "#fff", textTransform: "uppercase", textShadow: "2px 2px 0 #000" }}>
                {data.realTitle}
              </div>
              <div style={{ display: "flex", gap: 8, marginTop: 12 }}>
                <span style={{ background: "#238636", borderRadius: 999, padding: "3px 10px", fontSize: 13, fontWeight: 600, color: "#fff" }}>✓ Open</span>
                <span style={{ border: "1px solid #30363d", borderRadius: 999, padding: "3px 10px", fontSize: 13, color: "#8b949e" }}>{data.issue}</span>
              </div>
              <div style={{ marginTop: 16, fontFamily: MONO, fontSize: 12, color: "#c8d0c0", lineHeight: 1.6, flex: 1, overflow: "hidden" }}>
                {`## ${data.headline}\n\nReal GitHub issue.\nSwipe right → agent ships it.\nSwipe left → skip, spec untouched.\nTap → read the exact spec.`}
              </div>
            </div>
            <Sticker style={{ top: 20, right: 20 }}>{data.sticker}</Sticker>
            <Sticker bg={PINK} color="#fff" rotate={-6} style={{ bottom: 30, left: 20 }}>
              {data.badge}
            </Sticker>
          </AbsoluteFill>
        </PhoneFrame>
      </div>
      <Scanlines />
      <Flash />
    </AbsoluteFill>
  );
};

/** 6. Proof: Joke (honest-failure beat — native card) */
const ProofJokeScene: React.FC = () => <ProofCardScene issueKey="joke" />;

/** 7. Proof: QA */
const ProofQAScene: React.FC = () => <ProofCardScene issueKey="qa" />;

/** 8. Proof: Goblin */
const ProofGoblinScene: React.FC = () => <ProofCardScene issueKey="goblin" />;

/** 9. Proof: Janitor */
const ProofJanitorScene: React.FC = () => <ProofCardScene issueKey="janitor" />;

/** 10. Swipe action -> cook -> PR open (6.5s). Uses waived italian_brainrot.mp4. */
const SwipeScene: React.FC = () => {
  const { fps } = useVideoConfig();
  const frame = useCurrentFrame();
  const scale = usePunchIn(1.02);

  // Card slides off to the right
  const cardSlid = interpolate(frame, [0, fps * 1.5], [0, 800], {
    easing: Easing.in(Easing.cubic),
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const cardRot = interpolate(frame, [0, fps * 1.5], [0, 20], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // Cook log scroller
  const showLogs = frame >= fps * 1.5 && frame < fps * 4.0;
  const showPR = frame >= fps * 4.0;

  // Real: PR #45 opened against misty-step/doomscrum, issue #43
  const logs = [
    "[SYS] Cooking: github-issues/43.md",
    "[SYS] Spec sha256: 9b960b22c649…",
    "[AI ] Generating backlog contract docs...",
    "[GIT] Branch: doomscrum/impl-…0fdfc3ac09",
    "[GIT] Pushing misty-step/doomscrum → master",
    "[GIT] Created Pull Request #45! Fixes #43.",
  ];

  return (
    <AbsoluteFill style={{ background: BG, justifyContent: "center", alignItems: "center" }}>
      <Riser volume={0.5} />

      {!showPR ? (
        <div style={{ display: "flex", gap: 80, flexDirection: "row", alignItems: "center" }}>
          {/* Swipe motion */}
          <PhoneFrame scale={scale}>
            {!showLogs ? (
              <div
                style={{
                  width: "100%",
                  height: "100%",
                  position: "relative",
                  transform: `translateX(${cardSlid}px) rotate(${cardRot}deg)`,
                }}
              >
                <OffthreadVideo
                  src={staticFile("italian_brainrot.mp4")}
                  muted
                  style={{ width: "100%", height: "100%", objectFit: "cover" }}
                />
                <Sticker style={{ top: 20, right: 20 }}>SWIPING➔</Sticker>
              </div>
            ) : (
              // Cooking Terminal logs scrolling
              <div
                style={{
                  width: "100%",
                  height: "100%",
                  background: "#07090b",
                  padding: "40px 20px 20px 20px",
                  fontFamily: MONO,
                  fontSize: 14,
                  color: ACID,
                  display: "flex",
                  flexDirection: "column",
                  gap: 12,
                }}
              >
                <div style={{ borderBottom: "1px solid " + ACID, paddingBottom: 8, fontWeight: "bold" }}>
                  🧠 AGENT COOKING...
                </div>
                {logs.slice(0, Math.floor((frame - fps * 1.5) / 8) + 1).map((log, idx) => (
                  <div key={idx} style={{ wordBreak: "break-all" }}>{log}</div>
                ))}
              </div>
            )}
          </PhoneFrame>

          <div style={{ width: 800, textAlign: "left" }}>
            <MemeText size={64} color={ACID} style={{ textAlign: "left" }}>SWIPE RIGHT.</MemeText>
            <MemeText size={64} color={PINK} style={{ marginTop: 12, textAlign: "left" }}>AGENT COOKS IT.</MemeText>
            <MemeText size={64} style={{ marginTop: 12, textAlign: "left" }}>PR CARD OPENS.</MemeText>
            {/* karaoke moved to scene level (see below) */}
          </div>
        </div>
      ) : (
        // Explode to real PR card
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            gap: 30,
          }}
        >
          <Boom volume={0.75} />
          <PrCard scale={spring({ fps, frame: frame - fps * 4.0, config: { damping: 10 } })} />
          <MemeText size={84} color={ACID}>SWEPT IT. SHIPPED IT.</MemeText>
        </div>
      )}

      <Karaoke words={CAPTIONS.swipe} bottom={50} size={50} color={INK} />
      <Scanlines />
      <Flash at={fps * 4} />
    </AbsoluteFill>
  );
};

/** 11. Price Gag (5.5s) */
const PriceScene: React.FC = () => {
  const { fps } = useVideoConfig();
  const frame = useCurrentFrame();
  const shake = useShake(8);

  return (
    <AbsoluteFill
      style={{
        background: BG,
        justifyContent: "center",
        alignItems: "center",
        flexDirection: "column",
        gap: 20,
        transform: shake,
      }}
    >
      <Boom volume={0.55} />
      <MemeText size={64} color={PINK}>PRICED TO BEAT ALL OTHERS.</MemeText>
      <Karaoke words={CAPTIONS.price} bottom={30} size={56} color={INK} />

      <div style={{ display: "flex", gap: 30, alignItems: "center", height: 400, position: "relative" }}>
        <Starburst size={380} bg="#ffd400" color="#c00" style={{ transform: "rotate(-6deg)" }}>
          $0.03 A CLIP!
        </Starburst>

        <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
          <div style={{ display: "flex", gap: 20, alignItems: "baseline" }}>
            <span style={{ fontSize: 34, color: "#8b949e", textDecoration: "line-through", textDecorationColor: "#c00" }}>$1.20/clip</span>
            <span style={{ fontSize: 76, fontFamily: IMPACT, color: ACID }}>→ $0.03/clip</span>
          </div>
          <div style={{ fontFamily: MONO, fontSize: 26, color: INK, background: "#161b22", border: "1px solid #30363d", padding: 16, borderRadius: 8 }}>
            === RECEIPT ===<br />
            this entire feed cost $2.80<br />
          </div>
        </div>
      </div>

      <MemeText size={52} color={ACID} delay={Math.round(2.0 * fps)}>
        OPERATORS ARE STANDING BY.
      </MemeText>
      <Scanlines />
      <Flash />
    </AbsoluteFill>
  );
};

/** 12. CTA / Install (8.0s) */
const CtaScene: React.FC = () => {
  const { fps } = useVideoConfig();
  const frame = useCurrentFrame();
  const typedChars = Math.floor(frame / 2);
  const command = "brew install misty-step/doomscrum/doomscrum";
  const typedCommand = command.slice(0, typedChars);

  return (
    <AbsoluteFill
      style={{
        background: BG,
        justifyContent: "center",
        alignItems: "center",
        flexDirection: "column",
        gap: 30,
      }}
    >
      <Riser volume={0.5} />
      <MemeText size={84} color={ACID}>GET DOOMSCRUM NOW.</MemeText>
      <Karaoke words={CAPTIONS.cta} bottom={30} size={56} color={INK} />

      {/* Cool animated Terminal shell */}
      <div
        style={{
          width: 1100,
          background: "#161b22",
          border: "3px solid " + ACID,
          borderRadius: 16,
          boxShadow: `0 10px 40px rgba(182,255,46,0.15)`,
          padding: 40,
          fontFamily: MONO,
          fontSize: 32,
          color: "#fff",
          textAlign: "left",
        }}
      >
        <div style={{ color: "#8b949e", borderBottom: "1px solid #30363d", paddingBottom: 16, marginBottom: 20, fontSize: 20 }}>
          Terminal — moomooskycow@macbook
        </div>
        <div style={{ lineHeight: 1.6 }}>
          <span style={{ color: ACID }}>$</span> {typedCommand}
          {frame % 16 < 8 && <span style={{ color: ACID }}>_</span>}
        </div>
        {frame > command.length * 2 && (
          <div style={{ color: "#8b949e", marginTop: 12, fontSize: 28 }}>
            ==&gt; Installing doomscrum...<br />
            ==&gt; Fetching brainrot codecs...<br />
            ==&gt; Swiping system active! 🚀
          </div>
        )}
      </div>

      <div style={{ fontFamily: IMPACT, fontSize: 48, color: PINK, textTransform: "uppercase", letterSpacing: "1px", margin: "20px 0" }}>
        github.com/misty-step/doomscrum
      </div>

      <VhsTag label="REC ●" />
      <Scanlines />
    </AbsoluteFill>
  );
};

// ----- the composition ------------------------------------------------------
export const Launch: React.FC = () => {
  return (
    <AbsoluteFill style={{ background: "#000" }}>
      {/* 120 BPM Backing music track */}
      <Audio src={staticFile("sfx/loop.wav")} volume={musicVolume} />

      {/* Narrative sequences */}
      <Sequence from={startOf("hook")} durationInFrames={sec(LAUNCH_T.hook)}>
        <Audio src={staticFile("vo/final/hook.wav")} volume={0.9} />
        <HookScene />
      </Sequence>

      <Sequence from={startOf("problem")} durationInFrames={sec(LAUNCH_T.problem)}>
        <Audio src={staticFile("vo/final/problem.wav")} volume={0.9} />
        <ProblemScene />
      </Sequence>

      <Sequence from={startOf("product1")} durationInFrames={sec(LAUNCH_T.product1)}>
        <Audio src={staticFile("vo/final/product1.wav")} volume={0.9} />
        <Product1Scene />
      </Sequence>

      <Sequence from={startOf("title")} durationInFrames={sec(LAUNCH_T.title)}>
        <TitleScene />
      </Sequence>

      <Sequence from={startOf("product2")} durationInFrames={sec(LAUNCH_T.product2)}>
        <Audio src={staticFile("vo/final/product2.wav")} volume={0.9} />
        <Product2Scene />
      </Sequence>

      <Sequence from={startOf("tapSpec")} durationInFrames={sec(LAUNCH_T.tapSpec)}>
        <TapSpecScene />
      </Sequence>

      <Sequence from={startOf("proofJoke")} durationInFrames={sec(LAUNCH_T.proofJoke)}>
        <ProofJokeScene />
      </Sequence>

      <Sequence from={startOf("proofQA")} durationInFrames={sec(LAUNCH_T.proofQA)}>
        <ProofQAScene />
      </Sequence>

      <Sequence from={startOf("proofGoblin")} durationInFrames={sec(LAUNCH_T.proofGoblin)}>
        <ProofGoblinScene />
      </Sequence>

      <Sequence from={startOf("proofJanitor")} durationInFrames={sec(LAUNCH_T.proofJanitor)}>
        <ProofJanitorScene />
      </Sequence>

      <Sequence from={startOf("swipe")} durationInFrames={sec(LAUNCH_T.swipe)}>
        <Audio src={staticFile("vo/final/swipe.wav")} volume={0.9} />
        <SwipeScene />
      </Sequence>

      <Sequence from={startOf("price")} durationInFrames={sec(LAUNCH_T.price)}>
        <Audio src={staticFile("vo/final/price.wav")} volume={0.9} />
        <PriceScene />
      </Sequence>

      <Sequence from={startOf("cta")} durationInFrames={sec(LAUNCH_T.cta)}>
        <Audio src={staticFile("vo/final/cta.wav")} volume={0.9} />
        <CtaScene />
      </Sequence>
    </AbsoluteFill>
  );
};
