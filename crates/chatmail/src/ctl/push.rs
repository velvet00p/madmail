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

pub async fn push(args: &Args, cmd: &PushCommand) -> Result<()> {
    let ctx = CtlContext::from_args(args)?;
    let pool = ctx.open_pool().await?;

    match cmd {
        PushCommand::Status => status(&pool).await,
        PushCommand::Auto => set_mode(&pool, PushMode::Auto, "auto").await,
        PushCommand::On => set_mode(&pool, PushMode::On, "on").await,
        PushCommand::Off => set_mode(&pool, PushMode::Off, "off").await,
    }
}

async fn status(pool: &chatmail_db::DbPool) -> Result<()> {
    let mode = push_mode(pool).await?;
    let enabled = push_runtime_enabled(pool).await?;
    println!();
    println!("  Push notifications (XDELTAPUSH)");
    println!("  Mode:       {}", mode.as_str());
    println!(
        "  Runtime:    {}",
        if enabled { "enabled" } else { "disabled" }
    );
    println!("  Successful: {}", push_stats_snapshot());
    println!(
        "  Failures:   {} (auto disables at {})",
        consecutive_failures(),
        AUTO_DISABLE_AFTER_FAILURES
    );
    if mode == PushMode::Auto {
        println!("  Auto mode:  disables push after {AUTO_DISABLE_AFTER_FAILURES} consecutive");
        println!("              notification-proxy failures (>20s or HTTP error)");
    }
    println!("  (run `madmail reload` to refresh IMAP XDELTAPUSH advertisement)");
    println!();
    Ok(())
}

async fn set_mode(pool: &chatmail_db::DbPool, mode: PushMode, label: &str) -> Result<()> {
    set_push_mode(pool, mode).await?;
    println!(
        "✅ Push mode set to {label} ({})",
        if mode.runtime_enabled() {
            "enabled"
        } else {
            "disabled"
        }
    );
    if mode == PushMode::Auto {
        println!("   Auto-disable after {AUTO_DISABLE_AFTER_FAILURES} consecutive proxy failures");
    }
    println!("   Run `madmail reload` to refresh IMAP capabilities");
    Ok(())
}
