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

//! Content-addressed blob store for deduplicating identical payloads (Stalwart CAS parity).

use std::path::{Path, PathBuf};

use chatmail_types::{ChatmailError, Result};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

/// SHA-256 digest of a blob payload.
pub type BlobHash = [u8; 32];

/// Bytes captured from the start of a streamed literal for policy checks (PGP / Secure-Join).
///
/// A valid PGP/MIME message carries its `application/pgp-encrypted` marker and MIME structure in
/// the first part header, and Delta Chat Secure-Join handshakes are tiny. Capturing a bounded
/// prefix during streaming lets the IMAP layer enforce encryption without re-reading or
/// materializing a multi-megabyte body (Dovecot validates from the stream head, not the tail).
pub const HEADER_SCAN_PREFIX: usize = 64 * 1024;

pub fn hash_bytes(data: &[u8]) -> BlobHash {
    Sha256::digest(data).into()
}

pub fn hash_to_hex(hash: &BlobHash) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

/// Shared blob directory under `{state_dir}/blobs/`.
#[derive(Debug, Clone)]
pub struct ContentStore {
    root: PathBuf,
}

impl ContentStore {
    pub fn new(state_dir: &Path) -> Self {
        Self {
            root: state_dir.join("blobs"),
        }
    }

    pub fn blob_path(&self, hash: &BlobHash) -> PathBuf {
        let hex = hash_to_hex(hash);
        self.root.join(&hex[..2]).join(hex)
    }

    /// Store bytes at the content hash if missing; returns the canonical blob path.
    pub async fn put_if_absent(&self, hash: BlobHash, body: &[u8]) -> Result<PathBuf> {
        let dest = self.blob_path(&hash);
        if tokio::fs::metadata(&dest)
            .await
            .map(|m| m.len() as usize == body.len())
            .unwrap_or(false)
        {
            return Ok(dest);
        }
        let tmp = dest.with_extension("part");
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut file = tokio::fs::File::create(&tmp).await?;
        file.write_all(body).await?;
        file.sync_data().await?;
        match tokio::fs::rename(&tmp, &dest).await {
            Ok(()) => Ok(dest),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                tokio::fs::remove_file(&tmp).await.ok();
                Ok(dest)
            }
            Err(e) => Err(ChatmailError::from(e)),
        }
    }

    /// Move a fully-written tmp file into the CAS tree, skipping the write when the hash exists.
    pub async fn ingest_tmp(&self, hash: BlobHash, tmp: &Path, size: u64) -> Result<PathBuf> {
        let dest = self.blob_path(&hash);
        if tokio::fs::metadata(&dest)
            .await
            .map(|m| m.len() == size)
            .unwrap_or(false)
        {
            tokio::fs::remove_file(tmp).await.ok();
            return Ok(dest);
        }
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        match tokio::fs::rename(tmp, &dest).await {
            Ok(()) => Ok(dest),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                tokio::fs::remove_file(tmp).await.ok();
                Ok(dest)
            }
            Err(e) => Err(ChatmailError::from(e)),
        }
    }

    /// Hard link (or copy on EXDEV) a canonical blob into a maildir destination.
    pub async fn link_into(&self, canonical: &Path, dest: &Path) -> Result<()> {
        if dest.exists() {
            return Err(ChatmailError::storage(format!(
                "destination {} already exists",
                dest.display()
            )));
        }
        match tokio::fs::hard_link(canonical, dest).await {
            Ok(()) => Ok(()),
            Err(e) if is_cross_device_link(&e) => {
                tokio::fs::copy(canonical, dest).await?;
                Ok(())
            }
            Err(e) => Err(ChatmailError::from(e)),
        }
    }
}

