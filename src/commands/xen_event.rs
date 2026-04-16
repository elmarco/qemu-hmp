// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::{require_int, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_xen_event_inject(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let port = require_int(args, "port")?;

    conn.execute(qapi_qmp::xen_event_inject { port: port as u32 })
        .await
        .map_err(CmdError::from)?;

    Ok(format!("Delivered port {port}\n"))
}

pub async fn cmd_xen_event_list(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let list = conn
        .execute(qapi_qmp::xen_event_list {})
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    for info in &list {
        write!(
            out,
            "port {:4}: vcpu: {} {}",
            info.port,
            info.vcpu,
            info.type_.name()
        )
        .unwrap();

        if info.type_ != qapi_qmp::EvtchnPortType::ipi {
            write!(out, "(").unwrap();
            if !info.remote_domain.is_empty() {
                write!(out, "{}:", info.remote_domain).unwrap();
            }
            write!(out, "{})", info.target).unwrap();
        }

        if info.pending {
            write!(out, " PENDING").unwrap();
        }
        if info.masked {
            write!(out, " MASKED").unwrap();
        }
        writeln!(out).unwrap();
    }

    Ok(out)
}
