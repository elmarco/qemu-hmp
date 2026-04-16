// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use serde::{Deserialize, Serialize};

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

// query-accelerators is a new QMP command (Since: 10.2.0) not yet in the
// qapi-rs crate.  Define the command and response types manually.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
struct query_accelerators {}

impl qapi_qmp::QmpCommand for query_accelerators {}
impl qapi::Command for query_accelerators {
    const NAME: &'static str = "query-accelerators";
    const ALLOW_OOB: bool = false;
    type Ok = AcceleratorInfo;
}

#[derive(Debug, Deserialize)]
struct AcceleratorInfo {
    enabled: String,
    present: Vec<String>,
}

pub async fn cmd_info_accelerators(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let info = conn
        .execute(query_accelerators {})
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    let len = info.present.len();
    for (i, accel) in info.present.iter().enumerate() {
        let trail = if i + 1 < len { ' ' } else { '\n' };
        if *accel == info.enabled {
            write!(out, "[{}]{}", accel, trail).unwrap();
        } else {
            write!(out, "{}{}", accel, trail).unwrap();
        }
    }

    Ok(out)
}
