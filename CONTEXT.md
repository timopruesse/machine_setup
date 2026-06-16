# machine_setup Context

The domain and architecture vocabulary for `machine_setup` — a declarative
machine-configuration tool that runs tasks defined in YAML/JSON. This file is
the canonical naming reference; prefer these terms (and avoid their listed
aliases) in code, comments, and reviews.

## Language

### Configuration

**Task**:
A named unit of setup work, made of an ordered list of commands, with optional
OS filter, conditions, dependencies, and retry.
_Avoid_: job, step, action.

**Command entry**:
One declarative operation inside a task — `copy`, `symlink`, `clone`, `run`, or
`machine_setup`. The `CommandEntry` enum in the config.
_Avoid_: step, instruction. (Do **not** shorten to "command" — see Flagged
ambiguities.)

**Sub-config**:
A nested configuration pulled in by a `machine_setup` command entry and executed
by its own runner one nesting level deeper.
_Avoid_: child config, included config.

### Execution

**Mode**:
The execution intent applied to a run — `Install`, `Update`, or `Uninstall`.
Derived once from the CLI verb; the only verbs the engine acts on.
_Avoid_: action, command, operation.

**Runner**:
The component that orders tasks, applies skip rules, and drives each task's
command entries to completion, emitting events as it goes (`TaskRunner`).
_Avoid_: engine (too broad), executor (means something narrower here).

**Command executor**:
The thing that runs one command entry for the current mode — one per command
entry type, behind the `CommandExecutor` interface (single `execute` method).
_Avoid_: handler, command (see Flagged ambiguities).

**Task event**:
A message emitted by the runner describing execution progress, consumed by the
TUI or the plain logger (`TaskEvent`).
_Avoid_: message, log, signal.

**History**:
The persisted record of which tasks are currently installed, used to skip
already-installed tasks unless forced.
_Avoid_: state, cache, ledger.

### Architecture seams

These name the deepened modules introduced to concentrate behavior; reviews
should refer to them by these names.

**Task graph**:
The single home for everything derived from `depends_on` edges — topological
order, parallel layers, cycle detection, and missing-edge detection
(`TaskGraph`). Both the runner and the validator read from it.
_Avoid_: dependency resolver, DAG, scheduler.

**File ops**:
The privilege seam for filesystem primitives (`mkdir`, copy, symlink, removal),
with two adapters — **DirectFs** (`std::fs`) and **SudoFs** (`sudo`) — chosen
once per command from its `sudo` flag (`FileOps`).
_Avoid_: fs helper, file utils.

**Tree materialization**:
The shared traversal behind `copy` and `symlink`: destination resolution (the
file-vs-directory target rule) plus the install/uninstall walk, parameterized by
a per-file operation.
_Avoid_: file walker, copier.

## Relationships

- A **Task** contains one or more **Command entries** and may declare
  dependencies on other tasks.
- The **Task graph** orders **Tasks**; the **Runner** executes them in that
  order under one **Mode**.
- The **Runner** turns each **Command entry** into a **Command executor** and
  calls `execute`, which acts according to the current **Mode**.
- The `copy` and `symlink` **Command executors** drive **Tree materialization**,
  which performs each file operation through a **File ops** adapter.
- A `machine_setup` **Command entry** loads a **Sub-config** and runs it with a
  nested **Runner**.
- The **Runner** emits **Task events** and consults/updates **History**.

## Example dialogue

> **Dev:** "When the runner hits a `copy` command entry in uninstall mode, who
> decides whether to use sudo?"
> **Maintainer:** "The copy executor picks a **File ops** adapter once from the
> entry's `sudo` flag, then hands it to **tree materialization**. The executor
> and the traversal never branch on sudo again — that decision lives behind the
> seam."
>
> **Dev:** "And the file-vs-directory target rule?"
> **Maintainer:** "That's `resolve_single_file_dest` inside tree
> materialization — one pure function, shared by copy and symlink, install and
> uninstall."

## Flagged ambiguities

- **"Command" was overloaded three ways** — resolved into three distinct terms:
  - **Command entry** (`CommandEntry`): a declarative op in the config
    (`copy`/`symlink`/…).
  - **Mode** (`Mode`): the execution intent (install/update/uninstall) — split
    out from the CLI `Command` so engine code stops carrying dead arms for
    non-execution verbs (`list`/`validate`/`completions`).
  - **CLI command** (`cli::Command`): the clap subcommand the user types,
    including non-execution verbs. Maps to a **Mode** for execution verbs only.
  Use the qualified term; never a bare "command".
- **"Engine" vs "Runner"** — the crate has an `engine` module, but the executing
  component is the **Runner**. Say "runner" for the thing that runs tasks.
