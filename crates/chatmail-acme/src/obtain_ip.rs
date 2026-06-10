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

//! Let's Encrypt short-lived certificates for public IP addresses (HTTP-01).

use std::net::IpAddr;

use chatmail_types::{ChatmailError, Result};
use instant_acme::{
    AuthorizationStatus, ChallengeType, Identifier, LetsEncrypt, NewOrder, OrderStatus, RetryPolicy,
};
use tracing::info;

use crate::acme_common::{
    ensure_http01_listen_available, ip_order_not_ready_error, load_or_create_le_account,
    map_instant_acme,
};
use crate::http01::Http01Solver;
use crate::obtain::{cert_needs_renewal, write_pem_pair, ObtainOptions};

/// Let's Encrypt profile for ~6-day IP certificates.
pub const LETS_ENCRYPT_SHORTLIVED_PROFILE: &str = "shortlived";

/// Renew IP certificates when fewer than this many days remain (~160h lifetime).
pub const IP_CERT_RENEW_WITHIN_DAYS: u32 = 4;

/// Parse and validate a public IP suitable for Let's Encrypt IP certificates.
pub fn parse_public_ip(domain: &str) -> Result<IpAddr> {
    let bare = domain.trim().trim_matches(|c| c == '[' || c == ']');
    let ip: IpAddr = bare
        .parse()
        .map_err(|_| ChatmailError::config(format!("not an IP address: {domain:?}")))?;
    if !is_public_ip(&ip) {
        return Err(ChatmailError::config(format!(
            "Let's Encrypt IP certificates require a public IP address, not {ip}"
        )));
    }
    Ok(ip)
}

/// Returns true when `ip` is globally routable (not private, loopback, link-local, etc.).
pub fn is_public_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
                || v4.octets()[0] == 0)
        }
        IpAddr::V6(v6) => {
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || (v6.segments()[0] & 0xff00) == 0xfe00)
        }
    }
}

pub async fn obtain_ip_certificate(opts: &ObtainOptions) -> Result<()> {
    let ip = parse_public_ip(&opts.domain)?;

    if opts.skip_if_valid && !cert_needs_renewal(&opts.cert_path(), IP_CERT_RENEW_WITHIN_DAYS)? {
        info!(
            cert = %opts.cert_path().display(),
            "existing IP certificate still valid; skipping issuance"
        );
        println!(
            "Certificate at {} is still valid (≥{IP_CERT_RENEW_WITHIN_DAYS} days left). Use `certificate regenerate` to force renewal.",
            opts.cert_path().display()
        );
        return Ok(());
    }

    ensure_http01_listen_available(&opts.http_listen)?;

    println!(
        "Starting HTTP-01 challenge listener on {} (short-lived IP certificate)…",
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

    let identifiers = [Identifier::Ip(ip)];
    let new_order = NewOrder::new(&identifiers).profile(LETS_ENCRYPT_SHORTLIVED_PROFILE);

    println!("Requesting short-lived certificate for {ip} from Let's Encrypt…");
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
                    "unexpected authorization status: {other:?}"
                )));
            }
        }

        let mut challenge = authz
            .challenge(ChallengeType::Http01)
            .ok_or_else(|| ChatmailError::config("no http-01 challenge for IP identifier"))?;

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
        return Err(ip_order_not_ready_error(status, ip));
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
        "✓ Short-lived IP certificate written:\n  {}\n  {}",
        opts.cert_path().display(),
        opts.key_path().display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn public_ip_validation() {
        assert!(is_public_ip(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_public_ip(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(!is_public_ip(&IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(parse_public_ip("[1.1.1.1]").is_ok());
        assert!(parse_public_ip("not-an-ip").is_err());
    }
}
