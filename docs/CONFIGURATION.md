# `doomscrum.toml` reference

Every field is optional and has a default — an empty or missing
`doomscrum.toml` runs fine (`doomscrum init` scaffolds one with these
defaults written out as comments). This page is the complete field
reference, verified against the config structs in `src/config.rs`; when the
two disagree, `src/config.rs` wins — it's the actual parser.

## `profile` (top level)

| Field | Type | Default | Meaning |
|---|---|---|---|
| `profile` | string | `""` (unset) | Active render profile, a key of `[profiles.*]`. Empty means "use the base `[video]` table as written." The `--profile` CLI flag overrides this for one run. Unknown profile names fail fast, naming the available profiles. |

See "Render profiles" in [OPERATIONS.md](OPERATIONS.md#render-profiles-dev-vs-content) for the `dev`/`content` split this exists to serve.

## `[repo]`

| Field | Type | Default | Meaning |
|---|---|---|---|
| `path` | string | `"."` | The repository DoomScrum is synced to. Backlog specs are read from `<path>/<backlog_dir>`, and agent worktrees are created from this repo. |
| `backlog_dir` | string | `"backlog.d"` | Backlog directory inside the synced repo. One markdown file per spec; files starting with `_` are ignored (use for `_done/`-style archives). |
| `state_dir` | string | `".doomscrum"` | Runtime state: renders, events, dispatch receipts, worktrees. Deleting this directory destroys only generated state, never specs. |

## `[feed]`

| Field | Type | Default | Meaning |
|---|---|---|---|
| `max_items` | integer | `10` | Caps the feed to the top N specs by priority (filename sort order). |
| `prefetch_depth` | integer | `3` | Just-in-time rendering window: serving the feed renders at most this many specs ahead of the viewport cursor, `[cursor, cursor + prefetch_depth)`. Specs deeper in the feed cost nothing until the cursor approaches them. `0` disables JIT rendering entirely (renders only happen via explicit `doomscrum generate`). |
| `render_max_attempts` | integer (u32) | `3` | A failed just-in-time render retries on a later feed poll, up to this many total attempts per spec. Bounds a persistent provider error from becoming a paid retry storm. |
| `render_retry_backoff_sec` | integer (u64) | `30` | Seconds a failed JIT render waits before it's eligible for its next attempt. |

## `[video]`

| Field | Type | Default | Meaning |
|---|---|---|---|
| `provider` | string | `"fake"` | `"fake"` renders an embedded fixture (offline, free, deterministic). `"fal"` generates real AI video via fal.ai — costs money, needs `FAL_API_KEY`. |
| `fal_model` | string | `"fal-ai/veo3.1/fast"` | The fal.ai model id used when `mix` is empty or `--model` forces a single pipeline. |
| `fal_base_url` | string | `"https://queue.fal.run"` | fal.ai queue API base URL. |
| `max_duration_sec` | integer (u32) | `8` | Clip duration in seconds for the single-pipeline path. |
| `price_per_second_usd` | float | `0.15` | Fallback $/s used for models not in the built-in price table (known models — see the pricing comment block in a fresh `doomscrum init` output — use their verified price automatically). |
| `max_total_spend_usd` | float | `25.0` | Hard wallet guard: real renders are refused once estimated total spend (summed from render provenance) would exceed this. |
| `max_daily_spend_usd` | float | `5.0` | Independent daily guard. Exceeding it returns HTTP `429` from feed routes and aborts CLI generation before the provider starts. |
| `mix` | array of `[[video.mix]]` tables | `[]` (empty) | Weighted render portfolio — see below. |

### `[[video.mix]]` entries

When non-empty, each spec deterministically draws one `(model, duration)` pair by content hash (stable across re-renders of the same spec), weighted across entries instead of every clip using the same pipeline.

| Field | Type | Default | Meaning |
|---|---|---|---|
| `model` | string | — (required) | fal.ai model id for this pipeline. |
| `duration_sec` | integer (u32) | — (required) | Clip duration in seconds for this pipeline. |
| `weight` | integer (u32) | `1` | Relative draw weight — weight `3` is picked ~3x as often as weight `1`. |

`[[video.mix]]` tables must come **after** the plain `[video]` keys in the file (TOML array-of-tables syntax requirement).

## `[script]`

| Field | Type | Default | Meaning |
|---|---|---|---|
| `mode` | string | `"llm"` | `"llm"` sends the full raw spec to an OpenRouter chat-completions model to write the spoken script + visual scene (real renders refuse to fall back silently). `"templates"` is the deterministic, offline, free template planner — used automatically by the `fake` provider. |
| `model` | string | `"openai/gpt-5.4-mini"` | OpenAI-compatible chat-completions model id, resolved via `base_url`. |
| `base_url` | string | `"https://openrouter.ai/api/v1"` | OpenAI-compatible API base. Key resolved from `OPENROUTER_API_KEY` (env or `~/.secrets`). |

## `[profiles.<name>]`

Partial `[video]` overrides, applied when `profile = "<name>"` is active (or `--profile <name>` is passed). Unset fields keep the base `[video]` value; an explicit `mix = []` clears the mix.

| Field | Type | Meaning |
|---|---|---|
| `provider` | string, optional | Overrides `video.provider`. |
| `fal_model` | string, optional | Overrides `video.fal_model`. |
| `max_duration_sec` | integer, optional | Overrides `video.max_duration_sec`. |
| `max_total_spend_usd` | float, optional | Overrides `video.max_total_spend_usd`. |
| `max_daily_spend_usd` | float, optional | Overrides `video.max_daily_spend_usd`. |
| `mix` | array, optional | Overrides `video.mix`. |

The two profiles a fresh `doomscrum init` ships are `dev` (`provider = "fake"`, `mix = []` — the safe, free default) and `content` (`provider = "fal"` — the real weighted render portfolio).

## `[agent]`

Right swipe runs `implement_cmd` in a fresh worktree; the explicit shape action runs `shape_cmd`. Command templates substitute `{worktree}` `{prompt}` `{branch}` `{spec_path}` `{title}` `{model}`; `pr_cmd` additionally substitutes `{body_file}`.

| Field | Type | Default | Meaning |
|---|---|---|---|
| `implement_cmd` | array of strings | `["opencode", "run", "--dir", "{worktree}", "-m", "{model}", "{prompt}"]` | Command run in the worktree to implement the spec. |
| `shape_cmd` | array of strings | same as `implement_cmd` | Command run for the explicit shape action (sharpen the spec itself). |
| `pr_cmd` | array of strings | `["gh", "pr", "create", "--head", "{branch}", "--title", "{title}", "--body-file", "{body_file}"]` | Command run after the agent commits and DoomScrum pushes the branch. |
| `agent_model` | string | `"openrouter/z-ai/glm-5.2"` | Substituted into `{model}` in the agent commands — swap models with a one-line change. Ignored by commands that don't reference `{model}`. |
| `env_allowlist` | array of strings | `["PATH", "HOME", "USER", "LOGNAME", "SHELL", "TERM", "TMPDIR", "TZ", "LANG", "LC_ALL", "LC_CTYPE", "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_DATA_HOME"]` | Environment variables the **untrusted agent stage** is allowed to inherit (`env_clear` then re-add only these — never the parent env). The agent runs spec content that can come from a foreign repo; its output is committed and pushed, so any key in its env can leak into a PR. Service-secret names (`FAL_API_KEY`, `OPENROUTER_API_KEY`, git tokens) are dropped even if listed here. The default `opencode` agent authenticates from its own credential file (`~/.local/share/opencode/auth.json`, reached through `HOME`), so it needs no key in env. Add a var only if your agent authenticates via an env var, accepting that it's then exposed to untrusted spec execution. |
| `open_pr` | bool | `true` | When `false`, dispatch stops after the agent commits — no push, no PR. |
| `max_concurrent_dispatches` | integer (usize) | `2` | Maximum agent runs allowed at once. Swipes beyond the limit stay as visible `queued` receipts until a slot opens. |
| `undo_window_sec` | integer (u64) | `5` | Seconds a dispatch sits `queued` and cancellable before the agent starts (mis-swipe undo). Cancelling within the window leaves zero git side-effects. `0` disables the window. |

Only trusted stages (`git worktree`, `git push`, `gh pr create`) inherit the operator's full process environment; the agent stage alone runs under `env_allowlist`.
