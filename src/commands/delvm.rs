// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::savevm::{find_snapshot_devices, wait_for_job};
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_delvm(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let tag = require_str(args, "name")?;
    let (_vmstate, devices) = find_snapshot_devices(conn).await?;

    let job_id = format!("delvm-{}", std::process::id());
    conn.execute(qapi::qmp::snapshot_delete {
        job_id: job_id.clone(),
        tag,
        devices,
    })
    .await
    .map_err(CmdError::from)?;

    wait_for_job(conn, &job_id).await?;

    Ok(String::new())
}
