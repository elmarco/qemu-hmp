// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

// x-query-lapic is a new QMP command (Since: 11.1) not yet in the
// qapi-rs crate.  Define the command struct manually.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
struct x_query_lapic {
    #[serde(rename = "apic-id", skip_serializing_if = "Option::is_none")]
    apic_id: Option<i64>,
}

impl qapi_qmp::QmpCommand for x_query_lapic {}
impl qapi::Command for x_query_lapic {
    const NAME: &'static str = "x-query-lapic";
    const ALLOW_OOB: bool = false;
    type Ok = qapi_qmp::HumanReadableText;
}

pub async fn cmd_info_lapic(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let apic_id = match args.get("apic-id") {
        Some(ArgValue::Int(n)) => Some(*n),
        _ => None,
    };

    let info = conn
        .execute(x_query_lapic { apic_id })
        .await
        .map_err(CmdError::from)?;

    Ok(info.human_readable_text)
}
