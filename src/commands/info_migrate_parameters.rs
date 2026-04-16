// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use serde::{Deserialize, Serialize};

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

// Define a local query-migrate-parameters command so the return type
// includes cpr-exec-command (since 10.2), which is not yet in
// qapi-rs.  All other fields come from the crate's MigrationParameters
// via #[serde(flatten)].

#[derive(Debug, Clone, Deserialize)]
struct MigrationParametersExt {
    #[serde(flatten)]
    inner: qapi_qmp::MigrationParameters,
    #[serde(rename = "cpr-exec-command", default)]
    cpr_exec_command: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
struct query_migrate_parameters_ext {}

impl qapi_qmp::QmpCommand for query_migrate_parameters_ext {}
impl qapi::Command for query_migrate_parameters_ext {
    const NAME: &'static str = "query-migrate-parameters";
    const ALLOW_OOB: bool = false;
    type Ok = MigrationParametersExt;
}

pub async fn cmd_info_migrate_parameters(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let result = conn
        .execute(query_migrate_parameters_ext {})
        .await
        .map_err(CmdError::from)?;

    let p = &result.inner;
    let mut out = String::new();

    use qapi::Enum;
    use qapi_qmp::MigrationParameter as MP;

    // Print in the exact same order as the C code
    writeln!(
        out,
        "{}: {} ms",
        MP::announce_initial.name(),
        p.announce_initial.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "{}: {} ms",
        MP::announce_max.name(),
        p.announce_max.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "{}: {}",
        MP::announce_rounds.name(),
        p.announce_rounds.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "{}: {} ms",
        MP::announce_step.name(),
        p.announce_step.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "{}: {}",
        MP::throttle_trigger_threshold.name(),
        p.throttle_trigger_threshold.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "{}: {}",
        MP::cpu_throttle_initial.name(),
        p.cpu_throttle_initial.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "{}: {}",
        MP::cpu_throttle_increment.name(),
        p.cpu_throttle_increment.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "{}: {}",
        MP::cpu_throttle_tailslow.name(),
        if p.cpu_throttle_tailslow.unwrap_or(false) {
            "on"
        } else {
            "off"
        }
    )
    .unwrap();
    writeln!(
        out,
        "{}: {}",
        MP::max_cpu_throttle.name(),
        p.max_cpu_throttle.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "{}: '{}'",
        MP::tls_creds.name(),
        p.tls_creds.as_deref().unwrap_or("")
    )
    .unwrap();
    writeln!(
        out,
        "{}: '{}'",
        MP::tls_hostname.name(),
        p.tls_hostname.as_deref().unwrap_or("")
    )
    .unwrap();
    writeln!(
        out,
        "{}: '{}'",
        MP::tls_authz.name(),
        p.tls_authz.as_deref().unwrap_or("")
    )
    .unwrap();
    writeln!(
        out,
        "{}: {} bytes/second",
        MP::max_bandwidth.name(),
        p.max_bandwidth.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "{}: {} bytes/second",
        MP::avail_switchover_bandwidth.name(),
        p.avail_switchover_bandwidth.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "{}: {} bytes/second",
        MP::max_postcopy_bandwidth.name(),
        p.max_postcopy_bandwidth.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "{}: {} ms",
        MP::downtime_limit.name(),
        p.downtime_limit.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "{}: {} ms",
        MP::x_checkpoint_delay.name(),
        p.x_checkpoint_delay.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "{}: {}",
        MP::multifd_channels.name(),
        p.multifd_channels.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "{}: {}",
        MP::multifd_compression.name(),
        p.multifd_compression
            .as_ref()
            .map(|v| v.name())
            .unwrap_or("none")
    )
    .unwrap();
    writeln!(
        out,
        "{}: {}",
        MP::zero_page_detection.name(),
        p.zero_page_detection
            .as_ref()
            .map(|v| v.name())
            .unwrap_or("none")
    )
    .unwrap();
    writeln!(
        out,
        "{}: {} bytes",
        MP::xbzrle_cache_size.name(),
        p.xbzrle_cache_size.unwrap_or(0)
    )
    .unwrap();

    // block-bitmap-mapping (optional, only printed if set)
    if let Some(ref mappings) = p.block_bitmap_mapping {
        writeln!(out, "{}:", MP::block_bitmap_mapping.name()).unwrap();
        for bmna in mappings {
            writeln!(out, "  '{}' -> '{}'", bmna.node_name, bmna.alias).unwrap();
            for bmba in &bmna.bitmaps {
                writeln!(out, "    '{}' -> '{}'", bmba.name, bmba.alias).unwrap();
            }
        }
    }

    writeln!(
        out,
        "{}: {} ms",
        MP::x_vcpu_dirty_limit_period.name(),
        p.x_vcpu_dirty_limit_period.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "{}: {} MB/s",
        MP::vcpu_dirty_limit.name(),
        p.vcpu_dirty_limit.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "{}: {}",
        MP::mode.name(),
        p.mode.as_ref().map(|v| v.name()).unwrap_or("normal")
    )
    .unwrap();

    if let Some(direct_io) = p.direct_io {
        writeln!(
            out,
            "{}: {}",
            MP::direct_io.name(),
            if direct_io { "on" } else { "off" }
        )
        .unwrap();
    }

    // cpr-exec-command (since 10.2, not in qapi-rs crate)
    write!(out, "cpr-exec-command:").unwrap();
    if let Some(ref args) = result.cpr_exec_command {
        for arg in args {
            write!(out, " {}", arg).unwrap();
        }
    }
    writeln!(out).unwrap();

    Ok(out)
}
