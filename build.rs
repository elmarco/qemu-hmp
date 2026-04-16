// SPDX-License-Identifier: GPL-2.0-or-later
//
// build.rs — parse hmp-commands.hx and hmp-commands-info.hx at build time
// and generate a static command registry.

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Represents one parsed HMP command entry from a .hx file.
struct HxEntry {
    name: String,
    args_type: String,
    params: String,
    help: String,
    flags: String,
    doc: String,
}

/// Parse a single .hx file and return all command entries found.
fn parse_hx_file(path: &Path) -> Vec<HxEntry> {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));

    let mut entries = Vec::new();
    let mut in_srst = false;
    let mut in_entry = false;
    let mut entry_lines: Vec<String> = Vec::new();
    let mut srst_lines: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip HXCOMM comments
        if trimmed.starts_with("HXCOMM") {
            continue;
        }

        // Track SRST/ERST blocks — collect the documentation text
        if trimmed == "SRST" {
            in_srst = true;
            srst_lines.clear();
            continue;
        }
        if trimmed == "ERST" {
            in_srst = false;
            // Attach the collected doc to the most recently parsed entry.
            if let Some(entry) = entries.last_mut() {
                let entry: &mut HxEntry = entry;
                if entry.doc.is_empty() {
                    entry.doc = srst_lines.join("\n");
                }
            }
            srst_lines.clear();
            continue;
        }
        if in_srst {
            srst_lines.push(line.to_string());
            continue;
        }

        // Skip preprocessor directives
        if trimmed.starts_with('#') {
            continue;
        }

        // Skip C comments
        if trimmed.starts_with("/*") || trimmed.starts_with("*/") || trimmed.starts_with("* ") {
            continue;
        }

        // Detect entry start: a lone `{`
        if trimmed == "{" && !in_entry {
            in_entry = true;
            entry_lines.clear();
            continue;
        }

        // Detect entry end: `},` or `}`
        if in_entry && (trimmed == "}," || trimmed == "}") {
            if let Some(entry) = parse_entry(&entry_lines) {
                entries.push(entry);
            }
            in_entry = false;
            entry_lines.clear();
            continue;
        }

        if in_entry {
            entry_lines.push(line.to_string());
        }
    }

    entries
}

/// Parse the lines between `{` and `}` into an HxEntry.
fn parse_entry(lines: &[String]) -> Option<HxEntry> {
    // Join all lines to handle multi-line string concatenation
    let joined = lines.join("\n");

    let name = extract_string_field(&joined, "name")?;
    let args_type = extract_string_field(&joined, "args_type").unwrap_or_default();
    let params = extract_string_field(&joined, "params").unwrap_or_default();
    let help = extract_string_field(&joined, "help").unwrap_or_default();
    let flags = extract_string_field(&joined, "flags").unwrap_or_default();

    Some(HxEntry {
        name,
        args_type,
        params,
        help,
        flags,
        doc: String::new(),
    })
}

/// Extract a string field value from the entry text.
///
/// Handles:
/// - Simple: `.name = "value",`
/// - Multi-line concatenation: `.help = "line1" \n "line2",`
/// - Tabs used as concatenation: `.args_type = "a:s," \n\t\t "b:s",`
fn extract_string_field(text: &str, field: &str) -> Option<String> {
    // Find the field assignment: `.fieldname` followed by `=`
    let pattern = format!(".{}", field);
    let field_pos = text.find(&pattern)?;

    // Find the '=' after the field name
    let after_field = &text[field_pos + pattern.len()..];
    let eq_pos = after_field.find('=')?;
    let after_eq = &after_field[eq_pos + 1..];

    // Now collect all quoted strings until we hit a line with a different `.field`
    // or a non-continuation line
    let mut result = String::new();
    let mut chars = after_eq.chars().peekable();
    let mut found_first_quote = false;

    loop {
        match chars.next() {
            None => break,
            Some('"') => {
                found_first_quote = true;
                // Read until closing quote, handling escape sequences
                loop {
                    match chars.next() {
                        None => break,
                        Some('\\') => {
                            // Escape sequence — take the next char literally
                            // but skip \n, \t in help strings (formatting)
                            if let Some(c) = chars.next() {
                                match c {
                                    'n' => result.push('\n'),
                                    't' => result.push('\t'),
                                    '\\' => result.push('\\'),
                                    '"' => result.push('"'),
                                    _ => {
                                        result.push('\\');
                                        result.push(c);
                                    }
                                }
                            }
                        }
                        Some('"') => break,
                        Some(c) => result.push(c),
                    }
                }
            }
            Some('.') if found_first_quote => {
                // We've hit the next field assignment — stop
                break;
            }
            Some(',') if found_first_quote => {
                // Check if this comma is followed (ignoring whitespace) by a '.'
                // or end — that means end of this field
                let remaining: String = chars.clone().collect();
                let remaining_trimmed = remaining.trim_start();
                if remaining_trimmed.starts_with('.')
                    || remaining_trimmed.is_empty()
                    || remaining_trimmed.starts_with('}')
                {
                    break;
                }
                // Otherwise it might be between concatenated strings — continue
            }
            _ => {
                // Skip whitespace, newlines, etc. between concatenated strings
            }
        }
    }

    if result.is_empty() && !found_first_quote {
        None
    } else {
        Some(result)
    }
}

