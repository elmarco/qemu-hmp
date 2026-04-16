// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

// x-query-mtree is a new QMP command (Since: 11.1) not yet in the
// qapi-rs crate.  Define the command struct manually.

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(non_camel_case_types)]
struct x_query_mtree {
    #[serde(skip_serializing_if = "Option::is_none")]
    flatview: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dispatch_tree: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disabled: Option<bool>,
}

impl qapi_qmp::QmpCommand for x_query_mtree {}
impl qapi::Command for x_query_mtree {
    const NAME: &'static str = "x-query-mtree";
    const ALLOW_OOB: bool = false;
    type Ok = qapi_qmp::HumanReadableText;
}

pub async fn cmd_info_mtree(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let flatview = match args.get("flatview") {
        Some(ArgValue::Bool(true)) => Some(true),
        _ => None,
    };
    let dispatch_tree = match args.get("dispatch_tree") {
        Some(ArgValue::Bool(true)) => Some(true),
        _ => None,
    };
    let owner = match args.get("owner") {
        Some(ArgValue::Bool(true)) => Some(true),
        _ => None,
    };
    let disabled = match args.get("disabled") {
        Some(ArgValue::Bool(true)) => Some(true),
        _ => None,
    };

    let info = conn
        .execute(x_query_mtree {
            flatview,
            dispatch_tree,
            owner,
            disabled,
        })
        .await
        .map_err(CmdError::from)?;

    Ok(info.human_readable_text)
}
