# SudoFs hybrid lives in copy/symlink executors (not FileOps apply_tree yet)

Privileged directory `copy` may take a bulk path when eligible (empty ignore,
no mtime-skip semantics); otherwise SudoFs script-batches per-file ops and
flushes once. Symlink sudo stays script-batched only. Executors choose bulk vs
`install_tree` + flush because they already know `sudo`, ignore, and mode.
Deepening **File ops** toward a shared `apply_tree` planner is deferred — reopen
when executor privilege branches become the friction, not before.
