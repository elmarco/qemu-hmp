// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_status(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let info = conn
        .execute(qapi::qmp::query_status {})
        .await
        .map_err(CmdError::from)?;
    if info.running {
        Ok("VM status: running".to_string())
    } else if matches!(info.status, qapi_qmp::RunState::paused) {
        Ok("VM status: paused".to_string())
    } else {
        Ok(format!("VM status: paused ({})", info.status.name()))
    }
}
