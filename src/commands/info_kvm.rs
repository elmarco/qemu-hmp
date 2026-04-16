// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_kvm(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let info = conn
        .execute(qapi::qmp::query_kvm {})
        .await
        .map_err(CmdError::from)?;
    let status = if !info.present {
        "not compiled in"
    } else if info.enabled {
        "enabled"
    } else {
        "disabled"
    };
    Ok(format!("kvm support: {status}"))
}