/// Stream `size` bytes from `reader` into `tmp_path`, returning the SHA-256 digest, the number of
/// bytes written, and a bounded header prefix (up to [`HEADER_SCAN_PREFIX`]) captured on the fly.
///
/// The prefix lets callers run header-based policy checks without re-reading the whole file, so a
/// 50 MB literal never has to be materialized in memory for the encryption gate.
///
/// `do_sync` controls whether we call sync_data on the tmp file after writing. This is wired
/// from the caller's StoragePolicy (via FsyncMode::sync_file_data) so that `mail_fsync never`
/// actually skips the sync on the large streaming APPEND path.
pub async fn stream_to_tmp_with_hash<R>(
    tmp_path: &Path,
    reader: &mut R,
    size: u64,
    do_sync: bool,
) -> Result<(BlobHash, u64, Vec<u8>)>
where
    R: AsyncRead + Unpin,
{
    let mut file = tokio::fs::File::create(tmp_path).await?;
    let mut hasher = Sha256::new();
    let mut remaining = size;
    let mut buf = vec![0u8; 64 * 1024];
    let mut header = Vec::with_capacity(HEADER_SCAN_PREFIX.min(size as usize));
    while remaining > 0 {
        let chunk = remaining.min(buf.len() as u64) as usize;
        reader.read_exact(&mut buf[..chunk]).await?;
        hasher.update(&buf[..chunk]);
        file.write_all(&buf[..chunk]).await?;
        if header.len() < HEADER_SCAN_PREFIX {
            let take = (HEADER_SCAN_PREFIX - header.len()).min(chunk);
            header.extend_from_slice(&buf[..take]);
        }
        remaining -= chunk as u64;
    }
    if do_sync {
        file.sync_data().await?;
    }
    Ok((hasher.finalize().into(), size, header))
}

/// Stream `size` bytes directly to `dest_path` (final location) **without** computing a full
/// content hash. Only a bounded header prefix is captured for policy checks.
///
/// This is the ultra-fast path for `mail_fsync=never` + large distinct messages (the main
/// concurrent benchmark workload). Dovecot never pays a full-body hash on every delivery
/// because it has no built-in content dedup at the maildir layer.
pub async fn stream_to_file_no_hash<R>(
    dest_path: &Path,
    reader: &mut R,
    size: u64,
) -> Result<(u64, Vec<u8>)>
where
    R: AsyncRead + Unpin,
{
    let mut file = tokio::fs::File::create(dest_path).await?;
    let mut remaining = size;
    let mut buf = vec![0u8; 64 * 1024];
    let mut header = Vec::with_capacity(HEADER_SCAN_PREFIX.min(size as usize));
    while remaining > 0 {
        let chunk = remaining.min(buf.len() as u64) as usize;
        reader.read_exact(&mut buf[..chunk]).await?;
        file.write_all(&buf[..chunk]).await?;
        if header.len() < HEADER_SCAN_PREFIX {
            let take = (HEADER_SCAN_PREFIX - header.len()).min(chunk);
            header.extend_from_slice(&buf[..take]);
        }
        remaining -= chunk as u64;
    }
    // No sync_data here — caller is responsible (Never mode → no sync)
    Ok((size, header))
}

/// Same as `stream_to_file_no_hash` but uses a much larger write buffer (1 MiB).
/// This reduces syscall overhead during very large writes under the relaxed
/// `mail_fsync=never` path (another small efficiency Dovecot tunes for high-throughput
/// relaxed configurations).
pub async fn stream_to_file_no_hash_largebuf<R>(
    dest_path: &Path,
    reader: &mut R,
    size: u64,
) -> Result<(u64, Vec<u8>)>
where
    R: AsyncRead + Unpin,
{
    let mut file = tokio::fs::File::create(dest_path).await?;
    let mut remaining = size;
    let mut buf = vec![0u8; 1024 * 1024]; // 1 MiB write buffer for speed
    let mut header = Vec::with_capacity(HEADER_SCAN_PREFIX.min(size as usize));
    while remaining > 0 {
        let chunk = remaining.min(buf.len() as u64) as usize;
        reader.read_exact(&mut buf[..chunk]).await?;
        file.write_all(&buf[..chunk]).await?;
        if header.len() < HEADER_SCAN_PREFIX {
            let take = (HEADER_SCAN_PREFIX - header.len()).min(chunk);
            header.extend_from_slice(&buf[..take]);
        }
        remaining -= chunk as u64;
    }
    Ok((size, header))
}

