// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_bool, require_str, CmdError};
use crate::qmp::QmpConnection;

/// Set the state of a trace event.
///
/// The built-in HMP handler accepts an optional `vcpu` argument but
/// ignores it — the QMP command has no per-vCPU support.  We mirror
/// that behaviour: `vcpu` is parsed by the arg spec but unused here.
pub async fn cmd_trace_event(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let name = require_str(args, "name")?;
    let enable = require_bool(args, "option")?;

    conn.execute(qapi::qmp::trace_event_set_state {
        name,
        enable,
        ignore_unavailable: Some(true),
    })
    .await
    .map_err(CmdError::from)?;

    Ok(String::new())
}
