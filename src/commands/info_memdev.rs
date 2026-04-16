// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

/// Format a sorted list of u16 values using range notation,
/// matching QEMU's string_output_visitor with human=false.
/// e.g. [0,1,2,5,7,8] -> "0-2,5,7-8"
fn format_host_nodes(nodes: &[u16]) -> String {
    if nodes.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    let mut i = 0;
    while i < nodes.len() {
        let start = nodes[i];
        let mut end = start;
        while i + 1 < nodes.len() && nodes[i + 1] == end + 1 {
            end = nodes[i + 1];
            i += 1;
        }
        if !out.is_empty() {
            out.push(',');
        }
        if start == end {
            write!(out, "{}", start).unwrap();
        } else {
            write!(out, "{}-{}", start, end).unwrap();
        }
        i += 1;
    }
    out
}

pub async fn cmd_info_memdev(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let list = conn
        .execute(qapi_qmp::query_memdev {})
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    for m in &list {
        writeln!(out, "memory backend: {}", m.id.as_deref().unwrap_or("")).unwrap();
        writeln!(out, "  size:  {}", m.size).unwrap();
        writeln!(out, "  merge: {}", if m.merge { "true" } else { "false" }).unwrap();
        writeln!(out, "  dump: {}", if m.dump { "true" } else { "false" }).unwrap();
        writeln!(
            out,
            "  prealloc: {}",
            if m.prealloc { "true" } else { "false" }
        )
        .unwrap();
        writeln!(out, "  share: {}", if m.share { "true" } else { "false" }).unwrap();
        if let Some(reserve) = m.reserve {
            writeln!(out, "  reserve: {}", if reserve { "true" } else { "false" }).unwrap();
        }
        writeln!(out, "  policy: {}", m.policy.name()).unwrap();
        let nodes = format_host_nodes(&m.host_nodes);
        if nodes.is_empty() {
            writeln!(out, "  host nodes:").unwrap();
        } else {
            writeln!(out, "  host nodes: {}", nodes).unwrap();
        }
    }

    writeln!(out).unwrap();

    Ok(out)
}
