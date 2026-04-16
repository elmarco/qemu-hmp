// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_chardev_send_break(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let id = require_str(args, "id")?;
    conn.execute(qapi::qmp::chardev_send_break { id })
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