#[cfg(unix)]
fn is_cross_device_link(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(18)
}

#[cfg(not(unix))]
fn is_cross_device_link(e: &std::io::Error) -> bool {
    let _ = e;
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P11-UT07: identical payloads share one on-disk CAS blob.
    #[tokio::test]
    async fn p11_ut07_cas_deduplicates_identical_bodies() {
        let tmp = tempfile::tempdir().unwrap();
        let cas = ContentStore::new(tmp.path());
        let body = b"From: a@test\r\n\r\nsame encrypted blob\xff\x00";

        let hash = hash_bytes(body);
        let p1 = cas.put_if_absent(hash, body).await.unwrap();
        let p2 = cas.put_if_absent(hash, body).await.unwrap();
        assert_eq!(p1, p2);

        let dest_a = tmp.path().join("mail_a");
        let dest_b = tmp.path().join("mail_b");
        cas.link_into(&p1, &dest_a).await.unwrap();
        cas.link_into(&p1, &dest_b).await.unwrap();

        assert_eq!(tokio::fs::read(&dest_a).await.unwrap(), body);
        assert_eq!(tokio::fs::read(&dest_b).await.unwrap(), body);

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                std::fs::metadata(&p1).unwrap().ino(),
                std::fs::metadata(&dest_a).unwrap().ino()
            );
        }
    }

    /// P11-UT08: streaming hash matches buffered hash.
    #[tokio::test]
    async fn p11_ut08_stream_to_tmp_hash_matches() {
        let tmp = tempfile::tempdir().unwrap();
        let body: Vec<u8> = (0u8..200).collect();
        let expected = hash_bytes(&body);

        let tmp_path = tmp.path().join("part");
        let mut cursor = std::io::Cursor::new(body.clone());
        let (hash, n, header) = stream_to_tmp_with_hash(&tmp_path, &mut cursor, body.len() as u64, true)
            .await
            .unwrap();

        assert_eq!(hash, expected);
        assert_eq!(n, body.len() as u64);
        assert_eq!(header, body);
        assert_eq!(tokio::fs::read(&tmp_path).await.unwrap(), body);
    }

    /// P11-UT25: header capture is bounded and stops at HEADER_SCAN_PREFIX for large literals.
    #[tokio::test]
    async fn p11_ut25_header_prefix_is_bounded() {
        let tmp = tempfile::tempdir().unwrap();
        let size = HEADER_SCAN_PREFIX + 4096;
        let body: Vec<u8> = (0u8..=255).cycle().take(size).collect();

        let tmp_path = tmp.path().join("big.part");
        let mut cursor = std::io::Cursor::new(body.clone());
        let (_, n, header) = stream_to_tmp_with_hash(&tmp_path, &mut cursor, size as u64, true)
            .await
            .unwrap();

        assert_eq!(n, size as u64);
        assert_eq!(header.len(), HEADER_SCAN_PREFIX);
        assert_eq!(header.as_slice(), &body[..HEADER_SCAN_PREFIX]);
    }

    /// P11-UT09: ingest_tmp skips rewrite when hash already stored.
    #[tokio::test]
    async fn p11_ut09_ingest_tmp_skips_existing_blob() {
        let tmp = tempfile::tempdir().unwrap();
        let cas = ContentStore::new(tmp.path());
        let body = b"payload";
        let hash = hash_bytes(body);

        let canonical = cas.put_if_absent(hash, body).await.unwrap();
        let tmp_part = tmp.path().join("streaming.part");
        tokio::fs::write(&tmp_part, body).await.unwrap();

        let again = cas
            .ingest_tmp(hash, &tmp_part, body.len() as u64)
            .await
            .unwrap();
        assert_eq!(again, canonical);
        assert!(!tmp_part.exists());
    }
}
