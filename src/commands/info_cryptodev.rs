// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_cryptodev(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let list = conn
        .execute(qapi_qmp::query_cryptodev {})
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    for info in &list {
        let services: Vec<&str> = info.service.iter().map(|s| s.name()).collect();
        let services = services.join("|");
        writeln!(out, "{}: service=[{services}]", info.id).unwrap();

        for client in &info.client {
            writeln!(
                out,
                "    queue {}: type={}",
                client.queue,
                client.type_.name()
            )
            .unwrap();
        }
    }

    Ok(out)
}
