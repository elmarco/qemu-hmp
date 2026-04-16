// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::{opt_bool, require_expr, require_int, CmdError};
use crate::qmp::QmpConnection;

// x-mce is a new QMP command (Since: 11.1) not yet in the qapi-rs crate.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_mce {
    #[serde(rename = "cpu-index")]
    pub cpu_index: i64,
    pub bank: i64,
    pub status: i64,
    #[serde(rename = "mcg-status")]
    pub mcg_status: i64,
    pub addr: i64,
    pub misc: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broadcast: Option<bool>,
}

impl qapi_qmp::QmpCommand for x_mce {}
impl qapi::Command for x_mce {
    const NAME: &'static str = "x-mce";
    const ALLOW_OOB: bool = false;
    type Ok = qapi::Empty;
}

pub async fn cmd_mce(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let broadcast = opt_bool(args, "broadcast");
    let cpu_index = require_int(args, "cpu_index")?;
    let bank = require_int(args, "bank")?;
    let status = require_expr(conn, args, "status").await?;
    let mcg_status = require_expr(conn, args, "mcg_status").await?;
    let addr = require_expr(conn, args, "addr").await?;
    let misc = require_expr(conn, args, "misc").await?;

    conn.execute(x_mce {
        cpu_index,
        bank,
        status,
        mcg_status,
        addr,
        misc,
        broadcast: if broadcast { Some(true) } else { None },
    })
    .await
    .map_err(CmdError::from)?;

    Ok(String::new())
}
