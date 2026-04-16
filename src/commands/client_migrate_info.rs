// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_client_migrate_info(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let protocol = require_str(args, "protocol")?;
    let hostname = require_str(args, "hostname")?;
    let port = match args.get("port") {
        Some(ArgValue::Int(n)) => Some(*n),
        _ => None,
    };
    let tls_port = match args.get("tls-port") {
        Some(ArgValue::Int(n)) => Some(*n),
        _ => None,
    };
    let cert_subject = match args.get("cert-subject") {
        Some(ArgValue::Str(s)) => Some(s.clone()),
        _ => None,
    };

    conn.execute(qapi_qmp::client_migrate_info {
        protocol,
        hostname,
        port,
        tls_port,
        cert_subject,
    })
    .await
    .map_err(CmdError::from)?;

    Ok(String::new())
}
