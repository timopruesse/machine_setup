# TODOs

- save timestamps (at least for install), so the next install only runs new tasks
  only update/uninstall should affect already run tasks

---

- Fix homebrew/tap setup
- Add option to run `copy` and `symlink` as root user (needed to move/link some system files)

## 2.0.0

[BREAKING]

- all tasks run in parallel by default
- `dependsOn` can be set for tasks to make them wait for other tasks

## Next

- conditional tasks
  - add the ability to add requirements for a task, e.g. only run if a certain file doesn't exist

### Low priority tasks

- release script
  - copy markdown changelog from tag to GitHub release automatically
- create command that installs (or updates) shell completions
- refactor progress logic
