// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_dump(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let result = conn
        .execute(qapi_qmp::query_dump {})
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    writeln!(out, "Status: {}", result.status.name()).unwrap();

    if matches!(result.status, qapi_qmp::DumpStatus::active) {
        let percent = 100.0 * result.completed as f64 / result.total as f64;
        writeln!(out, "Finished: {percent:.2} %").unwrap();
    }

    Ok(out)
}
