// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_migrate_continue(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let state_str = require_str(args, "state")?;

    let state = qapi_qmp::MigrationStatus::from_name(&state_str)
        .ok_or_else(|| CmdError::Command(format!("invalid parameter value: {state_str}")))?;

    conn.execute(qapi_qmp::migrate_continue { state })
        .await
        .map_err(CmdError::from)?;

    Ok(String::new())
}
