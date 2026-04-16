// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

// device_add uses 'gen': false in the QAPI schema, accepting arbitrary
// additional properties via #[serde(flatten)].  We define a raw struct
// so we can send an untyped JSON object directly.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct raw_device_add {
    #[serde(flatten)]
    pub args: serde_json::Value,
}

impl qapi_qmp::QmpCommand for raw_device_add {}
impl qapi::Command for raw_device_add {
    const NAME: &'static str = "device_add";
    const ALLOW_OOB: bool = false;
    type Ok = qapi::Empty;
}

/// Parse a QEMU keyval string into a JSON object.
///
/// The format is: `driver[,key=value,...]`
/// where the first positional value (no `=`) is assigned to `implied_key`.
fn parse_keyval(input: &str, implied_key: &str) -> Result<serde_json::Value, String> {
    let mut map = serde_json::Map::new();

    for (i, part) in input.split(',').enumerate() {
        if part.is_empty() {
            continue;
        }
        if let Some((key, value)) = part.split_once('=') {
            map.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        } else if i == 0 {
            // First positional value → implied key
            map.insert(
                implied_key.to_string(),
                serde_json::Value::String(part.to_string()),
            );
        } else {
            return Err(format!("unexpected positional value: '{part}'"));
        }
    }

    Ok(serde_json::Value::Object(map))
}

pub async fn cmd_device_add(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let spec = require_str(args, "device")?;
    let obj = parse_keyval(&spec, "driver")
        .map_err(|e| CmdError::Command(format!("invalid device spec: {e}")))?;
    conn.execute(raw_device_add { args: obj })
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
