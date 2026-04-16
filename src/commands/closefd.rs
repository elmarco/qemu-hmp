// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_closefd(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let fdname = require_str(args, "fdname")?;

    conn.execute(qapi_qmp::closefd { fdname })
        .await
        .map_err(CmdError::from)?;

    Ok(String::new())
}
