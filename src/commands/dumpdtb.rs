// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_dumpdtb(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let filename = require_str(args, "filename")?;

    conn.execute(qapi_qmp::dumpdtb {
        filename: filename.clone(),
    })
    .await
    .map_err(CmdError::from)?;

    Ok(format!("DTB dumped to '{filename}'\n"))
}
