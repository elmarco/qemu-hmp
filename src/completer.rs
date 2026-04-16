// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::sync::Arc;

use qapi::Enum;
use reedline::{Completer, Span, Suggestion};

use crate::args::{parse_arg_defs, ArgDef, ArgType};
use crate::commands::Registry;
use crate::qmp::QmpConnection;

/// Commands that take a QOM path as their first argument.
const QOM_PATH_COMMANDS: &[&str] = &["qom-list", "qom-get", "qom-set"];

/// Common block image formats for `change` format completion.
const BLOCK_FORMATS: &[&str] = &["qcow2", "raw", "vmdk", "vdi", "vhdx", "qed", "vpc", "luks"];

/// Valid values for the `read-only-mode` argument of `change`.
const READ_ONLY_MODES: &[&str] = &["retain", "read-only", "read-write"];

/// A member of an object-add variant: name, display type, and schema type ID.
#[derive(Clone)]
struct SchemaMember {
    name: String,
    /// Human-readable type (e.g. "str", "bool", "enum").
    display_type: String,
    /// Schema type ID for enum resolution.
    type_id: String,
}

#[derive(Clone)]
struct TaggedUnionInfo {
    tag: String,
    variants: HashMap<String, String>,
}

/// Subset of schema data needed for JSON QMP completion.
struct JsonSchemaCtx<'a> {
    commands: &'a [String],
    command_args: &'a HashMap<String, Vec<SchemaMember>>,
    object_types: &'a HashMap<String, Vec<SchemaMember>>,
    tagged_unions: &'a HashMap<String, TaggedUnionInfo>,
    command_arg_type_ids: &'a HashMap<String, String>,
}

/// Cached QMP schema data (static per QEMU instance).
struct SchemaCache {
    /// Enum schema ID → member values.
    enums: HashMap<String, Vec<String>>,
    /// QOM type name (e.g. "rng-random") → list of members.
    object_add_variants: HashMap<String, Vec<SchemaMember>>,
    /// Sorted list of all QMP command names.
    qmp_commands: Vec<String>,
    /// QMP command name → argument members.
    command_args: HashMap<String, Vec<SchemaMember>>,
    /// Schema type ID → object members (for resolving nested types).
    object_types: HashMap<String, Vec<SchemaMember>>,
    /// Schema type ID → tagged union info (for variant resolution).
    tagged_unions: HashMap<String, TaggedUnionInfo>,
    /// Command name → arg-type schema ID (for tagged union lookup).
    command_arg_type_ids: HashMap<String, String>,
}

/// Tab completer for HMP commands.
///
/// Completes the first word from all known command names (main commands +
/// "info" + "help").  After `info `, completes from info subcommand names.
/// For QOM commands, completes object paths by querying QMP.
pub(crate) struct HmpCompleter {
    /// All top-level command names, including "info" and "help".
    main_names: Vec<String>,
    /// All info subcommand names.
    info_names: Vec<String>,
    /// QMP connection for live QOM path queries.
    conn: Arc<QmpConnection>,
    /// Cached QMP schema (enums + object member types).
    schema: SchemaCache,
    /// Parsed argument definitions per main command name.
    main_arg_defs: HashMap<String, Vec<ArgDef>>,
    /// Parsed argument definitions per info subcommand name.
    info_arg_defs: HashMap<String, Vec<ArgDef>>,
    /// QKeyCode names for sendkey completion.
    sendkey_names: Vec<String>,
}

impl HmpCompleter {
    pub(crate) fn new(conn: Arc<QmpConnection>, registry: &Registry) -> Self {
        let mut main_names = registry.implemented_main_commands();
        // Add built-in commands (dispatched by the registry itself).
        main_names.push("info".to_string());
        main_names.push("help".to_string());
        main_names.sort();
        main_names.dedup();

        let mut info_names = registry.implemented_info_commands();
        info_names.sort();
        info_names.dedup();

        // Pre-parse argument definitions for all commands.
        let mut main_arg_defs = HashMap::new();
        for name in &main_names {
            if let Some(spec) = registry.main_args_type(name) {
                if let Ok(defs) = parse_arg_defs(spec) {
                    if !defs.is_empty() {
                        main_arg_defs.insert(name.clone(), defs);
                    }
                }
            }
        }
        let mut info_arg_defs = HashMap::new();
        for name in &info_names {
            if let Some(spec) = registry.info_args_type(name) {
                if let Ok(defs) = parse_arg_defs(spec) {
                    if !defs.is_empty() {
                        info_arg_defs.insert(name.clone(), defs);
                    }
                }
            }
        }

        let sendkey_names: Vec<String> = qapi_qmp::QKeyCode::NAMES
            .iter()
            .map(|s| (*s).to_string())
            .collect();

        Self {
            main_names,
            info_names,
            schema: Self::build_schema_cache(&conn),
            conn,
            main_arg_defs,
            info_arg_defs,
            sendkey_names,
        }
    }

