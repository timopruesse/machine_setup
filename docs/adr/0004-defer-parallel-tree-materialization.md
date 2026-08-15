# Parallel file apply inside Tree materialization

Directory installs create parents sequentially (WalkDir order), then apply
collected files on the **shared** Rayon pool owned by the Concurrency gate
(sized by `num_threads`, default: physical CPUs − 1; created on first pool
use). Sibling Command entries share that pool so `parallel: true` tasks do
not oversubscribe. Threshold:
fewer than 32 files stay sequential. SudoFs walks stay single-threaded (script
batch on flush). Symlink stays sequential (metadata-cheap) — remeasured 2026-08
(DirectFs 1k-file install: parallel ~2.5× slower than sequential). No second
concurrency knob.

## Deferred: chunked / streaming file lists

Install/uninstall still collect the full file (or dest) list, then apply.
Chunked or streaming apply (to cut peak `PathBuf` memory on huge trees) is
**deferred** until Command bench with a large fixture (or a real Config
document) shows memory or allocator pain worth complicating the
mkdir-then-apply invariant.
