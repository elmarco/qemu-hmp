// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_dirty_rate(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let info = conn
        .execute(qapi_qmp::query_dirty_rate {
            calc_time_unit: Some(qapi_qmp::TimeUnit::second),
        })
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    writeln!(out, "Status: {}", info.status.name()).unwrap();
    writeln!(out, "Start Time: {} (ms)", info.start_time).unwrap();
    if info.mode == qapi_qmp::DirtyRateMeasureMode::page_sampling {
        writeln!(out, "Sample Pages: {} (per GB)", info.sample_pages).unwrap();
    }
    writeln!(out, "Period: {} (sec)", info.calc_time).unwrap();
    writeln!(out, "Mode: {}", info.mode.name()).unwrap();

    if let Some(dirty_rate) = info.dirty_rate {
        writeln!(out, "Dirty rate: {} (MB/s)", dirty_rate).unwrap();
        if let Some(ref vcpu_rates) = info.vcpu_dirty_rate {
            for rate in vcpu_rates {
                writeln!(
                    out,
                    "vcpu[{}], Dirty rate: {} (MB/s)",
                    rate.id, rate.dirty_rate
                )
                .unwrap();
            }
        }
    } else {
        writeln!(out, "Dirty rate: (not ready)").unwrap();
    }

    Ok(out)
}
