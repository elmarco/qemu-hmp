// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_trace_events(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let name = match args.get("name") {
        Some(ArgValue::Str(s)) => s.clone(),
        _ => "*".to_string(),
    };

    let events = conn
        .execute(qapi_qmp::trace_event_get_state { name })
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    for ev in &events {
        let state = match ev.state {
            qapi_qmp::TraceEventState::enabled => 1,
            _ => 0,
        };
        writeln!(out, "{} : state {}", ev.name, state).unwrap();
    }

    Ok(out)
}
