// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::args::ArgValue;
use crate::commands::{require_expr, CmdError};
use crate::qmp::QmpConnection;

// x-gpa2hpa is not yet in the qapi-rs crate, define it locally.

#[derive(Debug, Deserialize)]
pub struct GpaToHpaInfo {
    pub hpa: i64,
    pub region: String,
}

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_gpa2hpa {
    pub addr: i64,
}

impl qapi_qmp::QmpCommand for x_gpa2hpa {}
impl qapi::Command for x_gpa2hpa {
    const NAME: &'static str = "x-gpa2hpa";
    const ALLOW_OOB: bool = false;
    type Ok = GpaToHpaInfo;
}

pub async fn cmd_gpa2hpa(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let addr = require_expr(conn, args, "addr").await?;

    let info = conn
        .execute(x_gpa2hpa { addr })
        .await
        .map_err(CmdError::from)?;

    Ok(format!(
        "Host physical address for 0x{:x} ({}) is 0x{:x}",
        addr as u64, info.region, info.hpa as u64
    ))
}