/// Escape a string for embedding in a Rust string literal.
fn escape_for_rust(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

fn write_entries(
    out: &mut impl Write,
    array_name: &str,
    entries: &[HxEntry],
) -> std::io::Result<()> {
    writeln!(out, "pub const {}: &[HxEntry] = &[", array_name)?;
    for entry in entries {
        writeln!(out, "    HxEntry {{")?;
        writeln!(out, "        name: \"{}\",", escape_for_rust(&entry.name))?;
        writeln!(
            out,
            "        args_type: \"{}\",",
            escape_for_rust(&entry.args_type)
        )?;
        writeln!(
            out,
            "        params: \"{}\",",
            escape_for_rust(&entry.params)
        )?;
        writeln!(out, "        help: \"{}\",", escape_for_rust(&entry.help))?;
        writeln!(out, "        flags: \"{}\",", escape_for_rust(&entry.flags))?;
        writeln!(out, "        doc: \"{}\",", escape_for_rust(&entry.doc))?;
        writeln!(out, "    }},")?;
    }
    writeln!(out, "];")?;
    Ok(())
}

/// Write a coverage report listing all parsed HMP commands with checkboxes.
fn write_coverage_report(out_dir: &Path, main_entries: &[HxEntry], info_entries: &[HxEntry]) {
    let report_path = out_dir.join("hmp_coverage_report.txt");
    let mut f = fs::File::create(&report_path).expect("Failed to create hmp_coverage_report.txt");

    let total = main_entries.len() + info_entries.len();

    writeln!(f, "HMP Command Coverage Report").unwrap();
    writeln!(f, "===========================").unwrap();
    writeln!(f).unwrap();
    writeln!(f, "Main commands: {}", main_entries.len()).unwrap();
    writeln!(f, "Info subcommands: {}", info_entries.len()).unwrap();
    writeln!(f, "Total: {}", total).unwrap();
    writeln!(f).unwrap();

    writeln!(f, "Main commands:").unwrap();
    for entry in main_entries {
        let help_summary = entry.help.lines().next().unwrap_or("");
        if help_summary.is_empty() {
            writeln!(f, "  [ ] {}", entry.name).unwrap();
        } else {
            writeln!(f, "  [ ] {} -- {}", entry.name, help_summary).unwrap();
        }
    }
    writeln!(f).unwrap();

    writeln!(f, "Info subcommands:").unwrap();
    for entry in info_entries {
        let help_summary = entry.help.lines().next().unwrap_or("");
        if help_summary.is_empty() {
            writeln!(f, "  [ ] info {}", entry.name).unwrap();
        } else {
            writeln!(f, "  [ ] info {} -- {}", entry.name, help_summary).unwrap();
        }
    }

    eprintln!(
        "build.rs: wrote coverage report to {}",
        report_path.display()
    );
}

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let schema_dir = match env::var("QEMU_SCHEMA_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => manifest_dir.join("schema"),
    };

    let hmp_commands_path = schema_dir.join("hmp-commands.hx");
    let hmp_info_commands_path = schema_dir.join("hmp-commands-info.hx");

    // Tell Cargo to re-run if these files change
    println!("cargo::rerun-if-changed={}", hmp_commands_path.display());
    println!(
        "cargo::rerun-if-changed={}",
        hmp_info_commands_path.display()
    );
    println!("cargo::rerun-if-env-changed=QEMU_SCHEMA_DIR");

    let main_entries = parse_hx_file(&hmp_commands_path);
    let info_entries = parse_hx_file(&hmp_info_commands_path);

    eprintln!(
        "build.rs: parsed {} main commands, {} info commands",
        main_entries.len(),
        info_entries.len()
    );

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let out_path = out_dir.join("generated_registry.rs");

    let mut out_file = fs::File::create(&out_path).expect("Failed to create generated_registry.rs");

    writeln!(out_file, "// Auto-generated by build.rs — do not edit").unwrap();
    writeln!(out_file).unwrap();

    write_entries(&mut out_file, "HMP_COMMANDS", &main_entries).unwrap();
    writeln!(out_file).unwrap();
    write_entries(&mut out_file, "HMP_INFO_COMMANDS", &info_entries).unwrap();

    write_coverage_report(&out_dir, &main_entries, &info_entries);
}
