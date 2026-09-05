use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Config file not found: {0}")]
    ConfigNotFound(PathBuf),

    #[error("Config file already exists: {0}")]
    ConfigAlreadyExists(PathBuf),

    #[error("Task already exists in Config document: {0}")]
    TaskAlreadyExists(String),

    #[error(
        "No Config document found (searched working directory, then git root). \
         Run `machine_setup init` or pass `-c <path>`."
    )]
    ConfigNotLocated,

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

    #[error("Invalid task name: {0}")]
    InvalidTaskName(String),

    #[error("Shell execution failed: {0}")]
    ShellFailed(String),

    #[error("Git operation failed: {0}")]
    GitFailed(String),

    #[error("Path error: {0}")]
    PathError(String),

    #[error("History error: {0}")]
    HistoryError(String),

    #[error("Schedule error: {0}")]
    ScheduleError(String),

    #[error("Sudo failed: {0}")]
    SudoFailed(String),

    #[error("Authoring recipe error: {0}")]
    RecipeError(String),

    #[error("Interactive prompt failed: {0}")]
    PromptFailed(String),

    #[error("Aborted.")]
    Aborted,

    #[error("Wizard requires an interactive terminal; use `init` / `add` instead")]
    WizardRequiresTty,

    #[error("Wizard requires a local Config document path, not a URL")]
    WizardRequiresLocalPath,

    #[error("Config document has validation errors")]
    ConfigValidationFailed,

    #[error("Failed to fetch remote config: {0}")]
    ConfigFetchFailed(String),

    #[error("Update check failed: {0}")]
    UpdateCheckFailed(String),

    #[error("{0} task(s) failed")]
    TasksFailed(usize),

    #[error("Task join failed: {0}")]
    TaskJoin(String),

    #[error("Cyclic dependency detected: {0}")]
    CyclicDependency(String),

    #[error("Unknown dependency: task '{0}' depends on '{1}' which does not exist")]
    MissingDependency(String, String),

    #[error(
        "Cannot remove task '{task}': still depended on by: {dependents}. \
         Re-run with `--fix-deps` to strip those edges, or run interactively to choose."
    )]
    RemoveBlocked { task: String, dependents: String },
}

pub type Result<T> = std::result::Result<T, Error>;
