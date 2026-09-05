use super::os::OsFilter;
use crate::engine::mode::Mode;
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// Root configuration structure.
///
/// Field order matters for YAML serialization: `tasks` must be last so
/// append-only `add task` can extend the `tasks:` map at EOF.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
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

    /// When false, skip the post-command self update-check notice (default true).
    #[serde(default = "default_true")]
    pub check_for_updates: bool,

    pub tasks: IndexMap<String, Arc<TaskConfig>>,
}

fn default_temp_dir() -> String {
    "~/.machine_setup".to_string()
}

fn default_shell() -> Shell {
    Shell::Bash
}

fn default_true() -> bool {
    true
}

fn default_retry_delay() -> u64 {
    1
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

fn is_default_retry_delay(value: &u64) -> bool {
    *value == default_retry_delay()
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

/// Optional daily auto-update schedule for a Task.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoUpdateConfig {
    /// Daily local clock time, e.g. `"07:30"`. Mutually exclusive with `cron`.
    #[serde(default)]
    pub at: Option<String>,

    /// 5-field cron. v1 accepts daily forms only (`M H * * *`). Mutually exclusive with `at`.
    #[serde(default)]
    pub cron: Option<String>,
}

/// A single task definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConfig {
    pub commands: Vec<CommandEntry>,

    /// OS filter — omit to run on all OSes
    #[serde(default, skip_serializing_if = "OsFilter::is_all")]
    pub os: OsFilter,

    /// Run commands within this task in parallel
    #[serde(default, skip_serializing_if = "is_false")]
    pub parallel: bool,

    /// Only run when all conditions are satisfied
    #[serde(default, skip_serializing_if = "Conditions::is_empty")]
    pub only_if: Conditions,

    /// Skip when any condition is satisfied
    #[serde(default, skip_serializing_if = "Conditions::is_empty")]
    pub skip_if: Conditions,

    /// Task names that must complete before this task runs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,

    /// Number of retry attempts on failure (0 = no retry)
    #[serde(default, skip_serializing_if = "is_zero")]
    pub retry: u32,

    /// Seconds to wait between retry attempts (default: 1)
    #[serde(
        default = "default_retry_delay",
        skip_serializing_if = "is_default_retry_delay"
    )]
    pub retry_delay_secs: u64,

    /// Daily OS-timer auto-update (see `schedule apply`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_update: Option<AutoUpdateConfig>,
}

/// Default-empty task body for authoring stubs and recipe emitters.
pub fn blank_task_config() -> TaskConfig {
    TaskConfig {
        commands: vec![],
        os: OsFilter::All,
        parallel: false,
        only_if: Conditions::default(),
        skip_if: Conditions::default(),
        depends_on: vec![],
        retry: 0,
        retry_delay_secs: 1,
        auto_update: None,
    }
}

/// A command entry in the config. Each entry is a single-key map.
/// Example YAML:
/// ```yaml
/// - copy:
///     src: "./files"
///     target: "~/.config"
/// ```
#[derive(Debug, Clone)]
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

        #[expect(clippy::unwrap_used, reason = "len == 1 was checked immediately above")]
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

impl Serialize for CommandEntry {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            CommandEntry::Copy(a) => map.serialize_entry("copy", a)?,
            CommandEntry::Symlink(a) => map.serialize_entry("symlink", a)?,
            CommandEntry::Clone(a) => map.serialize_entry("clone", a)?,
            CommandEntry::Run(a) => map.serialize_entry("run", a)?,
            CommandEntry::MachineSetup(a) => map.serialize_entry("machine_setup", a)?,
        }
        map.end()
    }
}

impl std::fmt::Display for CommandEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Behavioral display lives in the Command kind catalog (ADR-0006).
        f.write_str(&crate::engine::commands::catalog::description(self))
    }
}

impl std::fmt::Display for CopyArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = if self.sudo { "copy (sudo)" } else { "copy" };
        write!(f, "{prefix}: {} -> {}", self.src, self.target)
    }
}

impl std::fmt::Display for SymlinkArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = if self.sudo {
            "symlink (sudo)"
        } else {
            "symlink"
        };
        write!(f, "{prefix}: {} -> {}", self.src, self.target)
    }
}

impl std::fmt::Display for CloneArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "clone: {} -> {}", self.url, self.target)
    }
}

