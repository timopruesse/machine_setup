# SudoFs hybrid: shared apply_tree walk, privilege still at executors

## Status (updated 2026-09-05 — design accepted, not yet implemented)

Tree-shaped installs deepen File ops with a shared **`apply_tree`** entry
point: one walk/chunk/apply loop parameterized by an already-chosen
`&dyn FileOps`, pool, and progress. Copy/symlink executors still select
DirectFs vs SudoFs and may keep the bulk-sudo short-circuit *before* calling
`apply_tree` (they know `sudo`, ignore, and mode).

Privileged directory `copy` may take a bulk path when eligible (empty ignore,
no mtime-skip semantics); otherwise SudoFs script-batches per-file ops and
flushes once. Symlink sudo stays script-batched only.

Moving bulk/privilege *policy* fully into File ops (planner owns privilege) was
considered and rejected for this reopen — smaller seam, less ADR surface.
Revisit only if executor short-circuits remain the friction after `apply_tree`
lands.
