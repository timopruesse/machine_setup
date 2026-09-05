# One Concurrency gate for Command entries (shared with nested Sub-configs)

Amended by ADR-0010: the gate also owns **Exclusive lanes** (intra-run
package-manager families). That is exclusivity for OS package tools, not
tree-apply admission.

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

## Tree-apply admission (accepted 2026-09-05 — not yet implemented)

A second semaphore on the Concurrency gate admits **at most one** concurrent
tree `pool.install` (K=1). Leaf Command permits and Exclusive lanes are
unchanged. This prevents sibling tree Command entries from oversubscribing the
shared Rayon pool. Splitting pool width or reusing package-manager Exclusive
lanes for trees was rejected. K>1 remains a possible later knob; default is
exclusive tree-apply.
