// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

const DEFAULT_GDBSTUB_PORT: &str = "1234";

// x-gdbserver is not yet in the qapi-rs crate, define it locally.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_gdbserver {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

impl qapi_qmp::QmpCommand for x_gdbserver {}
impl qapi::Command for x_gdbserver {
    const NAME: &'static str = "x-gdbserver";
    const ALLOW_OOB: bool = false;
    type Ok = qapi::Empty;
}

pub async fn cmd_gdbserver(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let device = match args.get("device") {
        Some(ArgValue::Str(s)) => s.clone(),
        _ => format!("tcp::{DEFAULT_GDBSTUB_PORT}"),
    };

    conn.execute(x_gdbserver {
        device: Some(device.clone()),
    })
    .await
    .map_err(CmdError::from)?;

    if device == "none" {
        Ok("Disabled gdbserver".to_string())
    } else {
        Ok(format!("Waiting for gdb connection on device '{device}'"))
    }
}
