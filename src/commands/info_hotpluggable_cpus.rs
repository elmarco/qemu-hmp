// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_hotpluggable_cpus(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let list = conn
        .execute(qapi_qmp::query_hotpluggable_cpus {})
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    writeln!(out, "Hotpluggable CPUs:").unwrap();
    for cpu in &list {
        // C uses PRIu64 for vcpus_count (QAPI 'int' mapped to i64 in Rust)
        writeln!(out, "  type: \"{}\"", cpu.type_).unwrap();
        writeln!(out, "  vcpus_count: \"{}\"", cpu.vcpus_count as u64).unwrap();
        if let Some(ref path) = cpu.qom_path {
            writeln!(out, "  qom_path: \"{path}\"").unwrap();
        }

        let c = &cpu.props;
        writeln!(out, "  CPUInstance Properties:").unwrap();
        // Order must match C: node, drawer, book, socket, die,
        // cluster, module, core, thread
        if let Some(v) = c.node_id {
            writeln!(out, "    node-id: \"{}\"", v as u64).unwrap();
        }
        if let Some(v) = c.drawer_id {
            writeln!(out, "    drawer-id: \"{}\"", v as u64).unwrap();
        }
        if let Some(v) = c.book_id {
            writeln!(out, "    book-id: \"{}\"", v as u64).unwrap();
        }
        if let Some(v) = c.socket_id {
            writeln!(out, "    socket-id: \"{}\"", v as u64).unwrap();
        }
        if let Some(v) = c.die_id {
            writeln!(out, "    die-id: \"{}\"", v as u64).unwrap();
        }
        if let Some(v) = c.cluster_id {
            writeln!(out, "    cluster-id: \"{}\"", v as u64).unwrap();
        }
        if let Some(v) = c.module_id {
            writeln!(out, "    module-id: \"{}\"", v as u64).unwrap();
        }
        if let Some(v) = c.core_id {
            writeln!(out, "    core-id: \"{}\"", v as u64).unwrap();
        }
        if let Some(v) = c.thread_id {
            writeln!(out, "    thread-id: \"{}\"", v as u64).unwrap();
        }
    }

    Ok(out)
}
