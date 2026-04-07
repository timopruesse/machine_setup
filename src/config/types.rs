use super::os::OsFilter;
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// Root configuration structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub tasks: IndexMap<String, TaskConfig>,

    /// Directory for temp files and history (default: ~/.machine_setup)
    #[serde(default = "default_temp_dir")]
    pub temp_dir: String,

    /// Default shell for run commands
    #[serde(default = "default_shell")]
    pub default_shell: Shell,

    /// Run all tasks in parallel
    #[serde(default)]
    pub parallel: bool,

    /// Number of threads for parallel execution (default: num_cpus - 1)
    pub num_threads: Option<usize>,
}

fn default_temp_dir() -> String {
    "~/.machine_setup".to_string()
}

fn default_shell() -> Shell {
    Shell::Bash
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shell {
    #[default]
    Bash,
    Zsh,
    #[serde(rename = "powershell")]
    PowerShell,
}

impl std::fmt::Display for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Shell::Bash => write!(f, "bash"),
            Shell::Zsh => write!(f, "zsh"),
            Shell::PowerShell => write!(f, "powershell"),
        }
    }
}

/// A single task definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConfig {
    pub commands: Vec<CommandEntry>,

    /// OS filter — omit to run on all OSes
    #[serde(default)]
    pub os: OsFilter,

    /// Run commands within this task in parallel
    #[serde(default)]
    pub parallel: bool,
}

/// A command entry in the config. Each entry is a single-key map.
/// Example YAML:
/// ```yaml
/// - copy:
///     src: "./files"
///     target: "~/.config"
/// ```
#[derive(Debug, Clone, Serialize)]
pub enum CommandEntry {
    Copy(CopyArgs),
    Symlink(SymlinkArgs),
    Clone(CloneArgs),
    Run(RunArgs),
    MachineSetup(MachineSetupArgs),
}

impl<'de> Deserialize<'de> for CommandEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map: HashMap<String, serde_yaml::Value> = HashMap::deserialize(deserializer)?;

        if map.len() != 1 {
            return Err(serde::de::Error::custom(format!(
                "Expected exactly one command key, found {}",
                map.len()
            )));
        }

        let (key, value) = map.into_iter().next().unwrap();

        match key.as_str() {
            "copy" => {
                let args: CopyArgs =
                    serde_yaml::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(CommandEntry::Copy(args))
            }
            "symlink" => {
                let args: SymlinkArgs =
                    serde_yaml::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(CommandEntry::Symlink(args))
            }
            "clone" => {
                let args: CloneArgs =
                    serde_yaml::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(CommandEntry::Clone(args))
            }
            "run" => {
                let args: RunArgs =
                    serde_yaml::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(CommandEntry::Run(args))
            }
            "machine_setup" => {
                let args: MachineSetupArgs =
                    serde_yaml::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(CommandEntry::MachineSetup(args))
            }
            other => Err(serde::de::Error::custom(format!(
                "Unknown command type: {other}"
            ))),
        }
    }
}

