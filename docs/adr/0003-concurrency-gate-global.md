# One Concurrency gate for Command entries (shared with nested Sub-configs)

`num_threads` caps in-flight leaf Command executor work under one semaphore
(default: physical CPUs − 1). Permits are acquired per Command entry — not per
Task — so sequential and `parallel: true` Tasks share the same rule. The
`machine_setup` Command entry does **not** occupy a slot while it runs; nested
Sub-config Runners share the parent's gate and their leaf commands acquire
normally. That avoids a deadlock when `num_threads: 1` and a parent Task would
otherwise hold the only permit across nested work. Sync File ops (`copy` /
`symlink`) run via `spawn_blocking`. We accepted that a fat `parallel: true`
Task can starve siblings; per-Task sub-quotas are a future fix. The gate also
owns a shared Rayon pool of the same size for in-tree DirectFs file apply
(ADR-0004) so sibling commands do not each spawn a private worker set. The
pool is created lazily on first tree-apply use, not when the gate is built.

## Deferred: separate tree-apply admission

Leaf permits and the shared Rayon pool stay one width today. A second admission
knob (or exclusive tree-apply slot) so multiple `pool.install` callers cannot
oversubscribe the same workers is **deferred** until Command bench
(`parallel_two_copy_tasks_1k` or a successor) shows contention or unfairness
worth the complexity. Do not split the knobs preemptively.
