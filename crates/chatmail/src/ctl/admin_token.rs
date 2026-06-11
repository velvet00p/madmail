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

use crate::admin::resolve_admin_token;
use chatmail_config::Args;
use chatmail_types::Result;

use super::admin_login_qr::{
    build_admin_login_qr_url, login_qr_scan_payload, print_login_qr_terminal,
};
use super::admin_url::build_admin_url;
use super::context::CtlContext;
use super::output::CtlOut;

/// Display admin API credentials (Madmail `maddy admin-token`).
pub async fn admin_token(args: &Args, raw: bool, no_qr: bool) -> Result<()> {
    let ctx = CtlContext::from_args(args)?;
    ctx.require_db()?;
    let out = CtlOut::from_args(args, "admin-token");

    let token = resolve_admin_token(&ctx.state_dir, &ctx.config)?;
    let settings = ctx.load_settings_map().await?;
    let api_url = build_admin_url(&ctx.config, &settings);

    if raw && !args.json {
        print!("{token}");
        return Ok(());
    }

    if out.is_json() {
        return out.emit(serde_json::json!({
            "token": token,
            "api_url": api_url,
        }));
    }

    out.blank();
    out.line(format!("  Admin API URL:   {api_url}"));
    out.line(format!("  Admin Token:     {token}"));
    out.blank();

    if !no_qr {
        let login_url = build_admin_login_qr_url(&api_url, &token);
        let scan_payload = login_qr_scan_payload(&api_url, &token);
        out.line("  Scan with Madmail Admin (admin.madmail.chat):");
        print_login_qr_terminal(&scan_payload)?;
        out.blank();
        out.line(format!("  Or open: {login_url}"));
        out.blank();
    }

    Ok(())
}
