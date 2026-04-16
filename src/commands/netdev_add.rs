// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::{parse_keyval, require_str, CmdError};
use crate::qmp::QmpConnection;

// netdev_add uses 'boxed': true with the Netdev discriminated union,
// which has many variants (user, tap, socket, stream, etc.).  Rather
// than enumerating them all, we send the arguments as raw JSON using
// serde(flatten) on a serde_json::Value.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct raw_netdev_add {
    #[serde(flatten)]
    pub args: serde_json::Value,
}

impl qapi_qmp::QmpCommand for raw_netdev_add {}
impl qapi::Command for raw_netdev_add {
    const NAME: &'static str = "netdev_add";
    const ALLOW_OOB: bool = false;
    type Ok = qapi::Empty;
}

pub async fn cmd_netdev_add(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let spec = require_str(args, "netdev")?;
    let obj = parse_keyval(&spec, Some("type"))
        .map_err(|e| CmdError::Command(format!("invalid netdev spec: {e}")))?;
    conn.execute(raw_netdev_add { args: obj })
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
