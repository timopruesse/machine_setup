//! Interactive wizard — dialoguer adapter on the Config document module.
//!
//! Requires a TTY. Creates the Config document if needed (after confirm), then
//! loops: blank Task, Authoring recipe, or done.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use dialoguer::{Confirm, Input, Select};

use crate::error::{Error, Result};

use super::document::{self, validate_after_write};
use super::locator;
use super::recipes::{
    default_name_for_key, emit_by_key, recipe_menu_labels, RecipeEmitInput, DEFAULT_DOTFILES_SRC,
    DEFAULT_DOTFILES_TARGET, RECIPE_KEYS,
};
use super::{is_url, resolve_config_path};

fn build_menu() -> Vec<String> {
    let mut items = vec!["Add blank task".to_string()];
    items.extend(
        recipe_menu_labels()
            .iter()
            .map(|label| (*label).to_string()),
    );
    items.push("Done".to_string());
    items
}

/// Run the wizard. Returns the Config document path written/updated.
pub fn run(config_arg: Option<&str>, cwd: &Path) -> Result<PathBuf> {
    ensure_tty()?;

    let path = resolve_wizard_path(config_arg, cwd)?;

    if path.is_file() {
        println!("Using {}", path.display());
    } else {
        let create = Confirm::new()
            .with_prompt(format!("Create Config document at {}?", path.display()))
            .default(true)
            .interact()
            .map_err(|e| Error::Other(e.to_string()))?;
        if !create {
            return Err(Error::Other("Aborted.".into()));
        }
        document::init(&path)?;
        println!("Created {}", path.display());
        let _ = validate_after_write(&path)?;
    }

    loop {
        let menu = build_menu();
        let choice = Select::new()
            .with_prompt("What next?")
            .items(&menu)
            .default(0)
            .interact()
            .map_err(|e| Error::Other(e.to_string()))?;

        match choice {
            0 => {
                let name: String = Input::new()
                    .with_prompt("Task name")
                    .interact_text()
                    .map_err(|e| Error::Other(e.to_string()))?;
                document::add_task(&path, &name)?;
                println!("Added task `{name}`");
            }
            i if (1..=RECIPE_KEYS.len()).contains(&i) => {
                add_recipe(&path, RECIPE_KEYS[i - 1])?;
            }
            i if i == RECIPE_KEYS.len() + 1 => break,
            _ => unreachable!("menu index out of range"),
        }

        if validate_after_write(&path)? {
            // Keep going so the user can fix by editing; errors already printed.
            eprintln!("Validation reported errors; you can finish and edit the file.");
        }
    }

    println!("Wizard finished: {}", path.display());
    if validate_after_write(&path)? {
        return Err(Error::Other("Config document has validation errors".into()));
    }
    Ok(path)
}

fn ensure_tty() -> Result<()> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        Ok(())
    } else {
        Err(Error::Other(
            "wizard requires an interactive terminal; use `init` / `add` instead".into(),
        ))
    }
}

/// Resolve the path the wizard will use (may not exist yet).
fn resolve_wizard_path(config_arg: Option<&str>, cwd: &Path) -> Result<PathBuf> {
    match config_arg {
        Some(raw) if is_url(raw) => Err(Error::Other(
            "wizard requires a local Config document path, not a URL".into(),
        )),
        Some(raw) => {
            let path = Path::new(raw);
            if path.is_file() {
                Ok(path.to_path_buf())
            } else if let Ok(resolved) = resolve_config_path(path) {
                Ok(resolved)
            } else {
                Ok(document::resolve_init_path(Some(raw), cwd))
            }
        }
        None => match locator::find(cwd) {
            Ok(path) => Ok(path),
            Err(Error::ConfigNotLocated) => Ok(locator::init_path(cwd)),
            Err(e) => Err(e),
        },
    }
}

fn add_recipe(path: &Path, key: &str) -> Result<()> {
    let default_name = default_name_for_key(key).unwrap_or(key);
    let name: String = Input::new()
        .with_prompt("Task name")
        .default(default_name.to_string())
        .interact_text()
        .map_err(|e| Error::Other(e.to_string()))?;

    let emitted = match key {
        "dotfiles" => {
            let url: String = Input::new()
                .with_prompt("Git URL")
                .interact_text()
                .map_err(|e| Error::Other(e.to_string()))?;
            let src: String = Input::new()
                .with_prompt("Symlink source")
                .default(DEFAULT_DOTFILES_SRC.to_string())
                .interact_text()
                .map_err(|e| Error::Other(e.to_string()))?;
            let target: String = Input::new()
                .with_prompt("Symlink target")
                .default(DEFAULT_DOTFILES_TARGET.to_string())
                .interact_text()
                .map_err(|e| Error::Other(e.to_string()))?;
            emit_by_key(
                key,
                RecipeEmitInput::Dotfiles(super::recipes::DotfilesParams {
                    name: &name,
                    url: &url,
                    src: &src,
                    target: &target,
                    ignore: vec![],
                }),
            )?
        }
        "git-repo" => {
            let url: String = Input::new()
                .with_prompt("Git URL")
                .interact_text()
                .map_err(|e| Error::Other(e.to_string()))?;
            let target: String = Input::new()
                .with_prompt("Clone target")
                .interact_text()
                .map_err(|e| Error::Other(e.to_string()))?;
            emit_by_key(
                key,
                RecipeEmitInput::GitRepo(super::recipes::GitRepoParams {
                    name: &name,
                    url: &url,
                    target: &target,
                }),
            )?
        }
        "brew-bundle" => {
            let file: String = Input::new()
                .with_prompt("Brewfile path")
                .default("./Brewfile".to_string())
                .interact_text()
                .map_err(|e| Error::Other(e.to_string()))?;
            emit_by_key(
                key,
                RecipeEmitInput::BrewBundle(super::recipes::BrewBundleParams {
                    name: &name,
                    file: &file,
                }),
            )?
        }
        other => return Err(Error::Other(format!("unknown recipe key: {other}"))),
    };

    document::append_emitted(path, &emitted)?;
    println!("Added recipe task `{name}`");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn resolve_wizard_path_defaults_to_cwd_yaml_when_missing() {
        let dir = tempdir().unwrap();
        let path = resolve_wizard_path(None, dir.path()).unwrap();
        assert_eq!(path, dir.path().join("machine_setup.yaml"));
    }

    #[test]
    fn resolve_wizard_path_finds_existing() {
        let dir = tempdir().unwrap();
        let existing = dir.path().join("machine_setup.yaml");
        std::fs::write(&existing, "tasks: {}\n").unwrap();
        let path = resolve_wizard_path(None, dir.path()).unwrap();
        assert_eq!(path, existing);
    }

    #[test]
    fn resolve_wizard_path_rejects_url() {
        let dir = tempdir().unwrap();
        let err = resolve_wizard_path(Some("https://example.com/c.yaml"), dir.path()).unwrap_err();
        assert!(matches!(err, Error::Other(_)));
    }

    #[test]
    fn ensure_tty_fails_without_terminal_in_piped_test() {
        // In CI/tests stdin is often not a TTY — we only assert the helper's logic
        // when we know we're non-interactive; skip if somehow interactive.
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            assert!(ensure_tty().is_err());
        }
    }

    #[test]
    fn build_menu_includes_catalog_recipes() {
        let menu = build_menu();
        assert_eq!(menu.len(), RECIPE_KEYS.len() + 2);
        for (i, key) in RECIPE_KEYS.iter().enumerate() {
            assert!(menu[i + 1].contains(key));
        }
    }
}
