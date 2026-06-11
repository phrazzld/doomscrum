#!/usr/bin/env python3
"""Brainrot script bench: models x prompts x specs, LLM-judged.

The eval harness for DoomScrum's scriptwriter. Generates a script+scene for
every (spec, model, prompt) cell via OpenRouter, then has two non-contestant
frontier judges score each result against the spec. Artifacts land in
.doomscrum/bench/<run>/ — generations, judgments, and a report.md matrix.

Usage:
    python3 scripts/script_bench.py            # full run (~$0.60)
    python3 scripts/script_bench.py --judge-only <run-dir>   # re-judge

Key: OPENROUTER_API_KEY (env or ~/.secrets). Never printed.
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

BASE = "https://openrouter.ai/api/v1/chat/completions"
DURATION = 12
BUDGET = 20  # word_budget(12) in src/distill.rs
ROOT = Path(__file__).resolve().parent.parent

MODELS = [
    "moonshotai/kimi-k2.6",        # current production default
    "moonshotai/kimi-k2.5",        # cheaper sibling
    "deepseek/deepseek-v4-flash",  # price floor
    "google/gemini-3-flash-preview",
    "openai/gpt-5.4-mini",
    "minimax/minimax-m2.5",
    "z-ai/glm-5",
]

JUDGES = ["google/gemini-3.1-pro-preview", "x-ai/grok-4.3"]

SPECS = [
    ("doomscrum-006-throttle", ROOT / "backlog.d/006-throttle-every-money-path.md"),
    ("doomscrum-005-vibe-meter", ROOT / "backlog.d/005-brainrot-vibe-meter.md"),
    ("doomscrum-016-arbitrary-repos", ROOT / "backlog.d/016-multi-repo-sync.md"),
    (
        "olympus-072-pi-openrouter",
        Path.home()
        / "Development/adminifi/olympus/backlog.d/072-pi-openrouter-runtime-transition.md",
    ),
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
}

JUDGE_PROMPT = """You are judging a script for a {duration}-second vertical brainrot video whose \
job is to communicate a software backlog spec to a developer scrolling a feed.

THE SPEC (ground truth):
---
{spec}
---

THE SCRIPT (spoken words): "{script}"
THE SCENE (visual concept): "{scene}"

Score 0-10 on each dimension and reply with STRICT JSON only:
{{"spec_fidelity": n, "clarity": n, "brainrot_energy": n, "speakability": n, "overall": n, "verdict": "<one sentence>"}}

spec_fidelity: does the script state what the spec actually wants, without inventing claims? A script that could describe any ticket scores low.
clarity: would the scrolling developer know what this ticket IS after one listen?
brainrot_energy: is it actually funny/unhinged short-form content, or corporate copy with a costume?
speakability: does it sound like natural speech a character could deliver in {duration}s (max {budget} words), or fragment soup?
overall: your holistic quality call — weight spec_fidelity and clarity highest; brainrot is the delivery, the spec is the content."""


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
                ),
                0.0,
            )
            scores[judge] = parse_json_reply(content)
        except Exception as e:  # noqa: BLE001
            scores[judge] = {"error": str(e)[:200]}
    return scores


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
        f"{len(gens)} generations · models={len(MODELS)} prompts={len(PROMPTS)} "
        f"specs={len(SPECS)} · judges: {', '.join(JUDGES)}",
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
        cells = [
            (name, specs[name], model, prompt)
            for name in specs
            for model in MODELS
            for prompt in PROMPTS
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
    print(f"judging {len(ok)} results x {len(JUDGES)} judges")
    with ThreadPoolExecutor(max_workers=8) as pool:
        results = list(pool.map(lambda g: judge_cell(key, g, specs[g["spec"]]), ok))
    judgments = {slug(g["spec"], g["model"], g["prompt"]): r for g, r in zip(ok, results)}
    (run_dir / "judgments.json").write_text(json.dumps(judgments, indent=1))
    write_report(run_dir, gens, judgments)
    print(f"report: {run_dir / 'report.md'}")


if __name__ == "__main__":
    main()
