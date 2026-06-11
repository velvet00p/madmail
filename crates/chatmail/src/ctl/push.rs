// Copyright (C) 2026 themadorg
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use chatmail_config::cli::PushCommand;
use chatmail_config::Args;
use chatmail_push::{
    consecutive_failures, push_mode, push_runtime_enabled, push_stats_snapshot, set_push_mode,
    PushMode, AUTO_DISABLE_AFTER_FAILURES,
};
use chatmail_types::Result;

use super::context::CtlContext;
use super::output::CtlOut;

pub async fn push(args: &Args, cmd: &PushCommand) -> Result<()> {
    let ctx = CtlContext::from_args(args)?;
    let pool = ctx.open_pool().await?;

    match cmd {
        PushCommand::Status => status(args, &pool).await,
        PushCommand::Auto => set_mode(args, &pool, PushMode::Auto, "auto").await,
        PushCommand::On => set_mode(args, &pool, PushMode::On, "on").await,
        PushCommand::Off => set_mode(args, &pool, PushMode::Off, "off").await,
    }
}

async fn status(args: &Args, pool: &chatmail_db::DbPool) -> Result<()> {
    let out = CtlOut::from_args(args, "push status");
    let mode = push_mode(pool).await?;
    let enabled = push_runtime_enabled(pool).await?;
    let failures = consecutive_failures();

    if out.is_json() {
        return out.emit(serde_json::json!({
            "mode": mode.as_str(),
            "runtime_enabled": enabled,
            "failures": failures,
            "auto_disable_threshold": AUTO_DISABLE_AFTER_FAILURES,
        }));
    }

    out.blank();
    out.line("  Push notifications (XDELTAPUSH)");
    out.line(format!("  Mode:       {}", mode.as_str()));
    out.line(format!(
        "  Runtime:    {}",
        if enabled { "enabled" } else { "disabled" }
    ));
    out.line(format!("  Successful: {}", push_stats_snapshot()));
    out.line(format!(
        "  Failures:   {failures} (auto disables at {AUTO_DISABLE_AFTER_FAILURES})"
    ));
    if mode == PushMode::Auto {
        out.line(format!(
            "  Auto mode:  disables push after {AUTO_DISABLE_AFTER_FAILURES} consecutive"
        ));
        out.line("              notification-proxy failures (>20s or HTTP error)");
    }
    out.line("  (run `madmail reload` to refresh IMAP XDELTAPUSH advertisement)");
    out.blank();
    Ok(())
}

async fn set_mode(
    args: &Args,
    pool: &chatmail_db::DbPool,
    mode: PushMode,
    label: &str,
) -> Result<()> {
    let out = CtlOut::from_args(args, "push");
    set_push_mode(pool, mode).await?;
    let runtime = mode.runtime_enabled();
    let msg = format!("Push mode set to {label}");
    out.done_msg(
        format!(
            "✅ Push mode set to {label} ({})",
            if runtime { "enabled" } else { "disabled" }
        ),
        serde_json::json!({ "mode": label, "runtime_enabled": runtime }),
        msg,
    )
}
