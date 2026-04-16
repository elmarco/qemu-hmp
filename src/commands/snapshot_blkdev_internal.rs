// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use qapi_qmp::BlockdevSnapshotInternal;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_snapshot_blkdev_internal(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let device = require_str(args, "device")?;
    let name = require_str(args, "name")?;

    conn.execute(qapi_qmp::blockdev_snapshot_internal_sync(
        BlockdevSnapshotInternal { device, name },
    ))
    .await
    .map_err(CmdError::from)?;

    Ok(String::new())
}
