// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::{require_bool, require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_migrate_set_capability(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let cap_str = require_str(args, "capability")?;
    let state = require_bool(args, "state")?;

    let capability = qapi_qmp::MigrationCapability::from_name(&cap_str)
        .ok_or_else(|| CmdError::Command(format!("invalid parameter value: {cap_str}")))?;

    conn.execute(qapi_qmp::migrate_set_capabilities {
        capabilities: vec![qapi_qmp::MigrationCapabilityStatus { capability, state }],
    })
    .await
    .map_err(CmdError::from)?;

    Ok(String::new())
}
