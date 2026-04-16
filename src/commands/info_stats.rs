// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

/// SI prefixes for exponents of 10, multiples of 3 from -18 to 18.
fn si_prefix(exp10: i16) -> &'static str {
    const PREFIXES: &[&str] = &[
        "a", "f", "p", "n", "u", "m", "", "K", "M", "G", "T", "P", "E",
    ];
    let idx = ((exp10 + 18) / 3) as usize;
    PREFIXES[idx]
}

/// IEC binary prefixes for exponents of 2, multiples of 10 from 0 to 60.
fn iec_binary_prefix(exp2: i16) -> &'static str {
    const PREFIXES: &[&str] = &["", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei"];
    let idx = (exp2 / 10) as usize;
    PREFIXES[idx]
}

fn print_stats_schema_value(out: &mut String, value: &qapi_qmp::StatsSchemaValue) {
    let has_unit = value.unit.is_some();
    let exponent = value.exponent;

    write!(
        out,
        "    {} ({}{}",
        value.name,
        value.type_.name(),
        if has_unit || exponent != 0 { ", " } else { "" }
    )
    .unwrap();

    let mut unit_str: Option<&str> = None;

    if let Some(ref unit) = value.unit {
        match unit {
            qapi_qmp::StatsUnit::seconds => unit_str = Some("s"),
            qapi_qmp::StatsUnit::bytes => unit_str = Some("B"),
            _ => {}
        }
    }

    let base = value.base.unwrap_or(10) as i16;

    if unit_str.is_some() && base == 10 && (-18..=18).contains(&exponent) && exponent % 3 == 0 {
        write!(out, "{}", si_prefix(exponent)).unwrap();
    } else if unit_str.is_some() && base == 2 && (0..=60).contains(&exponent) && exponent % 10 == 0
    {
        write!(out, "{}", iec_binary_prefix(exponent)).unwrap();
    } else if exponent != 0 {
        write!(
            out,
            "* {}^{}{}",
            base,
            exponent,
            if has_unit { " " } else { "" }
        )
        .unwrap();
        unit_str = None;
    }

    if has_unit {
        let unit = value.unit.as_ref().unwrap();
        if let Some(u) = unit_str {
            write!(out, "{u}").unwrap();
        } else {
            write!(out, "{}", unit.name()).unwrap();
        }
    }

    if value.type_ == qapi_qmp::StatsType::linear_histogram {
        if let Some(bucket_size) = value.bucket_size {
            write!(out, ", bucket size={bucket_size}").unwrap();
        }
    }
    write!(out, ")").unwrap();
}

fn find_schema_value_list(
    schema: &[qapi_qmp::StatsSchema],
    provider: qapi_qmp::StatsProvider,
    target: qapi_qmp::StatsTarget,
) -> Option<&[qapi_qmp::StatsSchemaValue]> {
    for entry in schema {
        if entry.provider == provider && entry.target == target {
            return Some(&entry.stats);
        }
    }
    None
}

fn print_stats_results(
    out: &mut String,
    target: qapi_qmp::StatsTarget,
    show_provider: bool,
    result: &qapi_qmp::StatsResult,
    schema: &[qapi_qmp::StatsSchema],
) {
    let Some(schema_values) = find_schema_value_list(schema, result.provider, target) else {
        writeln!(
            out,
            "failed to find schema list for {}",
            result.provider.name()
        )
        .unwrap();
        return;
    };

    if show_provider {
        writeln!(out, "provider: {}", result.provider.name()).unwrap();
    }

    let mut schema_idx = 0;
    for stats in &result.stats {
        // Find matching schema entry
        while schema_idx < schema_values.len() && schema_values[schema_idx].name != stats.name {
            schema_idx += 1;
        }
        if schema_idx >= schema_values.len() {
            writeln!(out, "failed to find schema entry for {}", stats.name).unwrap();
            return;
        }

        print_stats_schema_value(out, &schema_values[schema_idx]);

        match &stats.value {
            qapi_qmp::StatsValue::scalar(v) => {
                writeln!(out, ": {}", *v as i64).unwrap();
            }
            qapi_qmp::StatsValue::boolean(b) => {
                writeln!(out, ": {}", if *b { "yes" } else { "no" }).unwrap();
            }
            qapi_qmp::StatsValue::list(list) => {
                write!(out, ": ").unwrap();
                for (i, v) in list.iter().enumerate() {
                    write!(out, "[{}]={} ", i + 1, *v as i64).unwrap();
                }
                writeln!(out).unwrap();
            }
        }
    }
}

