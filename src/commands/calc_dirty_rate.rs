// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_expr, CmdError};
use crate::expr::eval_expr;
use crate::qmp::QmpConnection;

pub async fn cmd_calc_dirty_rate(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let sec = require_expr(conn, args, "second").await?;
    let dirty_ring = matches!(args.get("dirty_ring"), Some(ArgValue::Bool(true)));
    let dirty_bitmap = matches!(args.get("dirty_bitmap"), Some(ArgValue::Bool(true)));

    if sec == 0 {
        return Ok("Incorrect period length specified!".to_string());
    }

    if dirty_ring && dirty_bitmap {
        return Ok("Either dirty ring or dirty bitmap can be specified!".to_string());
    }

    let mode = if dirty_bitmap {
        Some(qapi_qmp::DirtyRateMeasureMode::dirty_bitmap)
    } else if dirty_ring {
        Some(qapi_qmp::DirtyRateMeasureMode::dirty_ring)
    } else {
        Some(qapi_qmp::DirtyRateMeasureMode::page_sampling)
    };

    let sample_pages = match args.get("sample_pages_per_GB") {
        Some(ArgValue::Str(s)) => Some(eval_expr(s, conn).await?),
        Some(ArgValue::Int(n)) => Some(*n),
        _ => None,
    };

    conn.execute(qapi_qmp::calc_dirty_rate {
        calc_time: sec,
        calc_time_unit: None,
        sample_pages,
        mode,
    })
    .await
    .map_err(CmdError::from)?;

    Ok(format!(
        "Starting dirty rate measurement with period {sec} seconds\n\
         [Please use 'info dirty_rate' to check results]"
    ))
}
