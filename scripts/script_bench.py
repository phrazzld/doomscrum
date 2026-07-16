#!/usr/bin/env python3
"""Brainrot script bench: models x prompts x specs, LLM-judged.

The eval harness for DoomScrum's scriptwriter. Generates a script+scene for
every (spec, model, prompt) cell via OpenRouter, then has two non-contestant
frontier judges score each result against the spec. Artifacts land in
.doomscrum/bench/<run>/ — generations, judgments, and a report.md matrix.

Usage:
    python3 scripts/script_bench.py            # full run (~$0.60)
    python3 scripts/script_bench.py --judge-only <run-dir>   # re-judge

Key: OPENROUTER_API_KEY (env or ~/.secrets). OPENROUTER_BASE_URL optionally
points at an OpenAI-compatible proxy. Neither value is printed.
"""

import json
import os
import re
import sys
import time
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
from pathlib import Path

BASE = os.environ.get(
    "OPENROUTER_BASE_URL", "https://openrouter.ai/api/v1/chat/completions"
)
DURATION = 12
BUDGET = 20  # word_budget(12) in src/distill.rs
ROOT = Path(__file__).resolve().parent.parent

MODELS = [
    "openai/gpt-5.4-mini",         # prior production default; contract baseline
    "moonshotai/kimi-k2.6",        # 2026-06-11 bench winner, prior default
    "moonshotai/kimi-k2.5",        # cheaper sibling
    "deepseek/deepseek-v4-flash",  # price floor
    "google/gemini-3-flash-preview",
    "minimax/minimax-m2.5",
    "z-ai/glm-5",
    # 2026-07-16 owner-requested candidates; slugs verified against the live
    # OpenRouter /models catalog that day. Leaderboard reputation is signal,
    # not verdict — only this bench (spec_fidelity/clarity/brainrot_energy/
    # stranger_recall on real tickets) decides adoption.
    "anthropic/claude-fable-5",    # $10/$50 — EQ-bench creative #1, best comedic voice
    "anthropic/claude-sonnet-5",   # $2/$10 — Anthropic mid-tier
    "openai/gpt-5.6-luna",         # $1/$6 — cheap 5.6 variant
    "openai/gpt-5.6-sol",          # $5/$30 — 5.6 flagship, structured/punchy humor
    "x-ai/grok-4.5",               # $2/$6 — rep: weaker creative tone; sibling of judge grok-4.3
    "meta/muse-spark-1.1",         # $1.25/$4.25 — quirky, coherent
    "moonshotai/kimi-k3",          # $3/$15 — unverified rep, RP-leaning
    "aion-labs/aion-3.0",          # $3/$6 — owner-named lab
    # 2026-07-16 affordable finalists from Crucible doomscrum-script-v0.
    "mistralai/mistral-small-2603",        # $0.15/$0.60 — 11/12 deterministic
    "google/gemini-3.1-flash-lite",        # production default; judged affordable winner
    "qwen/qwen3.5-flash-02-23",            # $0.065/$0.26 — 11/12 deterministic
    "bytedance-seed/seed-2.0-mini",        # $0.10/$0.40 — 11/12 deterministic
    "openai/gpt-5.4-nano",                 # $0.20/$1.25 — 11/12 deterministic
]

JUDGES = ["google/gemini-3.1-pro-preview", "x-ai/grok-4.3"]

SPECS = [
    # In-repo fixture copies of real specs (the four behind the 2026-07-15
    # launch clips the owner flagged as hard to understand). The live
    # backlog moved to GitHub issues, so the bench pins these snapshots.
    ("joke-046-meme-audit", ROOT / "docs/bench/fixtures/046-meme-product-greatness-audit.md"),
    ("qa-013-persona-agent", ROOT / "docs/bench/fixtures/013-persona-qa-agent.md"),
    ("goblin-028-open-weights", ROOT / "docs/bench/fixtures/028-local-open-weights-provider.md"),
    ("janitor-038-purge-artifacts", ROOT / "docs/bench/fixtures/038-purge-orphaned-artifacts.md"),
]

SHARED_RULES = f"""Reply with STRICT JSON, no markdown fences, exactly two keys:
{{"script": "...", "scene": "..."}}
script: the complete spoken dialogue for a {DURATION}-second vertical video. \
HARD LIMIT {BUDGET} words. Never invent features, metrics, or claims the spec \
doesn't make. No hashtags, no emoji.
scene: one vivid paragraph for a text-to-video model describing the character \
and setting that DELIVERS the script — who speaks, where, camera energy. \
Do NOT include the dialogue text inside the scene."""

