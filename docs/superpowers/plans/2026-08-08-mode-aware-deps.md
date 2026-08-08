# Mode-Aware Dependency Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `-t`/`-s` expand `depends_on` only for install by default; update/uninstall stay exact unless `--with-deps` (uninstall may interactively multi-select deps); reverse uninstall layer order; warn on shared deps.

**Architecture:** Pure resolve helpers in `config/selection.rs` + graph closure/`dependents_outside`; `main` wires prompts; `TaskRunner` orders the concrete list and reverses layers on uninstall.

**Tech Stack:** Rust, clap, dialoguer, existing `TaskGraph` / `TaskRunner` / integration harness.

**Spec:** `docs/superpowers/specs/2026-08-08-mode-aware-deps-design.md`

## Global Constraints

- No interactive dialoguer in CI tests.
- `--no-tui` disables prompts even on a TTY.
- Uninstall parallel safety: reverse the **layer list**, do not re-`layers()` after reversing names.
- Parent/worker must not invent per-mode YAML `depends_on`.

---

### Task 1: Graph API — closure, order-within-set, dependents_outside

**Files:**
- Modify: `src/config/graph.rs`
- Test: unit tests in `src/config/graph.rs`

**Interfaces:**
- Produces:
  - `TaskGraph::closure(&self, seeds: &[String]) -> Result<Vec<String>>` — seeds ∪ transitive deps (stable-ish order not required; must include all reachable).
  - `TaskGraph::topo_order` — **within `requested` only** (no expansion).
  - `TaskGraph::dependents_outside(&self, run_set: &HashSet<str> or &[String]) -> Vec<(String, Vec<String>)>` — `(task_in_set, outside_dependents)`.

- [ ] **Step 1: Rewrite failing unit test for non-expanding topo**

Change `test_topo_orders_dependencies_first` so requesting only `c` yields only `c` (or order containing only `c`). Add `test_closure_pulls_transitive` expecting `a`,`b`,`c` from seed `c`. Add `test_topo_orders_within_selected_set` requesting `[c,a]` with middle `b` missing from set. Add `test_dependents_outside`.

- [ ] **Step 2: Run tests — expect old expand assumption to fail**

Run: `cargo test -p machine_setup graph::tests -- --nocapture`  
(or `cargo test test_topo_orders_dependencies_first`)

- [ ] **Step 3: Implement closure + change topo_order + dependents_outside**

`topo_order`: set `needed` = intersection of requested names that exist in the task map (preserve only requested; unknown requested names: either pass through as today or error — match current behavior for unknown tasks at runner level). Do **not** BFS into deps outside `requested`. Count in-degree only for edges where both ends ∈ needed.

`closure`: BFS from seeds like today’s expand loop; error on missing dep.

`dependents_outside`: for each task in run_set, scan all tasks not in run_set whose `depends_on` contains it.

- [ ] **Step 4: Run graph unit tests — expect PASS**

Run: `cargo test graph::`

---

### Task 2: Selection policy helpers

**Files:**
- Create: `src/config/selection.rs`
- Modify: `src/config/mod.rs` — `pub mod selection;`

**Interfaces:**
- Consumes: `TaskGraph`, `Mode`, `AppConfig`
- Produces:
  - `pub struct SelectionFlags { pub with_deps: bool, pub interactive: bool }`
  - `pub enum AbortReason { UserDeclinedSharedDeps }`
  - `pub fn plan_run_set(config: &AppConfig, selected: Vec<String>, mode: Mode, flags: SelectionFlags) -> Result<PlannedRun, Error>` where `PlannedRun { tasks: Vec<String>, shared_dep_warnings: Vec<(String, Vec<String>)> }` — **pure**: expands per mode policy but does **not** prompt; for uninstall without with_deps, does **not** add deps (candidates exposed separately).
  - `pub fn uninstall_extra_dep_candidates(config: &AppConfig, selected: &[String]) -> Result<Vec<String>>` — `closure(selected) − selected`, sorted for stable UI.
  - `pub fn apply_extra_deps(selected: Vec<String>, extras: Vec<String>) -> Vec<String>` — union.
  - After extras applied, caller recomputes `shared_dep_warnings` via `dependents_outside` on final set.

Simpler single function preferred if clean:

```rust
pub fn expand_for_mode(
    config: &AppConfig,
    selected: &[String],
    mode: Mode,
    with_deps: bool,
) -> Result<Vec<String>>;
// Install | with_deps => closure(selected)
// else => selected.to_vec()

pub fn uninstall_dep_candidates(...) -> Result<Vec<String>>;
pub fn shared_dep_warnings(config, run_set: &[String]) -> Vec<(String, Vec<String>)>;
```

- [ ] **Step 1: Unit tests for expand_for_mode / candidates / warnings**
- [ ] **Step 2: Implement until PASS**

Run: `cargo test selection::`

---

### Task 3: Runner — no expand, reverse uninstall layers

**Files:**
- Modify: `src/engine/runner.rs`
- Test: integration + any runner-level tests

**Interfaces:**
- Consumes: `topo_order` (within-set), `layers`
- Behavior: after building `layers` (parallel or singleton), if `self.mode == Mode::Uninstall { layers.reverse(); }`

- [ ] **Step 1: Integration test — uninstall full config runs dependent before dependency**

Config with `second depends_on first`, both echo + write markers or use event order. `Mode::Uninstall` via `run_all`: assert `second` completes before `first` starts.

- [ ] **Step 2: Integration — update run_tasks leaf only does not run dep**

Add helper `run_tasks_config(yaml, mode, &["leaf"])` calling `runner.run_tasks`. Assert dep not started on update; on install-with-pre-expanded list or after selection helper, dep runs.

Note: install expansion is in selection/main — integration can call `expand_for_mode` then `run_tasks` to mimic CLI.

- [ ] **Step 3: Implement layer reverse; confirm install ordering tests still pass**

Run: `cargo test --test integration depends_on`

---

### Task 4: CLI + main wiring + README/CHANGELOG

**Files:**
- Modify: `src/cli.rs` — `pub with_deps: bool`
- Modify: `src/main.rs` — after seed selection, call expand/candidates/prompts/warnings, then `run_execution`
- Modify: `README.md` — `--with-deps` row + short note under deps if present
- Modify: `CHANGELOG.md`

**Prompt wiring in main:**

```text
interactive = std::io::stdin().is_terminal() && !cli.no_tui
mode = Mode::from_command(...)
tasks = seed
if mode == Uninstall && !with_deps && interactive {
  candidates = uninstall_dep_candidates(...)
  if !candidates.is_empty() { multi-select; tasks = union }
} else {
  tasks = expand_for_mode(..., with_deps)?
}
warnings = shared_dep_warnings(..., &tasks)
if !warnings.is_empty() {
  print warning
  if interactive {
    if !Confirm::proceed { return Ok(()) }
  }
}
```

Install always `expand_for_mode` (with_deps ignored). Update: expand only if with_deps.

- [ ] **Step 1: Add flag + wire resolve before `run_execution`**
- [ ] **Step 2: `cargo test` + `make lint` (or fmt/clippy)**
- [ ] **Step 3: Docs**

---

## Spec coverage checklist

| Spec requirement | Task |
| --- | --- |
| Install expands | 2, 4 |
| Update exact / `--with-deps` | 2, 4 |
| Uninstall exact / prompt / `--with-deps` | 2, 4 |
| Reverse uninstall layers | 3 |
| Shared-dep warn + TTY confirm | 2, 4 |
| topo no longer expands | 1 |
| README / CHANGELOG | 4 |
