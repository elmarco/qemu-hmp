// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_blockstats(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let stats = conn
        .execute(qapi::qmp::query_blockstats { query_nodes: None })
        .await
        .map_err(CmdError::from)?;
    let mut lines = Vec::new();
    for entry in &stats {
        let device = entry
            .device
            .as_deref()
            .or(entry.node_name.as_deref())
            .unwrap_or("(unknown)");
        let s = &entry.stats;
        lines.push(format!(
            "{}: rd_bytes={} wr_bytes={} rd_operations={} wr_operations={}",
            device, s.rd_bytes, s.wr_bytes, s.rd_operations, s.wr_operations
        ));
    }
    Ok(lines.join("\n"))
}