PROMPTS = {
    # P1: the original coverage-first production prompt (replaced by P3 after
    # the 2026-06-11 run; kept as the regression baseline).
    "p1-production": f"""You write scripts for {DURATION}-second vertical brainrot videos. Each video \
communicates one software backlog spec — the user input is the raw spec file, in \
whatever format it happens to be. Your job: the tightest, clearest, funniest \
possible articulation of WHAT the spec wants and (when stated) WHEN it counts as \
done. The spec is the content; the brainrot is the delivery.
{SHARED_RULES}
Go absurd with the scene. House favorites for inspiration — a talking-fruit soap \
opera, a 90s infomercial pitchman, a cryptid filming a selfie vlog, an \
Italian-brainrot hybrid creature reveal, a street interview in the year 3024, a \
deadpan gen-z explainer — but inventing something equally unhinged is encouraged.""",
    # P2: natural speech > coverage. Attacks the staccato-fragment failure.
    "p2-natural": f"""You write the spoken script for a {DURATION}-second vertical brainrot video that \
communicates one software backlog spec (the user input, raw, any format).
Write 2-3 NATURAL, speakable sentences — the way an unhinged TikTok narrator \
actually talks. Flow beats coverage: pick the ONE thing the spec wants and (if \
stated) the moment it counts as done, and say that clearly. Dropping detail is \
correct; telegraphic fragment chains ("Dupes get receipt. HTTP tests.") are an \
automatic failure.
{SHARED_RULES}
Scene: invent an absurd character/setting (talking fruit, pitchman, cryptid \
vlog, hybrid creature, year-3024 interview, deadpan explainer, or something \
equally unhinged of your own).""",
    # P3: persona-first. Won 2026-06-11; now production in scriptwriter.rs.
    "p3-persona": f"""You create {DURATION}-second vertical brainrot videos that communicate software \
backlog specs (the user input is one raw spec, any format).
Work persona-first: FIRST invent one absurd character with a strong voice (a \
talking fruit in a soap opera, a 90s pitchman, a cryptid vlogger, an \
Italian-brainrot hybrid creature, a year-3024 street interviewee, a deadpan \
gen-z explainer, or something funnier you invent). THEN write the script as \
that character speaking IN VOICE — their verbal tics, their stakes, their \
drama — while still landing, unmistakably, WHAT the spec wants and (if stated) \
when it counts as done. The character serves the spec, never buries it.
{SHARED_RULES}""",
    # P4: persona-first + plain-open comprehension contract. Candidate fix
    # for the 2026-07-16 owner complaint: clips were funny but a viewer
    # couldn't tell what the ticket actually asked for.
    "p4-plain-open": f"""You create {DURATION}-second vertical brainrot videos that communicate software \
backlog specs (the user input is one raw spec, any format).
Work persona-first: FIRST invent one absurd character with a strong voice (a \
talking fruit in a soap opera, a 90s pitchman, a cryptid vlogger, an \
Italian-brainrot hybrid creature, a year-3024 street interviewee, a deadpan \
gen-z explainer, or something funnier you invent). THEN write the script as \
that character speaking IN VOICE — their verbal tics, their stakes, their drama.
THE COMPREHENSION CONTRACT, non-negotiable: a stranger who has never seen this \
backlog must be able to answer "what does this ticket ask for?" after one \
listen. The FIRST sentence names the ask in plain words — the character may \
say it in voice, but the subject and the want must be literal, never a riddle. \
Slang carries the delivery, never the content words: keep the spec's own \
concrete nouns (the feature, the artifact, the action). If a line doesn't help \
the stranger answer, cut it and spend the words on the ask.
{SHARED_RULES}""",
}

JUDGE_PROMPT = """You are judging a script for a {duration}-second vertical brainrot video whose \
job is to communicate a software backlog spec to a developer scrolling a feed.

THE SPEC (ground truth):
---
{spec}
---

THE SCRIPT (spoken words): "{script}"
THE SCENE (visual concept): "{scene}"

A STRANGER (a developer who never saw the spec) watched the clip and said the \
ticket asks for: "{stranger}"

Score 0-10 on each dimension and reply with STRICT JSON only:
{{"spec_fidelity": n, "clarity": n, "brainrot_energy": n, "speakability": n, "stranger_recall": n, "overall": n, "verdict": "<one sentence>"}}

spec_fidelity: does the script state what the spec actually wants, without inventing claims? A script that could describe any ticket scores low.
clarity: would the scrolling developer know what this ticket IS after one listen?
brainrot_energy: does the SCENE + character delivery land as unhinged short-form content? The spoken words are SUPPOSED to state the ask literally — the scene, the character, and the delivery carry the joke. Never penalize the script for plain content words; penalize a boring scene, a characterless delivery, or a scene that fights the message.
speakability: does it sound like natural speech a character could deliver in {duration}s (max {budget} words), or fragment soup?
stranger_recall: does the stranger's takeaway match the spec's actual ask? 10 = they could file the same ticket; 0 = they got it wrong or vague.
overall: your holistic quality call — weight spec_fidelity, clarity, and stranger_recall highest; brainrot is the delivery, the spec is the content."""

