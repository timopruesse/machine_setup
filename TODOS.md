# TODOs

- Improve the terminal output
  - use https://crates.io/crates/indicatif for task/command progress
  - use https://docs.rs/pretty_env_logger/0.4.0/pretty_env_logger/ for messages/logging

---

- Add option to run `copy` and `symlink` as root user (needed to move/link some system files)

## 2.0.0

[BREAKING]

- all tasks run in parallel by default
- `dependsOn` can be set for tasks to make them wait for other tasks

## Next

- conditional tasks
  - add the ability to add requirements for a task, e.g. only run if a certain file doesn't exist
