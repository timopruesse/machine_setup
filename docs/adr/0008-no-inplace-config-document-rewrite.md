# Config document structural edits

## Status

**Partially reopened (2026-09-05):** `remove task` and `replace task` / `replace recipe`
ship via serde full-file rewrite in `config::document_edit`. Append-only authoring
remains in `config::document` (`init`, `add task`, recipes, wizard). Recipe and blank
stub emitters now build typed `TaskConfig` (`EmittedTask { name, task }`); `add`
serializes a YAML fragment and appends (still refuses duplicate names).

**Still deferred:** comment-preserving YAML surgery.

## Decision

- **Remove:** load → mutate `AppConfig` → `serde_yaml` dump → prune History.
  Dependents must be auto-fixed (TTY prompt or `--fix-deps`) or the remove aborts.
- **Replace (upsert):** load → optional TTY confirm when the Task exists → IndexMap
  upsert (preserves key order on overwrite) → `serde_yaml` dump. Missing name creates
  the Task with a warning; History is unchanged.
- **Append path:** unchanged; does not round-trip the file.

## Consequences

`remove task` and `replace` may drop comments and reformat the Config document. Users
who care about hand-tuned YAML should edit by hand or avoid structural edits. `add`
remains append-only at the UX level.
