// Copyright (C) 2026 themadorg
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `chatmail tasks` — run scheduled maintenance jobs on demand.

use chatmail_config::{Args, TasksCommand};
use chatmail_storage::MailboxStore;
use chatmail_tasks::{
    parse_retention_arg, run_all_configured, run_task, MaintenanceConfig, TaskContext, TaskId,
};
use chatmail_types::{ChatmailError, Result};

use super::context::CtlContext;
use super::output::CtlOut;
use crate::supervisor::renew_autocert_from_cli;

pub async fn tasks(args: &Args, cmd: &TasksCommand) -> Result<()> {
    let ctx = CtlContext::from_args(args)?;
    ctx.require_db()?;
    let pool = ctx.open_pool().await?;
    let mailbox = MailboxStore::new(&ctx.state_dir);
    let maintenance = MaintenanceConfig::from_runtime(&pool, &ctx.config).await?;
    let task_ctx = TaskContext {
        pool: &pool,
        mailbox: &mailbox,
        maintenance: &maintenance,
    };
    let out = CtlOut::from_args(args, "tasks");

    match cmd {
        TasksCommand::List => {
            let mut jobs = Vec::new();
            if out.is_json() {
                for id in TaskId::ALL {
                    let enabled = task_enabled(*id, &maintenance, &ctx);
                    jobs.push(serde_json::json!({
                        "name": id.name(),
                        "description": id.description(),
                        "enabled": enabled,
                    }));
                }
                return out.emit(serde_json::json!({
                    "tasks": jobs,
                    "message_retention": maintenance.message_retention.map(|r| format!("{r:?}")),
                    "unused_account_retention": maintenance.unused_account_retention.map(|r| format!("{r:?}")),
                    "periodic_jobs_enabled": maintenance.periodic_jobs_enabled(),
                }));
            }
            out.line("Maintenance jobs (also run on a schedule when `chatmail run` is active):\n");
            for id in TaskId::ALL {
                let enabled = match id {
                    TaskId::PruneOldMessages => maintenance.message_retention.is_some(),
                    TaskId::PruneUnusedAccounts => maintenance.unused_account_retention.is_some(),
                    TaskId::PurgeSeenMessages => {
                        out.line(format!(
                            "  {} — {} (in-process every 15s when __AUTO_PURGE_SEEN__=enabled)",
                            id.name(),
                            id.description()
                        ));
                        continue;
                    }
                    TaskId::PruneUnreadOlder => false,
                    TaskId::RenewCertificate => ctx.config.tls_mode.as_deref() == Some("autocert"),
                };
                let cfg_note = match id {
                    TaskId::RenewCertificate if enabled => {
                        " [enabled — tls_mode autocert; every 24h]"
                    }
                    _ if enabled => " [enabled — DB or maddy.conf]",
                    _ => "",
                };
                out.line(format!(
                    "  {} — {}{}",
                    id.name(),
                    id.description(),
                    cfg_note
                ));
            }
            out.blank();
            if let Some(r) = maintenance.message_retention {
                out.line(format!("message file retention: {:?}", r));
            }
            if let Some(r) = maintenance.unused_account_retention {
                out.line(format!("storage.imapsql unused_account_retention: {:?}", r));
            }
            if !maintenance.periodic_jobs_enabled() {
                out.line(
                    "No periodic retention jobs — enable message retention in admin UI or set retention / unused_account_retention in maddy.conf",
                );
            } else {
                out.line("Periodic retention interval when server is running: 1h (Madmail parity)");
            }
        }
        TasksCommand::Run { task, retention } => {
            let id = TaskId::parse(task).ok_or_else(|| {
                ChatmailError::config(format!("unknown task {task:?}; use `chatmail tasks list`"))
            })?;
            let retention_override = match retention {
                Some(s) => Some(parse_retention_arg(s)?),
                None => None,
            };
            if id == TaskId::RenewCertificate {
                let outcome = renew_autocert_from_cli(&ctx.config, &ctx.state_dir).await?;
                if out.is_json() {
                    return out.emit(serde_json::json!({
                        "task": id.name(),
                        "skipped": outcome.skipped,
                        "renewed": outcome.renewed,
                        "detail": outcome.detail,
                    }));
                }
                if outcome.skipped {
                    out.line(format!(
                        "renew-certificate: skipped ({})",
                        outcome.detail.unwrap_or_default()
                    ));
                } else if outcome.renewed {
                    out.line(format!(
                        "renew-certificate: {}",
                        outcome.detail.unwrap_or_default()
                    ));
                }
                return Ok(());
            }
            let outcome = run_task(&task_ctx, id, retention_override).await?;
            if out.is_json() {
                return out.emit(serde_json::json!({
                    "task": id.name(),
                    "skipped": outcome.skipped,
                    "deleted": outcome.deleted,
                    "detail": outcome.detail,
                }));
            }
            if outcome.skipped {
                out.line(format!(
                    "{}: skipped ({})",
                    id.name(),
                    outcome.detail.unwrap_or_default()
                ));
            } else {
                out.line(format!(
                    "{}: deleted {} item(s){}",
                    id.name(),
                    outcome.deleted,
                    outcome
                        .detail
                        .map(|d| format!(" ({d})"))
                        .unwrap_or_default()
                ));
            }
        }
        TasksCommand::RunAll => {
            let report = run_all_configured(&task_ctx).await?;
            if out.is_json() {
                let outcomes: Vec<_> = report
                    .outcomes
                    .into_iter()
                    .map(|o| {
                        serde_json::json!({
                            "task": o.task.name(),
                            "skipped": o.skipped,
                            "deleted": o.deleted,
                            "detail": o.detail,
                        })
                    })
                    .collect();
                return out.emit(serde_json::json!({ "outcomes": outcomes }));
            }
            if report.outcomes.is_empty() {
                out.line("No jobs enabled in config (set storage.imapsql retention directives).");
            }
            for outcome in report.outcomes {
                if outcome.skipped {
                    out.line(format!(
                        "{}: skipped ({})",
                        outcome.task.name(),
                        outcome.detail.unwrap_or_default()
                    ));
                } else {
                    out.line(format!(
                        "{}: deleted {} item(s)",
                        outcome.task.name(),
                        outcome.deleted
                    ));
                }
            }
        }
    }
    Ok(())
}

fn task_enabled(id: TaskId, maintenance: &MaintenanceConfig, ctx: &CtlContext) -> bool {
    match id {
        TaskId::PruneOldMessages => maintenance.message_retention.is_some(),
        TaskId::PruneUnusedAccounts => maintenance.unused_account_retention.is_some(),
        TaskId::PurgeSeenMessages => false,
        TaskId::PruneUnreadOlder => false,
        TaskId::RenewCertificate => ctx.config.tls_mode.as_deref() == Some("autocert"),
    }
}
