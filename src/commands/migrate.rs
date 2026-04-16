// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{opt_bool, require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_migrate(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let uri = require_str(args, "uri")?;
    let resume = opt_bool(args, "resume");
    conn.execute(qapi::qmp::migrate {
        uri: Some(uri),
        channels: None,
        detach: None,
        resume: Some(resume),
    })
    .await
    .map_err(CmdError::from)?;
    Ok(String::new())
}
