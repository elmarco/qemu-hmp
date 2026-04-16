// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use serde::{Deserialize, Serialize};

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

// qom-list-get is a QMP command (Since: 10.1) not yet in the
// qapi-rs crate.  Define the command and response structs manually.

#[derive(Debug, Clone, Deserialize)]
pub struct ObjectPropertyValue {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ObjectPropertiesValues {
    pub properties: Vec<ObjectPropertyValue>,
}

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct qom_list_get {
    pub paths: Vec<String>,
}

impl qapi_qmp::QmpCommand for qom_list_get {}
impl qapi::Command for qom_list_get {
    const NAME: &'static str = "qom-list-get";
    const ALLOW_OOB: bool = false;
    type Ok = Vec<ObjectPropertiesValues>;
}

/// Extract the type name from a "child<TYPE>" property type string.
fn child_type(prop_type: &str) -> Option<&str> {
    prop_type
        .strip_prefix("child<")
        .and_then(|s| s.strip_suffix('>'))
}

/// Recursively print the QOM composition tree.
async fn print_qom_tree(
    conn: &QmpConnection,
    path: &str,
    name: &str,
    type_name: &str,
    indent: usize,
    out: &mut String,
) -> Result<(), CmdError> {
    writeln!(
        out,
        "{:indent$}/{} ({})",
        "",
        name,
        type_name,
        indent = indent
    )
    .unwrap();

    // Get all properties (with values) for this object
    let results = conn
        .execute(qom_list_get {
            paths: vec![path.to_string()],
        })
        .await
        .map_err(CmdError::from)?;

    let props = &results[0].properties;

    // Collect children (properties with type "child<...>"), sorted by name
    let mut children: Vec<(&str, &str)> = Vec::new();
    for prop in props {
        if let Some(ctype) = child_type(&prop.type_) {
            children.push((&prop.name, ctype));
        }
    }
    children.sort_by(|a, b| a.0.cmp(b.0));

    // Recurse into each child
    for (child_name, ctype) in children {
        let child_path = if path == "/" {
            format!("/{}", child_name)
        } else {
            format!("{}/{}", path, child_name)
        };
        Box::pin(print_qom_tree(
            conn,
            &child_path,
            child_name,
            ctype,
            indent + 2,
            out,
        ))
        .await?;
    }

    Ok(())
}

pub async fn cmd_info_qom_tree(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let path = match args.get("path") {
        Some(ArgValue::Str(s)) => s.clone(),
        _ => "/machine".to_string(),
    };

    // Get the type of the root object via qom-list-get
    let results = conn
        .execute(qom_list_get {
            paths: vec![path.clone()],
        })
        .await
        .map_err(CmdError::from)?;

    let type_name = results[0]
        .properties
        .iter()
        .find(|p| p.name == "type")
        .and_then(|p| p.value.as_ref())
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Extract the leaf name from the path
    let name = path.rsplit('/').next().unwrap_or("");

    let mut out = String::new();
    print_qom_tree(conn, &path, name, &type_name, 0, &mut out).await?;

    Ok(out)
}
