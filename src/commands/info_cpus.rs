// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

/// Extract the base fields from any CpuInfoFast variant.
pub(crate) fn cpu_info_base(cpu: &qapi::qmp::CpuInfoFast) -> &qapi::qmp::CpuInfoFastBase {
    use qapi::qmp::CpuInfoFast::*;
    match cpu {
        s390x { base, .. } => base,
        aarch64(b) | alpha(b) | arm(b) | avr(b) | cris(b) | hppa(b) | i386(b) | loongarch64(b)
        | m68k(b) | microblaze(b) | microblazeel(b) | mips(b) | mips64(b) | mips64el(b)
        | mipsel(b) | or1k(b) | ppc(b) | ppc64(b) | riscv32(b) | riscv64(b) | rx(b) | sh4(b)
        | sh4eb(b) | sparc(b) | sparc64(b) | tricore(b) | x86_64(b) | xtensa(b) | xtensaeb(b) => b,
    }
}

/// Get the CPU model name from a QOM path by querying the "type" property.
///
/// The QOM type name for CPUs follows the pattern `<model>-<target>-cpu`
/// (e.g. `qemu64-x86_64-cpu`). We strip the `-cpu` suffix and then the
/// `-<target>` suffix to get the model name.
async fn cpu_model_name(conn: &QmpConnection, qom_path: &str) -> Option<String> {
    let type_val = conn
        .execute(qapi::qmp::qom_get {
            path: qom_path.to_string(),
            property: "type".to_string(),
        })
        .await
        .ok()?;
    let type_name = type_val.as_str()?;
    // Strip the "-cpu" suffix, then the "-<target>" suffix to get the model.
    let without_cpu = type_name.strip_suffix("-cpu")?;
    // Find the last '-' to strip the target architecture portion.
    let last_dash = without_cpu.rfind('-')?;
    Some(without_cpu[..last_dash].to_string())
}

pub async fn cmd_info_cpus(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let cpus = conn
        .execute(qapi::qmp::query_cpus_fast {})
        .await
        .map_err(CmdError::from)?;

    // The current CPU index; default to 0 when not explicitly set.
    let current_cpu = conn.cpu_index().unwrap_or(0);

    let mut lines = Vec::new();
    for cpu in &cpus {
        let base = cpu_info_base(cpu);
        let marker = if base.cpu_index == current_cpu {
            "* "
        } else {
            "  "
        };
        let mut line = format!(
            "{}CPU #{}: thread_id={}",
            marker, base.cpu_index, base.thread_id
        );
        if let Some(model) = cpu_model_name(conn, &base.qom_path).await {
            line.push_str(&format!(" model={}", model));
        }
        lines.push(line);
    }
    Ok(lines.join("\n"))
}
