// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_expire_password(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let protocol = require_str(args, "protocol")?;
    let time = require_str(args, "time")?;

    let display = match args.get("display") {
        Some(ArgValue::Str(s)) => Some(s.clone()),
        _ => None,
    };

    let opts = match protocol.as_str() {
        "vnc" => qapi_qmp::ExpirePasswordOptions::vnc {
            time,
            vnc: qapi_qmp::ExpirePasswordOptionsVnc { display },
        },
        "spice" => qapi_qmp::ExpirePasswordOptions::spice(time),
        _ => {
            return Err(CmdError::Command(format!(
                "invalid parameter value '{protocol}': expected vnc or spice"
            )));
        }
    };

    conn.execute(qapi_qmp::expire_password(opts))
        .await
        .map_err(CmdError::from)?;

    Ok(String::new())
}
