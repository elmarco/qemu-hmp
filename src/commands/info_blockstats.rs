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
        let idle = s.idle_time_ns.unwrap_or(0);
        lines.push(format!(
            "{device}: rd_bytes={} wr_bytes={} rd_operations={} wr_operations={} \
             flush_operations={} wr_total_time_ns={} rd_total_time_ns={} \
             flush_total_time_ns={} rd_merged={} wr_merged={} idle_time_ns={idle}",
            s.rd_bytes,
            s.wr_bytes,
            s.rd_operations,
            s.wr_operations,
            s.flush_operations,
            s.wr_total_time_ns,
            s.rd_total_time_ns,
            s.flush_total_time_ns,
            s.rd_merged,
            s.wr_merged
        ));
    }
    Ok(lines.join("\n"))
}
