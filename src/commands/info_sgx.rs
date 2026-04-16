// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_sgx(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let info = match conn.execute(qapi_qmp::query_sgx {}).await {
        Ok(i) => i,
        Err(e) => return Ok(format!("{e}\n")),
    };

    let mut out = String::new();
    writeln!(
        out,
        "SGX support: {}",
        if info.sgx { "enabled" } else { "disabled" }
    )
    .unwrap();
    writeln!(
        out,
        "SGX1 support: {}",
        if info.sgx1 { "enabled" } else { "disabled" }
    )
    .unwrap();
    writeln!(
        out,
        "SGX2 support: {}",
        if info.sgx2 { "enabled" } else { "disabled" }
    )
    .unwrap();
    writeln!(
        out,
        "FLC support: {}",
        if info.flc { "enabled" } else { "disabled" }
    )
    .unwrap();

    let mut total_size: u64 = 0;
    for section in &info.sections {
        writeln!(out, "NUMA node #{}: size={}", section.node, section.size).unwrap();
        total_size += section.size;
    }
    writeln!(out, "total size={total_size}").unwrap();

    Ok(out)
}
