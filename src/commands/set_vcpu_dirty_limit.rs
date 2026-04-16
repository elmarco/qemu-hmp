// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_expr, CmdError};
use crate::expr::eval_expr;
use crate::qmp::QmpConnection;

pub async fn cmd_set_vcpu_dirty_limit(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let dirty_rate = require_expr(conn, args, "dirty_rate").await?;

    if dirty_rate < 0 {
        return Err(CmdError::Command(format!(
            "invalid dirty page limit {dirty_rate}"
        )));
    }

    let cpu_index = match args.get("cpu_index") {
        Some(ArgValue::Str(s)) => Some(eval_expr(s, conn).await?),
        Some(ArgValue::Int(n)) => Some(*n),
        _ => None,
    };

    conn.execute(qapi_qmp::set_vcpu_dirty_limit {
        cpu_index,
        dirty_rate: dirty_rate as u64,
    })
    .await
    .map_err(CmdError::from)?;

    Ok(String::new())
}
