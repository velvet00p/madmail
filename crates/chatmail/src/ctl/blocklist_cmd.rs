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

//! `chatmail blocklist` and shared ban-list output.

use chatmail_config::cli::BlocklistCommand;
use chatmail_config::Args;
use chatmail_db::{blocklist, DbPool, MANUAL_BLOCK_REASON};
use chatmail_types::{ChatmailError, Result};
use serde::Serialize;

use super::context::CtlContext;
use super::output::CtlOut;
use super::util::confirm;

#[derive(Serialize)]
struct BanListEntry {
    username: String,
    reason: String,
    blocked_at: String,
}

pub async fn blocklist(args: &Args, cmd: &BlocklistCommand) -> Result<()> {
    let ctx = CtlContext::from_args(args)?;
    let pool = ctx.open_pool().await?;

    match cmd {
        BlocklistCommand::List => {
            let out = CtlOut::from_args(args, "blocklist list");
            print_ban_list(&pool, &out).await
        }
        BlocklistCommand::Add { username, reason } => {
            let out = CtlOut::from_args(args, "blocklist add");
            let u = normalize_blocklist_username(username)?;
            let r = reason.as_deref().unwrap_or(MANUAL_BLOCK_REASON);
            blocklist::block_user(&pool, &u, r).await?;
            out.done_msg(
                format!("Blocked: {u} ({r})"),
                serde_json::json!({ "username": u, "reason": r }),
                format!("Blocked: {u} ({r})"),
            )
        }
        BlocklistCommand::Remove { username, yes } => {
            let out = CtlOut::from_args(args, "blocklist remove");
            let u = normalize_blocklist_username(username)?;
            if !confirm(&format!("Unblock {u}?"), *yes)? {
                return out.aborted();
            }
            blocklist::unblock_user(&pool, &u).await?;
            out.done_msg(
                format!("Unblocked: {u}"),
                serde_json::json!({ "username": u }),
                format!("Unblocked: {u}"),
            )
        }
    }
}

pub async fn print_ban_list(pool: &DbPool, out: &CtlOut) -> Result<()> {
    let rows = blocklist::list_blocked_users(pool).await?;
    if out.is_json() {
        let entries: Vec<BanListEntry> = rows
            .into_iter()
            .map(|(username, reason, blocked_at)| BanListEntry {
                username,
                reason,
                blocked_at,
            })
            .collect();
        return out.emit(serde_json::json!({ "entries": entries }));
    }
    if rows.is_empty() {
        out.line("(no blocked users)");
        return Ok(());
    }
    for (username, reason, blocked_at) in rows {
        out.line(format!("{username}\t{reason}\t{blocked_at}"));
    }
    Ok(())
}

fn normalize_blocklist_username(raw: &str) -> Result<String> {
    let t = raw.trim();
    if t.is_empty() {
        return Err(ChatmailError::config("username is required"));
    }
    if t.contains('@') {
        chatmail_auth::normalize_username(t)
    } else {
        Ok(t.to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chatmail_config::Cli;
    use clap::Parser;

    #[test]
    fn normalize_email_usernames() {
        assert_eq!(
            normalize_blocklist_username("User@1.2.3.4").unwrap(),
            "user@[1.2.3.4]"
        );
    }

    #[test]
    fn normalize_bare_localpart() {
        assert_eq!(
            normalize_blocklist_username("BadActor").unwrap(),
            "badactor"
        );
    }

    #[tokio::test]
    async fn print_ban_list_empty_and_populated() {
        let pool = chatmail_db::init_memory_db().await.unwrap();
        let out = CtlOut::from_args(&Cli::try_parse_from(["chatmail"]).unwrap().args, "ban-list");
        print_ban_list(&pool, &out).await.unwrap();

        blocklist::block_user(&pool, "a@x.org", "r1").await.unwrap();
        blocklist::block_user(&pool, "b@x.org", "r2").await.unwrap();
        print_ban_list(&pool, &out).await.unwrap();
    }
}
