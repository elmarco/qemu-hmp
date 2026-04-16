// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

// x-trace-file is not yet in the qapi-rs crate, define it locally.

#[derive(Debug, Deserialize)]
pub struct TraceFileInfo {
    pub filename: String,
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_trace_file {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flush: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

impl qapi_qmp::QmpCommand for x_trace_file {}
impl qapi::Command for x_trace_file {
    const NAME: &'static str = "x-trace-file";
    const ALLOW_OOB: bool = false;
    type Ok = TraceFileInfo;
}

pub async fn cmd_trace_file(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let op = match args.get("op") {
        Some(ArgValue::Str(s)) => Some(s.as_str()),
        _ => None,
    };
    let arg = match args.get("arg") {
        Some(ArgValue::Str(s)) => Some(s.clone()),
        _ => None,
    };

    let (enable, flush, filename) = match op {
        None => (None, None, None),
        Some("on") => (Some(true), None, None),
        Some("off") => (Some(false), None, None),
        Some("flush") => (None, Some(true), None),
        Some("set") => (None, None, arg),
        Some(other) => {
            return Err(CmdError::Command(format!(
                "unexpected argument \"{other}\""
            )));
        }
    };

    let info = conn
        .execute(x_trace_file {
            enable,
            flush,
            filename,
        })
        .await
        .map_err(CmdError::from)?;

    if op.is_none() {
        Ok(format!(
            "Trace file \"{}\" {}.",
            info.filename,
            if info.enabled { "on" } else { "off" }
        ))
    } else {
        Ok(String::new())
    }
}
