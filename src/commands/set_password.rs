// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_set_password(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let protocol = require_str(args, "protocol")?;
    let password = require_str(args, "password")?;

    let connected = match args.get("connected") {
        Some(ArgValue::Str(s)) => Some(s.parse().map_err(|()| {
            CmdError::Command(format!(
                "invalid parameter value '{s}': expected keep, fail, or disconnect"
            ))
        })?),
        _ => None,
    };

    let display = match args.get("display") {
        Some(ArgValue::Str(s)) => Some(s.clone()),
        _ => None,
    };

    let base = qapi_qmp::SetPasswordOptionsBase {
        password,
        connected,
    };

    let opts = match protocol.as_str() {
        "vnc" => qapi_qmp::SetPasswordOptions::vnc {
            base,
            vnc: qapi_qmp::SetPasswordOptionsVnc { display },
        },
        "spice" => qapi_qmp::SetPasswordOptions::spice(base),
        _ => {
            return Err(CmdError::Command(format!(
                "invalid parameter value '{protocol}': expected vnc or spice"
            )));
        }
    };

    conn.execute(qapi_qmp::set_password(opts))
        .await
        .map_err(CmdError::from)?;

    Ok(String::new())
}
