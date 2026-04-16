// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::{parse_keyval, require_str, CmdError};
use crate::qmp::QmpConnection;

// object-add uses a boxed ObjectOptions union with 60+ variants.
// Rather than enumerating them all, we send the arguments as raw JSON
// using serde(flatten) on a serde_json::Value.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct raw_object_add {
    #[serde(flatten)]
    pub args: serde_json::Value,
}

impl qapi_qmp::QmpCommand for raw_object_add {}
impl qapi::Command for raw_object_add {
    const NAME: &'static str = "object-add";
    const ALLOW_OOB: bool = false;
    type Ok = serde_json::Value;
}

pub async fn cmd_object_add(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let spec = require_str(args, "object")?;
    let obj = parse_keyval(&spec, Some("qom-type"))
        .map_err(|e| CmdError::Command(format!("invalid object spec: {e}")))?;
    conn.execute(raw_object_add { args: obj })
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
