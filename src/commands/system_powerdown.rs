// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_system_powerdown(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    conn.execute(qapi::qmp::system_powerdown {})
        .await
        .map_err(CmdError::from)?;
    Ok(String::new())
}
