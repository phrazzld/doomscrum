import React from "react";
import {
  AbsoluteFill,
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
  Riser,
  Starburst,
  usePunchIn,
  useShake,
  VhsTag,
} from "./fx";

export const FPS = 30;
const sec = (s: number) => Math.round(s * FPS);

// ----- scene timing (seconds) ---------------------------------------------
// Clip scenes are sized from each render's measured speech-end (whisper
// transcript) + a held beat, so the demo never cuts a line mid-sentence.
// sora-2 clips run 12s, speak title + goal + criterion, and burn their own
// word-synced captions; the NOT-DONE-UNTIL ribbon repeats the criterion.
const T = {
  coldOpen: 9.8, // 001 fruit drama: speech ends 9.07s
  beat: 1.8,
  title: 2.4,
  clipA: 7.6, // 005 infomercial: speech ends 6.87s
  clipB: 11.3, // 006 cryptid: speech ends 10.59s
  swipe: 2.2,
  pr: 6.2,
  clipC: 10.1, // 007 italian: speech ends 9.38s
  clipD: 10.0, // 008 street interview: set from measured speech end
  close: 6.5,
};
const ORDER: (keyof typeof T)[] = [
  "coldOpen",
  "beat",
  "title",
  "clipA",
  "clipB",
  "swipe",
  "pr",
  "clipC",
  "clipD",
  "close",
];
const startOf = (k: keyof typeof T) =>
  sec(ORDER.slice(0, ORDER.indexOf(k)).reduce((a, x) => a + T[x], 0));
export const DEMO_DURATION_FRAMES = sec(ORDER.reduce((a, x) => a + T[x], 0));

// ----- palette -------------------------------------------------------------
const ACID = "#b6ff2e";
const PINK = "#ff2ea6";
const INK = "#e8ffe0";
const BG = "#07090b";
const IMPACT = "Impact, 'Arial Black', sans-serif";
const MONO = "ui-monospace, 'SF Mono', Menlo, monospace";

// ----- building blocks ------------------------------------------------------
const Scanlines: React.FC = () => (
  <AbsoluteFill
    style={{
      background:
        "repeating-linear-gradient(0deg, rgba(0,0,0,.22) 0 2px, transparent 2px 6px)",
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
      fontSize: 38,
      letterSpacing: "0.06em",
      padding: "14px 24px",
      border: "5px solid #000",
      boxShadow: "10px 10px 0 #000",
      background: bg,
      color,
      transform: `rotate(${rotate}deg)`,
      ...style,
    }}
  >
    {children}
  </div>
);

/** Infomercial-style criterion ribbon: the spec's "done when". */
const NotDoneUntil: React.FC<{ text: string }> = ({ text }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const enter = spring({
    fps,
    frame: frame - Math.round(0.8 * fps),
    config: { damping: 13, stiffness: 160 },
  });
  if (frame < Math.round(0.8 * fps)) return null;
  return (
    <div
      style={{
        position: "absolute",
        bottom: 64,
        left: 30,
        right: 30,
        transform: `translateY(${(1 - enter) * 120}px) rotate(-1.5deg)`,
        background: "#c00",
        border: "5px solid #fff",
        boxShadow: "10px 10px 0 #000",
        padding: "16px 22px",
        textAlign: "center",
      }}
    >
      <span
        style={{
          fontFamily: IMPACT,
          fontSize: 34,
          color: "#ffd400",
          textTransform: "uppercase",
          letterSpacing: "0.04em",
        }}
      >
        not done until:{" "}
      </span>
      <span
        style={{
          fontFamily: IMPACT,
          fontSize: 34,
          color: "#fff",
          textTransform: "uppercase",
        }}
      >
        {text}
      </span>
    </div>
  );
};

