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

//! Coalesced directory fsync for `FsyncMode::Optimized` (Dovecot transaction batching).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chatmail_types::{ChatmailError, Result};
use tokio::sync::Mutex;

use crate::maildir::fsync_dir;
use crate::storage_policy::FsyncMode;

const BATCH_DELAY: Duration = Duration::from_millis(10);

#[derive(Debug)]
struct FsyncCoordinatorInner {
    mode: FsyncMode,
    pending_dirs: Mutex<HashSet<PathBuf>>,
    flush_scheduled: AtomicBool,
}

/// Applies per-file and per-directory durability according to [`FsyncMode`].
#[derive(Debug, Clone)]
pub struct FsyncCoordinator {
    inner: Arc<FsyncCoordinatorInner>,
}

impl FsyncCoordinator {
    pub fn new(mode: FsyncMode) -> Self {
        Self {
            inner: Arc::new(FsyncCoordinatorInner {
                mode,
                pending_dirs: Mutex::new(HashSet::new()),
                flush_scheduled: AtomicBool::new(false),
            }),
        }
    }

    pub fn mode(&self) -> FsyncMode {
        self.inner.mode
    }

    pub async fn sync_file_data(&self, file: &mut tokio::fs::File) -> Result<()> {
        if !self.inner.mode.sync_file_data() {
            return Ok(());
        }
        file.sync_data().await.map_err(ChatmailError::from)
    }

    pub async fn commit_directory(&self, dir: &Path) -> Result<()> {
        match self.inner.mode {
            FsyncMode::Always => fsync_dir(dir).await,
            FsyncMode::Never => Ok(()),
            FsyncMode::Optimized => self.defer_directory_fsync(dir).await,
        }
    }

    async fn defer_directory_fsync(&self, dir: &Path) -> Result<()> {
        self.inner
            .pending_dirs
            .lock()
            .await
            .insert(dir.to_path_buf());
        if self
            .inner
            .flush_scheduled
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            let inner = Arc::clone(&self.inner);
            tokio::spawn(async move {
                tokio::time::sleep(BATCH_DELAY).await;
                let _ = flush_pending_inner(&inner).await;
                inner.flush_scheduled.store(false, Ordering::Release);
            });
        }
        Ok(())
    }

    /// Flush all deferred directory fsyncs (tests and explicit batch boundaries).
    pub async fn flush_pending(&self) -> Result<()> {
        flush_pending_inner(&self.inner).await
    }

    #[cfg(test)]
    pub async fn pending_count(&self) -> usize {
        self.inner.pending_dirs.lock().await.len()
    }
}

async fn flush_pending_inner(inner: &FsyncCoordinatorInner) -> Result<()> {
    let dirs: Vec<PathBuf> = inner.pending_dirs.lock().await.drain().collect();
    for dir in dirs {
        fsync_dir(&dir).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P11-UT03: optimized mode defers directory fsync until flush.
    #[tokio::test]
    async fn p11_ut03_optimized_batches_directory_fsync() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("new");
        tokio::fs::create_dir_all(&dir).await.unwrap();

        let coord = FsyncCoordinator::new(FsyncMode::Optimized);
        coord.commit_directory(&dir).await.unwrap();
        coord.commit_directory(&dir).await.unwrap();

        assert_eq!(coord.pending_count().await, 1, "same dir deduped in pending set");
        coord.flush_pending().await.unwrap();
        assert_eq!(coord.pending_count().await, 0, "flush drains pending");

        let always = FsyncCoordinator::new(FsyncMode::Always);
        always.commit_directory(&dir).await.unwrap();

        let never = FsyncCoordinator::new(FsyncMode::Never);
        never.commit_directory(&dir).await.unwrap();
        assert_eq!(never.pending_count().await, 0);
    }

    /// P11-UT18: optimized mode coalesces multiple distinct directories.
    #[tokio::test]
    async fn p11_ut18_optimized_coalesces_multiple_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let dir_a = tmp.path().join("new");
        let dir_b = tmp.path().join("cur");
        tokio::fs::create_dir_all(&dir_a).await.unwrap();
        tokio::fs::create_dir_all(&dir_b).await.unwrap();

        let coord = FsyncCoordinator::new(FsyncMode::Optimized);
        coord.commit_directory(&dir_a).await.unwrap();
        coord.commit_directory(&dir_b).await.unwrap();
        assert_eq!(coord.pending_count().await, 2);
        coord.flush_pending().await.unwrap();
        assert_eq!(coord.pending_count().await, 0);
    }

    /// P11-UT19: never mode skips file sync_data.
    #[tokio::test]
    async fn p11_ut19_never_mode_skips_file_sync() {
        let coord = FsyncCoordinator::new(FsyncMode::Never);
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("f");
        let mut file = tokio::fs::File::create(&path).await.unwrap();
        tokio::io::AsyncWriteExt::write_all(&mut file, b"x")
            .await
            .unwrap();
        coord.sync_file_data(&mut file).await.unwrap();
    }
}
