// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::args::ArgValue;
use crate::commands::{require_int, CmdError};
use crate::qmp::QmpConnection;

// x-query-sum is not yet in the qapi-rs crate, define it locally.

#[derive(Debug, Deserialize)]
pub struct SumInfo {
    pub sum: i64,
}

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_sum {
    pub start: i64,
    pub size: i64,
}

impl qapi_qmp::QmpCommand for x_sum {}
impl qapi::Command for x_sum {
    const NAME: &'static str = "x-sum";
    const ALLOW_OOB: bool = false;
    type Ok = SumInfo;
}

pub async fn cmd_sum(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let start = require_int(args, "start")?;
    let size = require_int(args, "size")?;
    let info = conn
        .execute(x_sum { start, size })
        .await
        .map_err(CmdError::from)?;
    Ok(format!("{:05}", info.sum))
}
