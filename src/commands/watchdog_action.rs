// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_watchdog_action(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let action_str = require_str(args, "action")?;
    let action_lower = action_str.to_ascii_lowercase();

    let action = qapi_qmp::WatchdogAction::from_name(&action_lower)
        .ok_or_else(|| CmdError::Command(format!("invalid parameter value: {action_lower}")))?;

    conn.execute(qapi_qmp::watchdog_set_action { action })
        .await
        .map_err(CmdError::from)?;

    Ok(String::new())
}