impl std::fmt::Display for RunArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.all_command_strings();
        match (iter.next(), iter.next()) {
            (None, _) => write!(f, "run: (no commands)"),
            (Some(c), None) => write!(f, "run: {c}"),
            (Some(_), Some(_)) => write!(f, "run: {} commands", 2 + iter.count()),
        }
    }
}

impl std::fmt::Display for MachineSetupArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "machine_setup: {}", self.config)?;
        if let Some(task) = &self.task {
            write!(f, " (task: {task})")?;
        }
        Ok(())
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

    /// When true, suppress subprocess stdout (stderr still logged; failures surface errors).
    #[serde(default)]
    pub quiet: bool,
}

impl RunArgs {
    /// Iterate all command strings regardless of mode (for display purposes).
    /// Returns an iterator so callers that only need count/first/is_empty
    /// don't force an intermediate Vec allocation.
    pub fn all_command_strings(&self) -> impl Iterator<Item = &str> {
        self.commands
            .as_slice()
            .iter()
            .chain(self.install.as_slice().iter())
            .chain(self.update.as_slice().iter())
            .chain(self.uninstall.as_slice().iter())
            .map(|s| s.as_str())
    }

    /// Get commands for a specific execution mode.
    pub fn commands_for_mode(&self, mode: crate::engine::mode::Mode) -> &[String] {
        use crate::engine::mode::Mode;
        match mode {
            Mode::Install => {
                if !self.install.as_slice().is_empty() {
                    self.install.as_slice()
                } else {
                    self.commands.as_slice()
                }
            }
            Mode::Update => {
                if !self.update.as_slice().is_empty() {
                    self.update.as_slice()
                } else {
                    // Update only runs if explicitly defined.
                    &[]
                }
            }
            Mode::Uninstall => {
                if !self.uninstall.as_slice().is_empty() {
                    self.uninstall.as_slice()
                } else {
                    &[]
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineSetupArgs {
    pub config: String,
    pub task: Option<String>,
    /// Bypass History skip in the nested Runner (default false).
    #[serde(default)]
    pub force: bool,
    /// When `task` is set, expand transitive `depends_on` like CLI `--with-deps`
    /// (default false; no-op when `task` is omitted).
    #[serde(default)]
    pub with_deps: bool,
}

/// A single task condition (path, env, command, or mode).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    Path(String),
    Env(String),
    Command(String),
    Mode(Vec<Mode>),
}

impl Serialize for Condition {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        match self {
            Condition::Path(p) => serializer.serialize_str(p),
            Condition::Env(e) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("env", e)?;
                map.end()
            }
            Condition::Command(c) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("command", c)?;
                map.end()
            }
            Condition::Mode(modes) => {
                let mut map = serializer.serialize_map(Some(1))?;
                if modes.len() == 1 {
                    map.serialize_entry("mode", &modes[0])?;
                } else {
                    map.serialize_entry("mode", modes)?;
                }
                map.end()
            }
        }
    }
}

/// Task conditions — backward compatible with plain path strings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Conditions(Vec<Condition>);

impl Conditions {
    pub fn iter(&self) -> impl Iterator<Item = &Condition> {
        self.0.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Vec<Condition>> for Conditions {
    fn from(value: Vec<Condition>) -> Self {
        Self(value)
    }
}

impl<'de> Deserialize<'de> for Conditions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ConditionsVisitor;

        impl<'de> serde::de::Visitor<'de> for ConditionsVisitor {
            type Value = Conditions;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a path string or a list of condition strings or objects")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Conditions(vec![Condition::Path(v.to_string())]))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut conditions = Vec::new();
                while let Some(entry) = seq.next_element::<ConditionEntry>()? {
                    conditions.push(match entry {
                        ConditionEntry::PathStr(s) => Condition::Path(s),
                        ConditionEntry::Obj(obj) => obj.into_condition(),
                    });
                }
                Ok(Conditions(conditions))
            }
        }

