pub mod history;
pub mod os;
pub mod types;
pub mod validate;

use std::path::Path;

use crate::error::{Error, Result};
use types::AppConfig;

/// Load a config from a local path or URL, auto-detecting format from extension.
/// If the path has no extension, tries `.yaml`, `.yml`, then `.json`.
///
/// Supported URL formats:
/// - `https://github.com/user/repo/blob/branch/file.yaml` (auto-converted to raw URL)
/// - `https://raw.githubusercontent.com/user/repo/branch/file.yaml`
/// - Any other URL returning YAML/JSON content
pub fn load_config(path_or_url: &str) -> Result<AppConfig> {
    if is_url(path_or_url) {
        load_config_from_url(path_or_url)
    } else {
        load_config_from_path(Path::new(path_or_url))
    }
}

pub fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

fn load_config_from_url(url: &str) -> Result<AppConfig> {
    let raw_url = to_raw_url(url);

    eprintln!("Fetching config from {}...", raw_url);

    let response = ureq::get(&raw_url)
        .call()
        .map_err(|e| Error::Other(format!("Failed to fetch config: {e}")))?;

    let content = response
        .into_body()
        .read_to_string()
        .map_err(|e| Error::Other(format!("Failed to read response: {e}")))?;

    let ext = url_extension(&raw_url);

    parse_config(&content, &ext)
}

fn load_config_from_path(path: &Path) -> Result<AppConfig> {
    let resolved = resolve_config_path(path)?;
    let content = std::fs::read_to_string(&resolved)?;
    let ext = resolved.extension().and_then(|e| e.to_str()).unwrap_or("");

    parse_config(&content, ext)
}

fn parse_config(content: &str, ext: &str) -> Result<AppConfig> {
    match ext {
        "yaml" | "yml" => {
            let config: AppConfig = serde_yaml::from_str(content)?;
            Ok(config)
        }
        "json" => {
            let config: AppConfig = serde_json::from_str(content)?;
            Ok(config)
        }
        other => Err(Error::UnsupportedFormat(other.to_string())),
    }
}

/// Convert GitHub blob URLs to raw.githubusercontent.com URLs.
/// Other URLs are returned as-is.
fn to_raw_url(url: &str) -> String {
    // https://github.com/user/repo/blob/branch/path/to/file.yaml
    // → https://raw.githubusercontent.com/user/repo/branch/path/to/file.yaml
    if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        if let Some(after_blob) = rest.split_once("/blob/") {
            return format!(
                "https://raw.githubusercontent.com/{}/{}",
                after_blob.0, after_blob.1
            );
        }
    }
    url.to_string()
}

/// Extract file extension from a URL path.
fn url_extension(url: &str) -> String {
    // Strip query string and fragment
    let path = url.split('?').next().unwrap_or(url);
    let path = path.split('#').next().unwrap_or(path);

    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("yaml")
        .to_string()
}

/// Resolve the config path, trying common extensions if none is present.
fn resolve_config_path(path: &Path) -> Result<std::path::PathBuf> {
    if path.exists() && path.is_file() {
        return Ok(path.to_path_buf());
    }

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

        let config = load_config(config_path.to_str().unwrap()).unwrap();
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

        let config = load_config(config_path.to_str().unwrap()).unwrap();
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

        let base_path = dir.path().join("machine_setup");
        let config = load_config(base_path.to_str().unwrap()).unwrap();
        assert_eq!(config.tasks.len(), 1);
    }

    #[test]
    fn test_config_not_found() {
        let result = load_config("/nonexistent/config");
        assert!(result.is_err());
    }

    #[test]
    fn test_is_url() {
        assert!(is_url("https://github.com/user/repo/blob/main/file.yaml"));
        assert!(is_url("http://example.com/config.yaml"));
        assert!(!is_url("./machine_setup.yaml"));
        assert!(!is_url("/home/user/config.yaml"));
    }

    #[test]
    fn test_to_raw_url_github_blob() {
        assert_eq!(
            to_raw_url("https://github.com/timopruesse/.dotfiles/blob/main/machine_setup.yaml"),
            "https://raw.githubusercontent.com/timopruesse/.dotfiles/main/machine_setup.yaml"
        );
    }

    #[test]
    fn test_to_raw_url_already_raw() {
        let url = "https://raw.githubusercontent.com/user/repo/main/file.yaml";
        assert_eq!(to_raw_url(url), url);
    }

    #[test]
    fn test_to_raw_url_non_github() {
        let url = "https://example.com/config.yaml";
        assert_eq!(to_raw_url(url), url);
    }

    #[test]
    fn test_url_extension() {
        assert_eq!(url_extension("https://example.com/config.yaml"), "yaml");
        assert_eq!(
            url_extension("https://example.com/config.json?token=abc"),
            "json"
        );
        assert_eq!(
            url_extension("https://example.com/config.yml#section"),
            "yml"
        );
    }
}
