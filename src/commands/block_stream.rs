// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_block_stream(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let device = require_str(args, "device")?;
    let speed = match args.get("speed") {
        Some(ArgValue::Int(n)) => Some(*n),
        _ => None,
    };
    let base = match args.get("base") {
        Some(ArgValue::Str(s)) => Some(s.clone()),
        _ => None,
    };

    conn.execute(qapi::qmp::block_stream {
        device: device.clone(),
        job_id: Some(device),
        base,
        base_node: None,
        backing_file: None,
        backing_mask_protocol: None,
        bottom: None,
        speed,
        on_error: Some(qapi::qmp::BlockdevOnError::report),
        filter_node_name: None,
        auto_finalize: None,
        auto_dismiss: None,
    })
    .await
    .map_err(CmdError::from)?;
    Ok(String::new())
}
