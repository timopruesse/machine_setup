# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Project Does

`machine_setup` is a Rust CLI tool for declarative machine configuration — it runs tasks defined in YAML/JSON config files to install, update, or uninstall system setups (dotfiles, symlinks, shell commands, git clones). It targets multi-platform use (macOS, Linux, Windows) and supports both interactive TUI mode and plain CI output.

## Commands

```bash
make check        # cargo check
make lint         # cargo fmt --check + cargo clippy --all-features -- -D warnings
make test         # cargo test
make build        # cargo build --release
make run          # cargo run -- install -c ./example_config.yaml
```

Run a single test by name:
```bash
cargo test <test_name>
```

## Architecture

### Execution Flow

1. `src/main.rs` — async Tokio entry point; parses CLI, loads config, selects TUI vs. plain output mode
2. `src/cli.rs` — clap CLI (subcommands: `install`, `update`, `uninstall`, `list`)
3. `src/config/` — loads YAML/JSON from local path or URL; `types.rs` defines `AppConfig`/`TaskConfig`/`CommandEntry`
4. `src/engine/runner.rs` — `TaskRunner` iterates tasks, checks OS filters and history, spawns command executors
5. `src/engine/commands/` — one executor per command type (`run`, `copy`, `symlink`, `clone`, `setup`); each implements a `CommandExecutor` trait
6. `src/engine/event.rs` — `TaskEvent` enum used for mpsc communication from engine to UI
7. `src/tui/` — ratatui TUI (interactive) or `src/tui/plain.rs` (CI/non-TTY); both consume `TaskEvent`s
8. `src/config/history.rs` — persists installed tasks to `~/.machine_setup/history.json`; `--force` bypasses

### Key Design Points

- **Async**: tokio drives everything; the runner spawns tasks on a thread pool; commands stream stdout/stderr via events
- **Parallel execution**: controlled at config-level (`parallel: true`) globally or per-task; thread count defaults to `num_cpus - 1`
- **Command modes**: `run` commands can have separate `install`/`update`/`uninstall` entries; other command types execute on all modes
- **OS filtering**: tasks/commands filtered by `os: [linux, macos, windows]` field
- **Remote config**: URLs are fetched via `ureq`; GitHub blob URLs are auto-converted to raw URLs
- **Nested configs**: `machine_setup` command type includes another config file recursively

### Config Format (YAML)

```yaml
default_shell: bash       # bash | zsh | powershell
parallel: false
tasks:
  my-task:
    os: [linux, macos]
    parallel: false
    commands:
      - run:
          install: echo "installing"
          update: echo "updating"
          uninstall: echo "removing"
      - symlink:
          source: ~/dotfiles/.zshrc
          target: ~/.zshrc
          force: true
      - copy:
          source: ./config/
          target: ~/.config/mytool/
      - clone:
          url: https://github.com/user/repo
          target: ~/projects/repo
```

## CI

GitHub Actions runs `cargo check`, `cargo test`, and `cargo clippy -D warnings` on every push/PR to `main`. Clippy warnings are treated as errors — the same applies locally via `make lint`.

Release builds are triggered by `v*` tags and produce binaries for macOS x86_64/ARM64, Linux x86_64, and Windows x86_64.
