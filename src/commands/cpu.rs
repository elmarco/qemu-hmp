// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::info_cpus::cpu_info_base;
use crate::commands::{require_int, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_cpu(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let index = require_int(args, "index")?;

    // Validate that the CPU index exists, matching the C handler's
    // monitor_set_cpu() which calls qemu_get_cpu().
    let cpus = conn
        .execute(qapi::qmp::query_cpus_fast {})
        .await
        .map_err(CmdError::from)?;

    let valid = cpus.iter().any(|cpu| cpu_info_base(cpu).cpu_index == index);
    if !valid {
        return Ok("invalid CPU index".to_string());
    }

    conn.set_cpu_index(index);
    Ok(String::new())
}
