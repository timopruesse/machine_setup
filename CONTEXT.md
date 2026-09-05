# machine_setup Context

The domain and architecture vocabulary for `machine_setup` — a declarative
machine-configuration tool that runs tasks defined in YAML/JSON. This file is
the canonical naming reference; prefer these terms (and avoid their listed
aliases) in code, comments, and reviews.

## Language

### Configuration

**Config document**:
The user-authored YAML/JSON file that declares root settings and Tasks. Distinct
from the loaded in-memory config and from History. Authoring creates (`init`),
appends (`add`), or structurally rewrites via serde (`remove`, `replace` in
`document_edit` — comments/formatting may change; ADR-0008). Comment-preserving
YAML edit stays deferred (a dedicated crate would not simplify the serde load
path used by the Runner).
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
in the Config document. Initial recipes are registered in the **recipe catalog**
(`config/recipes.rs`): `dotfiles`, `git-repo`, `brew-bundle`. CLI `add recipe`
and the **Config wizard** dispatch emitters through that catalog — not a plugin
loader (ADR-0006).
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
entry type. Concrete kinds implement the `CommandExecutor` interface; the
catalog's `create_executor` returns the closed `Executor` enum (static
dispatch — ADR-0006, no plugin `dyn` until a second adapter exists). The
Runner may **lazy-cache** `Arc<Executor>` lists per Task on first run
(`TaskRunner`-local; Config document types stay pure data) so parallel
Command entries share refcounts instead of re-cloning args each time.
_Avoid_: handler, command (see Flagged ambiguities).

**Command kind catalog**:
The single owner of Command-entry-kind behavior — parse helpers used after
deserialize, validate, `create_executor`, `requires_sudo`, unattended sudo
demotion, **Exclusive lane** inference from `run` script text, and display
wiring co-located with Command executors. The `CommandEntry` enum stays public
for exhaustiveness; Deserialize may match keys only to construct the enum.
Modules outside the catalog must not `match` on variants for behavior. New
kinds register here once. A new kind is justified only when the op needs
**Tree materialization**, **File ops**, **Sub-config** nesting, or Mode
semantics `run` cannot express — not for YAML sugar over shell recipes
(ADR-0006).
_Avoid_: command registry, plugin map, dispatcher (unless a second adapter
justifies a real plugin seam — see ADR-0006).

**Task event**:
A message describing execution progress (`TaskEvent`) — lifecycle and
per-line/per-file output alike. `task_name` is an `Arc<str>` interned once per
Task so per-line output clones a refcount, not a new allocation. Emitted
through the **Task event sink**; the TUI and plain logger consume events from
the channel-backed adapter.
_Avoid_: message, log, signal.

**History**:
The persisted record of which tasks are currently installed, used to skip
already-installed tasks unless forced.
_Avoid_: state, cache, ledger.

**Task status**:
The join of a Task as defined in the Config document with History (and OS
applicability): whether it is defined, installed, skipped for this OS, and
related timestamps. Presented via `list` and `doctor`. Doctor also surfaces
validate issues and History entries with no matching Task (orphans); `--fix`
may prune orphans.
_Avoid_: task state, install ledger view.

### Architecture seams

These name the deepened modules introduced to concentrate behavior; reviews
should refer to them by these names.

**Task condition**:
Declarative gate on whether a Task runs — `only_if` (all must pass) and
`skip_if` (any triggers skip). Each field accepts a path string, a list of
path strings (backward compatible), or rich objects: `{ path }`, `{ env }`,
`{ command }`, `{ mode }`. Evaluated in `engine::conditions` together with OS
filter and install History skip; the Runner calls `evaluate_skip` before
spawning work.
_Avoid_: when clause, if guard, predicate.

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
emit through this interface — not a raw sender. Subprocess line readers may
coalesce stdout/stderr into a **`CommandOutputBatch`** Task event before
emit (single-line `CommandOutput` remains for sparse progress); batching is
an engine policy, not a third sink adapter.
_Avoid_: logger, event bus, observer.

**Concurrency gate**:
The Runner's global cap on in-flight leaf Command executor work, driven by
`num_threads` (default: physical CPUs − 1). Permits are per Command entry;
`machine_setup` does not hold a permit so nested Sub-configs can share the
gate. Sync File ops work runs via `spawn_blocking` so it does not block Tokio
workers. Owns a shared Rayon FS apply pool (same size as the permit limit),
created lazily on first tree-apply use. Also owns **Exclusive lanes** (package
managers) and a separate **tree-apply** K=1 semaphore so only one
`pool.install` runs at a time (tree-apply does not reuse Exclusive lanes).
Does **not** order Tasks by dependency — that remains the **Task graph**.
_Avoid_: scheduler (do not call the FS pool the "scheduler").

**Exclusive lane**:
A named slot on the **Concurrency gate**; at most one Command entry in the
run holds a given lane at a time. Nested Sub-config Runners share the
parent's lanes with the parent's gate. A `run` Command entry joins a lane
by inference from its script text — authors do not declare lanes. Lanes are
per package-manager family (apt, brew, dnf, pacman, apk, winget, choco),
matching the real OS exclusive resource, not one global package-manager slot.
_Avoid_: mutex, lock, apt lock, resource lock, scheduler, exclusive group.

**Details pane**:
The run TUI module that resolves and renders Task output — single-task log,
**Runner grid** during parallel bursts, or expanded full log (`Enter`).
View resolution and scroll/follow policy live here; `reduce` updates Task state
only. The ratatui widget is an adapter behind the Details pane interface.
_Avoid_: log panel, log view, merge mode.