    /// Query the QMP schema once and build caches for enum values and
    /// object-add variant members.
    ///
    /// Uses raw JSON parsing instead of qapi-rs typed deserialization to
    /// avoid breakage when qapi-rs structs lag behind QEMU schema changes
    /// (e.g. deprecated fields without serde defaults).
    fn build_schema_cache(conn: &QmpConnection) -> SchemaCache {
        let handle = tokio::runtime::Handle::current();
        let result = tokio::task::block_in_place(|| {
            handle.block_on(async {
                conn.execute_raw(&serde_json::json!({"execute": "query-qmp-schema"}))
                    .await
            })
        });
        let empty = SchemaCache {
            enums: HashMap::new(),
            object_add_variants: HashMap::new(),
            qmp_commands: Vec::new(),
            command_args: HashMap::new(),
            object_types: HashMap::new(),
            tagged_unions: HashMap::new(),
            command_arg_type_ids: HashMap::new(),
        };
        let resp = match result {
            Ok(v) => v,
            Err(_) => return empty,
        };
        let Some(schema) = resp.get("return").and_then(|v| v.as_array()) else {
            return empty;
        };

        let mut id_to_name: HashMap<String, String> = HashMap::new();
        let mut enums = HashMap::new();
        let mut objects: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut object_variants: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut object_tags: HashMap<String, String> = HashMap::new();
        let mut command_arg_types: HashMap<String, String> = HashMap::new();

        for entry in schema {
            let Some(name) = entry.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            let meta_type = entry
                .get("meta-type")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            match meta_type {
                "builtin" => {
                    id_to_name.insert(name.to_string(), name.to_string());
                }
                "enum" => {
                    let values: Vec<String> = entry
                        .get("members")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                                .map(|s| s.to_string())
                                .collect()
                        })
                        .or_else(|| {
                            // Fallback to deprecated "values" array (plain strings).
                            entry.get("values").and_then(|v| v.as_array()).map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str())
                                    .map(|s| s.to_string())
                                    .collect()
                            })
                        })
                        .unwrap_or_default();
                    enums.insert(name.to_string(), values);
                }
                "object" => {
                    let members: Vec<(String, String)> = entry
                        .get("members")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|m| {
                                    let n = m.get("name")?.as_str()?;
                                    let t = m.get("type")?.as_str()?;
                                    Some((n.to_string(), t.to_string()))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    objects.insert(name.to_string(), members);
                    if let Some(variants) = entry.get("variants").and_then(|v| v.as_array()) {
                        let vars: Vec<(String, String)> = variants
                            .iter()
                            .filter_map(|v| {
                                let c = v.get("case")?.as_str()?;
                                let t = v.get("type")?.as_str()?;
                                Some((c.to_string(), t.to_string()))
                            })
                            .collect();
                        object_variants.insert(name.to_string(), vars);
                        if let Some(tag) = entry.get("tag").and_then(|v| v.as_str()) {
                            object_tags.insert(name.to_string(), tag.to_string());
                        }
                    }
                }
                "command" => {
                    let arg_type = entry.get("arg-type").and_then(|v| v.as_str()).unwrap_or("");
                    command_arg_types.insert(name.to_string(), arg_type.to_string());
                }
                _ => {}
            }
        }

        // Resolve a schema type ID to a human-readable display string.
        let resolve_display = |id: &str| -> String {
            if let Some(name) = id_to_name.get(id) {
                return name.clone();
            }
            if enums.contains_key(id) {
                return "enum".to_string();
            }
            "object".to_string()
        };

        let make_member = |name: &str, type_id: &str| -> SchemaMember {
            SchemaMember {
                name: name.to_string(),
                display_type: resolve_display(type_id),
                type_id: type_id.to_string(),
            }
        };

        // Walk: object-add → arg-type → variants → each variant's members.
        let mut object_add_variants = HashMap::new();
        if let Some(arg_type_id) = command_arg_types.get("object-add") {
            // Exclude the discriminator tag (qom-type) — it's the first
            // positional value in the keyval spec, not a named property.
            let tag = object_tags.get(arg_type_id).map(|s| s.as_str());
            let base_members: Vec<SchemaMember> = objects
                .get(arg_type_id)
                .map(|m| {
                    m.iter()
                        .filter(|(n, _)| Some(n.as_str()) != tag)
                        .map(|(n, t)| make_member(n, t))
                        .collect()
                })
                .unwrap_or_default();
            if let Some(variants) = object_variants.get(arg_type_id) {
                for (case_name, variant_type_id) in variants {
                    let mut members = base_members.clone();
                    if let Some(variant_members) = objects.get(variant_type_id) {
                        members.extend(variant_members.iter().map(|(n, t)| make_member(n, t)));
                    }
                    object_add_variants.insert(case_name.clone(), members);
                }
            }
        }

        let mut qmp_commands: Vec<String> = command_arg_types.keys().cloned().collect();
        qmp_commands.sort();

        let mut command_args = HashMap::new();
        for (cmd_name, arg_type_id) in &command_arg_types {
            if arg_type_id.is_empty() {
                continue;
            }
            if let Some(members) = objects.get(arg_type_id) {
                let schema_members: Vec<SchemaMember> =
                    members.iter().map(|(n, t)| make_member(n, t)).collect();
                if !schema_members.is_empty() {
                    command_args.insert(cmd_name.clone(), schema_members);
                }
            }
        }

        let mut schema_object_types = HashMap::new();
        for (type_id, members) in &objects {
            let schema_members: Vec<SchemaMember> =
                members.iter().map(|(n, t)| make_member(n, t)).collect();
            if !schema_members.is_empty() {
                schema_object_types.insert(type_id.clone(), schema_members);
            }
        }

        let mut tagged_unions_map = HashMap::new();
        for (type_id, tag) in &object_tags {
            if let Some(variants) = object_variants.get(type_id) {
                let variant_map: HashMap<String, String> = variants
                    .iter()
                    .map(|(case, vid)| (case.clone(), vid.clone()))
                    .collect();
                tagged_unions_map.insert(
                    type_id.clone(),
                    TaggedUnionInfo {
                        tag: tag.clone(),
                        variants: variant_map,
                    },
                );
            }
        }

        let command_arg_type_ids: HashMap<String, String> = command_arg_types
            .iter()
            .filter(|(_, id)| !id.is_empty())
            .map(|(cmd, id)| (cmd.clone(), id.clone()))
            .collect();

        SchemaCache {
            enums,
            object_add_variants,
            qmp_commands,
            command_args,
            object_types: schema_object_types,
            tagged_unions: tagged_unions_map,
            command_arg_type_ids,
        }
    }

    /// Execute a qom-list query synchronously from the async runtime.
    fn qom_list(&self, path: &str) -> Vec<(String, String)> {
        let conn = self.conn.clone();
        let path = path.to_string();
        let handle = tokio::runtime::Handle::current();
        let result = tokio::task::block_in_place(|| {
            handle.block_on(async { conn.execute(qapi::qmp::qom_list { path }).await })
        });
        match result {
            Ok(props) => props.into_iter().map(|p| (p.name, p.type_)).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Complete a QOM path argument.
    fn complete_qom_path(&self, partial: &str, span_start: usize, pos: usize) -> Vec<Suggestion> {
        // Split into parent path and the prefix being typed.
        // "/machine/un" → parent="/machine", prefix="un"
        // "/machine/"   → parent="/machine", prefix=""
        // "/"           → parent="/",        prefix=""
        // ""            → suggest "/"
        if partial.is_empty() {
            return vec![Suggestion {
                value: "/".to_string(),
                description: None,
                style: None,
                extra: None,
                span: Span::new(span_start, pos),
                append_whitespace: false,
                display_override: None,
                match_indices: None,
            }];
        }

        let (parent, prefix) = if partial.ends_with('/') {
            (partial.trim_end_matches('/'), "")
        } else if let Some(idx) = partial.rfind('/') {
            (&partial[..idx], &partial[idx + 1..])
        } else {
            return vec![];
        };

        // Normalise: empty parent means root "/"
        let query_path = if parent.is_empty() { "/" } else { parent };
        let props = self.qom_list(query_path);

        props
            .iter()
            .filter(|(_, ty)| ty.starts_with("child<"))
            .filter(|(name, _)| name.starts_with(prefix))
            .map(|(name, _)| {
                let full = if parent.is_empty() {
                    format!("/{name}")
                } else {
                    format!("{parent}/{name}")
                };
                Suggestion {
                    value: full,
                    description: None,
                    style: None,
                    extra: None,
                    span: Span::new(span_start, pos),
                    append_whitespace: false,
                    display_override: None,
                    match_indices: None,
                }
            })
            .collect()
    }

    /// Complete a QOM property name for a given path.
    fn complete_qom_property(
        &self,
        path: &str,
        prefix: &str,
        span_start: usize,
        pos: usize,
    ) -> Vec<Suggestion> {
        let props = self.qom_list(path);

        props
            .iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .map(|(name, ty)| Suggestion {
                value: name.clone(),
                description: Some(ty.clone()),
                style: None,
                extra: None,
                span: Span::new(span_start, pos),
                append_whitespace: true,
                display_override: None,
                match_indices: None,
            })
            .collect()
    }

    /// Complete a QOM property value for `qom-set`.
    ///
    /// Queries the property type via `qom-list` and checks the cached QMP
    /// schema.  Returns `Some(suggestions)` for recognized types:
    /// - **bool**: `true` / `false`
    /// - **enum**: member values from the QMP schema
    /// - **struct**: member names from the QMP schema
    /// - **other**: a type-aware placeholder hint
    ///
    /// Returns `None` only when `qom-list` fails or the property is not
    /// found, so the caller can fall through to generic completion.
    fn complete_qom_value(
        &self,
        path: &str,
        property: &str,
        prefix: &str,
        span_start: usize,
        pos: usize,
    ) -> Option<Vec<Suggestion>> {
        let props = self.qom_list(path);
        let prop_type = props
            .iter()
            .find(|(name, _)| name == property)
            .map(|(_, ty)| ty.as_str())?;

        let span = Span::new(span_start, pos);
        let description = Some(prop_type.to_string());

        if prop_type == "bool" {
            return Some(
                ["true", "false"]
                    .iter()
                    .filter(|v| v.starts_with(prefix))
                    .map(|v| Suggestion {
                        value: v.to_string(),
                        description: description.clone(),
                        style: None,
                        extra: None,
                        span,
                        append_whitespace: true,
                        display_override: None,
                        match_indices: None,
                    })
                    .collect(),
            );
        }

        if let Some(enum_values) = self.schema.enums.get(prop_type) {
            return Some(
                enum_values
                    .iter()
                    .filter(|v| v.starts_with(prefix))
                    .map(|v| Suggestion {
                        value: v.clone(),
                        description: description.clone(),
                        style: None,
                        extra: None,
                        span,
                        append_whitespace: true,
                        display_override: None,
                        match_indices: None,
                    })
                    .collect(),
            );
        }

        // Struct types: show member names from the schema.
        if let Some(members) = self.schema.object_types.get(prop_type) {
            return Some(
                members
                    .iter()
                    .filter(|m| m.name.starts_with(prefix))
                    .map(|m| Suggestion {
                        value: m.name.clone(),
                        description: Some(m.display_type.clone()),
                        style: None,
                        extra: None,
                        span,
                        append_whitespace: true,
                        display_override: None,
                        match_indices: None,
                    })
                    .collect(),
            );
        }

        // Known type but no specific values — show a type-aware hint.
        Some(vec![Suggestion {
            value: prefix.to_string(),
            description,
            style: None,
            extra: None,
            span,
            append_whitespace: false,
            display_override: Some("<value>".to_string()),
            match_indices: None,
        }])
    }

    /// Query user-creatable QOM type names.
    fn user_creatable_types(&self) -> Vec<String> {
        let conn = self.conn.clone();
        let handle = tokio::runtime::Handle::current();
        let result = tokio::task::block_in_place(|| {
            handle.block_on(async {
                conn.execute(qapi::qmp::qom_list_types {
                    implements: Some("user-creatable".into()),
                    abstract_: Some(false),
                })
                .await
            })
        });
        match result {
            Ok(types) => types.into_iter().map(|t| t.name).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Look up enum values from the cached QMP schema.
    fn enum_values(&self, type_name: &str) -> Option<&[String]> {
        self.schema.enums.get(type_name).map(|v| v.as_slice())
    }

    /// Look up object-add variant members from the cached QMP schema.
    fn schema_members(&self, typename: &str) -> Vec<SchemaMember> {
        self.schema
            .object_add_variants
            .get(typename)
            .cloned()
            .unwrap_or_default()
    }

    /// Complete an object_add keyval spec: `type,id=str,prop=val,...`
    fn complete_object_add(&self, spec: &str, span_start: usize, pos: usize) -> Vec<Suggestion> {
        complete_object_add_impl(
            spec,
            span_start,
            pos,
            &self.user_creatable_types(),
            |typename| self.schema_members(typename),
            |type_name| self.enum_values(type_name),
        )
    }

    /// Complete JSON QMP commands (`{"execute": "...", "arguments": {...}}`).
    fn complete_json(&self, line: &str, pos: usize) -> Vec<Suggestion> {
        let schema = JsonSchemaCtx {
            commands: &self.schema.qmp_commands,
            command_args: &self.schema.command_args,
            object_types: &self.schema.object_types,
            tagged_unions: &self.schema.tagged_unions,
            command_arg_type_ids: &self.schema.command_arg_type_ids,
        };
        complete_json_impl(line, pos, &schema, |type_name| self.enum_values(type_name))
    }

    /// Context-sensitive completion for the `change` command.
    fn complete_change(
        &self,
        arg_tokens: &[&str],
        trailing_space: bool,
        prefix: &str,
        span_start: usize,
        pos: usize,
    ) -> Vec<Suggestion> {
        complete_change_impl(
            arg_tokens,
            trailing_space,
            prefix,
            span_start,
            pos,
            || self.block_device_names(),
            complete_filenames,
        )
    }

    /// Query block device names from QMP.
    fn block_device_names(&self) -> Vec<String> {
        let conn = self.conn.clone();
        let handle = tokio::runtime::Handle::current();
        let result = tokio::task::block_in_place(|| {
            handle.block_on(async { conn.execute(qapi::qmp::query_block {}).await })
        });
        match result {
            Ok(blocks) => blocks.into_iter().map(|b| b.device).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Generic argument completion based on the `args_type` spec.
    ///
    /// Determines which argument position the cursor is at, then offers
    /// type-appropriate suggestions: flag strings for flags, "on"/"off"
    /// for booleans, block device names for block device args, etc.
    ///
    /// `extra_block_devices` provides additional completions for
    /// BlockDevice arguments (e.g. `"all"` for the `commit` command).
    #[allow(clippy::too_many_arguments)]
    fn complete_args(
        &self,
        defs: &[ArgDef],
        arg_tokens: &[&str],
        trailing_space: bool,
        prefix: &str,
        span_start: usize,
        pos: usize,
        extra_block_devices: &[&str],
    ) -> Vec<Suggestion> {
        let extra = extra_block_devices.to_vec();
        complete_args_impl(
            defs,
            arg_tokens,
            trailing_space,
            prefix,
            span_start,
            pos,
            || {
                let mut devs = self.block_device_names();
                devs.extend(extra.iter().map(|s| s.to_string()));
                devs
            },
            complete_filenames,
        )
    }
}

/// Pure logic for object_add keyval completion, testable without QMP.
///
/// `props_fn` returns schema members for a given type.
/// `enum_fn` returns enum member names if the type ID is an enum.
fn complete_object_add_impl<'a>(
    spec: &str,
    span_start: usize,
    pos: usize,
    types: &[String],
    props_fn: impl Fn(&str) -> Vec<SchemaMember>,
    enum_fn: impl Fn(&str) -> Option<&'a [String]>,
) -> Vec<Suggestion> {
    let last_comma = spec.rfind(',');

    match last_comma {
        None => {
            // No comma yet — completing the type name (first positional).
            let prefix = spec;
            types
                .iter()
                .filter(|t| t.starts_with(prefix))
                .map(|t| Suggestion {
                    value: t.clone(),
                    description: None,
                    style: None,
                    extra: None,
                    span: Span::new(span_start, pos),
                    append_whitespace: false,
                    display_override: None,
                    match_indices: None,
                })
                .collect()
        }
        Some(idx) => {
            // After a comma — determine if we're completing a key or a value.
            // Extract the type from the first part.
            let first_part = &spec[..spec.find(',').unwrap_or(spec.len())];
            let typename = first_part
                .split_once('=')
                .map(|(_, v)| v)
                .unwrap_or(first_part);

            let current = &spec[idx + 1..];

            if let Some((key, value_prefix)) = current.split_once('=') {
                // After '=' — completing a property value.
                // Look up the property's schema type ID, then check if it's an enum.
                let props = props_fn(typename);
                let member = props.iter().find(|m| m.name == key);
                if let Some(member) = member {
                    if let Some(values) = enum_fn(&member.type_id) {
                        let val_start = span_start + idx + 1 + key.len() + 1;
                        return values
                            .iter()
                            .filter(|v| v.starts_with(value_prefix))
                            .map(|v| Suggestion {
                                value: v.clone(),
                                description: Some(member.display_type.clone()),
                                style: None,
                                extra: None,
                                span: Span::new(val_start, pos),
                                append_whitespace: false,
                                display_override: None,
                                match_indices: None,
                            })
                            .collect();
                    }
                }
                return vec![];
            }

            // No '=' yet — completing a property key.
            // Collect already-used keys to exclude them.
            let used: Vec<&str> = spec
                .split(',')
                .filter_map(|part| part.split_once('=').map(|(k, _)| k))
                .collect();

            let props = props_fn(typename);
            let key_start = span_start + idx + 1;
            props
                .iter()
                .filter(|m| m.name.starts_with(current))
                .filter(|m| !used.contains(&m.name.as_str()))
                .map(|m| Suggestion {
                    value: format!("{}=", m.name),
                    description: Some(m.display_type.clone()),
                    style: None,
                    extra: None,
                    span: Span::new(key_start, pos),
                    append_whitespace: false,
                    display_override: None,
                    match_indices: None,
                })
                .collect()
        }
    }
}

/// Pure logic for `change` command completion, testable without QMP.
///
/// The `change` command is context-sensitive: the first positional argument
/// (device) determines what the remaining arguments should complete to.
///
/// - **device** (pos 0): block device names + `"vnc"`
/// - **target** (pos 1): filenames (block device) or `"passwd"`/`"password"` (vnc)
/// - **arg** (pos 2): image format names (block device) or password hint (vnc)
/// - **read-only-mode** (pos 3): `"retain"` / `"read-only"` / `"read-write"` (block device only)
///
/// The `-f` flag is position-independent and offered whenever not yet consumed.
fn complete_change_impl(
    arg_tokens: &[&str],
    trailing_space: bool,
    prefix: &str,
    span_start: usize,
    pos: usize,
    block_devices_fn: impl Fn() -> Vec<String>,
    filenames_fn: impl Fn(&str) -> Vec<(String, bool)>,
) -> Vec<Suggestion> {
    // Separate completed tokens from the partial being typed.
    let completed = if trailing_space || arg_tokens.is_empty() {
        arg_tokens
    } else {
        &arg_tokens[..arg_tokens.len() - 1]
    };

    // Split completed tokens into flag vs positional.
    let mut flag_seen = false;
    let mut positionals: Vec<&str> = Vec::new();
    for tok in completed {
        if *tok == "-f" && !flag_seen {
            flag_seen = true;
        } else {
            positionals.push(tok);
        }
    }

    let positional_index = positionals.len();
    let device = positionals.first().copied();
    let is_vnc = device == Some("vnc");
    let span = Span::new(span_start, pos);
    let mut suggestions = Vec::new();

    // Offer the -f flag if not yet consumed and matches prefix.
    if !flag_seen && "-f".starts_with(prefix) {
        suggestions.push(Suggestion {
            value: "-f".to_string(),
            description: Some("force".to_string()),
            style: None,
            extra: None,
            span,
            append_whitespace: true,
            display_override: None,
            match_indices: None,
        });
    }

    match positional_index {
        0 => {
            // Device: block devices + "vnc"
            let mut devices = block_devices_fn();
            devices.push("vnc".to_string());
            for d in &devices {
                if d.starts_with(prefix) {
                    suggestions.push(Suggestion {
                        value: d.clone(),
                        description: Some("device".to_string()),
                        style: None,
                        extra: None,
                        span,
                        append_whitespace: true,
                        display_override: None,
                        match_indices: None,
                    });
                }
            }
        }
        1 => {
            if is_vnc {
                // VNC: suggest "passwd" / "password"
                for v in &["passwd", "password"] {
                    if v.starts_with(prefix) {
                        suggestions.push(Suggestion {
                            value: v.to_string(),
                            description: Some("target".to_string()),
                            style: None,
                            extra: None,
                            span,
                            append_whitespace: true,
                            display_override: None,
                            match_indices: None,
                        });
                    }
                }
            } else {
                // Block device: filename completion
                let files = filenames_fn(prefix);
                for (path, is_dir) in &files {
                    suggestions.push(Suggestion {
                        value: path.clone(),
                        description: Some("target".to_string()),
                        style: None,
                        extra: None,
                        span,
                        append_whitespace: !is_dir,
                        display_override: None,
                        match_indices: None,
                    });
                }
            }
        }
        2 => {
            if is_vnc {
                // VNC password — just show a hint
                suggestions.push(Suggestion {
                    value: prefix.to_string(),
                    description: Some("string".to_string()),
                    style: None,
                    extra: None,
                    span,
                    append_whitespace: false,
                    display_override: Some("<password>".to_string()),
                    match_indices: None,
                });
            } else {
                // Block image format
                for f in BLOCK_FORMATS {
                    if f.starts_with(prefix) {
                        suggestions.push(Suggestion {
                            value: f.to_string(),
                            description: Some("format".to_string()),
                            style: None,
                            extra: None,
                            span,
                            append_whitespace: true,
                            display_override: None,
                            match_indices: None,
                        });
                    }
                }
            }
        }
        3 if !is_vnc => {
            // Read-only mode
            for m in READ_ONLY_MODES {
                if m.starts_with(prefix) {
                    suggestions.push(Suggestion {
                        value: m.to_string(),
                        description: Some("read-only-mode".to_string()),
                        style: None,
                        extra: None,
                        span,
                        append_whitespace: true,
                        display_override: None,
                        match_indices: None,
                    });
                }
            }
        }
        _ => {}
    }

    suggestions
}

/// Find all candidate argument definitions at the current cursor position.
///
/// Flags are position-independent (matching `parse_args`): any flag that
/// hasn't been consumed by a completed token is a candidate regardless of
/// where it was defined.  Positional (non-flag) arguments are resolved by
/// counting how many non-flag tokens have been completed.  When the cursor
/// is at an optional positional argument, subsequent optional arguments
/// and the first required argument are also included.
fn find_candidate_defs<'a>(
    defs: &'a [ArgDef],
    arg_tokens: &[&str],
    trailing_space: bool,
) -> Vec<&'a ArgDef> {
    // Collect all flag definitions (both boolean and string-valued).
    // The bool tracks whether this flag takes a string value.
    let all_flags: Vec<(&ArgDef, &str, bool)> = defs
        .iter()
        .filter_map(|d| match &d.arg_type {
            ArgType::Flag(s) => Some((d, s.as_str(), false)),
            ArgType::FlagStr(s) => Some((d, s.as_str(), true)),
            _ => None,
        })
        .collect();

    // Completed tokens: everything except the partial last token (if any).
    let completed = if trailing_space || arg_tokens.is_empty() {
        arg_tokens
    } else {
        &arg_tokens[..arg_tokens.len() - 1]
    };

    // Match completed tokens against flag definitions.
    let mut flag_consumed = vec![false; all_flags.len()];
    let mut positional_count: usize = 0;

    let mut skip_next = false;
    for token in completed {
        if skip_next {
            skip_next = false;
            continue; // This token is a flag's string value
        }
        let mut matched = false;
        for (i, (_, flag_str, takes_value)) in all_flags.iter().enumerate() {
            if !flag_consumed[i] && *token == *flag_str {
                flag_consumed[i] = true;
                matched = true;
                if *takes_value {
                    skip_next = true;
                }
                break;
            }
        }
        if !matched {
            positional_count += 1;
        }
    }

    let mut candidates: Vec<&ArgDef> = Vec::new();

    // 1. All unconsumed flags are always candidates.
    for (i, (def, _, _)) in all_flags.iter().enumerate() {
        if !flag_consumed[i] {
            candidates.push(def);
        }
    }

    // 2. Determine the current positional arg from the non-flag token count.
    let positional_defs: Vec<&ArgDef> = defs
        .iter()
        .filter(|d| !matches!(d.arg_type, ArgType::Flag(_) | ArgType::FlagStr(_)))
        .collect();

    let mut collecting = false;
    for (i, def) in positional_defs.iter().enumerate() {
        if collecting {
            candidates.push(def);
            if def.optional {
                continue;
            }
            break; // Required — include it but stop
        }

        if i < positional_count {
            continue; // Already consumed by a completed token
        }

        // This is the positional def at the cursor.
        candidates.push(def);
        if trailing_space || arg_tokens.is_empty() {
            // Starting a new token — also collect subsequent optional defs.
            if def.optional {
                collecting = true;
                continue;
            }
        }
        break;
    }

    candidates
}

/// Pure logic for generic argument completion, testable without QMP.
///
/// Walks the argument definitions to find candidate defs at the cursor
/// position.  When an argument is optional (including flags), subsequent
/// arguments are also offered, up to the first required argument.
///
/// Generates suggestions appropriate for each argument type:
/// - **Flag**: the flag string (e.g., `-f`)
/// - **Bool**: `on` / `off`
/// - **BlockDevice**: device names from `block_devices_fn`
/// - **Filename**: file paths from `filenames_fn`
/// - **Other types**: a type hint shown in the menu (e.g., `<size>`)
#[allow(clippy::too_many_arguments)]
fn complete_args_impl(
    defs: &[ArgDef],
    arg_tokens: &[&str],
    trailing_space: bool,
    prefix: &str,
    span_start: usize,
    pos: usize,
    block_devices_fn: impl Fn() -> Vec<String>,
    filenames_fn: impl Fn(&str) -> Vec<(String, bool)>,
) -> Vec<Suggestion> {
    let target_defs = find_candidate_defs(defs, arg_tokens, trailing_space);

    let mut suggestions = Vec::new();
    let span = Span::new(span_start, pos);

    for def in &target_defs {
        let description = Some(def.name.clone());

        match &def.arg_type {
            ArgType::Flag(flag_str) | ArgType::FlagStr(flag_str) => {
                if flag_str.starts_with(prefix) {
                    suggestions.push(Suggestion {
                        value: flag_str.clone(),
                        description,
                        style: None,
                        extra: None,
                        span,
                        append_whitespace: true,
                        display_override: None,
                        match_indices: None,
                    });
                }
            }
            ArgType::Bool => {
                for v in &["on", "off"] {
                    if v.starts_with(prefix) {
                        suggestions.push(Suggestion {
                            value: v.to_string(),
                            description: description.clone(),
                            style: None,
                            extra: None,
                            span,
                            append_whitespace: true,
                            display_override: None,
                            match_indices: None,
                        });
                    }
                }
            }
            ArgType::BlockDevice => {
                let devices = block_devices_fn();
                for d in &devices {
                    if d.starts_with(prefix) {
                        suggestions.push(Suggestion {
                            value: d.clone(),
                            description: description.clone(),
                            style: None,
                            extra: None,
                            span,
                            append_whitespace: true,
                            display_override: None,
                            match_indices: None,
                        });
                    }
                }
            }
            ArgType::Filename => {
                let files = filenames_fn(prefix);
                for (path, is_dir) in &files {
                    suggestions.push(Suggestion {
                        value: path.clone(),
                        description: description.clone(),
                        style: None,
                        extra: None,
                        span,
                        append_whitespace: !is_dir,
                        display_override: None,
                        match_indices: None,
                    });
                }
            }
            other => {
                let hint = format!("<{}>", def.name);
                let type_desc = match other {
                    ArgType::Str => "string",
                    ArgType::Int | ArgType::DotInt => "integer",
                    ArgType::Long => "expression ($reg, hex, arithmetic)",
                    ArgType::Size => "size (K/M/G/T)",
                    ArgType::Mebibytes => "megabytes",
                    ArgType::Format => "format spec",
                    ArgType::Object => "key=val,...",
                    _ => "",
                };
                suggestions.push(Suggestion {
                    value: prefix.to_string(),
                    description: Some(type_desc.to_string()),
                    style: None,
                    extra: None,
                    span,
                    append_whitespace: false,
                    display_override: Some(hint),
                    match_indices: None,
                });
            }
        }
    }

    suggestions
}

/// List files and directories matching a prefix for filename completion.
fn complete_filenames(prefix: &str) -> Vec<(String, bool)> {
    let (dir_path, file_prefix) = if prefix.is_empty() {
        (".", "")
    } else if prefix.ends_with('/') {
        (prefix.trim_end_matches('/'), "")
    } else {
        match prefix.rfind('/') {
            Some(idx) => (&prefix[..idx], &prefix[idx + 1..]),
            None => (".", prefix),
        }
    };

    let dir_path = if dir_path.is_empty() { "/" } else { dir_path };
    let show_hidden = file_prefix.starts_with('.');

    let entries = match std::fs::read_dir(dir_path) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    let mut results: Vec<(String, bool)> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') && !show_hidden {
                return None;
            }
            if !name.starts_with(file_prefix) {
                return None;
            }
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let full_path = if dir_path == "." && !prefix.contains('/') {
                name
            } else if dir_path == "/" {
                format!("/{name}")
            } else {
                format!("{dir_path}/{name}")
            };
            let full_path = if is_dir {
                format!("{full_path}/")
            } else {
                full_path
            };
            Some((full_path, is_dir))
        })
        .collect();

    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}

