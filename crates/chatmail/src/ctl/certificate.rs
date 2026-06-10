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

//! `madmail certificate get|regenerate|status|autocert` — TLS via instant-acme or PEM files.

use chatmail_acme::{
    is_valid_ip_for_acme, obtain_certificate, parse_http_listen, read_certificate_info,
    CertIssuerKind, ObtainOptions, DNS_CERT_RENEW_WITHIN_DAYS, IP_CERT_RENEW_WITHIN_DAYS,
};
use chatmail_config::install_cli::{
    CertificateArgs, CertificateAutocertCommand, CertificateAutocertEnableArgs, CertificateCommand,
};
use chatmail_config::{effective_tls_pem_paths, load_config, update_config_autocert, Args};
use chatmail_types::{wrap_ip_domain, ChatmailError, Result};

pub async fn certificate(args: &Args, cmd: &CertificateCommand) -> Result<()> {
    match cmd {
        CertificateCommand::Status => certificate_status(args).await,
        CertificateCommand::Get(a) | CertificateCommand::Regenerate(a) => {
            certificate_obtain(args, cmd, a).await
        }
        CertificateCommand::Autocert(sub) => certificate_autocert(args, sub).await,
    }
}

async fn certificate_autocert(args: &Args, cmd: &CertificateAutocertCommand) -> Result<()> {
    match cmd {
        CertificateAutocertCommand::Enable(a) => autocert_enable(args, a).await,
        CertificateAutocertCommand::Status => autocert_status(args).await,
    }
}

async fn autocert_enable(args: &Args, a: &CertificateAutocertEnableArgs) -> Result<()> {
    let cfg = load_config(&args.config)?;
    let domain = cfg
        .primary_domain
        .clone()
        .or(cfg.mail_domain.clone())
        .or(cfg.hostname.clone())
        .ok_or_else(|| {
            ChatmailError::config(
                "no domain: set primary_domain in config before enabling autocert",
            )
        })?;
    let domain = wrap_ip_domain(&domain);

    update_config_autocert(&args.config, &a.email)?;
    println!("Autocert enabled in {}", args.config.display());
    println!("  tls_mode:   autocert");
    println!("  acme_email: {}", a.email);
    println!("  domain:     {domain}");

    if !a.obtain {
        println!("\nRun `madmail certificate get` to obtain a certificate, then `madmail reload`.");
        return Ok(());
    }

    let http_listen = parse_http_listen(&a.http_listen)?;
    let (cert_path, key_path) = effective_tls_pem_paths(&cfg, &args.state_dir);
    let opts = ObtainOptions {
        domain: domain.clone(),
        email: a.email.clone(),
        state_dir: args.state_dir.clone(),
        cert_path: Some(cert_path),
        key_path: Some(key_path),
        http_listen,
        staging: a.staging,
        skip_if_valid: true,
    };

    println!("\nObtaining Let's Encrypt certificate (HTTP-01 on port 80)…");
    obtain_certificate(&opts).await?;
    println!("\nAutocert is active. Run `madmail reload` (or restart) so the in-process renew-certificate task starts.");
    Ok(())
}