STRANGER_MODEL = "openai/gpt-5.4-mini"
STRANGER_PROMPT = """You are a developer scrolling a shortform feed. You just watched a {duration}-second \
video. You have NEVER seen the backlog it came from.

The spoken words were: "{script}"
The visuals showed: "{scene}"

In ONE plain sentence: what does this backlog ticket ask for? If you honestly \
can't tell, say exactly what remains unclear. No hedging boilerplate."""


def get_key():
    v = os.environ.get("OPENROUTER_API_KEY", "").strip()
    if v:
        return v
    secrets = Path.home() / ".secrets"
    if secrets.exists():
        for line in secrets.read_text().splitlines():
            line = line.strip().removeprefix("export ").strip()
            if line.startswith("OPENROUTER_API_KEY="):
                return line.split("=", 1)[1].strip().strip("\"'")
    sys.exit("no OPENROUTER_API_KEY in env or ~/.secrets")


def chat(key, model, system, user, temperature, retries=2):
    body = json.dumps(
        {
            "model": model,
            "temperature": temperature,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        }
    ).encode()
    for attempt in range(retries + 1):
        try:
            req = urllib.request.Request(
                BASE,
                data=body,
                headers={
                    "Authorization": f"Bearer {key}",
                    "Content-Type": "application/json",
                },
            )
            with urllib.request.urlopen(req, timeout=120) as resp:
                payload = json.load(resp)
            return payload["choices"][0]["message"]["content"]
        except Exception as e:  # noqa: BLE001 — bench tool, retry then surface
            if attempt == retries:
                raise
            time.sleep(2 * (attempt + 1))


def parse_json_reply(content):
    t = content.strip()
    m = re.search(r"\{.*\}", t, re.S)
    if not m:
        raise ValueError(f"no JSON object in reply: {t[:120]}")
    return json.loads(m.group(0))


def generate_cell(key, spec_name, spec_text, model, prompt_name):
    t0 = time.time()
    try:
        content = chat(key, model, PROMPTS[prompt_name], spec_text, 0.9)
        obj = parse_json_reply(content)
        script, scene = obj["script"].strip(), obj["scene"].strip()
        return {
            "spec": spec_name,
            "model": model,
            "prompt": prompt_name,
            "ok": True,
            "script": script,
            "scene": scene,
            "words": len(script.split()),
            "latency_s": round(time.time() - t0, 1),
        }
    except Exception as e:  # noqa: BLE001
        return {
            "spec": spec_name,
            "model": model,
            "prompt": prompt_name,
            "ok": False,
            "error": str(e)[:300],
            "latency_s": round(time.time() - t0, 1),
        }


def judge_cell(key, gen, spec_text):
    scores = {}
    for judge in JUDGES:
        try:
            content = chat(
                key,
                judge,
                "You are a strict, evidence-based judge. JSON only.",
                JUDGE_PROMPT.format(
                    duration=DURATION,
                    budget=BUDGET,
                    spec=spec_text,
                    script=gen["script"],
                    scene=gen["scene"],
                    stranger=gen.get("stranger", "(no probe run)"),
                ),
                0.0,
            )
            scores[judge] = parse_json_reply(content)
        except Exception as e:  # noqa: BLE001
            scores[judge] = {"error": str(e)[:200]}
    return scores


def probe_cell(key, gen):
    """Stranger test: a model that never saw the spec reconstructs the ask."""
    try:
        gen["stranger"] = chat(
            key,
            STRANGER_MODEL,
            "Answer in one sentence.",
            STRANGER_PROMPT.format(duration=DURATION, script=gen["script"], scene=gen["scene"]),
            0.0,
        ).strip()
    except Exception as e:  # noqa: BLE001
        gen["stranger"] = f"(probe failed: {str(e)[:120]})"
    return gen


def slug(*parts):
    return "__".join(p.replace("/", "-") for p in parts)


