// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

// object-add uses a boxed ObjectOptions union with 60+ variants.
// Rather than enumerating them all, we send the arguments as raw JSON
// using serde(flatten) on a serde_json::Value.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct raw_object_add {
    #[serde(flatten)]
    pub args: serde_json::Value,
}

impl qapi_qmp::QmpCommand for raw_object_add {}
impl qapi::Command for raw_object_add {
    const NAME: &'static str = "object-add";
    const ALLOW_OOB: bool = false;
    type Ok = serde_json::Value;
}

/// Parse a QEMU keyval string into a JSON object.
///
/// The format is: `type,key=value,key=value,...`
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

pub async fn cmd_object_add(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let spec = require_str(args, "object")?;
    let obj = parse_keyval(&spec, "qom-type")
        .map_err(|e| CmdError::Command(format!("invalid object spec: {e}")))?;
    conn.execute(raw_object_add { args: obj })
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
