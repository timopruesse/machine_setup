# One Concurrency gate for Command entries (shared with nested Sub-configs)

`num_threads` caps in-flight leaf Command executor work under one semaphore
(default: physical CPUs − 1). Permits are acquired per Command entry — not per
Task — so sequential and `parallel: true` Tasks share the same rule. The
`machine_setup` Command entry does **not** occupy a slot while it runs; nested
Sub-config Runners share the parent's gate and their leaf commands acquire
normally. That avoids a deadlock when `num_threads: 1` and a parent Task would
otherwise hold the only permit across nested work. Sync File ops (`copy` /
`symlink`) run via `spawn_blocking`. We accepted that a fat `parallel: true`
Task can starve siblings; per-Task sub-quotas are a future fix. A dedicated FS
thread pool waits until in-tree parallel apply needs it.
