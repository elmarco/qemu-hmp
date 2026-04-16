// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

// x-sync-profile is not yet in the qapi-rs crate, define it locally.

#[derive(Debug, Deserialize)]
pub struct SyncProfileInfo {
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_sync_profile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op: Option<String>,
}

impl qapi_qmp::QmpCommand for x_sync_profile {}
impl qapi::Command for x_sync_profile {
    const NAME: &'static str = "x-sync-profile";
    const ALLOW_OOB: bool = false;
    type Ok = SyncProfileInfo;
}

pub async fn cmd_sync_profile(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let op = match args.get("op") {
        Some(ArgValue::Str(s)) => Some(s.as_str()),
        _ => None,
    };

    // Validate the operation string, matching the C handler's accepted values.
    if let Some(op_str) = op {
        match op_str {
            "on" | "off" | "reset" => {}
            _ => {
                return Err(CmdError::Command(format!(
                    "invalid parameter '{op_str}',\
                     expecting 'on', 'off', or 'reset'"
                )));
            }
        }
    }

    let info = conn
        .execute(x_sync_profile {
            op: op.map(|s| s.to_string()),
        })
        .await
        .map_err(CmdError::from)?;

    if op.is_none() {
        let state = if info.enabled { "on" } else { "off" };
        Ok(format!("sync-profile is {state}"))
    } else {
        Ok(String::new())
    }
}
