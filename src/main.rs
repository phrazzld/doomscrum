use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use doomscrum::config::Config;
use doomscrum::dispatch;
use doomscrum::gc::{self, GcOptions};
use doomscrum::providers::load_renders;
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
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = match cli.root {
        Some(root) => root.canonicalize()?,
        None => std::env::current_dir()?,
    };
    let mut cfg = Config::load_with_profile(&root, cli.profile.as_deref())?;

    match cli.command {
        Command::Serve { host, port } => {
            let ctx = AppCtx::new(root, cfg);
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
                let spent = server::total_spend(&existing);
                // Each spec may draw a different pipeline from the mix, so
                // the planned spend is the sum of per-spec unit costs.
                let planned = server::planned_fal_spend(&ctx.cfg.video, &targets);
                let cap = ctx.cfg.video.max_total_spend_usd;
                anyhow::ensure!(
                    spent + planned <= cap,
                    "spend cap: ${spent:.2} already spent + ${planned:.2} planned for {} render(s) \
                     exceeds max_total_spend_usd ${cap:.2} — raise it in doomscrum.toml [video]",
                    targets.len()
                );
                let now = chrono::Utc::now();
                let today = server::daily_spend(&existing, now);
                let daily_cap = ctx.cfg.video.max_daily_spend_usd;
                anyhow::ensure!(
                    today + planned <= daily_cap,
                    "daily render budget: ${today:.2} already spent today + ${planned:.2} planned for {} render(s) \
                     exceeds max_daily_spend_usd ${daily_cap:.2}; resets at {}",
                    targets.len(),
                    server::next_daily_reset_at(now)
                );
                println!(
                    "wallet: ${spent:.2} spent lifetime, ${today:.2} spent today, ${planned:.2} planned, ${cap:.2} lifetime cap, ${daily_cap:.2} daily cap"
                );
            }

            let script_key = doomscrum::secrets::get(&["OPENROUTER_API_KEY"]);
            let mut count = 0usize;
            for prd in targets {
                let vcfg = ctx.cfg.video.with_pipeline(&prd.sha256);
                let provider = ctx.provider_with(&provider_name, &vcfg)?;
                let storyboard = doomscrum::scriptwriter::storyboard(
                    &ctx.cfg.script,
                    script_key.as_deref(),
                    &prd,
                    provider.clip_duration(vcfg.max_duration_sec),
                    &ctx.state_dir().join("scripts"),
                    provider_name != "fake",
                )
                .await?;
                let storyboards_dir = ctx.state_dir().join("storyboards");
                std::fs::create_dir_all(&storyboards_dir)?;
                std::fs::write(
                    storyboards_dir.join(format!("{}.json", prd.sha256)),
                    serde_json::to_string_pretty(&storyboard)?,
                )?;
                let render = provider.render(&storyboard, &ctx.renders_dir()).await?;
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
            println!("specs={}", prds.len());
            println!(
                "spend=${:.2} cap=${:.2}",
                server::total_spend(&renders),
                ctx.cfg.video.max_total_spend_usd
            );
            println!(
                "renders={} ready={}",
                renders.len(),
                renders.iter().filter(|r| r.status == "ready").count()
            );
            println!("dispatches={}", dispatches.len());
            for d in dispatches.iter().take(10) {
                println!(
                    "  [{}] {:?} {} -> {} {}",
                    d.status,
                    d.kind,
                    d.prd_title,
                    d.branch,
                    d.pr_url.clone().unwrap_or_default()
                );
            }
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
    }
    Ok(())
}
