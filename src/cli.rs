use clap::{Parser, Subcommand};
use std::path::PathBuf;

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

    /// Preview execution without making filesystem changes or running commands
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Backup existing files before overwriting with symlinks
    #[arg(long, global = true)]
    pub backup: bool,

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
    /// Report Task status, validation issues, and orphan History entries
    Doctor {
        /// Remove History entries for Tasks no longer in the Config document
        #[arg(long)]
        fix: bool,
    },
    /// Create a new empty Config document (refuses if it already exists)
    Init,
    /// Interactive Config document setup (TTY required)
    Wizard,
    /// Append to the Config document
    Add {
        #[command(subcommand)]
        target: AddTarget,
    },
    /// Remove from the Config document (rewrites the file; comments/formatting may change)
    Remove {
        #[command(subcommand)]
        target: RemoveTarget,
    },
    /// Replace (upsert) a Task in the Config document (rewrites the file; comments/formatting may change)
    Replace {
        #[command(subcommand)]
        target: ReplaceTarget,
    },
    /// Manage OS-timer auto-update schedules
    Schedule {
        #[command(subcommand)]
        action: ScheduleAction,
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
pub enum ScheduleAction {
    /// Install/refresh OS timer units from the Config document
    Apply {
        /// Do not modify shell rc files (hook script under temp_dir is still written)
        #[arg(long)]
        no_install_hook: bool,
    },
    /// Remove managed OS timer units
    Remove {
        /// Leave shell hook stubs and hook script in place
        #[arg(long)]
        keep_hook: bool,
    },
    /// Run updates for all installed tasks with this schedule key (timer entrypoint)
    Run {
        /// Schedule key (`0730`) or clock time (`07:30`)
        #[arg(long)]
        key: String,
    },
    /// Show configured schedules and managed units
    Status,
    /// Print and clear one unseen schedule notice (shell hook target)
    Notify {
        /// Temp dir with schedule_notices.json (hook embeds this; default ~/.machine_setup)
        #[arg(long = "temp-dir", value_name = "DIR")]
        temp_dir: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum RemoveTarget {
    /// Delete a Task by name
    Task {
        /// Task name
        name: String,
        /// Strip this name from other Tasks' depends_on, then remove (required non-interactively when dependents exist)
        #[arg(long)]
        fix_deps: bool,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum ReplaceTarget {
    /// Replace a Task by name (creates if missing)
    Task {
        /// Task name
        name: String,
    },
    /// Replace a Task from an Authoring recipe (existing Command entry kinds only)
    Recipe {
        #[command(subcommand)]
        recipe: RecipeCommand,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum AddTarget {
    /// Append a minimal Task stub
    Task {
        /// Task name
        name: String,
    },
    /// Append a Task from an Authoring recipe (existing Command entry kinds only)
    Recipe {
        #[command(subcommand)]
        recipe: RecipeCommand,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum RecipeCommand {
    /// Clone a dotfiles repo into `.` and symlink `src` → `target`
    Dotfiles {
        /// Git repository URL
        #[arg(long)]
        url: String,
        /// Symlink source directory inside the clone
        #[arg(long, default_value = "./home")]
        src: String,
        /// Symlink target (usually home)
        #[arg(long, default_value = "~")]
        target: String,
        /// Extra ignore patterns (`.cursor` is always included)
        #[arg(long)]
        ignore: Vec<String>,
        /// Task name
        #[arg(long, default_value = "dotfiles")]
        name: String,
    },
    /// Clone a single git repository
    GitRepo {
        /// Git repository URL
        #[arg(long)]
        url: String,
        /// Clone destination
        #[arg(long)]
        target: String,
        /// Task name
        #[arg(long, default_value = "git-repo")]
        name: String,
    },
    /// macOS `brew bundle` on install and update
    BrewBundle {
        /// Path to Brewfile
        #[arg(long)]
        file: String,
        /// Task name
        #[arg(long, default_value = "brew-bundle")]
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
            Command::Doctor { .. } => write!(f, "doctor"),
            Command::Init => write!(f, "init"),
            Command::Wizard => write!(f, "wizard"),
            Command::Add { .. } => write!(f, "add"),
            Command::Remove { .. } => write!(f, "remove"),
            Command::Replace { .. } => write!(f, "replace"),
            Command::Schedule { .. } => write!(f, "schedule"),
            Command::Schema => write!(f, "schema"),
            Command::Completions { .. } => write!(f, "completions"),
        }
    }
}
