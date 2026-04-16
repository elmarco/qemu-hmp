// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

type ChardevOpts = (
    String,
    Option<String>,
    serde_json::Map<String, serde_json::Value>,
);

// chardev-add takes a ChardevBackend union with 22 variants,
// each with its own Wrapper type.  Rather than mapping every
// HMP opts key to the QMP field names, we send a raw JSON object
// in the {"type": ..., "data": {...}} format that QEMU expects.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct raw_chardev_add {
    pub id: String,
    pub backend: serde_json::Value,
}

impl qapi_qmp::QmpCommand for raw_chardev_add {}
impl qapi::Command for raw_chardev_add {
    const NAME: &'static str = "chardev-add";
    const ALLOW_OOB: bool = false;
    type Ok = qapi_qmp::ChardevReturn;
}

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct raw_chardev_change {
    pub id: String,
    pub backend: serde_json::Value,
}

impl qapi_qmp::QmpCommand for raw_chardev_change {}
impl qapi::Command for raw_chardev_change {
    const NAME: &'static str = "chardev-change";
    const ALLOW_OOB: bool = false;
    type Ok = qapi_qmp::ChardevReturn;
}

/// Parse a chardev HMP opts string into a backend type, id, and
/// remaining key-value pairs.
///
/// Format: `type[,id=<id>][,key=value,...]`
///
/// The first positional value (before any `=`) is the backend type.
fn parse_chardev_opts(input: &str) -> Result<ChardevOpts, String> {
    let mut backend_type = None;
    let mut id = None;
    let mut data = serde_json::Map::new();

    for (i, part) in input.split(',').enumerate() {
        if part.is_empty() {
            continue;
        }
        if let Some((key, value)) = part.split_once('=') {
            if key == "id" {
                id = Some(value.to_string());
            } else if key == "backend" {
                backend_type = Some(value.to_string());
            } else {
                // Convert on/off to booleans for QMP
                let json_val = match value {
                    "on" => serde_json::Value::Bool(true),
                    "off" => serde_json::Value::Bool(false),
                    _ => serde_json::Value::String(value.to_string()),
                };
                data.insert(key.to_string(), json_val);
            }
        } else if i == 0 {
            backend_type = Some(part.to_string());
        } else {
            return Err(format!("unexpected positional value: '{part}'"));
        }
    }

    let backend_type = backend_type.ok_or_else(|| "missing backend type".to_string())?;

    Ok((backend_type, id, data))
}

/// Build the backend JSON in the {"type": ..., "data": {...}} format.
fn build_backend(
    backend_type: &str,
    data: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut backend = serde_json::Map::new();
    backend.insert(
        "type".to_string(),
        serde_json::Value::String(backend_type.to_string()),
    );
    backend.insert("data".to_string(), serde_json::Value::Object(data));
    serde_json::Value::Object(backend)
}

pub async fn cmd_chardev_add(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let spec = require_str(args, "args")?;

    let (backend_type, id, data) = parse_chardev_opts(&spec)
        .map_err(|e| CmdError::Command(format!("Parsing chardev args failed: {e}")))?;

    let id =
        id.ok_or_else(|| CmdError::Command("Parsing chardev args failed: missing id".to_string()))?;

    let backend = build_backend(&backend_type, data);

    conn.execute(raw_chardev_add { id, backend })
        .await
        .map_err(CmdError::from)?;

    Ok(String::new())
}

pub async fn cmd_chardev_change(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let id = require_str(args, "id")?;
    let spec = require_str(args, "args")?;

    let (backend_type, opts_id, data) = parse_chardev_opts(&spec)
        .map_err(|e| CmdError::Command(format!("Parsing chardev args failed: {e}")))?;

    if opts_id.is_some() {
        return Err(CmdError::Command("Unexpected 'id' parameter".to_string()));
    }

    let backend = build_backend(&backend_type, data);

    conn.execute(raw_chardev_change { id, backend })
        .await
        .map_err(CmdError::from)?;

    Ok(String::new())
}
