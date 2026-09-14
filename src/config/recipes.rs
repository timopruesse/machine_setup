//! Authoring recipes — named emitters of Tasks built from existing Command entry kinds.
//!
//! Not new kinds (ADR-0006). Output is append-only YAML via the Config document module.
//! CLI `add recipe` and the Config wizard both dispatch through this catalog.

use crate::cli::RecipeCommand;
use crate::config::os::{Os, OsFilter};
use crate::config::types::{
    blank_task_config, CloneArgs, CommandEntry, RunArgs, SymlinkArgs, TaskConfig,
};
use crate::error::{Error, Result};

/// A Task ready to append under `tasks:`.
#[derive(Debug, Clone)]
pub struct EmittedTask {
    pub name: String,
    pub task: TaskConfig,
}

/// Parameters for the `dotfiles` recipe.
#[derive(Debug, Clone)]
pub struct DotfilesParams<'a> {
    pub name: &'a str,
    pub url: &'a str,
    /// Symlink source (default `./home`).
    pub src: &'a str,
    /// Symlink target (default `~`).
    pub target: &'a str,
    /// Extra ignore entries (`.cursor` is always included).
    pub ignore: Vec<&'a str>,
}

/// Parameters for the `git-repo` recipe.
#[derive(Debug, Clone)]
pub struct GitRepoParams<'a> {
    pub name: &'a str,
    pub url: &'a str,
    pub target: &'a str,
}

/// Parameters for the `brew-bundle` recipe.
#[derive(Debug, Clone)]
pub struct BrewBundleParams<'a> {
    pub name: &'a str,
    pub file: &'a str,
}

/// Registered recipe keys in menu / CLI order.
pub const RECIPE_KEYS: &[&str] = &["dotfiles", "git-repo", "brew-bundle", "onepassword-ssh"];

pub const DEFAULT_DOTFILES_NAME: &str = "dotfiles";
pub const DEFAULT_GIT_REPO_NAME: &str = "git-repo";
pub const DEFAULT_BREW_BUNDLE_NAME: &str = "brew-bundle";
pub const DEFAULT_ONEPASSWORD_SSH_NAME: &str = "onepassword-ssh";
pub const DEFAULT_DOTFILES_SRC: &str = "./home";
pub const DEFAULT_DOTFILES_TARGET: &str = "~";
pub const DEFAULT_DOTFILES_IGNORE: &str = ".cursor";

/// Wizard menu labels for each [`RECIPE_KEYS`] entry, same order.
pub fn recipe_menu_labels() -> &'static [&'static str] {
    &[
        "Add recipe: dotfiles",
        "Add recipe: git-repo",
        "Add recipe: brew-bundle",
        "Add recipe: onepassword-ssh",
    ]
}

