// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_snapshot_delete_blkdev_internal(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let device = require_str(args, "device")?;
    let name = require_str(args, "name")?;
    let id = match args.get("id") {
        Some(ArgValue::Str(s)) => Some(s.clone()),
        _ => None,
    };

    conn.execute(qapi_qmp::blockdev_snapshot_delete_internal_sync {
        device,
        id,
        name: Some(name),
    })
    .await
    .map_err(CmdError::from)?;

    Ok(String::new())
}
