// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_int, require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_ringbuf_write(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let device = require_str(args, "device")?;
    let data = require_str(args, "data")?;

    conn.execute(qapi_qmp::ringbuf_write {
        device,
        data,
        format: None,
    })
    .await
    .map_err(CmdError::from)?;

    Ok(String::new())
}

pub async fn cmd_ringbuf_read(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let device = require_str(args, "device")?;
    let size = require_int(args, "size")?;

    let data = conn
        .execute(qapi_qmp::ringbuf_read {
            device,
            size,
            format: None,
        })
        .await
        .map_err(CmdError::from)?;

    // Format output exactly like the C handler: escape backslashes and
    // non-printable characters, then append a newline.
    let mut output = String::new();
    for ch in data.bytes() {
        if ch == b'\\' {
            output.push_str("\\\\");
        } else if (ch < 0x20 && ch != b'\n' && ch != b'\t') || ch == 0x7F {
            output.push_str(&format!("\\u{:04X}", ch));
        } else {
            output.push(ch as char);
        }
    }
    output.push('\n');

    Ok(output)
}
