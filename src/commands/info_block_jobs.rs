// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

fn base(job: &qapi_qmp::BlockJobInfo) -> &qapi_qmp::BlockJobInfoBase {
    match job {
        qapi_qmp::BlockJobInfo::mirror { base, .. } => base,
        qapi_qmp::BlockJobInfo::commit(b)
        | qapi_qmp::BlockJobInfo::stream(b)
        | qapi_qmp::BlockJobInfo::backup(b)
        | qapi_qmp::BlockJobInfo::create(b)
        | qapi_qmp::BlockJobInfo::amend(b)
        | qapi_qmp::BlockJobInfo::snapshot_load(b)
        | qapi_qmp::BlockJobInfo::snapshot_save(b)
        | qapi_qmp::BlockJobInfo::snapshot_delete(b) => b,
    }
}

pub async fn cmd_info_block_jobs(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let list = conn
        .execute(qapi_qmp::query_block_jobs {})
        .await
        .map_err(CmdError::from)?;

    if list.is_empty() {
        return Ok("No active jobs\n".to_string());
    }

    let mut out = String::new();
    for job in &list {
        let b = base(job);
        if job.type_() == qapi_qmp::JobType::stream {
            writeln!(
                out,
                "Streaming device {}: Completed {} of {} bytes, speed limit {} bytes/s",
                b.device, b.offset, b.len, b.speed
            )
            .unwrap();
        } else {
            writeln!(
                out,
                "Type {}, device {}: Completed {} of {} bytes, speed limit {} bytes/s",
                job.type_().name(),
                b.device,
                b.offset,
                b.len,
                b.speed
            )
            .unwrap();
        }
    }

    Ok(out)
}
