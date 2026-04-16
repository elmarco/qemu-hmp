// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use base64::prelude::*;
use serde::{Deserialize, Serialize};

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

// query-firmware-log is a new QMP command (Since: 10.2) not yet in the
// qapi-rs crate.  Define the command and response structs manually.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
struct query_firmware_log {
    #[serde(rename = "max-size", skip_serializing_if = "Option::is_none")]
    max_size: Option<u64>,
}

impl qapi_qmp::QmpCommand for query_firmware_log {}
impl qapi::Command for query_firmware_log {
    const NAME: &'static str = "query-firmware-log";
    const ALLOW_OOB: bool = false;
    type Ok = FirmwareLog;
}

#[derive(Debug, Deserialize)]
struct FirmwareLog {
    #[serde(default)]
    version: Option<String>,
    log: String,
}

/// Escape a byte slice like GLib's g_strescape(), preserving bytes
/// listed in `exceptions` as literal characters.
fn g_strescape(data: &[u8], exceptions: &[u8]) -> String {
    let mut out = String::new();
    for &b in data {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            b'\x08' => out.push_str("\\b"),
            b'\x0c' => out.push_str("\\f"),
            b'\n' => {
                if exceptions.contains(&b'\n') {
                    out.push('\n');
                } else {
                    out.push_str("\\n");
                }
            }
            b'\r' => {
                if exceptions.contains(&b'\r') {
                    out.push('\r');
                } else {
                    out.push_str("\\r");
                }
            }
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(b as char),
            _ => {
                // Octal escape for control characters and high bytes
                out.push_str(&format!("\\{:03o}", b));
            }
        }
    }
    out
}

pub async fn cmd_info_firmware_log(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let max_size = match args.get("max-size") {
        Some(ArgValue::Int(n)) => {
            if *n < 0 {
                None
            } else {
                Some(*n as u64)
            }
        }
        _ => None,
    };

    let log = conn
        .execute(query_firmware_log { max_size })
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();

    if let Some(ref version) = log.version {
        let esc = g_strescape(version.as_bytes(), &[]);
        out.push_str(&format!("[ firmware version: {esc} ]\n"));
    }

    let log_bytes = BASE64_STANDARD
        .decode(&log.log)
        .map_err(|e| CmdError::Command(format!("base64 decode error: {e}")))?;
    let log_esc = g_strescape(&log_bytes, b"\r\n");
    out.push_str(&log_esc);
    out.push('\n');

    Ok(out)
}
