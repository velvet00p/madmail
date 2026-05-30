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

use std::sync::Arc;
use std::time::Duration;

use chatmail_db::{db_execute, DbPool};
use chatmail_types::Result;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::debug;

use crate::events::EventBus;
use crate::tracker::FederationTracker;

pub struct FlusherHandle {
    shutdown_tx: watch::Sender<bool>,
    task: JoinHandle<()>,
}

impl FlusherHandle {
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);
        let _ = self.task.await;
    }
}

pub fn start_flusher(
    pool: DbPool,
    tracker: Arc<FederationTracker>,
    events: Arc<EventBus>,
) -> FlusherHandle {
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    let task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = flush_federation_stats(&pool, &tracker).await {
                        tracing::warn!(error = %e, "federation stats flush failed");
                    } else {
                        debug!("federation stats flushed to database");
                    }
                    if let Err(e) = flush_modseq(&pool, &events).await {
                        tracing::warn!(error = %e, "mailbox modseq flush failed");
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        let _ = flush_federation_stats(&pool, &tracker).await;
                        let _ = flush_modseq(&pool, &events).await;
                        break;
                    }
                }
            }
        }
    });

    FlusherHandle { shutdown_tx, task }
}

/// Persist the in-memory INBOX versions as durable modseq high-water marks.
pub async fn flush_modseq(pool: &DbPool, events: &EventBus) -> Result<()> {
    let snapshot: Vec<(String, i64)> = events
        .inbox_version_snapshot()
        .into_iter()
        .map(|(user, version)| (user, version.min(i64::MAX as u64) as i64))
        .collect();
    if snapshot.is_empty() {
        return Ok(());
    }
    chatmail_db::upsert_modseq(pool, &snapshot).await
}

pub async fn flush_federation_stats(pool: &DbPool, tracker: &FederationTracker) -> Result<()> {
    let cols = chatmail_db::schema::federation_stats_columns(pool).await?;
    let sql = format!(
        "INSERT INTO federation_server_stats (
                domain, queued_messages, failed_http, {fh}, failed_smtp,
                success_http, {sh}, success_smtp, inbound_deliveries,
                successful_deliveries, total_latency_ms, last_active
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(domain) DO UPDATE SET
                queued_messages = excluded.queued_messages,
                failed_http = excluded.failed_http,
                {fh} = excluded.{fh},
                failed_smtp = excluded.failed_smtp,
                success_http = excluded.success_http,
                {sh} = excluded.{sh},
                success_smtp = excluded.success_smtp,
                inbound_deliveries = excluded.inbound_deliveries,
                successful_deliveries = excluded.successful_deliveries,
                total_latency_ms = excluded.total_latency_ms,
                last_active = excluded.last_active",
        fh = cols.failed_https,
        sh = cols.success_https
    );
    for row in tracker.snapshot() {
        db_execute!(
            pool,
            &sql,
            row.domain,
            row.queued_messages,
            row.failed_http,
            row.failed_https,
            row.failed_smtp,
            row.success_http,
            row.success_https,
            row.success_smtp,
            row.inbound_deliveries,
            row.successful_deliveries,
            row.total_latency_ms,
            row.last_active
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventBus;
    use crate::tracker::FederationTracker;
    use chatmail_db::{db_fetch_one, init_memory_db};

    #[tokio::test]
    async fn p2_ut06_test_flusher_writes_to_db() {
        let pool = init_memory_db().await.unwrap();
        let tracker = Arc::new(FederationTracker::new());
        tracker.record_success("peer.example.org", 100, "HTTPS");
        tracker.record_failure("peer.example.org", "HTTP");
        tracker.increment_queue("peer.example.org");

        flush_federation_stats(&pool, &tracker).await.unwrap();

        let row: (i64, i64, i64) = db_fetch_one!(
            pool,
            (i64, i64, i64),
            "SELECT success_https, failed_http, queued_messages FROM federation_server_stats WHERE domain = ?",
            "peer.example.org"
        )
        .unwrap();

        assert_eq!(row, (1, 1, 1));
    }

    /// P10-UT09: INBOX versions flush to the durable modseq table and reload monotonically.
    #[tokio::test]
    async fn p10_ut09_modseq_flush_and_seed() {
        let pool = init_memory_db().await.unwrap();
        let bus = EventBus::new();
        bus.bump_inbox_version("u@test");
        bus.bump_inbox_version("u@test");
        let before = bus.inbox_version("u@test");
        assert_eq!(before, 2);

        flush_modseq(&pool, &bus).await.unwrap();

        // A fresh process (new EventBus) seeds from the persisted high-water mark.
        let restarted = EventBus::new();
        for (user, modseq) in chatmail_db::load_all_modseq(&pool).await.unwrap() {
            restarted.seed_inbox_version(&user, modseq as u64);
        }
        assert_eq!(
            restarted.inbox_version("u@test"),
            before,
            "modseq is monotonic across restart"
        );
        // Subsequent bumps continue above the seed (never reuse a change-id).
        restarted.bump_inbox_version("u@test");
        assert_eq!(restarted.inbox_version("u@test"), before + 1);
    }
}