async fn autocert_status(args: &Args) -> Result<()> {
    let cfg = load_config(&args.config)?;
    let domain = cfg
        .primary_domain
        .clone()
        .or(cfg.mail_domain.clone())
        .or(cfg.hostname.clone());
    let enabled = cfg.tls_mode.as_deref() == Some("autocert");

    println!(
        "Autocert mode:   {}",
        if enabled { "enabled" } else { "disabled" }
    );
    if !enabled {
        if let Some(mode) = cfg.tls_mode.as_deref() {
            println!("Current tls_mode: {mode}");
        }
        println!("\nEnable with: madmail certificate autocert enable --email you@example.com");
        return Ok(());
    }

    if let Some(d) = &domain {
        println!("Primary domain:  {}", wrap_ip_domain(d));
    }
    println!(
        "ACME email:      {}",
        domain
            .as_deref()
            .map(|d| cfg.effective_acme_email(d))
            .unwrap_or_else(|| cfg.acme_email.clone().unwrap_or_else(|| "(unset)".into()))
    );

    let renew_within = domain
        .as_deref()
        .map(|d| {
            let wrapped = wrap_ip_domain(d);
            if is_valid_ip_for_acme(&wrapped) {
                IP_CERT_RENEW_WITHIN_DAYS
            } else {
                DNS_CERT_RENEW_WITHIN_DAYS
            }
        })
        .unwrap_or(DNS_CERT_RENEW_WITHIN_DAYS);
    println!(
        "Auto-renewal:    in-process task every 24h (renews when <{renew_within} days remain)"
    );
    println!(
        "Renewal task:    active when server is running (`madmail reload` after config change)"
    );

    let (cert_path, key_path) = effective_tls_pem_paths(&cfg, &args.state_dir);
    println!("Certificate:     {}", cert_path.display());
    println!("Private key:     {}", key_path.display());

    if !key_path.is_file() {
        println!("\nCertificate:     not present — run `madmail certificate get`");
        return Ok(());
    }

    let Some(info) = read_certificate_info(&cert_path)? else {
        println!("\nCertificate:     file missing or unreadable");
        return Ok(());
    };

    println!("\nIssuer:          {}", info.issuer);
    println!("Valid until:     {}", info.not_after);
    println!("Days remaining:  {}", info.days_remaining);
    if info.days_remaining <= 0 {
        println!("Status:          expired — renewal will run on next check");
    } else if info.days_remaining <= renew_within as i64 {
        println!("Status:          valid — renewal due soon");
    } else {
        println!("Status:          valid — renewal not needed yet");
    }

    warn_mode_mismatch("autocert", info.issuer_kind);

    Ok(())
}

async fn certificate_status(args: &Args) -> Result<()> {
    let cfg = load_config(&args.config)?;
    let (cert_path, key_path) = effective_tls_pem_paths(&cfg, &args.state_dir);
    let domain = cfg
        .primary_domain
        .clone()
        .or(cfg.mail_domain.clone())
        .or(cfg.hostname.clone());

    let management = describe_management_mode(&cfg.tls_mode);
    println!("TLS management:  {management}");

    if let Some(d) = &domain {
        println!("Primary domain:  {}", wrap_ip_domain(d));
    }

    if cfg.tls_mode.as_deref() == Some("autocert") {
        println!(
            "ACME email:      {}",
            domain
                .as_deref()
                .map(|d| cfg.effective_acme_email(d))
                .unwrap_or_else(|| cfg.acme_email.clone().unwrap_or_else(|| "(unset)".into()))
        );
    }

    println!("Certificate:     {}", cert_path.display());
    println!("Private key:     {}", key_path.display());

    if !key_path.is_file() {
        println!("\nStatus:          no private key on disk");
        return Ok(());
    }

    let Some(info) = read_certificate_info(&cert_path)? else {
        println!("\nStatus:          certificate file missing or unreadable");
        return Ok(());
    };

    let cert_kind = describe_cert_kind(&cfg.tls_mode, info.issuer_kind);
    println!("\nCertificate type: {cert_kind}");
    println!("Issuer:          {}", info.issuer);
    if !info.subject.is_empty() {
        println!("Subject:         {}", info.subject);
    }
    if !info.subject_alt_names.is_empty() {
        println!("SANs:            {}", info.subject_alt_names.join(", "));
    }
    println!("Valid from:      {}", info.not_before);
    println!("Valid until:     {}", info.not_after);
    println!("Days remaining:  {}", info.days_remaining);

    if info.days_remaining <= 0 {
        println!("Status:          expired");
    } else {
        println!("Status:          valid");
    }

    if cfg.tls_mode.as_deref() == Some("autocert") {
        let renew_within = domain
            .as_deref()
            .map(|d| {
                let wrapped = wrap_ip_domain(d);
                if is_valid_ip_for_acme(&wrapped) {
                    IP_CERT_RENEW_WITHIN_DAYS
                } else {
                    DNS_CERT_RENEW_WITHIN_DAYS
                }
            })
            .unwrap_or(DNS_CERT_RENEW_WITHIN_DAYS);
        println!(
            "Auto-renewal:    enabled (in-process task every 24h; renews when <{renew_within} days remain)"
        );
        if info.days_remaining <= renew_within as i64 {
            println!("Renewal:         due soon (within renewal window)");
        } else {
            println!("Renewal:         not needed yet");
        }
    } else if cfg.tls_mode.as_deref() == Some("file") {
        println!("Auto-renewal:    disabled (replace PEM files manually, then reload)");
        println!("Tip:             `madmail certificate autocert enable --email you@example.com`");
    } else if cfg.tls_mode.as_deref() == Some("self_signed") {
        println!("Auto-renewal:    disabled (self-signed; use `madmail certificate regenerate` or reinstall)");
    }

    if let (Some(mode), kind) = (cfg.tls_mode.as_deref(), info.issuer_kind) {
        warn_mode_mismatch(mode, kind);
    }

    Ok(())
}