/** A real render in the app's phone frame: punch-in, shake, karaoke, ribbon. */
const PhoneClip: React.FC<{
  src: string;
  sticker: string;
  prio: string;
  caption: string;
  notDoneUntil: string;
  hint?: string;
}> = ({ src, sticker, prio, caption, notDoneUntil, hint }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const scale = usePunchIn(1.04);
  const shake = useShake(7);
  return (
    <AbsoluteFill style={{ background: BG, justifyContent: "center", alignItems: "center" }}>
      <Boom />
      <div
        style={{
          position: "relative",
          width: 760,
          aspectRatio: "9 / 16",
          border: "8px solid " + INK,
          boxShadow: `26px 26px 0 rgba(182,255,46,.35), 0 0 120px rgba(0,0,0,.9)`,
          background: "#000",
          transform: `${shake} scale(${scale})`,
        }}
      >
        <OffthreadVideo
          src={staticFile(src)}
          style={{ width: "100%", height: "100%", objectFit: "cover" }}
        />
        <Sticker style={{ top: -34, right: -44 }}>{sticker}</Sticker>
        <Sticker bg={PINK} color="#fff" rotate={-5} style={{ bottom: -30, left: -38 }}>
          {prio}
        </Sticker>
      </div>
      <div style={{ position: "absolute", top: 170, left: 60, right: 60 }}>
        <MemeText size={68} delay={Math.round(0.35 * fps)}>
          {caption}
        </MemeText>
      </div>
      <NotDoneUntil text={notDoneUntil} />
      {hint ? (
        <div
          style={{
            position: "absolute",
            top: 90,
            width: "100%",
            textAlign: "center",
            fontFamily: MONO,
            fontSize: 34,
            letterSpacing: "0.18em",
            color: ACID,
            textTransform: "uppercase",
            opacity: interpolate(frame, [fps, fps * 1.5], [0, 1], {
              extrapolateRight: "clamp",
            }),
          }}
        >
          {hint}
        </div>
      ) : null}
      <Scanlines />
      <Flash />
    </AbsoluteFill>
  );
};

const CardScene: React.FC<{ lines: { text: string; size?: number; color?: string }[] }> = ({
  lines,
}) => {
  const { fps } = useVideoConfig();
  const shake = useShake(4);
  return (
    <AbsoluteFill
      style={{
        background: BG,
        justifyContent: "center",
        alignItems: "center",
        flexDirection: "column",
        gap: 60,
        padding: 70,
        transform: shake,
      }}
    >
      <Hit />
      {lines.map((l, i) => (
        <MemeText key={i} size={l.size ?? 100} color={l.color ?? "#fff"} delay={Math.round(i * 0.4 * fps)}>
          {l.text}
        </MemeText>
      ))}
      <Scanlines />
      <Flash />
    </AbsoluteFill>
  );
};

// ----- scenes ---------------------------------------------------------------
const ColdOpen: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const scale = usePunchIn(1.05);
  return (
    <AbsoluteFill style={{ background: "#000" }}>
      <div style={{ position: "absolute", inset: 0, transform: `scale(${scale})` }}>
        <OffthreadVideo
          src={staticFile("fruit_drama.mp4")}
          style={{ width: "100%", height: "100%", objectFit: "cover" }}
        />
      </div>
      <VhsTag />
      {frame > fps * 1.4 ? (
        <div style={{ position: "absolute", top: 200, left: 50, right: 50 }}>
          <MemeText size={80}>this is one of our specs</MemeText>
        </div>
      ) : null}
      <Scanlines />
    </AbsoluteFill>
  );
};

const Title: React.FC = () => {
  const frame = useCurrentFrame();
  const jitter = Math.sin(frame * 1.7) * 2;
  const shake = useShake(8);
  return (
    <AbsoluteFill
      style={{ background: BG, justifyContent: "center", alignItems: "center", flexDirection: "column", gap: 40, transform: shake }}
    >
      <Boom volume={0.85} />
      <div
        style={{
          fontFamily: IMPACT,
          fontSize: 190,
          color: ACID,
          textTransform: "uppercase",
          WebkitTextStroke: "5px #000",
          textShadow: `14px 14px 0 ${PINK}`,
          transform: `rotate(${jitter * 0.2}deg) translateX(${jitter}px)`,
        }}
      >
        DoomScrum
      </div>
      <div
        style={{
          fontFamily: MONO,
          fontSize: 44,
          letterSpacing: "0.3em",
          color: INK,
          textTransform: "uppercase",
        }}
      >
        doomscroll your backlog
      </div>
      <Scanlines />
      <Flash />
    </AbsoluteFill>
  );
};

