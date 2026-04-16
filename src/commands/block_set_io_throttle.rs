// SPDX-License-Identifier: GPL-2.0-or-later

#![allow(deprecated)]

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_expr, require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_block_set_io_throttle(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let device = require_str(args, "device")?;
    let bps = require_expr(conn, args, "bps").await?;
    let bps_rd = require_expr(conn, args, "bps_rd").await?;
    let bps_wr = require_expr(conn, args, "bps_wr").await?;
    let iops = require_expr(conn, args, "iops").await?;
    let iops_rd = require_expr(conn, args, "iops_rd").await?;
    let iops_wr = require_expr(conn, args, "iops_wr").await?;

    let throttle = qapi_qmp::BlockIOThrottle {
        id: None,
        device: Some(device),
        bps,
        bps_rd,
        bps_wr,
        iops,
        iops_rd,
        iops_wr,
        bps_max: None,
        bps_rd_max: None,
        bps_wr_max: None,
        bps_max_length: None,
        bps_rd_max_length: None,
        bps_wr_max_length: None,
        iops_max: None,
        iops_rd_max: None,
        iops_wr_max: None,
        iops_max_length: None,
        iops_rd_max_length: None,
        iops_wr_max_length: None,
        iops_size: None,
        group: None,
    };

    conn.execute(qapi_qmp::block_set_io_throttle(throttle))
        .await
        .map_err(CmdError::from)?;

    Ok(String::new())
}
