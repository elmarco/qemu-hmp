// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use qapi_qmp::{BlockdevSnapshotSync, NewImageMode};

use crate::args::ArgValue;
use crate::commands::{opt_bool, require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_snapshot_blkdev(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let device = require_str(args, "device")?;
    let reuse = opt_bool(args, "reuse");

    let snapshot_file = match args.get("snapshot-file") {
        Some(ArgValue::Str(s)) => s.clone(),
        _ => {
            return Err(CmdError::Command(
                "Parameter 'snapshot-file' is missing".to_string(),
            ));
        }
    };

    let format = match args.get("format") {
        Some(ArgValue::Str(s)) => Some(s.clone()),
        _ => None,
    };

    let mode = if reuse {
        NewImageMode::existing
    } else {
        NewImageMode::absolute_paths
    };

    conn.execute(qapi_qmp::blockdev_snapshot_sync(BlockdevSnapshotSync {
        device: Some(device),
        node_name: None,
        snapshot_file,
        snapshot_node_name: None,
        format,
        mode: Some(mode),
    }))
    .await
    .map_err(CmdError::from)?;

    Ok(String::new())
}
