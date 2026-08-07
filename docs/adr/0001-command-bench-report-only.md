# Command bench budgets are report-only

We measure Command executor / Tree materialization / Runner wall-clock with
Criterion (**Command bench**) but do **not** fail CI on absolute millisecond
budgets. Machine and WSL noise make hard asserts flaky. Soft Criterion baseline
comparison in CI is a deliberate follow-up once we have stable local numbers —
not part of the first land.
