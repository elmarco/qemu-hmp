// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::args::ArgValue;
use crate::commands::{require_expr, CmdError};
use crate::qmp::QmpConnection;

// x-gpa2hva is not yet in the qapi-rs crate, define it locally.

#[derive(Debug, Deserialize)]
pub struct GpaToHvaInfo {
    pub hva: i64,
    pub region: String,
}

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_gpa2hva {
    pub addr: i64,
}

impl qapi_qmp::QmpCommand for x_gpa2hva {}
impl qapi::Command for x_gpa2hva {
    const NAME: &'static str = "x-gpa2hva";
    const ALLOW_OOB: bool = false;
    type Ok = GpaToHvaInfo;
}

pub async fn cmd_gpa2hva(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let addr = require_expr(conn, args, "addr").await?;

    let info = conn
        .execute(x_gpa2hva { addr })
        .await
        .map_err(CmdError::from)?;

    Ok(format!(
        "Host virtual address for 0x{:x} ({}) is 0x{:x}",
        addr as u64, info.region, info.hva as u64
    ))
}
