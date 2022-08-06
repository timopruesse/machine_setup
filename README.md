# Machine Setup

[![Tests](https://github.com/Chroma91/machine-setup/actions/workflows/test.yml/badge.svg?branch=main)](https://github.com/Chroma91/machine-setup/actions/workflows/test.yml)
[![Builds](https://github.com/Chroma91/machine-setup/actions/workflows/build.yml/badge.svg?branch=main)](https://github.com/Chroma91/machine-setup/actions/workflows/build.yml) [![Crates.io](https://img.shields.io/crates/v/machine_setup)](https://crates.io/crates/machine_setup)

The idea is to be able to replicate a certain setup (when reseting your machine or using a completely new machine).
Additionally, it should be possible to update the setup easily when needed, e.g. an update to your vim config files.
So, it will help with managing dotfiles, symlinks, etc.

A real world example can be found in my [.dotfiles repository](https://github.com/Chroma91/.dotfiles/blob/main/machine_setup.yaml).

You can also use it for other tasks such as making the onboarding process of a new colleague easier by providing them a config that installs certain dependencies and checks out important repositories.

## Install

Install via `cargo` or download a binary from the [release page](https://github.com/Chroma91/machine_setup/releases).

```bash
cargo install machine_setup
```

### Install Shell Completions

#### zsh

> This might differ for your setup.

1. Download the shell completions and put them in the appropriate folder that is in your `fpath`

```bash
sudo wget -P /usr/local/share/zsh/site-functions/ https://raw.githubusercontent.com/Chroma91/machine_setup/main/completions/_machine_setup
```

2. Reload zsh

```bash
exec zsh
```

#### other

The shell completions for other shells such as fish can be found in [completions](https://github.com/Chroma91/machine_setup/tree/main/completions).  
You're welcome to submit a PR with installation instructions for your favorite shell. 😇

## Run

### Subcommands

| command   | description                   | example                   |
| --------- | ----------------------------- | ------------------------- |
| install   | install the defined tasks     | `machine_setup install`   |
| update    | update the defined tasks      | `machine_setup update`    |
| uninstall | uninstall the defined tasks   | `machine_setup uninstall` |
| list      | list all of the defined tasks | `machine_setup list`      |

By default, `machine_setup` will look for a file called `machine_setup` with a supported file format.  
Supported file formats are: `yaml`, `yml`, and `json`.

### Command line parameters

| flag             | value                                             | example                                           |
| ---------------- | ------------------------------------------------- | ------------------------------------------------- |
| -c<br> --config  | specify a different path to the config file       | `machine_setup install -c ./config/my_setup.yaml` |
| -t<br> --task    | only run the specified task                       | `machine_setup install -t my_task2`               |
| -s<br> --select  | select a task to run                              | `machine_setup install -s`                        |
| -h<br> --help    | display help information                          | `machine_setup --help`                            |
| -v<br> --version | display version information                       | `machine_setup --version`                         |
| -d<br> --debug   | print additional debug information                | `machine_setup install --debug`                   |
| -l<br> --level   | set a log level (info, warn, error, debug, trace) | `machine_setup install --level=info`              |

### Supported config file formats

The supported formats are `YAML` and `JSON`.

## Configure

Tasks can be defined under the `tasks` root key.
Every task can contain an arbitrary number of commands.

| key           | description                                          | values            | default                      |
| ------------- | ---------------------------------------------------- | ----------------- | ---------------------------- |
| tasks         | root key for all of the tasks                        |                   |
| default_shell | shell that is used when not specified by the command | `bash`, `zsh`     | `bash`                       |
| temp_dir      | define where temporary files are stored              |                   | `~/.machine_setup`           |
| parallel      | run all of the tasks in parallel                     | `true` or `false` | `false`                      |
| num_threads   | number of threads when run in parallel               | numeric > 1       | physical processor count - 1 |

### Task specific configuration

| key      | description                                                | values                                                                       | examples                      |
| -------- | ---------------------------------------------------------- | ---------------------------------------------------------------------------- | ----------------------------- |
| os       | only run on the specified os                               | [possible values](https://doc.rust-lang.org/std/env/consts/constant.OS.html) | "linux" or ["linux", "macos"] |
| parallel | run all of the commands in parallel (1 thread per command) | `true` or `false`                                                            | `false`                       |

> TODO: Add JSON examples...

Check out the example configuration below:

```yaml
temp_dir: "~/my_temp" # defaults to "~/.machine_setup"
default_shell: "zsh" # defaults to "bash"
parallel: true
num_threads: 2
tasks:
  my_task1:
    os: ["linux", "windows"]
    parallel: true
    commands:
      - copy:
          src: "./src/files"
          target: "/tmp/target"
      - copy:
          src: "./src/files_2"
          target: "/tmp/target"

  my_task2:
    os: ["linux"]
    parallel: true
    commands:
      - shell: "sudo apt-get install git -y"
      - symlink:
          src: "./src/config"
          target: "~/.dotfiles"
```

### Extend a configuration

Extensibility is not explicitly built in.
However, it's possible to execute tasks from another configuration via the `run` command.
You could also only execute a single task by providing the `task` argument.

```yaml
tasks:
  my_other_config:
    commands:
      - run:
          commands:
          install: "machine_setup install -c ./my_other_config.yaml"
          update: "machine_setup update -c ./my_other_config.yaml"
          uninstall: "machine_setup uninstall -c ./my_other_config.yaml"
```

### Available config commands

#### copy

This command copies the contents of a directory to another directory.

| argument | value                               | required | example                                |
| -------- | ----------------------------------- | :------: | -------------------------------------- |
| src      | source directory/file               |    ✅    | "./src/files" or "./src/test.txt"      |
| target   | target directory/file               |    ✅    | "/tmp/target" or "/tmp/target/new.txt" |
| ignore   | list of files/directories to ignore |    ➖    | ["dist", "package-lock.json"]          |

##### example

```yaml
copy:
  src: "./src/files"
  target: "/tmp/target"
  ignore: ["dist", "package-lock.json"]
```

#### clone

This command clones a git repository to the specified destination.

| argument | value                   | required | example                                     |
| -------- | ----------------------- | :------: | ------------------------------------------- |
| url      | URL to a git repository |    ✅    | "git@github.com:Chroma91/machine_setup.git" |
| target   | target directory        |    ✅    | "~/machine_setup"                           |

##### example

```yaml
clone:
  url: "git@github.com:Chroma91/machine_setup.git"
  target: "~/machine_setup"
```

#### symlink

This command symlinks all the files from the source directory to the target directory.

| argument | value                               | required | example                           |
| -------- | ----------------------------------- | :------: | --------------------------------- |
| src      | source directory/file               |    ✅    | "./src/files" or "./src/test.txt" |
| target   | target directory/file               |    ✅    | "/tmp/target" or "/tmp/new.txt"   |
| ignore   | list of files/directories to ignore |    ➖    | ["dist", "package-lock.json"]     |
| force    | true/false                          |    ➖    |                                   |

> If `force` is set to `true`, existing files will be **removed** and replaced by the symlinks.

##### example

```yaml
symlink:
  src: "./src/files"
  target: "/tmp/target"
  ignore: ["dist", "package-lock.json"]
  force: true
```

#### run

This command executes a shell command.

> Hint: Avoid the usage of interactive commands when possible.

| argument | description           | required | default | values        |
| -------- | --------------------- | :------: | ------- | ------------- |
| env      | environment variables |    ➖    |         |               |
| shell    | shell that is used    |    ➖    | "bash"  | "bash", "zsh" |

By default, shell commands will be skipped when updating or uninstalling.
You can change that by prodiving `update` and/or `uninstall`.

The following arguments can be passed to `commands`:

| argument  | description              | required | example                         |
| --------- | ------------------------ | :------: | ------------------------------- |
| install   | command for installing   |    ➖    | "sudo apt-get -y install git"   |
| update    | command for updating     |    ➖    | "sudo apt-get -y upgrade git"   |
| uninstall | command for uninstalling |    ➖    | "sudo apt-get -y uninstall git" |

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
    commands:
      install: "sudo apt-get -y install git"
      update: "sudo apt-get -y upgrade git"
      uninstall: "sudo apt-get -y uninstall git"

updatable_multiline_command:
  run:
    env:
      SOME_TOKEN: "abc123"
    commands:
      install:
        - "sudo apt update"
        - "sudo apt-get -y install git"
      update:
        - "sudo apt-get -y upgrade git"
        - ...
      uninstall:
        - "sudo apt-get -y uninstall git"
        - ...
```

#### machine_setup

With this command it's possible to include other `machine_setup` configuration files.

| argument | description                             | required | example                  |
| -------- | --------------------------------------- | :------: | ------------------------ |
| config   | path to the other config file           |    ✅    | "./my_other_config.yaml" |
| task     | define a single task that should be run |    ➖    | "my_other_task"          |

##### example

```yaml
machine_setup:
  config: "./my_other_config.yaml"
  task: "my_other_task"
```
