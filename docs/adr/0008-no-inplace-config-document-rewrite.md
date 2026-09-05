# Config document structural edits

## Status

**Partially reopened (2026-09-05):** `remove task` ships via serde full-file
rewrite in `config::document_edit`. Append-only authoring remains in
`config::document` (`init`, `add task`, recipes, wizard).

**Still deferred:** upsert / replace of an existing Task block; comment-preserving
YAML surgery.

## Decision

- **Remove:** load → mutate `AppConfig` → `serde_yaml` dump → prune History.
  Dependents must be auto-fixed (TTY prompt or `--fix-deps`) or the remove aborts.
- **Upsert:** still deferred until a concrete need appears.
- **Append path:** unchanged; does not round-trip the file.

## Consequences

`remove task` may drop comments and reformat the Config document. Users who care
about hand-tuned YAML should edit by hand or avoid `remove`.
