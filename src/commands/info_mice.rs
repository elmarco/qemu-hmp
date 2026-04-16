// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_mice(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let mice = conn
        .execute(qapi::qmp::query_mice {})
        .await
        .map_err(CmdError::from)?;

    if mice.is_empty() {
        return Ok("No mouse devices connected".to_string());
    }

    let mut lines = Vec::new();
    for mouse in &mice {
        lines.push(format!(
            "{} Mouse #{}: {}{}",
            if mouse.current { '*' } else { ' ' },
            mouse.index,
            mouse.name,
            if mouse.absolute { " (absolute)" } else { "" },
        ));
    }
    Ok(lines.join("\n"))
}
