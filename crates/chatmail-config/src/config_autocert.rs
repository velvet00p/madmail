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

//! Persist `tls_mode` and `acme_email` in `maddy.conf` / `chatmail.toml`.

use std::path::Path;

use chatmail_types::{ChatmailError, Result};

/// Enable or update autocert settings in the on-disk config file.
pub fn update_config_autocert(config_path: &Path, acme_email: &str) -> Result<()> {
    if acme_email.trim().is_empty() || !acme_email.contains('@') {
        return Err(ChatmailError::config(
            "invalid ACME email (expected user@domain)",
        ));
    }
    if !config_path.is_file() {
        return Err(ChatmailError::config(format!(
            "config file not found: {}",
            config_path.display()
        )));
    }

    let ext = config_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    if ext == "toml" {
        update_toml_autocert(config_path, acme_email)
    } else {
        update_maddy_autocert(config_path, acme_email)
    }
}

fn update_toml_autocert(config_path: &Path, acme_email: &str) -> Result<()> {
    let raw = std::fs::read_to_string(config_path)?;
    let mut doc: toml::Table =
        toml::from_str(&raw).map_err(|e| ChatmailError::config(format!("invalid TOML: {e}")))?;

    doc.insert("tls_mode".into(), toml::Value::String("autocert".into()));
    doc.insert(
        "acme_email".into(),
        toml::Value::String(acme_email.to_string()),
    );

    let out = toml::to_string_pretty(&doc)
        .map_err(|e| ChatmailError::config(format!("serialize TOML: {e}")))?;
    std::fs::write(config_path, out).map_err(ChatmailError::from)?;
    Ok(())
}

fn update_maddy_autocert(config_path: &Path, acme_email: &str) -> Result<()> {
    let data = std::fs::read_to_string(config_path)?;
    let lines: Vec<&str> = data.lines().collect();
    let mut new_lines: Vec<String> = Vec::new();
    let mut tls_mode_set = false;
    let mut acme_email_set = false;
    let mut comment_updated = false;
    let mut insert_after: Option<usize> = None;

    for line in lines {
        let trimmed = line.trim();

        if trimmed.starts_with("# TLS certificate paths (mode:") {
            new_lines.push("# TLS certificate paths (mode: autocert)".to_string());
            comment_updated = true;
            insert_after = Some(new_lines.len());
            continue;
        }

        if trimmed.starts_with("tls_mode ") {
            new_lines.push("tls_mode autocert".to_string());
            tls_mode_set = true;
            insert_after = Some(new_lines.len());
            continue;
        }

        if trimmed.starts_with("acme_email ") {
            new_lines.push(format!("acme_email {acme_email}"));
            acme_email_set = true;
            continue;
        }

        new_lines.push(line.to_string());

        if trimmed.starts_with("tls file ") && insert_after.is_none() {
            insert_after = Some(new_lines.len());
        }
    }

    if !tls_mode_set || !acme_email_set {
        let pos = insert_after.unwrap_or(new_lines.len());
        let mut inserts = Vec::new();
        if !comment_updated {
            inserts.push("# TLS certificate paths (mode: autocert)".to_string());
        }
        if !tls_mode_set {
            inserts.push("tls_mode autocert".to_string());
        }
        if !acme_email_set {
            inserts.push(format!("acme_email {acme_email}"));
        }
        for (offset, line) in inserts.into_iter().enumerate() {
            new_lines.insert(pos + offset, line);
        }
    }

    std::fs::write(config_path, new_lines.join("\n") + "\n").map_err(ChatmailError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maddy_conf_autocert_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("maddy.conf");
        std::fs::write(
            &path,
            r#"$(primary_domain) = example.org
state_dir /var/lib/madmail

# TLS certificate paths (mode: file)
tls file /var/lib/madmail/certs/fullchain.pem /var/lib/madmail/certs/privkey.pem
"#,
        )
        .unwrap();
        update_config_autocert(&path, "admin@example.org").unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("tls_mode autocert"));
        assert!(body.contains("acme_email admin@example.org"));
        assert!(body.contains("# TLS certificate paths (mode: autocert)"));
        assert!(body.contains("tls file "));
    }

    #[test]
    fn toml_autocert_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chatmail.toml");
        std::fs::write(&path, "primary_domain = \"example.org\"\n").unwrap();
        update_config_autocert(&path, "ops@example.org").unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("tls_mode = \"autocert\""));
        assert!(body.contains("acme_email = \"ops@example.org\""));
    }
}
