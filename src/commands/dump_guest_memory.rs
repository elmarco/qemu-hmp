// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use qapi::qmp::DumpGuestMemoryFormat;

use crate::args::ArgValue;
use crate::commands::{opt_bool, require_str, CmdError};
use crate::expr::eval_expr;
use crate::qmp::QmpConnection;

pub async fn cmd_dump_guest_memory(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let paging = opt_bool(args, "paging");
    let detach = opt_bool(args, "detach");
    let win_dmp = opt_bool(args, "windmp");
    let zlib = opt_bool(args, "zlib");
    let lzo = opt_bool(args, "lzo");
    let snappy = opt_bool(args, "snappy");
    let raw = opt_bool(args, "raw");
    let filename = require_str(args, "filename")?;

    if (zlib as u8 + lzo as u8 + snappy as u8 + win_dmp as u8) > 1 {
        return Err(CmdError::Command(
            "only one of '-z|-l|-s|-w' can be set".to_string(),
        ));
    }

    let dump_format = if win_dmp {
        DumpGuestMemoryFormat::win_dmp
    } else if zlib {
        if raw {
            DumpGuestMemoryFormat::kdump_raw_zlib
        } else {
            DumpGuestMemoryFormat::kdump_zlib
        }
    } else if lzo {
        if raw {
            DumpGuestMemoryFormat::kdump_raw_lzo
        } else {
            DumpGuestMemoryFormat::kdump_lzo
        }
    } else if snappy {
        if raw {
            DumpGuestMemoryFormat::kdump_raw_snappy
        } else {
            DumpGuestMemoryFormat::kdump_snappy
        }
    } else {
        DumpGuestMemoryFormat::elf
    };

    let begin = match args.get("begin") {
        Some(ArgValue::Str(s)) => Some(eval_expr(s, conn).await?),
        Some(ArgValue::Int(n)) => Some(*n),
        _ => None,
    };
    let length = match args.get("length") {
        Some(ArgValue::Str(s)) => Some(eval_expr(s, conn).await?),
        Some(ArgValue::Int(n)) => Some(*n),
        _ => None,
    };

    let protocol = format!("file:{filename}");

    conn.execute(qapi::qmp::dump_guest_memory {
        paging,
        protocol,
        detach: Some(detach),
        begin,
        length,
        format: Some(dump_format),
    })
    .await
    .map_err(CmdError::from)?;

    Ok(String::new())
}
