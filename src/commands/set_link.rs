// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_bool, require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_set_link(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let name = require_str(args, "name")?;
    let up = require_bool(args, "up")?;
    conn.execute(qapi::qmp::set_link { name, up })
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
