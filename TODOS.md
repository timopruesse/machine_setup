# TODOs

- Add first class support for extending/including other configs (currently, output is not shown with default log level when doing it "interception" style)
- Add release script (Makefile) that automatically tags, etc.

- Add option to run `copy` and `symlink` as root user (needed to move/link some system files)

## 2.0.0

[BREAKING]

- all tasks run in parallel by default
- `dependsOn` can be set for tasks to make them wait for other tasks

## Next

- conditional tasks
  - add the ability to add requirements for a task, e.g. only run if a certain file doesn't exist
