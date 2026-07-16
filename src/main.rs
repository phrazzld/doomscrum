use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod canary;

use doomscrum::config::Config;
use doomscrum::dispatch;
use doomscrum::gc::{self, GcOptions};
use doomscrum::preflight::{self, Facts, Status};
use doomscrum::providers::load_renders;
use doomscrum::providers::samples;
use doomscrum::render::{pipeline, wallet};
use doomscrum::secrets;
use doomscrum::server::{self, AppCtx};

#[derive(Parser)]
#[command(
    name = "doomscrum",
    version,
    about = "Backlog specs as swipeable brainrot video; swipes dispatch agents"
)]
struct Cli {
    /// Project root containing doomscrum.toml (defaults to cwd).
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    /// Render profile from [profiles.<name>] (overrides the `profile` key,
    /// e.g. dev for free local iteration, content for real renders).
    #[arg(long, global = true)]
    profile: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the swipe-feed server.
    Serve {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 4173)]
        port: u16,
    },
    /// Generate videos for the top backlog specs.
    Generate {
        /// Video provider: fake (offline fixture) or fal (real, costs money,
        /// sends spec-derived prompts to the provider).
        #[arg(long)]
        provider: Option<String>,
        /// Cap the number of specs (overrides feed.max_items).
        #[arg(long)]
        limit: Option<usize>,
        /// Re-render even when a render already exists.
        #[arg(long, default_value_t = false)]
        force: bool,
        /// Override the fal model for this run
        /// (e.g. fal-ai/sora-2/text-to-video).
        #[arg(long)]
        model: Option<String>,
        /// Only render specs whose title or id contains this substring
        /// (case-insensitive).
        #[arg(long)]
        spec: Option<String>,
    },
    /// Write (or replay from cache) the LLM script for one spec and print
    /// it — preview the words before paying for video.
    Script {
        /// Substring of the spec title or id (case-insensitive).
        spec: String,
        /// Ignore the cache and pay for a fresh take.
        #[arg(long, default_value_t = false)]
        reroll: bool,
    },
    /// Print a summary of specs, renders, and dispatches.
    Report,
    /// Check the setup is ready to dispatch: agent auth, gh auth, git remote,
    /// keys. Exits non-zero if any check fails.
    Doctor,
    /// Scaffold a starter doomscrum.toml (if absent) and print the setup
    /// checklist + current readiness.
    Init {
        /// Repository the new config should point at (defaults to ".").
        #[arg(long)]
        repo: Option<String>,
    },
    /// Print the runtime data-egress disclosure: exactly what spec-derived
    /// text is sent to OpenRouter (scriptwriter) and fal.ai (render prompt),
    /// with the source code path for each. (backlog 022, security lane.)
    Egress,
    /// Garbage-collect generated renders, dispatch worktrees, and event logs.
    Gc {
        /// Print actions without deleting or rotating anything.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Delete terminal dispatch worktrees at least this many days old.
        #[arg(long, default_value_t = 7)]
        worktree_max_age_days: u64,
        /// Rotate events.ndjson once it exceeds this many bytes.
        #[arg(long, default_value_t = 5_000_000)]
        events_max_bytes: u64,
        /// Keep this many recent bytes, rounded to complete event lines.
        #[arg(long, default_value_t = 200_000)]
        events_keep_bytes: u64,
    },
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    // Comprehensive coverage, before anything else can panic or log: the
    // panic hook reports `doomscrum.panic` on any panic anywhere in the
    // process, and the tracing subscriber makes `tracing::error!(...)` —
    // wherever it fires, including inside the `doomscrum` lib crate's
    // server/dispatch code — auto-forwarded error capture with zero
    // per-site wiring. Both no-op without CANARY_ENDPOINT/CANARY_API_KEY.
    canary::install_panic_hook();
    use tracing_subscriber::filter::LevelFilter;
    use tracing_subscriber::layer::{Layer as _, SubscriberExt};
    use tracing_subscriber::util::SubscriberInitExt;
    // RUST_LOG controls the console (fmt) layer only; CanaryLayer keeps its own
    // ERROR filter so error reporting can never be silenced by a log-level env.
    let fmt_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(fmt_filter))
        .with(canary::CanaryLayer.with_filter(LevelFilter::ERROR))
        .try_init();

    let result = run().await;
    if let Err(err) = &result {
        // Top-level failure path: report to Canary, flush before exit so a
        // one-shot CLI invocation's proof event beats process teardown, and
        // still surface + exit non-zero exactly as before.
        canary::report_error("doomscrum.run.failed", &format!("{err:#}"));
    }
    canary::flush();
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err:?}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    // Fire as early as possible so any invocation (including one that later
    // errors) proves the process ran. One-shot per invocation — no
    // background loop; doomscrum is a CLI/local-run tool, not a service.
    canary::check_in();
    let root = match cli.root {
        Some(root) => root.canonicalize()?,
        None => std::env::current_dir()?,
    };
    let mut cfg = Config::load_with_profile(&root, cli.profile.as_deref())?;

    match cli.command {
        Command::Serve { host, port } => {
            // `serve` runs until killed — a standing service, not a
            // one-shot CLI invocation. The `check_in()` above only proves
            // the process started; without a continuous loop it reads as
            // falsely overdue past the 120s TTL while still perfectly
            // healthy. Call at the top of every long-running bootstrap.
            canary::start_health_loop();
            let ctx = AppCtx::new(root.clone(), cfg);
            let _ = samples::bootstrap(
                &ctx.repo(),
                &ctx.cfg.repo.source,
                &ctx.cfg.repo.backlog_dir,
                ctx.cfg.feed.max_items,
                &ctx.renders_dir(),
            );
            // Reconcile runtime truth from disk before serving: a crash mid-
            // dispatch must not leave a frozen agent_running status or a
            // GC-protected orphan worktree.
            match ctx.reconcile_on_boot() {
                Ok(reconciled) => {
                    for receipt in &reconciled {
                        println!(
                            "reconciled stranded dispatch {} ({}) -> failed",
                            receipt.id, receipt.prd_title
                        );
                    }
                }
                // Was println-swallowed — now an `error!` so the CanaryLayer
                // forwards it instead of the boot-reconcile failure only
                // ever reaching whoever happens to be tailing stdout.
                Err(err) => tracing::error!("boot reconcile failed: {err:#}"),
            }
            let app = server::router(ctx);
            let listener = tokio::net::TcpListener::bind((host.as_str(), port)).await?;
            println!("doomscrum listening on http://{host}:{port}");
            axum::serve(listener, app).await?;
        }
        Command::Generate {
            provider,
            limit,
            force,
            model,
            spec,
        } => {
            if let Some(limit) = limit {
                cfg.feed.max_items = limit;
            }
            if let Some(model) = model {
                // An explicit --model overrides the render mix too: the
                // operator asked for exactly this pipeline.
                cfg.video.fal_model = model;
                cfg.video.mix.clear();
            }
            let provider_name = provider.unwrap_or_else(|| cfg.video.provider.clone());
            let ctx = AppCtx::new(root, cfg);
            let _ = samples::bootstrap(
                &ctx.repo(),
                &ctx.cfg.repo.source,
                &ctx.cfg.repo.backlog_dir,
                ctx.cfg.feed.max_items,
                &ctx.renders_dir(),
            );
            let render_provider = server::render_provider_id(&provider_name)?;
            let existing = load_renders(&ctx.renders_dir()).unwrap_or_default();

            let spec_filter = spec.map(|s| s.to_lowercase());
            let targets: Vec<_> = ctx
                .scan()?
                .into_iter()
                .filter(|prd| {
                    spec_filter
                        .as_ref()
                        .is_none_or(|f| prd.title.to_lowercase().contains(f) || prd.id.contains(f))
                })
                .filter(|prd| {
                    let already = existing
                        .iter()
                        .any(|r| r.prd_id == prd.id && r.provider == render_provider);
                    if already && !force {
                        println!("skip   {} (already rendered)", prd.title);
                    }
                    force || !already
                })
                .collect();

            if provider_name == "fal" {
                // Spend truth from the durable cost ledger (unioned with any
                // provenance it does not know about) — a wiped renders dir
                // must not reopen the wallet.
                let entries = ctx.spend_entries(&existing);
                let spent = server::total_spend(&entries);
                // Each spec may draw a different pipeline from the mix, so
                // the planned spend is the sum of per-spec unit costs.
                let planned = server::planned_fal_spend(&ctx.cfg.video, &targets);
                let cap = ctx.cfg.video.max_total_spend_usd;
                let now = chrono::Utc::now();
                let today = server::daily_spend(&entries, now);
                let daily_cap = ctx.cfg.video.max_daily_spend_usd;
                // Same gate as the server, via the one arithmetic site. No
                // pending term: the CLI has no concurrent in-flight reservations.
                match wallet::cap_breach(spent, today, planned, cap, daily_cap) {
                    wallet::CapBreach::Lifetime => anyhow::bail!(
                        "spend cap: ${spent:.2} already spent + ${planned:.2} planned for {} render(s) \
                         exceeds max_total_spend_usd ${cap:.2} — raise it in doomscrum.toml [video]",
                        targets.len()
                    ),
                    wallet::CapBreach::Daily => anyhow::bail!(
                        "daily render budget: ${today:.2} already spent today + ${planned:.2} planned for {} render(s) \
                         exceeds max_daily_spend_usd ${daily_cap:.2}; resets at {}",
                        targets.len(),
                        server::next_daily_reset_at(now)
                    ),
                    wallet::CapBreach::None => {}
                }
                println!(
                    "wallet: ${spent:.2} spent lifetime, ${today:.2} spent today, ${planned:.2} planned, ${cap:.2} lifetime cap, ${daily_cap:.2} daily cap"
                );
            }

            let mut count = 0usize;
            for prd in targets {
                let render = pipeline::render_spec(&ctx, &provider_name, &prd).await?;
                println!(
                    "render {} provider={} model={} audio={} latency_ms={}",
                    prd.title,
                    render.provider,
                    render.model,
                    render.native_audio,
                    render.latency_ms
                );
                count += 1;
            }
            println!("done: {count} new render(s)");
        }
        Command::Script { spec, reroll } => {
            let ctx = AppCtx::new(root, cfg);
            let needle = spec.to_lowercase();
            let prd = ctx
                .scan()?
                .into_iter()
                .find(|p| p.title.to_lowercase().contains(&needle) || p.id.contains(&needle))
                .ok_or_else(|| anyhow::anyhow!("no spec matching {spec:?} in the feed"))?;
            let cache_dir = ctx.state_dir().join("scripts");
            if reroll {
                // Drop any cached takes for this spec so the next write pays
                // for a fresh one.
                if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                    for e in entries.flatten() {
                        if e.file_name().to_string_lossy().starts_with(&prd.sha256) {
                            let _ = std::fs::remove_file(e.path());
                        }
                    }
                }
            }
            let key = doomscrum::secrets::get(&["OPENROUTER_API_KEY"]);
            let duration = ctx.cfg.video.with_pipeline(&prd.sha256).max_duration_sec;
            let script = doomscrum::scriptwriter::write_script(
                &ctx.cfg.script,
                key.as_deref(),
                &prd,
                duration,
                &cache_dir,
            )
            .await?;
            println!("spec: {} ({}s clip)", prd.title, duration);
            println!("model: {}", script.model);
            println!(
                "script ({} words):\n  {}",
                script.script.split_whitespace().count(),
                script.script
            );
            println!("scene:\n  {}", script.scene);
        }
        Command::Report => {
            let ctx = AppCtx::new(root, cfg);
            let prds = ctx.scan()?;
            let renders = load_renders(&ctx.renders_dir()).unwrap_or_default();
            let dispatches =
                dispatch::load_receipts(&ctx.dispatcher().dispatches_dir).unwrap_or_default();
            let entries = ctx.spend_entries(&renders);
            let events = doomscrum::events::read_all(&ctx.events_path()).unwrap_or_default();
            print!(
                "{}",
                doomscrum::report::render(&doomscrum::report::ReportInputs {
                    specs: &prds,
                    entries: &entries,
                    renders: &renders,
                    receipts: &dispatches,
                    events: &events,
                    video: &ctx.cfg.video,
                    max_concurrent_dispatches: ctx.cfg.agent.max_concurrent_dispatches,
                    now: chrono::Utc::now(),
                })
            );
        }
        Command::Egress => {
            print!("{}", doomscrum::egress::render_cli());
        }
        Command::Gc {
            dry_run,
            worktree_max_age_days,
            events_max_bytes,
            events_keep_bytes,
        } => {
            let ctx = AppCtx::new(root, cfg);
            let report = gc::collect(
                &ctx,
                GcOptions {
                    dry_run,
                    worktree_max_age_days,
                    events_max_bytes,
                    events_keep_bytes,
                },
            )?;
            print!("{}", report.render_cli());
        }
        Command::Doctor => {
            let checks = preflight::evaluate(&gather_facts(&root, &cfg));
            print!("{}", preflight::format_report(&checks));
            if preflight::worst(&checks) == Status::Fail {
                std::process::exit(1);
            }
        }
        Command::Init { repo } => {
            let toml_path = root.join("doomscrum.toml");
            if toml_path.exists() {
                println!("doomscrum.toml already exists — leaving it untouched.");
            } else {
                std::fs::write(
                    &toml_path,
                    preflight::starter_config_toml(repo.as_deref().unwrap_or(".")),
                )?;
                println!("wrote {}", toml_path.display());
            }
            println!("\nsetup checklist:");
            println!("  1. opencode auth login   # store your OpenRouter credential for dispatch");
            println!("  2. gh auth login         # so dispatches can open PRs");
            println!("  3. (optional) OPENROUTER_API_KEY (env or ~/.secrets) for LLM scripts");
            println!("  4. (optional) FAL_API_KEY (env or ~/.secrets) for real AI video");
            let cfg = Config::load_with_profile(&root, None)?;
            let repo_path = root.join(&cfg.repo.path);
            let renders_dir = root.join(&cfg.repo.state_dir).join("renders");
            let _ = samples::bootstrap(
                &repo_path,
                &cfg.repo.source,
                &cfg.repo.backlog_dir,
                cfg.feed.max_items,
                &renders_dir,
            );
            println!("\ncurrent state:\n");
            print!(
                "{}",
                preflight::format_report(&preflight::evaluate(&gather_facts(&root, &cfg)))
            );
        }
    }
    Ok(())
}