async fn certificate_obtain(
    args: &Args,
    cmd: &CertificateCommand,
    a: &CertificateArgs,
) -> Result<()> {
    let cfg = load_config(&args.config)?;
    let domain = a
        .domain
        .clone()
        .or(cfg.primary_domain.clone())
        .or(cfg.mail_domain.clone())
        .or(cfg.hostname.clone())
        .ok_or_else(|| {
            ChatmailError::config("no domain: pass --domain or set primary_domain in config")
        })?;
    let domain = wrap_ip_domain(&domain);
    let bare = domain.trim_matches(|c| c == '[' || c == ']');
    let email = a
        .email
        .clone()
        .unwrap_or_else(|| cfg.effective_acme_email(bare));

    let (skip_if_valid, force_label) = match cmd {
        CertificateCommand::Get { .. } => (!a.force, "get"),
        CertificateCommand::Regenerate { .. } => (false, "regenerate"),
        CertificateCommand::Status | CertificateCommand::Autocert { .. } => unreachable!(),
    };

    let http_listen = parse_http_listen(&a.http_listen)?;
    let (cert_path, key_path) = effective_tls_pem_paths(&cfg, &args.state_dir);
    let opts = ObtainOptions {
        domain: domain.clone(),
        email,
        state_dir: args.state_dir.clone(),
        cert_path: Some(cert_path),
        key_path: Some(key_path),
        http_listen,
        staging: a.staging,
        skip_if_valid,
    };

    println!("madmail certificate {force_label} for {domain}");
    obtain_certificate(&opts).await
}

fn describe_management_mode(tls_mode: &Option<String>) -> &'static str {
    match tls_mode.as_deref() {
        Some("autocert") => "autocert (Let's Encrypt, auto-managed)",
        Some("self_signed") => "self_signed (local self-signed)",
        Some("file") => "file (static PEM paths)",
        Some(_other) => "unknown (check config tls_mode)",
        None => "unspecified (defaults to tls file paths in config)",
    }
}

fn describe_cert_kind(tls_mode: &Option<String>, issuer_kind: CertIssuerKind) -> &'static str {
    match tls_mode.as_deref() {
        Some("autocert") => "Let's Encrypt (auto-managed)",
        Some("self_signed") => "self-signed",
        Some("file") => "static file (operator-provided)",
        _ => match issuer_kind {
            CertIssuerKind::LetsEncrypt => "Let's Encrypt",
            CertIssuerKind::SelfSigned => "self-signed",
            CertIssuerKind::Other => "third-party / static",
        },
    }
}

fn warn_mode_mismatch(config_mode: &str, issuer_kind: CertIssuerKind) {
    let mismatch = match (config_mode, issuer_kind) {
        ("autocert", CertIssuerKind::SelfSigned) => {
            Some("config says autocert but the on-disk certificate looks self-signed")
        }
        ("self_signed", CertIssuerKind::LetsEncrypt) => {
            Some("config says self_signed but the on-disk certificate is from Let's Encrypt")
        }
        _ => None,
    };
    if let Some(msg) = mismatch {
        eprintln!("\nWarning: {msg}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn management_mode_labels() {
        assert!(describe_management_mode(&Some("autocert".into())).contains("auto-managed"));
        assert!(describe_management_mode(&Some("file".into())).contains("static"));
    }
}
