// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

// x-query-registers is a new QMP command (Since: 11.1) not yet in the
// qapi-rs crate.  Define the command struct manually.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
struct x_query_registers {
    #[serde(skip_serializing_if = "Option::is_none")]
    cpu: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    all: Option<bool>,
}

impl qapi_qmp::QmpCommand for x_query_registers {}
impl qapi::Command for x_query_registers {
    const NAME: &'static str = "x-query-registers";
    const ALLOW_OOB: bool = false;
    type Ok = qapi_qmp::HumanReadableText;
}

pub async fn cmd_info_registers(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let all = match args.get("cpustate_all") {
        Some(ArgValue::Bool(true)) => Some(true),
        _ => None,
    };

    let cpu = match args.get("vcpu") {
        Some(ArgValue::Int(n)) => Some(*n),
        _ => None,
    };

    let info = conn
        .execute(x_query_registers { cpu, all })
        .await
        .map_err(CmdError::from)?;

    Ok(info.human_readable_text)
}
