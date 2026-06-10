// Copyright (C) 2026 themadorg
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::time::{SystemTime, UNIX_EPOCH};

use chatmail_db::{db_execute, db_fetch_all, DbPool};
use chatmail_types::{ChatmailError, Result};

/// IMAP METADATA key Delta Chat uses for encrypted device tokens.
pub const DEVICETOKEN_KEY: &str = "/private/devicetoken";

const MAX_TOKEN_LEN: usize = 8192;
/// Tokens older than this are pruned (chatmaild metadata.py: 90 days).
const TOKEN_MAX_AGE_SECS: i64 = 3600 * 24 * 90;

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn validate_token(token: &str) -> Result<()> {
    if token.is_empty() {
        return Err(ChatmailError::protocol("empty device token"));
    }
    if token.len() > MAX_TOKEN_LEN {
        return Err(ChatmailError::protocol("device token too long"));
    }
    Ok(())
}

/// Store or refresh a device token for `username`.
pub async fn upsert_device_token(pool: &DbPool, username: &str, token: &str) -> Result<()> {
    validate_token(token)?;
    let now = now_secs();
    db_execute!(
        pool,
        "INSERT INTO push_tokens (username, device_token, updated_at) VALUES (?, ?, ?)
         ON CONFLICT(username, device_token) DO UPDATE SET updated_at = excluded.updated_at",
        username,
        token,
        now
    )?;
    prune_stale_tokens(pool, username).await?;
    Ok(())
}

/// List non-expired device tokens for `username` (space-join for GETMETADATA).
pub async fn list_device_tokens(pool: &DbPool, username: &str) -> Result<Vec<String>> {
    prune_stale_tokens(pool, username).await?;
    let rows: Vec<(String,)> = db_fetch_all!(
        pool,
        (String,),
        "SELECT device_token FROM push_tokens WHERE username = ? ORDER BY updated_at",
        username
    )?;
    Ok(rows.into_iter().map(|(token,)| token).collect())
}

/// Remove a token after the notification proxy returns HTTP 410 Gone.
pub async fn remove_device_token(pool: &DbPool, username: &str, token: &str) -> Result<()> {
    db_execute!(
        pool,
        "DELETE FROM push_tokens WHERE username = ? AND device_token = ?",
        username,
        token
    )?;
    Ok(())
}

async fn prune_stale_tokens(pool: &DbPool, username: &str) -> Result<()> {
    let cutoff = now_secs() - TOKEN_MAX_AGE_SECS;
    db_execute!(
        pool,
        "DELETE FROM push_tokens WHERE username = ? AND updated_at < ?",
        username,
        cutoff
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chatmail_db::init_memory_db;

    #[tokio::test]
    async fn upsert_and_list_tokens() {
        let pool = init_memory_db().await.unwrap();
        upsert_device_token(&pool, "u@test", "openpgp:aaa")
            .await
            .unwrap();
        upsert_device_token(&pool, "u@test", "openpgp:bbb")
            .await
            .unwrap();
        let tokens = list_device_tokens(&pool, "u@test").await.unwrap();
        assert_eq!(tokens.len(), 2);
        assert!(tokens.contains(&"openpgp:aaa".to_string()));
        assert!(tokens.contains(&"openpgp:bbb".to_string()));
    }

    #[tokio::test]
    async fn remove_token() {
        let pool = init_memory_db().await.unwrap();
        upsert_device_token(&pool, "u@test", "tok1").await.unwrap();
        remove_device_token(&pool, "u@test", "tok1").await.unwrap();
        assert!(list_device_tokens(&pool, "u@test").await.unwrap().is_empty());
    }
}