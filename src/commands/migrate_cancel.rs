// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_migrate_cancel(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    conn.execute(qapi::qmp::migrate_cancel {})
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
