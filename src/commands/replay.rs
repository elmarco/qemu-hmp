// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_expr, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_replay_break(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let icount = require_expr(conn, args, "icount").await?;

    match conn.execute(qapi_qmp::replay_break { icount }).await {
        Ok(_) => Ok(String::new()),
        Err(e) => Ok(e.to_string()),
    }
}

pub async fn cmd_replay_delete_break(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    match conn.execute(qapi_qmp::replay_delete_break {}).await {
        Ok(_) => Ok(String::new()),
        Err(e) => Ok(e.to_string()),
    }
}

pub async fn cmd_replay_seek(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let icount = require_expr(conn, args, "icount").await?;

    match conn.execute(qapi_qmp::replay_seek { icount }).await {
        Ok(_) => Ok(String::new()),
        Err(e) => Ok(e.to_string()),
    }
}

pub async fn cmd_info_replay(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let info = conn
        .execute(qapi_qmp::query_replay {})
        .await
        .map_err(CmdError::from)?;

    if info.mode == qapi_qmp::ReplayMode::none {
        return Ok("Record/replay is not active".to_string());
    }

    let mode_str = if info.mode == qapi_qmp::ReplayMode::record {
        "Recording"
    } else {
        "Replaying"
    };

    let filename = info.filename.as_deref().unwrap_or("");

    Ok(format!(
        "{mode_str} execution '{filename}': instruction count = {}",
        info.icount
    ))
}
