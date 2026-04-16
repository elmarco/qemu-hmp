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

pub async fn cmd_info_cpus(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let cpus = conn
        .execute(qapi::qmp::query_cpus_fast {})
        .await
        .map_err(CmdError::from)?;
    let mut lines = Vec::new();
    for cpu in &cpus {
        let base = cpu_info_base(cpu);
        lines.push(format!(
            "CPU #{}: thread_id={}",
            base.cpu_index, base.thread_id
        ));
    }
    Ok(lines.join("\n"))
}
