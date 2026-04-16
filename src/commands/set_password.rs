// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use qapi::Enum;

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

    match protocol.as_str() {
        "vnc" => {
            let base = qapi_qmp::SetPasswordOptionsBase {
                password,
                connected,
            };
            let opts = qapi_qmp::SetPasswordOptions::vnc {
                base,
                vnc: qapi_qmp::SetPasswordOptionsVnc { display },
            };
            conn.execute(qapi_qmp::set_password(opts))
                .await
                .map_err(CmdError::from)?;
        }
        "spice" => {
            // serde can't serialize the spice newtype variant with
            // internal tagging, so build the JSON manually.
            let mut arguments = serde_json::json!({
                "protocol": "spice",
                "password": password,
            });
            if let Some(c) = connected {
                arguments["connected"] = serde_json::Value::String(c.name().to_string());
            }
            let json = serde_json::json!({
                "execute": "set_password",
                "arguments": arguments,
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
