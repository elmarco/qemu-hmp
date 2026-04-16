// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_int, require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_block_resize(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let device = require_str(args, "device")?;
    let size = require_int(args, "size")?;
    conn.execute(qapi::qmp::block_resize {
        device: Some(device),
        node_name: None,
        size,
    })
    .await
    .map_err(CmdError::from)?;
    Ok(String::new())
}
