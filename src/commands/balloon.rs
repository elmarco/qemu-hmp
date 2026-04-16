// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_int, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_balloon(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let value = require_int(args, "value")?;
    conn.execute(qapi::qmp::balloon { value })
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
