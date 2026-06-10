use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use doomscrum::config::Config;
use doomscrum::dispatch;
use doomscrum::distill::{compile_storyboard, distill};
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
    /// Print a summary of specs, renders, and dispatches.
    Report,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = match cli.root {
        Some(root) => root.canonicalize()?,
        None => std::env::current_dir()?,
    };
    let mut cfg = Config::load(&root)?;

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
                cfg.video.fal_model = model;
            }
            let provider_name = provider.unwrap_or_else(|| cfg.video.provider.clone());
            let ctx = AppCtx::new(root, cfg);
            let provider = ctx.provider(&provider_name)?;
            let existing = load_renders(&ctx.renders_dir()).unwrap_or_default();

            let spec_filter = spec.map(|s| s.to_lowercase());
            let targets: Vec<_> = ctx
                .scan()?
                .into_iter()
                .filter(|prd| {
                    spec_filter.as_ref().is_none_or(|f| {
                        prd.title.to_lowercase().contains(f) || prd.id.contains(f)
                    })
                })
                .filter(|prd| {
                    let already = existing
                        .iter()
                        .any(|r| r.prd_id == prd.id && r.provider == provider.name());
                    if already && !force {
                        println!("skip   {} (already rendered)", prd.title);
                    }
                    force || !already
                })
                .collect();

            if matches!(provider, doomscrum::providers::Provider::Fal(_)) {
                let spent = server::total_spend(&existing);
                let per_render = doomscrum::providers::fal::unit_cost(&ctx.cfg.video);
                let planned = per_render * targets.len() as f64;
                let cap = ctx.cfg.video.max_total_spend_usd;
                anyhow::ensure!(
                    spent + planned <= cap,
                    "spend cap: ${spent:.2} already spent + ${planned:.2} planned for {} render(s) \
                     exceeds max_total_spend_usd ${cap:.2} — raise it in doomscrum.toml [video]",
                    targets.len()
                );
                println!("wallet: ${spent:.2} spent, ${planned:.2} planned, ${cap:.2} cap");
            }

            let mut count = 0usize;
            for prd in targets {
                let storyboard = compile_storyboard(
                    &prd,
                    &distill(&prd),
                    provider.clip_duration(ctx.cfg.video.max_duration_sec),
                );
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
        Command::Report => {
            let ctx = AppCtx::new(root, cfg);
            let prds = ctx.scan()?;
            let renders = load_renders(&ctx.renders_dir()).unwrap_or_default();
            let dispatches =
                dispatch::load_receipts(&ctx.dispatcher.dispatches_dir).unwrap_or_default();
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
    }
    Ok(())
}
