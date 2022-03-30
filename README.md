# Machine Setup

## Configure

Tasks can be defined under the `tasks` root key.  
Every task can contain an arbitrary number of commands.

> _Hint_  
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
You can provide the path to another file via the `-c` flag (or `--config`).

If you only want to run a single command, `-t` (or `--task`) and the name of the task can be set for any of the terminal commands.

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

### clone

### symlink

### shell

---

## TODOs

- Theoretically, other config formats can be used. However, a lot of the types are still hardcoded to Yaml...
