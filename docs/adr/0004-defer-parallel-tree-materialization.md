# Parallel file apply inside Tree materialization

Directory installs create parents sequentially (WalkDir order), then apply
collected files on the **shared** Rayon pool owned by the Concurrency gate
(sized by `num_threads`, default: physical CPUs − 1; created on first pool
use). Sibling Command entries share that pool so `parallel: true` tasks do
not oversubscribe. Threshold:
fewer than 32 files stay sequential. SudoFs walks stay single-threaded (script
batch on flush). Symlink stays sequential (metadata-cheap). No second
concurrency knob.
