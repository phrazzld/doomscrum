//! The spec → storyboard → render pipeline. `render_spec` is the single owner of
//! that flow: the feed prefetch, `/api/generate`, and CLI `generate` all call it,
//! so there is exactly one `scriptwriter::storyboard` call site outside tests.

use crate::backlog::PrdSource;
use crate::providers::VideoRender;
use crate::server::AppCtx;

/// Script + storyboard + render + the `rendered` ledger event for one spec.
pub async fn render_spec(
    ctx: &AppCtx,
    provider_name: &str,
    prd: &PrdSource,
) -> anyhow::Result<VideoRender> {
    let vcfg = ctx.cfg.video.with_pipeline(&prd.sha256);
    let provider = ctx.provider_with(provider_name, &vcfg)?;
    let script_key = crate::secrets::get(&["OPENROUTER_API_KEY"]);
    let storyboard = crate::scriptwriter::storyboard(
        &ctx.cfg.script,
        script_key.as_deref(),
        prd,
        provider.clip_duration(vcfg.max_duration_sec),
        &ctx.state_dir().join("scripts"),
        provider_name != "fake",
    )
    .await
    .map_err(|err| anyhow::anyhow!("scriptwriter failed: {err:#}"))?;
    let storyboards_dir = ctx.state_dir().join("storyboards");
    let _ = std::fs::create_dir_all(&storyboards_dir);
    let _ = std::fs::write(
        storyboards_dir.join(format!("{}.json", prd.sha256)),
        serde_json::to_string_pretty(&storyboard).unwrap_or_default(),
    );
    let render = provider.render(&storyboard, &ctx.renders_dir()).await?;
    // Paid spend is recorded in the durable append-only cost ledger the
    // moment provenance exists. Best-effort by design: if this append fails,
    // `ledger::spend_entries` still counts the render from its provenance
    // JSON (union by render id), so no spend goes missing.
    if render.provider == "fal" {
        let _ = crate::render::ledger::append(
            &ctx.ledger_path(),
            &crate::render::ledger::CostEntry::from_render(&render),
        );
    }
    let _ = crate::events::append(
        &ctx.events_path(),
        &prd.id,
        &prd.sha256,
        "rendered",
        Some(format!("{}/{}", render.provider, render.model)),
    );
    Ok(render)
}
