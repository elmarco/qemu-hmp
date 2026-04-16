// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_migrate_capabilities(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let caps = conn
        .execute(qapi_qmp::query_migrate_capabilities {})
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    for cap in &caps {
        writeln!(
            out,
            "{}: {}",
            cap.capability.name(),
            if cap.state { "on" } else { "off" }
        )
        .unwrap();
    }

    Ok(out)
}
