// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_version(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let info = conn
        .execute(qapi::qmp::query_version {})
        .await
        .map_err(CmdError::from)?;
    let v = &info.qemu;
    let pkg = &info.package;
    Ok(format!("{}.{}.{}{}", v.major, v.minor, v.micro, pkg))
}