/// All known StatsProvider variants, used to iterate when no specific
/// provider is requested.
const ALL_PROVIDERS: &[qapi_qmp::StatsProvider] = &[
    qapi_qmp::StatsProvider::kvm,
    qapi_qmp::StatsProvider::cryptodev,
];

pub async fn cmd_info_stats(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let target_str = match args.get("target") {
        Some(ArgValue::Str(s)) => s.clone(),
        _ => {
            return Err(CmdError::Command(
                "missing required argument 'target'".to_string(),
            ))
        }
    };

    let provider_str = match args.get("provider") {
        Some(ArgValue::Str(s)) => Some(s.clone()),
        _ => None,
    };

    let names = match args.get("names") {
        Some(ArgValue::Str(s)) => Some(s.clone()),
        _ => None,
    };

    // Parse target
    let target: qapi_qmp::StatsTarget = target_str
        .parse()
        .map_err(|_| ())
        .or(Err(()))
        .unwrap_or(qapi_qmp::StatsTarget::vm);

    // Validate target by checking the name round-trips
    if target.name() != target_str {
        return Ok(format!("invalid stats target {target_str}\n"));
    }

    // Parse provider
    let provider: Option<qapi_qmp::StatsProvider> = if let Some(ref ps) = provider_str {
        match ps.parse::<qapi_qmp::StatsProvider>() {
            Ok(p) => Some(p),
            Err(_) => return Ok(format!("invalid stats provider {ps}\n")),
        }
    } else {
        None
    };

    // Query schemas
    let schema = match conn
        .execute(qapi_qmp::query_stats_schemas { provider })
        .await
    {
        Ok(s) => s,
        Err(e) => return Err(CmdError::from(e)),
    };

    // Build names list for StatsRequest
    let names_list: Option<Vec<String>> = match &names {
        Some(n) if n != "*" => Some(n.split(',').map(|s| s.to_string()).collect()),
        _ => None,
    };

    // Build providers list
    let build_providers = names.is_some() || provider.is_some();
    let providers: Vec<qapi_qmp::StatsRequest> = if build_providers {
        let targets: Vec<qapi_qmp::StatsProvider> = if let Some(p) = provider {
            vec![p]
        } else {
            ALL_PROVIDERS.to_vec()
        };
        targets
            .into_iter()
            .map(|p| qapi_qmp::StatsRequest {
                provider: p,
                names: names_list.clone(),
            })
            .collect()
    } else {
        vec![]
    };

    // Build filter
    let filter = match target {
        qapi_qmp::StatsTarget::vm => qapi_qmp::StatsFilter::vm(providers),
        qapi_qmp::StatsTarget::vcpu => {
            // Get the current CPU's QOM path
            let cpu_index = conn.cpu_index().unwrap_or(0);
            let cpus = conn
                .execute(qapi::qmp::query_cpus_fast {})
                .await
                .map_err(CmdError::from)?;
            let qom_path = cpus
                .iter()
                .find_map(|cpu| {
                    let base = crate::commands::info_cpus::cpu_info_base(cpu);
                    if base.cpu_index == cpu_index {
                        Some(base.qom_path.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();

            let vcpu_filter = qapi_qmp::StatsVCPUFilter {
                vcpus: Some(vec![qom_path]),
            };
            qapi_qmp::StatsFilter::vcpu {
                providers: if build_providers {
                    Some(providers)
                } else {
                    None
                },
                vcpu: vcpu_filter,
            }
        }
        qapi_qmp::StatsTarget::cryptodev => qapi_qmp::StatsFilter::cryptodev(providers),
    };

    // Query stats
    let stats = match conn.execute(qapi_qmp::query_stats(filter)).await {
        Ok(s) => s,
        Err(e) => return Err(CmdError::from(e)),
    };

    let mut out = String::new();
    for entry in &stats {
        print_stats_results(&mut out, target, provider_str.is_none(), entry, &schema);
    }

    Ok(out)
}
