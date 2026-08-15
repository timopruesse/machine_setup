//! Config schema — JSON Schema generated from types + Command kind catalog.
//!
//! Structural only; semantic checks stay in `validate`. The checked-in artifact
//! at `schema/machine_setup.schema.json` must match `generate()` (CI / `make schema-check`).

use serde_json::{json, Value};

use crate::engine::commands::catalog::KIND_KEYS;

/// Build the Config document JSON Schema.
pub fn generate() -> Value {
    let command_entry = command_entry_schema();
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://raw.githubusercontent.com/timopruesse/machine_setup/main/schema/machine_setup.schema.json",
        "title": "machine_setup Config document",
        "type": "object",
        "required": ["tasks"],
        "additionalProperties": false,
        "properties": {
            "tasks": {
                "type": "object",
                "description": "Named Tasks",
                "additionalProperties": { "$ref": "#/$defs/task" }
            },
            "temp_dir": {
                "type": "string",
                "description": "Directory for temp files and History (default: ~/.machine_setup)"
            },
            "default_shell": {
                "type": "string",
                "enum": ["bash", "zsh", "powershell"],
                "default": "bash"
            },
            "parallel": {
                "type": "boolean",
                "default": false,
                "description": "Run all Tasks in parallel"
            },
            "num_threads": {
                "type": "integer",
                "minimum": 1,
                "description": "Concurrency gate size (default: physical CPUs - 1)"
            },
            "check_for_updates": {
                "type": "boolean",
                "default": true,
                "description": "When false, skip the post-command self update-check notice"
            }
        },
        "$defs": {
            "stringOrVec": {
                "oneOf": [
                    { "type": "string" },
                    { "type": "array", "items": { "type": "string" } }
                ]
            },
            "osFilter": {
                "oneOf": [
                    { "type": "string" },
                    { "type": "array", "items": { "type": "string" } }
                ]
            },
            "task": {
                "type": "object",
                "required": ["commands"],
                "additionalProperties": false,
                "properties": {
                    "commands": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/commandEntry" }
                    },
                    "os": { "$ref": "#/$defs/osFilter" },
                    "parallel": { "type": "boolean", "default": false },
                    "only_if": { "$ref": "#/$defs/stringOrVec" },
                    "skip_if": { "$ref": "#/$defs/stringOrVec" },
                    "depends_on": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "retry": { "type": "integer", "minimum": 0, "default": 0 },
                    "auto_update": { "$ref": "#/$defs/autoUpdate" }
                }
            },
            "autoUpdate": {
                "type": "object",
                "description": "Daily OS-timer auto-update (at XOR cron; daily only in v1)",
                "additionalProperties": false,
                "properties": {
                    "at": {
                        "type": "string",
                        "description": "Daily local time HH:MM"
                    },
                    "cron": {
                        "type": "string",
                        "description": "5-field cron; v1 accepts daily M H * * * only"
                    }
                }
            },
            "commandEntry": command_entry
        }
    })
}

fn command_entry_schema() -> Value {
    // Single-key map per kind — mirrors custom Deserialize in types.rs.
    let mut variants = Vec::new();
    for key in KIND_KEYS {
        let args = kind_args_schema(key);
        let mut properties = serde_json::Map::new();
        properties.insert((*key).to_string(), args);
        variants.push(json!({
            "type": "object",
            "required": [(*key).to_string()],
            "additionalProperties": false,
            "properties": properties
        }));
    }
    json!({
        "description": "One Command entry (exactly one kind key)",
        "oneOf": variants
    })
}

fn kind_args_schema(kind: &str) -> Value {
    match kind {
        "copy" => json!({
            "type": "object",
            "required": ["src", "target"],
            "additionalProperties": false,
            "properties": {
                "src": { "type": "string" },
                "target": { "type": "string" },
                "ignore": { "type": "array", "items": { "type": "string" } },
                "sudo": { "type": "boolean", "default": false }
            }
        }),
        "symlink" => json!({
            "type": "object",
            "required": ["src", "target"],
            "additionalProperties": false,
            "properties": {
                "src": { "type": "string" },
                "target": { "type": "string" },
                "ignore": { "type": "array", "items": { "type": "string" } },
                "force": { "type": "boolean", "default": false },
                "sudo": { "type": "boolean", "default": false }
            }
        }),
        "clone" => json!({
            "type": "object",
            "required": ["url", "target"],
            "additionalProperties": false,
            "properties": {
                "url": { "type": "string" },
                "target": { "type": "string" }
            }
        }),
        "run" => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "commands": { "$ref": "#/$defs/stringOrVec" },
                "install": { "$ref": "#/$defs/stringOrVec" },
                "update": { "$ref": "#/$defs/stringOrVec" },
                "uninstall": { "$ref": "#/$defs/stringOrVec" },
                "shell": { "type": "string", "enum": ["bash", "zsh", "powershell"] },
                "env": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                }
            }
        }),
        "machine_setup" => json!({
            "type": "object",
            "required": ["config"],
            "additionalProperties": false,
            "properties": {
                "config": { "type": "string" },
                "task": { "type": "string" }
            }
        }),
        other => panic!("KIND_KEYS out of sync with kind_args_schema: {other}"),
    }
}

/// Pretty-printed JSON for the schema artifact / CLI dump.
pub fn generate_pretty() -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&generate())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_lists_all_kind_keys() {
        let schema = generate();
        let one_of = schema["$defs"]["commandEntry"]["oneOf"]
            .as_array()
            .expect("oneOf");
        assert_eq!(one_of.len(), KIND_KEYS.len());
        for key in KIND_KEYS {
            assert!(
                one_of.iter().any(|v| v["required"][0] == *key),
                "missing kind {key}"
            );
        }
    }

    #[test]
    fn schema_requires_tasks() {
        let schema = generate();
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "tasks"));
    }
}
