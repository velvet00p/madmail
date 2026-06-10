// Copyright (C) 2026 themadorg
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::Deserialize;
use serde_json::{json, Value};

use chatmail_push::{
    consecutive_failures, push_mode, push_runtime_enabled, push_stats_snapshot, set_push_mode,
    PushMode, AUTO_DISABLE_AFTER_FAILURES,
};

use super::status_storage::db_err;
use crate::AdminState;

#[derive(Deserialize)]
struct ActionBody {
    action: String,
}

pub async fn service(st: &AdminState, method: &str, body: &Value) -> super::AdminResult {
    match method {
        "GET" => {
            let mode = push_mode(&st.pool).await.map_err(db_err)?;
            let enabled = push_runtime_enabled(&st.pool).await.map_err(db_err)?;
            Ok((
                200,
                Some(json!({
                    "status": if enabled { "enabled" } else { "disabled" },
                    "mode": mode.as_str(),
                    "successful_notifications": push_stats_snapshot(),
                    "consecutive_failures": consecutive_failures(),
                    "auto_disable_after": AUTO_DISABLE_AFTER_FAILURES,
                })),
            ))
        }
        "POST" => {
            let req: ActionBody = serde_json::from_value(body.clone())
                .map_err(|e| (400, format!("invalid body: {e}")))?;
            let mode = match req.action.to_ascii_lowercase().as_str() {
                "enable" | "on" => PushMode::On,
                "disable" | "off" => PushMode::Off,
                "auto" => PushMode::Auto,
                _ => {
                    return Err((400, "action must be enable, disable, or auto".into()));
                }
            };
            set_push_mode(&st.pool, mode).await.map_err(db_err)?;
            let enabled = mode.runtime_enabled();
            Ok((
                200,
                Some(json!({
                    "status": if enabled { "enabled" } else { "disabled" },
                    "mode": mode.as_str(),
                    "restart_required": true,
                })),
            ))
        }
        _ => Err((405, format!("method {method} not allowed"))),
    }
}
