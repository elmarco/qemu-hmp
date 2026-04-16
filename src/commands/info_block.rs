// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_block(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let blocks = conn
        .execute(qapi::qmp::query_block {})
        .await
        .map_err(CmdError::from)?;
    let mut entries = Vec::new();
    for blk in &blocks {
        let mut lines = Vec::new();

        // First line: device name, optional node-name, and inserted info or [not inserted]
        let mut header = blk.device.clone();
        if let Some(ref ins) = blk.inserted {
            // Add node-name in parens if present: "snap0 (#block196)"
            if let Some(ref node_name) = ins.node_name {
                header.push_str(&format!(" ({node_name})"));
            }
            header.push_str(&format!(": {} ({})", ins.file, ins.drv));
            if ins.ro {
                header.push_str(" [read-only]");
            }
            if ins.encrypted {
                header.push_str(" [encrypted]");
            }
        } else {
            header.push_str(": [not inserted]");
        }
        lines.push(header);

        // "Attached to:" line from qdev field
        if let Some(ref qdev) = blk.qdev {
            if !qdev.is_empty() {
                lines.push(format!("    Attached to:      {}", qdev));
            }
        }

        // "Removable device:" line for removable devices
        if blk.removable {
            let lock_state = if blk.locked { "locked" } else { "not locked" };
            let tray_state = if blk.tray_open.unwrap_or(false) {
                "tray open"
            } else {
                "tray closed"
            };
            lines.push(format!(
                "    Removable device: {}, {}",
                lock_state, tray_state
            ));
        }

        // "Cache mode:" line when inserted
        if let Some(ref ins) = blk.inserted {
            let cache_mode = if ins.cache.writeback {
                "writeback"
            } else {
                "writethrough"
            };
            lines.push(format!("    Cache mode:       {}", cache_mode));

            if ins.backing_file_depth > 0 {
                if let Some(ref backing) = ins.backing_file {
                    lines.push(format!(
                        "    Backing file:     {} (chain depth: {})",
                        backing, ins.backing_file_depth
                    ));
                }
            }
        }

        entries.push(lines.join("\n"));
    }
    Ok(entries.join("\n\n"))
}
