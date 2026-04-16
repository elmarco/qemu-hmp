// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::expr::eval_expr;
use crate::qmp::QmpConnection;

pub async fn cmd_cancel_vcpu_dirty_limit(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let cpu_index = match args.get("cpu_index") {
        Some(ArgValue::Str(s)) => Some(eval_expr(s, conn).await?),
        Some(ArgValue::Int(n)) => Some(*n),
        _ => None,
    };

    conn.execute(qapi_qmp::cancel_vcpu_dirty_limit { cpu_index })
        .await
        .map_err(CmdError::from)?;

    Ok("[Please use 'info vcpu_dirty_limit' to query dirty limit for virtual CPU]".to_string())
}
