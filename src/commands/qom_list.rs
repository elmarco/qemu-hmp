// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_qom_list(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let path = match args.get("path") {
        Some(ArgValue::Str(s)) => s.clone(),
        _ => return Ok("/".to_string()),
    };
    let props = conn
        .execute(qapi::qmp::qom_list { path })
        .await
        .map_err(CmdError::from)?;
    let mut lines = Vec::new();
    for p in &props {
        lines.push(format!("{} ({})", p.name, p.type_));
    }
    Ok(lines.join("\n"))
}
