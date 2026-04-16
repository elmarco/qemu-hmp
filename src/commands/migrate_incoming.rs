// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_migrate_incoming(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let uri = require_str(args, "uri")?;
    conn.execute(qapi::qmp::migrate_incoming {
        uri: Some(uri),
        channels: None,
        exit_on_error: None,
    })
    .await
    .map_err(CmdError::from)?;
    Ok(String::new())
}
