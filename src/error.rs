use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Config file not found: {0}")]
    ConfigNotFound(PathBuf),

    #[error("Unsupported config format: {0}")]
    UnsupportedFormat(String),

    #[error("Failed to parse YAML config: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    #[error("Failed to parse JSON config: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Shell execution failed: {0}")]
    ShellFailed(String),

    #[error("Git operation failed: {0}")]
    GitFailed(String),

    #[error("Path error: {0}")]
    PathError(String),

    #[error("History error: {0}")]
    HistoryError(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
