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

//! In-process Let's Encrypt renewal (`chatmail-tasks` / `tls_mode = autocert`).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chatmail_acme::{
    cert_needs_renewal, is_valid_ip_for_acme, obtain_certificate, ObtainOptions,
    DNS_CERT_RENEW_WITHIN_DAYS, IP_CERT_RENEW_WITHIN_DAYS,
};
use chatmail_config::{effective_tls_pem_paths, AppConfig};
use chatmail_tasks::{CertRenewOutcome, CertificateRenewer};
use chatmail_types::{wrap_ip_domain, ChatmailError, Result};
use tokio::time::timeout;
use tracing::info;

use super::SupervisorInner;

pub(crate) fn supervisor_cert_renewer(inner: Arc<SupervisorInner>) -> Arc<dyn CertificateRenewer> {
    Arc::new(SupervisorCertRenewer { inner })
}

struct SupervisorCertRenewer {
    inner: Arc<SupervisorInner>,
}

#[async_trait]
impl CertificateRenewer for SupervisorCertRenewer {
    async fn renew_if_needed(&self) -> Result<CertRenewOutcome> {
        self.inner.renew_autocert_certificate().await
    }
}

impl SupervisorInner {
    pub(super) async fn renew_autocert_certificate(&self) -> Result<CertRenewOutcome> {
        if self.file_config.tls_mode.as_deref() != Some("autocert") {
            return Ok(CertRenewOutcome::skipped("tls_mode is not autocert"));
        }

        let (cert_path, key_path, domain, renew_within_days) =
            renewal_target(&self.file_config, &self.state_dir)?;

        if !cert_needs_renewal(&cert_path, renew_within_days)? {
            return Ok(CertRenewOutcome::skipped(format!(
                "certificate still valid (≥{renew_within_days} days left)"
            )));
        }

        info!(
            domain = %domain,
            cert = %cert_path.display(),
            "scheduled certificate renewal: releasing port 80"
        );
        self.stop_http_plain_listener().await?;

        let opts = ObtainOptions {
            domain: domain.clone(),
            email: self.file_config.effective_acme_email(&domain),
            state_dir: self.state_dir.clone(),
            cert_path: Some(cert_path.clone()),
            key_path: Some(key_path),
            http_listen: "0.0.0.0:80".parse().expect("valid default listen"),
            staging: false,
            skip_if_valid: true,
        };

        let renew_result = obtain_certificate(&opts).await;
        self.soft_reload().await?;
        renew_result?;

        Ok(CertRenewOutcome::renewed(format!(
            "Let's Encrypt certificate renewed for {domain}"
        )))
    }

    pub(super) async fn stop_http_plain_listener(&self) -> Result<()> {
        let mut guard = self.listeners.lock().await;
        let Some(active) = guard.as_mut() else {
            return Ok(());
        };
        if let Some(slot) = active.http_plain.take() {
            slot.cancel.cancel();
            let _ = timeout(Duration::from_secs(8), slot.join).await;
        }
        Ok(())
    }
}

/// CLI / `madmail tasks run renew-certificate` when the server is not running.
pub async fn renew_autocert_from_cli(
    config: &AppConfig,
    state_dir: &Path,
) -> Result<CertRenewOutcome> {
    if config.tls_mode.as_deref() != Some("autocert") {
        return Ok(CertRenewOutcome::skipped("tls_mode is not autocert"));
    }

    let (cert_path, key_path, domain, renew_within_days) = renewal_target(config, state_dir)?;

    if !cert_needs_renewal(&cert_path, renew_within_days)? {
        return Ok(CertRenewOutcome::skipped(format!(
            "certificate still valid (≥{renew_within_days} days left)"
        )));
    }

    let opts = ObtainOptions {
        domain: domain.clone(),
        email: config.effective_acme_email(&domain),
        state_dir: state_dir.to_path_buf(),
        cert_path: Some(cert_path),
        key_path: Some(key_path),
        http_listen: "0.0.0.0:80".parse().expect("valid default listen"),
        staging: false,
        skip_if_valid: true,
    };

    obtain_certificate(&opts).await?;
    Ok(CertRenewOutcome::renewed(format!(
        "Let's Encrypt certificate renewed for {domain}"
    )))
}

fn renewal_target(
    config: &AppConfig,
    state_dir: &Path,
) -> Result<(std::path::PathBuf, std::path::PathBuf, String, u32)> {
    let domain = config
        .primary_domain
        .clone()
        .or(config.mail_domain.clone())
        .or(config.hostname.clone())
        .ok_or_else(|| {
            ChatmailError::config("no primary_domain in config for certificate renewal")
        })?;
    let domain = wrap_ip_domain(&domain);
    let (cert_path, key_path) = effective_tls_pem_paths(config, state_dir);
    let renew_within_days = if is_valid_ip_for_acme(&domain) {
        IP_CERT_RENEW_WITHIN_DAYS
    } else {
        DNS_CERT_RENEW_WITHIN_DAYS
    };
    Ok((cert_path, key_path, domain, renew_within_days))
}
