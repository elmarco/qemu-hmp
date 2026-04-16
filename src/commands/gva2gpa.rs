// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::args::ArgValue;
use crate::commands::{require_expr, CmdError};
use crate::qmp::QmpConnection;

// x-gva2gpa is not yet in the qapi-rs crate, define it locally.

#[derive(Debug, Deserialize)]
pub struct GvaToGpaInfo {
    pub gpa: i64,
}

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_gva2gpa {
    pub addr: i64,
}

impl qapi_qmp::QmpCommand for x_gva2gpa {}
impl qapi::Command for x_gva2gpa {
    const NAME: &'static str = "x-gva2gpa";
    const ALLOW_OOB: bool = false;
    type Ok = GvaToGpaInfo;
}

pub async fn cmd_gva2gpa(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let addr = require_expr(conn, args, "addr").await?;

    let info = conn
        .execute(x_gva2gpa { addr })
        .await
        .map_err(CmdError::from)?;

    Ok(format!("gpa: 0x{:x}", info.gpa as u64))
}
