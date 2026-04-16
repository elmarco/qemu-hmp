// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_chardev(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let chardevs = conn
        .execute(qapi::qmp::query_chardev {})
        .await
        .map_err(CmdError::from)?;
    let mut lines = Vec::new();
    for cd in &chardevs {
        lines.push(format!("{}: filename={}", cd.label, cd.filename));
    }
    Ok(lines.join("\n"))
}
