// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_vcpu_dirty_limit(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let list = conn
        .execute(qapi_qmp::query_vcpu_dirty_limit {})
        .await
        .map_err(CmdError::from)?;

    if list.is_empty() {
        return Ok("Dirty page limit not enabled!\n".to_string());
    }

    let mut out = String::new();
    for info in &list {
        // C uses PRIi64 for all three fields
        writeln!(
            out,
            "vcpu[{}], limit rate {} (MB/s), current rate {} (MB/s)",
            info.cpu_index, info.limit_rate as i64, info.current_rate as i64,
        )
        .unwrap();
    }

    Ok(out)
}
