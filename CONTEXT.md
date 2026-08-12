# machine_setup Context

The domain and architecture vocabulary for `machine_setup` — a declarative
machine-configuration tool that runs tasks defined in YAML/JSON. This file is
the canonical naming reference; prefer these terms (and avoid their listed
aliases) in code, comments, and reviews.

## Language

### Configuration

**Config document**:
The user-authored YAML/JSON file that declares root settings and Tasks. Distinct
from the loaded in-memory config and from History. Authoring mutates it only by
creating a new file or appending a new Task — not by rewriting existing Tasks.
_Avoid_: setup file, machine file, config source.

**Config schema**:
The machine-readable description of a valid Config document (JSON Schema),
generated from the same types and Command kind catalog the loader uses. Used by
editors and by a CLI dump; not a second hand-written source of truth.
_Avoid_: config types, OpenAPI, JSON types.

**Config locator**:
The rule for choosing a Config document when no explicit path or URL is given:
look in the working directory, then at the git repository root, using the usual
filename and extension probe. Explicit `-c` paths and URLs bypass it.
_Avoid_: config discovery, config search path, XDG config.

**Authoring recipe**:
A named emitter that appends one or more Tasks built only from existing Command
entry kinds (`clone`, `symlink`, `run`, …). Not a new kind and not YAML sugar
in the Config document. Initial recipes: `dotfiles`, `brew-bundle`, `git-repo`.
_Avoid_: plugin, template pack, command kind.

**Config wizard**:
The interactive (TTY) adapter for creating a Config document if needed and
appending blank Tasks or Authoring recipes through prompts. Non-interactive
authoring stays on `init` / `add`.
_Avoid_: setup TUI, interactive init.

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

**Command kind catalog**:
The single owner of Command-entry-kind behavior — parse helpers used after
deserialize, validate, `create_executor`, `requires_sudo`, and display wiring
co-located with Command executors. The `CommandEntry` enum stays public for
exhaustiveness; Deserialize may match keys only to construct the enum.
Modules outside the catalog must not `match` on variants for behavior. New
kinds register here once. A new kind is justified only when the op needs
**Tree materialization**, **File ops**, **Sub-config** nesting, or Mode
semantics `run` cannot express — not for YAML sugar over shell recipes
(ADR-0006).
_Avoid_: command registry, plugin map, dispatcher (unless a second adapter
justifies a real plugin seam — see ADR-0006).

**Task event**:
A message describing execution progress (`TaskEvent`) — lifecycle and
per-line/per-file output alike. Emitted through the **Task event sink**; the
TUI and plain logger consume events from the channel-backed adapter.
_Avoid_: message, log, signal.

**History**:
The persisted record of which tasks are currently installed, used to skip
already-installed tasks unless forced.
_Avoid_: state, cache, ledger.

**Task status**:
The join of a Task as defined in the Config document with History (and OS
applicability): whether it is defined, installed, skipped for this OS, and
related timestamps. Presented first via `list`; a fuller doctor report is a
later adapter on the same module.
_Avoid_: task state, install ledger view.

### Architecture seams

These name the deepened modules introduced to concentrate behavior; reviews
should refer to them by these names.

**Task graph**:
The single home for everything derived from `depends_on` edges — transitive
closure, topological order within a run set, parallel layers, shared-dep
lookups, cycle detection, and missing-edge detection (`TaskGraph`). Mode-aware
expansion for `-t`/`-s` lives in `config::selection` (called from `main`); the
runner only orders the concrete list (and reverses layers on uninstall).
_Avoid_: dependency resolver, DAG, scheduler (do not reuse "scheduler" for
concurrency — see **Concurrency gate**).

**Task event sink**:
The seam for emitting **Task events**. Adapters: **ChannelSink** (mpsc →
TUI/plain) and **NullSink** (benches). The Runner and `CommandContext` both
emit through this interface — not a raw sender.
_Avoid_: logger, event bus, observer.

