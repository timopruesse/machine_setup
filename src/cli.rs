use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "machine_setup",
    version,
    about = "Automate machine configuration and setup tasks"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Path or URL to config file (YAML/JSON). When omitted, searches cwd then git root.
    #[arg(short, long, global = true)]
    pub config: Option<String>,

    /// Run only a specific task by name
    #[arg(short, long, global = true)]
    pub task: Option<String>,

    /// Interactively select tasks to run
    #[arg(short, long, global = true)]
    pub select: bool,

    /// Expand transitive dependencies of selected tasks (install / `--with-deps`)
    #[arg(long, global = true)]
    pub with_deps: bool,

    /// Force execution (bypass history checks)
    #[arg(short, long, global = true)]
    pub force: bool,

    /// Disable TUI (plain log output)
    #[arg(long, global = true)]
    pub no_tui: bool,

    /// Enable debug output
    #[arg(short, long, global = true)]
    pub debug: bool,

    /// Log level
    #[arg(short, long, global = true, default_value = "warn")]
    pub level: String,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Install all or selected tasks
    Install,
    /// Update all or selected tasks
    Update,
    /// Uninstall all or selected tasks
    Uninstall,
    /// List defined tasks with install status from History
    List,
    /// Validate config file without executing
    Validate,
    /// Create a new empty Config document (refuses if it already exists)
    Init,
    /// Append to the Config document
    Add {
        #[command(subcommand)]
        target: AddTarget,
    },
    /// Print the Config schema (JSON Schema) to stdout
    Schema,
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum AddTarget {
    /// Append a minimal Task stub
    Task {
        /// Task name
        name: String,
    },
}

impl std::fmt::Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::Install => write!(f, "install"),
            Command::Update => write!(f, "update"),
            Command::Uninstall => write!(f, "uninstall"),
            Command::List => write!(f, "list"),
            Command::Validate => write!(f, "validate"),
            Command::Init => write!(f, "init"),
            Command::Add { .. } => write!(f, "add"),
            Command::Schema => write!(f, "schema"),
            Command::Completions { .. } => write!(f, "completions"),
        }
    }
}
