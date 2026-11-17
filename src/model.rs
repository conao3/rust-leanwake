use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct Profile {
    pub schema_version: u32,
    pub tool: Tool,
    pub toolchain: String,
    pub conditions: Conditions,
    pub files: Vec<File>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct Tool {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct Conditions {
    pub r#async: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct File {
    pub path: String,
    pub module: String,
    pub source_sha256: String,
    pub has_errors: bool,
    pub import_elapsed_ns: u64,
    pub commands: Vec<Command>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct Command {
    pub key: String,
    pub kind: String,
    pub names: Vec<String>,
    pub range: Range,
    pub heartbeats: u64,
    pub elapsed_ns: u64,
    pub has_errors: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct Position {
    pub line: u64,
    pub column: u64,
}