impl std::fmt::Display for CommandEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandEntry::Copy(args) => write!(f, "copy: {} -> {}", args.src, args.target),
            CommandEntry::Symlink(args) => write!(f, "symlink: {} -> {}", args.src, args.target),
            CommandEntry::Clone(args) => write!(f, "clone: {} -> {}", args.url, args.target),
            CommandEntry::Run(args) => {
                let cmds = args.all_command_strings();
                if cmds.is_empty() {
                    write!(f, "run: (no commands)")
                } else if cmds.len() == 1 {
                    write!(f, "run: {}", cmds[0])
                } else {
                    write!(f, "run: {} commands", cmds.len())
                }
            }
            CommandEntry::MachineSetup(args) => {
                write!(f, "machine_setup: {}", args.config)?;
                if let Some(task) = &args.task {
                    write!(f, " (task: {task})")?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyArgs {
    pub src: String,
    pub target: String,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub sudo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymlinkArgs {
    pub src: String,
    pub target: String,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub sudo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneArgs {
    pub url: String,
    pub target: String,
}

/// Run command arguments. Supports both simple and mode-specific commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunArgs {
    /// Commands to run (used for install mode, or all modes if mode-specific not set)
    #[serde(default)]
    pub commands: StringOrVec,

    /// Commands to run only during install
    #[serde(default)]
    pub install: StringOrVec,

    /// Commands to run only during update
    #[serde(default)]
    pub update: StringOrVec,

    /// Commands to run only during uninstall
    #[serde(default)]
    pub uninstall: StringOrVec,

    /// Shell override for this command
    pub shell: Option<Shell>,

    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl RunArgs {
    /// Get all command strings regardless of mode (for display purposes).
    pub fn all_command_strings(&self) -> Vec<&str> {
        let mut cmds = Vec::new();
        cmds.extend(self.commands.as_slice().iter().map(|s| s.as_str()));
        cmds.extend(self.install.as_slice().iter().map(|s| s.as_str()));
        cmds.extend(self.update.as_slice().iter().map(|s| s.as_str()));
        cmds.extend(self.uninstall.as_slice().iter().map(|s| s.as_str()));
        cmds
    }

    /// Get commands for a specific mode.
    pub fn commands_for_mode(&self, mode: &crate::cli::Command) -> &[String] {
        match mode {
            crate::cli::Command::Install => {
                if !self.install.as_slice().is_empty() {
                    self.install.as_slice()
                } else {
                    self.commands.as_slice()
                }
            }
            crate::cli::Command::Update => {
                if !self.update.as_slice().is_empty() {
                    self.update.as_slice()
                } else {
                    // In v1, update only runs if explicitly defined
                    &[]
                }
            }
            crate::cli::Command::Uninstall => {
                if !self.uninstall.as_slice().is_empty() {
                    self.uninstall.as_slice()
                } else {
                    &[]
                }
            }
            crate::cli::Command::List => &[],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineSetupArgs {
    pub config: String,
    pub task: Option<String>,
}

/// A value that can be a single string or a list of strings.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StringOrVec(Vec<String>);

impl StringOrVec {
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for StringOrVec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Inner {
            Single(String),
            Multiple(Vec<String>),
        }

        match Inner::deserialize(deserializer)? {
            Inner::Single(s) => Ok(StringOrVec(vec![s])),
            Inner::Multiple(v) => Ok(StringOrVec(v)),
        }
    }
}

impl AppConfig {
    /// Check if any commands in the selected tasks require sudo.
    pub fn requires_sudo(&self, task_names: &[String]) -> bool {
        self.tasks
            .iter()
            .filter(|(name, _)| task_names.iter().any(|t| t == *name))
            .any(|(_, task)| {
                task.commands.iter().any(|cmd| match cmd {
                    CommandEntry::Run(args) => args
                        .all_command_strings()
                        .iter()
                        .any(|s| s.contains("sudo")),
                    CommandEntry::Copy(args) => args.sudo,
                    CommandEntry::Symlink(args) => args.sudo,
                    _ => false,
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let yaml = r#"
tasks:
  test_task:
    commands:
      - run:
          commands: "echo hello"
"#;
        let config: AppConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.tasks.len(), 1);
        assert!(config.tasks.contains_key("test_task"));
    }

    #[test]
    fn test_parse_full_config() {
        let yaml = r#"
temp_dir: "~/.my_setup"
default_shell: "zsh"
parallel: true
num_threads: 4

tasks:
  dotfiles:
    os: ["linux", "macos"]
    parallel: false
    commands:
      - clone:
          url: "git@github.com:user/.dotfiles.git"
          target: "~/.dotfiles"
      - symlink:
          src: "~/.dotfiles/config"
          target: "~/.config"
          ignore: ["README.md"]
      - copy:
          src: "./extra"
          target: "~/.local"
      - run:
          commands:
            - "echo done"
          env:
            MY_VAR: "hello"
      - machine_setup:
          config: "./other.yaml"
          task: "sub_task"
"#;
        let config: AppConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.temp_dir, "~/.my_setup");
        assert_eq!(config.default_shell, Shell::Zsh);
        assert!(config.parallel);
        assert_eq!(config.num_threads, Some(4));

        let task = &config.tasks["dotfiles"];
        assert_eq!(task.commands.len(), 5);
        assert!(!task.parallel);
    }

    #[test]
    fn test_string_or_vec_single() {
        let val: StringOrVec = serde_yaml::from_str(r#""hello""#).unwrap();
        assert_eq!(val.as_slice(), &["hello"]);
    }

    #[test]
    fn test_string_or_vec_multiple() {
        let val: StringOrVec = serde_yaml::from_str(r#"["a", "b"]"#).unwrap();
        assert_eq!(val.as_slice(), &["a", "b"]);
    }

    #[test]
    fn test_run_args_mode_specific() {
        let yaml = r#"
install: "npm install"
update: "npm update"
uninstall: "npm uninstall"
"#;
        let args: RunArgs = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            args.commands_for_mode(&crate::cli::Command::Install),
            &["npm install"]
        );
        assert_eq!(
            args.commands_for_mode(&crate::cli::Command::Update),
            &["npm update"]
        );
        assert_eq!(
            args.commands_for_mode(&crate::cli::Command::Uninstall),
            &["npm uninstall"]
        );
    }

    #[test]
    fn test_parse_json_config() {
        let json = r#"{
            "tasks": {
                "test": {
                    "commands": [
                        {"run": {"commands": "echo hi"}}
                    ]
                }
            }
        }"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.tasks.len(), 1);
    }

    #[test]
    fn test_run_args_env_parsing() {
        let yaml = r#"
env:
  MY_VAR: "test_value"
  OTHER: "hello"
commands: "echo $MY_VAR"
"#;
        let args: RunArgs = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(args.env.len(), 2);
        assert_eq!(args.env.get("MY_VAR").unwrap(), "test_value");
        assert_eq!(args.env.get("OTHER").unwrap(), "hello");
    }

    #[test]
    fn test_command_entry_display() {
        let entry = CommandEntry::Copy(CopyArgs {
            src: "./src".to_string(),
            target: "~/dest".to_string(),
            ignore: vec![],
            sudo: false,
        });
        assert_eq!(format!("{entry}"), "copy: ./src -> ~/dest");
    }
}
