// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_x_colo_lost_heartbeat(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    conn.execute(qapi_qmp::x_colo_lost_heartbeat {})
        .await
        .map_err(CmdError::from)?;

    Ok(String::new())
}
