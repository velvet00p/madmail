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

//! In-process maintenance scheduler (started with `chatmail run`).

use std::sync::Arc;

use chatmail_config::AppConfig;
use chatmail_db::DbPool;
use chatmail_storage::MailboxStore;
use std::path::Path;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use crate::cert_renew::CertificateRenewer;
use crate::config::{
    MaintenanceConfig, AUTO_PURGE_SEEN_INTERVAL, CERT_RENEWAL_INTERVAL, PERIODIC_INTERVAL,
};
use crate::jobs::{
    run_all_configured, run_auto_purge_seen_if_enabled, run_certificate_renewal, TaskContext,
};

pub struct MaintenanceHandle {
    cancel: CancellationToken,
    join: JoinHandle<()>,
}

impl MaintenanceHandle {
    pub fn shutdown(self) {
        self.cancel.cancel();
    }

    pub async fn wait(self) {
        let _ = self.join.await;
    }
}

/// Background loops: hourly retention jobs, 15s auto-purge seen, daily autocert renewal.
pub fn spawn_maintenance_scheduler(
    pool: DbPool,
    state_dir: &Path,
    file_config: &AppConfig,
    cert_renewer: Option<Arc<dyn CertificateRenewer>>,
) -> MaintenanceHandle {
    let cancel = CancellationToken::new();
    let cancel_child = cancel.clone();
    let state_dir = state_dir.to_path_buf();
    let file_config = file_config.clone();

    let join = tokio::spawn(async move {
        let mailbox = MailboxStore::new(&state_dir);
        let maintenance = match MaintenanceConfig::from_runtime(&pool, &file_config).await {
            Ok(m) => m,
            Err(e) => {
                error!("maintenance: invalid config: {e}");
                return;
            }
        };

        let mut periodic_tick = tokio::time::interval(PERIODIC_INTERVAL);
        periodic_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut seen_tick = tokio::time::interval(AUTO_PURGE_SEEN_INTERVAL);
        seen_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut cert_tick = cert_renewer.as_ref().map(|_| {
            let mut tick = tokio::time::interval(CERT_RENEWAL_INTERVAL);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            tick
        });

        // First run after one interval (Madmail starts ticker then waits).
        periodic_tick.tick().await;
        seen_tick.tick().await;
        if let Some(tick) = cert_tick.as_mut() {
            tick.tick().await;
        }

        loop {
            tokio::select! {
                _ = cancel_child.cancelled() => break,
                _ = periodic_tick.tick() => {
                    let ctx = TaskContext {
                        pool: &pool,
                        mailbox: &mailbox,
                        maintenance: &maintenance,
                    };
                    match run_all_configured(&ctx).await {
                        Ok(report) => {
                            for o in report.outcomes {
                                if o.skipped {
                                    debug!(task = o.task.name(), "maintenance: skipped");
                                } else {
                                    debug!(
                                        task = o.task.name(),
                                        deleted = o.deleted,
                                        detail = ?o.detail,
                                        "maintenance: completed"
                                    );
                                }
                            }
                        }
                        Err(e) => error!("maintenance periodic run failed: {e}"),
                    }
                }
                _ = seen_tick.tick() => {
                    match run_auto_purge_seen_if_enabled(&pool, &mailbox).await {
                        Ok(Some(n)) if n > 0 => {
                            debug!(deleted = n, "auto-purge seen messages");
                        }
                        Ok(_) => {}
                        Err(e) => error!("auto-purge seen failed: {e}"),
                    }
                }
                _ = async {
                    if let Some(tick) = cert_tick.as_mut() {
                        tick.tick().await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                }, if cert_renewer.is_some() => {
                    if let Some(renewer) = cert_renewer.as_ref() {
                        match run_certificate_renewal(renewer.as_ref()).await {
                            Ok(outcome) if outcome.skipped => {
                                debug!(
                                    detail = ?outcome.detail,
                                    "certificate renewal: skipped"
                                );
                            }
                            Ok(outcome) if outcome.renewed => {
                                info!(
                                    detail = ?outcome.detail,
                                    "certificate renewal: completed"
                                );
                            }
                            Ok(_) => {}
                            Err(e) => error!("certificate renewal failed: {e}"),
                        }
                    }
                }
            }
        }
    });

    MaintenanceHandle { cancel, join }
}