#[derive(Debug, Clone)]
struct NestingFrame<'a> {
    key: &'a str,
    parent_kv: Vec<(&'a str, &'a str)>,
}

/// Context for JSON QMP command completion.
enum JsonCompletionContext<'a> {
    /// The `{"execute": "` boilerplate hasn't been completed yet.
    Boilerplate { span_start: usize },
    /// The `, "arguments": {` boilerplate after a complete command name.
    ArgumentsBoilerplate { command: &'a str, span_start: usize },
    /// Completing the command name after `"execute": "`.
    CommandName { prefix: &'a str, span_start: usize },
    /// Completing an argument key inside `"arguments": {`.
    ArgKey {
        command: &'a str,
        prefix: &'a str,
        span_start: usize,
        in_quotes: bool,
        used_keys: Vec<&'a str>,
        nesting: Vec<NestingFrame<'a>>,
        level_kv: Vec<(&'a str, &'a str)>,
    },
    /// Completing an argument value after `"key": `.
    ArgValue {
        command: &'a str,
        key: &'a str,
        prefix: &'a str,
        span_start: usize,
        /// Whether the cursor is inside an opening `"`.
        in_quotes: bool,
        nesting: Vec<NestingFrame<'a>>,
        level_kv: Vec<(&'a str, &'a str)>,
    },
}

/// Find positions of commas at brace/bracket depth 0, outside strings.
fn toplevel_comma_positions(s: &str) -> Vec<usize> {
    let mut positions = Vec::new();
    let mut depth: u32 = 0;
    let mut in_string = false;
    let mut escape = false;

    for (i, c) in s.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            match c {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' | '[' => depth += 1,
            '}' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => positions.push(i),
            _ => {}
        }
    }
    positions
}

/// Extract key and optional string value from a JSON segment like `"key": "val"`.
///
/// Returns `Some((key, Some(string_value)))` for string values,
/// `Some((key, None))` for non-string values (objects, arrays, numbers, bools),
/// or `None` if the segment doesn't contain a complete key-value pair.
fn extract_segment_kv(segment: &str) -> Option<(&str, Option<&str>)> {
    let trimmed = segment.trim();
    let rest = trimmed.strip_prefix('"')?;
    let key_end = rest.find('"')?;
    let key = &rest[..key_end];
    let after_key = rest[key_end + 1..].trim_start();
    let after_colon = after_key.strip_prefix(':')?.trim_start();

    if let Some(val_content) = after_colon.strip_prefix('"') {
        let val_end = val_content.find('"')?;
        Some((key, Some(&val_content[..val_end])))
    } else if after_colon.is_empty() {
        None
    } else {
        Some((key, None))
    }
}

/// Parse argument content recursively, building nesting frames as we
/// descend into nested `{}` values.
///
/// `content` is everything after the opening `{` at the current nesting level.
/// `pos` is the cursor position in the original line (for Span calculation).
fn parse_json_args_recursive<'a>(
    content: &'a str,
    command: &'a str,
    pos: usize,
    nesting: Vec<NestingFrame<'a>>,
) -> Option<JsonCompletionContext<'a>> {
    let commas = toplevel_comma_positions(content);

    // Split content into segments at top-level commas.
    let mut segments: Vec<&str> = Vec::new();
    let mut start = 0;
    for &comma_pos in &commas {
        segments.push(&content[start..comma_pos]);
        start = comma_pos + 1;
    }
    segments.push(&content[start..]);

    // Collect used keys and string kv pairs from completed segments.
    let mut used_keys: Vec<&str> = Vec::new();
    let mut kv_pairs: Vec<(&str, &str)> = Vec::new();
    let completed = if segments.len() > 1 {
        &segments[..segments.len() - 1]
    } else {
        &[]
    };
    for seg in completed {
        if let Some((key, str_val)) = extract_segment_kv(seg) {
            used_keys.push(key);
            if let Some(v) = str_val {
                kv_pairs.push((key, v));
            }
        }
    }

    // The last segment is where the cursor sits.
    let last_seg = segments.last()?;
    let trimmed = last_seg.trim_start();

    if trimmed.is_empty() {
        return Some(JsonCompletionContext::ArgKey {
            command,
            prefix: "",
            span_start: pos,
            in_quotes: false,
            used_keys,
            nesting,
            level_kv: kv_pairs,
        });
    }

    if let Some(after_key_quote) = trimmed.strip_prefix('"') {
        if let Some(key_end) = after_key_quote.find('"') {
            let key = &after_key_quote[..key_end];
            let after_key = after_key_quote[key_end + 1..].trim_start();
            if let Some(after_colon) = after_key.strip_prefix(':') {
                let after_colon = after_colon.trim_start();

                // Nested object — recurse
                if let Some(inner) = after_colon.strip_prefix('{') {
                    let frame = NestingFrame {
                        key,
                        parent_kv: kv_pairs,
                    };
                    let mut new_nesting = nesting;
                    new_nesting.push(frame);
                    return parse_json_args_recursive(inner, command, pos, new_nesting);
                }

                // String value in progress
                if let Some(val_content) = after_colon.strip_prefix('"') {
                    if !val_content.contains('"') {
                        return Some(JsonCompletionContext::ArgValue {
                            command,
                            key,
                            prefix: val_content,
                            span_start: pos - val_content.len(),
                            in_quotes: true,
                            nesting,
                            level_kv: kv_pairs,
                        });
                    }
                }

                // Non-string, non-object, non-array value
                if !after_colon.starts_with('[') {
                    return Some(JsonCompletionContext::ArgValue {
                        command,
                        key,
                        prefix: after_colon,
                        span_start: pos - after_colon.len(),
                        in_quotes: false,
                        nesting,
                        level_kv: kv_pairs,
                    });
                }
            }
            return None;
        }
        // Key not closed — typing a key name
        return Some(JsonCompletionContext::ArgKey {
            command,
            prefix: after_key_quote,
            span_start: pos - after_key_quote.len(),
            in_quotes: true,
            used_keys,
            nesting,
            level_kv: kv_pairs,
        });
    }

    None
}

