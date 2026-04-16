// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_announce_self(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    // Get the announce timing defaults from migration parameters,
    // matching the C handler which clones migrate_announce_params().
    let migrate_params = conn
        .execute(qapi_qmp::query_migrate_parameters {})
        .await
        .map_err(CmdError::from)?;

    let initial = migrate_params.announce_initial.unwrap_or(50) as i64;
    let max = migrate_params.announce_max.unwrap_or(550) as i64;
    let rounds = migrate_params.announce_rounds.unwrap_or(5) as i64;
    let step = migrate_params.announce_step.unwrap_or(100) as i64;

    // Parse optional interfaces (comma-separated) and id.
    let interfaces = match args.get("interfaces") {
        Some(ArgValue::Str(s)) if !s.is_empty() => {
            Some(s.split(',').map(|i| i.to_string()).collect::<Vec<_>>())
        }
        _ => None,
    };

    let id = match args.get("id") {
        Some(ArgValue::Str(s)) => Some(s.clone()),
        _ => None,
    };

    conn.execute(qapi_qmp::announce_self(qapi_qmp::AnnounceParameters {
        initial,
        max,
        rounds,
        step,
        interfaces,
        id,
    }))
    .await
    .map_err(CmdError::from)?;

    Ok(String::new())
}
