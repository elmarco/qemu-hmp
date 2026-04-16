// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

// x-logfile is not yet in the qapi-rs crate, define it locally.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_logfile {
    pub filename: String,
}

impl qapi_qmp::QmpCommand for x_logfile {}
impl qapi::Command for x_logfile {
    const NAME: &'static str = "x-logfile";
    const ALLOW_OOB: bool = false;
    type Ok = qapi::Empty;
}

pub async fn cmd_logfile(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let filename = require_str(args, "filename")?;
    conn.execute(x_logfile { filename })
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
