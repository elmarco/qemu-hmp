// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_roms(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let result = conn
        .execute(qapi_qmp::x_query_roms {})
        .await
        .map_err(CmdError::from)?;

    Ok(result.human_readable_text)
}
