// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_memory_size_summary(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let info = conn
        .execute(qapi_qmp::query_memory_size_summary {})
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    writeln!(out, "base memory: {}", info.base_memory).unwrap();
    if let Some(plugged) = info.plugged_memory {
        writeln!(out, "plugged memory: {plugged}").unwrap();
    }

    Ok(out)
}
