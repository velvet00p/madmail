// Copyright (C) 2026 themadorg
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Delta Chat push (`XDELTAPUSH` / `SETMETADATA /private/devicetoken`).

use chatmail_db::DbPool;
use chatmail_types::Result;

/// Whether push notifications are enabled (admin toggle, default off).
pub async fn push_enabled(pool: &DbPool) -> Result<bool> {
    chatmail_push::push_runtime_enabled(pool).await
}