/// Params collected from the wizard (or other callers) before emit-by-key dispatch.
pub enum RecipeEmitInput<'a> {
    Dotfiles(DotfilesParams<'a>),
    GitRepo(GitRepoParams<'a>),
    BrewBundle(BrewBundleParams<'a>),
    OnePasswordSsh(OnePasswordSshParams<'a>),
}

/// Parameters for the `onepassword-ssh` recipe.
#[derive(Debug, Clone)]
pub struct OnePasswordSshParams<'a> {
    pub name: &'a str,
}

/// Default task name for a catalog recipe key.
pub fn default_name_for_key(key: &str) -> Option<&'static str> {
    match key {
        "dotfiles" => Some(DEFAULT_DOTFILES_NAME),
        "git-repo" => Some(DEFAULT_GIT_REPO_NAME),
        "brew-bundle" => Some(DEFAULT_BREW_BUNDLE_NAME),
        "onepassword-ssh" => Some(DEFAULT_ONEPASSWORD_SSH_NAME),
        _ => None,
    }
}

/// Dispatch to the emitter registered for `key`.
pub fn emit_by_key(key: &str, input: RecipeEmitInput<'_>) -> Result<EmittedTask> {
    match key {
        "dotfiles" => {
            let RecipeEmitInput::Dotfiles(p) = input else {
                return Err(Error::RecipeError(format!(
                    "recipe `{key}` expects dotfiles params"
                )));
            };
            emit_dotfiles(&p)
        }
        "git-repo" => {
            let RecipeEmitInput::GitRepo(p) = input else {
                return Err(Error::RecipeError(format!(
                    "recipe `{key}` expects git-repo params"
                )));
            };
            emit_git_repo(&p)
        }
        "brew-bundle" => {
            let RecipeEmitInput::BrewBundle(p) = input else {
                return Err(Error::RecipeError(format!(
                    "recipe `{key}` expects brew-bundle params"
                )));
            };
            emit_brew_bundle(&p)
        }
        "onepassword-ssh" => {
            let RecipeEmitInput::OnePasswordSsh(p) = input else {
                return Err(Error::RecipeError(format!(
                    "recipe `{key}` expects onepassword-ssh params"
                )));
            };
            emit_onepassword_ssh(&p)
        }
        other => Err(Error::RecipeError(format!("unknown recipe key: {other}"))),
    }
}

/// Single CLI dispatch site for `add recipe` subcommands.
pub fn emit_from_cli(cmd: &RecipeCommand) -> Result<EmittedTask> {
    match cmd {
        RecipeCommand::Dotfiles {
            url,
            src,
            target,
            ignore,
            name,
        } => {
            let ignore_refs: Vec<&str> = ignore.iter().map(String::as_str).collect();
            emit_dotfiles(&DotfilesParams {
                name,
                url,
                src,
                target,
                ignore: ignore_refs,
            })
        }
        RecipeCommand::GitRepo { url, target, name } => {
            emit_git_repo(&GitRepoParams { name, url, target })
        }
        RecipeCommand::BrewBundle { file, name } => {
            emit_brew_bundle(&BrewBundleParams { name, file })
        }
        RecipeCommand::OnePasswordSsh { name } => {
            emit_onepassword_ssh(&OnePasswordSshParams { name })
        }
    }
}

/// Emit `clone` (into `.`) + `symlink` (force; ignore includes `.cursor`).
pub fn emit_dotfiles(p: &DotfilesParams<'_>) -> Result<EmittedTask> {
    crate::config::document::validate_task_name(p.name)?;
    let mut ignore: Vec<String> = p.ignore.iter().map(|s| (*s).to_string()).collect();
    if !ignore.iter().any(|i| i == DEFAULT_DOTFILES_IGNORE) {
        ignore.insert(0, DEFAULT_DOTFILES_IGNORE.to_string());
    }
    let task = TaskConfig {
        commands: vec![
            CommandEntry::Clone(CloneArgs {
                url: p.url.to_string(),
                target: ".".to_string(),
                os: OsFilter::All,
            }),
            CommandEntry::Symlink(SymlinkArgs {
                src: p.src.to_string(),
                target: p.target.to_string(),
                ignore,
                force: true,
                sudo: false,
                os: OsFilter::All,
                backup: false,
            }),
        ],
        ..blank_task_config()
    };
    Ok(EmittedTask {
        name: p.name.to_string(),
        task,
    })
}

/// Emit a single `clone` Command entry.
pub fn emit_git_repo(p: &GitRepoParams<'_>) -> Result<EmittedTask> {
    crate::config::document::validate_task_name(p.name)?;
    let task = TaskConfig {
        commands: vec![CommandEntry::Clone(CloneArgs {
            url: p.url.to_string(),
            target: p.target.to_string(),
            os: OsFilter::All,
        })],
        ..blank_task_config()
    };
    Ok(EmittedTask {
        name: p.name.to_string(),
        task,
    })
}

/// Emit `run` with brew bundle on install + update; `os: [macos]`.
pub fn emit_brew_bundle(p: &BrewBundleParams<'_>) -> Result<EmittedTask> {
    crate::config::document::validate_task_name(p.name)?;
    let cmd = format!("brew bundle --file={}", shell_single_quote(p.file));
    let task = TaskConfig {
        os: OsFilter::Multiple(vec![Os::Macos]),
        commands: vec![CommandEntry::Run(RunArgs {
            commands: Default::default(),
            install: cmd.clone().into(),
            update: cmd.into(),
            uninstall: Default::default(),
            shell: None,
            env: Default::default(),
            quiet: false,
            os: OsFilter::All,
        })],
        ..blank_task_config()
    };
    Ok(EmittedTask {
        name: p.name.to_string(),
        task,
    })
}

/// Emit `run` that wires `~/.ssh/config` IdentityAgent to the 1Password SSH agent.
///
/// Does not rewrite the Config `secrets:` block (comment-preserving authoring).
/// After install, add:
/// ```yaml
/// secrets:
///   default_provider: onepassword
///   ssh_agent: true
/// ```
pub fn emit_onepassword_ssh(p: &OnePasswordSshParams<'_>) -> Result<EmittedTask> {
    crate::config::document::validate_task_name(p.name)?;
    // macOS + Linux agent socket paths from 1Password docs; idempotent append.
    let install = r#"set -e
mkdir -p "$HOME/.ssh"
chmod 700 "$HOME/.ssh"
CONFIG="$HOME/.ssh/config"
touch "$CONFIG"
chmod 600 "$CONFIG"
if grep -Eq 'IdentityAgent.*(1password|2BUA8C4H2G)' "$CONFIG" 2>/dev/null; then
  echo "1Password IdentityAgent already present in ~/.ssh/config"
else
  if [ "$(uname -s)" = "Darwin" ]; then
    AGENT="$HOME/Library/Group Containers/2BUA8C4H2G.com.1password/t/agent.sock"
  else
    AGENT="$HOME/.1password/agent.sock"
  fi
  printf '\n# machine_setup onepassword-ssh\nHost *\n\tIdentityAgent "%s"\n' "$AGENT" >> "$CONFIG"
  echo "Added 1Password IdentityAgent to ~/.ssh/config"
fi
echo "Next: unlock 1Password, enable Settings → Developer → Integrate with 1Password CLI"
echo "and SSH agent, then add to your Config document:"
echo "  secrets:"
echo "    default_provider: onepassword"
echo "    ssh_agent: true"
echo "Or run: machine_setup auth enable onepassword"
"#;
    let task = TaskConfig {
        os: OsFilter::Multiple(vec![Os::Macos, Os::Linux]),
        commands: vec![CommandEntry::Run(RunArgs {
            commands: Default::default(),
            install: install.to_string().into(),
            update: Default::default(),
            uninstall: Default::default(),
            shell: None,
            env: Default::default(),
            quiet: false,
            os: OsFilter::All,
        })],
        ..blank_task_config()
    };
    Ok(EmittedTask {
        name: p.name.to_string(),
        task,
    })
}

fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::document::{self, load_after_write};
    use crate::config::types::CommandEntry;
    use tempfile::tempdir;

    #[test]
    fn emit_git_repo_is_typed_clone() {
        let emitted = emit_git_repo(&GitRepoParams {
            name: "my-repo",
            url: "https://github.com/user/repo.git",
            target: "~/projects/repo",
        })
        .unwrap();
        assert_eq!(emitted.name, "my-repo");
        assert_eq!(emitted.task.commands.len(), 1);
        match &emitted.task.commands[0] {
            CommandEntry::Clone(a) => {
                assert_eq!(a.url, "https://github.com/user/repo.git");
                assert_eq!(a.target, "~/projects/repo");
            }
            other => panic!("expected Clone, got {other:?}"),
        }
    }

    #[test]
    fn emit_dotfiles_is_typed_clone_and_symlink() {
        let emitted = emit_dotfiles(&DotfilesParams {
            name: DEFAULT_DOTFILES_NAME,
            url: "git@github.com:user/.dotfiles.git",
            src: DEFAULT_DOTFILES_SRC,
            target: DEFAULT_DOTFILES_TARGET,
            ignore: vec![],
        })
        .unwrap();
        assert_eq!(emitted.name, DEFAULT_DOTFILES_NAME);
        assert_eq!(emitted.task.commands.len(), 2);
        match &emitted.task.commands[0] {
            CommandEntry::Clone(a) => {
                assert_eq!(a.url, "git@github.com:user/.dotfiles.git");
                assert_eq!(a.target, ".");
            }
            other => panic!("expected Clone, got {other:?}"),
        }
        match &emitted.task.commands[1] {
            CommandEntry::Symlink(a) => {
                assert_eq!(a.src, DEFAULT_DOTFILES_SRC);
                assert_eq!(a.target, DEFAULT_DOTFILES_TARGET);
                assert!(a.force);
                assert!(a.ignore.iter().any(|i| i == ".cursor"));
            }
            other => panic!("expected Symlink, got {other:?}"),
        }
    }

    #[test]
    fn emit_onepassword_ssh_is_typed_unix_run() {
        use crate::config::os::{Os, OsFilter};
        let emitted = emit_onepassword_ssh(&OnePasswordSshParams {
            name: DEFAULT_ONEPASSWORD_SSH_NAME,
        })
        .unwrap();
        assert_eq!(emitted.name, DEFAULT_ONEPASSWORD_SSH_NAME);
        match &emitted.task.os {
            OsFilter::Multiple(oses) => {
                assert!(oses.contains(&Os::Macos));
                assert!(oses.contains(&Os::Linux));
            }
            other => panic!("expected macos+linux filter, got {other:?}"),
        }
        match &emitted.task.commands[0] {
            CommandEntry::Run(a) => {
                let joined = a.install.as_slice().join("\n");
                assert!(joined.contains("IdentityAgent"));
                assert!(joined.contains("1password"));
            }
            other => panic!("expected Run, got {other:?}"),
        }
    }

    #[test]
    fn emit_brew_bundle_is_typed_macos_run() {
        use crate::config::os::{Os, OsFilter};
        let emitted = emit_brew_bundle(&BrewBundleParams {
            name: DEFAULT_BREW_BUNDLE_NAME,
            file: "./Brewfile",
        })
        .unwrap();
        assert_eq!(emitted.name, DEFAULT_BREW_BUNDLE_NAME);
        match &emitted.task.os {
            OsFilter::Multiple(oses) => assert_eq!(oses.as_slice(), &[Os::Macos]),
            OsFilter::Single(Os::Macos) => {}
            other => panic!("expected macos filter, got {other:?}"),
        }
        match &emitted.task.commands[0] {
            CommandEntry::Run(a) => {
                assert_eq!(a.install.as_slice(), &["brew bundle --file='./Brewfile'"]);
                assert_eq!(a.update.as_slice(), &["brew bundle --file='./Brewfile'"]);
            }
            other => panic!("expected Run, got {other:?}"),
        }
    }

    #[test]
    fn dotfiles_parses_as_clone_and_symlink() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        document::init(&path).unwrap();
        let emitted = emit_dotfiles(&DotfilesParams {
            name: DEFAULT_DOTFILES_NAME,
            url: "git@github.com:user/.dotfiles.git",
            src: DEFAULT_DOTFILES_SRC,
            target: DEFAULT_DOTFILES_TARGET,
            ignore: vec![],
        })
        .unwrap();
        document::append_emitted(&path, &emitted).unwrap();
        let config = load_after_write(&path).unwrap();
        let task = &config.tasks["dotfiles"];
        assert_eq!(task.commands.len(), 2);
        assert!(matches!(task.commands[0], CommandEntry::Clone(_)));
        assert!(matches!(task.commands[1], CommandEntry::Symlink(_)));
        if let CommandEntry::Symlink(args) = &task.commands[1] {
            assert!(args.force);
            assert!(args.ignore.iter().any(|i| i == ".cursor"));
        }
    }

    #[test]
    fn git_repo_parses_as_clone() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        document::init(&path).unwrap();
        let emitted = emit_git_repo(&GitRepoParams {
            name: "my-repo",
            url: "https://github.com/user/repo.git",
            target: "~/projects/repo",
        })
        .unwrap();
        document::append_emitted(&path, &emitted).unwrap();
        let config = load_after_write(&path).unwrap();
        assert!(matches!(
            config.tasks["my-repo"].commands[0],
            CommandEntry::Clone(_)
        ));
    }

    #[test]
    fn brew_bundle_is_macos_run() {
        use crate::config::os::{Os, OsFilter};
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        document::init(&path).unwrap();
        let emitted = emit_brew_bundle(&BrewBundleParams {
            name: DEFAULT_BREW_BUNDLE_NAME,
            file: "./Brewfile",
        })
        .unwrap();
        document::append_emitted(&path, &emitted).unwrap();
        let config = load_after_write(&path).unwrap();
        let task = &config.tasks["brew-bundle"];
        match &task.os {
            OsFilter::Multiple(oses) => assert_eq!(oses.as_slice(), &[Os::Macos]),
            OsFilter::Single(Os::Macos) => {}
            other => panic!("expected macos filter, got {other:?}"),
        }
        assert!(matches!(task.commands[0], CommandEntry::Run(_)));
    }

    #[test]
    fn recipe_keys_match_catalog_emitters() {
        assert_eq!(RECIPE_KEYS.len(), recipe_menu_labels().len());
        assert!(default_name_for_key(RECIPE_KEYS[0]).is_some());
        assert!(default_name_for_key(RECIPE_KEYS[1]).is_some());
        assert!(default_name_for_key(RECIPE_KEYS[2]).is_some());
        assert!(default_name_for_key("unknown").is_none());
    }

    #[test]
    fn recipe_refuses_duplicate_task_name() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        document::init(&path).unwrap();
        let emitted = emit_git_repo(&GitRepoParams {
            name: "repo",
            url: "https://example.com/r.git",
            target: "~/r",
        })
        .unwrap();
        document::append_emitted(&path, &emitted).unwrap();
        let err = document::append_emitted(&path, &emitted).unwrap_err();
        assert!(matches!(err, crate::error::Error::TaskAlreadyExists(_)));
    }
}
