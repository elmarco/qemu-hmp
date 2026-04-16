// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

// x-hostfwd-remove is a new QMP command (Since: 11.1) not yet in the
// qapi-rs crate.  Define the command struct manually.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_hostfwd_remove {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub rule: String,
}

impl qapi_qmp::QmpCommand for x_hostfwd_remove {}
impl qapi::Command for x_hostfwd_remove {
    const NAME: &'static str = "x-hostfwd-remove";
    const ALLOW_OOB: bool = false;
    type Ok = qapi::Empty;
}

pub async fn cmd_hostfwd_remove(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let arg1 = require_str(args, "arg1")?;
    let arg2 = args.get("arg2").and_then(|v| {
        if let ArgValue::Str(s) = v {
            Some(s.clone())
        } else {
            None
        }
    });

    let (id, rule) = if let Some(a2) = arg2 {
        (Some(arg1), a2)
    } else {
        (None, arg1)
    };

    match conn
        .execute(x_hostfwd_remove {
            id,
            rule: rule.clone(),
        })
        .await
    {
        Ok(_) => Ok(format!("host forwarding rule for {rule} removed\n")),
        Err(e) => Ok(e.to_string()),
    }
}
