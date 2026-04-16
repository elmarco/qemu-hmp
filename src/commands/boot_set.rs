// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

// x-boot-set is not yet in the qapi-rs crate, define it locally.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_boot_set {
    pub bootdevice: String,
}

impl qapi_qmp::QmpCommand for x_boot_set {}
impl qapi::Command for x_boot_set {
    const NAME: &'static str = "x-boot-set";
    const ALLOW_OOB: bool = false;
    type Ok = qapi::Empty;
}

pub async fn cmd_boot_set(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let bootdevice = require_str(args, "bootdevice")?;

    conn.execute(x_boot_set {
        bootdevice: bootdevice.clone(),
    })
    .await
    .map_err(CmdError::from)?;

    Ok(format!("boot device list now set to {bootdevice}"))
}
