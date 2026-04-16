// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{opt_bool, require_str, CmdError};
use crate::qmp::QmpConnection;

/// Parse a read-only-mode string into the corresponding enum value.
fn parse_read_only_mode(s: &str) -> Result<qapi::qmp::BlockdevChangeReadOnlyMode, CmdError> {
    s.parse().map_err(|()| {
        CmdError::Command(format!(
            "invalid read-only-mode '{s}': expected retain, read-only, or read-write"
        ))
    })
}

pub async fn cmd_change(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let device = require_str(args, "device")?;
    let force = opt_bool(args, "force");
    let target = require_str(args, "target")?;
    let arg = match args.get("arg") {
        Some(ArgValue::Str(s)) => Some(s.clone()),
        _ => None,
    };
    let read_only = match args.get("read-only-mode") {
        Some(ArgValue::Str(s)) => Some(s.clone()),
        _ => None,
    };

    if device == "vnc" {
        // VNC password change: "change vnc password <password>"
        if target != "passwd" && target != "password" {
            return Err(CmdError::Command(
                "Expected 'password' after 'vnc'".to_string(),
            ));
        }
        if read_only.is_some() {
            return Err(CmdError::Command(
                "Parameter 'read-only-mode' is invalid for VNC".to_string(),
            ));
        }
        let Some(password) = arg else {
            return Err(CmdError::Command(
                "VNC password must be provided as an argument in the external HMP".to_string(),
            ));
        };
        conn.execute(qapi::qmp::change_vnc_password { password })
            .await
            .map_err(CmdError::from)?;
    } else {
        // Block device medium change.
        let read_only_mode = match read_only {
            Some(ref s) => Some(parse_read_only_mode(s)?),
            None => None,
        };
        #[allow(deprecated)]
        conn.execute(qapi::qmp::blockdev_change_medium {
            device: Some(device),
            id: None,
            filename: target,
            format: arg,
            force: Some(force),
            read_only_mode,
        })
        .await
        .map_err(CmdError::from)?;
    }

    Ok(String::new())
}
