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

//! Durable per-user INBOX modseq / change-id.
//!
//! The in-memory `EventBus::inbox_version` is monotonic *within a process* but resets on restart.
//! Persisting it (and seeding at boot) keeps the change-id monotonic across restarts — the
//! invariant a future CONDSTORE/QRESYNC delta-sync requires. Stalwart attaches a durable
//! `change_id` at every storage commit for the same reason. This stays internal (not advertised on
//! the wire) until full CONDSTORE support lands; it is the durable foundation.

use crate::pool::DbPool;
use chatmail_types::Result;

/// Load all persisted `(username, modseq)` pairs for seeding the in-memory versions at boot.
pub async fn load_all_modseq(pool: &DbPool) -> Result<Vec<(String, i64)>> {
    crate::db_fetch_all!(
        pool,
        (String, i64),
        "SELECT username, modseq FROM mailbox_modseq"
    )
}

/// Persist a batch of `(username, modseq)` high-water marks (upsert; called by the state flusher).
pub async fn upsert_modseq(pool: &DbPool, entries: &[(String, i64)]) -> Result<()> {
    for (user, modseq) in entries {
        crate::db_execute!(
            pool,
            "INSERT INTO mailbox_modseq (username, modseq) VALUES (?, ?) \
             ON CONFLICT(username) DO UPDATE SET modseq = excluded.modseq",
            user,
            modseq
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_memory_db;

    /// P10-UT09: modseq persists and reloads (the durable change-id round-trip).
    #[tokio::test]
    async fn p10_ut09_modseq_roundtrip() {
        let pool = init_memory_db().await.unwrap();
        assert!(load_all_modseq(&pool).await.unwrap().is_empty());

        upsert_modseq(&pool, &[("a@test".into(), 5), ("b@test".into(), 9)])
            .await
            .unwrap();
        let mut got = load_all_modseq(&pool).await.unwrap();
        got.sort();
        assert_eq!(got, vec![("a@test".into(), 5), ("b@test".into(), 9)]);

        // Upsert updates in place (monotonic advance).
        upsert_modseq(&pool, &[("a@test".into(), 42)])
            .await
            .unwrap();
        let got = load_all_modseq(&pool).await.unwrap();
        let a = got.iter().find(|(u, _)| u == "a@test").unwrap();
        assert_eq!(a.1, 42);
    }
}
