// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use qapi_qmp::{DriveMirror, MirrorSyncMode, NewImageMode};

use crate::args::ArgValue;
use crate::commands::{opt_bool, require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_drive_mirror(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let device = require_str(args, "device")?;
    let target = require_str(args, "target")?;
    let reuse = opt_bool(args, "reuse");
    let full = opt_bool(args, "full");

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

    conn.execute(qapi_qmp::drive_mirror(DriveMirror {
        device,
        target,
        format,
        sync,
        mode: Some(mode),
        unmap: Some(true),
        auto_dismiss: None,
        auto_finalize: None,
        buf_size: None,
        copy_mode: None,
        granularity: None,
        job_id: None,
        node_name: None,
        on_source_error: None,
        on_target_error: None,
        replaces: None,
        speed: None,
    }))
    .await
    .map_err(CmdError::from)?;

    Ok(String::new())
}