**Runner grid**:
The Details pane layout shown while ≥2 Tasks are `Running`: up to four fixed
bands (one per runner), each showing command progress and a scrolling tail of
that Task's log. Overflow runners appear as a count in the title bar.
_Avoid_: merge stream, multiplex log, parallel log.

**File ops**:
The privilege seam for filesystem primitives (`mkdir`, copy, symlink, removal),
with two adapters — **DirectFs** (`std::fs`) and **SudoFs** (`sudo`) — chosen
once per command from its `sudo` flag (`FileOps`). SudoFs may script-batch
per-file ops and flush once; eligible directory `copy` installs may use a bulk
privileged path. Symlink sudo stays script-batched. Tree-shaped work also
exposes a shared **`apply_tree`** entry (walk/chunk/apply) that takes an
already-selected File ops adapter — privilege policy stays with the executor.
_Avoid_: fs helper, file utils.

**Tree materialization**:
The shared traversal behind `copy` and `symlink`: destination resolution (the
file-vs-directory target rule) plus the install/uninstall walk, parameterized by
a per-file operation. Ignore patterns (exact component names, `foo/bar` path
sequences, glob-lite `*`/`?` within a component — not substring-anywhere) skip
matching files and do not descend into matching directories (the walk root is
never ignored). Directory installs mkdir sequentially, then apply files on the
Concurrency gate's shared Rayon pool (ADR-0004). Peak PathBuf memory is capped
by **chunked apply**: a single walk accumulates paths until the PathBuf list
estimate would exceed the `tree_measure` gate, then flushes/applies and
continues (whole trees under the gate stay one chunk). Applies to all
list-collecting Tree materialization paths (DirectFs and SudoFs).
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
soft regression thresholds are a human call, not CI. Memory harness
(`examples/tree_memory_harness.rs`) is separate from Criterion: 100k DirectFs
install, peak RSS + PathBuf estimate, `PASS`/`RECOMMEND_CHUNK` against ADR-0004
thresholds; also report-only. Opt-in Criterion size via
`MACHINE_SETUP_BENCH_TREE_SIZE=10000`.
_Avoid_: performance test (ambiguous with correctness tests), profiling.

## Relationships

- A **Task** contains one or more **Command entries** and may declare
  dependencies on other tasks.
- The **Task graph** orders **Tasks**; the **Runner** executes them in that
  order under one **Mode**, admitting work through the **Concurrency gate**.
  Nested **Sub-config** Runners share the parent's **Concurrency gate** and
  **Exclusive lanes**. A `run` Command entry joins a lane by inference from
  its script text (one family per matching package manager). A Command entry
  joins at most one lane — the first match in a stable family order. Command
  entries that share a lane serialize on it while still occupying a gate permit.
  The **Runner** acquires the lane first, then the permit, so waiters do not
  occupy a slot.   Dual-family scripts belong in two Command entries. Waiting
  on a lane is visible as a lifecycle **Task event** (admission, not executor
  chatter), emitted after `CommandStarted` and only when the lane is already
  held — not a silent hang and not an event on the uncontended path
  (ADR-0010).
- The **Runner** turns each **Command entry** into a **Command executor** via the
  **Command kind catalog** and calls `execute`, which acts according to the
  current **Mode**.
- Validation, `requires_sudo`, and **Exclusive lane** inference for
  **Command entries** go through the **Command kind catalog** — not ad-hoc
  matches in foreign modules. The **Concurrency gate** waits on the inferred
  lane; the **Runner** admits (lane first, then permit) before `execute`. The
  `run` Command executor does not know about lanes.
- The `copy` and `symlink` **Command executors** use the **Tree-op driver**, which
  drives **Tree materialization** through a **File ops** adapter (executors may
  still choose a bulk SudoFs path when eligible).
- A `machine_setup` **Command entry** loads a **Sub-config** and runs it with a
  nested **Runner**. Optional `force` bypasses **History** skip in that nested
  run; optional `with_deps` (when `task` is set) expands transitive
  `depends_on` like CLI `--with-deps`.
- The **Runner** and Command executors emit **Task events** through the
  **Task event sink** and consult/update **History**.
- The **Command bench** exercises Tree materialization, File ops, the Runner,
  and **Command kind catalog** registration (registry microbench) across the
  same seams production uses (NullSink in smoke runs).
- The **Config locator** chooses a **Config document** when `-c` is omitted.
  **Config document** authoring (`init` / `add task`) appends only; the
  **Config schema** is generated for editors and must stay in sync with the
  **Command kind catalog** kind keys. **Task status** joins Tasks with History
  for `list` / `doctor`. Splitting files for execution remains **Sub-config**
  (ADR-0007) — not load-time include. In-place Task rewrite is rejected
  (ADR-0008). **Authoring recipes** and the **Config wizard** append via the
  Config document module, sharing the **recipe catalog** in `config/recipes.rs`.

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
>
> **Dev:** "Two parallel Tasks both `apt install` — who waits?"
> **Maintainer:** "The **Command kind catalog** infers family `apt` from the
> script. The **Runner** takes that **Exclusive lane** on the **Concurrency
> gate** before a permit. The second Command entry emits a wait Task event
> only because the lane is already held. The `run` executor never sees the
> lane. Unattended-upgrades is out of scope."

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
  Non-execution verbs now include `list`, `validate`, `doctor`, `init`, `wizard`,
  `add`, `schema`, and `completions`.
- **"Engine" vs "Runner"** — the crate has an `engine` module, but the executing
  component is the **Runner**. Say "runner" for the thing that runs tasks.