        deserializer.deserialize_any(ConditionsVisitor)
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ConditionEntry {
    PathStr(String),
    Obj(ConditionObj),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ConditionObj {
    Path { path: String },
    Env { env: String },
    Command { command: String },
    Mode { mode: ModeConditionValue },
}

impl ConditionObj {
    fn into_condition(self) -> Condition {
        match self {
            ConditionObj::Path { path } => Condition::Path(path),
            ConditionObj::Env { env } => Condition::Env(env),
            ConditionObj::Command { command } => Condition::Command(command),
            ConditionObj::Mode { mode } => Condition::Mode(mode.into_modes()),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ModeConditionValue {
    One(Mode),
    Many(Vec<Mode>),
}

impl ModeConditionValue {
    fn into_modes(self) -> Vec<Mode> {
        match self {
            ModeConditionValue::One(m) => vec![m],
            ModeConditionValue::Many(v) => v,
        }
    }
}

/// A value that can be a single string or a list of strings.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StringOrVec(Vec<String>);

impl StringOrVec {
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    pub fn as_mut_slice(&mut self) -> &mut [String] {
        &mut self.0
    }
}

impl From<String> for StringOrVec {
    fn from(s: String) -> Self {
        StringOrVec(vec![s])
    }
}

impl From<&str> for StringOrVec {
    fn from(s: &str) -> Self {
        StringOrVec(vec![s.to_string()])
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
        crate::engine::commands::catalog::tasks_require_sudo(self, task_names)
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
    fn test_conditions_path_string_compat() {
        let val: Conditions = serde_yaml::from_str(r#""~/.ssh""#).unwrap();
        assert_eq!(
            val.iter().collect::<Vec<_>>(),
            vec![&Condition::Path("~/.ssh".into())]
        );
    }

    #[test]
    fn test_conditions_path_list_compat() {
        let val: Conditions = serde_yaml::from_str(r#"["/a", "/b"]"#).unwrap();
        assert_eq!(val.iter().count(), 2);
    }

    #[test]
    fn test_conditions_rich_forms() {
        let yaml = r#"
- path: "/etc/hosts"
- env: "HOME"
- command: "which git"
- mode: install
- mode: [update, uninstall]
"#;
        let val: Conditions = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(val.iter().count(), 5);
    }

    #[test]
    fn test_run_args_mode_specific() {
        let yaml = r#"
install: "npm install"
update: "npm update"
uninstall: "npm uninstall"
"#;
        let args: RunArgs = serde_yaml::from_str(yaml).unwrap();
        use crate::engine::mode::Mode;
        assert_eq!(args.commands_for_mode(Mode::Install), &["npm install"]);
        assert_eq!(args.commands_for_mode(Mode::Update), &["npm update"]);
        assert_eq!(args.commands_for_mode(Mode::Uninstall), &["npm uninstall"]);
    }

    #[test]
    fn test_run_args_install_falls_back_to_commands() {
        // When no mode-specific `install` is set, install mode uses `commands`.
        let yaml = r#"
commands: "echo shared"
"#;
        let args: RunArgs = serde_yaml::from_str(yaml).unwrap();
        use crate::engine::mode::Mode;
        assert_eq!(args.commands_for_mode(Mode::Install), &["echo shared"]);
        // Update/uninstall don't fall back to `commands`.
        assert!(args.commands_for_mode(Mode::Update).is_empty());
        assert!(args.commands_for_mode(Mode::Uninstall).is_empty());
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

    #[test]
    fn command_entry_yaml_roundtrip_copy() {
        let yaml = r#"
- copy:
    src: ./a
    target: ~/b
"#;
        let entries: Vec<CommandEntry> = serde_yaml::from_str(yaml).unwrap();
        let out = serde_yaml::to_string(&entries).unwrap();
        let again: Vec<CommandEntry> = serde_yaml::from_str(&out).unwrap();
        assert!(matches!(again[0], CommandEntry::Copy(_)));
    }

    #[test]
    fn command_entry_yaml_roundtrip_run_symlink_clone() {
        let yaml = r#"
- run:
    commands: "echo hi"
- symlink:
    src: ./x
    target: ~/x
    force: true
- clone:
    url: https://example.com/r.git
    target: ~/r
"#;
        let entries: Vec<CommandEntry> = serde_yaml::from_str(yaml).unwrap();
        let out = serde_yaml::to_string(&entries).unwrap();
        let again: Vec<CommandEntry> = serde_yaml::from_str(&out).unwrap();
        assert_eq!(again.len(), 3);
    }

    #[test]
    fn conditions_yaml_roundtrip() {
        let yaml = r#"
- ~/.ssh
- env: HOME
- command: "true"
- mode: install
"#;
        let c: Conditions = serde_yaml::from_str(yaml).unwrap();
        let out = serde_yaml::to_string(&c).unwrap();
        let again: Conditions = serde_yaml::from_str(&out).unwrap();
        assert_eq!(c, again);
    }
}
