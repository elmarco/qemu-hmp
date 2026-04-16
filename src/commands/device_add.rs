// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::{parse_keyval, require_str, CmdError};
use crate::qmp::QmpConnection;

// device_add uses 'gen': false in the QAPI schema, accepting arbitrary
// additional properties via #[serde(flatten)].  We define a raw struct
// so we can send an untyped JSON object directly.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct raw_device_add {
    #[serde(flatten)]
    pub args: serde_json::Value,
}

impl qapi_qmp::QmpCommand for raw_device_add {}
impl qapi::Command for raw_device_add {
    const NAME: &'static str = "device_add";
    const ALLOW_OOB: bool = false;
    type Ok = qapi::Empty;
}

pub async fn cmd_device_add(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let spec = require_str(args, "device")?;
    let obj = parse_keyval(&spec, Some("driver"))
        .map_err(|e| CmdError::Command(format!("invalid device spec: {e}")))?;
    conn.execute(raw_device_add { args: obj })
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
