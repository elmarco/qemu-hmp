// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

const SEV_POLICY_NODBG: u32 = 0x1;
const SEV_POLICY_NOKS: u32 = 0x2;
const SEV_SNP_POLICY_SMT: u64 = 0x10000;
const SEV_SNP_POLICY_DBG: u64 = 0x80000;

pub async fn cmd_info_sev(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let info = match conn.execute(qapi_qmp::query_sev {}).await {
        Ok(i) => i,
        Err(_) => return Ok("SEV is not enabled\n".to_string()),
    };

    let (base, sev_type) = match &info {
        qapi_qmp::SevInfo::sev { base, .. } => (base, qapi_qmp::SevGuestType::sev),
        qapi_qmp::SevInfo::sev_snp { base, .. } => (base, qapi_qmp::SevGuestType::sev_snp),
    };

    if !base.enabled {
        return Ok("SEV is not enabled\n".to_string());
    }

    let mut out = String::new();
    writeln!(out, "SEV type: {}", sev_type.name()).unwrap();
    writeln!(out, "state: {}", base.state.name()).unwrap();
    writeln!(out, "build: {}", base.build_id).unwrap();
    writeln!(out, "api version: {}.{}", base.api_major, base.api_minor).unwrap();

    match &info {
        qapi_qmp::SevInfo::sev_snp { sev_snp, .. } => {
            writeln!(
                out,
                "debug: {}",
                if sev_snp.snp_policy & SEV_SNP_POLICY_DBG != 0 {
                    "on"
                } else {
                    "off"
                }
            )
            .unwrap();
            writeln!(
                out,
                "SMT allowed: {}",
                if sev_snp.snp_policy & SEV_SNP_POLICY_SMT != 0 {
                    "on"
                } else {
                    "off"
                }
            )
            .unwrap();
        }
        qapi_qmp::SevInfo::sev { sev, .. } => {
            writeln!(out, "handle: {}", sev.handle).unwrap();
            writeln!(
                out,
                "debug: {}",
                if sev.policy & SEV_POLICY_NODBG != 0 {
                    "off"
                } else {
                    "on"
                }
            )
            .unwrap();
            writeln!(
                out,
                "key-sharing: {}",
                if sev.policy & SEV_POLICY_NOKS != 0 {
                    "off"
                } else {
                    "on"
                }
            )
            .unwrap();
        }
    }

    Ok(out)
}
