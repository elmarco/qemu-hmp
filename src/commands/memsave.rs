// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_expr, require_int, require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_memsave(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let addr = require_expr(conn, args, "val").await? as u64;
    let size = require_int(args, "size")? as u64;
    let filename = require_str(args, "filename")?;

    // The C handler uses monitor_get_cpu_index() which returns the
    // CPU set by the 'cpu' command.  We forward our stored index.
    let cpu_index = conn.cpu_index();

    conn.execute(qapi_qmp::memsave {
        val: addr,
        size,
        filename,
        cpu_index,
    })
    .await
    .map_err(CmdError::from)?;

    Ok(String::new())
}

pub async fn cmd_pmemsave(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let addr = require_expr(conn, args, "val").await? as u64;
    let size = require_int(args, "size")? as u64;
    let filename = require_str(args, "filename")?;

    conn.execute(qapi_qmp::pmemsave {
        val: addr,
        size,
        filename,
    })
    .await
    .map_err(CmdError::from)?;

    Ok(String::new())
}
