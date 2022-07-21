# TODOs

## Step 1

- make it possible to specify that all commands in a task should be run in parallel (this might be useful when cloning/updating a chunk of repos for example)

## Step 2

- make it possible to run tasks in parallel (not only commands inside)
- Improve the terminal output
  - hide most of it behind a `--verbose` flag

## Step 3

- Add option to run `copy` and `symlink` as root user (needed to move/link some system files)

## 2.0.0

[BREAKING]

- all tasks run in parallel by default
- `dependsOn` can be set for tasks to make them wait for other tasks

## Next

- conditional tasks
  - add the ability to add requirements for a task, e.g. only run if a certain file doesn't exist
