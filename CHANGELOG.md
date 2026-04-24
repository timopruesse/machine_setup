# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [2.4.1]

### Changed
- `list` command output now marks copy/symlink commands that use sudo with a `(sudo)` annotation, matching what the TUI already showed

### Performance
- Pipe shell scripts directly to bash/zsh stdin instead of writing, reading, and executing a temp file (PowerShell still uses `-File` and a temp file)
- Avoid allocating a `HashSet<usize>` per TUI frame in the task list render
- Borrow task names on the no-dependency fast path of `topological_sort` instead of cloning
- Skip cloning `AppConfig` into `TaskRunner`; move ownership instead
- Use `mem::take` in interactive task selection to avoid re-cloning selected names
- Return an iterator from `RunArgs::all_command_strings`; callers no longer force a `Vec<&str>` allocation
- Use a `HashSet` for selected-task lookup in `requires_sudo`

### Refactored
- Extract `config::resolve_config_dir`, `utils::process::stream_and_wait`, and `utils::path::walk_relative` helpers, removing duplicated logic across `main.rs`, `setup.rs`, `run.rs`, `clone.rs`, `copy.rs`, and `symlink.rs`
- Unify `CommandEntry::Display` with each executor's `description()` via per-args `Display` impls

## [2.4.0]

### Added
- `depends_on` field for DAG-based task dependency resolution
- Conditional tasks with `only_if` and `skip_if` fields
- Task retry on failure with `retry` field
- TUI task filtering/search with `/` keybinding
- JSON example configuration file (`example_config.json`)
- CHANGELOG.md with release workflow integration

## [2.3.0]

### Added
- Shell completions (`completions` subcommand)
- Config validation (`validate` subcommand)
- Environment variable injection prevention (env values are escaped)
- Sub-config task indentation in TUI

### Changed
- Updated dependencies

## [2.2.3]

### Fixed
- Handle sub-config tasks in TUI without panicking

## [2.2.0]

### Added
- `sudo` option for `copy` and `symlink` commands

## [2.0.0]

### Changed
- Complete rewrite: async engine with TUI, YAML/JSON config, parallel execution
