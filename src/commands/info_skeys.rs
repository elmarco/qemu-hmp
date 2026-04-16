// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::{require_expr, CmdError};
use crate::qmp::QmpConnection;

// x-query-skeys is a new QMP command (Since: 11.1) not yet in the
// qapi-rs crate.  Define the command struct manually.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
struct x_query_skeys {
    addr: i64,
}

impl qapi_qmp::QmpCommand for x_query_skeys {}
impl qapi::Command for x_query_skeys {
    const NAME: &'static str = "x-query-skeys";
    const ALLOW_OOB: bool = false;
    type Ok = qapi_qmp::HumanReadableText;
}

pub async fn cmd_info_skeys(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let addr = require_expr(conn, args, "addr").await?;

    let info = conn
        .execute(x_query_skeys { addr })
        .await
        .map_err(CmdError::from)?;

    Ok(info.human_readable_text)
}