def write_report(run_dir, gens, judgments):
    def mean_overall(g):
        vals = [
            j["overall"]
            for j in judgments.get(slug(g["spec"], g["model"], g["prompt"]), {}).values()
            if isinstance(j.get("overall"), (int, float))
        ]
        return sum(vals) / len(vals) if vals else None

    lines = [
        f"# Script bench {run_dir.name}",
        "",
        f"{len(gens)} generations · models={len({g['model'] for g in gens})} "
        f"prompts={len({g['prompt'] for g in gens})} "
        f"specs={len({g['spec'] for g in gens})} · judges: {', '.join(JUDGES)}",
        "",
        "## Leaderboard (mean overall, both judges, all specs)",
        "",
        "| model | prompt | mean overall | fidelity | clarity | energy | speak | budget viol |",
        "|---|---|---|---|---|---|---|---|",
    ]
    combos = {}
    for g in gens:
        if not g["ok"]:
            continue
        key = (g["model"], g["prompt"])
        combos.setdefault(key, []).append(g)
    rows = []
    for (model, prompt), cells in combos.items():
        dims = {d: [] for d in ["overall", "spec_fidelity", "clarity", "brainrot_energy", "speakability"]}
        viol = sum(1 for c in cells if c["words"] > BUDGET + 3)
        for c in cells:
            for j in judgments.get(slug(c["spec"], c["model"], c["prompt"]), {}).values():
                for d in dims:
                    if isinstance(j.get(d), (int, float)):
                        dims[d].append(j[d])
        m = {d: (sum(v) / len(v) if v else 0) for d, v in dims.items()}
        rows.append((m["overall"], model, prompt, m, viol, len(cells)))
    rows.sort(reverse=True)
    for overall, model, prompt, m, viol, n in rows:
        lines.append(
            f"| {model} | {prompt} | **{overall:.2f}** | {m['spec_fidelity']:.1f} "
            f"| {m['clarity']:.1f} | {m['brainrot_energy']:.1f} | {m['speakability']:.1f} "
            f"| {viol}/{n} |"
        )
    lines += ["", "## All generations", ""]
    for g in sorted(gens, key=lambda g: (g["spec"], g["prompt"], g["model"])):
        s = slug(g["spec"], g["model"], g["prompt"])
        lines.append(f"### {g['spec']} · {g['model']} · {g['prompt']}")
        if not g["ok"]:
            lines += [f"FAILED: {g['error']}", ""]
            continue
        mo = mean_overall(g)
        verdicts = " / ".join(
            str(j.get("verdict", j.get("error", "")))
            for j in judgments.get(s, {}).values()
        )
        lines += [
            f"- script ({g['words']}w): {g['script']}",
            f"- scene: {g['scene']}",
            f"- mean overall: {mo if mo is None else round(mo, 2)} — {verdicts}",
            "",
        ]
    (run_dir / "report.md").write_text("\n".join(lines))


def main():
    key = get_key()
    if len(sys.argv) > 2 and sys.argv[1] == "--judge-only":
        run_dir = Path(sys.argv[2])
        gens = [json.loads(l) for l in (run_dir / "generations.jsonl").read_text().splitlines()]
    else:
        run_dir = ROOT / ".doomscrum/bench" / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        run_dir.mkdir(parents=True)
        specs = {name: path.read_text() for name, path in SPECS}
        # Focused A/B runs: comma-separated substring filters, e.g.
        #   SCRIPT_BENCH_MODELS=gpt-5.4-mini SCRIPT_BENCH_PROMPTS=p3,p4
        model_f = [m for m in os.environ.get("SCRIPT_BENCH_MODELS", "").split(",") if m]
        prompt_f = [p for p in os.environ.get("SCRIPT_BENCH_PROMPTS", "").split(",") if p]
        models = [m for m in MODELS if not model_f or any(f in m for f in model_f)]
        prompts = [p for p in PROMPTS if not prompt_f or any(f in p for f in prompt_f)]
        cells = [
            (name, specs[name], model, prompt)
            for name in specs
            for model in models
            for prompt in prompts
        ]
        print(f"generating {len(cells)} cells -> {run_dir}")
        with ThreadPoolExecutor(max_workers=8) as pool:
            gens = list(pool.map(lambda c: generate_cell(key, *c), cells))
        with open(run_dir / "generations.jsonl", "w") as f:
            for g in gens:
                f.write(json.dumps(g) + "\n")
        failed = [g for g in gens if not g["ok"]]
        print(f"generated: {len(gens) - len(failed)} ok, {len(failed)} failed")

    specs = {name: path.read_text() for name, path in SPECS}
    ok = [g for g in gens if g["ok"]]
    print(f"stranger-probing {len(ok)} results")
    with ThreadPoolExecutor(max_workers=8) as pool:
        ok = list(pool.map(lambda g: probe_cell(key, g), ok))
    with open(run_dir / "generations.jsonl", "w") as f:
        for g in gens:
            f.write(json.dumps(g) + "\n")
    print(f"judging {len(ok)} results x {len(JUDGES)} judges")
    with ThreadPoolExecutor(max_workers=8) as pool:
        results = list(pool.map(lambda g: judge_cell(key, g, specs[g["spec"]]), ok))
    judgments = {slug(g["spec"], g["model"], g["prompt"]): r for g, r in zip(ok, results)}
    (run_dir / "judgments.json").write_text(json.dumps(judgments, indent=1))
    write_report(run_dir, gens, judgments)
    print(f"report: {run_dir / 'report.md'}")


if __name__ == "__main__":
    main()
