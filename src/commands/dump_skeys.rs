// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_dump_skeys(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let filename = require_str(args, "filename")?;
    conn.execute(qapi_qmp::dump_skeys { filename })
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