/// Gather real environment facts for the [`preflight`] evaluator: config-derived
/// flags plus live env / `gh` / `git` / opencode-auth lookups.
fn gather_facts(root: &std::path::Path, cfg: &Config) -> Facts {
    let repo = root.join(&cfg.repo.path);
    Facts {
        agent_is_opencode: cfg
            .agent
            .implement_cmd
            .first()
            .map(|c| c == "opencode")
            .unwrap_or(false),
        script_llm_mode: cfg.script.mode == "llm",
        openrouter_key: secrets::get(&["OPENROUTER_API_KEY"]).is_some(),
        opencode_stored_auth: opencode_has_stored_openrouter(),
        gh_authed: command_ok_in(None, "gh", &["auth", "status"]),
        repo_is_git: command_ok_in(Some(&repo), "git", &["rev-parse", "--is-inside-work-tree"]),
        repo_has_remote: git_has_remote(&repo),
        provider_is_fal: cfg.video.provider == "fal",
        fal_key: secrets::get(&["FAL_API_KEY", "FAL_KEY"]).is_some(),
        stills_pipeline_required: cfg.video.uses_stills_pipeline(),
        ffmpeg_on_path: command_ok_in(None, "ffmpeg", &["-version"]),
        ffprobe_on_path: command_ok_in(None, "ffprobe", &["-version"]),
        repo_source: cfg.repo.source.clone(),
        repo_has_github_remote: git_origin_is_github(&repo),
    }
}

