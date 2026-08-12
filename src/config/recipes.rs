//! Authoring recipes — named emitters of Tasks built from existing Command entry kinds.
//!
//! Not new kinds (ADR-0006). Output is append-only YAML via the Config document module.

use crate::error::Result;

/// A Task ready to append under `tasks:` (already indented with two spaces).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedTask {
    pub name: String,
    /// Full YAML block including the task key line, indented for nesting under `tasks:`.
    pub yaml: String,
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

pub const DEFAULT_DOTFILES_NAME: &str = "dotfiles";
pub const DEFAULT_GIT_REPO_NAME: &str = "git-repo";
pub const DEFAULT_BREW_BUNDLE_NAME: &str = "brew-bundle";
pub const DEFAULT_DOTFILES_SRC: &str = "./home";
pub const DEFAULT_DOTFILES_TARGET: &str = "~";
pub const DEFAULT_DOTFILES_IGNORE: &str = ".cursor";

/// Emit `clone` (into `.`) + `symlink` (force; ignore includes `.cursor`).
pub fn emit_dotfiles(p: &DotfilesParams<'_>) -> Result<EmittedTask> {
    crate::config::document::validate_task_name(p.name)?;
    let mut ignore = p.ignore.clone();
    if !ignore.contains(&DEFAULT_DOTFILES_IGNORE) {
        ignore.insert(0, DEFAULT_DOTFILES_IGNORE);
    }
    let ignore_yaml = format_ignore_list(&ignore);
    let yaml = format!(
        "\n  # Authoring recipe: dotfiles\n  {name}:\n    commands:\n      - clone:\n          url: {url}\n          target: \".\"\n      - symlink:\n          src: {src}\n          target: {target}\n          force: true\n          ignore:\n{ignore}",
        name = p.name,
        url = quote_yaml(p.url),
        src = quote_yaml(p.src),
        target = quote_yaml(p.target),
        ignore = ignore_yaml,
    );
    Ok(EmittedTask {
        name: p.name.to_string(),
        yaml,
    })
}

/// Emit a single `clone` Command entry.
pub fn emit_git_repo(p: &GitRepoParams<'_>) -> Result<EmittedTask> {
    crate::config::document::validate_task_name(p.name)?;
    let yaml = format!(
        "\n  # Authoring recipe: git-repo\n  {name}:\n    commands:\n      - clone:\n          url: {url}\n          target: {target}\n",
        name = p.name,
        url = quote_yaml(p.url),
        target = quote_yaml(p.target),
    );
    Ok(EmittedTask {
        name: p.name.to_string(),
        yaml,
    })
}

/// Emit `run` with brew bundle on install + update; `os: [macos]`.
pub fn emit_brew_bundle(p: &BrewBundleParams<'_>) -> Result<EmittedTask> {
    crate::config::document::validate_task_name(p.name)?;
    let cmd = format!("brew bundle --file={}", shell_single_quote(p.file));
    let yaml = format!(
        "\n  # Authoring recipe: brew-bundle\n  {name}:\n    os: [macos]\n    commands:\n      - run:\n          install: {install}\n          update: {update}\n",
        name = p.name,
        install = quote_yaml(&cmd),
        update = quote_yaml(&cmd),
    );
    Ok(EmittedTask {
        name: p.name.to_string(),
        yaml,
    })
}

fn format_ignore_list(items: &[&str]) -> String {
    items
        .iter()
        .map(|i| format!("            - {}\n", quote_yaml(i)))
        .collect()
}

fn quote_yaml(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
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
