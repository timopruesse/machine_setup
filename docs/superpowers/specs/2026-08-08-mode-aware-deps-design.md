# Mode-aware dependency expansion for `-t` / `-s`

Date: 2026-08-08  
Status: approved  
Repo: `timopruesse/machine_setup`  
Approach: **1** — resolve the final task set in a pre-runner step; runner only orders

## Context

`-t` / `--task` and `-s` / `--select` build a seed list of task names. Today
`TaskGraph::topo_order` always unions that list with the full transitive
`depends_on` closure for every mode (install, update, uninstall). That is
correct for install (deps must exist) but surprising for update (“refresh this
leaf”) and dangerous for uninstall (tearing down a shared base).

## Goals

- **Install:** always expand transitive deps (unchanged user-visible default).
- **Update:** run exactly the selected set unless `--with-deps`.
- **Uninstall:** run exactly the selected set by default; on an interactive TTY
  (and not `--no-tui`), offer a multi-select of remaining transitive deps (none
  pre-checked). `--with-deps` expands without prompting. Non-interactive /
  `--no-tui` never expands and never prompts.
- **Uninstall order:** dependents before dependencies whenever the run set is
  ordered (fix today’s “deps first” bug for full uninstalls too).
- **Shared-dep warning (uninstall):** if a task in the run set is still listed
  in `depends_on` of a task *outside* the run set, warn. TTY (and not
  `--no-tui`): confirm to proceed; decline aborts. Non-TTY / `--no-tui`: warn
  and continue.
- Cover with unit + non-interactive integration tests.

## Non-goals

- Per-mode `depends_on` in YAML.
- Nested `machine_setup` / sub-config dependency policy changes.
- Prompting on update.
- Using install history for the shared-dep check (config graph only).

## Decisions (locked)

| Topic | Choice |
| --- | --- |
| Default asymmetry | Install expands; update/uninstall exact |
| Escape hatch | Global `--with-deps` (no-op on install) |
| Uninstall extra deps UX | `dialoguer::MultiSelect`, none pre-checked, candidates = closure − selected |
| When to prompt | `stdin` is a TTY **and** `!cli.no_tui` |
| Shared-dep warning | Advisory + TTY confirm; non-TTY / `--no-tui` warn-and-continue |
| Shared-dep basis | Config graph edges, not history |
| Architecture | Resolve in library helper called from `main`; runner does not expand |
| Uninstall parallel order | Build forward layers, then **reverse the layer list** |

## Behavior

### CLI

New global flag: `--with-deps`.

| Mode | `-t` / `-s` / all | `--with-deps` |
| --- | --- | --- |
| Install | Always expand transitive closure | No-op |
| Update | Exact selection | Expand closure |
| Uninstall | Exact; optional interactive multi-select of missing transitive deps | Expand closure (skip multi-select) |

“All tasks” (no `-t`/`-s`) already includes every name → no uninstall multi-select
(empty candidate list). Expansion is a no-op when the closure ⊆ selected.

### Resolve pipeline (before runtime / TUI)

1. Build seed list from `-t` / `-s` / all keys (existing).
2. Apply mode policy → final run set (may prompt for uninstall extras).
3. Uninstall shared-dep check on the final run set (may confirm or warn).
4. Pass concrete `Vec<String>` into the engine. No further expansion.

Interactive prompts run in `main` **before** TUI start (same as today’s `-s`).

### Graph / runner

- `TaskGraph::closure(seeds) -> Result<Vec<String>>` — transitive deps including seeds (or a documented equivalent). Used by resolve.
- `TaskGraph::topo_order(requested)` — order **within** `requested` only; edges to tasks outside the set are ignored for ordering (do not pull them in). Missing/cycle errors still apply for edges among members / unknown names encountered while walking declared deps of members when computing closure.
- `TaskGraph::dependents_outside(run_set) -> Vec<(String, Vec<String>)>` — for each task in `run_set`, list config tasks outside `run_set` that `depends_on` it (for the warning).
- Runner: `topo_order` then build layers (parallel or one-task layers). If mode is uninstall, **reverse the layer list** so dependents run first. Sequential uninstall is the reversed degenerate layers.

### Shared-dep warning copy (informative, not prescribed verbatim)

Warn that uninstalling `base` may affect `leaf` (and any other outside dependents). TTY confirm; on decline, exit without running tasks.

## Testing

- Unit: `closure`; `topo_order` does not add unselected deps; uninstall layer reverse; `dependents_outside`; uninstall multi-select candidate list (`closure − selected`).
- Integration via `run_tasks` (not only `run_all`): update exact vs `--with-deps` simulated by pre-expanded lists; install still expands at resolve; uninstall exact vs expanded with reverse order; non-TTY shared-dep warn path tested at the pure helper level.
- No interactive `dialoguer` in CI.

## Files (expected)

- `src/cli.rs` — `--with-deps`
- `src/config/graph.rs` — closure, order-within-set, dependents_outside; update unit tests
- `src/config/selection.rs` (new) — pure resolve policy helpers
- `src/config/mod.rs` — export selection
- `src/engine/runner.rs` — no expansion; reverse layers on uninstall
- `src/main.rs` — wire resolve + prompts
- `README.md` — flag + brief mode note
- `tests/integration.rs` — selection/order cases
- `CHANGELOG.md` — user-facing note

## Open follow-ups (out of scope)

None locked for this change.
