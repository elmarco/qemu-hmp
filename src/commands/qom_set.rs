// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde_json::Value;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_qom_set(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let path = require_str(args, "path")?;
    let property = require_str(args, "property")?;
    let value_str = require_str(args, "value")?;

    // Parse value as JSON; fall back to treating it as a plain string.
    let value: Value = serde_json::from_str(&value_str).unwrap_or(Value::String(value_str));

    conn.execute(qapi::qmp::qom_set {
        path,
        property,
        value,
    })
    .await
    .map_err(CmdError::from)?;
    Ok(String::new())
}
