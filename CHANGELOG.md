# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- Command-bench tree size ladder (1k default / 10k via `MACHINE_SETUP_BENCH_TREE_SIZE`) and a report-only 100k tree memory harness for the ADR-0004 chunking gate

## [2.9.0]

### Added
- Richer Task conditions on `only_if` / `skip_if`: path (backward compatible), `env`, `command`, and `mode` predicates
- Configurable `retry_delay_secs` on Tasks (default `1`)
- `machine_setup` Sub-config admission knobs: `force` and `with_deps`
- Documented Tree materialization ignore language: exact path components, `foo/bar` sequences, and glob-lite `*`/`?` (no substring-anywhere matches)

### Changed
- Command kind catalog owns unattended sudo demotion; schedule only decides when to demote
- Closed `Executor` enum from the catalog (static dispatch) instead of `Box<dyn CommandExecutor>`
- Authoring recipes register through a shared recipe catalog used by CLI and wizard
- Tree-shaped Command executors implement Tree-op policy directly (no shallow kind shells)
- Task event `task_name` is interned as `Arc<str>` to cut per-line allocations

## [2.8.1]

### Performance
- Config locator walks `.git` before spawning `git rev-parse`, so the common repo/worktree path skips a process
- History writes compact `history.json`; pretty-printed files still load

## [2.8.0]

### Added
- Runner grid details pane during parallel runs: up to four task bands with overflow count, `j`/`k` band selection, and `Enter` to expand/collapse a full log
- Per-command progress on task events (`command_index` / `command_total`)
- Structured `OutputKind` on engine log lines so the TUI and plain mode can style and filter output
- `quiet: true` on `run` commands to suppress subprocess stdout and emit a completion summary instead

### Changed
- Replace the flat parallel merge stream with per-runner bands in the details pane
- Shorter copy/symlink/clone progress messages with `~/…` path shortening
- Quieter `git clone` / `git pull`; subprocess streaming collapses blank lines and truncates long lines

## [2.7.2]

### Fixed
- Strip ANSI escape sequences from TUI log/merge lines so colored command output no longer shows leftover CSI text (e.g. `[33m`)

## [2.7.1]

### Fixed
- Non-interactive `schedule run` demotes/clears `sudo` so launchd/systemd timers do not hang waiting for a password prompt; validate warns when scheduled tasks still declare sudo

### Performance
- Tree walks prune ignored directories instead of visiting their contents
- Shared mtime-skip for copy (fewer syscalls on already-synced trees); drop redundant DirectFs parent mkdir and per-file symlink `ensure_real_dir`

## [2.7.0]

### Added
- Post-command self update-check notice when a newer GitHub release exists (cached ~24h), with install-method-specific update command; disable via `check_for_updates: false` or `MACHINE_SETUP_NO_UPDATE_CHECK=1`
- Per-task `auto_update` (`at: "HH:MM"` or daily `cron`) with `machine_setup schedule apply|remove|run|status|notify` — launchd (macOS) / systemd user timers (Linux), bundled by schedule key, shell hook notices

### Performance
- Stale self update-checks refresh in a detached background process so CLI exit is not blocked on the network
- `schedule notify` skips Config locate/load (hook embeds `--temp-dir`) for a faster shell-hook path

## [2.6.1]

### Added
- `doctor` uses the shared catalog TUI (summary banner + per-task validation detail) when a TTY is available

### Changed
- `doctor --no-tui` / non-TTY output uses the same colored columnar task view as `list`, plus Validation and History orphans sections

## [2.6.0]

### Added
- Shared catalog TUI for browsing tasks (`list`) with master–detail view and `/` filter
- `-s` / `--select` uses the catalog multi-select TUI when available (status + detail)

### Changed
- `list` uses the TUI by default on a TTY; `--no-tui` / non-TTY falls back to a colored columnar plain view (`NO_COLOR` respected)

## [2.5.0]

