// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

// x-query-sync-profile is a new QMP command (Since: 11.1) not yet in the
// qapi-rs crate.  Define the command struct manually.

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(non_camel_case_types)]
struct x_query_sync_profile {
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sort_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    coalesce: Option<bool>,
}

impl qapi_qmp::QmpCommand for x_query_sync_profile {}
impl qapi::Command for x_query_sync_profile {
    const NAME: &'static str = "x-query-sync-profile";
    const ALLOW_OOB: bool = false;
    type Ok = qapi_qmp::HumanReadableText;
}

pub async fn cmd_info_sync_profile(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let max = args.get("max").and_then(|v| match v {
        ArgValue::Int(n) => Some(*n),
        _ => None,
    });
    let mean = args
        .get("mean")
        .and_then(|v| match v {
            ArgValue::Bool(b) => Some(*b),
            _ => None,
        })
        .unwrap_or(false);
    let no_coalesce = args
        .get("no_coalesce")
        .and_then(|v| match v {
            ArgValue::Bool(b) => Some(*b),
            _ => None,
        })
        .unwrap_or(false);

    let sort_by = if mean {
        Some("avg-wait-time".to_string())
    } else {
        None
    };
    let coalesce = if no_coalesce { Some(false) } else { None };

    let info = conn
        .execute(x_query_sync_profile {
            max,
            sort_by,
            coalesce,
        })
        .await
        .map_err(CmdError::from)?;

    Ok(info.human_readable_text)
}
