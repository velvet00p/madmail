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

//! Obtain / renew Let's Encrypt certificates via [instant-acme](https://github.com/djc/instant-acme)
//! (DNS hostnames and public IP short-lived profile).

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use chatmail_types::{ChatmailError, Result};
use instant_acme::{
    AuthorizationStatus, ChallengeType, Identifier, LetsEncrypt, NewOrder, OrderStatus, RetryPolicy,
};
use openssl::x509::X509;
use tracing::info;

use crate::acme_common::{
    dns_order_not_ready_error, ensure_http01_listen_available, load_or_create_le_account,
    map_instant_acme,
};
use crate::http01::Http01Solver;
use crate::obtain_ip::{obtain_ip_certificate, parse_public_ip};

/// Options for `certificate get` / `regenerate` and install-time issuance.
#[derive(Debug, Clone)]
pub struct ObtainOptions {
    pub domain: String,
    pub email: String,
    pub state_dir: PathBuf,
    /// PEM output paths (default: `{state_dir}/certs/`).
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
    /// Listen address for HTTP-01 (default `0.0.0.0:80`).
    pub http_listen: SocketAddr,
    pub staging: bool,
    /// Skip issuance when an existing cert is still valid (`get` only).
    pub skip_if_valid: bool,
}

/// Renewal threshold for normal 90-day DNS certificates.
pub const DNS_CERT_RENEW_WITHIN_DAYS: u32 = 30;

impl ObtainOptions {
    pub fn cert_path(&self) -> PathBuf {
        self.cert_path
            .clone()
            .unwrap_or_else(|| self.state_dir.join("certs/fullchain.pem"))
    }

    pub fn key_path(&self) -> PathBuf {
        self.key_path
            .clone()
            .unwrap_or_else(|| self.state_dir.join("certs/privkey.pem"))
    }

    pub fn account_key_path(&self) -> PathBuf {
        self.state_dir.join("autocert/account.key.pem")
    }

    pub fn le_account_path(&self) -> PathBuf {
        self.state_dir.join("autocert/le-account.json")
    }
}

/// Obtain (or renew) a certificate and write PEM files.
pub async fn obtain_certificate(opts: &ObtainOptions) -> Result<()> {
    if parse_public_ip(&opts.domain).is_ok() {
        return obtain_ip_certificate(opts).await;
    }
    obtain_dns_certificate(opts).await
}