**Concurrency gate**:
The Runner's global cap on in-flight leaf Command executor work, driven by
`num_threads` (default: physical CPUs − 1). Permits are per Command entry;
`machine_setup` does not hold a permit so nested Sub-configs can share the
gate. Sync File ops work runs via `spawn_blocking` so it does not block Tokio
workers. Owns a shared Rayon FS apply pool (same size as the permit limit),
created lazily on first tree-apply use. Does **not** order Tasks by
dependency — that remains the **Task graph**.
_Avoid_: scheduler (do not call the FS pool the "scheduler").

**File ops**:
The privilege seam for filesystem primitives (`mkdir`, copy, symlink, removal),
with two adapters — **DirectFs** (`std::fs`) and **SudoFs** (`sudo`) — chosen
once per command from its `sudo` flag (`FileOps`). SudoFs may script-batch
per-file ops and flush once; eligible directory `copy` installs may use a bulk
privileged path. Symlink sudo stays script-batched.
_Avoid_: fs helper, file utils.

**Tree materialization**:
The shared traversal behind `copy` and `symlink`: destination resolution (the
file-vs-directory target rule) plus the install/uninstall walk, parameterized by
a per-file operation. Directory installs mkdir sequentially, then apply files on
the Concurrency gate's shared Rayon pool (ADR-0004).
_Avoid_: file walker, copier.

**Tree-op driver**:
The shared Command-executor shell around Tree materialization for tree-shaped
kinds — path expand, existence checks, `spawn_blocking`, File ops selection,
progress, flush. Kind-specific policy (bulk sudo, `force`, pool choice) stays in
thin per-kind Command executors that supply a per-file strategy. Does not move
privilege planning into File ops (ADR-0002).
_Avoid_: unified tree command, generic file command.

**Command bench**:
The measurement module for Command executor / Tree materialization / Runner
wall-clock speed — Criterion microbenches plus thin Runner smoke over
generate-once fixtures, plus a registry microbench (parse Command entries →
`create_executor`) so **Command kind catalog** changes have a signal when tree
benches barely move. Also tracks fixed bring-up (`TaskRunner::new`, empty-task
smoke). Process `--help` wall-clock stays outside Criterion (manual / OS
spawn). Report-only (no absolute ms CI asserts). SudoFs cases opt-in via
`MACHINE_SETUP_BENCH_SUDO=1`. Deepening steps capture before/after locally;
soft regression thresholds are a human call, not CI.
_Avoid_: performance test (ambiguous with correctness tests), profiling.

## Relationships

- A **Task** contains one or more **Command entries** and may declare
  dependencies on other tasks.
- The **Task graph** orders **Tasks**; the **Runner** executes them in that
  order under one **Mode**, admitting work through the **Concurrency gate**.
  Nested **Sub-config** Runners share the parent's **Concurrency gate**.
- The **Runner** turns each **Command entry** into a **Command executor** via the
  **Command kind catalog** and calls `execute`, which acts according to the
  current **Mode**.
- Validation and `requires_sudo` for **Command entries** go through the
  **Command kind catalog** — not ad-hoc matches in foreign modules.
- The `copy` and `symlink` **Command executors** use the **Tree-op driver**, which
  drives **Tree materialization** through a **File ops** adapter (executors may
  still choose a bulk SudoFs path when eligible).
- A `machine_setup` **Command entry** loads a **Sub-config** and runs it with a
  nested **Runner**.
- The **Runner** and Command executors emit **Task events** through the
  **Task event sink** and consult/update **History**.
- The **Command bench** exercises Tree materialization, File ops, the Runner,
  and **Command kind catalog** registration (registry microbench) across the
  same seams production uses (NullSink in smoke runs).
- The **Config locator** chooses a **Config document** when `-c` is omitted.
  **Config document** authoring (`init` / `add task`) appends only; the
  **Config schema** is generated for editors and must stay in sync with the
  **Command kind catalog** kind keys. **Task status** joins Tasks with History
  for `list`. Splitting files for execution remains **Sub-config** (ADR-0007) —
  not load-time include. **Authoring recipes** are deferred emitters on the
  Config document module.

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
  Non-execution verbs now include `list`, `validate`, `init`, `wizard`, `add`,
  `schema`, and `completions`.
- **"Engine" vs "Runner"** — the crate has an `engine` module, but the executing
  component is the **Runner**. Say "runner" for the thing that runs tasks.
