// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_screendump(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let filename = require_str(args, "filename")?;
    let format = match args.get("format") {
        Some(ArgValue::Str(s)) => Some(s.parse().map_err(|()| {
            CmdError::Command(format!("invalid format '{s}': expected ppm or png"))
        })?),
        _ => None,
    };
    let device = match args.get("device") {
        Some(ArgValue::Str(s)) => Some(s.clone()),
        _ => None,
    };
    let head = match args.get("head") {
        Some(ArgValue::Int(n)) => Some(*n),
        _ => None,
    };

    conn.execute(qapi::qmp::screendump {
        filename: filename.clone(),
        device,
        head,
        format,
    })
    .await
    .map_err(CmdError::from)?;

    crate::terminal_image::display_image_inline(&filename);

    Ok(String::new())
}
