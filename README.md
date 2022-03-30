# Machine Setup

## Configure

Tasks can be defined under the `tasks` root key.  
Every task can contain an arbitrary number of commands.

> **Hint**  
> Currently, there can only be one command of the same type per task.  
> The last command in a task will take precedence.  
> This is an open bug and will be fixed in a future release.

Check out the example configuration below:

```yaml
tasks:
  my_task1:
    copy:
      src: "./src/files"
      target: "/tmp/target"

  my_task2:
    shell: "sudo apt-get install git -y"

    symlink:
      src: "./src/config"
      target: "~/.dotfiles"
```

## Run

By default, `machine_setup` will look for a file called `machine_setup.yaml`.

| flag            | value                                       | example                                           |
| --------------- | ------------------------------------------- | ------------------------------------------------- |
| -c<br> --config | specify a different path to the config file | `machine_setup install -c ./config/my_setup.yaml` |
| -t<br> --task   | only run the specified task                 | `machine_setup install -t my_task2`               |

### Install

```bash
machine_setup install
```

### Update

```bash
machine_setup update
```

### Uninstall

```bash
machine_setup uninstall
```

## Available commands

### copy

This command copies the contents of a directory to another directory.

> Hint: `.git`, `.gitignore`, and `.gitmodules` is automatically ignored. You should consider `clone` when working with git repositories.

| argument | value                               | required | example                       |
| -------- | ----------------------------------- | :------: | ----------------------------- |
| src      | source directory                    |    ✅    | "./src/files"                 |
| target   | target directory                    |    ✅    | "/tmp/target"                 |
| ignore   | list of files/directories to ignore |    ➖    | ["dist", "package-lock.json"] |

#### example

```yaml
copy:
  src: "./src/files"
  target: "/tmp/target"
  ignore: ["dist", "package-lock.json"]
```

### clone

This command clones a git repository to the specified destination.

| argument | value                   | required | example                                     |
| -------- | ----------------------- | :------: | ------------------------------------------- |
| url      | URL to a git repository |    ✅    | "git@github.com:Chroma91/machine_setup.git" |
| target   | target directory        |    ✅    | "~/machine_setup"                           |

#### example

```yaml
clone:
  url: "git@github.com:Chroma91/machine_setup.git"
  target: "~/machine_setup"
```

### symlink

This command symlinks all the files from the source directory to the target directory.

| argument | value                               | required | example                       |
| -------- | ----------------------------------- | :------: | ----------------------------- |
| src      | source directory                    |    ✅    | "./src/files"                 |
| target   | target directory                    |    ✅    | "/tmp/target"                 |
| ignore   | list of files/directories to ignore |    ➖    | ["dist", "package-lock.json"] |

#### example

```yaml
symlink:
  src: "./src/files"
  target: "/tmp/target"
  ignore: ["dist", "package-lock.json"]
```

### shell

This command executes a shell command.

> Hint: Avoid the usage of interactive commands when possible.

| argument  | value                    | required | example                         |
| --------- | ------------------------ | :------: | ------------------------------- |
| install   | command for installing   |    ➖    | "sudo apt-get -y install git"   |
| update    | command for updating     |    ➖    | "sudo apt-get -y upgrade git"   |
| uninstall | command for uninstalling |    ➖    | "sudo apt-get -y uninstall git" |

#### example

```yaml
inline_command:
  shell: "sudo apt-get -y install git"

multiline_command:
  shell:
    - "sudo apt-get update"
    - "sudo apt-get -y install git"

updatable_command:
  shell:
    install: "sudo apt-get -y install git"
    update: "sudo apt-get -y upgrade git"
    uninstall: "sudo apt-get -y uninstall git"
```

---

## TODOs

- Theoretically, other config formats can be used. However, a lot of the types are still hardcoded to Yaml...
