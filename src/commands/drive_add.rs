// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::{opt_bool, require_str, CmdError};
use crate::qmp::QmpConnection;

// blockdev-add takes a boxed BlockdevOptions union with many driver
// variants.  Rather than using the generated type, we send the
// arguments as raw JSON using serde(flatten), matching the pattern
// used by device_add and object_add.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct raw_blockdev_add {
    #[serde(flatten)]
    pub args: serde_json::Value,
}

impl qapi_qmp::QmpCommand for raw_blockdev_add {}
impl qapi::Command for raw_blockdev_add {
    const NAME: &'static str = "blockdev-add";
    const ALLOW_OOB: bool = false;
    type Ok = qapi::Empty;
}

/// Parse a QEMU keyval string into a JSON object.
///
/// The format is: `key=value,key=value,...`
fn parse_keyval(input: &str) -> Result<serde_json::Value, String> {
    let mut map = serde_json::Map::new();

    for part in input.split(',') {
        if part.is_empty() {
            continue;
        }
        if let Some((key, value)) = part.split_once('=') {
            map.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        } else {
            return Err(format!("expected key=value, got: '{part}'"));
        }
    }

    Ok(serde_json::Value::Object(map))
}

/// Add a block device node.
///
/// The `-n` (node) mode is supported, mapping to the `blockdev-add` QMP
/// command.  The legacy mode (without `-n`) relies on QEMU-internal APIs
/// (`drive_new`) that have no QMP equivalent, so it is not supported in
/// the external HMP.
pub async fn cmd_drive_add(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let node = opt_bool(args, "node");
    let opts = require_str(args, "opts")?;

    if !node {
        return Err(CmdError::Command(
            "legacy drive_add (without -n) has no QMP equivalent; \
             use 'drive_add -n' with blockdev options instead"
                .to_string(),
        ));
    }

    let obj = parse_keyval(&opts)
        .map_err(|e| CmdError::Command(format!("invalid drive options: {e}")))?;

    // Match the built-in HMP check: node-name must be specified.
    match obj.get("node-name").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => {}
        _ => {
            return Err(CmdError::Command(
                "'node-name' needs to be specified".to_string(),
            ));
        }
    }

    conn.execute(raw_blockdev_add { args: obj })
        .await
        .map_err(CmdError::from)?;

    Ok(String::new())
}
