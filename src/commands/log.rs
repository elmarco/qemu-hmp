// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

// x-log is not yet in the qapi-rs crate, define it locally.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_log {
    pub items: String,
}

impl qapi_qmp::QmpCommand for x_log {}
impl qapi::Command for x_log {
    const NAME: &'static str = "x-log";
    const ALLOW_OOB: bool = false;
    type Ok = qapi::Empty;
}

pub async fn cmd_log(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let items = require_str(args, "items")?;
    conn.execute(x_log { items })
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
