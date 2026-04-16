// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_memory_devices(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let list = conn
        .execute(qapi_qmp::query_memory_devices {})
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    for info in &list {
        let type_name = info.type_().name();
        match info {
            qapi_qmp::MemoryDeviceInfo::dimm(w) | qapi_qmp::MemoryDeviceInfo::nvdimm(w) => {
                let di = &w.data;
                writeln!(
                    out,
                    "Memory device [{}]: \"{}\"",
                    type_name,
                    di.id.as_deref().unwrap_or("")
                )
                .unwrap();
                writeln!(out, "  addr: 0x{:x}", di.addr).unwrap();
                writeln!(out, "  slot: {}", di.slot).unwrap();
                writeln!(out, "  node: {}", di.node).unwrap();
                writeln!(out, "  size: {}", di.size as u64).unwrap();
                writeln!(out, "  memdev: {}", di.memdev).unwrap();
                writeln!(
                    out,
                    "  hotplugged: {}",
                    if di.hotplugged { "true" } else { "false" }
                )
                .unwrap();
                writeln!(
                    out,
                    "  hotpluggable: {}",
                    if di.hotpluggable { "true" } else { "false" }
                )
                .unwrap();
            }
            qapi_qmp::MemoryDeviceInfo::virtio_pmem(w) => {
                let vpi = &w.data;
                writeln!(
                    out,
                    "Memory device [{}]: \"{}\"",
                    type_name,
                    vpi.id.as_deref().unwrap_or("")
                )
                .unwrap();
                writeln!(out, "  memaddr: 0x{:x}", vpi.memaddr).unwrap();
                writeln!(out, "  size: {}", vpi.size).unwrap();
                writeln!(out, "  memdev: {}", vpi.memdev).unwrap();
            }
            qapi_qmp::MemoryDeviceInfo::virtio_mem(w) => {
                let vmi = &w.data;
                writeln!(
                    out,
                    "Memory device [{}]: \"{}\"",
                    type_name,
                    vmi.id.as_deref().unwrap_or("")
                )
                .unwrap();
                writeln!(out, "  memaddr: 0x{:x}", vmi.memaddr).unwrap();
                writeln!(out, "  node: {}", vmi.node).unwrap();
                writeln!(out, "  requested-size: {}", vmi.requested_size).unwrap();
                writeln!(out, "  size: {}", vmi.size).unwrap();
                writeln!(out, "  max-size: {}", vmi.max_size).unwrap();
                writeln!(out, "  block-size: {}", vmi.block_size).unwrap();
                writeln!(out, "  memdev: {}", vmi.memdev).unwrap();
            }
            qapi_qmp::MemoryDeviceInfo::sgx_epc(w) => {
                let se = &w.data;
                writeln!(
                    out,
                    "Memory device [{}]: \"{}\"",
                    type_name,
                    se.id.as_deref().unwrap_or("")
                )
                .unwrap();
                writeln!(out, "  memaddr: 0x{:x}", se.memaddr).unwrap();
                writeln!(out, "  size: {}", se.size).unwrap();
                writeln!(out, "  node: {}", se.node).unwrap();
                writeln!(out, "  memdev: {}", se.memdev).unwrap();
            }
            qapi_qmp::MemoryDeviceInfo::hv_balloon(w) => {
                let hi = &w.data;
                writeln!(
                    out,
                    "Memory device [{}]: \"{}\"",
                    type_name,
                    hi.id.as_deref().unwrap_or("")
                )
                .unwrap();
                if let Some(memaddr) = hi.memaddr {
                    writeln!(out, "  memaddr: 0x{:x}", memaddr).unwrap();
                }
                writeln!(out, "  max-size: {}", hi.max_size).unwrap();
                if let Some(ref memdev) = hi.memdev {
                    writeln!(out, "  memdev: {}", memdev).unwrap();
                }
            }
        }
    }

    Ok(out)
}