/// Parse a partial JSON line to determine the completion context.
fn parse_json_context(line: &str, pos: usize) -> Option<JsonCompletionContext<'_>> {
    if !line.trim_start().starts_with('{') {
        return None;
    }

    let brace_pos = line.find('{')?;

    let exec_idx = match line.find("\"execute\"") {
        Some(idx) => idx,
        None => {
            let after_brace = line[brace_pos + 1..].trim_start();
            if after_brace.is_empty() {
                return Some(JsonCompletionContext::Boilerplate {
                    span_start: brace_pos,
                });
            }
            if let Some(partial) = after_brace.strip_prefix('"') {
                if "execute\"".starts_with(partial) {
                    return Some(JsonCompletionContext::Boilerplate {
                        span_start: brace_pos,
                    });
                }
            }
            return None;
        }
    };

    let after_exec = line[exec_idx + "\"execute\"".len()..].trim_start();
    let after_colon = match after_exec.strip_prefix(':') {
        Some(r) => r.trim_start(),
        None => {
            if after_exec.is_empty() {
                return Some(JsonCompletionContext::Boilerplate {
                    span_start: brace_pos,
                });
            }
            return None;
        }
    };
    let after_quote = match after_colon.strip_prefix('"') {
        Some(r) => r,
        None => {
            if after_colon.is_empty() {
                return Some(JsonCompletionContext::Boilerplate {
                    span_start: brace_pos,
                });
            }
            return None;
        }
    };

    match after_quote.find('"') {
        None => Some(JsonCompletionContext::CommandName {
            prefix: after_quote,
            span_start: pos - after_quote.len(),
        }),
        Some(end_quote) => {
            let command = &after_quote[..end_quote];
            let after_cmd = &after_quote[end_quote + 1..];
            let cmd_end_pos = pos - after_cmd.len();

            let args_boilerplate = || {
                Some(JsonCompletionContext::ArgumentsBoilerplate {
                    command,
                    span_start: cmd_end_pos,
                })
            };

            if let Some(args_idx) = after_cmd.find("\"arguments\"") {
                let after_args = after_cmd[args_idx + "\"arguments\"".len()..].trim_start();
                let after_colon = match after_args.strip_prefix(':') {
                    Some(r) => r.trim_start(),
                    None => return args_boilerplate(),
                };
                let args_content = match after_colon.strip_prefix('{') {
                    Some(r) => r,
                    None => return args_boilerplate(),
                };

                return parse_json_args_recursive(args_content, command, pos, Vec::new());
            }

            // No "arguments" found — check for partial prefix
            let stripped = after_cmd.trim_start();
            if stripped.is_empty() {
                return args_boilerplate();
            }
            if let Some(after_comma) = stripped.strip_prefix(',') {
                let after_comma = after_comma.trim_start();
                if after_comma.is_empty() {
                    return args_boilerplate();
                }
                if after_comma
                    .strip_prefix('"')
                    .is_some_and(|p| "arguments\"".starts_with(p))
                {
                    return args_boilerplate();
                }
            }

            None
        }
    }
}

