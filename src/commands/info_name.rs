// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_name(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let info = conn
        .execute(qapi::qmp::query_name {})
        .await
        .map_err(CmdError::from)?;
    Ok(info.name.unwrap_or_default())
}
