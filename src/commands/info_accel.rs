// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

// x-accel-stats is a QMP command (Since: 10.1) not yet in the
// qapi-rs crate.  Define the command struct manually.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
struct x_accel_stats {}

impl qapi_qmp::QmpCommand for x_accel_stats {}
impl qapi::Command for x_accel_stats {
    const NAME: &'static str = "x-accel-stats";
    const ALLOW_OOB: bool = false;
    type Ok = qapi_qmp::HumanReadableText;
}

pub async fn cmd_info_accel(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let info = conn
        .execute(x_accel_stats {})
        .await
        .map_err(CmdError::from)?;

    Ok(info.human_readable_text)
}
