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

//! Machine-readable JSON output for operator CLI commands (`--json`).

use chatmail_config::Args;
use chatmail_types::{ChatmailError, Result};
use serde::Serialize;
use serde_json::Value;

/// JSON error envelope written to stderr on CLI failure.
#[derive(Serialize)]
pub struct CtlErrorResponse {
    pub ok: bool,
    pub error: String,
}

#[derive(Serialize)]
struct CtlSuccessResponse<T: Serialize> {
    ok: bool,
    command: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    data: T,
}

/// Per-command output helper (human text vs JSON envelope).
#[derive(Debug, Clone)]
pub struct CtlOut {
    command: &'static str,
    json: bool,
}

impl CtlOut {
    pub fn from_args(args: &Args, command: &'static str) -> Self {
        Self {
            command,
            json: args.json,
        }
    }

    pub fn is_json(&self) -> bool {
        self.json
    }

    /// Print a human-only line (skipped in `--json` mode).
    pub fn line(&self, text: impl AsRef<str>) {
        if !self.json {
            println!("{}", text.as_ref());
        }
    }

    /// Print a human-only blank line.
    pub fn blank(&self) {
        if !self.json {
            println!();
        }
    }

    /// Emit the standard success envelope to stdout (`--json` only).
    pub fn emit<T: Serialize>(&self, data: T) -> Result<()> {
        if !self.json {
            return Ok(());
        }
        self.write_envelope(data, None)
    }

    /// Human text or JSON envelope depending on mode.
    pub fn done<T: Serialize>(&self, human: impl AsRef<str>, data: T) -> Result<()> {
        if self.json {
            self.emit(data)
        } else {
            println!("{}", human.as_ref());
            Ok(())
        }
    }

    /// Human text or JSON envelope with a `message` field.
    pub fn done_msg<T: Serialize>(
        &self,
        human: impl AsRef<str>,
        data: T,
        message: impl Into<String>,
    ) -> Result<()> {
        if self.json {
            self.write_envelope(data, Some(message.into()))
        } else {
            println!("{}", human.as_ref());
            Ok(())
        }
    }

    /// Confirmation cancelled — human "Aborted." or JSON with message.
    pub fn aborted(&self) -> Result<()> {
        if self.json {
            self.write_envelope(Value::Object(Default::default()), Some("aborted".into()))
        } else {
            println!("Aborted.");
            Ok(())
        }
    }

    fn write_envelope<T: Serialize>(&self, data: T, message: Option<String>) -> Result<()> {
        let envelope = CtlSuccessResponse {
            ok: true,
            command: self.command,
            message,
            data,
        };
        let body = serde_json::to_string(&envelope)
            .map_err(|e| ChatmailError::config(format!("JSON output: {e}")))?;
        println!("{body}");
        Ok(())
    }
}

/// Print `{"ok":false,"error":"..."}` to stderr (used by `main` on CLI errors).
pub fn print_error_json(error: &str) {
    let resp = CtlErrorResponse {
        ok: false,
        error: error.to_string(),
    };
    if let Ok(body) = serde_json::to_string(&resp) {
        eprintln!("{body}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chatmail_config::Cli;
    use clap::Parser;

    fn json_args() -> Args {
        Cli::try_parse_from(["chatmail", "--json"])
            .expect("parse")
            .args
    }

    #[test]
    fn ctl_out_emits_standard_envelope() {
        let out = CtlOut::from_args(&json_args(), "accounts status");
        out.emit(serde_json::json!({ "login_count": 3 }))
            .expect("emit");
        // stdout capture is awkward here; verify serialization shape directly.
        let envelope = CtlSuccessResponse {
            ok: true,
            command: "accounts status",
            message: None,
            data: serde_json::json!({ "login_count": 3 }),
        };
        let s = serde_json::to_string(&envelope).unwrap();
        assert!(s.contains("\"ok\":true"));
        assert!(s.contains("\"command\":\"accounts status\""));
        assert!(s.contains("\"login_count\":3"));
    }

    #[test]
    fn print_error_json_shape() {
        let resp = CtlErrorResponse {
            ok: false,
            error: "bad".into(),
        };
        let s = serde_json::to_string(&resp).unwrap();
        assert_eq!(s, r#"{"ok":false,"error":"bad"}"#);
    }
}
