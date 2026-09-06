# Machine Setup

[![Tests](https://github.com/timopruesse/machine_setup/actions/workflows/test.yml/badge.svg?branch=main)](https://github.com/timopruesse/machine_setup/actions/workflows/test.yml)
[![Builds](https://github.com/timopruesse/machine_setup/actions/workflows/build.yml/badge.svg?branch=main)](https://github.com/timopruesse/machine_setup/actions/workflows/build.yml)
[![Crates.io](https://img.shields.io/crates/v/machine_setup)](https://crates.io/crates/machine_setup)
[![License](https://img.shields.io/crates/l/machine_setup)](https://crates.io/crates/machine_setup)
[![Changelog](https://img.shields.io/badge/changelog-Keep%20a%20Changelog-blue)](CHANGELOG.md)

Declarative machine setup — replicate a workstation after a wipe, keep dotfiles and symlinks in sync, or hand a colleague a config that installs deps and clones the right repos.

Real-world example: [.dotfiles `machine_setup.yaml`](https://github.com/timopruesse/.dotfiles/blob/main/machine_setup.yaml).

## What's New in v2

- **TUI Dashboard**: Real-time progress with task list, per-task logs, elapsed times, and a runner grid (up to four bands) during parallel runs. Powered by [ratatui](https://ratatui.rs/). Use `--no-tui` (or a non-TTY) for CI.
- **Async Engine**: Task execution powered by [tokio](https://tokio.rs/) for concurrent I/O (file ops, git, shell commands with streaming output).
- **Task History**: Tracks install/update/uninstall timestamps in `~/.machine_setup/history.json`. Already-installed tasks are skipped unless `--force` is used.
- **PowerShell Support**: Use `powershell` as a shell option alongside `bash` and `zsh`.
- **Remote Configs**: Point directly at a URL instead of a local file — great for bootstrapping a clean machine without cloning first.

See [CHANGELOG.md](CHANGELOG.md) for release-by-release notes.

## Install

### Quick Install (no dependencies needed)

**macOS / Linux:**

```sh
curl -fsSL https://raw.githubusercontent.com/timopruesse/machine_setup/main/install/install.sh | sh
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/timopruesse/machine_setup/main/install/install.ps1 | iex
```

### Other methods

**Via Homebrew (macOS / Linux):**

```bash
brew install timopruesse/repo/machine_setup
```

**Via Cargo:**

```bash
cargo install machine_setup
```

**Manual download:** Grab a binary from the [release page](https://github.com/timopruesse/machine_setup/releases).

## Run

### Subcommands

| command      | description                              | example                              |
| ------------ | ---------------------------------------- | ------------------------------------ |
| install      | install the defined tasks                | `machine_setup install`              |
| update       | update the defined tasks                 | `machine_setup update`               |
| uninstall    | uninstall the defined tasks              | `machine_setup uninstall`            |
| list         | list tasks with install status           | `machine_setup list`                 |
| validate     | validate the config without executing    | `machine_setup validate`             |
| doctor       | status + validate + History orphans      | `machine_setup doctor` / `doctor --fix` |
| init         | create a new empty Config document       | `machine_setup init`                 |
| wizard       | interactive Config document setup (TTY)  | `machine_setup wizard`               |
| add task     | append a Task stub to the Config document| `machine_setup add task dotfiles`    |
| remove task  | delete a Task (rewrites file; may drop comments) | `machine_setup remove task tools` |
| replace task | upsert a Task stub (rewrites file; may drop comments) | `machine_setup replace task dotfiles` |
| add recipe   | append a Task from an Authoring recipe   | `machine_setup add recipe git-repo --url … --target ~` |
| replace recipe | upsert a Task from an Authoring recipe (rewrites file; may drop comments) | `machine_setup replace recipe git-repo --url … --target ~` |
| schema       | print the Config JSON Schema to stdout   | `machine_setup schema`               |
| schedule     | apply/remove OS timers for auto_update   | `machine_setup schedule apply`       |
| completions  | generate shell completions               | `machine_setup completions zsh`      |

By default (no `-c`), `machine_setup` looks for `machine_setup.yaml` / `.yml` / `.json` in the current directory, then at the git repository root. Explicit `-c` still accepts a path or URL. Supported formats are YAML and JSON.

`init` always creates `./machine_setup.yaml` in the cwd when `-c` is omitted (it does not write into the git root). `add task` requires an existing file (`init` first) and refuses duplicate Task names. `remove task` rewrites the file via serde (comments may be lost); when other tasks depend on the target, the CLI prompts on a TTY or requires `--fix-deps` to strip those edges, and History for the removed Task is pruned. `replace task` / `replace recipe` upsert a Task: if the name is missing, the CLI creates it with a warning; if it already exists, the CLI prompts on a TTY before overwriting (non-TTY overwrites with a notice). Replace also rewrites the file via serde (comments may be lost) but leaves History unchanged. After `init` / `add` / `remove` / `replace`, the Config document is validated automatically.

### Command line parameters

| flag              | value                                             | example                                            |
| ----------------- | ------------------------------------------------- | -------------------------------------------------- |
| -c<br> --config   | path or URL to the config file (omit to search cwd, then git root) | `machine_setup install -c ./config/my_setup.yaml`  |
| -t<br> --task     | only run the specified task                       | `machine_setup install -t my_task2`                |
| -s<br> --select   | select a task to run                              | `machine_setup install -s`                         |
| --with-deps       | also run transitive `depends_on` tasks            | `machine_setup update -t leaf --with-deps`         |
| -f<br> --force    | force execution (bypass history checks)           | `machine_setup install --force`                    |
| --dry-run         | preview execution without modifying filesystem or history | `machine_setup install --dry-run`          |
| --backup          | backup existing files before overwriting with symlinks | `machine_setup install --backup`           |
| --no-tui          | disable TUI; also auto-disabled on non-TTY / CI   | `machine_setup install --no-tui`                   |
| -h<br> --help     | display help information                          | `machine_setup --help`                             |
| -v<br> --version  | display version information                       | `machine_setup --version`                          |
| -d<br> --debug    | print additional debug information                | `machine_setup install --debug`                    |
| -l<br> --level    | set a log level (info, warn, error, debug, trace) | `machine_setup install --level=info`               |

### Remote config files

You can pass a URL instead of a local path — the config is fetched and executed directly. GitHub blob URLs are automatically converted to raw URLs.

```bash
machine_setup install -c https://github.com/timopruesse/.dotfiles/blob/main/machine_setup.yaml
```

This is especially useful for setting up a fresh machine without cloning your dotfiles first:

```bash
# Install machine_setup
curl -fsSL https://raw.githubusercontent.com/timopruesse/machine_setup/main/install/install.sh | sh

# Run your dotfiles setup directly from GitHub
machine_setup install -c https://github.com/timopruesse/.dotfiles/blob/main/machine_setup.yaml
```

### TUI Dashboard

When running in an interactive terminal, a TUI dashboard is shown with:
- Task list with status indicators (pending, running, completed, failed, skipped)
- Details pane: per-task scrollable log output; during parallel runs, a **runner grid** shows up to four running tasks as fixed bands (`Enter` expands/collapses the selected band to full log)
- Progress bar with completion stats and elapsed times
- SilkCircuit Neon theme (electric purple + neon cyan); set `NO_COLOR` for ANSI-only output
- Keyboard navigation:
  - `j`/`k` or `Up`/`Down` — navigate tasks (or bands in the runner grid); inside search mode (`/`), `j` and `k` type normally, while `Up`/`Down` and `Ctrl+p`/`Ctrl+n` navigate
  - `/` — filter tasks by name (`Enter` to apply, `Esc` to cancel search or clear filter)
  - `PgUp`/`PgDn` — scroll logs; `Home`/`End` — jump to top/bottom of log (`End` also re-enables follow when auto-follow is off)
  - While running: `q` or `Ctrl+C` — cancel; when done: `Esc` or `q` — quit

The TUI is automatically disabled in non-interactive environments (piped output, CI). You can also explicitly disable it with `--no-tui`.

## Configure

Tasks can be defined under the `tasks` root key.
Every task can contain an arbitrary number of commands.

Scaffold a new file and grow it with Task stubs:

```bash
machine_setup init
machine_setup add task tools
machine_setup add recipe dotfiles --url git@github.com:user/.dotfiles.git
machine_setup add recipe brew-bundle --file ./Brewfile
machine_setup add recipe git-repo --url https://github.com/user/repo.git --target ~/projects/repo
# or interactively:
machine_setup wizard
# edit as needed, then:
machine_setup validate
```

Authoring recipes emit existing Command entry kinds only (`clone`, `symlink`, `run`) — not new kinds. Defaults: Task names `dotfiles` / `brew-bundle` / `git-repo` (override with `--name`); `dotfiles` clones into `.`, symlinks `./home` → `~` with `force` and ignores `.cursor`; `brew-bundle` is `os: [macos]` with install+update.

Editors: `init` writes a `# yaml-language-server: $schema=…` modeline pointing at the checked-in [schema/machine_setup.schema.json](schema/machine_setup.schema.json). Regenerate with `make schema` (CI fails if the artifact is stale). Semantic checks (`depends_on`, missing sources, …) stay in `machine_setup validate` — the schema is structural only.

| key           | description                                          | values                       | default                      |
| ------------- | ---------------------------------------------------- | ---------------------------- | ---------------------------- |
| tasks         | root key for all of the tasks                        |                              |                              |
| default_shell | shell that is used when not specified by the command  | `bash`, `zsh`, `powershell`  | `bash`                       |
| temp_dir      | define where temporary files are stored              |                              | `~/.machine_setup`           |
| parallel      | run all of the tasks in parallel                     | `true` or `false`            | `false`                      |
| num_threads   | number of threads when run in parallel               | numeric > 1                  | physical processor count - 1 |

### Task specific configuration

| key        | description                                                | values                                                                       | examples                      |
| ---------- | ---------------------------------------------------------- | ---------------------------------------------------------------------------- | ----------------------------- |
| os         | only run on the specified os                               | [possible values](https://doc.rust-lang.org/std/env/consts/constant.OS.html) | "linux" or ["linux", "macos"] |
| parallel   | run all of the commands in parallel (1 thread per command) | `true` or `false`                                                            | `false`                       |
| depends_on | run these tasks first (install always expands the chain)   | list of task names                                                           | `["base"]`                    |
| only_if    | only run if all listed paths exist                         | string or list                                                               | `"~/.ssh"`                    |
| skip_if    | skip if any listed path exists                             | string or list                                                               | `"/opt/skip"`                 |
| retry      | retry count on failure (0 = no retry)                      | integer ≥ 0                                                                  | `2`                           |

On `update` / `uninstall`, `-t` / `-s` run only the selected tasks unless you pass `--with-deps`. Interactive uninstall can offer remaining dependencies; uninstall also warns if other tasks still depend on something in the run set.

Minimal example (see also [`example_config.yaml`](example_config.yaml) for a fuller demo used by `make run`):

```yaml
default_shell: "zsh"
parallel: true
tasks:
  tools:
    os: ["linux", "macos"]
    commands:
      - run:
          install: "echo 'install tools'"
          update: "echo 'update tools'"
          uninstall: "echo 'remove tools'"
      - symlink:
          src: "./dotfiles/.zshrc"
          target: "~/.zshrc"
          force: true

  repos:
    commands:
      - clone:
          url: "git@github.com:timopruesse/machine_setup.git"
          target: "~/machine_setup"
```

### Auto-update schedules

Opt a Task into a daily OS timer with `auto_update`. Tasks that share the same
daily time are bundled into one launchd agent (macOS) or systemd user timer
(Linux). Timers are idle until due — no always-on daemon.

```yaml
tasks:
  bun:
    auto_update:
      at: "07:30" # or cron: "30 7 * * *" (daily only in v1)
    commands:
      - run:
          install: curl -fsSL https://bun.sh/install | bash
          update: bun upgrade --canary
```

```bash
machine_setup install -t bun          # install the task once
machine_setup schedule apply          # install/refresh OS units + shell hook
machine_setup schedule status
machine_setup schedule remove         # tear down units (and hook stubs)
```

`schedule apply` writes a hook script under `temp_dir` and, when `~/.zshrc` /
`~/.bashrc` exist, inserts a marked `source` stub so new shells can show a short
notice after a background update. Use `--no-install-hook` to skip rc edits.
Only **installed** tasks are updated when a timer fires. Re-run `schedule apply`
after changing schedule keys or moving the config/binary.

Timers have no TTY, so `schedule run` demotes privilege: copy/symlink `sudo: true`
is cleared and leading `sudo` prefixes are stripped from `run` strings, with a
warning in `schedule.log`. Prefer non-sudo update commands for scheduled tasks;
`validate` / `doctor` emit a warning when `auto_update` meets sudo.

### Self update-check

After most commands, `machine_setup` may print a short stderr notice when a newer
release is on GitHub (checked at most about once per day; cached under
`temp_dir`). Stale checks refresh in a detached background process so the CLI
does not wait on the network. The notice includes an update command guessed from
how the binary was installed (Homebrew, cargo, or the curl/PowerShell installer).

Disable with either:

```yaml
check_for_updates: false
```

or:

```bash
export MACHINE_SETUP_NO_UPDATE_CHECK=1
```

Skipped for `completions`, `schema`, and `schedule notify`.

### Extend a configuration

Extensibility is not explicitly built in.
However, it's possible to execute tasks from another configuration via the [machine_setup](#machine_setup) command.

### Available config commands

All command entries support an optional `os` filter (`"linux"`, `"macos"`, `"windows"`, or a list `["linux", "macos"]`), allowing you to collapse cross-platform tasks into a single unified task:

```yaml
tasks:
  editor:
    commands:
      - run:
          os: "linux"
          commands: "sudo apt-get install -y neovim"
      - run:
          os: "macos"
          commands: "brew install neovim"
      - symlink:
          src: "./nvim"
          target: "~/.config/nvim"
          force: true
          backup: true
```

#### copy

This command copies the contents of a directory to another directory.

| argument | value                               | required | example                                |
| -------- | ----------------------------------- | :------: | -------------------------------------- |
| src      | source directory/file               |    Y     | "./src/files" or "./src/test.txt"      |
| target   | target directory/file               |    Y     | "/tmp/target" or "/tmp/target/new.txt" |
| ignore   | list of files/directories to ignore |    -     | ["dist", "package-lock.json"]          |
| sudo     | run file operations with sudo       |    -     | true                                   |
| os       | only run on specified os            |    -     | "linux" or ["linux", "macos"]          |

##### example

```yaml
copy:
  src: "./src/files"
  target: "/tmp/target"
  ignore: ["dist", "package-lock.json"]

# Copy to a protected path
copy:
  src: "./etc/wsl.conf"
  target: "/etc/wsl.conf"
  sudo: true
  os: "linux"
```

#### clone

This command clones a git repository to the specified destination.

| argument | value                   | required | example                                        |
| -------- | ----------------------- | :------: | ---------------------------------------------- |
| url      | URL to a git repository |    Y     | "git@github.com:timopruesse/machine_setup.git" |
| target   | target directory        |    Y     | "~/machine_setup"                              |
| os       | only run on specified os|    -     | "linux" or ["linux", "macos"]                  |

##### example

```yaml
clone:
  url: "git@github.com:timopruesse/machine_setup.git"
  target: "~/machine_setup"
```

#### symlink

This command symlinks all the files from the source directory to the target directory.

| argument | value                               | required | example                           |
| -------- | ----------------------------------- | :------: | --------------------------------- |
| src      | source directory/file               |    Y     | "./src/files" or "./src/test.txt" |
| target   | target directory/file               |    Y     | "/tmp/target" or "/tmp/new.txt"   |
| ignore   | list of files/directories to ignore |    -     | ["dist", "package-lock.json"]     |
| force    | true/false                          |    -     | true                              |
| backup   | backup existing target on overwrite |    -     | true                              |
| sudo     | run file operations with sudo       |    -     | true                              |
| os       | only run on specified os            |    -     | "linux" or ["linux", "macos"]     |

> If `force` is set to `true`, existing files will be **removed** and replaced by the symlinks.
>
> When `backup: true` (or the `--backup` CLI flag) is used, any existing file or directory at the destination is safely renamed to `<target>.bak` (with a timestamp suffix `<target>.bak.<timestamp>` if `.bak` already exists) before creating the symlink.
>
> When `src` is a directory, intermediate destinations are always **real directories**.
> Leftover directory symlinks under `target` are unwrapped (the link inode is
> removed and replaced with an empty real directory; the tree the link pointed
> at is left untouched). This prevents file symlinks from being written through
> into the source tree.

##### example

```yaml
symlink:
  src: "./src/files"
  target: "/tmp/target"
  ignore: ["dist", "package-lock.json"]
  force: true
  backup: true

# Symlink to a protected path
symlink:
  src: "./etc/my.conf"
  target: "/etc/my.conf"
  sudo: true
  force: true
```

#### run

This command executes a shell command.

> Hint: Avoid the usage of interactive commands when possible.

| argument | description           | required | default | values                       |
| -------- | --------------------- | :------: | ------- | ---------------------------- |
| env      | environment variables |    -     |         |                              |
| shell    | shell that is used    |    -     | "bash"  | "bash", "zsh", "powershell"  |
| os       | only run on specified os | -     |         | "linux", "macos", "windows"  |

By default, shell commands will only run during `install`.
You can provide mode-specific commands using `install`, `update`, and `uninstall` instead of `commands`:

| argument  | description                 | required | example                         |
| --------- | --------------------------- | :------: | ------------------------------- |
| commands  | commands for install only   |    -     | "sudo apt-get -y install git"   |
| install   | commands for installing     |    -     | "sudo apt-get -y install git"   |
| update    | commands for updating       |    -     | "sudo apt-get -y upgrade git"   |
| uninstall | commands for uninstalling   |    -     | "sudo apt-get -y uninstall git" |

> Use either `commands` (runs on install only) or `install`/`update`/`uninstall` for mode-specific behavior. They are all top-level keys under `run`.

##### example

```yaml
inline_command:
  run:
    commands: "sudo apt-get -y install git"

multiline_command:
  run:
    commands:
      - "sudo apt-get update"
      - "sudo apt-get -y install git"

updatable_command:
  run:
    env:
      SOME_TOKEN: "abc123"
    install: "sudo apt-get -y install git"
    update: "sudo apt-get -y upgrade git"
    uninstall: "sudo apt-get -y uninstall git"

updatable_multiline_command:
  run:
    env:
      SOME_TOKEN: "abc123"
    install:
      - "sudo apt update"
      - "sudo apt-get -y install git"
    update:
      - "sudo apt-get -y upgrade git"
    uninstall:
      - "sudo apt-get -y uninstall git"
```

#### machine_setup

With this command it's possible to include other `machine_setup` configuration files.

| argument | description                             | required | example                  |
| -------- | --------------------------------------- | :------: | ------------------------ |
| config   | path to the other config file           |    Y     | "./my_other_config.yaml" |
| task     | define a single task that should be run |    -     | "my_other_task"          |
| os       | only run on specified os                |    -     | "linux" or ["linux", "macos"] |

##### example

```yaml
machine_setup:
  config: "./my_other_config.yaml"
  task: "my_other_task" # optional
```
