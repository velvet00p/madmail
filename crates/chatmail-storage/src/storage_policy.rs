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

//! Maildir durability and throughput policy (Dovecot `mail_fsync` parity).

/// When to fsync message files and maildir directories after delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FsyncMode {
    /// `sync_data` + directory fsync on every write (safest, slowest under concurrency).
    #[default]
    Always,
    /// Content fsync per file; directory fsyncs coalesced across concurrent writes.
    Optimized,
    /// Skip fsync entirely (relay throughput; clients hold local copies).
    Never,
}

impl FsyncMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "always" | "yes" | "on" => Some(Self::Always),
            "optimized" | "opt" => Some(Self::Optimized),
            "never" | "no" | "off" => Some(Self::Never),
            _ => None,
        }
    }

    pub fn sync_file_data(self) -> bool {
        !matches!(self, Self::Never)
    }

    pub fn sync_directory(self) -> bool {
        matches!(self, Self::Always)
    }

    pub fn batch_directory(self) -> bool {
        matches!(self, Self::Optimized)
    }
}

/// Tunables for maildir write, listing, and deduplication paths.
#[derive(Debug, Clone)]
pub struct StoragePolicy {
    pub fsync_mode: FsyncMode,
    /// Content-addressed blob dedup for identical payloads (group media).
    pub cas_enabled: bool,
    /// APPEND bodies at or above this size stream socket → tmp instead of a full `Vec` first.
    pub stream_threshold: usize,
}

impl Default for StoragePolicy {
    fn default() -> Self {
        Self {
            fsync_mode: FsyncMode::Always,
            cas_enabled: true,
            stream_threshold: 64 * 1024,
        }
    }
}

impl StoragePolicy {
    pub fn from_config(mail_fsync: Option<&str>, blob_dedup: Option<&str>) -> Self {
        let mut policy = Self::default();
        if let Some(mode) = mail_fsync.and_then(FsyncMode::parse) {
            policy.fsync_mode = mode;
        }
        if let Some(v) = blob_dedup {
            policy.cas_enabled = matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "on" | "yes" | "true" | "1"
            );
        }
        policy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P11-UT01: fsync mode parsing and batching flags.
    #[test]
    fn p11_ut01_fsync_mode_parse_and_flags() {
        assert_eq!(FsyncMode::parse("always"), Some(FsyncMode::Always));
        assert_eq!(FsyncMode::parse("OPTIMIZED"), Some(FsyncMode::Optimized));
        assert_eq!(FsyncMode::parse("never"), Some(FsyncMode::Never));
        assert!(FsyncMode::parse("bogus").is_none());

        assert!(FsyncMode::Always.sync_file_data());
        assert!(FsyncMode::Always.sync_directory());
        assert!(!FsyncMode::Always.batch_directory());

        assert!(FsyncMode::Optimized.sync_file_data());
        assert!(!FsyncMode::Optimized.sync_directory());
        assert!(FsyncMode::Optimized.batch_directory());

        assert!(!FsyncMode::Never.sync_file_data());
        assert!(!FsyncMode::Never.sync_directory());
    }

    /// P11-UT02: storage policy defaults and config overrides.
    #[test]
    fn p11_ut02_storage_policy_from_config() {
        let default = StoragePolicy::default();
        assert_eq!(default.fsync_mode, FsyncMode::Always);
        assert!(default.cas_enabled);
        assert_eq!(default.stream_threshold, 64 * 1024);

        let fast = StoragePolicy::from_config(Some("never"), Some("off"));
        assert_eq!(fast.fsync_mode, FsyncMode::Never);
        assert!(!fast.cas_enabled);

        let relay = StoragePolicy::from_config(Some("optimized"), None);
        assert_eq!(relay.fsync_mode, FsyncMode::Optimized);
        assert!(relay.cas_enabled);
    }
}