async fn obtain_dns_certificate(opts: &ObtainOptions) -> Result<()> {
    let domain = normalize_acme_domain(&opts.domain)?;
    if opts.skip_if_valid && !cert_needs_renewal(&opts.cert_path(), DNS_CERT_RENEW_WITHIN_DAYS)? {
        info!(
            cert = %opts.cert_path().display(),
            "existing certificate still valid; skipping issuance"
        );
        println!(
            "Certificate at {} is still valid (≥{DNS_CERT_RENEW_WITHIN_DAYS} days left). Use `certificate regenerate` to force renewal.",
            opts.cert_path().display()
        );
        return Ok(());
    }

    ensure_http01_listen_available(&opts.http_listen)?;

    println!(
        "Starting HTTP-01 challenge listener on {}…",
        opts.http_listen
    );
    let solver = Http01Solver::new();
    let http_handle = solver
        .start(&opts.http_listen)
        .map_err(|e| ChatmailError::config(format!("bind HTTP-01 on {}: {e}", opts.http_listen)))?;

    let directory_url = if opts.staging {
        LetsEncrypt::Staging.url().to_owned()
    } else {
        LetsEncrypt::Production.url().to_owned()
    };

    let account = load_or_create_le_account(opts, &directory_url).await?;

    let identifiers = [Identifier::Dns(domain.clone())];
    let new_order = NewOrder::new(&identifiers);

    println!("Requesting certificate for {domain} from Let's Encrypt…");
    let mut order = account
        .new_order(&new_order)
        .await
        .map_err(map_instant_acme)?;

    let mut authorizations = order.authorizations();
    while let Some(result) = authorizations.next().await {
        let mut authz = result.map_err(map_instant_acme)?;
        match authz.status {
            AuthorizationStatus::Valid => continue,
            AuthorizationStatus::Pending => {}
            other => {
                return Err(ChatmailError::config(format!(
                    "unexpected authorization status for {domain}: {other:?}"
                )));
            }
        }

        let mut challenge = authz
            .challenge(ChallengeType::Http01)
            .ok_or_else(|| ChatmailError::config(format!("no http-01 challenge for {domain}")))?;

        let host = challenge.identifier().to_string();
        let token = challenge.token.clone();
        let key_auth = challenge.key_authorization().as_str().to_string();

        solver.present(host.clone(), token.clone(), key_auth);
        challenge.set_ready().await.map_err(map_instant_acme)?;
        info!(%host, %token, "http-01 challenge marked ready (waiting for Let's Encrypt validation)");
    }

    let retry = RetryPolicy::new().timeout(std::time::Duration::from_secs(60));
    let status = order.poll_ready(&retry).await.map_err(map_instant_acme)?;
    if status != OrderStatus::Ready {
        return Err(dns_order_not_ready_error(status, &domain));
    }

    let key_pem = order.finalize().await.map_err(map_instant_acme)?;
    let chain_pem = order
        .poll_certificate(&RetryPolicy::default())
        .await
        .map_err(map_instant_acme)?;

    write_pem_pair(
        &opts.cert_path(),
        &opts.key_path(),
        chain_pem.as_bytes(),
        key_pem.as_bytes(),
    )?;

    http_handle.stop().await.map_err(ChatmailError::config)?;

    println!(
        "✓ Certificate written:\n  {}\n  {}",
        opts.cert_path().display(),
        opts.key_path().display()
    );
    Ok(())
}

fn normalize_acme_domain(domain: &str) -> Result<String> {
    let d = domain.trim().trim_matches(|c| c == '[' || c == ']');
    if d.is_empty() {
        return Err(ChatmailError::config("domain is empty"));
    }
    if d.parse::<std::net::IpAddr>().is_ok() {
        return Err(ChatmailError::config(
            "use obtain_certificate with a public IP or --auto-ip-cert on install (IP certs use the shortlived profile)",
        ));
    }
    if !d.contains('.') || d == "localhost" || d.ends_with(".local") {
        return Err(ChatmailError::config(format!(
            "invalid domain for ACME: {d}"
        )));
    }
    Ok(d.to_string())
}

/// Returns true if cert is missing or expires within `renew_within_days`.
pub fn cert_needs_renewal(cert_path: &Path, renew_within_days: u32) -> Result<bool> {
    let Ok(pem) = std::fs::read(cert_path) else {
        return Ok(true);
    };
    let cert =
        X509::from_pem(&pem).map_err(|e| ChatmailError::config(format!("parse cert: {e}")))?;
    let not_after = cert.not_after();
    let renew_at = openssl::asn1::Asn1Time::days_from_now(renew_within_days)
        .map_err(|e| ChatmailError::config(format!("renew threshold: {e}")))?;
    Ok(not_after < renew_at.as_ref())
}

pub(crate) fn write_pem_pair(
    cert_path: &Path,
    key_path: &Path,
    chain_pem: &[u8],
    key_pem: &[u8],
) -> Result<()> {
    if let Some(parent) = cert_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ChatmailError::config(format!("create {}: {e}", parent.display())))?;
    }
    std::fs::write(cert_path, chain_pem)
        .map_err(|e| ChatmailError::config(format!("write {}: {e}", cert_path.display())))?;
    std::fs::write(key_path, key_pem)
        .map_err(|e| ChatmailError::config(format!("write {}: {e}", key_path.display())))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(cert_path, std::fs::Permissions::from_mode(0o640)).ok();
        std::fs::set_permissions(key_path, std::fs::Permissions::from_mode(0o600)).ok();
    }
    Ok(())
}

pub fn parse_http_listen(addr: &str) -> Result<SocketAddr> {
    addr.parse()
        .map_err(|e| ChatmailError::config(format!("invalid --http-listen {addr:?}: {e}")))
}
