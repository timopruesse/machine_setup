# Defer parallel file apply inside Tree materialization

In-tree concurrent `on_file` for DirectFs is deferred until Command bench
baselines show sequential walk/apply as the next cliff after SudoFs hybrid and
the Concurrency gate. Keep the `install_tree` / `uninstall_tree` interface;
parallelism would be implementation-only. Do not introduce a second concurrency
knob independent of `num_threads`.
