// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

/// Delete a block device.
///
/// The built-in HMP handler has two code paths: it first tries to look
/// up the id as a node name and calls `blockdev-del`, then falls back
/// to legacy `blk_by_name()` cleanup.  The legacy path has no QMP
/// equivalent, so the external HMP only supports the `blockdev-del`
/// path (i.e. devices added via `blockdev-add`).
pub async fn cmd_drive_del(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let id = require_str(args, "id")?;
    conn.execute(qapi::qmp::blockdev_del { node_name: id })
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