/// True if `opencode` has a *stored* OpenRouter credential. The dispatched agent
/// runs with a scrubbed env, so an env-only key won't reach it — only the
/// credential file (`$XDG_DATA_HOME/opencode/auth.json`, else
/// `~/.local/share/opencode/auth.json`) survives.
fn opencode_has_stored_openrouter() -> bool {
    let base = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| std::path::Path::new(&h).join(".local/share"))
        });
    base.map(|b| b.join("opencode/auth.json"))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.contains("openrouter"))
        .unwrap_or(false)
}

/// Run `bin args` (optionally in `dir`), discarding output; true on exit 0.
fn command_ok_in(dir: Option<&std::path::Path>, bin: &str, args: &[&str]) -> bool {
    let mut cmd = std::process::Command::new(bin);
    cmd.args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(dir) = dir {
        cmd.current_dir(dir);
    }
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

/// True if the repo has an **`origin`** remote specifically — not just any
/// remote. Dispatch pushes to and opens PRs against `origin` (see
/// `dispatch.rs`), so a repo with only an `upstream` remote can't open a PR;
/// doctor must check the same thing dispatch will use, or it green-lights a
/// dispatch that silently stays local.
fn git_has_remote(dir: &std::path::Path) -> bool {
    command_ok_in(Some(dir), "git", &["remote", "get-url", "origin"])
}

/// True if the synced repo's `origin` remote points at GitHub. The
/// `github-issues` source needs this for `gh issue list` to resolve the repo.
fn git_origin_is_github(dir: &std::path::Path) -> bool {
    match std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(dir)
        .output()
    {
        Ok(out) if out.status.success() => {
            let url = String::from_utf8_lossy(&out.stdout);
            url.contains("github.com") || url.contains("github.com:")
        }
        _ => false,
    }
}
