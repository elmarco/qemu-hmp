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
    let mut lines = Vec::new();
    for blk in &blocks {
        let mut desc = blk.device.clone();
        if let Some(ref ins) = blk.inserted {
            desc.push_str(&format!(": {}({})", ins.file, ins.drv));
            if ins.ro {
                desc.push_str(" [read-only]");
            }
        } else {
            desc.push_str(": (not inserted)");
        }
        if blk.locked {
            desc.push_str(" [locked]");
        }
        lines.push(desc);
    }
    Ok(lines.join("\n"))
}
