// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

fn fmt_basic_info(name: &str, info: &qapi::qmp::VncBasicInfo) -> String {
    format!(
        "  {}: {}:{} ({}{})",
        name,
        info.host,
        info.service,
        info.family.name(),
        if info.websocket { " (Websocket)" } else { "" },
    )
}

fn fmt_authcrypt(
    indent: &str,
    auth: &qapi::qmp::VncPrimaryAuth,
    vencrypt: Option<&qapi::qmp::VncVencryptSubAuth>,
) -> String {
    format!(
        "{}Auth: {} (Sub: {})",
        indent,
        auth.name(),
        vencrypt.map_or("none", |v| v.name()),
    )
}

pub async fn cmd_info_vnc(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let servers = conn
        .execute(qapi::qmp::query_vnc_servers {})
        .await
        .map_err(CmdError::from)?;

    if servers.is_empty() {
        return Ok("None".to_string());
    }

    let mut lines = Vec::new();
    for info in &servers {
        lines.push(format!("{}:", info.id));

        // Servers
        for sinfo in &info.server {
            lines.push(fmt_basic_info("Server", &sinfo.base));
            lines.push(fmt_authcrypt("    ", &sinfo.auth, sinfo.vencrypt.as_ref()));
        }

        // Clients
        for cinfo in &info.clients {
            lines.push(fmt_basic_info("Client", &cinfo.base));
            lines.push(format!(
                "    x509_dname: {}",
                cinfo.x509_dname.as_deref().unwrap_or("none")
            ));
            lines.push(format!(
                "    sasl_username: {}",
                cinfo.sasl_username.as_deref().unwrap_or("none")
            ));
        }

        // Auth info for reverse connections (no server entries)
        if info.server.is_empty() {
            lines.push(fmt_authcrypt("  ", &info.auth, info.vencrypt.as_ref()));
        }

        if let Some(ref display) = info.display {
            lines.push(format!("  Display: {display}"));
        }
    }

    Ok(lines.join("\n"))
}
