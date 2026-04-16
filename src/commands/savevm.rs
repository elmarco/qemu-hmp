// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

/// Find snapshot-capable (qcow2, writable) block device node-names.
///
/// Returns `(vmstate, devices)` where `vmstate` is the first suitable
/// node-name and `devices` is the full list.
pub(super) async fn find_snapshot_devices(
    conn: &QmpConnection,
) -> Result<(String, Vec<String>), CmdError> {
    let blocks = conn
        .execute(qapi::qmp::query_block {})
        .await
        .map_err(CmdError::from)?;

    let mut devices = Vec::new();
    for blk in &blocks {
        if let Some(ref info) = blk.inserted {
            // Only qcow2 supports internal snapshots, and it must be writable
            if info.drv == "qcow2" && !info.ro {
                if let Some(ref node) = info.node_name {
                    devices.push(node.clone());
                }
            }
        }
    }

    if devices.is_empty() {
        return Err(CmdError::Command(
            "No block device supports snapshots".to_string(),
        ));
    }

    let vmstate = devices[0].clone();
    Ok((vmstate, devices))
}

/// Wait for a job to reach "concluded" status, then dismiss it.
///
/// Returns the error string from the job if it failed, or Ok(()) on success.
pub(super) async fn wait_for_job(conn: &QmpConnection, job_id: &str) -> Result<(), CmdError> {
    loop {
        let jobs = conn
            .execute(qapi::qmp::query_jobs {})
            .await
            .map_err(CmdError::from)?;

        let job = jobs.iter().find(|j| j.id == job_id);
        match job {
            Some(j) => match j.status {
                qapi::qmp::JobStatus::concluded => {
                    let err = j.error.clone();
                    // Dismiss the job so it's cleaned up
                    let _ = conn
                        .execute(qapi::qmp::job_dismiss {
                            id: job_id.to_string(),
                        })
                        .await;
                    return match err {
                        Some(msg) => Err(CmdError::Command(msg)),
                        None => Ok(()),
                    };
                }
                qapi::qmp::JobStatus::aborting => {
                    // Will transition to concluded soon, keep polling
                }
                _ => {}
            },
            None => {
                // Job disappeared (already dismissed or never created)
                return Err(CmdError::Command(format!("job '{job_id}' not found")));
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

pub async fn cmd_savevm(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let tag = match args.get("name") {
        Some(ArgValue::Str(s)) => s.clone(),
        _ => {
            // Generate auto-tag like QEMU's save_snapshot: vm-YYYYMMDDHHMMSS
            let now = chrono::Local::now();
            now.format("vm-%Y%m%d%H%M%S").to_string()
        }
    };

    let (vmstate, devices) = find_snapshot_devices(conn).await?;

    // snapshot-save uses overwrite=false internally, so if a snapshot
    // with this tag already exists we must delete it first (the built-in
    // HMP savevm uses overwrite=true).
    let job_id = format!("savevm-del-{}", std::process::id());
    // Try to delete — ignore errors (snapshot may not exist).
    if conn
        .execute(qapi::qmp::snapshot_delete {
            job_id: job_id.clone(),
            tag: tag.clone(),
            devices: devices.clone(),
        })
        .await
        .is_ok()
    {
        let _ = wait_for_job(conn, &job_id).await;
    }

    let job_id = format!("savevm-{}", std::process::id());
    conn.execute(qapi::qmp::snapshot_save {
        job_id: job_id.clone(),
        tag,
        vmstate,
        devices,
    })
    .await
    .map_err(CmdError::from)?;

    wait_for_job(conn, &job_id).await?;

    Ok(String::new())
}
