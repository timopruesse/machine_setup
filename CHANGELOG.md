# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

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
