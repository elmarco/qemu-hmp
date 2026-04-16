// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::format::format_size;
use crate::qmp::QmpConnection;

pub async fn cmd_info_migrate(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let info = conn
        .execute(qapi::qmp::query_migrate {})
        .await
        .map_err(CmdError::from)?;

    let mut lines = Vec::new();

    if let Some(ref status) = info.status {
        lines.push(format!("Migration status: {}", status.name()));
    }

    if let Some(ref ram) = info.ram {
        lines.push(format!("transferred ram: {}", format_size(ram.transferred)));
        lines.push(format!("total ram: {}", format_size(ram.total)));
        lines.push(format!("remaining ram: {}", format_size(ram.remaining)));
    }

    Ok(lines.join("\n"))
}
