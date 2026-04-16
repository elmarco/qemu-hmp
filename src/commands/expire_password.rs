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

    match protocol.as_str() {
        "vnc" => {
            let opts = qapi_qmp::ExpirePasswordOptions::vnc {
                time,
                vnc: qapi_qmp::ExpirePasswordOptionsVnc { display },
            };
            conn.execute(qapi_qmp::expire_password(opts))
                .await
                .map_err(CmdError::from)?;
        }
        "spice" => {
            // serde can't serialize the spice newtype variant with
            // internal tagging, so build the JSON manually.
            let json = serde_json::json!({
                "execute": "expire_password",
                "arguments": { "protocol": "spice", "time": time }
            });
            let resp = conn
                .execute_raw(&json)
                .await
                .map_err(|e| CmdError::from(qapi::ExecuteError::Io(e)))?;
            if let Some(err) = resp.get("error") {
                let desc = err
                    .get("desc")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                return Err(CmdError::Command(desc.to_string()));
            }
        }
        _ => {
            return Err(CmdError::Command(format!(
                "invalid parameter value '{protocol}': expected vnc or spice"
            )));
        }
    }

    Ok(String::new())
}
