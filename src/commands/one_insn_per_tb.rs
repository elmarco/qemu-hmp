// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde_json::Value;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_one_insn_per_tb(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let enabled = match args.get("option") {
        None => true,
        Some(ArgValue::Str(s)) => match s.as_str() {
            "on" => true,
            "off" => false,
            other => {
                return Err(CmdError::Command(format!("unexpected option {other}")));
            }
        },
        _ => true,
    };

    conn.execute(qapi::qmp::qom_set {
        path: "/machine/accel".to_string(),
        property: "one-insn-per-tb".to_string(),
        value: Value::Bool(enabled),
    })
    .await
    .map_err(CmdError::from)?;
    Ok(String::new())
}
