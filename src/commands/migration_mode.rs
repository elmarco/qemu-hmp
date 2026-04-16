// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::Serialize;

use crate::args::ArgValue;
use crate::commands::{require_int, CmdError};
use crate::qmp::QmpConnection;

// x-set-migration-mode is not yet in the qapi-rs crate, define it locally.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_set_migration_mode {
    pub mode: bool,
}

impl qapi_qmp::QmpCommand for x_set_migration_mode {}
impl qapi::Command for x_set_migration_mode {
    const NAME: &'static str = "x-set-migration-mode";
    const ALLOW_OOB: bool = false;
    type Ok = qapi::Empty;
}

pub async fn cmd_migration_mode(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let mode = require_int(args, "mode")?;
    conn.execute(x_set_migration_mode { mode: mode != 0 })
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
