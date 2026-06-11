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

//! `chatmail endpoint-cache` — Madmail `ctl/dnscache.go`.

use chatmail_config::cli::EndpointCacheCommand;
use chatmail_config::Args;
use chatmail_db::{
    get_endpoint_override, list_endpoint_overrides, remove_endpoint_override, set_endpoint_override,
};
use chatmail_types::{ChatmailError, Result};

use super::context::CtlContext;
use super::output::CtlOut;

pub async fn endpoint_cache(args: &Args, cmd: &EndpointCacheCommand) -> Result<()> {
    let ctx = CtlContext::from_args(args)?;
    let pool = ctx.open_pool().await?;
    let out = CtlOut::from_args(args, "endpoint-cache");

    match cmd {
        EndpointCacheCommand::List => {
            let rows = list_endpoint_overrides(&pool).await?;
            if out.is_json() {
                let entries: Vec<_> = rows
                    .into_iter()
                    .map(|o| {
                        serde_json::json!({
                            "lookup_key": o.lookup_key,
                            "target_host": o.target_host,
                            "comment": o.comment,
                            "created_at": o.created_at,
                            "updated_at": o.updated_at,
                        })
                    })
                    .collect();
                return out.emit(serde_json::json!({ "entries": entries }));
            }
            if rows.is_empty() {
                eprintln!("No endpoint override entries.");
                return Ok(());
            }
            out.line("LOOKUP KEY\tTARGET HOST\tCOMMENT\tCREATED AT\tUPDATED AT");
            for o in rows {
                out.line(format!(
                    "{}\t{}\t{}\t{}\t{}",
                    o.lookup_key,
                    o.target_host,
                    o.comment.as_deref().unwrap_or(""),
                    o.created_at.as_deref().unwrap_or(""),
                    o.updated_at.as_deref().unwrap_or(""),
                ));
            }
        }
        EndpointCacheCommand::Set {
            lookup_key,
            target_host,
            comment,
        } => {
            let comment = comment.as_deref().unwrap_or("");
            set_endpoint_override(&pool, lookup_key, target_host, comment).await?;
            out.done_msg(
                format!("Successfully set endpoint override: {lookup_key} → {target_host}"),
                serde_json::json!({
                    "lookup_key": lookup_key,
                    "target_host": target_host,
                    "comment": comment,
                }),
                format!("Set endpoint override: {lookup_key}"),
            )?;
        }
        EndpointCacheCommand::Get { lookup_key } => {
            let row = get_endpoint_override(&pool, lookup_key)
                .await?
                .ok_or_else(|| {
                    ChatmailError::config(format!("no endpoint override found for {lookup_key:?}"))
                })?;
            if out.is_json() {
                out.emit(serde_json::json!({
                    "lookup_key": row.lookup_key,
                    "target_host": row.target_host,
                    "comment": row.comment,
                    "created_at": row.created_at,
                    "updated_at": row.updated_at,
                }))?;
            } else {
                out.line(format!("Lookup Key:\t{}", row.lookup_key));
                out.line(format!("Target Host:\t{}", row.target_host));
                out.line(format!(
                    "Comment:\t{}",
                    row.comment.as_deref().unwrap_or("")
                ));
                out.line(format!(
                    "Created At:\t{}",
                    row.created_at.as_deref().unwrap_or("")
                ));
                out.line(format!(
                    "Updated At:\t{}",
                    row.updated_at.as_deref().unwrap_or("")
                ));
            }
        }
        EndpointCacheCommand::Remove { lookup_key } => {
            if !remove_endpoint_override(&pool, lookup_key).await? {
                return Err(ChatmailError::config(format!(
                    "no endpoint override found for {lookup_key:?}"
                )));
            }
            out.done_msg(
                format!("Successfully removed endpoint override: {lookup_key}"),
                serde_json::json!({ "lookup_key": lookup_key }),
                format!("Removed endpoint override: {lookup_key}"),
            )?;
        }
    }
    Ok(())
}
