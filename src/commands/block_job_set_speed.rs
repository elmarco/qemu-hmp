// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_int, require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_block_job_set_speed(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let device = require_str(args, "device")?;
    let speed = require_int(args, "speed")?;
    conn.execute(qapi::qmp::block_job_set_speed { device, speed })
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
