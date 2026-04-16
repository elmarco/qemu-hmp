// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::{require_int, CmdError};
use crate::qmp::QmpConnection;

// x-mouse-set is not yet in the qapi-rs crate, define it locally.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_mouse_set {
    pub index: i64,
}

impl qapi_qmp::QmpCommand for x_mouse_set {}
impl qapi::Command for x_mouse_set {
    const NAME: &'static str = "x-mouse-set";
    const ALLOW_OOB: bool = false;
    type Ok = qapi::Empty;
}

pub async fn cmd_mouse_set(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let index = require_int(args, "index")?;

    conn.execute(x_mouse_set { index })
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