/// Generate JSON QMP completion suggestions.
///
/// Supports five contexts:
/// - `{"execute": "` boilerplate
/// - `, "arguments": {` boilerplate (for commands with known args)
/// - Command name after `"execute": "`
/// - Argument key inside `"arguments": {`
/// - Argument enum value after `"key": "`
fn complete_json_impl<'a>(
    line: &str,
    pos: usize,
    schema: &JsonSchemaCtx<'_>,
    enum_fn: impl Fn(&str) -> Option<&'a [String]>,
) -> Vec<Suggestion> {
    let ctx = match parse_json_context(line, pos) {
        Some(c) => c,
        None => return vec![],
    };

    let resolve_members = |command: &str,
                           nesting: &[NestingFrame<'_>],
                           level_kv: &[(&str, &str)]|
     -> Vec<SchemaMember> {
        let mut members = schema
            .command_args
            .get(command)
            .cloned()
            .unwrap_or_default();

        // Resolve tagged union at the top level using the command's arg-type ID.
        let kv_for_top = if nesting.is_empty() {
            level_kv
        } else {
            &nesting[0].parent_kv
        };
        if let Some(arg_type_id) = schema.command_arg_type_ids.get(command) {
            if let Some(info) = schema.tagged_unions.get(arg_type_id) {
                if let Some((_, tag_val)) = kv_for_top.iter().find(|(k, _)| *k == info.tag) {
                    if let Some(variant_type_id) = info.variants.get(*tag_val) {
                        if let Some(variant_members) = schema.object_types.get(variant_type_id) {
                            members.extend(variant_members.iter().cloned());
                        }
                    }
                }
            }
        }

        // Walk the nesting path, resolving types at each level.
        for (i, frame) in nesting.iter().enumerate() {
            let type_id = match members.iter().find(|m| m.name == frame.key) {
                Some(m) => m.type_id.clone(),
                None => return Vec::new(),
            };
            members = schema
                .object_types
                .get(&type_id)
                .cloned()
                .unwrap_or_default();

            if let Some(info) = schema.tagged_unions.get(&type_id) {
                let kv = if i + 1 < nesting.len() {
                    &nesting[i + 1].parent_kv
                } else {
                    level_kv
                };
                if let Some((_, tag_val)) = kv.iter().find(|(k, _)| *k == info.tag) {
                    if let Some(variant_type_id) = info.variants.get(*tag_val) {
                        if let Some(variant_members) = schema.object_types.get(variant_type_id) {
                            members.extend(variant_members.iter().cloned());
                        }
                    }
                }
            }
        }

        members
    };

    match ctx {
        JsonCompletionContext::Boilerplate { span_start } => {
            vec![Suggestion {
                value: r#"{"execute": ""#.to_string(),
                description: Some("QMP command".to_string()),
                style: None,
                extra: None,
                span: Span::new(span_start, pos),
                append_whitespace: false,
                display_override: None,
                match_indices: None,
            }]
        }
        JsonCompletionContext::ArgumentsBoilerplate {
            command,
            span_start,
        } => {
            if schema.command_args.contains_key(command) {
                vec![Suggestion {
                    value: r#", "arguments": {"#.to_string(),
                    description: Some("QMP arguments".to_string()),
                    style: None,
                    extra: None,
                    span: Span::new(span_start, pos),
                    append_whitespace: false,
                    display_override: None,
                    match_indices: None,
                }]
            } else {
                vec![]
            }
        }
        JsonCompletionContext::CommandName { prefix, span_start } => schema
            .commands
            .iter()
            .filter(|cmd| cmd.starts_with(prefix))
            .map(|cmd| Suggestion {
                value: format!("{cmd}\""),
                description: None,
                style: None,
                extra: None,
                span: Span::new(span_start, pos),
                append_whitespace: false,
                display_override: Some(cmd.clone()),
                match_indices: None,
            })
            .collect(),
        JsonCompletionContext::ArgKey {
            command,
            prefix,
            span_start,
            in_quotes,
            used_keys,
            nesting,
            level_kv,
        } => {
            let members = resolve_members(command, &nesting, &level_kv);
            members
                .iter()
                .filter(|m| m.name.starts_with(prefix))
                .filter(|m| !used_keys.contains(&m.name.as_str()))
                .map(|m| {
                    let value = if in_quotes {
                        format!("{}\": ", m.name)
                    } else {
                        format!("\"{}\": ", m.name)
                    };
                    Suggestion {
                        value,
                        description: Some(m.display_type.clone()),
                        style: None,
                        extra: None,
                        span: Span::new(span_start, pos),
                        append_whitespace: false,
                        display_override: Some(m.name.clone()),
                        match_indices: None,
                    }
                })
                .collect()
        }
        JsonCompletionContext::ArgValue {
            command,
            key,
            prefix,
            span_start,
            in_quotes,
            nesting,
            level_kv,
        } => {
            let members = resolve_members(command, &nesting, &level_kv);
            let mut suggestions = Vec::new();
            if let Some(member) = members.iter().find(|m| m.name == key) {
                if let Some(values) = enum_fn(&member.type_id) {
                    suggestions.extend(values.iter().filter(|v| v.starts_with(prefix)).map(|v| {
                        let value = if in_quotes {
                            format!("{v}\"")
                        } else {
                            format!("\"{v}\"")
                        };
                        Suggestion {
                            value,
                            description: Some(member.display_type.clone()),
                            style: None,
                            extra: None,
                            span: Span::new(span_start, pos),
                            append_whitespace: false,
                            display_override: Some(v.clone()),
                            match_indices: None,
                        }
                    }));
                } else if !in_quotes && member.display_type == "bool" {
                    for v in &["true", "false"] {
                        if v.starts_with(prefix) {
                            suggestions.push(Suggestion {
                                value: v.to_string(),
                                description: Some("bool".to_string()),
                                style: None,
                                extra: None,
                                span: Span::new(span_start, pos),
                                append_whitespace: false,
                                display_override: None,
                                match_indices: None,
                            });
                        }
                    }
                }
            }
            suggestions
        }
    }
}

impl Completer for HmpCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let line = &line[..pos];

        // JSON QMP command completion
        if line.trim_start().starts_with('{') {
            return self.complete_json(line, pos);
        }

        // Check if we are completing after "info "
        if let Some(rest) = line.strip_prefix("info ") {
            let prefix = rest.trim_start();
            // Only complete if we are still on the subcommand word (no further spaces)
            if !prefix.contains(' ') {
                let start = line.len() - prefix.len();
                return self
                    .info_names
                    .iter()
                    .filter(|name| name.starts_with(prefix))
                    .map(|name| Suggestion {
                        value: name.clone(),
                        description: None,
                        style: None,
                        extra: None,
                        span: Span::new(start, pos),
                        append_whitespace: true,
                        display_override: None,
                        match_indices: None,
                    })
                    .collect();
            }
            // Info subcommand argument completion
            let info_words: Vec<&str> = prefix.split_whitespace().collect();
            let subcmd = info_words[0];
            if let Some(defs) = self.info_arg_defs.get(subcmd) {
                let after_sub = &prefix[subcmd.len()..].trim_start();
                let arg_tokens: Vec<&str> = if after_sub.is_empty() {
                    vec![]
                } else {
                    after_sub.split_whitespace().collect()
                };
                let trailing_space = after_sub.is_empty() || after_sub.ends_with(' ');
                let partial = if trailing_space {
                    ""
                } else {
                    arg_tokens.last().copied().unwrap_or("")
                };
                let span_start = line.len() - partial.len();
                return self.complete_args(
                    defs,
                    &arg_tokens,
                    trailing_space,
                    partial,
                    span_start,
                    pos,
                    &[],
                );
            }
            return vec![];
        }

        let words: Vec<&str> = line.split_whitespace().collect();
        let first_word = words.first().copied().unwrap_or("");

        // help/? completion: offer all command names (main + info).
        if (first_word == "help" || first_word == "?") && line.contains(' ') {
            let after_cmd = line[first_word.len()..].trim_start();
            let help_words: Vec<&str> = after_cmd.split_whitespace().collect();
            let trailing_space = after_cmd.ends_with(' ');

            // "help info <partial>" → complete info subcommand names.
            if help_words.first() == Some(&"info") {
                if help_words.len() > 2 || (help_words.len() == 2 && trailing_space) {
                    return vec![];
                }
                let partial = if help_words.len() == 2 {
                    help_words[1]
                } else {
                    ""
                };
                let span_start = line.len() - partial.len();
                return self
                    .info_names
                    .iter()
                    .filter(|name| name.starts_with(partial))
                    .map(|name| Suggestion {
                        value: name.clone(),
                        description: None,
                        style: None,
                        extra: None,
                        span: Span::new(span_start, pos),
                        append_whitespace: true,
                        display_override: None,
                        match_indices: None,
                    })
                    .collect();
            }

            // Only complete the first argument.
            if help_words.len() > 1 || (help_words.len() == 1 && trailing_space) {
                return vec![];
            }

            let partial = help_words.first().copied().unwrap_or("");
            let span_start = line.len() - partial.len();

            return self
                .main_names
                .iter()
                .chain(self.info_names.iter())
                .filter(|name| name.starts_with(partial))
                .map(|name| Suggestion {
                    value: name.clone(),
                    description: None,
                    style: None,
                    extra: None,
                    span: Span::new(span_start, pos),
                    append_whitespace: true,
                    display_override: None,
                    match_indices: None,
                })
                .collect();
        }

        // QOM command argument completion (path + property via QMP,
        // remaining args fall through to generic completion below).
        if QOM_PATH_COMMANDS.contains(&first_word) && line.contains(' ') {
            let after_cmd = line[first_word.len()..].trim_start();
            let arg_words: Vec<&str> = after_cmd.split_whitespace().collect();
            let trailing_space = after_cmd.ends_with(' ');

            // If the partial token is a flag prefix, skip QOM-specific
            // completion and let the generic completer handle it.
            let partial_is_flag =
                !trailing_space && arg_words.last().is_some_and(|w| w.starts_with('-'));

            if !partial_is_flag {
                // Completed tokens: everything before the partial being typed.
                let completed: &[&str] = if trailing_space || arg_words.is_empty() {
                    &arg_words
                } else {
                    &arg_words[..arg_words.len() - 1]
                };
                // Count only non-flag tokens as positional arguments
                // (skip e.g. -j for qom-set).
                let positional_count = completed.iter().filter(|w| !w.starts_with('-')).count();

                // Arg 1: path completion
                if positional_count == 0 {
                    let partial = if trailing_space {
                        ""
                    } else {
                        arg_words.last().copied().unwrap_or("")
                    };
                    let span_start = line.len() - partial.len();
                    return self.complete_qom_path(partial, span_start, pos);
                }

                // Arg 2: property completion (qom-get, qom-set only)
                if positional_count == 1 && (first_word == "qom-get" || first_word == "qom-set") {
                    let path = *completed.iter().find(|w| !w.starts_with('-')).unwrap();
                    let prefix = if trailing_space {
                        ""
                    } else {
                        arg_words.last().copied().unwrap_or("")
                    };
                    let span_start = line.len() - prefix.len();
                    return self.complete_qom_property(path, prefix, span_start, pos);
                }

                // Arg 3: value completion (qom-set only).
                // For enum/bool properties, offer specific values.
                // For other types, fall through to generic completion.
                if positional_count == 2 && first_word == "qom-set" {
                    let mut positionals = completed.iter().filter(|w| !w.starts_with('-'));
                    let path = positionals.next().unwrap();
                    let property = positionals.next().unwrap();
                    let prefix = if trailing_space {
                        ""
                    } else {
                        arg_words.last().copied().unwrap_or("")
                    };
                    let span_start = line.len() - prefix.len();
                    if let Some(suggestions) =
                        self.complete_qom_value(path, property, prefix, span_start, pos)
                    {
                        return suggestions;
                    }
                }
            }

            // Remaining args or unknown property types fall through
            // to the generic arg completer below.
        }

        // object_add keyval completion: type,id=str,prop=val,...
        if first_word == "object_add" && line.contains(' ') {
            let spec = line[first_word.len()..].trim_start();
            let span_start = line.len() - spec.len();
            return self.complete_object_add(spec, span_start, pos);
        }

        // change command: context-sensitive (device determines subsequent args)
        if first_word == "change" && line.contains(' ') {
            let after_cmd = line[first_word.len()..].trim_start();
            let arg_tokens: Vec<&str> = if after_cmd.is_empty() {
                vec![]
            } else {
                after_cmd.split_whitespace().collect()
            };
            let trailing_space = after_cmd.is_empty() || after_cmd.ends_with(' ');
            let partial = if trailing_space {
                ""
            } else {
                arg_tokens.last().copied().unwrap_or("")
            };
            let span_start = line.len() - partial.len();
            return self.complete_change(&arg_tokens, trailing_space, partial, span_start, pos);
        }

        // sendkey completion: complete key names (QKeyCode), handling '-' as separator.
        if first_word == "sendkey" && line.contains(' ') {
            let after_cmd = line[first_word.len()..].trim_start();
            let arg_words: Vec<&str> = after_cmd.split_whitespace().collect();
            let trailing_space = after_cmd.ends_with(' ');

            // Only complete the first argument (keys); second is hold-time.
            if arg_words.len() > 1 || (arg_words.len() == 1 && trailing_space) {
                return vec![];
            }

            let keys_so_far = arg_words.first().copied().unwrap_or("");
            let (prefix_keep, partial) = match keys_so_far.rfind('-') {
                Some(i) => (&keys_so_far[..=i], &keys_so_far[i + 1..]),
                None => ("", keys_so_far),
            };
            let span_start = line.len() - partial.len();

            return self
                .sendkey_names
                .iter()
                .filter(|name| name.starts_with(partial))
                .map(|name| Suggestion {
                    value: format!("{prefix_keep}{name}"),
                    description: None,
                    style: None,
                    extra: None,
                    span: Span::new(span_start, pos),
                    append_whitespace: false,
                    display_override: Some(name.clone()),
                    match_indices: None,
                })
                .collect();
        }

        // Generic argument completion for commands with no specialized completer.
        if line.contains(' ') {
            if let Some(defs) = self.main_arg_defs.get(first_word) {
                let after_cmd = line[first_word.len()..].trim_start();
                let arg_tokens: Vec<&str> = if after_cmd.is_empty() {
                    vec![]
                } else {
                    after_cmd.split_whitespace().collect()
                };
                let trailing_space = after_cmd.is_empty() || after_cmd.ends_with(' ');
                let partial = if trailing_space {
                    ""
                } else {
                    arg_tokens.last().copied().unwrap_or("")
                };
                let span_start = line.len() - partial.len();
                let extra_block: &[&str] = match first_word {
                    "commit" => &["all"],
                    _ => &[],
                };
                return self.complete_args(
                    defs,
                    &arg_tokens,
                    trailing_space,
                    partial,
                    span_start,
                    pos,
                    extra_block,
                );
            }
            return vec![];
        }

        let prefix = first_word;
        let start = 0;
        self.main_names
            .iter()
            .filter(|name| name.starts_with(prefix))
            .map(|name| Suggestion {
                value: name.clone(),
                description: None,
                style: None,
                extra: None,
                span: Span::new(start, pos),
                append_whitespace: true,
                display_override: None,
                match_indices: None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_types() -> Vec<String> {
        vec![
            "dbus-display".into(),
            "memory-backend-file".into(),
            "memory-backend-memfd".into(),
            "rng-random".into(),
        ]
    }

    fn member(name: &str, display_type: &str, type_id: &str) -> SchemaMember {
        SchemaMember {
            name: name.into(),
            display_type: display_type.into(),
            type_id: type_id.into(),
        }
    }

    fn test_props(typename: &str) -> Vec<SchemaMember> {
        match typename {
            "dbus-display" => vec![
                member("id", "str", "str"),
                member("gl-mode", "enum", "42"),
                member("rendernode", "str", "str"),
            ],
            "memory-backend-file" => vec![
                member("id", "str", "str"),
                member("size", "int", "int"),
                member("mem-path", "str", "str"),
                member("share", "bool", "bool"),
            ],
            _ => vec![],
        }
    }

    fn test_enum_values() -> HashMap<String, Vec<String>> {
        let mut m = HashMap::new();
        // Keyed by schema type ID, matching what test_props returns
        m.insert(
            "42".into(),
            vec!["off".into(), "on".into(), "core".into(), "es".into()],
        );
        m
    }

    fn enum_lookup<'a>(
        cache: &'a HashMap<String, Vec<String>>,
    ) -> impl Fn(&str) -> Option<&'a [String]> {
        move |type_name| cache.get(type_name).map(|v| v.as_slice())
    }

    fn values(suggestions: &[Suggestion]) -> Vec<&str> {
        suggestions.iter().map(|s| s.value.as_str()).collect()
    }

    #[test]
    fn complete_type_name_empty() {
        let types = test_types();
        let enums = test_enum_values();
        let result = complete_object_add_impl("", 11, 11, &types, test_props, enum_lookup(&enums));
        assert_eq!(
            values(&result),
            &[
                "dbus-display",
                "memory-backend-file",
                "memory-backend-memfd",
                "rng-random"
            ]
        );
    }

    #[test]
    fn complete_type_name_prefix() {
        let types = test_types();
        let enums = test_enum_values();
        let result =
            complete_object_add_impl("mem", 11, 14, &types, test_props, enum_lookup(&enums));
        assert_eq!(
            values(&result),
            &["memory-backend-file", "memory-backend-memfd"]
        );
    }

    #[test]
    fn complete_property_key() {
        let types = test_types();
        let enums = test_enum_values();
        let result = complete_object_add_impl(
            "dbus-display,",
            11,
            24,
            &types,
            test_props,
            enum_lookup(&enums),
        );
        assert_eq!(values(&result), &["id=", "gl-mode=", "rendernode="]);
    }

    #[test]
    fn complete_property_key_with_prefix() {
        let types = test_types();
        let enums = test_enum_values();
        let result = complete_object_add_impl(
            "dbus-display,gl",
            11,
            26,
            &types,
            test_props,
            enum_lookup(&enums),
        );
        assert_eq!(values(&result), &["gl-mode="]);
    }

    #[test]
    fn complete_property_key_excludes_used() {
        let types = test_types();
        let enums = test_enum_values();
        let result = complete_object_add_impl(
            "dbus-display,id=foo,",
            11,
            31,
            &types,
            test_props,
            enum_lookup(&enums),
        );
        let vals = values(&result);
        assert!(!vals.contains(&"id="));
        assert!(vals.contains(&"gl-mode="));
        assert!(vals.contains(&"rendernode="));
    }

    #[test]
    fn complete_enum_value_empty() {
        let types = test_types();
        let enums = test_enum_values();
        let result = complete_object_add_impl(
            "dbus-display,gl-mode=",
            11,
            32,
            &types,
            test_props,
            enum_lookup(&enums),
        );
        assert_eq!(values(&result), &["off", "on", "core", "es"]);
        assert_eq!(result[0].description.as_deref(), Some("enum"));
    }

    #[test]
    fn complete_enum_value_prefix() {
        let types = test_types();
        let enums = test_enum_values();
        let result = complete_object_add_impl(
            "dbus-display,gl-mode=o",
            11,
            33,
            &types,
            test_props,
            enum_lookup(&enums),
        );
        assert_eq!(values(&result), &["off", "on"]);
    }

    #[test]
    fn complete_non_enum_value_empty() {
        let types = test_types();
        let enums = test_enum_values();
        let result = complete_object_add_impl(
            "dbus-display,id=",
            11,
            27,
            &types,
            test_props,
            enum_lookup(&enums),
        );
        assert!(result.is_empty());
    }

    #[test]
    fn complete_enum_after_other_props() {
        let types = test_types();
        let enums = test_enum_values();
        let result = complete_object_add_impl(
            "dbus-display,id=mydisp,gl-mode=",
            11,
            42,
            &types,
            test_props,
            enum_lookup(&enums),
        );
        assert_eq!(values(&result), &["off", "on", "core", "es"]);
    }

    #[test]
    fn complete_span_for_enum_value() {
        let types = test_types();
        let enums = test_enum_values();
        // "object_add dbus-display,gl-mode=o"
        //             ^span_start=11         ^pos=33
        let result = complete_object_add_impl(
            "dbus-display,gl-mode=o",
            11,
            33,
            &types,
            test_props,
            enum_lookup(&enums),
        );
        assert!(!result.is_empty());
        // val_start = span_start + idx + 1 + key.len() + 1
        //           = 11 + 12 + 1 + 7 + 1 = 32
        let span = result[0].span;
        assert_eq!(span.start, 32);
        assert_eq!(span.end, 33);
    }

    #[test]
    fn complete_unknown_type_gives_no_props() {
        let types = test_types();
        let enums = test_enum_values();
        let result = complete_object_add_impl(
            "no-such-type,",
            11,
            24,
            &types,
            test_props,
            enum_lookup(&enums),
        );
        assert!(result.is_empty());
    }

    // --- Generic arg completion tests ---

    fn no_block_devices() -> Vec<String> {
        vec![]
    }

    fn test_block_devices() -> Vec<String> {
        vec!["drive0".into(), "drive1".into(), "virtio0".into()]
    }

    fn no_filenames(_prefix: &str) -> Vec<(String, bool)> {
        vec![]
    }

    fn test_filenames(prefix: &str) -> Vec<(String, bool)> {
        let all = vec![
            ("file1.txt", false),
            ("file2.log", false),
            ("subdir/", true),
        ];
        all.into_iter()
            .filter(|(p, _)| p.starts_with(prefix))
            .map(|(p, d)| (p.to_string(), d))
            .collect()
    }

    #[test]
    fn args_complete_flag_present() {
        // "eject " with defs: force:-f,device:B
        let defs = parse_arg_defs("force:-f,device:B").unwrap();
        // Cursor right after "eject " — first arg position, flag expected
        let result = complete_args_impl(&defs, &[], true, "", 6, 6, no_block_devices, no_filenames);
        assert_eq!(values(&result), &["-f"]);
    }

    #[test]
    fn args_complete_flag_partial() {
        let defs = parse_arg_defs("force:-f,device:B").unwrap();
        // Typing "-" after "eject "
        let result = complete_args_impl(
            &defs,
            &["-"],
            false,
            "-",
            6,
            7,
            no_block_devices,
            no_filenames,
        );
        assert_eq!(values(&result), &["-f"]);
    }

    #[test]
    fn args_complete_block_device_after_flag() {
        let defs = parse_arg_defs("force:-f,device:B").unwrap();
        // "eject -f " — flag consumed, now completing block device
        let result = complete_args_impl(
            &defs,
            &["-f"],
            true,
            "",
            10,
            10,
            test_block_devices,
            no_filenames,
        );
        assert_eq!(values(&result), &["drive0", "drive1", "virtio0"]);
    }

    #[test]
    fn args_complete_block_device_without_flag() {
        let defs = parse_arg_defs("force:-f,device:B").unwrap();
        // "eject " — flag is optional, so both the flag and the next
        // required arg (block device) are offered as candidates.
        let result =
            complete_args_impl(&defs, &[], true, "", 6, 6, test_block_devices, no_filenames);
        assert_eq!(values(&result), &["-f", "drive0", "drive1", "virtio0"]);
    }

    #[test]
    fn args_complete_block_device_with_prefix() {
        let defs = parse_arg_defs("device:B").unwrap();
        // "block_resize dr" — partial block device name
        let result = complete_args_impl(
            &defs,
            &["dr"],
            false,
            "dr",
            13,
            15,
            test_block_devices,
            no_filenames,
        );
        assert_eq!(values(&result), &["drive0", "drive1"]);
    }

    #[test]
    fn args_complete_bool() {
        let defs = parse_arg_defs("name:s,state:b").unwrap();
        // "set_link net0 " — name consumed, now completing bool
        let result = complete_args_impl(
            &defs,
            &["net0"],
            true,
            "",
            14,
            14,
            no_block_devices,
            no_filenames,
        );
        assert_eq!(values(&result), &["on", "off"]);
    }

    #[test]
    fn args_complete_bool_partial() {
        let defs = parse_arg_defs("name:s,state:b").unwrap();
        // "set_link net0 o" — partial bool
        let result = complete_args_impl(
            &defs,
            &["net0", "o"],
            false,
            "o",
            14,
            15,
            no_block_devices,
            no_filenames,
        );
        assert_eq!(values(&result), &["on", "off"]);
    }

    #[test]
    fn args_complete_bool_partial_on() {
        let defs = parse_arg_defs("name:s,state:b").unwrap();
        let result = complete_args_impl(
            &defs,
            &["net0", "on"],
            false,
            "on",
            14,
            16,
            no_block_devices,
            no_filenames,
        );
        assert_eq!(values(&result), &["on"]);
    }

    #[test]
    fn args_complete_hint_for_string() {
        let defs = parse_arg_defs("name:s").unwrap();
        let result = complete_args_impl(&defs, &[], true, "", 5, 5, no_block_devices, no_filenames);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].display_override.as_deref(), Some("<name>"));
        assert_eq!(result[0].description.as_deref(), Some("string"));
    }

    #[test]
    fn args_complete_hint_for_int() {
        let defs = parse_arg_defs("value:i").unwrap();
        let result = complete_args_impl(&defs, &[], true, "", 5, 5, no_block_devices, no_filenames);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].display_override.as_deref(), Some("<value>"));
        assert_eq!(result[0].description.as_deref(), Some("integer"));
    }

    #[test]
    fn args_complete_empty_defs() {
        let defs = parse_arg_defs("").unwrap();
        let result = complete_args_impl(&defs, &[], true, "", 5, 5, no_block_devices, no_filenames);
        assert!(result.is_empty());
    }

    #[test]
    fn args_complete_all_args_consumed() {
        let defs = parse_arg_defs("device:B").unwrap();
        // Already typed the only arg — no more completions
        let result = complete_args_impl(
            &defs,
            &["drive0"],
            true,
            "",
            13,
            13,
            test_block_devices,
            no_filenames,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn args_complete_multiple_flags_then_required() {
        // Like dump-guest-memory: several flags, then a required filename
        let defs = parse_arg_defs("paging:-p,detach:-d,filename:F").unwrap();
        let result =
            complete_args_impl(&defs, &[], true, "", 5, 5, no_block_devices, test_filenames);
        let vals = values(&result);
        assert!(vals.contains(&"-p"));
        assert!(vals.contains(&"-d"));
        assert!(vals.contains(&"file1.txt"));
        assert!(vals.contains(&"file2.log"));
        assert!(vals.contains(&"subdir/"));
    }

    #[test]
    fn args_complete_flags_consumed_then_next() {
        let defs = parse_arg_defs("paging:-p,detach:-d,filename:F").unwrap();
        // "-p" consumed — should show remaining flag "-d" + filenames
        let result = complete_args_impl(
            &defs,
            &["-p"],
            true,
            "",
            8,
            8,
            no_block_devices,
            test_filenames,
        );
        let vals = values(&result);
        assert!(vals.contains(&"-d"));
        assert!(vals.contains(&"file1.txt"));
        assert!(!vals.contains(&"-p"));
    }

    #[test]
    fn args_complete_filename_with_prefix() {
        let defs = parse_arg_defs("filename:F").unwrap();
        let result = complete_args_impl(
            &defs,
            &["fi"],
            false,
            "fi",
            5,
            7,
            no_block_devices,
            test_filenames,
        );
        assert_eq!(values(&result), &["file1.txt", "file2.log"]);
    }

    #[test]
    fn args_complete_filename_dir_no_space() {
        let defs = parse_arg_defs("filename:F").unwrap();
        let result = complete_args_impl(
            &defs,
            &["su"],
            false,
            "su",
            5,
            7,
            no_block_devices,
            test_filenames,
        );
        assert_eq!(values(&result), &["subdir/"]);
        // Directories should not append whitespace so the user can keep completing.
        assert!(!result[0].append_whitespace);
    }

    #[test]
    fn args_complete_optional_shows_next() {
        // Optional string then required bool
        let defs = parse_arg_defs("name:s?,state:b").unwrap();
        let result = complete_args_impl(&defs, &[], true, "", 5, 5, no_block_devices, no_filenames);
        // Should show hint for optional name AND bool values for required state
        assert_eq!(result.len(), 3); // <name> hint + on + off
        assert_eq!(result[0].display_override.as_deref(), Some("<name>"));
        assert_eq!(result[1].value, "on");
        assert_eq!(result[2].value, "off");
    }

    #[test]
    fn args_complete_hint_for_size() {
        let defs = parse_arg_defs("amount:o").unwrap();
        let result = complete_args_impl(&defs, &[], true, "", 5, 5, no_block_devices, no_filenames);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].display_override.as_deref(), Some("<amount>"));
        assert_eq!(result[0].description.as_deref(), Some("size (K/M/G/T)"));
    }

    #[test]
    fn args_complete_hint_for_long() {
        let defs = parse_arg_defs("addr:l").unwrap();
        let result = complete_args_impl(&defs, &[], true, "", 5, 5, no_block_devices, no_filenames);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].display_override.as_deref(), Some("<addr>"));
        assert_eq!(
            result[0].description.as_deref(),
            Some("expression ($reg, hex, arithmetic)")
        );
    }

    #[test]
    fn args_complete_hint_for_mebibytes() {
        let defs = parse_arg_defs("value:M").unwrap();
        let result = complete_args_impl(&defs, &[], true, "", 5, 5, no_block_devices, no_filenames);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].display_override.as_deref(), Some("<value>"));
        assert_eq!(result[0].description.as_deref(), Some("megabytes"));
    }

    #[test]
    fn args_complete_flag_after_positional() {
        // After typing the device, the unconsumed flag should still appear.
        let defs = parse_arg_defs("force:-f,device:B").unwrap();
        let result = complete_args_impl(
            &defs,
            &["drive0"],
            true,
            "",
            13,
            13,
            test_block_devices,
            no_filenames,
        );
        assert_eq!(values(&result), &["-f"]);
    }

    #[test]
    fn args_complete_flag_consumed_anywhere() {
        // Flag consumed after the device — no longer offered.
        let defs = parse_arg_defs("force:-f,device:B").unwrap();
        let result = complete_args_impl(
            &defs,
            &["drive0", "-f"],
            true,
            "",
            17,
            17,
            test_block_devices,
            no_filenames,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn args_complete_mid_token_filters_by_prefix() {
        // Typing a non-flag value skips the flag and matches block device
        let defs = parse_arg_defs("force:-f,device:B").unwrap();
        let result = complete_args_impl(
            &defs,
            &["dr"],
            false,
            "dr",
            6,
            8,
            test_block_devices,
            no_filenames,
        );
        let vals = values(&result);
        // "-f" doesn't start with "dr" → filtered out
        assert!(!vals.contains(&"-f"));
        assert!(vals.contains(&"drive0"));
        assert!(vals.contains(&"drive1"));
    }

    // --- change command completion tests ---

    #[test]
    fn change_complete_device_empty() {
        // "change " — should offer -f, block devices, and "vnc"
        let result = complete_change_impl(&[], true, "", 7, 7, test_block_devices, no_filenames);
        let vals = values(&result);
        assert!(vals.contains(&"-f"));
        assert!(vals.contains(&"drive0"));
        assert!(vals.contains(&"vnc"));
    }

    #[test]
    fn change_complete_device_prefix() {
        // "change v" — should match "vnc" and "virtio0"
        let result =
            complete_change_impl(&["v"], false, "v", 7, 8, test_block_devices, no_filenames);
        let vals = values(&result);
        assert!(vals.contains(&"vnc"));
        assert!(vals.contains(&"virtio0"));
        assert!(!vals.contains(&"drive0"));
        assert!(!vals.contains(&"-f"));
    }

    #[test]
    fn change_vnc_complete_target() {
        // "change vnc " — should offer "passwd" and "password"
        let result =
            complete_change_impl(&["vnc"], true, "", 11, 11, test_block_devices, no_filenames);
        let vals = values(&result);
        assert!(vals.contains(&"passwd"));
        assert!(vals.contains(&"password"));
        assert!(vals.contains(&"-f"));
    }

    #[test]
    fn change_vnc_complete_target_prefix() {
        // "change vnc pass" — should match both "passwd" and "password"
        let result = complete_change_impl(
            &["vnc", "pass"],
            false,
            "pass",
            11,
            15,
            test_block_devices,
            no_filenames,
        );
        let vals = values(&result);
        assert!(vals.contains(&"passwd"));
        assert!(vals.contains(&"password"));
    }

    #[test]
    fn change_vnc_complete_password_hint() {
        // "change vnc password " — should show a password hint
        let result = complete_change_impl(
            &["vnc", "password"],
            true,
            "",
            20,
            20,
            test_block_devices,
            no_filenames,
        );
        assert_eq!(result.len(), 2); // -f + password hint
        let pw = result
            .iter()
            .find(|s| s.display_override.is_some())
            .unwrap();
        assert_eq!(pw.display_override.as_deref(), Some("<password>"));
    }

    #[test]
    fn change_block_complete_target_filenames() {
        // "change drive0 " — should offer filenames and -f flag
        let result = complete_change_impl(
            &["drive0"],
            true,
            "",
            14,
            14,
            test_block_devices,
            test_filenames,
        );
        let vals = values(&result);
        assert!(vals.contains(&"-f"));
        assert!(vals.contains(&"file1.txt"));
        assert!(vals.contains(&"file2.log"));
        assert!(vals.contains(&"subdir/"));
    }

    #[test]
    fn change_block_complete_format() {
        // "change drive0 /tmp/disk.img " — should offer image formats + -f
        let result = complete_change_impl(
            &["drive0", "/tmp/disk.img"],
            true,
            "",
            25,
            25,
            test_block_devices,
            no_filenames,
        );
        let vals = values(&result);
        assert!(vals.contains(&"-f"));
        assert!(vals.contains(&"qcow2"));
        assert!(vals.contains(&"raw"));
        assert!(vals.contains(&"vmdk"));
    }

    #[test]
    fn change_block_complete_format_prefix() {
        // "change drive0 /tmp/disk.img q" — should match qcow2 and qed
        let result = complete_change_impl(
            &["drive0", "/tmp/disk.img", "q"],
            false,
            "q",
            25,
            26,
            test_block_devices,
            no_filenames,
        );
        let vals = values(&result);
        assert!(vals.contains(&"qcow2"));
        assert!(vals.contains(&"qed"));
        assert!(!vals.contains(&"raw"));
    }

    #[test]
    fn change_block_complete_read_only_mode() {
        // "change drive0 /tmp/disk.img qcow2 " — should offer read-only modes
        let result = complete_change_impl(
            &["drive0", "/tmp/disk.img", "qcow2"],
            true,
            "",
            31,
            31,
            test_block_devices,
            no_filenames,
        );
        let vals = values(&result);
        assert!(vals.contains(&"retain"));
        assert!(vals.contains(&"read-only"));
        assert!(vals.contains(&"read-write"));
    }

    #[test]
    fn change_block_complete_read_only_mode_prefix() {
        // "change drive0 /tmp/disk.img qcow2 read-" — should filter
        let result = complete_change_impl(
            &["drive0", "/tmp/disk.img", "qcow2", "read-"],
            false,
            "read-",
            31,
            36,
            test_block_devices,
            no_filenames,
        );
        let vals = values(&result);
        assert!(vals.contains(&"read-only"));
        assert!(vals.contains(&"read-write"));
        assert!(!vals.contains(&"retain"));
    }

    #[test]
    fn change_flag_before_device() {
        // "change -f " — flag consumed, now completing device
        let result =
            complete_change_impl(&["-f"], true, "", 10, 10, test_block_devices, no_filenames);
        let vals = values(&result);
        assert!(!vals.contains(&"-f")); // flag already consumed
        assert!(vals.contains(&"drive0"));
        assert!(vals.contains(&"vnc"));
    }

    #[test]
    fn change_flag_between_device_and_target() {
        // "change drive0 -f " — flag consumed after device, completing target
        let result = complete_change_impl(
            &["drive0", "-f"],
            true,
            "",
            17,
            17,
            test_block_devices,
            test_filenames,
        );
        let vals = values(&result);
        assert!(!vals.contains(&"-f"));
        assert!(vals.contains(&"file1.txt"));
    }

    #[test]
    fn change_vnc_no_read_only_mode() {
        // "change vnc password secret " — no further completions for VNC
        let result = complete_change_impl(
            &["vnc", "password", "secret"],
            true,
            "",
            27,
            27,
            test_block_devices,
            no_filenames,
        );
        // Only the unconsumed -f flag, no positional suggestions
        let vals = values(&result);
        assert_eq!(vals, &["-f"]);
    }

    #[test]
    fn change_all_consumed() {
        // "change drive0 /tmp/disk.img qcow2 retain " — everything consumed
        let result = complete_change_impl(
            &["drive0", "/tmp/disk.img", "qcow2", "retain"],
            true,
            "",
            38,
            38,
            test_block_devices,
            no_filenames,
        );
        // Only the unconsumed -f flag
        let vals = values(&result);
        assert_eq!(vals, &["-f"]);
    }

    // --- JSON QMP command completion tests ---

    fn test_qmp_commands() -> Vec<String> {
        vec![
            "blockdev-change-medium".into(),
            "device-add".into(),
            "device_del".into(),
            "query-block".into(),
            "query-blockstats".into(),
            "query-cpus-fast".into(),
            "query-status".into(),
            "query-version".into(),
            "system_reset".into(),
        ]
    }

    fn test_command_args() -> HashMap<String, Vec<SchemaMember>> {
        let mut m = HashMap::new();
        m.insert(
            "device-add".into(),
            vec![
                member("driver", "str", "str"),
                member("bus", "str", "str"),
                member("id", "str", "str"),
            ],
        );
        m.insert(
            "blockdev-change-medium".into(),
            vec![
                member("device", "str", "str"),
                member("filename", "str", "str"),
                member("format", "str", "str"),
                member("read-only-mode", "enum", "BlockdevChangeReadOnlyMode"),
            ],
        );
        m.insert(
            "screendump".into(),
            vec![
                member("filename", "str", "str"),
                member("device", "str", "str"),
                member("head", "int", "int"),
                member("format", "enum", "ImageFormat"),
            ],
        );
        m.insert(
            "set-action".into(),
            vec![
                member("reboot", "enum", "RebootAction"),
                member("shutdown", "enum", "ShutdownAction"),
                member("watchdog", "enum", "WatchdogAction"),
                member("panic", "enum", "PanicAction"),
            ],
        );
        m.insert(
            "migrate-set-capabilities".into(),
            vec![member("auto-converge", "bool", "bool")],
        );
        m
    }

    fn test_json_enums() -> HashMap<String, Vec<String>> {
        let mut m = test_enum_values();
        m.insert(
            "BlockdevChangeReadOnlyMode".into(),
            vec!["retain".into(), "read-only".into(), "read-write".into()],
        );
        m.insert(
            "ImageFormat".into(),
            vec!["ppm".into(), "png".into(), "jpg".into()],
        );
        m
    }

    fn test_json_schema<'a>(
        commands: &'a [String],
        command_args: &'a HashMap<String, Vec<SchemaMember>>,
    ) -> JsonSchemaCtx<'a> {
        JsonSchemaCtx {
            commands,
            command_args,
            object_types: &EMPTY_SCHEMA_MEMBERS,
            tagged_unions: &EMPTY_TAGGED_UNIONS,
            command_arg_type_ids: &EMPTY_ARG_TYPE_IDS,
        }
    }

    static EMPTY_SCHEMA_MEMBERS: std::sync::LazyLock<HashMap<String, Vec<SchemaMember>>> =
        std::sync::LazyLock::new(HashMap::new);
    static EMPTY_TAGGED_UNIONS: std::sync::LazyLock<HashMap<String, TaggedUnionInfo>> =
        std::sync::LazyLock::new(HashMap::new);
    static EMPTY_ARG_TYPE_IDS: std::sync::LazyLock<HashMap<String, String>> =
        std::sync::LazyLock::new(HashMap::new);

    #[test]
    fn json_complete_boilerplate_just_brace() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = "{";
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, r#"{"execute": ""#);
    }

    #[test]
    fn json_complete_boilerplate_partial_key() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"ex"#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, r#"{"execute": ""#);
    }

    #[test]
    fn json_complete_boilerplate_full_key_no_colon() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute""#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, r#"{"execute": ""#);
    }

    #[test]
    fn json_complete_boilerplate_colon_no_quote() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, r#"{"execute": ""#);
    }

    #[test]
    fn json_complete_boilerplate_with_space() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = "{ ";
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, r#"{"execute": ""#);
    }

    #[test]
    fn json_complete_boilerplate_non_execute_key() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"other"#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert!(result.is_empty());
    }

    #[test]
    fn json_complete_args_boilerplate_after_command() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "device-add""#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, r#", "arguments": {"#);
    }

    #[test]
    fn json_complete_args_boilerplate_after_comma() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "device-add", "#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, r#", "arguments": {"#);
    }

    #[test]
    fn json_complete_args_boilerplate_partial_key() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "device-add", "arg"#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, r#", "arguments": {"#);
    }

    #[test]
    fn json_complete_args_boilerplate_no_args_command() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        // query-version has no args in test data
        let line = r#"{"execute": "query-version""#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert!(result.is_empty());
    }

    #[test]
    fn json_complete_args_boilerplate_colon_no_brace() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "device-add", "arguments": "#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, r#", "arguments": {"#);
    }

    #[test]
    fn json_complete_command_name_empty() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": ""#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert_eq!(result.len(), cmds.len());
        let vals = values(&result);
        assert!(vals.contains(&r#"device-add""#));
        assert!(vals.contains(&r#"query-version""#));
    }

    #[test]
    fn json_complete_command_name_prefix() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "query-"#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        let vals = values(&result);
        assert!(vals.contains(&r#"query-block""#));
        assert!(vals.contains(&r#"query-version""#));
        assert!(!vals.iter().any(|v| v.starts_with("device")));
    }

    #[test]
    fn json_complete_command_name_unique() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "query-v"#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert_eq!(values(&result), &[r#"query-version""#]);
    }

    #[test]
    fn json_complete_command_display_override() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "query-v"#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert_eq!(result[0].display_override.as_deref(), Some("query-version"));
    }

    #[test]
    fn json_complete_command_with_spaces() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{ "execute" : "query-"#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        let vals = values(&result);
        assert!(vals.contains(&r#"query-block""#));
    }

    #[test]
    fn json_complete_arg_key_no_quote() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "device-add", "arguments": {"#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        let vals = values(&result);
        assert!(vals.contains(&r#""driver": "#));
        assert!(vals.contains(&r#""bus": "#));
        assert!(vals.contains(&r#""id": "#));
    }

    #[test]
    fn json_complete_arg_key_in_quotes() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "device-add", "arguments": {"dr"#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert_eq!(values(&result), &[r#"driver": "#]);
    }

    #[test]
    fn json_complete_arg_key_excludes_used() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "device-add", "arguments": {"driver": "e1000", ""#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        let vals = values(&result);
        assert!(!vals.iter().any(|v| v.contains("driver")));
        assert!(vals.contains(&r#"bus": "#));
        assert!(vals.contains(&r#"id": "#));
    }

    #[test]
    fn json_complete_arg_key_display_override() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "device-add", "arguments": {"dr"#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert_eq!(result[0].display_override.as_deref(), Some("driver"));
    }

    #[test]
    fn json_complete_arg_enum_value() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "blockdev-change-medium", "arguments": {"read-only-mode": ""#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        let vals = values(&result);
        assert!(vals.contains(&r#"retain""#));
        assert!(vals.contains(&r#"read-only""#));
        assert!(vals.contains(&r#"read-write""#));
    }

    #[test]
    fn json_complete_arg_enum_value_prefix() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line =
            r#"{"execute": "blockdev-change-medium", "arguments": {"read-only-mode": "read-o"#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert_eq!(values(&result), &[r#"read-only""#]);
    }

    #[test]
    fn json_complete_non_json_returns_empty() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = "info version";
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert!(result.is_empty());
    }

    #[test]
    fn json_complete_no_execute_key() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"other": "value"#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert!(result.is_empty());
    }

    #[test]
    fn json_complete_unknown_command_args() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "query-version", "arguments": {"#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        // query-version has no args in our test data
        assert!(result.is_empty());
    }

    #[test]
    fn json_complete_arg_key_after_comma() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "device-add", "arguments": {"driver": "e1000", "#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        let vals = values(&result);
        // "driver" is used, should not appear; "bus" and "id" should
        assert!(!vals.iter().any(|v| v.contains("driver")));
        assert!(vals.contains(&r#""bus": "#));
        assert!(vals.contains(&r#""id": "#));
    }

    #[test]
    fn json_complete_command_span() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "query-v"#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert!(!result.is_empty());
        // Span should cover "query-v" (7 chars from end)
        let span = result[0].span;
        assert_eq!(span.start, pos - "query-v".len());
        assert_eq!(span.end, pos);
    }

    // --- Unquoted value completion tests ---

    #[test]
    fn json_complete_enum_value_no_quote() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        // "format":  with no opening quote — should offer quoted enum values
        let line = r#"{"execute": "screendump", "arguments": {"format": "#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        let vals = values(&result);
        assert!(vals.contains(&r#""ppm""#));
        assert!(vals.contains(&r#""png""#));
        assert!(vals.contains(&r#""jpg""#));
    }

    #[test]
    fn json_complete_enum_value_no_quote_prefix() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        // Partial unquoted value
        let line = r#"{"execute": "screendump", "arguments": {"format": p"#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        let vals = values(&result);
        assert!(vals.contains(&r#""ppm""#));
        assert!(vals.contains(&r#""png""#));
        assert!(!vals.iter().any(|v| v.contains("jpg")));
    }

    #[test]
    fn json_complete_enum_value_in_quote() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        // With opening quote — existing behavior, value includes only closing quote
        let line = r#"{"execute": "screendump", "arguments": {"format": ""#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        let vals = values(&result);
        assert!(vals.contains(&r#"ppm""#));
        assert!(vals.contains(&r#"png""#));
        assert!(vals.contains(&r#"jpg""#));
    }

    #[test]
    fn json_complete_bool_value_no_quote() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "migrate-set-capabilities", "arguments": {"auto-converge": "#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        let vals = values(&result);
        assert!(vals.contains(&"true"));
        assert!(vals.contains(&"false"));
    }

    #[test]
    fn json_complete_bool_value_partial() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "migrate-set-capabilities", "arguments": {"auto-converge": t"#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert_eq!(values(&result), &["true"]);
    }

    #[test]
    fn json_complete_bool_not_in_quotes() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        // Inside quotes — booleans shouldn't be suggested (not valid JSON)
        let line = r#"{"execute": "migrate-set-capabilities", "arguments": {"auto-converge": ""#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        assert!(result.is_empty());
    }

    #[test]
    fn json_complete_enum_display_override_no_quote() {
        let cmds = test_qmp_commands();
        let args = test_command_args();
        let enums = test_json_enums();
        let line = r#"{"execute": "screendump", "arguments": {"format": "#;
        let pos = line.len();
        let result = complete_json_impl(
            line,
            pos,
            &test_json_schema(&cmds, &args),
            enum_lookup(&enums),
        );
        // Display override should show clean value without quotes
        assert_eq!(result[0].display_override.as_deref(), Some("ppm"));
    }

    #[test]
    fn toplevel_commas_flat() {
        let s = r#""a": 1, "b": 2, "c": 3"#;
        assert_eq!(toplevel_comma_positions(s), vec![6, 14]);
    }

    #[test]
    fn toplevel_commas_nested_braces() {
        let s = r#""a": {"x": 1, "y": 2}, "b": 3"#;
        assert_eq!(toplevel_comma_positions(s), vec![21]);
    }

    #[test]
    fn toplevel_commas_nested_brackets() {
        let s = r#""a": [1, 2, 3], "b": 4"#;
        assert_eq!(toplevel_comma_positions(s), vec![14]);
    }

    #[test]
    fn toplevel_commas_string_with_comma() {
        let s = r#""a": "hello, world", "b": 5"#;
        assert_eq!(toplevel_comma_positions(s), vec![19]);
    }

    #[test]
    fn toplevel_commas_empty() {
        assert_eq!(toplevel_comma_positions(""), Vec::<usize>::new());
    }

    #[test]
    fn toplevel_commas_no_comma() {
        let s = r#""a": 1"#;
        assert_eq!(toplevel_comma_positions(s), Vec::<usize>::new());
    }

    #[test]
    fn toplevel_commas_deeply_nested() {
        let s = r#""a": {"b": {"c": 1, "d": 2}}, "e": 3"#;
        assert_eq!(toplevel_comma_positions(s), vec![28]);
    }

    #[test]
    fn toplevel_commas_escaped_quote_in_string() {
        let s = r#""a": "he said \"hi\", ok", "b": 1"#;
        assert_eq!(toplevel_comma_positions(s), vec![25]);
    }

    #[test]
    fn extract_kv_string_value() {
        assert_eq!(
            extract_segment_kv(r#" "driver": "e1000""#),
            Some(("driver", Some("e1000")))
        );
    }

    #[test]
    fn extract_kv_number_value() {
        assert_eq!(extract_segment_kv(r#" "count": 42"#), Some(("count", None)));
    }

    #[test]
    fn extract_kv_object_value() {
        assert_eq!(
            extract_segment_kv(r#" "file": {"path": "/tmp"}"#),
            Some(("file", None))
        );
    }

    #[test]
    fn extract_kv_bool_value() {
        assert_eq!(
            extract_segment_kv(r#" "enabled": true"#),
            Some(("enabled", None))
        );
    }

    #[test]
    fn extract_kv_incomplete_key() {
        assert_eq!(extract_segment_kv(r#" "dri"#), None);
    }

    #[test]
    fn extract_kv_empty() {
        assert_eq!(extract_segment_kv("  "), None);
    }

    // --- Helpers for nested/tagged union tests ---

    fn test_object_types() -> HashMap<String, Vec<SchemaMember>> {
        let mut m = HashMap::new();
        m.insert(
            "BlockdevOptionsFile".into(),
            vec![
                member("filename", "str", "str"),
                member("aio", "enum", "BlockdevAioOptions"),
                member("locking", "enum", "OnOffAuto"),
            ],
        );
        m.insert(
            "SocketAddressInet".into(),
            vec![member("host", "str", "str"), member("port", "str", "str")],
        );
        m.insert(
            "BlockdevOptionsNbd".into(),
            vec![
                member("server", "object", "SocketAddressInet"),
                member("export", "str", "str"),
            ],
        );
        m
    }

    fn test_tagged_unions() -> HashMap<String, TaggedUnionInfo> {
        let mut m = HashMap::new();
        m.insert(
            "BlockdevOptionsArgs".into(),
            TaggedUnionInfo {
                tag: "driver".into(),
                variants: HashMap::from([
                    ("file".into(), "BlockdevOptionsFile".into()),
                    ("nbd".into(), "BlockdevOptionsNbd".into()),
                ]),
            },
        );
        m
    }

    fn test_command_arg_type_ids() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("blockdev-add".into(), "BlockdevOptionsArgs".into());
        m
    }

    fn test_command_args_with_nesting() -> HashMap<String, Vec<SchemaMember>> {
        let mut m = test_command_args();
        m.insert(
            "blockdev-add".into(),
            vec![
                member("driver", "enum", "BlockdevDriver"),
                member("node-name", "str", "str"),
                member("file", "object", "BlockdevOptionsFile"),
            ],
        );
        m
    }

    fn test_qmp_commands_with_nesting() -> Vec<String> {
        let mut cmds = test_qmp_commands();
        cmds.push("blockdev-add".into());
        cmds.sort();
        cmds
    }

    // --- Nested object completion tests ---

    #[test]
    fn json_complete_nested_object_keys() {
        let cmds = test_qmp_commands_with_nesting();
        let args = test_command_args_with_nesting();
        let obj_types = test_object_types();
        let tagged = HashMap::new();
        let tag_ids = HashMap::new();
        let enums = test_json_enums();
        let line = r#"{"execute": "blockdev-add", "arguments": {"file": {"#;
        let pos = line.len();
        let schema = JsonSchemaCtx {
            commands: &cmds,
            command_args: &args,
            object_types: &obj_types,
            tagged_unions: &tagged,
            command_arg_type_ids: &tag_ids,
        };
        let result = complete_json_impl(line, pos, &schema, enum_lookup(&enums));
        let vals = values(&result);
        assert!(vals.contains(&r#""filename": "#));
        assert!(vals.contains(&r#""aio": "#));
        assert!(vals.contains(&r#""locking": "#));
    }

    #[test]
    fn json_complete_nested_object_excludes_used() {
        let cmds = test_qmp_commands_with_nesting();
        let args = test_command_args_with_nesting();
        let obj_types = test_object_types();
        let tagged = HashMap::new();
        let tag_ids = HashMap::new();
        let enums = test_json_enums();
        let line = r#"{"execute": "blockdev-add", "arguments": {"file": {"filename": "/tmp/x", "#;
        let pos = line.len();
        let schema = JsonSchemaCtx {
            commands: &cmds,
            command_args: &args,
            object_types: &obj_types,
            tagged_unions: &tagged,
            command_arg_type_ids: &tag_ids,
        };
        let result = complete_json_impl(line, pos, &schema, enum_lookup(&enums));
        let vals = values(&result);
        assert!(!vals.iter().any(|v| v.contains("filename")));
        assert!(vals.contains(&r#""aio": "#));
        assert!(vals.contains(&r#""locking": "#));
    }

    #[test]
    fn json_complete_nested_enum_value() {
        let cmds = test_qmp_commands_with_nesting();
        let args = test_command_args_with_nesting();
        let obj_types = test_object_types();
        let tagged = HashMap::new();
        let tag_ids = HashMap::new();
        let mut enums = test_json_enums();
        enums.insert(
            "BlockdevAioOptions".into(),
            vec!["threads".into(), "native".into(), "io_uring".into()],
        );
        let line = r#"{"execute": "blockdev-add", "arguments": {"file": {"aio": ""#;
        let pos = line.len();
        let schema = JsonSchemaCtx {
            commands: &cmds,
            command_args: &args,
            object_types: &obj_types,
            tagged_unions: &tagged,
            command_arg_type_ids: &tag_ids,
        };
        let result = complete_json_impl(line, pos, &schema, enum_lookup(&enums));
        let vals = values(&result);
        assert!(vals.contains(&r#"threads""#));
        assert!(vals.contains(&r#"native""#));
        assert!(vals.contains(&r#"io_uring""#));
    }

    #[test]
    fn json_complete_double_nested_keys() {
        let cmds = test_qmp_commands_with_nesting();
        let mut args = test_command_args_with_nesting();
        args.insert(
            "blockdev-add".into(),
            vec![
                member("driver", "enum", "BlockdevDriver"),
                member("node-name", "str", "str"),
                member("server", "object", "BlockdevOptionsNbd"),
            ],
        );
        let obj_types = test_object_types();
        let tagged = HashMap::new();
        let tag_ids = HashMap::new();
        let enums = test_json_enums();
        let line = r#"{"execute": "blockdev-add", "arguments": {"server": {"server": {"#;
        let pos = line.len();
        let schema = JsonSchemaCtx {
            commands: &cmds,
            command_args: &args,
            object_types: &obj_types,
            tagged_unions: &tagged,
            command_arg_type_ids: &tag_ids,
        };
        let result = complete_json_impl(line, pos, &schema, enum_lookup(&enums));
        let vals = values(&result);
        assert!(vals.contains(&r#""host": "#));
        assert!(vals.contains(&r#""port": "#));
    }

    #[test]
    fn json_complete_comma_inside_nested_no_break() {
        let cmds = test_qmp_commands_with_nesting();
        let args = test_command_args_with_nesting();
        let obj_types = test_object_types();
        let tagged = HashMap::new();
        let tag_ids = HashMap::new();
        let enums = test_json_enums();
        let line = r#"{"execute": "blockdev-add", "arguments": {"file": {"filename": "/tmp", "aio": "native"}, "#;
        let pos = line.len();
        let schema = JsonSchemaCtx {
            commands: &cmds,
            command_args: &args,
            object_types: &obj_types,
            tagged_unions: &tagged,
            command_arg_type_ids: &tag_ids,
        };
        let result = complete_json_impl(line, pos, &schema, enum_lookup(&enums));
        let vals = values(&result);
        assert!(!vals.iter().any(|v| v.contains("file")));
        assert!(vals.contains(&r#""driver": "#));
        assert!(vals.contains(&r#""node-name": "#));
    }

    // --- Tagged union variant resolution tests ---

    #[test]
    fn json_complete_tagged_union_variant_members() {
        let cmds = test_qmp_commands_with_nesting();
        let mut args = test_command_args_with_nesting();
        args.insert(
            "blockdev-add".into(),
            vec![
                member("driver", "enum", "BlockdevDriver"),
                member("node-name", "str", "str"),
            ],
        );
        let obj_types = test_object_types();
        let tagged = test_tagged_unions();
        let tag_ids = test_command_arg_type_ids();
        let enums = test_json_enums();
        let line =
            r#"{"execute": "blockdev-add", "arguments": {"driver": "file", "node-name": "n1", "#;
        let pos = line.len();
        let schema = JsonSchemaCtx {
            commands: &cmds,
            command_args: &args,
            object_types: &obj_types,
            tagged_unions: &tagged,
            command_arg_type_ids: &tag_ids,
        };
        let result = complete_json_impl(line, pos, &schema, enum_lookup(&enums));
        let vals = values(&result);
        assert!(vals.contains(&r#""filename": "#));
        assert!(vals.contains(&r#""aio": "#));
        assert!(!vals.iter().any(|v| v.contains("driver")));
        assert!(!vals.iter().any(|v| v.contains("node-name")));
    }

    #[test]
    fn json_complete_tagged_union_no_tag_value() {
        let cmds = test_qmp_commands_with_nesting();
        let mut args = test_command_args_with_nesting();
        args.insert(
            "blockdev-add".into(),
            vec![
                member("driver", "enum", "BlockdevDriver"),
                member("node-name", "str", "str"),
            ],
        );
        let obj_types = test_object_types();
        let tagged = test_tagged_unions();
        let tag_ids = test_command_arg_type_ids();
        let enums = test_json_enums();
        let line = r#"{"execute": "blockdev-add", "arguments": {"#;
        let pos = line.len();
        let schema = JsonSchemaCtx {
            commands: &cmds,
            command_args: &args,
            object_types: &obj_types,
            tagged_unions: &tagged,
            command_arg_type_ids: &tag_ids,
        };
        let result = complete_json_impl(line, pos, &schema, enum_lookup(&enums));
        let vals = values(&result);
        assert!(vals.contains(&r#""driver": "#));
        assert!(vals.contains(&r#""node-name": "#));
        assert!(!vals.iter().any(|v| v.contains("filename")));
    }
}