const SwipeRight: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const x = interpolate(frame, [fps * 0.6, fps * 1.9], [0, 1400], {
    easing: Easing.in(Easing.cubic),
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const rot = interpolate(frame, [fps * 0.6, fps * 1.9], [0, 18], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  return (
    <AbsoluteFill style={{ background: BG, justifyContent: "center", alignItems: "center" }}>
      <Riser />
      <div
        style={{
          width: 700,
          aspectRatio: "9 / 16",
          border: "8px solid " + INK,
          background: "#000",
          boxShadow: `26px 26px 0 rgba(182,255,46,.35)`,
          transform: `translateX(${x}px) rotate(${rot}deg)`,
          overflow: "hidden",
        }}
      >
        <OffthreadVideo
          src={staticFile("infomercial.mp4")}
          muted
          style={{ width: "100%", height: "100%", objectFit: "cover", opacity: 0.85 }}
        />
      </div>
      <div style={{ position: "absolute", bottom: 260, left: 50, right: 50 }}>
        <MemeText size={110} color={ACID}>
          swipe right…
        </MemeText>
      </div>
      <div
        style={{
          position: "absolute",
          fontFamily: IMPACT,
          fontSize: 240,
          color: ACID,
          textShadow: "10px 10px 0 #000",
          opacity: interpolate(frame, [0, fps * 0.5], [0, 1], { extrapolateRight: "clamp" }),
        }}
      >
        →
      </div>
      <Scanlines />
    </AbsoluteFill>
  );
};

/** GitHub-dark PR card with the real PR #1 data, plus the infomercial turn. */
const PrScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const enter = spring({ fps, frame, config: { damping: 13 } });
  return (
    <AbsoluteFill
      style={{ background: "#0d1117", justifyContent: "center", alignItems: "center", flexDirection: "column", gap: 50, padding: 60 }}
    >
      <Boom />
      <Starburst delay={Math.round(0.3 * fps)} style={{ top: 40, right: 30 }} size={420}>
        but wait, there&apos;s more
      </Starburst>
      <div
        style={{
          width: 920,
          background: "#161b22",
          border: "1px solid #30363d",
          borderRadius: 18,
          padding: 50,
          transform: `scale(${enter})`,
          fontFamily: "-apple-system, 'Segoe UI', Helvetica, sans-serif",
          color: "#e6edf3",
        }}
      >
        <div style={{ fontSize: 46, fontWeight: 600 }}>
          Shape: Cache Chaos Exorcism <span style={{ color: "#8b949e" }}>#1</span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 18, marginTop: 28 }}>
          <span
            style={{
              background: "#238636",
              borderRadius: 999,
              padding: "10px 26px",
              fontSize: 30,
              fontWeight: 600,
            }}
          >
            ✓ Open
          </span>
          <span style={{ fontFamily: MONO, fontSize: 26, color: "#8b949e" }}>
            doomscrum/shape-cache-chaos-exorcism → main
          </span>
        </div>
        <div style={{ marginTop: 30, fontSize: 30, color: "#8b949e" }}>
          <span style={{ color: "#3fb950" }}>+76</span>{" "}
          <span style={{ color: "#f85149" }}>−7</span> · 1 commit · opened by a swipe
        </div>
        <div
          style={{
            marginTop: 30,
            borderTop: "1px solid #30363d",
            paddingTop: 26,
            fontSize: 28,
            color: "#8b949e",
          }}
        >
          🤖 The agent read the live codebase and rewrote the spec with
          repo-aware acceptance criteria.
        </div>
      </div>
      <MemeText size={84} delay={Math.round(0.5 * fps)}>
        a real agent implements it
      </MemeText>
      <MemeText size={84} color={ACID} delay={Math.round(1.1 * fps)}>
        and opens a real PR
      </MemeText>
      <MemeText size={56} color={PINK} delay={Math.round(2.1 * fps)}>
        (PR #1 happened. it&apos;s real.)
      </MemeText>
      <Flash />
    </AbsoluteFill>
  );
};

const Close: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const jitter = Math.sin(frame * 1.7) * 2;
  return (
    <AbsoluteFill
      style={{ background: BG, justifyContent: "center", alignItems: "center", flexDirection: "column", gap: 44 }}
    >
      <Boom volume={0.8} />
      <div
        style={{
          fontFamily: IMPACT,
          fontSize: 170,
          color: ACID,
          textTransform: "uppercase",
          WebkitTextStroke: "5px #000",
          textShadow: `14px 14px 0 ${PINK}`,
          transform: `translateX(${jitter}px)`,
        }}
      >
        DoomScrum
      </div>
      <div style={{ display: "flex", gap: 30, alignItems: "center" }}>
        <span
          style={{
            fontFamily: IMPACT,
            fontSize: 54,
            color: "#8b949e",
            textDecoration: "line-through",
            textDecorationColor: "#c00",
            textDecorationThickness: 6,
          }}
        >
          $5,000 explainer video
        </span>
        <span style={{ fontFamily: IMPACT, fontSize: 72, color: "#ffd400", WebkitTextStroke: "2px #000" }}>
          $0.40
        </span>
      </div>
      <div style={{ fontFamily: MONO, fontSize: 36, color: INK, letterSpacing: "0.1em" }}>
        built in rust · videos by fal · agents by codex
      </div>
      <div
        style={{
          fontFamily: MONO,
          fontSize: 44,
          color: "#fff",
          background: "#161b22",
          border: `4px solid ${ACID}`,
          boxShadow: "10px 10px 0 #000",
          padding: "22px 40px",
          opacity: interpolate(frame, [fps, fps * 1.6], [0, 1], { extrapolateRight: "clamp" }),
        }}
      >
        github.com/phrazzld/doomscrum
      </div>
      <MemeText size={60} color={PINK} delay={Math.round(2.2 * fps)}>
        operators are standing by
      </MemeText>
      <VhsTag label="REC ●" />
      <Scanlines />
      <Flash />
    </AbsoluteFill>
  );
};

// ----- the demo -------------------------------------------------------------
export const Demo: React.FC = () => (
  <AbsoluteFill style={{ background: "#000" }}>
    <Sequence from={startOf("coldOpen")} durationInFrames={sec(T.coldOpen)}>
      <ColdOpen />
    </Sequence>
    <Sequence from={startOf("beat")} durationInFrames={sec(T.beat)}>
      <CardScene
        lines={[
          { text: "yes. a spec.", size: 120 },
          { text: "(a strawberry is narrating our cache bug)", size: 56, color: ACID },
        ]}
      />
    </Sequence>
    <Sequence from={startOf("title")} durationInFrames={sec(T.title)}>
      <Title />
    </Sequence>
    <Sequence from={startOf("clipA")} durationInFrames={sec(T.clipA)}>
      <PhoneClip
        src="infomercial.mp4"
        sticker="new format"
        prio="#2"
        caption="every spec becomes brainrot"
        notDoneUntil="rate a clip from cursed to corporate"
      />
    </Sequence>
    <Sequence from={startOf("clipB")} durationInFrames={sec(T.clipB)}>
      <PhoneClip
        src="cryptid_vlog.mp4"
        sticker="fresh"
        prio="#3"
        caption="accurate. word for word."
        notDoneUntil="excess swipes queue with visible status"
        hint="↑ skip · tap = read the actual spec"
      />
    </Sequence>
    <Sequence from={startOf("swipe")} durationInFrames={sec(T.swipe)}>
      <SwipeRight />
    </Sequence>
    <Sequence from={startOf("pr")} durationInFrames={sec(T.pr)}>
      <PrScene />
    </Sequence>
    <Sequence from={startOf("clipC")} durationInFrames={sec(T.clipC)}>
      <PhoneClip
        src="italian_brainrot.mp4"
        sticker="fresh"
        prio="#4"
        caption="six formats. no two alike."
        notDoneUntil="media streams only the requested byte range"
      />
    </Sequence>
    <Sequence from={startOf("clipD")} durationInFrames={sec(T.clipD)}>
      <PhoneClip
        src="street_interview.mp4"
        sticker="pr opened"
        prio="#5"
        caption="your specs deserve to be seen"
        notDoneUntil="doomscrum gc removes superseded renders"
      />
    </Sequence>
    <Sequence from={startOf("close")} durationInFrames={sec(T.close)}>
      <Close />
    </Sequence>
  </AbsoluteFill>
);
