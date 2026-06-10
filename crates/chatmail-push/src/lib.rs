// Copyright (C) 2026 themadorg
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Delta Chat push notifications (`XDELTAPUSH` / `SETMETADATA /private/devicetoken`).
//!
//! Device tokens are stored per user; on inbound delivery the notifier POSTs encrypted
//! tokens to the central Delta Chat notification proxy (`notifications.delta.chat`).

mod enabled;
mod mode;
mod notifier;
mod stats;
mod store;

pub use enabled::push_runtime_enabled;
pub use mode::{
    consecutive_failures, push_mode, record_delivery_failure, record_delivery_success,
    reset_consecutive_failures, set_push_mode, PushMode, AUTO_DISABLE_AFTER_FAILURES,
};
pub use notifier::PushNotifier;
pub use stats::{
    hydrate as hydrate_push_stats, record_successful_delivery, snapshot as push_stats_snapshot,
    start_flush_task as start_push_stats_flush_task,
};
pub use store::{list_device_tokens, remove_device_token, upsert_device_token, DEVICETOKEN_KEY};

/// Default HTTPS endpoint (Dovecot/chatmaild `notifier.py`).
pub const DEFAULT_NOTIFY_URL: &str = "https://notifications.delta.chat/notify";