// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use qapi_qmp::{BackupCommon, DriveBackup, MirrorSyncMode, NewImageMode};

use crate::args::ArgValue;
use crate::commands::{opt_bool, require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_drive_backup(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let device = require_str(args, "device")?;
    let target = require_str(args, "target")?;
    let reuse = opt_bool(args, "reuse");
    let full = opt_bool(args, "full");
    let compress = opt_bool(args, "compress");

    let format = match args.get("format") {
        Some(ArgValue::Str(s)) => Some(s.clone()),
        _ => None,
    };

    let sync = if full {
        MirrorSyncMode::full
    } else {
        MirrorSyncMode::top
    };

    let mode = if reuse {
        NewImageMode::existing
    } else {
        NewImageMode::absolute_paths
    };

    #[allow(deprecated)]
    conn.execute(qapi_qmp::drive_backup(DriveBackup {
        base: BackupCommon {
            device,
            sync,
            compress: if compress { Some(true) } else { None },
            auto_dismiss: None,
            auto_finalize: None,
            bitmap: None,
            bitmap_mode: None,
            discard_source: None,
            filter_node_name: None,
            job_id: None,
            on_source_error: None,
            on_target_error: None,
            speed: None,
            x_perf: None,
        },
        target,
        format,
        mode: Some(mode),
    }))
    .await
    .map_err(CmdError::from)?;

    Ok(String::new())
}