### Added
- `machine_setup init` and `machine_setup add task <name>` for Config document scaffolding (append-only; validate after write)
- Authoring recipes: `add recipe dotfiles|git-repo|brew-bundle` (emit existing Command entry kinds only)
- `machine_setup wizard` — interactive Config document setup (TTY; reuses init/add/recipes)
- `machine_setup schema` plus checked-in `schema/machine_setup.schema.json` (CI stale check; yaml-language-server modeline on `init`)
- Config locator: when `-c` is omitted, search cwd then git repository root for `machine_setup.{yaml,yml,json}`
- `list` shows install status and History timestamps alongside each Task
- `doctor` reports Task status, validation issues, and orphan History entries (`--fix` prunes orphans)

### Changed
- `-c` / `--config` no longer defaults to `./machine_setup`; omit it to use the Config locator

## [2.4.7]

### Added
- `--with-deps` to force transitive `depends_on` expansion on update/uninstall (install already expands)

### Changed
- `-t` / `-s` no longer pull in dependency chains on update/uninstall by default; install still expands
- Interactive uninstall offers a multi-select of remaining dependencies (skipped with `--no-tui` / non-TTY)
- Uninstall runs dependents before dependencies (reversed dependency layers)
- Uninstall warns when removing a task other config tasks still depend on (confirm on TTY)

## [2.4.6]

### Added
- Show run total and per-task elapsed/final durations in the interactive TUI
- While two or more tasks are running, multiplex their output into a tagged parallel log stream with stable accent colors and a running-count cue

### Changed
- Rewrite interactive TUI around a pure `UiState` / `Message` reducer with an async event loop (engine events, keys, spinner ticks)
- Soft-follow task selection during parallel runs (avoid thrashing while the selected task is still running)

### Fixed
- Log view no longer snaps to the bottom while manually scrolled (follow mode; `End` re-enables)
- Esc clears an active task filter instead of always quitting (`q` / Ctrl+C still cancel and quit)
- Help bar documents `j`/`k`, `Home`/`End`, filter clear, and follow hints
- Jump selection to the first failed task when a run finishes with failures (and prefer burst failures when leaving parallel merge)
- Cap per-task and parallel-merge log buffers to avoid unbounded memory growth
- Keep keyboard input responsive after the engine channel closes when tasks finish

## [2.4.5]

### Performance
- Defer Tokio multi-thread runtime until install/update/uninstall so sync verbs (`list`, `validate`, completions, `--help`) skip runtime bring-up
- Lazily create the Concurrency gate Rayon FS pool on first tree apply instead of at `TaskRunner` construction
- Slim the release binary with thin LTO, single codegen unit, strip, and a trimmed tokio feature set

### Changed
- Add fixed-cost Command benches for `TaskRunner::new` and empty-task Runner smoke (report-only)

## [2.4.4]

### Performance
- Parallelize DirectFs directory copy/uninstall file apply on a shared Rayon pool sized by `num_threads`, so large tree installs scale without oversubscribing sibling tasks
- Coalesce per-file copy/symlink progress logs for large trees (first few, then periodically, plus a summary)
- Skip redundant parent `create_dir_all` when the destination directory already exists
- Avoid `canonicalize` syscalls when creating new symlinks whose destination does not yet exist

### Changed
- Cap concurrent leaf Command executor work with a shared concurrency gate (`num_threads`, default CPUs − 1)
- Add Criterion Command bench for tree materialization and Runner smoke (report-only; SudoFs opt-in via `MACHINE_SETUP_BENCH_SUDO=1`)

### Refactored
- Deepen engine architecture around File ops adapters, shared tree materialization, Task event sinks, and CONTEXT.md vocabulary

## [2.4.3]

### Fixed
- Directory `symlink` walks now unwrap leftover destination directory symlinks into real directories (removing only the link inode) before creating file symlinks, so nested leftovers like `~/skills/pack → <src>/skills/pack` can no longer turn source files into self-symlinks

## [2.4.2]

### Fixed
- Homebrew release workflow now bumps the macOS `aarch64`/`x86_64` prebuilt-binary URLs and checksums alongside the source tarball, so `brew upgrade` installs the new version instead of the stale binaries left behind by the previous tap-update action

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
