pub mod history;
pub mod os;
pub mod types;

use std::path::Path;

use crate::error::{Error, Result};
use types::AppConfig;

/// Load a config file, auto-detecting format from extension.
/// If the path has no extension, tries `.yaml`, `.yml`, then `.json`.
pub fn load_config(path: &Path) -> Result<AppConfig> {
    let resolved = resolve_config_path(path)?;
    let content = std::fs::read_to_string(&resolved)?;

    let ext = resolved.extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext {
        "yaml" | "yml" => {
            let config: AppConfig = serde_yaml::from_str(&content)?;
            Ok(config)
        }
        "json" => {
            let config: AppConfig = serde_json::from_str(&content)?;
            Ok(config)
        }
        other => Err(Error::UnsupportedFormat(other.to_string())),
    }
}

/// Resolve the config path, trying common extensions if none is present.
fn resolve_config_path(path: &Path) -> Result<std::path::PathBuf> {
    // If path exists as-is, use it
    if path.exists() && path.is_file() {
        return Ok(path.to_path_buf());
    }

    // Try common extensions
    for ext in &["yaml", "yml", "json"] {
        let with_ext = path.with_extension(ext);
        if with_ext.exists() && with_ext.is_file() {
            return Ok(with_ext);
        }
    }

    Err(Error::ConfigNotFound(path.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_load_yaml_config() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("machine_setup.yaml");
        fs::write(
            &config_path,
            r#"
tasks:
  test:
    commands:
      - run:
          commands: "echo hello"
"#,
        )
        .unwrap();

        let config = load_config(&config_path).unwrap();
        assert_eq!(config.tasks.len(), 1);
    }

    #[test]
    fn test_load_json_config() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        fs::write(
            &config_path,
            r#"{"tasks": {"test": {"commands": [{"run": {"commands": "echo hi"}}]}}}"#,
        )
        .unwrap();

        let config = load_config(&config_path).unwrap();
        assert_eq!(config.tasks.len(), 1);
    }

    #[test]
    fn test_auto_detect_extension() {
        let dir = tempdir().unwrap();
        let yaml_path = dir.path().join("machine_setup.yaml");
        fs::write(
            &yaml_path,
            r#"
tasks:
  test:
    commands:
      - run:
          commands: "echo hello"
"#,
        )
        .unwrap();

        // Pass path without extension
        let base_path = dir.path().join("machine_setup");
        let config = load_config(&base_path).unwrap();
        assert_eq!(config.tasks.len(), 1);
    }

    #[test]
    fn test_config_not_found() {
        let result = load_config(Path::new("/nonexistent/config"));
        assert!(result.is_err());
    }
}
