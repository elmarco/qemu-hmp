// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_iothreads(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let list = conn
        .execute(qapi_qmp::query_iothreads {})
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    for info in &list {
        writeln!(out, "{}:", info.id).unwrap();
        writeln!(out, "  thread_id={}", info.thread_id).unwrap();
        writeln!(out, "  poll-max-ns={}", info.poll_max_ns).unwrap();
        writeln!(out, "  poll-grow={}", info.poll_grow).unwrap();
        writeln!(out, "  poll-shrink={}", info.poll_shrink).unwrap();
        writeln!(out, "  aio-max-batch={}", info.aio_max_batch).unwrap();
    }

    Ok(out)
}
