// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{opt_bool, require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_block_job_cancel(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let force = opt_bool(args, "force");
    let device = require_str(args, "device")?;
    conn.execute(qapi::qmp::block_job_cancel {
        device,
        force: Some(force),
    })
    .await
    .map_err(CmdError::from)?;
    Ok(String::new())
}
