use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use specifi::config::Config;
use specifi::dispatch;
use specifi::distill::{compile_storyboard, distill};
use specifi::providers::load_renders;
use specifi::server::{self, AppCtx};

#[derive(Parser)]
#[command(
    name = "specifi",
    version,
    about = "Backlog specs as swipeable brainrot video; swipes dispatch agents"
)]
struct Cli {
    /// Project root containing specifi.toml (defaults to cwd).
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
            println!("specifi listening on http://{host}:{port}");
            axum::serve(listener, app).await?;
        }
        Command::Generate {
            provider,
            limit,
            force,
        } => {
            if let Some(limit) = limit {
                cfg.feed.max_items = limit;
            }
            let provider_name = provider.unwrap_or_else(|| cfg.video.provider.clone());
            let ctx = AppCtx::new(root, cfg);
            let provider = ctx.provider(&provider_name)?;
            let existing = load_renders(&ctx.renders_dir()).unwrap_or_default();
            let mut count = 0usize;
            for prd in ctx.scan()? {
                let already = existing
                    .iter()
                    .any(|r| r.prd_id == prd.id && r.provider == provider.name());
                if already && !force {
                    println!("skip   {} (already rendered)", prd.title);
                    continue;
                }
                let storyboard =
                    compile_storyboard(&prd, &distill(&prd), ctx.cfg.video.max_duration_sec);
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
